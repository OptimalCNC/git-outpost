#[allow(dead_code)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use outpost_core::ops::prune;
use outpost_core::{BranchName, OutpostResult, RegistryEntry, RemoteName, SourceRepo};

#[test]
fn prune_removes_missing_registry_entries() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let missing_path = canonical(&outpost);
    fs::remove_dir_all(&outpost).expect("remove outpost dir");

    let report = prune::run(
        &source,
        prune::PruneOptions {
            dry_run: false,
            verbose: false,
        },
    )
    .expect("prune missing outpost");

    assert_eq!(report.removed_entries, vec![missing_path]);
    assert!(report.orphaned_source_missing.is_empty());
    assert!(report.locked_entries.is_empty());
    assert!(!report.dry_run);
    assert_registry_empty(&source);
}

#[test]
fn prune_keeps_existing_valid_outposts() {
    let fixture = AbcFixture::new();
    let one = fixture.add_outpost("C1").expect("add C1");
    let two = fixture.add_outpost("C2").expect("add C2");
    let source = fixture.source_repo().expect("source repo");

    let report = prune::run(
        &source,
        prune::PruneOptions {
            dry_run: false,
            verbose: false,
        },
    )
    .expect("prune valid outposts");

    assert!(report.removed_entries.is_empty());
    assert!(report.orphaned_source_missing.is_empty());
    assert!(report.locked_entries.is_empty());
    assert_registry_paths(&source, &[canonical(&one), canonical(&two)]);
    assert!(one.exists());
    assert!(two.exists());
}

#[test]
fn prune_does_not_delete_real_dirs_or_source_branches() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let unrelated_sentinel = create_unrelated_dir(&fixture, "unrelated-pr03");
    let source = fixture.source_repo().expect("source repo");
    register_existing_path(
        &source,
        unrelated_sentinel.parent().expect("sentinel parent"),
    )
    .expect("register unrelated dir");

    let report = prune::run(
        &source,
        prune::PruneOptions {
            dry_run: false,
            verbose: false,
        },
    )
    .expect("prune existing entries");

    assert!(report.removed_entries.is_empty());
    assert!(outpost.exists());
    assert!(unrelated_sentinel.exists());
    assert_source_branch_exists(&source, "main");
}

#[test]
fn prune_report_lists_removed_missing_entries() {
    let fixture = AbcFixture::new();
    let one = fixture.add_outpost("C1").expect("add C1");
    let two = fixture.add_outpost("C2").expect("add C2");
    let keep = fixture.add_outpost("C3").expect("add C3");
    let source = fixture.source_repo().expect("source repo");
    let one_missing = canonical(&one);
    let two_missing = canonical(&two);
    fs::remove_dir_all(&one).expect("remove C1");
    fs::remove_dir_all(&two).expect("remove C2");

    let report = prune::run(
        &source,
        prune::PruneOptions {
            dry_run: false,
            verbose: true,
        },
    )
    .expect("prune missing outposts");

    assert_eq!(report.removed_entries, vec![one_missing, two_missing]);
    assert_registry_paths(&source, &[canonical(&keep)]);
}

#[test]
fn prune_keeps_unrelated_dirs_and_wrong_source_outposts_registered() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    fs::remove_dir_all(&outpost).expect("remove original outpost");
    let other = AbcFixture::new();
    let other_outpost = other.add_outpost("C").expect("add other C");
    fs::rename(&other_outpost, &outpost).expect("move wrong-source outpost");
    let unrelated_sentinel = create_unrelated_dir(&fixture, "unrelated-pr05");
    let unrelated = unrelated_sentinel.parent().expect("sentinel parent");
    let source = fixture.source_repo().expect("source repo");
    register_existing_path(&source, unrelated).expect("register unrelated dir");

    let report = prune::run(
        &source,
        prune::PruneOptions {
            dry_run: false,
            verbose: false,
        },
    )
    .expect("prune unrelated and wrong-source entries");

    assert!(report.removed_entries.is_empty());
    assert!(report.orphaned_source_missing.is_empty());
    assert!(report.locked_entries.is_empty());
    assert!(outpost.join(".git").exists());
    assert!(unrelated_sentinel.exists());
    assert_registry_paths(&source, &[canonical(&outpost), canonical(unrelated)]);
}

#[test]
fn prune_dry_run_reports_but_does_not_modify_registry() {
    let fixture = AbcFixture::new();
    let stale = fixture.add_outpost("C1").expect("add C1");
    let keep = fixture.add_outpost("C2").expect("add C2");
    let source = fixture.source_repo().expect("source repo");
    let stale_missing = canonical(&stale);
    fs::remove_dir_all(&stale).expect("remove stale outpost");

    let report = prune::run(
        &source,
        prune::PruneOptions {
            dry_run: true,
            verbose: false,
        },
    )
    .expect("dry-run prune");

    assert_eq!(report.removed_entries, vec![stale_missing.clone()]);
    assert!(report.dry_run);
    assert_registry_paths(&source, &[stale_missing, canonical(&keep)]);
}

#[test]
fn prune_reports_existing_outpost_whose_source_repo_is_missing() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let outpost_path = canonical(&outpost);
    fixture
        .invoker(&outpost)
        .run_check([
            "config",
            "--local",
            "outpost.sourceRepo",
            fixture
                .root
                .join("missing-source")
                .to_str()
                .expect("utf-8 path"),
        ])
        .expect("point outpost source at missing path");

    let report = prune::run(
        &source,
        prune::PruneOptions {
            dry_run: false,
            verbose: false,
        },
    )
    .expect("prune source-missing outpost");

    assert!(report.removed_entries.is_empty());
    assert_eq!(report.orphaned_source_missing, vec![outpost_path.clone()]);
    assert!(report.locked_entries.is_empty());
    assert_registry_paths(&source, &[outpost_path]);
    assert!(outpost.exists());
}

#[test]
fn prune_keeps_locked_stale_entries_and_reports_locked() {
    let fixture = AbcFixture::new();
    let locked = fixture.add_outpost("C1").expect("add C1");
    let stale = fixture.add_outpost("C2").expect("add C2");
    let source = fixture.source_repo().expect("source repo");
    let locked_missing = canonical(&locked);
    let stale_missing = canonical(&stale);
    lock_registry_entry(&source, &locked, Some("keep")).expect("lock setup");
    fs::remove_dir_all(&locked).expect("remove locked outpost");
    fs::remove_dir_all(&stale).expect("remove stale outpost");

    let report = prune::run(
        &source,
        prune::PruneOptions {
            dry_run: false,
            verbose: false,
        },
    )
    .expect("prune locked and stale entries");

    assert_eq!(report.locked_entries, vec![locked_missing.clone()]);
    assert_eq!(report.removed_entries, vec![stale_missing]);
    assert_registry_paths(&source, &[locked_missing]);
}

#[test]
fn prune_report_removed_entries_is_independent_of_verbose() {
    let quiet = AbcFixture::new();
    let quiet_one = quiet.add_outpost("C1").expect("add quiet C1");
    let quiet_two = quiet.add_outpost("C2").expect("add quiet C2");
    let quiet_source = quiet.source_repo().expect("quiet source repo");
    let quiet_missing = vec![canonical(&quiet_one), canonical(&quiet_two)];
    fs::remove_dir_all(&quiet_one).expect("remove quiet C1");
    fs::remove_dir_all(&quiet_two).expect("remove quiet C2");

    let quiet_report = prune::run(
        &quiet_source,
        prune::PruneOptions {
            dry_run: false,
            verbose: false,
        },
    )
    .expect("quiet prune");

    let verbose = AbcFixture::new();
    let verbose_one = verbose.add_outpost("C1").expect("add verbose C1");
    let verbose_two = verbose.add_outpost("C2").expect("add verbose C2");
    let verbose_source = verbose.source_repo().expect("verbose source repo");
    let verbose_missing = vec![canonical(&verbose_one), canonical(&verbose_two)];
    fs::remove_dir_all(&verbose_one).expect("remove verbose C1");
    fs::remove_dir_all(&verbose_two).expect("remove verbose C2");

    let verbose_report = prune::run(
        &verbose_source,
        prune::PruneOptions {
            dry_run: false,
            verbose: true,
        },
    )
    .expect("verbose prune");

    assert_eq!(quiet_report.removed_entries, quiet_missing);
    assert_eq!(verbose_report.removed_entries, verbose_missing);
    assert_eq!(
        quiet_report.removed_entries.len(),
        verbose_report.removed_entries.len()
    );
}

fn assert_registry_empty(source: &SourceRepo) {
    let registry = source.registry().expect("registry");
    assert!(registry.entries().is_empty());
}

fn assert_registry_paths(source: &SourceRepo, expected: &[PathBuf]) {
    let registry = source.registry().expect("registry");
    let paths = registry
        .entries()
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    assert_eq!(paths, expected);
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

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("canonical path")
}
