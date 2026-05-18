#[allow(dead_code)]
mod common;

use std::fs;
use std::path::Path;

use common::fixture::{AbcFixture, CapturingReporter};
use outpost_core::ops::merge::{run, MergeOptions};
use outpost_core::{Outpost, OutpostError, OutpostResult, SourceRemoteRef, StepKind};

#[test]
fn mr01_merge_fetches_source_branch_and_merges_remote_tracking_ref() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .invoker(&outpost_path)
        .run_check(["switch", "-c", "feature/merge-main"])
        .expect("create outpost feature branch");
    let outpost_oid = fixture
        .commit_file_in_outpost(
            &outpost_path,
            "outpost side",
            "outpost.txt",
            "from outpost\n",
        )
        .expect("outpost commit");
    let source_oid = fixture
        .commit_file_in_source("source side", "source.txt", "from source\n")
        .expect("source commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let report = run(
        &outpost,
        MergeOptions {
            source_ref: source_ref("local/main"),
        },
        &mut reporter,
    )
    .expect("merge");

    assert_eq!(report.source_ref, source_ref("local/main"));
    assert_eq!(
        fixture
            .rev_parse(&outpost_path, "refs/remotes/local/main")
            .expect("local/main"),
        source_oid
    );
    assert_ancestor(&fixture, &outpost_path, &source_oid, "HEAD");
    assert_ancestor(&fixture, &outpost_path, &outpost_oid, "HEAD");
    assert_eq!(
        fs::read_to_string(outpost_path.join("source.txt")).expect("source file"),
        "from source\n"
    );
    assert_eq!(
        fs::read_to_string(outpost_path.join("outpost.txt")).expect("outpost file"),
        "from outpost\n"
    );
}

#[test]
fn mr03_merge_uses_custom_source_remote_name() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture
        .add_outpost_with_remote("C", "custom")
        .expect("add custom outpost");
    fixture
        .invoker(&outpost_path)
        .run_check(["switch", "-c", "feature/custom-merge"])
        .expect("create custom merge branch");
    fixture
        .commit_file_in_outpost(
            &outpost_path,
            "outpost side",
            "custom-outpost.txt",
            "from outpost\n",
        )
        .expect("outpost commit");
    let source_oid = fixture
        .commit_file_in_source("source side", "custom-source.txt", "from source\n")
        .expect("source commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    run(
        &outpost,
        MergeOptions {
            source_ref: source_ref("custom/main"),
        },
        &mut reporter,
    )
    .expect("custom merge");

    assert_eq!(
        fixture
            .rev_parse(&outpost_path, "refs/remotes/custom/main")
            .expect("custom/main"),
        source_oid
    );
    assert_ancestor(&fixture, &outpost_path, &source_oid, "HEAD");
    assert!(
        fixture
            .invoker(&outpost_path)
            .run_capture(["remote", "get-url", "local"])
            .is_err(),
        "custom outpost should not rely on a local remote"
    );
}

#[test]
fn mr04_merge_rejects_wrong_remote_before_fetching() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .invoker(&outpost_path)
        .run_check([
            "remote",
            "add",
            "origin",
            fixture.source.to_str().expect("source path"),
        ])
        .expect("add decoy origin remote");
    let local_main_before = fixture
        .rev_parse(&outpost_path, "refs/remotes/local/main")
        .expect("local/main before");
    fixture
        .commit_file_in_source("source side", "wrong-remote.txt", "from source\n")
        .expect("source commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        run(
            &outpost,
            MergeOptions {
                source_ref: source_ref("origin/main"),
            },
            &mut reporter,
        ),
        "merge should reject wrong remote",
    );

    assert!(matches!(err, OutpostError::InvalidRefName { name } if name == "origin/main"));
    assert!(reporter.steps.is_empty());
    assert_eq!(
        fixture
            .rev_parse(&outpost_path, "refs/remotes/local/main")
            .expect("local/main after"),
        local_main_before
    );
    assert!(
        fixture
            .rev_parse(&outpost_path, "refs/remotes/origin/main")
            .is_err(),
        "wrong remote should not be fetched"
    );
}

#[test]
fn mr05_merge_records_outpost_fetch_event() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_file_in_source("source side", "event.txt", "from source\n")
        .expect("source commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    run(
        &outpost,
        MergeOptions {
            source_ref: source_ref("local/main"),
        },
        &mut reporter,
    )
    .expect("merge");

    assert_eq!(reporter.step_kinds(), vec![StepKind::OutpostFetch]);
    assert!(
        reporter.warnings.is_empty(),
        "merge should not warn on baseline path: {:?}",
        reporter.warnings
    );
}

#[test]
fn merge_uses_full_remote_tracking_ref_when_local_branch_name_collides() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .invoker(&outpost_path)
        .run_check(["branch", "local/main"])
        .expect("create colliding local branch");
    fixture
        .invoker(&outpost_path)
        .run_check(["switch", "-c", "feature/colliding-merge"])
        .expect("create outpost feature branch");
    fixture
        .commit_file_in_outpost(
            &outpost_path,
            "outpost side",
            "colliding-outpost.txt",
            "from outpost\n",
        )
        .expect("outpost commit");
    let source_oid = fixture
        .commit_file_in_source("source side", "colliding-source.txt", "from source\n")
        .expect("source commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    run(
        &outpost,
        MergeOptions {
            source_ref: source_ref("local/main"),
        },
        &mut reporter,
    )
    .expect("merge");

    assert_eq!(
        fixture
            .rev_parse(&outpost_path, "refs/remotes/local/main")
            .expect("local/main"),
        source_oid
    );
    assert_ancestor(&fixture, &outpost_path, &source_oid, "HEAD");
}

#[test]
fn mr06_merge_on_detached_head_returns_attached_branch_error_before_fetching() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let local_main_before = fixture
        .rev_parse(&outpost_path, "refs/remotes/local/main")
        .expect("local/main before");
    fixture
        .commit_file_in_source("source side", "detached.txt", "from source\n")
        .expect("source commit");
    fixture
        .invoker(&outpost_path)
        .run_check(["checkout", "--detach"])
        .expect("detach outpost HEAD");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        run(
            &outpost,
            MergeOptions {
                source_ref: source_ref("local/main"),
            },
            &mut reporter,
        ),
        "merge should reject detached HEAD",
    );

    assert!(matches!(err, OutpostError::NoUpstreamTracking { branch } if branch == "HEAD"));
    assert!(reporter.steps.is_empty());
    assert_eq!(
        fixture
            .rev_parse(&outpost_path, "refs/remotes/local/main")
            .expect("local/main after"),
        local_main_before
    );
}

fn outpost(fixture: &AbcFixture, path: &Path) -> Outpost {
    Outpost::at_with(path, &fixture.git_env).expect("open outpost")
}

fn source_ref(value: &str) -> SourceRemoteRef {
    SourceRemoteRef::parse(value).expect("source remote ref")
}

fn assert_ancestor(fixture: &AbcFixture, repo: &Path, ancestor: &str, descendant: &str) {
    fixture
        .invoker(repo)
        .run_check(["merge-base", "--is-ancestor", ancestor, descendant])
        .expect("expected ancestor relationship");
}

fn expect_error<T>(result: OutpostResult<T>, message: &str) -> OutpostError {
    match result {
        Ok(_) => panic!("{message}"),
        Err(err) => err,
    }
}
