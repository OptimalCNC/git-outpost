#[allow(dead_code)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use outpost_core::{Outpost, OutpostError};

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
fn metadata_unknown_fields_are_rejected() {
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
