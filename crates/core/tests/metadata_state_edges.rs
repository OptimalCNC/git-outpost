#[allow(dead_code)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use outpost_core::{
    ConfigKey, Metadata, Outpost, OutpostError, OutpostStateStore, RemoteName, SourceConfig,
    SourceStateStore, Stored,
};

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("canonical path")
}

fn metadata_json(path: &Path, contents: &str) {
    fs::write(path, contents).expect("metadata contents");
}

#[test]
fn absent_metadata_is_not_an_outpost() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let opened = Outpost::at_with(&outpost, &fixture.git_env).expect("opened outpost");
    fs::remove_file(opened.metadata_path()).expect("remove metadata");

    assert!(matches!(
        Outpost::at_with(&outpost, &fixture.git_env),
        Err(OutpostError::NotAnOutpost(path)) if path == canonical(&outpost)
    ));
}

#[test]
fn metadata_validation_reports_version_path_remote_and_json_errors() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let opened = Outpost::at_with(&outpost, &fixture.git_env).expect("opened outpost");
    let path = opened.metadata_path();
    let outpost_path = canonical(&outpost);
    let source_path = canonical(&fixture.source);

    let cases = [
        ("{".to_owned(), None),
        (
            format!(
                r#"{{"version":2,"source_repo":{},"remote_name":"local"}}"#,
                serde_json::to_string(&source_path).expect("serialize source path")
            ),
            Some("unsupported metadata version 2"),
        ),
        (
            r#"{"version":1,"source_repo":"relative","remote_name":"local"}"#.to_owned(),
            Some("source_repo must be an absolute path"),
        ),
        (
            format!(
                r#"{{"version":1,"source_repo":{},"remote_name":"bad remote"}}"#,
                serde_json::to_string(&source_path).expect("serialize source path")
            ),
            Some("invalid ref name: bad remote"),
        ),
    ];

    for (contents, expected_reason) in cases {
        metadata_json(&path, &contents);
        let error = match Outpost::at_with(&outpost, &fixture.git_env) {
            Ok(_) => panic!("invalid metadata must be rejected"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            OutpostError::BadMetadata { outpost: actual, reason }
                if actual == outpost_path
                    && expected_reason.map_or(true, |expected| reason.contains(expected))
        ));
    }
}

#[test]
fn metadata_unknown_fields_are_rejected_without_legacy_fallback() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let opened = Outpost::at_with(&outpost, &fixture.git_env).expect("opened outpost");
    let contents = format!(
        r#"{{"version":1,"source_repo":{},"remote_name":"local","extra":true}}"#,
        serde_json::to_string(&canonical(&fixture.source)).expect("serialize source path")
    );
    metadata_json(&opened.metadata_path(), &contents);

    let error = match Outpost::at_with(&outpost, &fixture.git_env) {
        Ok(_) => panic!("unknown metadata fields must be rejected"),
        Err(error) => error,
    };
    assert!(matches!(error, OutpostError::BadMetadata { .. }));
}

#[test]
fn legacy_managed_boolean_spellings_control_migration() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let opened = Outpost::at_with(&outpost, &fixture.git_env).expect("opened outpost");
    fs::remove_file(opened.metadata_path()).expect("remove metadata");
    let git = fixture.invoker(&outpost);

    for value in ["true", "yes", "on", "1"] {
        git.run_check(["config", "--local", "outpost.managed", value])
            .expect("set managed marker");
        let error = match Outpost::at_with(&outpost, &fixture.git_env) {
            Ok(_) => panic!("managed marker without fields is invalid"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            OutpostError::BadMetadata { reason, .. } if reason.contains("missing outpost.sourceRepo")
        ));
    }

    for value in ["false", "no", "off", "0"] {
        git.run_check(["config", "--local", "outpost.managed", value])
            .expect("set unmanaged marker");
        assert!(matches!(
            Outpost::at_with(&outpost, &fixture.git_env),
            Err(OutpostError::NotAnOutpost(_))
        ));
    }

    git.run_check(["config", "--local", "outpost.managed", "maybe"])
        .expect("set invalid marker");
    let error = match Outpost::at_with(&outpost, &fixture.git_env) {
        Ok(_) => panic!("invalid managed marker must be rejected"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        OutpostError::BadMetadata { reason, .. }
            if reason == "invalid outpost.managed value: maybe"
    ));
}

#[test]
fn metadata_initialization_is_no_clobber_and_write_replaces() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let opened = Outpost::at_with(&outpost, &fixture.git_env).expect("opened outpost");
    let metadata = Metadata {
        source_repo: canonical(&fixture.source),
        remote_name: RemoteName::parse("local").expect("remote"),
    };
    fs::remove_file(opened.metadata_path()).expect("remove metadata");

    opened
        .state_store()
        .initialize_metadata(&metadata)
        .expect("initialize metadata");
    let error = opened
        .state_store()
        .initialize_metadata(&metadata)
        .expect_err("second initialization must not clobber");
    assert!(matches!(
        error,
        OutpostError::IoAt { path, source }
            if path == opened.metadata_path() && source.kind() == std::io::ErrorKind::AlreadyExists
    ));

    let replacement = Metadata {
        source_repo: canonical(&fixture.source),
        remote_name: RemoteName::parse("replacement").expect("remote"),
    };
    replacement
        .write(&fixture.invoker(&outpost))
        .expect("compatibility write replaces metadata");
    assert_eq!(
        Outpost::at_with(&outpost, &fixture.git_env)
            .expect("reopen outpost")
            .metadata()
            .remote_name
            .as_str(),
        "replacement"
    );
}

#[test]
fn source_state_store_distinguishes_absent_and_present_values() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let store = source.state_store();

    assert!(matches!(
        store.read_config().expect("read absent config"),
        Stored::Absent
    ));
    assert!(matches!(
        store.read_registry().expect("read absent registry"),
        Stored::Absent
    ));

    let container = fixture.root.join("containers");
    fs::create_dir(&container).expect("container");
    let config = SourceConfig {
        outpost_container: Some(container.clone()),
    };
    store.write_config(&config).expect("write config");
    assert!(matches!(
        store.read_config().expect("read config"),
        Stored::Present(actual) if actual.outpost_container == Some(canonical(&container))
    ));

    source.unset_outpost_container().expect("unset config");
    assert!(matches!(
        store.read_config().expect("read empty config"),
        Stored::Present(actual) if actual.outpost_container.is_none()
    ));
    assert!(source.config_path().is_file());

    let registry = source.registry().expect("load empty registry");
    store.write_registry(&registry).expect("write registry");
    assert!(matches!(
        store.read_registry().expect("read registry"),
        Stored::Present(_)
    ));
}

#[cfg(unix)]
#[test]
fn legacy_cleanup_rejects_symlinked_parent_and_directory_path() {
    use std::os::unix::fs::symlink;

    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let target = fixture.root.join("legacy-target");
    fs::create_dir(&target).expect("target dir");
    let legacy_dir = source.work_tree().join(".outpost");
    symlink(&target, &legacy_dir).expect("legacy parent symlink");
    fs::write(target.join("config.json"), r#"{"version":1}"#).expect("legacy config");
    source
        .state_store()
        .write_config(&SourceConfig {
            outpost_container: None,
        })
        .expect("current config");

    let error = source
        .config()
        .get(ConfigKey::OutpostContainer)
        .expect_err("symlink legacy parent must be rejected");
    assert!(matches!(
        error,
        OutpostError::IoAt { path, source }
            if path == legacy_dir.join("config.json")
                && source.kind() == std::io::ErrorKind::InvalidInput
    ));
    assert!(legacy_dir.is_symlink(), "legacy parent symlink must remain");

    fs::remove_file(source.config_path()).expect("remove current config");
    fs::remove_file(target.join("config.json")).expect("remove legacy config");
    fs::remove_file(&legacy_dir).expect("remove legacy symlink");
    fs::create_dir_all(&legacy_dir).expect("real legacy dir");
    fs::create_dir(legacy_dir.join("config.json")).expect("legacy directory path");
    fs::write(source.config_path(), r#"{"version":1}"#).expect("current config");
    let error = source
        .config()
        .get(ConfigKey::OutpostContainer)
        .expect_err("legacy directory path must be rejected");
    assert!(matches!(
        error,
        OutpostError::IoAt { path, source }
            if path == legacy_dir.join("config.json")
                && source.kind() == std::io::ErrorKind::InvalidInput
    ));
    assert!(
        legacy_dir.join("config.json").is_dir(),
        "legacy directory path must remain"
    );
}
