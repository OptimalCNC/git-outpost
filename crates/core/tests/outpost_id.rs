#[allow(dead_code)]
mod common;

use std::fs;
use std::path::Path;

use common::fixture::AbcFixture;
use outpost_core::{OutpostId, OutpostIdPrefix, Registry, RegistryEntry, RemoteName};

#[test]
fn outpost_id_parse_requires_full_lowercase_hex() {
    let id = OutpostId::parse("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
        .expect("id parses");

    assert_eq!(
        id.as_str(),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    );
    assert!(OutpostId::parse("01234").is_err());
    assert!(
        OutpostId::parse("0123456789ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef")
            .is_err()
    );
    assert!(
        OutpostId::parse("0123456789abcdeg0123456789abcdef0123456789abcdef0123456789abcdef")
            .is_err()
    );
}

#[test]
fn outpost_id_prefix_parse_normalizes_case_and_enforces_minimum() {
    let prefix = OutpostIdPrefix::parse("AbC12").expect("prefix parses");

    assert_eq!(prefix.as_str(), "abc12");
    assert!(OutpostIdPrefix::parse("abc1").is_err());
    assert!(OutpostIdPrefix::parse("abc1z").is_err());
}

#[test]
fn outpost_id_generation_is_lowercase_64_hex() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source = canonical(temp.path());
    let destination = source.join("C");
    fs::create_dir(&destination).expect("destination dir");

    let id = OutpostId::derive(&source, &canonical(&destination));

    assert_eq!(id.as_str().len(), 64);
    assert!(id.as_str().chars().all(|ch| ch.is_ascii_hexdigit()));
    assert!(id.as_str().chars().all(|ch| !ch.is_ascii_uppercase()));
}

#[test]
fn registry_round_trip_does_not_store_outpost_ids() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let outpost = fixture.add_outpost("C").expect("add C");

    let mut registry = source.registry_mut().expect("registry mut");
    registry
        .add(RegistryEntry::new(outpost.clone(), RemoteName::parse("local").unwrap()).unwrap())
        .expect("re-add outpost");
    registry.save().expect("save registry");

    let loaded = Registry::load(&source).expect("reload registry");
    assert_eq!(loaded.entries().len(), 1);
    let json = fs::read_to_string(source.registry_path()).expect("registry json");
    assert!(
        !json.contains("\"id\""),
        "registry JSON should not persist derived id field:\n{json}"
    );
}

#[test]
fn registry_save_drops_stale_outpost_id_fields() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let outpost = fixture.root.join("C");
    fs::create_dir(&outpost).expect("outpost dir");
    fs::create_dir_all(source.registry_path().parent().unwrap()).expect("registry dir");
    fs::write(
        source.registry_path(),
        format!(
            r#"{{
    "version": 1,
  "outposts": [
    {{
      "id": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
      "path": {},
      "created_at": "1970-01-01T00:00:01Z",
      "remote_name": "local",
      "locked": false,
      "lock_reason": null,
      "locked_at": null
    }}
  ]
}}"#,
            serde_json::to_string(&outpost).expect("json path")
        ),
    )
    .expect("write legacy registry");

    let loaded = Registry::load(&source).expect("load legacy registry");
    assert_eq!(loaded.entries().len(), 1);
    loaded.save().expect("save registry");

    let json = fs::read_to_string(source.registry_path()).expect("registry json");
    assert!(
        !json.contains("\"id\""),
        "registry save should drop stale derived id field:\n{json}"
    );
}

fn canonical(path: &Path) -> std::path::PathBuf {
    fs::canonicalize(path).expect("canonical path")
}
