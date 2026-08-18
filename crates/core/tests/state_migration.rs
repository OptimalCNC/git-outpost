#[allow(dead_code)]
mod common;

use std::fs;
use std::path::PathBuf;

use common::fixture::AbcFixture;
use outpost_core::ops::status::{ConfigProblem, StatusReport, run_with};
use outpost_core::{
    ConfigKey, Outpost, OutpostError, RegistryEntry, RemoteName, SourceConfig, SourceRepo,
    SourceStateStore,
};

const EMPTY_LEGACY_REGISTRY: &str = r#"{"version":1,"outposts":[]}"#;

#[test]
fn source_state_paths_are_under_the_exact_git_directory() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");

    assert_eq!(
        source.config_path(),
        source.git_dir().join("outpost/config.json")
    );
    assert_eq!(
        source.registry_path(),
        source.git_dir().join("outpost/registry.json")
    );
}

#[test]
fn source_state_writer_rejects_unvalidated_config_paths() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let err = source
        .state_store()
        .write_config(&SourceConfig {
            outpost_container: Some(PathBuf::from("relative-container")),
        })
        .expect_err("relative container must be rejected");

    assert!(matches!(err, OutpostError::BadConfig { .. }));
    assert!(!source.config_path().exists());
}

#[test]
fn legacy_source_config_is_migrated_on_first_read_and_removed() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let container = fixture.root.join("outposts");
    fs::create_dir(&container).expect("container");
    let legacy_path = source.work_tree().join(".outpost/config.json");
    fs::create_dir_all(legacy_path.parent().expect("legacy parent")).expect("legacy dir");
    let unrelated_path = source.work_tree().join(".outpost/keep.txt");
    fs::write(&unrelated_path, "keep\n").expect("unrelated legacy-directory file");
    fs::write(
        &legacy_path,
        format!(
            "{{\"version\":1,\"outpost_container\":{}}}",
            serde_json::to_string(&container).expect("container json")
        ),
    )
    .expect("legacy config");

    let value = source
        .config()
        .get(ConfigKey::OutpostContainer)
        .expect("read config")
        .expect("configured container");

    assert_eq!(
        value.to_string(),
        fs::canonicalize(&container).unwrap().display().to_string()
    );
    assert!(source.config_path().is_file());
    assert!(!legacy_path.exists());
    assert!(unrelated_path.is_file());
}

#[test]
fn current_source_config_cleans_legacy_file_left_by_previous_migration() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let current_container = fixture.root.join("current-outposts");
    let legacy_container = fixture.root.join("legacy-outposts");
    fs::create_dir(&current_container).expect("current container");
    fs::create_dir(&legacy_container).expect("legacy container");
    source
        .state_store()
        .write_config(&SourceConfig {
            outpost_container: Some(current_container.clone()),
        })
        .expect("current config");
    let legacy_path = source.work_tree().join(".outpost/config.json");
    fs::create_dir_all(legacy_path.parent().expect("legacy parent")).expect("legacy dir");
    fs::write(
        &legacy_path,
        format!(
            "{{\"version\":1,\"outpost_container\":{}}}",
            serde_json::to_string(&legacy_container).expect("container json")
        ),
    )
    .expect("legacy config");

    let value = source
        .config()
        .get(ConfigKey::OutpostContainer)
        .expect("read current config")
        .expect("configured container");

    assert_eq!(
        value.to_string(),
        fs::canonicalize(&current_container)
            .unwrap()
            .display()
            .to_string()
    );
    assert!(!legacy_path.exists());
}

#[cfg(unix)]
#[test]
fn current_source_config_cleanup_failure_keeps_both_states_and_retries() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let current_container = fixture.root.join("current-outposts");
    let legacy_container = fixture.root.join("legacy-outposts");
    fs::create_dir(&current_container).expect("current container");
    fs::create_dir(&legacy_container).expect("legacy container");
    source
        .state_store()
        .write_config(&SourceConfig {
            outpost_container: Some(current_container),
        })
        .expect("current config");
    let legacy_path = source.work_tree().join(".outpost/config.json");
    let legacy_dir = legacy_path.parent().expect("legacy parent");
    fs::create_dir_all(legacy_dir).expect("legacy dir");
    fs::write(
        &legacy_path,
        format!(
            "{{\"version\":1,\"outpost_container\":{}}}",
            serde_json::to_string(&legacy_container).expect("container json")
        ),
    )
    .expect("legacy config");
    fs::set_permissions(legacy_dir, fs::Permissions::from_mode(0o555))
        .expect("block legacy cleanup");

    let result = source.config().get(ConfigKey::OutpostContainer);

    fs::set_permissions(legacy_dir, fs::Permissions::from_mode(0o755))
        .expect("restore legacy directory");
    let error = result.expect_err("non-writable legacy directory must make cleanup fail");
    assert!(matches!(
        error,
        OutpostError::IoAt { path, source }
            if path == legacy_path && source.kind() == std::io::ErrorKind::PermissionDenied
    ));
    assert!(source.config_path().is_file());
    assert!(legacy_path.is_file());

    source
        .config()
        .get(ConfigKey::OutpostContainer)
        .expect("retry cleanup from current config");
    assert!(source.config_path().is_file());
    assert!(!legacy_path.exists());
}

#[cfg(unix)]
#[test]
fn failed_fresh_source_config_migration_keeps_legacy_for_retry() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let container = fixture.root.join("outposts");
    fs::create_dir(&container).expect("container");
    let legacy_path = source.work_tree().join(".outpost/config.json");
    fs::create_dir_all(legacy_path.parent().expect("legacy parent")).expect("legacy dir");
    fs::write(
        &legacy_path,
        format!(
            "{{\"version\":1,\"outpost_container\":{}}}",
            serde_json::to_string(&container).expect("container json")
        ),
    )
    .expect("legacy config");
    let current_path = source.config_path();
    let current_dir = current_path.parent().expect("current state parent");
    fs::create_dir_all(current_dir).expect("current state dir");
    fs::set_permissions(current_dir, fs::Permissions::from_mode(0o555))
        .expect("block current write");

    let result = source.config().get(ConfigKey::OutpostContainer);

    fs::set_permissions(current_dir, fs::Permissions::from_mode(0o755))
        .expect("restore current state directory");
    let error = result.expect_err("non-writable current directory must make migration fail");
    assert!(matches!(
        error,
        OutpostError::IoAt { source, .. }
            if source.kind() == std::io::ErrorKind::PermissionDenied
    ));
    assert!(!current_path.exists());
    assert!(legacy_path.is_file());

    source
        .config()
        .get(ConfigKey::OutpostContainer)
        .expect("retry source migration");
    assert!(current_path.is_file());
    assert!(!legacy_path.exists());
}

#[cfg(unix)]
#[test]
fn source_cleanup_refuses_a_symlinked_legacy_directory() {
    use std::os::unix::fs::symlink;

    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let container = fixture.root.join("outposts");
    fs::create_dir(&container).expect("container");
    source
        .state_store()
        .write_config(&SourceConfig {
            outpost_container: Some(container),
        })
        .expect("current config");
    let legacy_dir = source.work_tree().join(".outpost");
    let legacy_path = legacy_dir.join("config.json");
    symlink(source.git_dir().join("outpost"), &legacy_dir)
        .expect("symlink legacy directory to current state");

    let error = source
        .config()
        .get(ConfigKey::OutpostContainer)
        .expect_err("cleanup must reject a symlinked legacy directory");

    assert!(matches!(
        error,
        OutpostError::IoAt { path, source }
            if path == legacy_path && source.kind() == std::io::ErrorKind::InvalidInput
    ));
    assert!(source.config_path().is_file());
    assert!(
        fs::symlink_metadata(&legacy_dir)
            .expect("legacy directory symlink remains")
            .file_type()
            .is_symlink()
    );
}

#[test]
fn legacy_outpost_metadata_is_migrated_to_the_exact_git_directory() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let current = Outpost::at(&outpost).expect("current outpost");
    let metadata_path = current.metadata_path();
    fs::remove_file(&metadata_path).expect("remove current metadata");
    let git = fixture.invoker(&outpost);
    git.run_check(["config", "--local", "outpost.managed", "true"])
        .expect("managed marker");
    git.run_check([
        "config",
        "--local",
        "outpost.sourceRepo",
        fixture.source.to_str().expect("source path"),
    ])
    .expect("source marker");
    git.run_check(["config", "--local", "outpost.remoteName", "local"])
        .expect("remote marker");
    git.run_check(["config", "--local", "outpost.unrelated", "keep"])
        .expect("unrelated config");

    let opened = Outpost::at(&outpost).expect("migrated outpost");

    assert!(metadata_path.is_file());
    assert_eq!(opened.metadata_path(), metadata_path);
    for key in [
        "outpost.managed",
        "outpost.sourceRepo",
        "outpost.remoteName",
    ] {
        assert!(
            !git.run_status(["config", "--local", "--get", key])
                .expect("query legacy key"),
            "legacy key remains: {key}"
        );
    }
    assert_eq!(
        git.run_capture(["config", "--local", "--get", "outpost.unrelated"])
            .expect("unrelated config remains"),
        "keep"
    );
}

#[test]
fn current_outpost_metadata_cleans_all_values_of_known_legacy_keys() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let current = Outpost::at(&outpost).expect("current outpost");
    let metadata_path = current.metadata_path();
    let git = fixture.invoker(&outpost);
    for (key, values) in [
        ("outpost.managed", ["maybe", "false"]),
        ("outpost.sourceRepo", ["relative-source", "/other-source"]),
        ("outpost.remoteName", ["invalid remote", "other"]),
    ] {
        for value in values {
            git.run_check(["config", "--local", "--add", key, value])
                .expect("add legacy value");
        }
    }

    let opened = Outpost::at(&outpost).expect("clean leftovers from current metadata");

    assert_eq!(opened.metadata_path(), metadata_path);
    assert!(metadata_path.is_file());
    for key in [
        "outpost.managed",
        "outpost.sourceRepo",
        "outpost.remoteName",
    ] {
        assert!(
            !git.run_status(["config", "--local", "--get-all", key])
                .expect("query all legacy values"),
            "legacy values remain: {key}"
        );
    }
}

#[test]
fn failed_outpost_cleanup_is_reported_and_retried_from_current_metadata() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let opened = Outpost::at(&outpost).expect("opened outpost");
    let metadata_path = opened.metadata_path();
    let config_lock = opened.git_dir().join("config.lock");
    fs::remove_file(&metadata_path).expect("remove current metadata");
    let git = fixture.invoker(&outpost);
    git.run_check(["config", "--local", "outpost.managed", "true"])
        .expect("managed marker");
    git.run_check([
        "config",
        "--local",
        "outpost.sourceRepo",
        fixture.source.to_str().expect("source path"),
    ])
    .expect("source marker");
    git.run_check(["config", "--local", "outpost.remoteName", "local"])
        .expect("remote marker");
    fs::write(&config_lock, "locked\n").expect("block config writes");

    let error = match Outpost::at(&outpost) {
        Ok(_) => panic!("locked config must make cleanup fail"),
        Err(error) => error,
    };
    assert!(matches!(error, OutpostError::GitFailed { .. }));
    assert!(metadata_path.is_file());
    assert!(
        git.run_status(["config", "--local", "--get", "outpost.managed"])
            .expect("legacy marker remains after failed cleanup")
    );

    fs::remove_file(config_lock).expect("unblock config writes");
    Outpost::at(&outpost).expect("retry cleanup from current metadata");
    for key in [
        "outpost.managed",
        "outpost.sourceRepo",
        "outpost.remoteName",
    ] {
        assert!(
            !git.run_status(["config", "--local", "--get", key])
                .expect("query cleaned key"),
            "legacy key remains after retry: {key}"
        );
    }
}

#[test]
fn source_status_propagates_outpost_cleanup_failure_and_retries() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let opened = Outpost::at(&outpost).expect("opened outpost");
    let config_lock = opened.git_dir().join("config.lock");
    let git = fixture.invoker(&outpost);
    git.run_check(["config", "--local", "outpost.managed", "true"])
        .expect("legacy marker");
    fs::write(&config_lock, "locked\n").expect("block config writes");

    let error = run_with(&fixture.source, &fixture.git_env)
        .expect_err("source status must report outpost cleanup failure");

    assert!(matches!(error, OutpostError::GitFailed { .. }));
    assert!(
        git.run_status(["config", "--local", "--get", "outpost.managed"])
            .expect("legacy marker remains after failed source status")
    );

    fs::remove_file(config_lock).expect("unblock config writes");
    assert!(matches!(
        run_with(&fixture.source, &fixture.git_env).expect("retry source status"),
        StatusReport::Source(_)
    ));
    assert!(
        !git.run_status(["config", "--local", "--get", "outpost.managed"])
            .expect("legacy marker cleaned on retry")
    );
}

#[test]
fn legacy_registry_is_migrated_and_removed() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let outpost = fixture.add_outpost("C").expect("outpost");
    let current_path = source.registry_path();
    fs::remove_file(&current_path).expect("remove current registry");
    let legacy_path = source.work_tree().join(".outpost/registry.json");
    fs::create_dir_all(legacy_path.parent().expect("legacy parent")).expect("legacy dir");
    let entry =
        RegistryEntry::new(outpost.clone(), RemoteName::parse("local").unwrap()).expect("entry");
    let legacy = format!(
        r#"{{
  "version": 1,
  "outposts": [{{
    "path": {path},
    "created_at": {created_at},
    "remote_name": "local",
    "locked": false,
    "lock_reason": null,
    "locked_at": null
  }}]
}}"#,
        path = serde_json::to_string(&entry.path).expect("path JSON"),
        created_at = serde_json::to_string(&entry.created_at).expect("created-at JSON"),
    );
    fs::write(&legacy_path, legacy).expect("legacy registry");

    let loaded = source.registry().expect("migrate registry");
    assert_eq!(loaded.entries().len(), 1);
    assert!(current_path.is_file());
    assert!(!legacy_path.exists());
}

#[test]
fn current_registry_cleans_legacy_file_left_by_previous_migration() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    fixture.add_outpost("C").expect("outpost");
    let legacy_path = source.work_tree().join(".outpost/registry.json");
    fs::create_dir_all(legacy_path.parent().expect("legacy parent")).expect("legacy dir");
    fs::write(&legacy_path, EMPTY_LEGACY_REGISTRY).expect("stale legacy registry");

    let loaded = source.registry().expect("read current registry");

    assert_eq!(loaded.entries().len(), 1);
    assert!(!legacy_path.exists());
}

#[cfg(unix)]
#[test]
fn current_registry_cleanup_failure_keeps_both_states_and_retries() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    fixture.add_outpost("C").expect("outpost");
    let legacy_path = source.work_tree().join(".outpost/registry.json");
    let legacy_dir = legacy_path.parent().expect("legacy parent");
    fs::create_dir_all(legacy_dir).expect("legacy dir");
    fs::write(&legacy_path, EMPTY_LEGACY_REGISTRY).expect("legacy registry");
    fs::set_permissions(legacy_dir, fs::Permissions::from_mode(0o555))
        .expect("block legacy cleanup");

    let result = source.registry();

    fs::set_permissions(legacy_dir, fs::Permissions::from_mode(0o755))
        .expect("restore legacy directory");
    let error = result.expect_err("non-writable legacy directory must make cleanup fail");
    assert!(matches!(
        error,
        OutpostError::IoAt { path, source }
            if path == legacy_path && source.kind() == std::io::ErrorKind::PermissionDenied
    ));
    assert!(source.registry_path().is_file());
    assert!(legacy_path.is_file());

    let loaded = source
        .registry()
        .expect("retry cleanup from current registry");
    assert_eq!(loaded.entries().len(), 1);
    assert!(source.registry_path().is_file());
    assert!(!legacy_path.exists());
}

#[cfg(unix)]
#[test]
fn failed_fresh_registry_migration_keeps_legacy_for_retry() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let legacy_path = source.work_tree().join(".outpost/registry.json");
    fs::create_dir_all(legacy_path.parent().expect("legacy parent")).expect("legacy dir");
    fs::write(&legacy_path, EMPTY_LEGACY_REGISTRY).expect("legacy registry");
    let current_path = source.registry_path();
    let current_dir = current_path.parent().expect("current state parent");
    fs::create_dir_all(current_dir).expect("current state dir");
    fs::set_permissions(current_dir, fs::Permissions::from_mode(0o555))
        .expect("block current write");

    let result = source.registry();

    fs::set_permissions(current_dir, fs::Permissions::from_mode(0o755))
        .expect("restore current state directory");
    let error = result.expect_err("non-writable current directory must make migration fail");
    assert!(matches!(
        error,
        OutpostError::IoAt { source, .. }
            if source.kind() == std::io::ErrorKind::PermissionDenied
    ));
    assert!(!current_path.exists());
    assert!(legacy_path.is_file());

    let loaded = source.registry().expect("retry registry migration");
    assert!(loaded.entries().is_empty());
    assert!(current_path.is_file());
    assert!(!legacy_path.exists());
}

#[test]
fn invalid_current_registry_does_not_fall_back_or_clean_legacy() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let current_path = source.registry_path();
    fs::create_dir_all(current_path.parent().expect("current parent")).expect("current dir");
    fs::write(&current_path, "{\n").expect("invalid current registry");
    let legacy_path = source.work_tree().join(".outpost/registry.json");
    fs::create_dir_all(legacy_path.parent().expect("legacy parent")).expect("legacy dir");
    fs::write(&legacy_path, EMPTY_LEGACY_REGISTRY).expect("legacy registry");

    let error = source
        .registry()
        .expect_err("invalid current registry must remain authoritative");

    assert!(matches!(error, OutpostError::BadRegistry { path, .. } if path == current_path));
    assert!(current_path.is_file());
    assert!(legacy_path.is_file());
}

#[test]
fn malformed_legacy_registry_is_retained_without_current_state() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let legacy_path = source.work_tree().join(".outpost/registry.json");
    fs::create_dir_all(legacy_path.parent().expect("legacy parent")).expect("legacy dir");
    fs::write(&legacy_path, "{\n").expect("malformed legacy registry");

    let error = source
        .registry()
        .expect_err("malformed legacy registry must be reported");

    assert!(matches!(error, OutpostError::BadRegistry { path, .. } if path == legacy_path));
    assert!(!source.registry_path().exists());
    assert!(legacy_path.is_file());
}

#[test]
fn source_config_migration_succeeds_independently_of_bad_legacy_registry() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let container = fixture.root.join("outposts");
    fs::create_dir(&container).expect("container");
    let legacy_dir = source.work_tree().join(".outpost");
    let legacy_config = legacy_dir.join("config.json");
    let legacy_registry = legacy_dir.join("registry.json");
    fs::create_dir_all(&legacy_dir).expect("legacy dir");
    fs::write(
        &legacy_config,
        format!(
            "{{\"version\":1,\"outpost_container\":{}}}",
            serde_json::to_string(&container).expect("container json")
        ),
    )
    .expect("legacy config");
    fs::write(&legacy_registry, "{\n").expect("malformed legacy registry");

    source
        .config()
        .get(ConfigKey::OutpostContainer)
        .expect("config migration succeeds");
    let registry_error = source
        .registry()
        .expect_err("malformed registry remains independent");

    assert!(source.config_path().is_file());
    assert!(!legacy_config.exists());
    assert!(matches!(
        registry_error,
        OutpostError::BadRegistry { path, .. } if path == legacy_registry
    ));
    assert!(!source.registry_path().exists());
    assert!(legacy_registry.is_file());
}

#[test]
fn invalid_new_metadata_does_not_fall_back_to_legacy_and_status_keeps_outpost_context() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let opened = Outpost::at(&outpost).expect("opened outpost");
    let metadata_path = opened.metadata_path();
    fs::write(
        &metadata_path,
        r#"{"version":1,"source_repo":"/source","remote_name":"local","extra":true}"#,
    )
    .expect("invalid current metadata");
    let git = fixture.invoker(&outpost);
    git.run_check(["config", "--local", "outpost.managed", "true"])
        .expect("legacy marker");

    let report = run_with(&outpost, &fixture.git_env).expect("diagnostic status");
    let StatusReport::Outpost(report) = report else {
        panic!("invalid current metadata must remain outpost context");
    };
    assert!(
        report
            .problems
            .iter()
            .any(|problem| matches!(problem, ConfigProblem::InvalidMetadata { .. }))
    );
    assert!(matches!(
        Outpost::at(&outpost),
        Err(OutpostError::BadMetadata { outpost: path, .. })
            if path == fs::canonicalize(&outpost).expect("canonical outpost")
    ));
    assert!(
        git.run_status(["config", "--local", "--get", "outpost.managed"])
            .expect("legacy marker remains")
    );
}

#[test]
fn false_legacy_marker_ignores_malformed_stale_fields() {
    let fixture = AbcFixture::new();
    let git = fixture.invoker(&fixture.source);
    git.run_check(["config", "--local", "outpost.managed", "false"])
        .expect("false marker");
    git.run_check(["config", "--local", "outpost.remoteName", "invalid remote"])
        .expect("stale remote field");

    let report = run_with(&fixture.source, &fixture.git_env).expect("source status");
    assert!(matches!(report, StatusReport::Source(_)));
}

#[test]
fn invalid_legacy_remote_is_reported_as_invalid_metadata() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let opened = Outpost::at(&outpost).expect("opened outpost");
    let metadata_path = opened.metadata_path();
    fs::remove_file(&metadata_path).expect("remove current metadata");
    let git = fixture.invoker(&outpost);
    git.run_check(["config", "--local", "outpost.managed", "true"])
        .expect("managed marker");
    git.run_check([
        "config",
        "--local",
        "outpost.sourceRepo",
        fixture.source.to_str().expect("source path"),
    ])
    .expect("source marker");
    git.run_check(["config", "--local", "outpost.remoteName", "invalid remote"])
        .expect("invalid remote marker");

    let report = run_with(&outpost, &fixture.git_env).expect("diagnostic status");
    let StatusReport::Outpost(report) = report else {
        panic!("invalid legacy metadata must remain outpost context");
    };
    assert!(
        report
            .problems
            .iter()
            .any(|problem| matches!(problem, ConfigProblem::InvalidMetadata { .. }))
    );
    assert!(!metadata_path.exists());
    assert!(
        git.run_status(["config", "--local", "--get", "outpost.remoteName"])
            .expect("invalid legacy metadata remains")
    );
}

#[test]
fn invalid_legacy_source_path_is_reported_without_creating_current_metadata() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let opened = Outpost::at(&outpost).expect("opened outpost");
    let metadata_path = opened.metadata_path();
    fs::remove_file(&metadata_path).expect("remove current metadata");
    let git = fixture.invoker(&outpost);
    git.run_check(["config", "--local", "outpost.managed", "true"])
        .expect("managed marker");
    git.run_check(["config", "--local", "outpost.sourceRepo", "relative-source"])
        .expect("relative source marker");
    git.run_check(["config", "--local", "outpost.remoteName", "local"])
        .expect("remote marker");

    let report = run_with(&outpost, &fixture.git_env).expect("diagnostic status");
    let StatusReport::Outpost(report) = report else {
        panic!("invalid legacy metadata must remain outpost context");
    };
    assert!(report.problems.iter().any(|problem| matches!(
        problem,
        ConfigProblem::InvalidMetadata { reason } if reason.contains("absolute")
    )));
    assert!(!metadata_path.exists());
    assert!(
        git.run_status(["config", "--local", "--get", "outpost.sourceRepo"])
            .expect("invalid legacy metadata remains")
    );
}

#[test]
fn status_migration_preserves_report_and_is_stable_on_the_second_read() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let expected = run_with(&outpost, &fixture.git_env).expect("current status");
    let opened = Outpost::at(&outpost).expect("opened outpost");
    let metadata_path = opened.metadata_path();
    fs::remove_file(&metadata_path).expect("remove current metadata");
    let git = fixture.invoker(&outpost);
    git.run_check(["config", "--local", "outpost.managed", "true"])
        .expect("managed marker");
    git.run_check([
        "config",
        "--local",
        "outpost.sourceRepo",
        fixture.source.to_str().expect("source path"),
    ])
    .expect("source marker");
    git.run_check(["config", "--local", "outpost.remoteName", "local"])
        .expect("remote marker");

    let first = run_with(&outpost, &fixture.git_env).expect("legacy status");
    let migrated_bytes = fs::read(&metadata_path).expect("migrated metadata");
    let second = run_with(&outpost, &fixture.git_env).expect("second status");

    assert_eq!(first, expected);
    assert_eq!(second, expected);
    assert!(
        !git.run_status(["config", "--local", "--get", "outpost.managed"])
            .expect("query cleaned status marker")
    );
    assert_eq!(
        fs::read(&metadata_path).expect("metadata remains"),
        migrated_bytes
    );
}

#[test]
fn git_clean_does_not_remove_private_state_in_git_directories() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let source = fixture.source_repo().expect("source");
    let container = fixture.root.join("outposts");
    fs::create_dir(&container).expect("container");
    source
        .config()
        .set(
            ConfigKey::OutpostContainer,
            outpost_core::ConfigValue::OutpostContainer(container),
        )
        .expect("source config");
    let config_path = source.config_path();
    let registry_path = source.registry_path();
    let metadata_path = Outpost::at(&outpost)
        .expect("opened outpost")
        .metadata_path();

    fixture
        .invoker(&fixture.source)
        .run_check(["clean", "-fdx"])
        .expect("clean source worktree");
    fixture
        .invoker(&outpost)
        .run_check(["clean", "-fdx"])
        .expect("clean outpost worktree");

    assert!(config_path.is_file());
    assert!(registry_path.is_file());
    assert!(metadata_path.is_file());
    assert!(Outpost::at(&outpost).is_ok());
}

#[test]
fn source_status_migrates_only_the_registered_outpost_it_inspects() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let expected = run_with(&fixture.source, &fixture.git_env).expect("current source status");
    let opened = Outpost::at(&outpost).expect("opened outpost");
    let metadata_path = opened.metadata_path();
    fs::remove_file(&metadata_path).expect("remove current metadata");
    let git = fixture.invoker(&outpost);
    git.run_check(["config", "--local", "outpost.managed", "true"])
        .expect("managed marker");
    git.run_check([
        "config",
        "--local",
        "outpost.sourceRepo",
        fixture.source.to_str().expect("source path"),
    ])
    .expect("source marker");
    git.run_check(["config", "--local", "outpost.remoteName", "local"])
        .expect("remote marker");

    let migrated = run_with(&fixture.source, &fixture.git_env).expect("source status");

    assert_eq!(migrated, expected);
    assert!(metadata_path.is_file());
}

#[test]
fn linked_source_worktrees_have_independent_state_directories() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let linked_path = fixture.root.join("linked-source");
    fixture
        .invoker(&fixture.source)
        .run_check([
            "worktree",
            "add",
            "-b",
            "linked-state",
            linked_path.to_str().expect("linked path"),
        ])
        .expect("linked worktree");
    let linked = SourceRepo::at_with(&linked_path, &fixture.git_env).expect("linked source");

    assert_ne!(source.git_dir(), linked.git_dir());
    assert_ne!(source.config_path(), linked.config_path());
    assert_eq!(
        linked.config_path(),
        linked.git_dir().join("outpost/config.json")
    );

    let container = fixture.root.join("linked-outposts");
    fs::create_dir(&container).expect("container");
    linked
        .config()
        .set(
            ConfigKey::OutpostContainer,
            outpost_core::ConfigValue::OutpostContainer(container),
        )
        .expect("linked config");
    assert!(!source.config_path().exists());
    assert!(linked.config_path().is_file());
}
