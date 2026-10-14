#[allow(dead_code)]
mod common;

use std::fs;
use std::path::Path;

use common::fixture::AbcFixture;
use outpost_core::ops::branch_analysis::{
    BranchCleanupCandidate, BranchCleanupProof, analyze_branch_cleanup,
};
use outpost_core::{BranchName, Outpost, RemoteName};

// Git accepts these names through check-ref-format and ordinary branch commands.
// Existing ASCII-name tests cannot detect Unicode whitespace being trimmed from
// branch identities or mistaken for a separator in ls-remote records.
#[test]
fn source_queries_and_default_fetch_preserve_unicode_whitespace_in_refs() {
    let fixture = AbcFixture::new();
    let branches = publish_unicode_branches(&fixture);
    let git = fixture.invoker(&fixture.source);
    git.run_check(["switch", branches.candidate.as_str()])
        .expect("switch to Unicode branch");
    if git
        .run_status(["symbolic-ref", "--quiet", "refs/remotes/origin/HEAD"])
        .expect("probe local origin HEAD")
    {
        git.run_check(["symbolic-ref", "--delete", "refs/remotes/origin/HEAD"])
            .expect("remove local default alias so discovery queries the remote");
    }
    let tracking_ref = format!("refs/remotes/origin/{}", branches.default.as_str());
    git.run_check(["update-ref", "-d", &tracking_ref])
        .expect("remove local default tracking ref");
    let source = fixture.source_repo().expect("source repo");

    assert_eq!(
        source.current_branch().expect("current Unicode branch"),
        branches.candidate
    );
    assert_eq!(
        source
            .origin_branch_oid(&branches.candidate)
            .expect("advertised Unicode branch OID"),
        Some(branches.candidate_oid.clone())
    );
    assert_eq!(
        source
            .fetch_origin_default_branch()
            .expect("discover and fetch Unicode default")
            .expect("remote advertises a default"),
        (branches.default, branches.default_oid.clone())
    );
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, &tracking_ref)
            .expect("fetched exact default tracking ref"),
        branches.default_oid
    );
    assert_eq!(
        source.current_branch().expect("preserved current branch"),
        branches.candidate
    );
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "HEAD")
            .expect("preserved source commit"),
        branches.candidate_oid
    );
    assert_eq!(
        fs::read(fixture.source.join("tracked.txt")).expect("preserved source worktree"),
        b"candidate\n"
    );
}

// A real remote may advertise Unicode whitespace in both its default and
// candidate names. Cleanup must retain those identities in its safety proof;
// the ASCII fallback scenario does not protect this accepted input class.
#[test]
fn cleanup_fallback_preserves_unicode_ref_proof_and_repository_state() {
    let fixture = AbcFixture::new();
    let branches = publish_unicode_branches(&fixture);
    let outpost_path = fixture
        .add_outpost_on_branch("C", Some(branches.candidate.clone()))
        .expect("add Unicode-branch outpost");
    let source = fixture.source_repo().expect("source repo");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("outpost");
    fs::write(fixture.source.join("local-notes.txt"), "keep source work\n")
        .expect("uncommitted source work");
    let source_state = worktree_state(&fixture, &fixture.source);
    let outpost_state = worktree_state(&fixture, &outpost_path);
    let upstream_refs = refs(&fixture, &fixture.upstream);
    let registry = fs::read(source.registry_path()).expect("registry before analysis");
    let metadata = fs::read(outpost.metadata_path()).expect("metadata before analysis");

    let analysis = analyze_branch_cleanup(&source, &outpost, None);

    assert!(analysis.findings.is_empty(), "{:?}", analysis.findings);
    assert_eq!(
        analysis.candidate,
        Some(BranchCleanupCandidate {
            branch: branches.candidate,
            source_oid: branches.candidate_oid.clone(),
            upstream_remote: RemoteName::parse("origin").expect("origin"),
            upstream_oid: Some(branches.candidate_oid),
            proof: BranchCleanupProof::AncestorOfDefaultBranch {
                remote: RemoteName::parse("origin").expect("origin"),
                default_branch: branches.default,
                default_oid: branches.default_oid,
            },
        })
    );
    assert_eq!(worktree_state(&fixture, &fixture.source), source_state);
    assert_eq!(worktree_state(&fixture, &outpost_path), outpost_state);
    assert_eq!(refs(&fixture, &fixture.upstream), upstream_refs);
    assert_eq!(
        fs::read(source.registry_path()).expect("registry after analysis"),
        registry
    );
    assert_eq!(
        fs::read(outpost.metadata_path()).expect("metadata after analysis"),
        metadata
    );
    assert_eq!(
        fs::read(fixture.source.join("local-notes.txt")).expect("preserved source work"),
        b"keep source work\n"
    );
    assert_eq!(
        fs::read(outpost_path.join("tracked.txt")).expect("preserved outpost worktree"),
        b"candidate\n"
    );
}

struct UnicodeBranches {
    candidate: BranchName,
    candidate_oid: String,
    default: BranchName,
    default_oid: String,
}

fn publish_unicode_branches(fixture: &AbcFixture) -> UnicodeBranches {
    let git = fixture.invoker(&fixture.source);
    let candidate = fixture
        .create_source_branch("\u{a0}topic/a\u{a0}b\u{a0}")
        .expect("Git-valid Unicode candidate name");
    git.run_check(["switch", candidate.as_str()])
        .expect("switch to candidate");
    let candidate_oid = fixture
        .commit_file_in_source("candidate work", "tracked.txt", "candidate\n")
        .expect("candidate commit");
    let default = fixture
        .create_source_branch("\u{a0}main/default\u{a0}")
        .expect("Git-valid Unicode default name");
    git.run_check(["switch", default.as_str()])
        .expect("switch to default");
    let default_oid = fixture
        .commit_file_in_source("default includes candidate", "tracked.txt", "default\n")
        .expect("default descendant commit");
    git.run_check(["switch", "main"])
        .expect("leave candidate unchecked out in source");
    fixture
        .push_source_branch(&candidate)
        .expect("publish candidate");
    fixture
        .push_source_branch(&default)
        .expect("publish default");
    fixture
        .invoker(&fixture.upstream)
        .run_check([
            "symbolic-ref",
            "HEAD",
            &format!("refs/heads/{}", default.as_str()),
        ])
        .expect("advertise Unicode default through real Git HEAD");
    UnicodeBranches {
        candidate,
        candidate_oid,
        default,
        default_oid,
    }
}

#[derive(Debug, PartialEq, Eq)]
struct WorktreeState {
    refs: String,
    head: String,
    status: String,
}

fn worktree_state(fixture: &AbcFixture, path: &Path) -> WorktreeState {
    let git = fixture.invoker(path);
    WorktreeState {
        refs: refs(fixture, path),
        head: git
            .run_capture(["symbolic-ref", "HEAD"])
            .expect("attached HEAD"),
        status: git
            .run_capture(["status", "--porcelain=v1", "--untracked-files=all"])
            .expect("index and worktree state"),
    }
}

fn refs(fixture: &AbcFixture, path: &Path) -> String {
    fixture
        .invoker(path)
        .run_capture(["for-each-ref", "--format=%(refname) %(objectname)"])
        .expect("repository refs")
}
