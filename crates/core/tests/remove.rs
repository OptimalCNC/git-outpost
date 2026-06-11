#[allow(dead_code)]
mod common;

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use outpost_core::ops::remove;
use outpost_core::selector::OutpostSelector;
use outpost_core::{
    BranchName, OutpostError, OutpostId, OutpostResult, RegistryEntry, RemoteName, SourceRepo,
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
            selector: OutpostSelector::from_path(outpost.clone()),
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
fn remove_accepts_unique_id_prefix() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let prefix = single_entry_prefix(&source);

    remove::run(
        &source,
        remove::RemoveOptions {
            selector: OutpostSelector::from_cli_arg(&fixture.root, prefix.into()),
            force: false,
        },
    )
    .expect("remove by id");

    assert!(!outpost.exists());
    assert_registry_empty(&source);
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
                selector: OutpostSelector::from_path(outpost.clone()),
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
                selector: OutpostSelector::from_path(outpost.clone()),
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
            selector: OutpostSelector::from_path(outpost.clone()),
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
            selector: OutpostSelector::from_path(outpost.clone()),
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
                selector: OutpostSelector::from_path(path.clone()),
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
            selector: OutpostSelector::from_path(outpost.clone()),
            force: false,
        },
    )
    .expect("remove missing registered path");

    assert!(!outpost.exists());
    assert_registry_empty(&source);
    assert!(sentinel.exists());
}

#[test]
fn remove_id_prefix_deregisters_missing_registered_path() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let prefix = single_entry_prefix(&source);
    fs::remove_dir_all(&outpost).expect("remove outpost dir");

    remove::run(
        &source,
        remove::RemoveOptions {
            selector: OutpostSelector::from_cli_arg(&fixture.root, prefix.into()),
            force: false,
        },
    )
    .expect("remove missing by id");

    assert_registry_empty(&source);
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
                selector: OutpostSelector::from_path(unrelated.clone()),
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
                selector: OutpostSelector::from_path(outpost.clone()),
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
                selector: OutpostSelector::from_path(outpost.clone()),
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
            selector: OutpostSelector::from_path(outpost.clone()),
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
                selector: OutpostSelector::from_path(outpost.clone()),
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
            selector: OutpostSelector::from_path(outpost.clone()),
            force: true,
        },
    )
    .expect("force remove locked missing path");

    assert!(!outpost.exists());
    assert_registry_empty(&source);
    assert!(sentinel.exists());
}

#[test]
fn remove_with_cleanup_deletes_source_branch_proven_by_default_ancestor() {
    let fixture = AbcFixture::new();
    ensure_origin_head(&fixture);
    let branch = fixture.create_source_branch("feat").expect("create feat");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let mut prompt = TestPrompt::new([true], []);

    let report = remove::run_with_cleanup(
        &source,
        remove::RemoveOptions {
            selector: OutpostSelector::from_path(outpost.clone()),
            force: false,
        },
        remove::BranchCleanupMode::Prompt(remove::BranchCleanupOptions {
            provider: None,
            prompt: &mut prompt,
        }),
    )
    .expect("remove with cleanup");

    assert!(!outpost.exists());
    assert_source_branch_missing(&source, "feat");
    assert_eq!(prompt.source_prompts, vec![branch.clone()]);
    assert!(report.branch_cleanup.iter().any(|outcome| matches!(
        outcome,
        remove::BranchCleanupOutcome::DeletedSourceBranch { branch: deleted } if deleted == &branch
    )));
}

#[test]
fn remove_with_cleanup_skips_default_branch_even_when_not_checked_out() {
    let fixture = AbcFixture::new();
    ensure_origin_head(&fixture);
    fixture.create_source_branch("dev").expect("create dev");
    switch_source(&fixture, "dev");
    let main = BranchName::parse("main".to_owned()).expect("main branch");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(main.clone()))
        .expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let mut prompt = TestPrompt::new([true], []);

    let report = remove::run_with_cleanup(
        &source,
        remove::RemoveOptions {
            selector: OutpostSelector::from_path(outpost),
            force: false,
        },
        remove::BranchCleanupMode::Prompt(remove::BranchCleanupOptions {
            provider: None,
            prompt: &mut prompt,
        }),
    )
    .expect("remove main outpost");

    assert_source_branch_exists(&source, "main");
    assert!(prompt.source_prompts.is_empty());
    assert!(report.branch_cleanup.iter().any(|outcome| matches!(
        outcome,
        remove::BranchCleanupOutcome::Skipped {
            branch: Some(branch),
            reason: remove::BranchCleanupSkipReason::DefaultBranch,
        } if branch == &main
    )));
}

#[test]
fn remove_with_cleanup_skips_checked_out_source_branch() {
    let fixture = AbcFixture::new();
    ensure_origin_head(&fixture);
    let branch = fixture.create_source_branch("feat").expect("create feat");
    switch_source(&fixture, "feat");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let mut prompt = TestPrompt::new([true], []);

    let report = remove::run_with_cleanup(
        &source,
        remove::RemoveOptions {
            selector: OutpostSelector::from_path(outpost),
            force: false,
        },
        remove::BranchCleanupMode::Prompt(remove::BranchCleanupOptions {
            provider: None,
            prompt: &mut prompt,
        }),
    )
    .expect("remove checked-out branch outpost");

    assert_source_branch_exists(&source, "feat");
    assert!(prompt.source_prompts.is_empty());
    assert!(report.branch_cleanup.iter().any(|outcome| matches!(
        outcome,
        remove::BranchCleanupOutcome::Skipped {
            branch: Some(skipped),
            reason: remove::BranchCleanupSkipReason::BranchCheckedOut,
        } if skipped == &branch
    )));
}

#[test]
fn remove_with_cleanup_accepts_matching_merged_pr_proof() {
    let fixture = AbcFixture::new();
    ensure_origin_head(&fixture);
    let branch = source_branch_with_unmerged_commit(&fixture, "feat");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let source_oid = source
        .branch_oid(&branch)
        .expect("source oid")
        .expect("branch oid");
    let provider = FakeProvider {
        proof: Some(remove::MergedPullRequest {
            id: "#12".to_owned(),
            head_ref_name: branch.clone(),
            head_ref_oid: source_oid,
        }),
    };
    let mut prompt = TestPrompt::new([true], []);

    let report = remove::run_with_cleanup(
        &source,
        remove::RemoveOptions {
            selector: OutpostSelector::from_path(outpost),
            force: false,
        },
        remove::BranchCleanupMode::Prompt(remove::BranchCleanupOptions {
            provider: Some(&provider),
            prompt: &mut prompt,
        }),
    )
    .expect("remove with pr proof");

    assert_source_branch_missing(&source, "feat");
    assert_eq!(prompt.source_prompts, vec![branch.clone()]);
    assert!(report.branch_cleanup.iter().any(|outcome| matches!(
        outcome,
        remove::BranchCleanupOutcome::DeletedSourceBranch { branch: deleted } if deleted == &branch
    )));
}

#[test]
fn remove_with_cleanup_rejects_mismatched_merged_pr_proof() {
    let fixture = AbcFixture::new();
    ensure_origin_head(&fixture);
    let branch = source_branch_with_unmerged_commit(&fixture, "feat");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let provider = FakeProvider {
        proof: Some(remove::MergedPullRequest {
            id: "#12".to_owned(),
            head_ref_name: branch.clone(),
            head_ref_oid: "0000000000000000000000000000000000000000".to_owned(),
        }),
    };
    let mut prompt = TestPrompt::new([true], []);

    let report = remove::run_with_cleanup(
        &source,
        remove::RemoveOptions {
            selector: OutpostSelector::from_path(outpost),
            force: false,
        },
        remove::BranchCleanupMode::Prompt(remove::BranchCleanupOptions {
            provider: Some(&provider),
            prompt: &mut prompt,
        }),
    )
    .expect("remove with mismatched pr proof");

    assert_source_branch_exists(&source, "feat");
    assert!(prompt.source_prompts.is_empty());
    assert!(report.branch_cleanup.iter().any(|outcome| matches!(
        outcome,
        remove::BranchCleanupOutcome::Skipped {
            branch: Some(skipped),
            reason: remove::BranchCleanupSkipReason::NoProof,
        } if skipped == &branch
    )));
}

#[test]
fn remove_with_cleanup_prompts_separately_for_upstream_branch() {
    let fixture = AbcFixture::new();
    ensure_origin_head(&fixture);
    let branch = fixture.create_source_branch("feat").expect("create feat");
    fixture
        .push_source_branch(&branch)
        .expect("push feat to origin");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let mut prompt = TestPrompt::new([true], [false]);

    let report = remove::run_with_cleanup(
        &source,
        remove::RemoveOptions {
            selector: OutpostSelector::from_path(outpost),
            force: false,
        },
        remove::BranchCleanupMode::Prompt(remove::BranchCleanupOptions {
            provider: None,
            prompt: &mut prompt,
        }),
    )
    .expect("remove with upstream branch");

    assert_source_branch_missing(&source, "feat");
    assert_origin_branch_exists(&source, "feat");
    assert_eq!(prompt.source_prompts, vec![branch.clone()]);
    assert_eq!(prompt.upstream_prompts, vec![branch.clone()]);
    assert!(report.branch_cleanup.iter().any(|outcome| matches!(
        outcome,
        remove::BranchCleanupOutcome::DeclinedUpstreamBranch {
            remote,
            branch: declined,
        } if remote.as_str() == "origin" && declined == &branch
    )));
}

#[test]
fn remove_with_cleanup_declined_source_prompt_leaves_branches_intact() {
    let fixture = AbcFixture::new();
    ensure_origin_head(&fixture);
    let branch = fixture.create_source_branch("feat").expect("create feat");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add C");
    let source = fixture.source_repo().expect("source repo");
    let mut prompt = TestPrompt::new([false], []);

    let report = remove::run_with_cleanup(
        &source,
        remove::RemoveOptions {
            selector: OutpostSelector::from_path(outpost),
            force: false,
        },
        remove::BranchCleanupMode::Prompt(remove::BranchCleanupOptions {
            provider: None,
            prompt: &mut prompt,
        }),
    )
    .expect("remove with declined cleanup");

    assert_source_branch_exists(&source, "feat");
    assert_eq!(prompt.source_prompts, vec![branch.clone()]);
    assert!(report.branch_cleanup.iter().any(|outcome| matches!(
        outcome,
        remove::BranchCleanupOutcome::DeclinedSourceBranch { branch: declined } if declined == &branch
    )));
}

#[test]
fn remove_with_cleanup_source_branch_oid_race_leaves_branch_intact() {
    let fixture = AbcFixture::new();
    ensure_origin_head(&fixture);
    let branch = fixture.create_source_branch("feat").expect("create feat");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add C");
    let new_oid = fixture
        .commit_in_source("main moved locally")
        .expect("commit on main");
    let source = fixture.source_repo().expect("source repo");
    let mut prompt = RacePrompt {
        fixture: &fixture,
        branch: branch.clone(),
        new_oid: new_oid.clone(),
    };

    let report = remove::run_with_cleanup(
        &source,
        remove::RemoveOptions {
            selector: OutpostSelector::from_path(outpost),
            force: false,
        },
        remove::BranchCleanupMode::Prompt(remove::BranchCleanupOptions {
            provider: None,
            prompt: &mut prompt,
        }),
    )
    .expect("remove with branch race");

    assert_eq!(
        source.branch_oid(&branch).expect("branch oid").as_deref(),
        Some(new_oid.as_str())
    );
    assert!(report.branch_cleanup.iter().any(|outcome| matches!(
        outcome,
        remove::BranchCleanupOutcome::Warning {
            branch: Some(warned),
            ..
        } if warned == &branch
    )));
}

#[test]
fn remove_force_does_not_bypass_branch_cleanup_proof() {
    let fixture = AbcFixture::new();
    ensure_origin_head(&fixture);
    let branch = source_branch_with_unmerged_commit(&fixture, "feat");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add C");
    fixture
        .commit_file_in_outpost(&outpost, "dirty proof bypass check", "x.txt", "x\n")
        .expect("outpost-only commit");
    let source = fixture.source_repo().expect("source repo");
    let mut prompt = TestPrompt::new([true], []);

    let report = remove::run_with_cleanup(
        &source,
        remove::RemoveOptions {
            selector: OutpostSelector::from_path(outpost),
            force: true,
        },
        remove::BranchCleanupMode::Prompt(remove::BranchCleanupOptions {
            provider: None,
            prompt: &mut prompt,
        }),
    )
    .expect("force remove without cleanup proof");

    assert_source_branch_exists(&source, "feat");
    assert!(prompt.source_prompts.is_empty());
    assert!(report.branch_cleanup.iter().any(|outcome| matches!(
        outcome,
        remove::BranchCleanupOutcome::Skipped {
            branch: Some(skipped),
            reason: remove::BranchCleanupSkipReason::OutpostHeadMismatch,
        } if skipped == &branch
    )));
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

fn assert_source_branch_missing(source: &SourceRepo, branch: &str) {
    let branch = BranchName::parse(branch.to_owned()).expect("branch name");
    assert!(
        !source.branch_exists(&branch).expect("branch exists query"),
        "source branch {} should be deleted",
        branch.as_str()
    );
}

fn assert_origin_branch_exists(source: &SourceRepo, branch: &str) {
    let branch = BranchName::parse(branch.to_owned()).expect("branch name");
    assert!(
        source
            .origin_branch_oid(&branch)
            .expect("origin branch oid")
            .is_some(),
        "origin branch {} should remain",
        branch.as_str()
    );
}

fn ensure_origin_head(fixture: &AbcFixture) {
    fixture
        .invoker(&fixture.source)
        .run_check(["remote", "set-head", "origin", "main"])
        .expect("set origin head");
}

fn switch_source(fixture: &AbcFixture, branch: &str) {
    fixture
        .invoker(&fixture.source)
        .run_check(["switch", branch])
        .expect("switch source branch");
}

fn source_branch_with_unmerged_commit(fixture: &AbcFixture, branch: &str) -> BranchName {
    let branch = fixture
        .create_source_branch(branch)
        .expect("create source branch");
    switch_source(fixture, branch.as_str());
    fixture
        .commit_in_source("feature commit")
        .expect("feature commit");
    switch_source(fixture, "main");
    branch
}

struct FakeProvider {
    proof: Option<remove::MergedPullRequest>,
}

impl remove::BranchCleanupProvider for FakeProvider {
    fn merged_pull_request(
        &self,
        _branch: &BranchName,
        _source_oid: &str,
    ) -> OutpostResult<Option<remove::MergedPullRequest>> {
        Ok(self.proof.clone())
    }
}

struct TestPrompt {
    source_responses: VecDeque<bool>,
    upstream_responses: VecDeque<bool>,
    source_prompts: Vec<BranchName>,
    upstream_prompts: Vec<BranchName>,
}

impl TestPrompt {
    fn new<const S: usize, const U: usize>(source: [bool; S], upstream: [bool; U]) -> Self {
        Self {
            source_responses: VecDeque::from(source),
            upstream_responses: VecDeque::from(upstream),
            source_prompts: Vec::new(),
            upstream_prompts: Vec::new(),
        }
    }
}

impl remove::BranchCleanupPrompt for TestPrompt {
    fn confirm_source_branch_delete(&mut self, candidate: &remove::BranchCleanupCandidate) -> bool {
        self.source_prompts.push(candidate.branch.clone());
        self.source_responses.pop_front().unwrap_or(false)
    }

    fn confirm_upstream_branch_delete(
        &mut self,
        candidate: &remove::BranchCleanupCandidate,
    ) -> bool {
        self.upstream_prompts.push(candidate.branch.clone());
        self.upstream_responses.pop_front().unwrap_or(false)
    }
}

struct RacePrompt<'a> {
    fixture: &'a AbcFixture,
    branch: BranchName,
    new_oid: String,
}

impl remove::BranchCleanupPrompt for RacePrompt<'_> {
    fn confirm_source_branch_delete(
        &mut self,
        _candidate: &remove::BranchCleanupCandidate,
    ) -> bool {
        let branch_ref = format!("refs/heads/{}", self.branch.as_str());
        self.fixture
            .invoker(&self.fixture.source)
            .run_check(["update-ref", &branch_ref, &self.new_oid])
            .expect("move branch during cleanup prompt");
        true
    }

    fn confirm_upstream_branch_delete(
        &mut self,
        _candidate: &remove::BranchCleanupCandidate,
    ) -> bool {
        false
    }
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
