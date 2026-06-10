#[allow(dead_code)]
mod common;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use outpost_core::selector::{OutpostSelector, resolve_entry, resolve_live_entry};
use outpost_core::{OutpostError, OutpostId, RegistryEntry, RemoteName, SourceRepo};

#[test]
fn selector_resolves_unique_id_prefix() {
    let fixture = AbcFixture::new();
    fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let entry = single_entry(&source);
    let id = OutpostId::derive(source.work_tree(), &entry.path);
    let selector = OutpostSelector::from_cli_arg(&fixture.root, id.as_str()[..5].into());

    let resolved = resolve_live_entry(&source, &selector).expect("resolve prefix");

    assert_eq!(resolved.path, entry.path);
}

#[test]
fn selector_rejects_ambiguous_id_prefix() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let (first, second, prefix) = colliding_paths(&fixture, &source);
    fs::create_dir(&first).expect("first dir");
    fs::create_dir(&second).expect("second dir");
    replace_registry(&source, [entry_at(&first), entry_at(&second)]);
    let selector = OutpostSelector::from_cli_arg(&fixture.root, prefix.clone().into());

    let err = resolve_entry(&source, &selector).expect_err("ambiguous prefix should fail");

    assert!(matches!(err, OutpostError::OutpostIdPrefixAmbiguous(actual) if actual == prefix));
}

#[test]
fn selector_rejects_missing_id_prefix() {
    let fixture = AbcFixture::new();
    fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let missing = missing_prefix(source.work_tree(), &[single_entry(&source).path]);
    let selector = OutpostSelector::from_cli_arg(&fixture.root, missing.clone().into());

    let err = resolve_entry(&source, &selector).expect_err("missing prefix should fail");

    assert!(matches!(err, OutpostError::OutpostIdPrefixNotFound(actual) if actual == missing));
}

#[test]
fn selector_fails_closed_when_bare_hex_path_and_id_match_different_entries() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let other = fixture.root.join("other");
    fs::create_dir(&other).expect("other dir");
    let prefix = OutpostId::derive(source.work_tree(), &canonical(&other)).as_str()[..5].to_owned();
    let hex_path = fixture.root.join(&prefix);
    fs::create_dir(&hex_path).expect("hex path dir");
    replace_registry(&source, [entry_at(&hex_path), entry_at(&other)]);
    let selector = OutpostSelector::from_cli_arg(&fixture.root, prefix.clone().into());

    let err = resolve_entry(&source, &selector).expect_err("selector should be ambiguous");

    assert!(matches!(err, OutpostError::OutpostSelectorAmbiguous(value) if value == prefix));
}

#[test]
fn selector_treats_trailing_separator_as_path_only() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let other = fixture.root.join("other");
    fs::create_dir(&other).expect("other dir");
    let prefix = OutpostId::derive(source.work_tree(), &canonical(&other)).as_str()[..5].to_owned();
    let hex_path = fixture.root.join(&prefix);
    fs::create_dir(&hex_path).expect("hex path dir");
    replace_registry(&source, [entry_at(&hex_path), entry_at(&other)]);
    let selector = OutpostSelector::from_cli_arg(&fixture.root, format!("{prefix}/").into());

    let resolved = resolve_entry(&source, &selector).expect("resolve path with separator");

    assert_eq!(resolved.entry.path, canonical(&hex_path));
}

#[test]
fn selector_entry_resolution_allows_missing_registered_path_by_id() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let entry = single_entry(&source);
    fs::remove_dir_all(&outpost).expect("remove outpost dir");
    let id = OutpostId::derive(source.work_tree(), &entry.path);
    let selector = OutpostSelector::from_cli_arg(&fixture.root, id.as_str()[..5].into());

    let resolved = resolve_entry(&source, &selector).expect("resolve missing registered path");

    assert_eq!(resolved.path, entry.path);
}

fn single_entry(source: &SourceRepo) -> RegistryEntry {
    let registry = source.registry().expect("registry");
    assert_eq!(registry.entries().len(), 1);
    registry.entries()[0].clone()
}

fn replace_registry<const N: usize>(source: &SourceRepo, entries: [RegistryEntry; N]) {
    let mut registry = source.registry_mut().expect("registry mut");
    for entry in entries {
        registry.add(entry).expect("add entry");
    }
    registry.save().expect("save registry");
}

fn entry_at(path: &Path) -> RegistryEntry {
    RegistryEntry::new(path.to_path_buf(), RemoteName::parse("local").unwrap()).expect("entry")
}

fn colliding_paths(fixture: &AbcFixture, source: &SourceRepo) -> (PathBuf, PathBuf, String) {
    let mut prefixes = HashMap::<String, PathBuf>::new();
    for index in 0..5000 {
        let path = fixture.root.join(format!("candidate-{index}"));
        let id = OutpostId::derive(source.work_tree(), &path);
        let prefix = id.as_str()[..5].to_owned();
        if let Some(first) = prefixes.insert(prefix.clone(), path.clone()) {
            return (first, path, prefix);
        }
    }
    panic!("could not find derived ID prefix collision");
}

fn missing_prefix(source_path: &Path, paths: &[PathBuf]) -> String {
    ["fffff", "eeeee", "ddddd", "ccccc"]
        .into_iter()
        .find(|prefix| {
            !paths.iter().any(|path| {
                OutpostId::derive(source_path, path)
                    .as_str()
                    .starts_with(prefix)
            })
        })
        .expect("missing prefix")
        .to_owned()
}

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("canonical path")
}
