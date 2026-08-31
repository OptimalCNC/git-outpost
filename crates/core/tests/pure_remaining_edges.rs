#[allow(dead_code)]
mod common;

use std::fs;
use std::path::PathBuf;

use common::fixture::AbcFixture;
use outpost_core::selector::{OutpostSelector, resolve_entry};
use outpost_core::{
    BranchName, OutpostError, OutpostId, OutpostIdPrefix, RemoteName, SourceRemoteRef,
};

#[test]
fn outpost_id_prefix_normalizes_full_length_uppercase_input() {
    let prefix =
        OutpostIdPrefix::parse("ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789")
            .expect("full-length hexadecimal prefix should parse");

    assert_eq!(
        prefix.as_str(),
        "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
    );

    let id = OutpostId::parse(prefix.as_str()).expect("normalized prefix is a full id");
    assert!(id.starts_with(&prefix));
    let other = OutpostIdPrefix::parse("fffff").expect("other prefix");
    assert!(!id.starts_with(&other));
}

#[cfg(unix)]
#[test]
fn outpost_id_derivation_uses_raw_non_utf8_path_bytes() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let source = PathBuf::from(OsString::from_vec(b"source-\xff".to_vec()));
    let outpost = PathBuf::from(OsString::from_vec(b"outpost-\xfe".to_vec()));
    let replacement_source = PathBuf::from(OsString::from_vec(b"source-\xef\xbf\xbd".to_vec()));

    let id = OutpostId::derive(&source, &outpost);

    assert_eq!(id.as_str().len(), 64);
    assert_ne!(id, OutpostId::derive(&replacement_source, &outpost));
}

#[test]
fn refname_parsers_cover_allowed_remote_characters_and_composite_errors() {
    let remote = RemoteName::parse("remote._-9").expect("allowed remote characters");
    assert_eq!(remote.as_str(), "remote._-9");

    let branch = BranchName::parse("topic.with_underscores-9").expect("valid branch");
    assert_eq!(branch.to_string(), "topic.with_underscores-9");

    for value in ["/main", "origin/", "origin"] {
        let err = SourceRemoteRef::parse(value).expect_err("invalid source ref");
        assert!(matches!(err, OutpostError::InvalidRefName { .. }));
    }

    let nested = SourceRemoteRef::parse("remote._-9/topic/child").expect("nested branch");
    assert_eq!(nested.remote, remote);
    assert_eq!(nested.branch.as_str(), "topic/child");
}

#[test]
fn selector_resolves_path_variant_and_simple_bare_path() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let source = fixture.source_repo().expect("source repo");

    let from_path = OutpostSelector::from_path(outpost.clone());
    let resolved = resolve_entry(&source, &from_path).expect("path selector");
    assert_eq!(
        resolved.path,
        fs::canonicalize(&outpost).expect("canonical outpost")
    );

    let bare_path = OutpostSelector::from_cli_arg(&fixture.root, PathBuf::from("C"));
    let resolved = resolve_entry(&source, &bare_path).expect("bare path selector");
    assert_eq!(
        resolved.path,
        fs::canonicalize(outpost).expect("canonical outpost")
    );
}

#[test]
fn selector_uses_path_match_when_bare_hex_id_does_not_match() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let candidates = ["abcde", "12345", "fffff", "01234", "fedcb"];
    let name = candidates
        .into_iter()
        .find(|name| {
            let path = fixture.root.join(name);
            let canonical = fs::canonicalize(path.parent().expect("fixture parent"))
                .expect("canonical fixture root")
                .join(name);
            !OutpostId::derive(source.work_tree(), &canonical)
                .as_str()
                .starts_with(name)
        })
        .expect("one candidate should not collide with its own id prefix");
    let outpost = fixture.add_outpost(name).expect("hex-named outpost");

    let selector = OutpostSelector::from_cli_arg(&fixture.root, PathBuf::from(name));
    let resolved = resolve_entry(&source, &selector).expect("path match should win");

    assert_eq!(
        resolved.path,
        fs::canonicalize(outpost).expect("canonical outpost")
    );
}

#[test]
fn selector_normalizes_missing_intermediate_components() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("other").expect("outpost");
    let source = fixture.source_repo().expect("source repo");
    let root = fs::canonicalize(&fixture.root).expect("canonical fixture root");
    let selector = OutpostSelector::from_cli_arg(&root, PathBuf::from("missing/../other"));

    let resolved = resolve_entry(&source, &selector).expect("normalized path selector");

    assert_eq!(
        resolved.path,
        fs::canonicalize(outpost).expect("canonical outpost")
    );
}

#[test]
fn selector_handles_empty_and_single_component_path_forms() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let root = fs::canonicalize(&fixture.root).expect("canonical fixture root");

    let empty = OutpostSelector::from_path(PathBuf::new());
    let err = resolve_entry(&source, &empty).expect_err("empty path has no registry entry");
    assert!(
        matches!(err, OutpostError::RegistryEntryNotFound(path) if path.as_os_str().is_empty())
    );

    let current = OutpostSelector::from_cli_arg(&root, PathBuf::from("."));
    let err = resolve_entry(&source, &current).expect_err("current directory is not an outpost");
    assert!(matches!(err, OutpostError::RegistryEntryNotFound(path) if path == root));
}

#[cfg(unix)]
#[test]
fn selector_treats_non_utf8_cli_value_as_explicit_path() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let value = PathBuf::from(OsString::from_vec(b"bad-\xff".to_vec()));
    let expected = fs::canonicalize(&fixture.root)
        .expect("canonical fixture root")
        .join(&value);
    let selector = OutpostSelector::from_cli_arg(&fixture.root, value);

    let err = resolve_entry(&source, &selector).expect_err("missing non-utf8 path");

    assert!(matches!(err, OutpostError::RegistryEntryNotFound(path) if path == expected));
}
