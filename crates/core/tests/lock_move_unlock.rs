#[allow(dead_code)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use outpost_core::ops::r#move as move_op;
use outpost_core::ops::{lock, unlock};
use outpost_core::selector::OutpostSelector;
use outpost_core::{OutpostError, OutpostId, OutpostResult, RegistryEntry, SourceRepo};

#[test]
fn lock_with_reason_marks_registry_entry_locked() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");

    lock::run(
        &source,
        lock::LockOptions {
            selector: OutpostSelector::from_path(outpost),
            reason: Some("keep".to_owned()),
        },
    )
    .expect("lock outpost");

    let entry = single_entry(&source);
    assert!(entry.locked);
    assert_eq!(entry.lock_reason.as_deref(), Some("keep"));
    assert!(entry.locked_at.is_some());
}

#[test]
fn unlock_clears_registry_lock_state_and_reason() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");
    lock_registry_entry(&source, &outpost, Some("keep")).expect("lock setup");

    unlock::run(
        &source,
        unlock::UnlockOptions {
            selector: OutpostSelector::from_path(outpost),
        },
    )
    .expect("unlock outpost");

    let entry = single_entry(&source);
    assert!(!entry.locked);
    assert!(entry.lock_reason.is_none());
    assert!(entry.locked_at.is_none());
}

#[test]
fn lock_and_unlock_accept_unique_id_prefix() {
    let fixture = AbcFixture::new();
    fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let prefix = single_entry_prefix(&source);

    lock::run(
        &source,
        lock::LockOptions {
            selector: OutpostSelector::from_cli_arg(&fixture.root, prefix.clone().into()),
            reason: Some("keep".to_owned()),
        },
    )
    .expect("lock by id");
    assert!(single_entry(&source).locked);

    unlock::run(
        &source,
        unlock::UnlockOptions {
            selector: OutpostSelector::from_cli_arg(&fixture.root, prefix.into()),
        },
    )
    .expect("unlock by id");
    assert!(!single_entry(&source).locked);
}

#[test]
fn move_updates_filesystem_and_registry_path() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let destination = fixture.root.join("D");
    let source = fixture.source_repo().expect("source repo");

    move_op::run(
        &source,
        move_op::MoveOptions {
            selector: OutpostSelector::from_path(outpost.clone()),
            new_path: destination.clone(),
            force: false,
        },
    )
    .expect("move outpost");

    assert!(!outpost.exists());
    assert!(destination.join(".git").exists());
    assert_eq!(single_entry(&source).path, canonical(&destination));
}

#[test]
fn move_accepts_unique_id_prefix_and_updates_derived_id() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let destination = fixture.root.join("D");
    let source = fixture.source_repo().expect("source repo");
    let before = OutpostId::derive(source.work_tree(), &single_entry(&source).path);
    let prefix = before.as_str()[..5].to_owned();

    move_op::run(
        &source,
        move_op::MoveOptions {
            selector: OutpostSelector::from_cli_arg(&fixture.root, prefix.into()),
            new_path: destination.clone(),
            force: false,
        },
    )
    .expect("move by id");

    assert!(!outpost.exists());
    assert!(destination.join(".git").exists());
    let entry = single_entry(&source);
    assert_eq!(entry.path, canonical(&destination));
    assert_ne!(OutpostId::derive(source.work_tree(), &entry.path), before);
}

#[test]
fn move_refuses_locked_outpost_without_force() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let destination = fixture.root.join("D");
    let source = fixture.source_repo().expect("source repo");
    lock_registry_entry(&source, &outpost, Some("keep")).expect("lock setup");

    let err = expect_error(
        move_op::run(
            &source,
            move_op::MoveOptions {
                selector: OutpostSelector::from_path(outpost.clone()),
                new_path: destination.clone(),
                force: false,
            },
        ),
        "locked move should fail",
    );

    assert!(
        matches!(err, OutpostError::OutpostLocked { path, reason } if path == canonical(&outpost) && reason == ": keep")
    );
    assert!(outpost.exists());
    assert!(!destination.exists());
    assert_eq!(single_entry(&source).path, canonical(&outpost));
}

#[test]
fn move_force_moves_locked_outpost_and_preserves_lock() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let destination = fixture.root.join("D");
    let source = fixture.source_repo().expect("source repo");
    lock_registry_entry(&source, &outpost, Some("keep")).expect("lock setup");

    move_op::run(
        &source,
        move_op::MoveOptions {
            selector: OutpostSelector::from_path(outpost.clone()),
            new_path: destination.clone(),
            force: true,
        },
    )
    .expect("force move locked outpost");

    assert!(!outpost.exists());
    assert!(destination.join(".git").exists());
    let entry = single_entry(&source);
    assert_eq!(entry.path, canonical(&destination));
    assert!(entry.locked);
    assert_eq!(entry.lock_reason.as_deref(), Some("keep"));
    assert!(entry.locked_at.is_some());
}

#[test]
fn move_refuses_dirty_outpost_but_force_succeeds() {
    let fixture = AbcFixture::new();
    let outpost = fixture.dirty_outpost("C").expect("dirty C");
    let destination = fixture.root.join("D");
    let source = fixture.source_repo().expect("source repo");

    let err = expect_error(
        move_op::run(
            &source,
            move_op::MoveOptions {
                selector: OutpostSelector::from_path(outpost.clone()),
                new_path: destination.clone(),
                force: false,
            },
        ),
        "dirty move should fail",
    );

    assert!(
        matches!(err, OutpostError::DirtyTree { repo, hint } if repo == canonical(&outpost) && hint == "pass --force")
    );
    assert!(outpost.exists());
    assert!(!destination.exists());

    move_op::run(
        &source,
        move_op::MoveOptions {
            selector: OutpostSelector::from_path(outpost.clone()),
            new_path: destination.clone(),
            force: true,
        },
    )
    .expect("force move dirty outpost");

    assert!(!outpost.exists());
    assert!(destination.join("x.txt").exists());
    assert_eq!(single_entry(&source).path, canonical(&destination));
}

#[test]
fn move_refuses_non_empty_destination() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let destination = fixture.root.join("D");
    fs::create_dir(&destination).expect("destination dir");
    fs::write(destination.join("file.txt"), "content").expect("destination file");
    let source = fixture.source_repo().expect("source repo");

    let err = expect_error(
        move_op::run(
            &source,
            move_op::MoveOptions {
                selector: OutpostSelector::from_path(outpost.clone()),
                new_path: destination.clone(),
                force: false,
            },
        ),
        "non-empty destination should fail",
    );

    assert!(matches!(err, OutpostError::DestinationExists(path) if path == destination));
    assert!(outpost.exists());
    assert_eq!(single_entry(&source).path, canonical(&outpost));
}

#[test]
fn move_refuses_destination_inside_unignored_source_repo() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let destination = fixture.source.join("D");
    let source = fixture.source_repo().expect("source repo");

    let err = expect_error(
        move_op::run(
            &source,
            move_op::MoveOptions {
                selector: OutpostSelector::from_path(outpost.clone()),
                new_path: destination.clone(),
                force: false,
            },
        ),
        "unignored in-repo destination should fail",
    );

    assert!(matches!(err, OutpostError::DestinationInsideRepo(path) if path == destination));
    assert!(outpost.exists());
    assert!(!destination.exists());
    assert_eq!(single_entry(&source).path, canonical(&outpost));
}

#[test]
fn move_allows_destination_inside_ignored_source_repo_path() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let destination = fixture.source.join("D");
    fs::write(fixture.source.join(".git/info/exclude"), "D/\n").expect("write exclude");
    let source = fixture.source_repo().expect("source repo");

    move_op::run(
        &source,
        move_op::MoveOptions {
            selector: OutpostSelector::from_path(outpost.clone()),
            new_path: destination.clone(),
            force: false,
        },
    )
    .expect("move to ignored source path");

    assert!(!outpost.exists());
    assert!(destination.join(".git").exists());
    assert_eq!(single_entry(&source).path, canonical(&destination));
}

#[test]
fn lock_move_unlock_reject_unregistered_paths() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let path = fixture.root.join("unregistered");
    fs::create_dir(&path).expect("unregistered dir");

    let lock_err = expect_error(
        lock::run(
            &source,
            lock::LockOptions {
                selector: OutpostSelector::from_path(path.clone()),
                reason: None,
            },
        ),
        "unregistered lock should fail",
    );
    let move_err = expect_error(
        move_op::run(
            &source,
            move_op::MoveOptions {
                selector: OutpostSelector::from_path(path.clone()),
                new_path: fixture.root.join("D"),
                force: false,
            },
        ),
        "unregistered move should fail",
    );
    let unlock_err = expect_error(
        unlock::run(
            &source,
            unlock::UnlockOptions {
                selector: OutpostSelector::from_path(path.clone()),
            },
        ),
        "unregistered unlock should fail",
    );

    assert!(
        matches!(lock_err, OutpostError::RegistryEntryNotFound(err_path) if err_path == canonical(&path))
    );
    assert!(
        matches!(move_err, OutpostError::RegistryEntryNotFound(err_path) if err_path == canonical(&path))
    );
    assert!(
        matches!(unlock_err, OutpostError::RegistryEntryNotFound(err_path) if err_path == canonical(&path))
    );
    assert!(path.exists());
}

#[test]
fn lock_move_unlock_reject_wrong_source_registered_path() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    fs::remove_dir_all(&outpost).expect("remove original outpost");
    let other = AbcFixture::new();
    let other_outpost = other.add_outpost("C").expect("add other C");
    fs::rename(&other_outpost, &outpost).expect("move wrong-source outpost");
    let source = fixture.source_repo().expect("source repo");

    let lock_err = expect_error(
        lock::run(
            &source,
            lock::LockOptions {
                selector: OutpostSelector::from_path(outpost.clone()),
                reason: Some("keep".to_owned()),
            },
        ),
        "wrong-source lock should fail",
    );
    let move_err = expect_error(
        move_op::run(
            &source,
            move_op::MoveOptions {
                selector: OutpostSelector::from_path(outpost.clone()),
                new_path: fixture.root.join("D"),
                force: true,
            },
        ),
        "wrong-source move should fail",
    );
    lock_registry_entry(&source, &outpost, Some("keep")).expect("lock setup");
    let unlock_err = expect_error(
        unlock::run(
            &source,
            unlock::UnlockOptions {
                selector: OutpostSelector::from_path(outpost.clone()),
            },
        ),
        "wrong-source unlock should fail",
    );

    assert!(
        matches!(lock_err, OutpostError::RegistryEntryNotManaged(path) if path == canonical(&outpost))
    );
    assert!(
        matches!(move_err, OutpostError::RegistryEntryNotManaged(path) if path == canonical(&outpost))
    );
    assert!(
        matches!(unlock_err, OutpostError::RegistryEntryNotManaged(path) if path == canonical(&outpost))
    );
    assert!(outpost.exists());
    assert!(!fixture.root.join("D").exists());
    let entry = single_entry(&source);
    assert_eq!(entry.path, canonical(&outpost));
    assert!(entry.locked);
    assert_eq!(entry.lock_reason.as_deref(), Some("keep"));
}

fn single_entry(source: &SourceRepo) -> RegistryEntry {
    let registry = source.registry().expect("registry");
    assert_eq!(registry.entries().len(), 1);
    registry.entries()[0].clone()
}

fn single_entry_prefix(source: &SourceRepo) -> String {
    let entry = single_entry(source);
    OutpostId::derive(source.work_tree(), &entry.path).as_str()[..5].to_owned()
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

fn expect_error<T>(result: OutpostResult<T>, message: &str) -> OutpostError {
    match result {
        Ok(_) => panic!("{message}"),
        Err(err) => err,
    }
}

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("canonical path")
}
