#[allow(dead_code)]
mod common;

use std::fs;
use std::path::PathBuf;

use common::fixture::AbcFixture;
use outpost_core::selector::{OutpostSelector, resolve_entry};
use outpost_core::{
    BranchName, ConfigKey, ConfigValue, OutpostError, OutpostId, OutpostIdPrefix, RefName,
    RemoteName, SourceRemoteRef, UpstreamRef,
};

#[test]
fn config_key_and_value_have_stable_text_forms() {
    let key = ConfigKey::parse("outpost-container").expect("known config key");
    assert_eq!(key.as_str(), "outpost-container");
    assert_eq!(key.to_string(), "outpost-container");

    let value = ConfigValue::OutpostContainer(PathBuf::from("/tmp/outposts"));
    assert_eq!(value.key(), key);
    assert_eq!(value.to_string(), "/tmp/outposts");
}

#[test]
fn config_read_reports_io_error_when_storage_path_is_a_directory() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let config_path = source.config().storage_path();
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config parent");
    fs::create_dir(&config_path).expect("directory at config path");

    let err = source
        .config()
        .get(ConfigKey::OutpostContainer)
        .expect_err("directory cannot be read as config text");
    assert!(matches!(err, OutpostError::IoAt { path, .. } if path == config_path));
}

#[test]
fn config_storage_rejects_non_string_and_missing_directory_values() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let config_path = source.config().storage_path();
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config parent");

    fs::write(&config_path, r#"{"version":1,"outpost_container":42}"#)
        .expect("write malformed config");
    let err = source
        .config()
        .get(ConfigKey::OutpostContainer)
        .expect_err("numeric path should be rejected");
    assert!(matches!(err, OutpostError::BadConfig { path, .. } if path == config_path));

    fs::write(
        &config_path,
        r#"{"version":1,"outpost_container":"/path/that/does/not/exist"}"#,
    )
    .expect("write missing-directory config");
    let err = source
        .config()
        .get(ConfigKey::OutpostContainer)
        .expect_err("missing directory should be rejected");
    assert!(matches!(err, OutpostError::BadConfig { path, .. } if path == config_path));
}

#[test]
fn refname_types_reject_empty_and_git_forbidden_values() {
    for parse in [
        BranchName::parse(""),
        BranchName::parse("-main"),
        BranchName::parse("main..broken"),
    ] {
        assert!(matches!(parse, Err(OutpostError::InvalidRefName { .. })));
    }
    for parse in [
        RefName::parse(""),
        RefName::parse("refs/heads/main..broken"),
        RefName::parse("refs/heads/main.lock"),
    ] {
        assert!(matches!(parse, Err(OutpostError::InvalidRefName { .. })));
    }
    for parse in [
        RemoteName::parse(""),
        RemoteName::parse("-origin"),
        RemoteName::parse("origin/name"),
    ] {
        assert!(matches!(parse, Err(OutpostError::InvalidRefName { .. })));
    }
}

#[test]
fn refname_composites_preserve_display_and_short_branch_semantics() {
    let source_ref = SourceRemoteRef::parse("origin/feature/topic").expect("source ref");
    assert_eq!(source_ref.remote.to_string(), "origin");
    assert_eq!(source_ref.branch.to_string(), "feature/topic");

    let upstream = UpstreamRef {
        remote: RemoteName::parse("origin").expect("remote"),
        merge_ref: RefName::parse("refs/remotes/origin/main").expect("remote ref"),
    };
    assert_eq!(upstream.short_branch(), None);
}

#[test]
fn outpost_id_and_prefix_cover_display_boundaries_and_matching() {
    let full = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let id = OutpostId::parse(full).expect("full id");
    let prefix = OutpostIdPrefix::parse("01234").expect("prefix");
    assert_eq!(id.to_string(), full);
    assert_eq!(prefix.to_string(), "01234");
    assert!(id.starts_with(&prefix));

    assert!(OutpostId::parse("").is_err());
    assert!(OutpostIdPrefix::parse(full).is_ok());
    assert!(OutpostIdPrefix::parse(format!("{full}0")).is_err());
    assert!(OutpostIdPrefix::parse("abcde-").is_err());
    assert_eq!(
        OutpostIdPrefix::parse("ABCDEF").unwrap().to_string(),
        "abcdef"
    );
}

#[test]
fn outpost_id_derivation_is_path_scoped_and_deterministic() {
    let root = tempfile::tempdir().expect("temp root");
    let source = root.path().join("source");
    let first = root.path().join("first");
    let second = root.path().join("second");
    let a = OutpostId::derive(&source, &first);
    assert_eq!(a, OutpostId::derive(&source, &first));
    assert_ne!(a, OutpostId::derive(&source, &second));
    assert_ne!(a, OutpostId::derive(root.path(), &first));
}

#[test]
fn selector_resolves_explicit_relative_path_and_reports_missing_bare_path() {
    let fixture = AbcFixture::new();
    let nested = fixture.root.join("nested");
    fs::create_dir(&nested).expect("nested dir");
    let outpost = fixture.add_outpost("nested/C").expect("nested outpost");
    let source = fixture.source_repo().expect("source repo");

    let explicit = OutpostSelector::from_cli_arg(&fixture.root, PathBuf::from("nested/C"));
    let resolved = resolve_entry(&source, &explicit).expect("explicit path selector");
    assert_eq!(
        resolved.path,
        fs::canonicalize(outpost).expect("canonical outpost")
    );

    let missing = OutpostSelector::from_cli_arg(&fixture.root, PathBuf::from("missing"));
    let err = resolve_entry(&source, &missing).expect_err("missing path selector");
    let expected = fs::canonicalize(&fixture.root)
        .expect("canonical fixture root")
        .join("missing");
    assert!(matches!(err, OutpostError::RegistryEntryNotFound(path) if path == expected));
}

#[cfg(unix)]
#[test]
fn selector_accepts_bare_hex_when_path_and_id_resolve_same_entry() {
    use std::os::unix::fs::symlink;

    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let source = fixture.source_repo().expect("source repo");
    let entry = source.registry().expect("registry").entries()[0].clone();
    let id = OutpostId::derive(source.work_tree(), &entry.path);
    let alias = fixture.root.join(&id.as_str()[..5]);
    symlink(&outpost, &alias).expect("symlink alias");

    let selector = OutpostSelector::from_cli_arg(&fixture.root, id.as_str()[..5].into());
    let resolved = resolve_entry(&source, &selector).expect("same path and id match");
    assert_eq!(resolved.path, entry.path);
}
