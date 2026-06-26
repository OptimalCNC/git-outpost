#[allow(dead_code)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use outpost_core::{ConfigEntry, ConfigKey, ConfigShowEntry, ConfigValue, OutpostError};

#[test]
fn missing_config_file_is_empty_config() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let store = source.config();

    assert_eq!(store.get(ConfigKey::OutpostContainer).unwrap(), None);
    assert!(store.list().unwrap().is_empty());
    assert!(!store.storage_path().exists());
}

#[test]
fn set_get_list_show_and_unset_round_trip() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let container = fixture.root.join("outposts");
    fs::create_dir(&container).expect("container dir");
    let canonical = fs::canonicalize(&container).expect("canonical container");
    let store = source.config();

    let saved = store
        .set(
            ConfigKey::OutpostContainer,
            ConfigValue::OutpostContainer(container.clone()),
        )
        .expect("set outpost container");

    assert_eq!(saved, ConfigValue::OutpostContainer(canonical.clone()));
    assert_eq!(
        store.get(ConfigKey::OutpostContainer).unwrap(),
        Some(ConfigValue::OutpostContainer(canonical.clone()))
    );
    assert_eq!(
        store.list().unwrap(),
        vec![ConfigEntry {
            key: ConfigKey::OutpostContainer,
            value: ConfigValue::OutpostContainer(canonical.clone()),
        }]
    );
    assert_eq!(store.show().unwrap().storage_path, store.storage_path());
    assert_eq!(
        store.show().unwrap().entries,
        vec![ConfigShowEntry {
            key: ConfigKey::OutpostContainer,
            value: Some(ConfigValue::OutpostContainer(canonical)),
        }]
    );

    store
        .unset(ConfigKey::OutpostContainer)
        .expect("unset outpost container");

    assert_eq!(store.get(ConfigKey::OutpostContainer).unwrap(), None);
    assert!(store.list().unwrap().is_empty());
    assert_eq!(
        store.show().unwrap().entries,
        vec![ConfigShowEntry {
            key: ConfigKey::OutpostContainer,
            value: None,
        }]
    );
    let file: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(store.storage_path()).expect("config file"))
            .expect("config json");
    assert_eq!(file, serde_json::json!({ "version": 1 }));
}

#[test]
fn config_save_uses_source_metadata_directory_and_local_ignore() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let container = fixture.root.join("outposts");
    fs::create_dir(&container).expect("container dir");
    let store = source.config();

    store
        .set(
            ConfigKey::OutpostContainer,
            ConfigValue::OutpostContainer(container),
        )
        .expect("set outpost container");

    assert_eq!(
        store.storage_path(),
        source.work_tree().join(".outpost").join("config.json")
    );
    assert!(
        fs::read_to_string(source.local_exclude_path_for_tests())
            .expect("local exclude")
            .lines()
            .any(|line| line == ".outpost/")
    );
}

#[test]
fn unknown_core_config_key_is_denied() {
    let err = ConfigKey::parse("unknown-key").expect_err("unknown key should fail");

    assert!(matches!(
        err,
        OutpostError::UnknownConfigKey { key } if key == "unknown-key"
    ));
}

#[test]
fn strict_storage_rejects_unknown_fields_unsupported_versions_and_bad_json() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let store = source.config();
    let container = fixture.root.join("outposts");
    fs::create_dir(&container).expect("container dir");
    let canonical = fs::canonicalize(&container).expect("canonical container");

    write_config(
        &store.storage_path(),
        &format!(
            r#"{{"version":1,"outpost_container":{},"extra":true}}"#,
            serde_json::to_string(&canonical).expect("path json")
        ),
    );
    assert_bad_config(store.get(ConfigKey::OutpostContainer).unwrap_err());

    write_config(
        &store.storage_path(),
        &format!(
            r#"{{"version":2,"outpost_container":{}}}"#,
            serde_json::to_string(&canonical).expect("path json")
        ),
    );
    assert_bad_config(store.get(ConfigKey::OutpostContainer).unwrap_err());

    write_config(&store.storage_path(), "{not json");
    assert_bad_config(store.get(ConfigKey::OutpostContainer).unwrap_err());

    write_config(
        &store.storage_path(),
        r#"{"version":1,"outpost_container":"relative/path"}"#,
    );
    assert_bad_config(store.get(ConfigKey::OutpostContainer).unwrap_err());

    write_config(
        &store.storage_path(),
        r#"{"version":1,"outpost_container":null}"#,
    );
    assert_bad_config(store.get(ConfigKey::OutpostContainer).unwrap_err());

    let file = fixture.root.join("not-a-directory");
    fs::write(&file, "file").expect("file");
    write_config(
        &store.storage_path(),
        &format!(
            r#"{{"version":1,"outpost_container":{}}}"#,
            serde_json::to_string(&file).expect("path json")
        ),
    );
    assert_bad_config(store.get(ConfigKey::OutpostContainer).unwrap_err());
}

#[test]
fn path_values_are_canonicalized_and_non_directories_are_rejected() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let store = source.config();
    let container = fixture.root.join("outposts");
    fs::create_dir(&container).expect("container dir");
    let file = fixture.root.join("not-a-directory");
    fs::write(&file, "file").expect("file");

    let saved = store
        .set(
            ConfigKey::OutpostContainer,
            ConfigValue::OutpostContainer(container.join(".")),
        )
        .expect("set relative container");
    assert_eq!(
        saved,
        ConfigValue::OutpostContainer(fs::canonicalize(&container).expect("canonical container"))
    );

    let err = store
        .set(
            ConfigKey::OutpostContainer,
            ConfigValue::OutpostContainer(file),
        )
        .expect_err("file is not valid container");
    assert!(matches!(err, OutpostError::InvalidConfigValue { .. }));
}

#[test]
fn source_git_config_outpost_container_is_neither_read_nor_written() {
    let fixture = AbcFixture::new();
    let legacy = fixture.root.join("legacy");
    let container = fixture.root.join("outposts");
    fs::create_dir(&legacy).expect("legacy dir");
    fs::create_dir(&container).expect("container dir");
    fixture
        .invoker(&fixture.source)
        .run_check([
            "config",
            "--local",
            "outpost.container",
            legacy.to_str().expect("legacy path"),
        ])
        .expect("write legacy git config");
    let source = fixture.source_repo().expect("source repo");
    let store = source.config();

    assert_eq!(store.get(ConfigKey::OutpostContainer).unwrap(), None);

    store
        .set(
            ConfigKey::OutpostContainer,
            ConfigValue::OutpostContainer(container),
        )
        .expect("set source config");

    let git_value = fixture
        .invoker(&fixture.source)
        .run_capture(["config", "--local", "--get", "outpost.container"])
        .expect("read legacy git config");
    assert_eq!(PathBuf::from(git_value), legacy);
}

fn write_config(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().expect("config parent")).expect("config dir");
    fs::write(path, contents).expect("write config");
}

fn assert_bad_config(err: OutpostError) {
    assert!(
        matches!(err, OutpostError::BadConfig { .. }),
        "expected BadConfig, got {err:?}"
    );
}
