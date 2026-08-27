#[allow(dead_code)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use outpost_core::{
    AheadBehind, MetadataState, Outpost, OutpostError, OutpostStateStore, SourceRepo,
};

#[test]
fn discovery_variants_canonicalize_nested_work_tree_and_git_directory() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add outpost");
    let nested = outpost_path.join("nested");
    fs::create_dir(&nested).expect("nested directory");
    let expected_work_tree = canonical(&outpost_path);
    let expected_git_dir = canonical(&outpost_path.join(".git"));

    let discovered = Outpost::discover(&nested).expect("discover outpost");
    let discovered_with =
        Outpost::discover_with(&nested, &fixture.git_env).expect("discover outpost with env");
    let at_work_tree = Outpost::at(&outpost_path).expect("open outpost work tree");
    let at_nested = Outpost::at_with(&nested, &fixture.git_env).expect("open nested outpost");

    for outpost in [&discovered, &discovered_with, &at_work_tree, &at_nested] {
        assert_eq!(outpost.work_tree(), expected_work_tree);
        assert_eq!(outpost.git_dir(), expected_git_dir);
        assert_eq!(outpost.location().work_tree(), expected_work_tree);
        assert_eq!(outpost.location().git_dir(), expected_git_dir);
        assert_eq!(
            outpost.metadata_path(),
            expected_git_dir.join("outpost/metadata.json")
        );
    }
}

#[test]
fn discovery_variants_report_non_repositories_with_the_requested_path() {
    let temp = tempfile::tempdir().expect("tempdir");
    let outside = temp.path().join("outside");
    fs::create_dir(&outside).expect("outside directory");

    let discover_error = into_error(
        Outpost::discover(&outside),
        "discovery should reject non-repo",
    );
    let discover_with_error = into_error(
        Outpost::discover_with(&outside, &std::collections::BTreeMap::new()),
        "discovery with env should reject non-repo",
    );
    let at_error = into_error(Outpost::at(&outside), "open should reject non-repo");

    assert!(matches!(discover_error, OutpostError::NotARepo(path) if path == outside));
    assert!(matches!(discover_with_error, OutpostError::NotARepo(path) if path == outside));
    assert!(matches!(at_error, OutpostError::NotARepo(path) if path == outside));
}

#[test]
fn at_rejects_a_managed_repository_with_invalid_metadata() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add outpost");
    let metadata_path = outpost_path.join(".git/outpost/metadata.json");
    fs::write(&metadata_path, "not json\n").expect("corrupt metadata");

    let error = into_error(
        Outpost::at_with(&outpost_path, &fixture.git_env),
        "invalid metadata should reject the outpost",
    );

    assert!(
        matches!(error, OutpostError::BadMetadata { outpost, .. } if outpost == canonical(&outpost_path))
    );
}

#[test]
fn state_and_source_accessors_expose_current_metadata_and_source() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add outpost");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("open outpost");

    assert_eq!(outpost.metadata().source_repo, canonical(&fixture.source));
    assert_eq!(outpost.metadata().remote_name.as_str(), "local");
    assert_eq!(
        outpost.source_repo().expect("open source").work_tree(),
        canonical(&fixture.source)
    );
    assert!(matches!(
        outpost.state_store().read_metadata().expect("read state"),
        MetadataState::Valid(metadata)
            if metadata == *outpost.metadata()
    ));
}

#[test]
fn source_accessor_reports_missing_recorded_source() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add outpost");
    let expected_source = canonical(&fixture.source);
    fs::rename(&fixture.source, fixture.root.join("B.moved")).expect("move source");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("open outpost");

    let error = into_error(outpost.source_repo(), "missing source should fail");

    assert!(matches!(error, OutpostError::SourceMissing(path) if path == expected_source));
}

#[test]
fn branch_and_dirty_accessors_distinguish_clean_dirty_and_detached_states() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add outpost");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("open outpost");

    assert_eq!(
        outpost.current_branch().expect("attached branch").as_str(),
        "main"
    );
    assert!(!outpost.is_dirty().expect("clean outpost"));
    fs::write(outpost_path.join("untracked.txt"), "dirty\n").expect("write untracked file");
    assert!(outpost.is_dirty().expect("dirty outpost"));

    fixture
        .invoker(&outpost_path)
        .run_check(["clean", "-fd"])
        .expect("remove untracked file");
    fixture
        .invoker(&outpost_path)
        .run_check(["checkout", "--detach"])
        .expect("detach HEAD");
    let error = into_error(outpost.current_branch(), "detached HEAD has no branch");
    assert!(
        matches!(error, OutpostError::BranchNotFound { branch, repo }
            if branch == "HEAD" && repo == canonical(&outpost_path))
    );
}

#[test]
fn upstream_tracking_returns_none_when_remote_is_missing() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add outpost");
    let git = fixture.invoker(&outpost_path);
    git.run_check(["config", "--unset-all", "branch.main.remote"])
        .expect("unset remote");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("open outpost");

    assert_eq!(outpost.upstream_tracking().expect("read tracking"), None);
}

#[test]
fn ahead_behind_and_unpushed_reject_missing_wrong_and_non_branch_tracking() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add outpost");
    let source = SourceRepo::at_with(&fixture.source, &fixture.git_env).expect("open source");
    let git = fixture.invoker(&outpost_path);

    git.run_check(["config", "--unset-all", "branch.main.remote"])
        .expect("unset remote");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("open outpost");
    assert_no_tracking(outpost.ahead_behind_source(), "main");
    assert_no_tracking(outpost.unpushed_commits(&source), "main");

    git.run_check([
        "remote",
        "add",
        "wrong",
        fixture.source.to_str().expect("source path"),
    ])
    .expect("add wrong remote");
    git.run_check(["config", "branch.main.remote", "wrong"])
        .expect("configure wrong remote");
    git.run_check(["config", "branch.main.merge", "refs/heads/main"])
        .expect("configure branch merge ref");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("open outpost");
    assert_no_tracking(outpost.ahead_behind_source(), "main");
    assert_no_tracking(outpost.unpushed_commits(&source), "main");

    git.run_check(["config", "branch.main.remote", "local"])
        .expect("configure source remote");
    git.run_check(["config", "branch.main.merge", "refs/tags/v1"])
        .expect("configure non-branch merge ref");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("open outpost");
    assert_non_branch(outpost.ahead_behind_source());
    assert_non_branch(outpost.unpushed_commits(&source));
}

#[test]
fn ahead_behind_fetches_source_and_reports_behind_then_diverged_history() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add outpost");
    let source_oid = fixture
        .commit_in_source("source advances")
        .expect("source commit");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("open outpost");

    assert_eq!(
        outpost.ahead_behind_source().expect("compare after fetch"),
        AheadBehind {
            ahead: 0,
            behind: 1
        }
    );
    assert_eq!(
        fixture
            .rev_parse(&outpost_path, "refs/remotes/local/main")
            .expect("fetched source ref"),
        source_oid
    );

    fixture
        .commit_in_outpost(&outpost_path, "outpost diverges")
        .expect("outpost commit");
    assert_eq!(
        outpost.ahead_behind_source().expect("compare divergence"),
        AheadBehind {
            ahead: 1,
            behind: 1
        }
    );
}

#[test]
fn ahead_behind_reports_synced_and_ahead_only_states() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add outpost");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("open outpost");

    assert_eq!(
        outpost
            .ahead_behind_source()
            .expect("compare synced history"),
        AheadBehind {
            ahead: 0,
            behind: 0
        }
    );
    fixture
        .commit_in_outpost(&outpost_path, "outpost only")
        .expect("outpost commit");
    assert_eq!(
        outpost
            .ahead_behind_source()
            .expect("compare ahead history"),
        AheadBehind {
            ahead: 1,
            behind: 0
        }
    );
}

#[test]
fn unpushed_counts_matching_local_commits_and_rejects_missing_source_branch() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add outpost");
    let source = SourceRepo::at_with(&fixture.source, &fixture.git_env).expect("open source");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("open outpost");

    assert_eq!(
        outpost.unpushed_commits(&source).expect("baseline count"),
        0
    );
    fixture
        .commit_in_outpost(&outpost_path, "unpushed")
        .expect("outpost commit");
    assert_eq!(
        outpost.unpushed_commits(&source).expect("unpushed count"),
        1
    );

    fixture
        .invoker(&fixture.source)
        .run_check(["checkout", "--orphan", "empty"])
        .expect("checkout orphan branch");
    fixture
        .invoker(&fixture.source)
        .run_check(["commit", "--allow-empty", "-m", "empty"])
        .expect("commit orphan branch");
    fixture
        .invoker(&fixture.source)
        .run_check(["branch", "-D", "main"])
        .expect("delete source main");
    let source = SourceRepo::at_with(&fixture.source, &fixture.git_env).expect("reopen source");

    let error = into_error(
        outpost.unpushed_commits(&source),
        "missing source branch should fail",
    );
    assert!(
        matches!(error, OutpostError::BranchNotFound { branch, repo }
            if branch == "main" && repo == canonical(&fixture.source))
    );
}

fn assert_no_tracking<T>(result: Result<T, OutpostError>, branch: &str) {
    assert!(
        matches!(result, Err(OutpostError::NoUpstreamTracking { branch: actual }) if actual == branch)
    );
}

fn assert_non_branch<T>(result: Result<T, OutpostError>) {
    assert!(
        matches!(result, Err(OutpostError::UpstreamNotABranch { merge_ref }) if merge_ref == "refs/tags/v1")
    );
}

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("canonical path")
}

fn into_error<T>(result: Result<T, OutpostError>, message: &str) -> OutpostError {
    match result {
        Ok(_) => panic!("{message}"),
        Err(error) => error,
    }
}
