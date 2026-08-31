mod common;

use std::ffi::OsStr;
use std::fs;

use common::fixture::AbcFixture;
use outpost_core::{BranchName, OutpostError, RemoteName, SourceRepo};

#[test]
fn remote_branch_oid_distinguishes_present_and_absent_refs() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let expected = fixture
        .rev_parse(&fixture.upstream, "refs/heads/main")
        .expect("upstream main oid");

    assert_eq!(
        source
            .origin_branch_oid(&branch("main"))
            .expect("origin main oid"),
        Some(expected)
    );
    assert_eq!(
        source
            .remote_branch_oid(&remote("origin"), &branch("missing"))
            .expect("absent remote branch"),
        None
    );
}

#[test]
fn remote_default_branch_requires_and_parses_the_local_remote_head_symref() {
    let fixture = AbcFixture::new();
    let git = fixture.invoker(&fixture.source);
    git.run_check(["remote", "set-head", "origin", "main"])
        .expect("create origin HEAD");
    let source = fixture.source_repo().expect("source repo");

    assert_eq!(
        source
            .origin_default_branch()
            .expect("local origin default"),
        Some(branch("main"))
    );

    git.run_check(["symbolic-ref", "--delete", "refs/remotes/origin/HEAD"])
        .expect("delete local origin HEAD");
    assert_eq!(
        source
            .remote_default_branch(&remote("origin"))
            .expect("missing local remote HEAD"),
        None
    );
}

#[test]
fn fetch_remote_default_branch_discovers_remote_head_and_fetches_exact_ref() {
    let fixture = AbcFixture::new();
    let git = fixture.invoker(&fixture.source);
    if git
        .run_status(["symbolic-ref", "--quiet", "refs/remotes/origin/HEAD"])
        .expect("probe origin HEAD")
    {
        git.run_check(["symbolic-ref", "--delete", "refs/remotes/origin/HEAD"])
            .expect("delete local origin HEAD");
    }
    let source = fixture.source_repo().expect("source repo");

    let (default_branch, oid) = source
        .fetch_origin_default_branch()
        .expect("fetch default branch")
        .expect("remote advertises a default branch");

    assert_eq!(default_branch, branch("main"));
    assert_eq!(
        oid,
        fixture
            .rev_parse(&fixture.upstream, "refs/heads/main")
            .expect("upstream main oid")
    );
}

#[test]
fn fetch_remote_default_branch_uses_an_existing_local_remote_head() {
    let fixture = AbcFixture::new();
    fixture
        .invoker(&fixture.source)
        .run_check(["remote", "set-head", "origin", "main"])
        .expect("create origin HEAD");
    let source = fixture.source_repo().expect("source repo");

    let result = source
        .fetch_remote_default_branch(&remote("origin"))
        .expect("fetch local default branch")
        .expect("local default branch");

    assert_eq!(result.0, branch("main"));
    assert_eq!(
        result.1,
        fixture
            .rev_parse(&fixture.upstream, "refs/heads/main")
            .expect("upstream main oid")
    );
}

#[test]
fn fetch_remote_branches_updates_requested_ref() {
    let fixture = AbcFixture::new();
    let feature = fixture
        .create_source_branch("feature/z")
        .expect("create feature");
    fixture
        .push_source_branch(&feature)
        .expect("publish feature");
    let source = fixture.source_repo().expect("source repo");
    source
        .fetch_remote_branches(&remote("origin"), std::slice::from_ref(&feature))
        .expect("prime remote-tracking feature");
    let stale_oid = fixture
        .rev_parse(&fixture.source, "refs/remotes/origin/feature/z")
        .expect("stale local feature ref");
    let expected = fixture
        .commit_in_upstream("feature/z", "advance feature")
        .expect("advance upstream feature");
    assert_ne!(stale_oid, expected, "upstream feature must advance");
    let main = branch("main");

    source
        .fetch_remote_branches(&remote("origin"), &[feature.clone(), main])
        .expect("fetch branches");

    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/remotes/origin/feature/z")
            .expect("fetched feature"),
        expected
    );
}

#[test]
fn checked_out_branch_queries_handle_detached_primary_and_linked_worktrees() {
    let fixture = AbcFixture::new();
    let feature = fixture
        .create_source_branch("feature/linked")
        .expect("create feature");
    let linked = fixture.root.join("linked-feature");
    let git = fixture.invoker(&fixture.source);
    git.run_check([
        OsStr::new("worktree"),
        OsStr::new("add"),
        linked.as_os_str(),
        OsStr::new(feature.as_str()),
    ])
    .expect("add linked worktree");
    let attached_source = SourceRepo::at(&fixture.source).expect("attached source repo");
    let attached_branches = attached_source
        .checked_out_branches()
        .expect("attached checked out branches");
    assert!(attached_branches.contains(&branch("main")));
    assert!(attached_branches.contains(&feature));
    git.run_check(["switch", "--detach"])
        .expect("detach primary worktree");
    let source = fixture.source_repo().expect("source repo");

    assert_eq!(
        source.checked_out_branches().expect("checked out branches"),
        vec![feature.clone()]
    );
    assert!(
        source
            .is_branch_checked_out(&feature)
            .expect("feature checkout")
    );
    assert!(
        !source
            .is_branch_checked_out(&branch("main"))
            .expect("main checkout")
    );
    assert_eq!(
        source
            .checked_out_worktree_for(&feature)
            .expect("feature worktree"),
        Some(fs::canonicalize(&linked).expect("canonical linked worktree"))
    );

    let linked_source = SourceRepo::at(&linked).expect("linked source repo");
    assert_ne!(linked_source.git_dir(), linked_source.git_common_dir());
    assert_eq!(linked_source.git_common_dir(), source.git_common_dir());
}

#[test]
fn branch_and_commit_probes_distinguish_missing_refs_and_history_direction() {
    let fixture = AbcFixture::new();
    let initial = fixture
        .rev_parse(&fixture.source, "HEAD")
        .expect("initial oid");
    fixture
        .create_source_branch("before")
        .expect("create before branch");
    let current = fixture
        .commit_in_source("advance main")
        .expect("advance main");
    let source = fixture.source_repo().expect("source repo");

    assert_eq!(
        source.branch_oid(&branch("before")).expect("before oid"),
        Some(initial.clone())
    );
    assert_eq!(
        source
            .branch_oid(&branch("missing"))
            .expect("missing branch"),
        None
    );
    assert!(source.has_commit_oid(&current).expect("known commit"));
    assert!(
        !source
            .has_commit_oid(&"0".repeat(40))
            .expect("unknown commit")
    );
    assert!(
        source
            .is_ancestor_oid(&initial, &current)
            .expect("forward ancestry")
    );
    assert!(
        !source
            .is_ancestor_oid(&current, &initial)
            .expect("reverse ancestry")
    );
}

#[test]
fn current_branch_reports_detached_head_as_a_branch_not_found_error() {
    let fixture = AbcFixture::new();
    fixture
        .invoker(&fixture.source)
        .run_check(["switch", "--detach"])
        .expect("detach source HEAD");
    let source = fixture.source_repo().expect("source repo");

    assert!(matches!(
        source.current_branch().expect_err("detached HEAD has no branch"),
        OutpostError::BranchNotFound { branch, repo }
            if branch == "HEAD" && repo == fs::canonicalize(&fixture.source).expect("canonical source")
    ));
}

#[test]
fn fast_forward_leaves_a_local_branch_unchanged_when_origin_is_behind() {
    let fixture = AbcFixture::new();
    let local_oid = fixture
        .commit_in_source("local advance")
        .expect("local commit");
    let source = fixture.source_repo().expect("source repo");

    source
        .fast_forward_branch_from_origin(&branch("main"))
        .expect("origin-behind refresh is a no-op");

    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/main")
            .expect("local main after refresh"),
        local_oid
    );
}

#[test]
fn fast_forward_reports_a_missing_source_branch() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");

    let err = source
        .fast_forward_branch_from_origin(&branch("feature/missing"))
        .expect_err("missing source branch must fail");

    assert!(matches!(
        err,
        OutpostError::BranchNotFound { branch, repo }
            if branch == "feature/missing"
                && repo == fs::canonicalize(&fixture.source).expect("canonical source")
    ));
}

#[test]
fn fast_forward_rejects_diverged_local_and_origin_histories() {
    let fixture = AbcFixture::new();
    fixture
        .commit_in_source("local divergence")
        .expect("local divergence commit");
    fixture
        .commit_in_upstream("main", "origin divergence")
        .expect("origin divergence commit");
    let source = fixture.source_repo().expect("source repo");

    assert!(matches!(
        source
            .fast_forward_branch_from_origin(&branch("main"))
            .expect_err("divergent history must fail"),
        OutpostError::Divergence { branch } if branch == "main"
    ));
}

#[test]
fn fast_forward_updates_an_unchecked_branch() {
    let fixture = AbcFixture::new();
    let feature = fixture
        .create_source_branch("feature/refresh")
        .expect("create feature");
    fixture
        .push_source_branch(&feature)
        .expect("publish feature");
    let new_oid = fixture
        .commit_in_upstream("feature/refresh", "advance feature")
        .expect("upstream feature commit");
    let source = fixture.source_repo().expect("source repo");

    source
        .fast_forward_branch_from_origin(&feature)
        .expect("fast-forward unchecked feature");

    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/feature/refresh")
            .expect("updated feature oid"),
        new_oid
    );
}

#[test]
fn fast_forward_updates_a_checked_out_linked_worktree_with_ff_only_merge() {
    let fixture = AbcFixture::new();
    let feature = fixture
        .create_source_branch("feature/checked-out")
        .expect("create feature");
    fixture
        .push_source_branch(&feature)
        .expect("publish feature");
    let linked = fixture.root.join("checked-out-feature");
    fixture
        .invoker(&fixture.source)
        .run_check([
            OsStr::new("worktree"),
            OsStr::new("add"),
            linked.as_os_str(),
            OsStr::new(feature.as_str()),
        ])
        .expect("add linked worktree");
    let expected = fixture
        .commit_in_upstream("feature/checked-out", "advance checked-out feature")
        .expect("upstream feature commit");
    let source = fixture.source_repo().expect("source repo");

    source
        .fast_forward_branch_from_origin(&feature)
        .expect("fast-forward linked checkout");

    assert_eq!(
        fixture
            .rev_parse(&linked, "HEAD")
            .expect("linked worktree HEAD"),
        expected
    );
}

#[test]
fn delete_branch_if_oid_preserves_a_ref_on_mismatch_then_deletes_on_match() {
    let fixture = AbcFixture::new();
    let feature = fixture
        .create_source_branch("feature/delete-local")
        .expect("create feature");
    let oid = fixture
        .rev_parse(&fixture.source, "refs/heads/feature/delete-local")
        .expect("feature oid");
    let source = fixture.source_repo().expect("source repo");

    source
        .delete_branch_if_oid(&feature, &"1".repeat(40))
        .expect_err("stale expected oid must fail");
    assert_eq!(
        source.branch_oid(&feature).expect("feature after mismatch"),
        Some(oid.clone())
    );

    source
        .delete_branch_if_oid(&feature, &oid)
        .expect("delete with matching oid");
    assert_eq!(source.branch_oid(&feature).expect("deleted feature"), None);
}

#[test]
fn delete_remote_branch_if_oid_enforces_the_force_with_lease() {
    let fixture = AbcFixture::new();
    let feature = fixture
        .create_source_branch("feature/delete-remote")
        .expect("create feature");
    fixture
        .push_source_branch(&feature)
        .expect("publish feature");
    let oid = fixture
        .rev_parse(&fixture.upstream, "refs/heads/feature/delete-remote")
        .expect("remote feature oid");
    let source = fixture.source_repo().expect("source repo");

    source
        .delete_origin_branch_if_oid(&feature, &"0".repeat(40))
        .expect_err("stale lease must reject deletion");
    assert_eq!(
        source
            .origin_branch_oid(&feature)
            .expect("remote feature after stale lease"),
        Some(oid.clone())
    );

    source
        .delete_remote_branch_if_oid(&remote("origin"), &feature, &oid)
        .expect("delete with matching lease");
    assert_eq!(
        source
            .origin_branch_oid(&feature)
            .expect("remote feature after deletion"),
        None
    );
}

#[test]
fn upstream_config_requires_both_valid_remote_and_merge_values() {
    let fixture = AbcFixture::new();
    let git = fixture.invoker(&fixture.source);
    git.run_check(["config", "--local", "branch.main.remote", "origin"])
        .expect("set remote only");
    git.run_check(["config", "--local", "--unset-all", "branch.main.merge"])
        .ok();
    let source = fixture.source_repo().expect("source repo");
    assert_eq!(
        source
            .upstream_for(&branch("main"))
            .expect("incomplete config"),
        None
    );

    git.run_check(["config", "--local", "branch.main.merge", "refs/heads/main"])
        .expect("set merge ref");
    git.run_check(["config", "--local", "branch.main.remote", "origin/invalid"])
        .expect("set invalid remote text");

    assert!(matches!(
        source
            .upstream_for(&branch("main"))
            .expect_err("invalid configured remote must fail"),
        OutpostError::InvalidRefName { name } if name == "origin/invalid"
    ));
}

#[test]
fn source_at_missing_working_directory_preserves_the_io_error_path() {
    let temp = tempfile::tempdir().expect("tempdir");
    let missing = temp.path().join("missing-repo");

    let err = SourceRepo::at(&missing)
        .err()
        .expect("missing working directory must fail");

    assert!(matches!(err, OutpostError::IoAt { path, .. } if path == missing));
}

fn branch(value: &str) -> BranchName {
    BranchName::parse(value).expect("branch")
}

fn remote(value: &str) -> RemoteName {
    RemoteName::parse(value).expect("remote")
}
