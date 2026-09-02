#[allow(dead_code)]
mod common;

use std::fs;

use common::fixture::AbcFixture;
use outpost_core::{ConfigKey, ConfigValue, Outpost, RegistryEntry, RemoteName, SourceRepo};

#[test]
fn source_storage_paths_use_exact_git_directory() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");

    assert_eq!(
        source.config_path(),
        source.git_dir().join("outpost/config.json")
    );
    assert_eq!(
        source.registry_path(),
        source.git_dir().join("outpost/registry.json")
    );
}

#[test]
fn git_clean_preserves_private_git_directory_state() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let source = fixture.source_repo().expect("source repo");
    let container = fixture.root.join("outposts");
    fs::create_dir(&container).expect("container");
    source
        .config()
        .set(
            ConfigKey::OutpostContainer,
            ConfigValue::OutpostContainer(container),
        )
        .expect("source config");

    let config_path = source.config_path();
    let registry_path = source.registry_path();
    let metadata_path = Outpost::at(&outpost)
        .expect("opened outpost")
        .metadata_path();

    fixture
        .invoker(&fixture.source)
        .run_check(["clean", "-fdx"])
        .expect("clean source worktree");
    fixture
        .invoker(&outpost)
        .run_check(["clean", "-fdx"])
        .expect("clean outpost worktree");

    assert!(config_path.is_file());
    assert!(registry_path.is_file());
    assert!(metadata_path.is_file());
    assert!(Outpost::at(&outpost).is_ok());
}

#[test]
fn linked_source_worktrees_have_independent_git_directory_state() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let linked_path = fixture.root.join("linked-source");
    fixture
        .invoker(&fixture.source)
        .run_check([
            "worktree",
            "add",
            "-b",
            "linked-state",
            linked_path.to_str().expect("linked path"),
        ])
        .expect("linked worktree");
    let linked = SourceRepo::at_with(&linked_path, &fixture.git_env).expect("linked source");

    assert_ne!(source.git_dir(), linked.git_dir());
    assert_ne!(source.config_path(), linked.config_path());
    assert_eq!(
        linked.config_path(),
        linked.git_dir().join("outpost/config.json")
    );

    let container = fixture.root.join("linked-outposts");
    fs::create_dir(&container).expect("container");
    linked
        .config()
        .set(
            ConfigKey::OutpostContainer,
            ConfigValue::OutpostContainer(container),
        )
        .expect("linked config");
    assert!(!source.config_path().exists());
    assert!(linked.config_path().is_file());

    let linked_outpost = fixture.root.join("linked-outpost");
    fs::create_dir(&linked_outpost).expect("linked outpost directory");
    let mut registry = linked.registry_mut().expect("linked registry");
    registry
        .add(
            RegistryEntry::new(
                linked_outpost.clone(),
                RemoteName::parse("local").expect("remote name"),
            )
            .expect("registry entry"),
        )
        .expect("add linked registry entry");
    registry.save().expect("save linked registry");

    assert_eq!(
        linked.registry_path(),
        linked.git_dir().join("outpost/registry.json")
    );
    assert_ne!(source.registry_path(), linked.registry_path());
    assert!(linked.registry_path().is_file());
    assert_eq!(
        linked.registry().expect("linked registry").entries().len(),
        1
    );
    assert!(
        source
            .registry()
            .expect("primary registry")
            .entries()
            .is_empty()
    );
    assert!(!source.registry_path().exists());
}
