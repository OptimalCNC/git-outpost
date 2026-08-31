#[allow(dead_code)]
mod common;

#[cfg(target_os = "linux")]
use std::ffi::OsString;
use std::fs;
#[cfg(target_os = "linux")]
use std::os::unix::ffi::OsStringExt;
#[cfg(unix)]
use std::os::unix::fs::symlink;

use common::fixture::AbcFixture;
use outpost_core::{ConfigKey, OutpostError, SourceConfig, SourceStateStore};

#[test]
fn direct_state_write_reports_a_regular_state_parent() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let config_path = source.config_path();
    let state_dir = config_path.parent().expect("state directory");
    fs::write(state_dir, "not a directory").expect("state parent file");
    let container = fixture.root.join("containers");
    fs::create_dir(&container).expect("container");

    let error = source
        .state_store()
        .write_config(&SourceConfig {
            outpost_container: Some(container),
        })
        .expect_err("regular state parent must block config write");

    assert!(matches!(
        error,
        OutpostError::IoAt { path, source }
            if path == state_dir
                && matches!(
                    source.kind(),
                    std::io::ErrorKind::AlreadyExists | std::io::ErrorKind::NotADirectory
                )
    ));
}

#[test]
#[cfg(target_os = "linux")]
fn direct_state_write_reports_invalid_utf8_in_a_container_path() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let container = fixture
        .root
        .join(OsString::from_vec(b"containers-\xff".to_vec()));
    fs::create_dir(&container).expect("non-UTF-8 container");
    let config_path = source.config_path();

    let error = source
        .state_store()
        .write_config(&SourceConfig {
            outpost_container: Some(container),
        })
        .expect_err("non-UTF-8 path cannot be serialized as JSON");

    assert!(matches!(
        error,
        OutpostError::IoAt { path, source }
            if path == config_path
                && source.kind() == std::io::ErrorKind::Other
    ));
}

#[test]
fn direct_state_write_reports_a_directory_at_the_config_path() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let config_path = source.config_path();
    fs::create_dir_all(config_path.parent().expect("state directory")).expect("state directory");
    fs::create_dir(&config_path).expect("config directory");
    let container = fixture.root.join("containers");
    fs::create_dir(&container).expect("container");

    let error = source
        .state_store()
        .write_config(&SourceConfig {
            outpost_container: Some(container),
        })
        .expect_err("directory at config path must block replacement");

    assert!(matches!(
        error,
        OutpostError::IoAt { path, source }
            if path == config_path
                && matches!(
                    source.kind(),
                    std::io::ErrorKind::IsADirectory
                        | std::io::ErrorKind::AlreadyExists
                        | std::io::ErrorKind::PermissionDenied
                )
    ));
}

#[test]
#[cfg(unix)]
fn config_migration_reports_a_current_symlink_that_cannot_be_replaced() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let container = fixture.root.join("containers");
    fs::create_dir(&container).expect("container");
    let legacy_path = source.work_tree().join(".outpost/config.json");
    fs::create_dir_all(legacy_path.parent().expect("legacy directory")).expect("legacy directory");
    fs::write(
        &legacy_path,
        format!(
            "{{\"version\":1,\"outpost_container\":{}}}",
            serde_json::to_string(&container).expect("container json")
        ),
    )
    .expect("legacy config");

    let current_path = source.config_path();
    fs::create_dir_all(current_path.parent().expect("state directory")).expect("state directory");
    symlink(
        source.git_dir().join("missing-config-target"),
        &current_path,
    )
    .expect("current config symlink");

    let error = source
        .config()
        .get(ConfigKey::OutpostContainer)
        .expect_err("migration cannot replace a current symlink");

    assert!(matches!(
        error,
        OutpostError::IoAt { path, .. } if path == current_path
    ));
    assert!(legacy_path.is_file());
}
