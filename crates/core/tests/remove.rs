#[allow(dead_code)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use outpost_core::ops::remove;
use outpost_core::{
    BranchName, OutpostError, OutpostResult, RegistryEntry, RemoteName, SourceRepo,
};

#[test]
fn remove_clean_fully_pushed_outpost_deletes_dir_and_registry_entry() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let sentinel = create_unrelated_dir(&fixture, "unrelated-r01");

    remove::run(
        &source,
        remove::RemoveOptions {
            path: outpost.clone(),
            force: false,
        },
    )
    .expect("remove clean outpost");

    assert!(!outpost.exists());
    assert_registry_empty(&source);
    assert!(sentinel.exists());
    assert_source_branch_exists(&source, "main");
}

#[test]
fn remove_dirty_outpost_returns_dirty_tree_with_force_hint() {
    let fixture = AbcFixture::new();
    let outpost = fixture.dirty_outpost("C").expect("dirty C");
    let source = fixture.source_repo().expect("source repo");

    let err = expect_error(
        remove::run(
            &source,
            remove::RemoveOptions {
                path: outpost.clone(),
                force: false,
            },
        ),
        "dirty remove should fail",
    );

    assert!(
        matches!(err, OutpostError::DirtyTree { repo, hint } if repo == canonical(&outpost) && hint == "pass --force")
    );
    assert!(outpost.exists());
    assert_eq!(single_entry(&source).path, canonical(&outpost));
}

#[test]
fn remove_unpushed_outpost_returns_unpushed_commits() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_in_outpost(&outpost, "outpost-only commit")
        .expect("commit in outpost");
    let source = fixture.source_repo().expect("source repo");

    let err = expect_error(
        remove::run(
            &source,
            remove::RemoveOptions {
                path: outpost.clone(),
                force: false,
            },
        ),
        "unpushed remove should fail",
    );

    assert!(
        matches!(err, OutpostError::UnpushedCommits { repo, branch, hint } if repo == canonical(&outpost) && branch == "main" && hint == "pass --force")
    );
    assert!(outpost.exists());
    assert_eq!(single_entry(&source).path, canonical(&outpost));
}

#[test]
fn remove_force_deletes_dirty_outpost() {
    let fixture = AbcFixture::new();
    let outpost = fixture.dirty_outpost("C").expect("dirty C");
    let source = fixture.source_repo().expect("source repo");
    let sentinel = create_unrelated_dir(&fixture, "unrelated-r04");

    remove::run(
        &source,
        remove::RemoveOptions {
            path: outpost.clone(),
            force: true,
        },
    )
    .expect("force remove dirty outpost");

    assert!(!outpost.exists());
    assert_registry_empty(&source);
    assert!(sentinel.exists());
    assert_source_branch_exists(&source, "main");
}

#[test]
fn remove_force_deletes_outpost_with_unpushed_commits() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_in_outpost(&outpost, "outpost-only commit")
        .expect("commit in outpost");
    let source = fixture.source_repo().expect("source repo");
    let sentinel = create_unrelated_dir(&fixture, "unrelated-r05");

    remove::run(
        &source,
        remove::RemoveOptions {
            path: outpost.clone(),
            force: true,
        },
    )
    .expect("force remove unpushed outpost");

    assert!(!outpost.exists());
    assert_registry_empty(&source);
    assert!(sentinel.exists());
    assert_source_branch_exists(&source, "main");
}

#[test]
fn remove_unregistered_path_returns_registry_entry_not_found() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let path = fixture.root.join("unregistered");
    fs::create_dir(&path).expect("unregistered dir");

    let err = expect_error(
        remove::run(
            &source,
            remove::RemoveOptions {
                path: path.clone(),
                force: false,
            },
        ),
        "unregistered remove should fail",
    );

    assert!(
        matches!(err, OutpostError::RegistryEntryNotFound(err_path) if err_path == canonical(&path))
    );
    assert!(path.exists());
    assert_registry_empty(&source);
}

#[test]
fn remove_unlocked_missing_registered_path_deregisters_without_rmtree() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let sentinel = create_unrelated_dir(&fixture, "unrelated-r07");
    fs::remove_dir_all(&outpost).expect("remove outpost dir");

    remove::run(
        &source,
        remove::RemoveOptions {
            path: outpost.clone(),
            force: false,
        },
    )
    .expect("remove missing registered path");

    assert!(!outpost.exists());
    assert_registry_empty(&source);
    assert!(sentinel.exists());
}

#[test]
fn remove_registry_entry_pointing_at_unrelated_dir_returns_not_managed() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let unrelated = fixture.root.join("unrelated");
    fs::create_dir(&unrelated).expect("unrelated dir");
    let sentinel = unrelated.join("keep.txt");
    fs::write(&sentinel, "keep").expect("unrelated file");
    register_existing_path(&source, &unrelated).expect("register unrelated path");

    let err = expect_error(
        remove::run(
            &source,
            remove::RemoveOptions {
                path: unrelated.clone(),
                force: true,
            },
        ),
        "unrelated registered path should fail",
    );

    assert!(
        matches!(err, OutpostError::RegistryEntryNotManaged(path) if path == canonical(&unrelated))
    );
    assert!(unrelated.exists());
    assert!(sentinel.exists());
    assert_eq!(single_entry(&source).path, canonical(&unrelated));
}

#[test]
fn remove_wrong_source_outpost_returns_not_managed() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    fs::remove_dir_all(&outpost).expect("remove original outpost");
    let other = AbcFixture::new();
    let other_outpost = other.add_outpost("C").expect("add other C");
    fs::rename(&other_outpost, &outpost).expect("move wrong-source outpost");
    let source = fixture.source_repo().expect("source repo");

    let err = expect_error(
        remove::run(
            &source,
            remove::RemoveOptions {
                path: outpost.clone(),
                force: true,
            },
        ),
        "wrong-source remove should fail",
    );

    assert!(
        matches!(err, OutpostError::RegistryEntryNotManaged(path) if path == canonical(&outpost))
    );
    assert!(outpost.exists());
    assert!(outpost.join(".git").exists());
    assert_eq!(single_entry(&source).path, canonical(&outpost));
}

#[test]
fn remove_refuses_locked_outpost_unless_forced() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let sentinel = create_unrelated_dir(&fixture, "unrelated-r10");
    lock_registry_entry(&source, &outpost, Some("keep")).expect("lock setup");

    let err = expect_error(
        remove::run(
            &source,
            remove::RemoveOptions {
                path: outpost.clone(),
                force: false,
            },
        ),
        "locked remove should fail",
    );

    assert!(
        matches!(err, OutpostError::OutpostLocked { path, reason } if path == canonical(&outpost) && reason == ": keep")
    );
    assert!(outpost.exists());
    let entry = single_entry(&source);
    assert_eq!(entry.path, canonical(&outpost));
    assert!(entry.locked);

    remove::run(
        &source,
        remove::RemoveOptions {
            path: outpost.clone(),
            force: true,
        },
    )
    .expect("force remove locked outpost");

    assert!(!outpost.exists());
    assert_registry_empty(&source);
    assert!(sentinel.exists());
    assert_source_branch_exists(&source, "main");
}

#[test]
fn remove_locked_missing_path_requires_force_then_deregisters() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let sentinel = create_unrelated_dir(&fixture, "unrelated-r11");
    lock_registry_entry(&source, &outpost, Some("keep")).expect("lock setup");
    fs::remove_dir_all(&outpost).expect("remove outpost dir");

    let err = expect_error(
        remove::run(
            &source,
            remove::RemoveOptions {
                path: outpost.clone(),
                force: false,
            },
        ),
        "locked missing remove should fail",
    );

    assert!(
        matches!(err, OutpostError::OutpostLocked { path, reason } if path == canonical_missing(&outpost) && reason == ": keep")
    );
    assert!(!outpost.exists());
    let entry = single_entry(&source);
    assert_eq!(entry.path, canonical_missing(&outpost));
    assert!(entry.locked);

    remove::run(
        &source,
        remove::RemoveOptions {
            path: outpost.clone(),
            force: true,
        },
    )
    .expect("force remove locked missing path");

    assert!(!outpost.exists());
    assert_registry_empty(&source);
    assert!(sentinel.exists());
}

fn single_entry(source: &SourceRepo) -> RegistryEntry {
    let registry = source.registry().expect("registry");
    assert_eq!(registry.entries().len(), 1);
    registry.entries()[0].clone()
}

fn assert_registry_empty(source: &SourceRepo) {
    let registry = source.registry().expect("registry");
    assert!(registry.entries().is_empty());
}

fn register_existing_path(source: &SourceRepo, path: &Path) -> OutpostResult<()> {
    let mut registry = source.registry_mut()?;
    registry.add(RegistryEntry::new(
        path.to_path_buf(),
        RemoteName::parse("local")?,
    )?)?;
    registry.save()
}

fn lock_registry_entry(
    source: &SourceRepo,
    path: &Path,
    reason: Option<&str>,
) -> OutpostResult<()> {
    let mut registry = source.registry_mut()?;
    registry.lock(path, reason.map(str::to_owned))?;
    registry.save()
}

fn create_unrelated_dir(fixture: &AbcFixture, name: &str) -> PathBuf {
    let path = fixture.root.join(name);
    fs::create_dir(&path).expect("unrelated dir");
    let sentinel = path.join("keep.txt");
    fs::write(&sentinel, "keep").expect("unrelated file");
    sentinel
}

fn assert_source_branch_exists(source: &SourceRepo, branch: &str) {
    let branch = BranchName::parse(branch.to_owned()).expect("branch name");
    assert!(
        source.branch_exists(&branch).expect("branch exists query"),
        "source branch {} should remain",
        branch.as_str()
    );
}

fn expect_error<T>(result: OutpostResult<T>, message: &str) -> OutpostError {
    match result {
        Ok(_) => panic!("{message}"),
        Err(err) => err,
    }
}

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("canonical path")
}

fn canonical_missing(path: &Path) -> PathBuf {
    let parent = path.parent().expect("path parent");
    canonical(parent).join(path.file_name().expect("file name"))
}
