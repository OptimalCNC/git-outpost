#[allow(dead_code)]
mod common;

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use common::fixture::{AbcFixture, CapturingReporter};
use outpost_core::ops::list;
use outpost_core::ops::push::{PushOptions, StepResult, run};
use outpost_core::ops::status::{
    ConfigProblem, OutpostHeadStatus, StatusReport, run_with as status_run_with,
};
use outpost_core::{AheadBehind, BranchName, Outpost, OutpostError, OutpostResult, StepKind};

#[test]
fn pu01_push_sends_outpost_branch_to_source_then_origin() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let outpost_oid = fixture
        .commit_file_in_outpost(
            &outpost_path,
            "outpost side",
            "outpost.txt",
            "from outpost\n",
        )
        .expect("outpost commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let report = run(&outpost, PushOptions, &mut reporter).expect("push");

    assert_pushed_commits(&report.outpost_to_source, 1);
    assert_pushed_commits(&report.source_to_origin, 1);
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/main")
            .expect("source main"),
        outpost_oid
    );
    assert_eq!(
        fixture
            .rev_parse(&fixture.upstream, "refs/heads/main")
            .expect("origin main"),
        outpost_oid
    );
}

#[test]
fn push_uses_the_exact_origin_branch_when_a_longer_name_has_diverged() {
    let fixture = AbcFixture::new();
    let topic = fixture.create_source_branch("topic").expect("topic branch");
    fixture.push_source_branch(&topic).expect("publish topic");
    let outpost_path = fixture
        .add_outpost_on_branch("C", Some(topic))
        .expect("topic outpost");
    let longer = BranchName::parse("refs/heads/topic").expect("longer branch");
    fixture
        .invoker(&fixture.source)
        .run_check(["switch", "-c", longer.as_str()])
        .expect("create the longer branch in the source");
    let published_longer_oid = fixture
        .commit_in_source("independent longer-named branch")
        .expect("longer branch commit");
    fixture
        .push_source_branch(&longer)
        .expect("publish longer branch with a complete refspec");
    let local_longer_oid = fixture
        .commit_in_source("unpublished work on the longer branch")
        .expect("local-only branch commit");
    let outpost_oid = fixture
        .commit_file_in_outpost(&outpost_path, "topic work", "topic.txt", "topic work\n")
        .expect("outpost commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let report = run(&outpost, PushOptions, &mut reporter)
        .expect("an unrelated branch must not prevent a fast-forward push");

    assert_pushed_commits(&report.outpost_to_source, 1);
    assert_pushed_commits(&report.source_to_origin, 1);
    for (repo, longer_oid) in [
        (&fixture.source, local_longer_oid),
        (&fixture.upstream, published_longer_oid),
    ] {
        assert_eq!(
            fixture.rev_parse(repo, "refs/heads/topic").unwrap(),
            outpost_oid
        );
        assert_eq!(
            fixture
                .rev_parse(repo, "refs/heads/refs/heads/topic")
                .unwrap(),
            longer_oid
        );
    }
}

#[test]
fn push_publishes_the_current_branch_when_a_tag_has_the_same_name() {
    let fixture = AbcFixture::new();
    let release = fixture
        .create_source_branch("release")
        .expect("release branch");
    let path = fixture
        .add_outpost_on_branch("C", Some(release.clone()))
        .expect("release outpost");
    let source_git = fixture.invoker(&fixture.source);
    source_git
        .run_check(["tag", "release", "refs/heads/release"])
        .expect("tag the current release");
    source_git
        .run_check(["push", "origin", "refs/heads/release", "refs/tags/release"])
        .expect("publish the branch and tag");
    fixture
        .invoker(&path)
        .run_check(["fetch", "local", "tag", "release"])
        .expect("fetch the release tag into the outpost");
    let tagged_tip = fixture
        .rev_parse(&fixture.source, "refs/tags/release")
        .expect("tagged tip");
    let new_tip = fixture
        .commit_in_outpost(&path, "prepare next release")
        .expect("new release commit");

    let StatusReport::Outpost(status) = status_run_with(&path, &fixture.git_env).expect("status")
    else {
        panic!("expected managed outpost status");
    };
    assert!(
        matches!(&status.head, OutpostHeadStatus::Attached { branch, .. } if branch == &release),
        "unexpected release branch in status: {:?}",
        status.head
    );
    let summaries = list::run(&fixture.source_repo().expect("source repo")).expect("list");
    assert_eq!(
        summaries[0].state,
        list::OutpostState::Present {
            head_oid: new_tip.clone(),
            head: list::OutpostHead::Attached(release),
        }
    );

    run(
        &outpost(&fixture, &path),
        PushOptions,
        &mut CapturingReporter::default(),
    )
    .expect("push release despite the tag of the same name");

    for repo in [&path, &fixture.source, &fixture.upstream] {
        assert_eq!(
            fixture.rev_parse(repo, "refs/heads/release").unwrap(),
            new_tip
        );
        assert_eq!(
            fixture.rev_parse(repo, "refs/tags/release").unwrap(),
            tagged_tip
        );
    }
}

#[test]
fn pu02_push_records_outpost_push_and_source_push_events() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_in_outpost(&outpost_path, "outpost side")
        .expect("outpost commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    run(&outpost, PushOptions, &mut reporter).expect("push");

    assert_eq!(
        reporter.step_kinds(),
        vec![StepKind::OutpostPush, StepKind::SourcePush]
    );
    assert!(
        reporter.warnings.is_empty(),
        "push should not warn on baseline path: {:?}",
        reporter.warnings
    );
}

#[test]
fn pu03_push_from_outpost_only_branch_returns_ambiguous_branch_creation() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .invoker(&outpost_path)
        .run_check(["switch", "-c", "feature/outpost-only"])
        .expect("create outpost-only branch");
    fixture
        .commit_in_outpost(&outpost_path, "outpost-only side")
        .expect("outpost commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        run(&outpost, PushOptions, &mut reporter),
        "push should reject outpost-only branch creation",
    );

    assert!(
        matches!(err, OutpostError::AmbiguousBranchCreation { branch } if branch == "feature/outpost-only")
    );
    assert!(reporter.steps.is_empty());
    assert!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/feature/outpost-only")
            .is_err(),
        "source branch should not be created"
    );
}

#[test]
fn pu04_push_when_source_diverged_from_outpost_returns_divergence() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_file_in_outpost(
            &outpost_path,
            "outpost side",
            "outpost.txt",
            "from outpost\n",
        )
        .expect("outpost commit");
    fixture
        .commit_file_in_source("source side", "source.txt", "from source\n")
        .expect("source commit");
    let source_before = fixture
        .rev_parse(&fixture.source, "refs/heads/main")
        .expect("source before");
    let origin_before = fixture
        .rev_parse(&fixture.upstream, "refs/heads/main")
        .expect("origin before");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        run(&outpost, PushOptions, &mut reporter),
        "push should reject outpost/source divergence",
    );

    assert!(matches!(err, OutpostError::Divergence { branch } if branch == "main"));
    assert!(reporter.steps.is_empty());
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/main")
            .expect("source after"),
        source_before
    );
    assert_eq!(
        fixture
            .rev_parse(&fixture.upstream, "refs/heads/main")
            .expect("origin after"),
        origin_before
    );
}

#[test]
fn push_when_outpost_is_behind_source_returns_divergence_before_push() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let source_oid = fixture
        .commit_file_in_source("source side", "source.txt", "from source\n")
        .expect("source commit");
    let origin_before = fixture
        .rev_parse(&fixture.upstream, "refs/heads/main")
        .expect("origin before");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        run(&outpost, PushOptions, &mut reporter),
        "push should reject outpost behind source before pushing",
    );

    assert!(matches!(err, OutpostError::Divergence { branch } if branch == "main"));
    assert!(reporter.steps.is_empty());
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/main")
            .expect("source after"),
        source_oid
    );
    assert_eq!(
        fixture
            .rev_parse(&fixture.upstream, "refs/heads/main")
            .expect("origin after"),
        origin_before
    );
}

#[test]
fn push_when_origin_is_ahead_returns_divergence_before_source_mutation() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_file_in_outpost(
            &outpost_path,
            "outpost side",
            "outpost.txt",
            "from outpost\n",
        )
        .expect("outpost commit");
    let source_before = fixture
        .rev_parse(&fixture.source, "refs/heads/main")
        .expect("source before");
    let origin_oid = fixture
        .commit_file_in_upstream("main", "origin side", "origin.txt", "from origin\n")
        .expect("origin commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        run(&outpost, PushOptions, &mut reporter),
        "push should reject origin divergence before mutating source",
    );

    assert!(matches!(err, OutpostError::Divergence { branch } if branch == "main"));
    assert!(reporter.steps.is_empty());
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/main")
            .expect("source after"),
        source_before
    );
    assert_eq!(
        fixture
            .rev_parse(&fixture.upstream, "refs/heads/main")
            .expect("origin after"),
        origin_oid
    );
}

#[test]
fn push_refuses_an_outpost_write_url_pointing_away_from_its_source() {
    let fixture = AbcFixture::new();
    let checkout = fixture.add_outpost("C").expect("add outpost");
    let old_source = fixture.root.join("old-source.git");
    fixture
        .invoker(&fixture.root)
        .run_check([
            OsStr::new("clone"),
            OsStr::new("--bare"),
            fixture.source.as_os_str(),
            old_source.as_os_str(),
        ])
        .expect("keep a writable copy of the source");
    fixture
        .invoker(&checkout)
        .run_check([
            OsStr::new("config"),
            OsStr::new("--add"),
            OsStr::new("remote.local.pushurl"),
            fixture.source.as_os_str(),
        ])
        .expect("configure the current source write path");
    fixture
        .invoker(&checkout)
        .run_check([
            OsStr::new("config"),
            OsStr::new("--add"),
            OsStr::new("remote.local.pushurl"),
            old_source.as_os_str(),
        ])
        .expect("leave an obsolete secondary write path");
    let original = fixture
        .rev_parse(&fixture.source, "refs/heads/main")
        .expect("source tip");
    let intended = fixture
        .commit_in_outpost(&checkout, "work for review")
        .expect("new work");
    let status = status_run_with(&checkout, &fixture.git_env).expect("outpost status");
    let StatusReport::Outpost(status) = status else {
        panic!("expected outpost status")
    };
    assert!(
        status
            .problems
            .contains(&ConfigProblem::LocalRemoteMismatch {
                configured: fs::canonicalize(&fixture.source).unwrap(),
                actual: fs::canonicalize(&old_source).unwrap(),
            })
    );

    let error = expect_error(
        run(
            &outpost(&fixture, &checkout),
            PushOptions,
            &mut CapturingReporter::default(),
        ),
        "refuse to publish outside the recorded source",
    );
    assert!(
        matches!(error, OutpostError::SourceRemoteMismatch { ref remote, ref expected, ref actual }
        if remote == "local" && *expected == fs::canonicalize(&fixture.source).unwrap() && *actual == fs::canonicalize(&old_source).unwrap())
    );
    for repo in [&fixture.source, &fixture.upstream, &old_source] {
        assert_eq!(
            fixture
                .rev_parse(repo, "refs/heads/main")
                .expect("preserved main"),
            original
        );
    }
    assert_eq!(
        fixture
            .rev_parse(&checkout, "HEAD")
            .expect("outpost keeps its work"),
        intended
    );
}

#[test]
fn push_keeps_the_source_and_old_clone_when_origin_has_a_stale_second_write_url() {
    let fixture = AbcFixture::new();
    let checkout = fixture.add_outpost("C").expect("add C");
    let old_origin = fixture.root.join("old-origin.git");
    fixture
        .invoker(&fixture.root)
        .run_check([
            OsStr::new("clone"),
            OsStr::new("--bare"),
            fixture.upstream.as_os_str(),
            old_origin.as_os_str(),
        ])
        .expect("clone the original upstream");
    let original = fixture
        .rev_parse(&fixture.source, "refs/heads/main")
        .expect("original branch tip");
    let intended = fixture
        .commit_in_outpost(&checkout, "work for review")
        .expect("outpost commit");
    fixture
        .invoker(&checkout)
        .run_check([
            OsStr::new("push"),
            fixture.upstream.as_os_str(),
            OsStr::new("HEAD:refs/heads/main"),
        ])
        .expect("publish the intended commit directly to the advertised origin");
    for destination in [Path::new("../A.git"), old_origin.as_path()] {
        fixture
            .invoker(&fixture.source)
            .run_check([
                OsStr::new("config"),
                OsStr::new("--add"),
                OsStr::new("remote.origin.pushurl"),
                destination.as_os_str(),
            ])
            .expect("set origin's write destinations");
    }

    let mut env = fixture.git_env.clone();
    env.insert("GIT_COMMON_DIR".into(), ".git".into());
    let outpost = Outpost::at_with(&checkout, &env).expect("open outpost");
    let error = expect_error(
        run(&outpost, PushOptions, &mut CapturingReporter::default()),
        "a stale write destination must be refused before either push",
    );

    assert!(
        matches!(error, OutpostError::UpstreamRepositoryMismatch { remote, fetch, push }
        if remote == "origin" && fetch == fs::canonicalize(&fixture.upstream).unwrap()
        && push == fs::canonicalize(&old_origin).unwrap())
    );
    for repo in [&fixture.source, &old_origin] {
        assert_eq!(
            fixture
                .rev_parse(repo, "refs/heads/main")
                .expect("preserved original branch"),
            original
        );
    }
    for repo in [&checkout, &fixture.upstream] {
        assert_eq!(
            fixture
                .rev_parse(repo, "refs/heads/main")
                .expect("preserved intended branch"),
            intended
        );
    }
}

#[test]
fn push_preserves_the_source_when_two_origin_addresses_are_pasted_into_one_url() {
    let fixture = AbcFixture::new();
    let checkout = fixture.add_outpost("C").expect("add C");
    let original = fixture
        .rev_parse(&fixture.source, "refs/heads/main")
        .expect("original source branch");
    let intended = fixture
        .commit_in_outpost(&checkout, "unpublished work")
        .expect("outpost commit");
    let pasted = format!(
        "{}\ngit@review.example:team/repo.git",
        dunce::simplified(&fixture.upstream).display()
    );
    fixture
        .invoker(&fixture.source)
        .run_check(["config", "remote.origin.pushurl", &pasted])
        .expect("store the pasted write address");

    let error = expect_error(
        run(
            &outpost(&fixture, &checkout),
            PushOptions,
            &mut CapturingReporter::default(),
        ),
        "refuse ambiguous write addresses before publishing",
    );

    assert!(
        matches!(error, OutpostError::IoAt { source, .. } if source.kind() == std::io::ErrorKind::InvalidData)
    );
    for repo in [&fixture.source, &fixture.upstream] {
        assert_eq!(
            fixture.rev_parse(repo, "refs/heads/main").expect("branch"),
            original
        );
    }
    assert_eq!(
        fixture
            .rev_parse(&checkout, "HEAD")
            .expect("outpost branch"),
        intended
    );
}

#[test]
fn push_preserves_unpublished_work_when_the_local_origin_write_repository_is_missing() {
    let fixture = AbcFixture::new();
    let origin_tip = fixture
        .rev_parse(&fixture.upstream, "refs/heads/main")
        .expect("published branch");
    let source_tip = fixture
        .commit_in_source("unpublished source commit")
        .expect("source commit");
    let checkout = fixture.add_outpost("C").expect("add C");
    let outpost_tip = fixture
        .commit_in_outpost(&checkout, "unpublished outpost commit")
        .expect("outpost commit");
    fixture
        .invoker(&fixture.source)
        .run_check(["remote", "set-url", "--push", "origin", "~/archive.git"])
        .expect("configure the old archive address");
    let home = dunce::simplified(&fixture.root);
    let mut env = fixture.git_env.clone();
    env.insert("HOME".into(), home.as_os_str().to_owned());
    let outpost = Outpost::at_with(&checkout, &env).expect("open outpost");

    let error = expect_error(
        run(&outpost, PushOptions, &mut CapturingReporter::default()),
        "refuse the missing archive before publishing any commits",
    );

    let OutpostError::IoAt { path, source } = error else {
        panic!("expected a local repository diagnostic, got {error:?}");
    };
    assert_eq!(path, home.join("archive.git"));
    assert_eq!(source.kind(), std::io::ErrorKind::InvalidData);
    for (repo, expected) in [
        (&checkout, outpost_tip),
        (&fixture.source, source_tip),
        (&fixture.upstream, origin_tip),
    ] {
        assert_eq!(
            fixture.rev_parse(repo, "refs/heads/main").expect("branch"),
            expected
        );
    }
}

#[test]
fn push_accepts_a_relative_origin_and_a_file_url_to_its_linked_worktree() {
    let fixture = AbcFixture::new();
    fs::create_dir(fixture.root.join("outposts")).expect("separate outpost directory");
    let checkout = fixture
        .add_outpost("outposts/C")
        .expect("add nested outpost");
    let linked = fixture.root.join("origin checkout");
    fixture
        .invoker(&fixture.upstream)
        .run_check([
            OsStr::new("worktree"),
            OsStr::new("add"),
            OsStr::new("--detach"),
            linked.as_os_str(),
        ])
        .expect("create a detached worktree of origin");
    let source = fixture.invoker(&fixture.source);
    source
        .run_check(["remote", "set-url", "origin", "../A"])
        .expect("relative origin without the .git suffix");
    let write_path = dunce::simplified(&linked)
        .to_str()
        .expect("fixture path")
        .replace('\\', "/")
        .replace('%', "%25")
        .replace(' ', "%20");
    source
        .run_check([
            "remote",
            "set-url",
            "--push",
            "origin",
            &format!("file://{write_path}"),
        ])
        .expect("write through the linked checkout");
    let intended = fixture
        .commit_in_outpost(&checkout, "publish via the alternate route")
        .expect("outpost commit");

    let report = run(
        &outpost(&fixture, &checkout),
        PushOptions,
        &mut CapturingReporter::default(),
    )
    .expect("same origin repository");

    assert_pushed_commits(&report.outpost_to_source, 1);
    assert_pushed_commits(&report.source_to_origin, 1);
    for repo in [&fixture.source, &fixture.upstream, &linked] {
        assert_eq!(
            fixture
                .rev_parse(repo, "refs/heads/main")
                .expect("published branch"),
            intended
        );
    }
}

#[test]
fn pu05_push_dirty_outpost_succeeds() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let outpost_oid = fixture
        .commit_file_in_outpost(
            &outpost_path,
            "committed outpost side",
            "committed.txt",
            "committed\n",
        )
        .expect("outpost commit");
    fs::write(outpost_path.join("dirty.txt"), "dirty\n").expect("dirty outpost");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let report = run(&outpost, PushOptions, &mut reporter).expect("push");

    assert_pushed_commits(&report.outpost_to_source, 1);
    assert_pushed_commits(&report.source_to_origin, 1);
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/main")
            .expect("source main"),
        outpost_oid
    );
    assert_eq!(
        fixture
            .rev_parse(&fixture.upstream, "refs/heads/main")
            .expect("origin main"),
        outpost_oid
    );
}

#[test]
fn push_reports_when_origin_accepts_the_branch_but_hides_it_from_verification() {
    let fixture = AbcFixture::new();
    let topic = fixture.create_source_branch("topic").expect("topic branch");
    let path = fixture
        .add_outpost_on_branch("C", Some(topic))
        .expect("topic outpost");
    let expected = fixture
        .commit_in_outpost(&path, "publish topic")
        .expect("new topic commit");
    fixture
        .invoker(&fixture.upstream)
        .run_check([
            "config",
            "--local",
            "uploadpack.hideRefs",
            "refs/heads/topic",
        ])
        .expect("hide topic from reads on origin");

    let error = expect_error(
        run(
            &outpost(&fixture, &path),
            PushOptions,
            &mut CapturingReporter::default(),
        ),
        "publication cannot be verified",
    );

    assert_eq!(error.exit_code(), 5);
    assert!(matches!(error, OutpostError::PushedBranchNotVisible { branch } if branch == "topic"));
    for repo in [&fixture.source, &fixture.upstream] {
        assert_eq!(
            fixture
                .rev_parse(repo, "refs/heads/topic")
                .expect("published topic"),
            expected,
            "topic in {}",
            repo.display()
        );
    }
}

#[test]
fn push_first_publication_to_absent_origin_branch_counts_only_new_commits() {
    let fixture = AbcFixture::new();
    let feature = fixture
        .create_source_branch("feature/first-publication")
        .expect("create source branch");
    let outpost_path = fixture
        .add_outpost_on_branch("C", Some(feature.clone()))
        .expect("add feature outpost");
    let outpost_oid = fixture
        .commit_file_in_outpost(&outpost_path, "feature side", "feature.txt", "feature\n")
        .expect("outpost commit");
    assert!(
        fixture
            .rev_parse(&fixture.upstream, "refs/heads/feature/first-publication")
            .is_err(),
        "origin feature branch should start absent"
    );
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let report = run(&outpost, PushOptions, &mut reporter).expect("push");

    assert_pushed_commits(&report.outpost_to_source, 1);
    assert_pushed_commits(&report.source_to_origin, 1);
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/feature/first-publication")
            .expect("source feature"),
        outpost_oid
    );
    assert_eq!(
        fixture
            .rev_parse(&fixture.upstream, "refs/heads/feature/first-publication")
            .expect("origin feature"),
        outpost_oid
    );
    let upstream = fixture
        .source_repo()
        .expect("source repo")
        .upstream_for(&feature)
        .expect("source upstream")
        .expect("feature branch upstream");
    assert_eq!(upstream.remote.as_str(), "origin");
    assert_eq!(
        upstream.merge_ref.as_str(),
        "refs/heads/feature/first-publication"
    );

    let status = status_run_with(&outpost_path, &fixture.git_env).expect("status after push");
    let StatusReport::Outpost(status) = status else {
        panic!("expected outpost status after push");
    };
    assert!(
        !status
            .problems
            .contains(&ConfigProblem::SourceUpstreamTrackingUnset {
                branch: feature.clone(),
            }),
        "push should leave source branch tracking origin"
    );
    assert_eq!(
        status.source_ahead_behind_upstream,
        Some(AheadBehind {
            ahead: 0,
            behind: 0,
        })
    );
}

#[test]
fn pu06_push_with_missing_source_returns_source_missing() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_in_outpost(&outpost_path, "outpost side")
        .expect("outpost commit");
    let expected_source = fs::canonicalize(&fixture.source).expect("canonical source");
    let moved_source = fixture.root.join("B.moved");
    fs::rename(&fixture.source, &moved_source).expect("move source repo");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        run(&outpost, PushOptions, &mut reporter),
        "push should reject missing source",
    );

    assert!(matches!(err, OutpostError::SourceMissing(path) if path == expected_source));
    assert!(reporter.steps.is_empty());
}

#[test]
fn pu07_push_uses_custom_remote_for_outpost_to_source_and_origin_for_source_to_upstream() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture
        .add_outpost_with_remote("C", "custom")
        .expect("add custom outpost");
    assert!(
        fixture
            .invoker(&outpost_path)
            .run_capture(["remote", "get-url", "local"])
            .is_err(),
        "custom outpost should not rely on a local remote"
    );
    let outpost_oid = fixture
        .commit_file_in_outpost(
            &outpost_path,
            "custom outpost side",
            "custom.txt",
            "custom\n",
        )
        .expect("outpost commit");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let report = run(&outpost, PushOptions, &mut reporter).expect("push");

    assert_pushed_commits(&report.outpost_to_source, 1);
    assert_pushed_commits(&report.source_to_origin, 1);
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/main")
            .expect("source main"),
        outpost_oid
    );
    assert_eq!(
        fixture
            .rev_parse(&fixture.upstream, "refs/heads/main")
            .expect("origin main"),
        outpost_oid
    );
}

#[test]
fn pu08_push_into_dirty_checked_out_source_branch_surfaces_update_instead_git_failed() {
    let fixture = AbcFixture::new();
    fixture
        .commit_file_in_source("baseline tracked file", "tracked.txt", "base\n")
        .expect("source baseline commit");
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_file_in_outpost(
            &outpost_path,
            "outpost side",
            "outpost.txt",
            "from outpost\n",
        )
        .expect("outpost commit");
    fs::write(fixture.source.join("tracked.txt"), "dirty source\n").expect("dirty source");
    let origin_before = fixture
        .rev_parse(&fixture.upstream, "refs/heads/main")
        .expect("origin before");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        run(&outpost, PushOptions, &mut reporter),
        "push should surface Git's updateInstead failure",
    );

    match err {
        OutpostError::GitFailed { stderr, .. } => {
            assert!(
                !stderr.is_empty(),
                "GitFailed should preserve stderr from updateInstead failure"
            );
        }
        other => panic!("expected GitFailed, got {other:?}"),
    }
    assert_eq!(reporter.step_kinds(), vec![StepKind::OutpostPush]);
    assert_eq!(
        fixture
            .rev_parse(&fixture.upstream, "refs/heads/main")
            .expect("origin after"),
        origin_before
    );
}

#[test]
fn pu09_push_with_deny_current_branch_refuse_returns_push_into_checked_out_branch() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_in_outpost(&outpost_path, "outpost side")
        .expect("outpost commit");
    fixture
        .invoker(&fixture.source)
        .run_check(["config", "--local", "receive.denyCurrentBranch", "refuse"])
        .expect("set denyCurrentBranch");
    let source_before = fixture
        .rev_parse(&fixture.source, "refs/heads/main")
        .expect("source before");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        run(&outpost, PushOptions, &mut reporter),
        "push should reject checked-out source branch when denyCurrentBranch refuses",
    );

    assert!(
        matches!(err, OutpostError::PushIntoCheckedOutBranch { source, branch } if source == canonical_source(&fixture) && branch == "main")
    );
    assert!(reporter.steps.is_empty());
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/main")
            .expect("source after"),
        source_before
    );
}

#[test]
fn pu10_push_on_detached_head_returns_no_upstream_tracking_head_before_push() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("add C");
    let source_before = fixture
        .rev_parse(&fixture.source, "refs/heads/main")
        .expect("source before");
    let origin_before = fixture
        .rev_parse(&fixture.upstream, "refs/heads/main")
        .expect("origin before");
    fixture
        .invoker(&outpost_path)
        .run_check(["checkout", "--detach"])
        .expect("detach outpost HEAD");
    let outpost = outpost(&fixture, &outpost_path);
    let mut reporter = CapturingReporter::default();

    let err = expect_error(
        run(&outpost, PushOptions, &mut reporter),
        "push should reject detached HEAD",
    );

    assert!(matches!(err, OutpostError::NoUpstreamTracking { branch } if branch == "HEAD"));
    assert!(reporter.steps.is_empty());
    assert_eq!(
        fixture
            .rev_parse(&fixture.source, "refs/heads/main")
            .expect("source after"),
        source_before
    );
    assert_eq!(
        fixture
            .rev_parse(&fixture.upstream, "refs/heads/main")
            .expect("origin after"),
        origin_before
    );
}

fn outpost(fixture: &AbcFixture, path: &Path) -> Outpost {
    Outpost::at_with(path, &fixture.git_env).expect("open outpost")
}

fn assert_pushed_commits(result: &StepResult, expected: u32) {
    match result {
        StepResult::Pushed { commits } => assert_eq!(*commits, expected),
    }
}

fn canonical_source(fixture: &AbcFixture) -> std::path::PathBuf {
    fs::canonicalize(&fixture.source).expect("canonical source")
}

fn expect_error<T>(result: OutpostResult<T>, message: &str) -> OutpostError {
    match result {
        Ok(_) => panic!("{message}"),
        Err(err) => err,
    }
}
