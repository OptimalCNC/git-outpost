#[allow(dead_code)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use outpost_core::{
    ConfigKey, Metadata, Outpost, OutpostError, RawMetadata, RemoteName, SourceConfig,
    SourceStateStore,
};

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("canonical path")
}

#[test]
fn raw_metadata_read_rejects_invalid_managed_boolean() {
    let fixture = AbcFixture::new();
    let git = fixture.invoker(&fixture.source);
    git.run_check(["config", "--local", "outpost.managed", "maybe"])
        .expect("invalid marker");

    let error = RawMetadata::read(&git).expect_err("invalid managed marker must fail");

    assert!(matches!(
        error,
        OutpostError::BadMetadata { outpost, reason }
            if outpost == canonical(&fixture.source)
                && reason == "invalid outpost.managed value: maybe"
    ));
}

#[test]
fn raw_metadata_conversion_rejects_relative_source_path() {
    let fixture = AbcFixture::new();
    let raw = RawMetadata {
        managed: Some(true),
        source_repo: Some(PathBuf::from("relative-source")),
        remote_name: Some(RemoteName::parse("local").expect("remote")),
    };

    let error =
        Metadata::from_raw(&fixture.root, raw).expect_err("relative source path must be rejected");

    assert!(matches!(
        error,
        OutpostError::BadMetadata { outpost, reason }
            if outpost == fixture.root && reason == "source_repo must be an absolute path"
    ));
}

#[test]
fn metadata_write_keeps_missing_absolute_source_and_rejects_relative_source() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("outpost");
    let git = fixture.invoker(&outpost_path);
    let missing_source = fixture.root.join("source-that-does-not-exist");
    let metadata = Metadata {
        source_repo: missing_source.clone(),
        remote_name: RemoteName::parse("local").expect("remote"),
    };

    metadata
        .write(&git)
        .expect("missing absolute source is recordable");
    let stored = Outpost::at_with(&outpost_path, &fixture.git_env).expect("reopen outpost");
    assert_eq!(stored.metadata().source_repo, missing_source);

    let relative = Metadata {
        source_repo: PathBuf::from("relative-source"),
        remote_name: RemoteName::parse("local").expect("remote"),
    };
    let error = relative
        .write(&git)
        .expect_err("relative source must be rejected");
    assert!(matches!(
        error,
        OutpostError::BadMetadata { outpost, reason }
            if outpost == Path::new("relative-source")
                && reason == "source_repo must be an absolute path"
    ));
}

#[test]
fn opening_outpost_reports_metadata_read_io_error() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("outpost");
    let opened = Outpost::at_with(&outpost_path, &fixture.git_env).expect("opened outpost");
    let metadata_path = opened.metadata_path();
    fs::remove_file(&metadata_path).expect("remove metadata");
    fs::create_dir(&metadata_path).expect("metadata directory");

    let error = match Outpost::at_with(&outpost_path, &fixture.git_env) {
        Ok(_) => panic!("directory metadata path must fail to read"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        OutpostError::IoAt { path, .. } if path == metadata_path
    ));
}

#[test]
fn current_source_state_ignores_a_missing_legacy_file_in_an_existing_directory() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    source
        .state_store()
        .write_config(&SourceConfig {
            outpost_container: None,
        })
        .expect("current config");
    let legacy_dir = source.work_tree().join(".outpost");
    fs::create_dir_all(&legacy_dir).expect("legacy directory");

    assert_eq!(
        source
            .config()
            .get(ConfigKey::OutpostContainer)
            .expect("read current config"),
        None
    );
    assert!(legacy_dir.is_dir());
}
