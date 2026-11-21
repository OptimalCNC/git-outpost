#[allow(dead_code)]
mod common;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use outpost_core::ops::cleanup_evidence::{
    CleanupEvidenceProvider, CleanupEvidenceRequest, CleanupEvidenceSnapshot, ObservedRemoteBranch,
};
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
fn remove_preserves_unpushed_work_even_when_git_rewrites_the_source_url() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let outpost_tip = fixture
        .commit_in_outpost(&outpost, "outpost-only commit")
        .expect("commit in outpost");
    let source = fixture.source_repo().expect("source repo");
    let source_tip = fixture
        .rev_parse(&fixture.source, "HEAD")
        .expect("source tip");
    let replica = fixture.root.join("old-source.git");
    let git = fixture.invoker(&outpost);
    git.run_check([
        OsStr::new("clone"),
        OsStr::new("--bare"),
        outpost.as_os_str(),
        replica.as_os_str(),
    ])
    .expect("make an old read replica containing the outpost work");
    let rewrite = format!("url.{}.insteadOf", replica.display());
    git.run_check([
        OsStr::new("config"),
        OsStr::new(&rewrite),
        crate::canonical(&fixture.source).as_os_str(),
    ])
    .expect("redirect Git reads of the source to the replica");

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
    assert_eq!(fixture.rev_parse(&outpost, "HEAD").unwrap(), outpost_tip);
    assert_eq!(
        fixture.rev_parse(&fixture.source, "HEAD").unwrap(),
        source_tip
    );
}

#[test]
fn remove_preserves_stashed_work_in_a_clean_outpost() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add outpost");
    let source = fixture.source_repo().expect("source repo");
    fs::write(outpost.join("notes"), "unfinished draft").expect("write draft");
    let git = fixture.invoker(&outpost);
    git.run_check(["stash", "push", "--include-untracked"])
        .expect("stash draft");
    let stash = fixture
        .rev_parse(&outpost, "refs/stash")
        .expect("stash tip");

    let error = remove::run(
        &source,
        remove::RemoveOptions {
            selector: OutpostSelector::from_path(outpost.clone()),
            force: false,
        },
    )
    .expect_err("stashed work needs explicit force");

    assert!(
        matches!(error, OutpostError::StashedChanges { repo, hint } if repo == canonical(&outpost) && hint == "pass --force")
    );
    assert_eq!(single_entry(&source).path, canonical(&outpost));
    assert_eq!(
        fixture
            .rev_parse(&outpost, "refs/stash")
            .expect("preserved stash"),
        stash
    );
}

#[test]
fn remove_preserves_a_side_branch_until_its_commit_is_saved_in_the_source() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add outpost");
    let source = fixture.source_repo().expect("source repo");
    let git = fixture.invoker(&outpost);
    git.run_check(["switch", "-c", "scratch"])
        .expect("start side branch");
    let tip = fixture
        .commit_in_outpost(&outpost, "side work")
        .expect("side branch commit");
    git.run_check(["push", "local", "scratch:refs/heads/temporary"])
        .expect("temporarily publish work");
    git.run_check(["push", "local", "--delete", "temporary"])
        .expect("drop the source's reference");
    git.run_check(["switch", "main"])
        .expect("return to synchronized branch");
    let options = || remove::RemoveOptions {
        selector: OutpostSelector::from_path(outpost.clone()),
        force: false,
    };

    let error =
        remove::run(&source, options()).expect_err("unpublished side work needs explicit force");

    assert!(matches!(error, OutpostError::UnpushedCommits { branch, .. } if branch == "scratch"));
    assert_eq!(single_entry(&source).path, canonical(&outpost));
    assert_eq!(
        fixture
            .rev_parse(&outpost, "refs/heads/scratch")
            .expect("preserved side branch"),
        tip
    );
    git.run_check(["push", "local", "scratch:refs/heads/saved-scratch"])
        .expect("save side branch in source");
    remove::run(&source, options()).expect("remove once the side work is saved");
    assert!(!outpost.exists());
    assert_registry_empty(&source);
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/saved-scratch")
            .expect("saved work"),
        tip
    );
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
    let provider = FakeProvider::new(None);
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
    .expect("remove checked-out branch outpost");

    assert_source_branch_exists(&source, "feat");
    assert!(prompt.source_prompts.is_empty());
    assert_eq!(provider.calls.get(), 0);
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
    let mut snapshot = cleanup_snapshot(
        &source,
        Some(remove::MergedPullRequest {
            id: "#12".to_owned(),
            head_ref_name: branch.clone(),
            head_ref_oid: source_oid.clone(),
        }),
    );
    snapshot
        .default_branch
        .as_mut()
        .expect("default branch")
        .oid = fixture
        .commit_in_upstream("main", "new upstream default tip")
        .expect("remote default ahead of source");
    let provider = FakeProvider::new(Some(snapshot));
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
    assert_eq!(provider.calls.get(), 1);
    let requests = provider.requests.borrow();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].upstream_remote.as_str(), "origin");
    assert_eq!(requests[0].upstream_url, fixture.upstream.to_string_lossy());
    assert_eq!(requests[0].branch, branch);
    assert_eq!(requests[0].source_oid, source_oid);
    assert!(report.branch_cleanup.iter().any(|outcome| matches!(
        outcome,
        remove::BranchCleanupOutcome::DeletedSourceBranch { branch: deleted } if deleted == &branch
    )));
}

#[test]
fn remove_with_cleanup_fetches_default_only_when_ancestry_needs_missing_object() {
    let fixture = AbcFixture::new();
    ensure_origin_head(&fixture);
    let branch = fixture.create_source_branch("feat").expect("create feat");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add C");
    let default_oid = fixture
        .commit_in_upstream("main", "advance upstream default")
        .expect("advance default");
    let source = fixture.source_repo().expect("source repo");
    assert!(
        !source
            .has_commit_oid(&default_oid)
            .expect("missing default object before analysis")
    );
    let provider = FakeProvider::new(Some(CleanupEvidenceSnapshot {
        default_branch: Some(ObservedRemoteBranch {
            branch: BranchName::parse("main".to_owned()).expect("main"),
            oid: default_oid.clone(),
        }),
        upstream_oid: None,
        merged_pull_request: None,
    }));
    let mut prompt = TestPrompt::new([true], []);

    remove::run_with_cleanup(
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
    .expect("remove with conditional default fetch");

    assert_source_branch_missing(&source, "feat");
    assert!(
        source
            .has_commit_oid(&default_oid)
            .expect("default object after analysis")
    );
    assert_eq!(provider.calls.get(), 1);
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
    let provider = FakeProvider::new(Some(cleanup_snapshot(
        &source,
        Some(remove::MergedPullRequest {
            id: "#12".to_owned(),
            head_ref_name: branch.clone(),
            head_ref_oid: fixture
                .rev_parse(&fixture.source, "refs/heads/feat^")
                .expect("previous feature tip"),
        }),
    )));
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
fn cleanup_keeps_a_collaborators_unmerged_upstream_tip() {
    let fixture = AbcFixture::new();
    let branch = fixture.create_source_branch("feat").expect("create branch");
    fixture
        .push_source_branch(&branch)
        .expect("publish feature");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add outpost");
    let new_tip = fixture
        .commit_in_upstream("feat", "collaborator's unpublished-to-main work")
        .expect("advance upstream feature");
    let source = fixture.source_repo().expect("source repo");
    let mut prompt = TestPrompt::new([true], [true]);

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
    .expect("remove merged local branch");

    assert_source_branch_missing(&source, "feat");
    assert_eq!(
        source.origin_branch_oid(&branch).expect("upstream feature"),
        Some(new_tip)
    );
    assert!(prompt.upstream_prompts.is_empty());
    assert!(report.branch_cleanup.iter().any(|outcome| matches!(outcome, remove::BranchCleanupOutcome::Warning { branch: Some(warned), message } if warned == &branch && message.contains("tip differs"))));
}

#[test]
fn cleanup_preserves_a_branch_checked_out_during_confirmation() {
    let fixture = AbcFixture::new();
    let branch = fixture.create_source_branch("feat").expect("create branch");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add outpost");
    let source = fixture.source_repo().expect("source repo");
    let before = source.branch_oid(&branch).expect("original branch");
    let checkout = fixture.root.join("native");
    let mut prompt = PromptActions {
        source: || {
            fixture
                .invoker(&fixture.source)
                .run_check([
                    OsStr::new("worktree"),
                    OsStr::new("add"),
                    checkout.as_os_str(),
                    OsStr::new("feat"),
                ])
                .expect("check out feature before confirming");
            true
        },
        upstream: || false,
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
    .expect("finish outpost removal");

    assert_eq!(
        source
            .branch_oid(&branch)
            .expect("checked out branch retained"),
        before
    );
    assert_eq!(
        fixture.rev_parse(&checkout, "HEAD").expect("live checkout"),
        before.expect("original commit")
    );
    assert!(
        report
            .branch_cleanup
            .contains(&remove::BranchCleanupOutcome::Skipped {
                branch: Some(branch),
                reason: remove::BranchCleanupSkipReason::BranchCheckedOut
            })
    );
}

#[test]
fn cleanup_requires_the_same_single_upstream_destination_through_confirmation() {
    enum Configuration {
        MultiplePushUrls,
        ChangedDuringConfirmation,
        DirectUrlWouldBeRewritten,
    }
    use Configuration::*;
    for config in [
        MultiplePushUrls,
        ChangedDuringConfirmation,
        DirectUrlWouldBeRewritten,
    ] {
        let fixture = AbcFixture::new();
        let branch = fixture.create_source_branch("feat").expect("create branch");
        fixture
            .push_source_branch(&branch)
            .expect("publish feature");
        let outpost = fixture
            .add_outpost_on_branch("C", Some(branch.clone()))
            .expect("add outpost");
        let other = fixture.root.join("old-upstream.git");
        fixture
            .invoker(&fixture.root)
            .run_check([
                OsStr::new("clone"),
                OsStr::new("--bare"),
                fixture.upstream.as_os_str(),
                other.as_os_str(),
            ])
            .expect("clone old upstream with the same feature tip");
        let original = fixture
            .rev_parse(&fixture.upstream, "refs/heads/feat")
            .expect("feature tip");
        let git = fixture.invoker(&fixture.source);
        assert_eq!(
            fixture
                .rev_parse(&fixture.source, "refs/remotes/origin/feat")
                .expect("feature is tracked"),
            original
        );
        let observed_url = git
            .run_capture(["remote", "get-url", "origin"])
            .expect("origin URL");
        if matches!(config, MultiplePushUrls | DirectUrlWouldBeRewritten) {
            git.run_check(["remote", "set-url", "--push", "origin", &observed_url])
                .expect("pin origin's first push URL");
        }
        match config {
            MultiplePushUrls => {
                git.run_check([
                    OsStr::new("remote"),
                    OsStr::new("set-url"),
                    OsStr::new("--add"),
                    OsStr::new("--push"),
                    OsStr::new("origin"),
                    other.as_os_str(),
                ])
                .expect("configure push destinations");
            }
            DirectUrlWouldBeRewritten => {
                let mut setting = OsString::from("url.");
                setting.push(other.as_os_str());
                setting.push(".pushInsteadOf");
                git.run_check([OsStr::new("config"), &setting, OsStr::new(&observed_url)])
                    .expect("redirect bare URL pushes to the old repository");
            }
            ChangedDuringConfirmation => {}
        }
        let upstream_prompted = Cell::new(false);
        let mut prompt = PromptActions {
            source: || true,
            upstream: || {
                upstream_prompted.set(true);
                if matches!(config, ChangedDuringConfirmation) {
                    git.run_check([
                        OsStr::new("remote"),
                        OsStr::new("set-url"),
                        OsStr::new("origin"),
                        other.as_os_str(),
                    ])
                    .expect("change remote while confirming deletion");
                }
                true
            },
        };
        let source = fixture.source_repo().expect("source repo");

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
        .expect("outpost removal");

        assert_source_branch_missing(&source, "feat");
        assert_eq!(upstream_prompted.get(), !matches!(config, MultiplePushUrls));
        assert_eq!(
            fixture
                .rev_parse(&other, "refs/heads/feat")
                .expect("old repository is untouched"),
            original
        );
        if matches!(config, DirectUrlWouldBeRewritten) {
            assert!(report.branch_cleanup.iter().any(|outcome| matches!(outcome, remove::BranchCleanupOutcome::DeletedUpstreamBranch { branch: deleted, .. } if deleted == &branch)), "{:?}", report.branch_cleanup);
            assert!(
                !fixture
                    .invoker(&fixture.upstream)
                    .run_status(["show-ref", "--verify", "--quiet", "refs/heads/feat"])
                    .expect("inspected upstream branch")
            );
            assert!(
                !git.run_status([
                    "show-ref",
                    "--verify",
                    "--quiet",
                    "refs/remotes/origin/feat"
                ])
                .expect("source tracking ref")
            );
        } else {
            assert_eq!(
                fixture
                    .rev_parse(&fixture.upstream, "refs/heads/feat")
                    .expect("inspected upstream is untouched"),
                original
            );
            assert!(report.branch_cleanup.iter().any(|outcome| matches!(outcome, remove::BranchCleanupOutcome::Warning { branch: Some(warned), message } if warned == &branch && message.contains("upstream branch was not deleted"))));
        }
    }
}

#[test]
fn remove_with_cleanup_uses_final_lease_when_upstream_moves_after_source_prompt() {
    let fixture = AbcFixture::new();
    ensure_origin_head(&fixture);
    let branch = fixture.create_source_branch("feat").expect("create feat");
    fixture
        .push_source_branch(&branch)
        .expect("push feat to origin");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("add C");
    let moved_oid = fixture
        .commit_in_upstream("main", "remote branch race target")
        .expect("create remote race target");
    let source = fixture.source_repo().expect("source repo");
    let source_oid = source
        .branch_oid(&branch)
        .expect("source branch query")
        .expect("source branch");
    let upstream_prompted = Cell::new(false);
    let mut prompt = PromptActions {
        source: || {
            fixture
                .invoker(&fixture.upstream)
                .run_check(["update-ref", "refs/heads/feat", &moved_oid])
                .expect("move upstream branch during confirmation");
            true
        },
        upstream: || {
            upstream_prompted.set(true);
            true
        },
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
    .expect("outpost removal survives leased remote deletion failure");

    assert_source_branch_missing(&source, "feat");
    assert!(upstream_prompted.get());
    assert_eq!(
        fixture
            .invoker(&fixture.upstream)
            .run_capture(["rev-parse", "refs/heads/feat"])
            .expect("remote feat OID"),
        moved_oid
    );
    assert!(report.branch_cleanup.iter().any(|outcome| matches!(
        outcome,
        remove::BranchCleanupOutcome::Warning {
            branch: Some(warned),
            message,
        } if warned == &branch && message.contains("upstream branch was not deleted")
    )));

    let commands = source.git_argv_log_for_tests();
    let source_delete = commands
        .iter()
        .position(|argv| {
            argv == &vec![
                OsString::from("update-ref"),
                OsString::from("-d"),
                OsString::from("refs/heads/feat"),
                OsString::from(source_oid.as_str()),
            ]
        })
        .expect("exact source deletion command");
    let after_source_delete = &commands[source_delete + 1..];
    assert!(
        after_source_delete
            .iter()
            .all(|argv| argv.first() != Some(&OsString::from("ls-remote"))),
        "no upstream observation should run after source deletion: {after_source_delete:?}"
    );
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
    let mut prompt = PromptActions {
        source: || {
            fixture
                .invoker(&fixture.source)
                .run_check(["update-ref", "refs/heads/feat", &new_oid])
                .expect("move source branch during confirmation");
            true
        },
        upstream: || false,
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

fn cleanup_snapshot(
    source: &SourceRepo,
    proof: Option<remove::MergedPullRequest>,
) -> CleanupEvidenceSnapshot {
    let main = BranchName::parse("main".to_owned()).expect("main");
    CleanupEvidenceSnapshot {
        default_branch: Some(ObservedRemoteBranch {
            oid: source
                .branch_oid(&main)
                .expect("default branch query")
                .expect("default branch"),
            branch: main,
        }),
        upstream_oid: None,
        merged_pull_request: proof,
    }
}

struct FakeProvider {
    snapshot: Option<CleanupEvidenceSnapshot>,
    calls: Cell<usize>,
    requests: RefCell<Vec<CleanupEvidenceRequest>>,
}

impl FakeProvider {
    fn new(snapshot: Option<CleanupEvidenceSnapshot>) -> Self {
        Self {
            snapshot,
            calls: Cell::new(0),
            requests: RefCell::new(Vec::new()),
        }
    }
}

impl CleanupEvidenceProvider for FakeProvider {
    fn snapshot(
        &self,
        request: &CleanupEvidenceRequest,
    ) -> OutpostResult<Option<CleanupEvidenceSnapshot>> {
        self.calls.set(self.calls.get() + 1);
        self.requests.borrow_mut().push(request.clone());
        Ok(self.snapshot.clone())
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

struct PromptActions<S, U> {
    source: S,
    upstream: U,
}

impl<S: FnMut() -> bool, U: FnMut() -> bool> remove::BranchCleanupPrompt for PromptActions<S, U> {
    fn confirm_source_branch_delete(&mut self, _: &remove::BranchCleanupCandidate) -> bool {
        (self.source)()
    }

    fn confirm_upstream_branch_delete(&mut self, _: &remove::BranchCleanupCandidate) -> bool {
        (self.upstream)()
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
