#[allow(dead_code)]
mod common;

#[cfg(target_os = "linux")]
use std::ffi::OsString;
use std::fs;
#[cfg(target_os = "linux")]
use std::os::unix::ffi::OsStringExt;

use common::fixture::AbcFixture;
use outpost_core::{ConfigKey, ConfigValue, OutpostError};

#[test]
fn config_write_reports_a_regular_state_parent() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let config_path = source.config_path();
    let state_dir = config_path.parent().expect("state directory");
    fs::write(state_dir, "not a directory").expect("state parent file");
    let container = fixture.root.join("containers");
    fs::create_dir(&container).expect("container");

    let error = source
        .config()
        .set(
            ConfigKey::OutpostContainer,
            ConfigValue::OutpostContainer(container),
        )
        .expect_err("regular state parent must block config write");

    assert!(matches!(
        error,
        OutpostError::IoAt { path, source }
            if path == config_path && source.kind() == std::io::ErrorKind::NotADirectory
    ));
}

#[test]
#[cfg(target_os = "linux")]
fn config_write_reports_invalid_utf8_in_a_container_path() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let container = fixture
        .root
        .join(OsString::from_vec(b"containers-\xff".to_vec()));
    fs::create_dir(&container).expect("non-UTF-8 container");
    let config_path = source.config_path();

    let error = source
        .config()
        .set(
            ConfigKey::OutpostContainer,
            ConfigValue::OutpostContainer(container),
        )
        .expect_err("non-UTF-8 path cannot be serialized as JSON");

    assert!(matches!(
        error,
        OutpostError::IoAt { path, source }
            if path == config_path
                && source.kind() == std::io::ErrorKind::Other
    ));
}

#[test]
fn config_write_reports_a_directory_at_the_config_path() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let config_path = source.config_path();
    fs::create_dir_all(config_path.parent().expect("state directory")).expect("state directory");
    fs::create_dir(&config_path).expect("config directory");
    let container = fixture.root.join("containers");
    fs::create_dir(&container).expect("container");

    let error = source
        .config()
        .set(
            ConfigKey::OutpostContainer,
            ConfigValue::OutpostContainer(container),
        )
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
