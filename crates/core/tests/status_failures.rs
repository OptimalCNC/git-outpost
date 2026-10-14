#[allow(dead_code)]
mod common;

use std::fs;

use common::fixture::AbcFixture;
use outpost_core::ops::analyze::{self, AnalyzeOptions, BranchDeleteSafety, Probe};
use outpost_core::ops::branch_analysis::{BranchCleanupFinding, BranchCleanupSkipReason};
use outpost_core::ops::status;
use outpost_core::selector::OutpostSelector;
use outpost_core::{AheadBehind, BranchName, Outpost, OutpostError};

// Git configuration accepts a whitespace-only URL. Status must reject that
// invalid route inventory without changing configuration or work; the existing
// absent-URL test protects Git's distinct remote-name fallback.
#[test]
fn status_rejects_a_whitespace_only_remote_url_without_mutating_the_source() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repository");
    fixture
        .invoker(&fixture.source)
        .run_check(["config", "--local", "remote.origin.url", "   "])
        .expect("configure a whitespace-only URL");
    assert_eq!(
        fixture
            .invoker(&fixture.source)
            .run_capture(["remote", "get-url", "--all", "origin"])
            .expect("Git reports the configured URL, which is blank after trimming"),
        ""
    );
    fs::write(
        fixture.source.join("draft.txt"),
        "uncommitted source work\n",
    )
    .expect("source work");
    let head = fixture
        .rev_parse(&fixture.source, "HEAD")
        .expect("source HEAD");
    let config_path = source.git_dir().join("config");
    let config = fs::read(&config_path).expect("source config");
    let registry_present = source.registry_path().exists();

    let error = status::run_with(&fixture.source, &fixture.git_env)
        .expect_err("a successful Git query with no URLs is invalid route evidence");

    assert!(matches!(
        error,
        OutpostError::IoAt { path, source: error }
            if path == source.work_tree() && error.kind() == std::io::ErrorKind::InvalidData
    ));
    assert_eq!(fixture.rev_parse(&fixture.source, "HEAD").unwrap(), head);
    assert_eq!(fs::read(config_path).unwrap(), config);
    assert_eq!(source.registry_path().exists(), registry_present);
    assert_eq!(
        fs::read_to_string(fixture.source.join("draft.txt")).unwrap(),
        "uncommitted source work\n"
    );
}

// External relocation followed by an ordinary file occupying the source parent
// makes the persisted reverse link inaccessible. Outpost-context status must
// propagate that error rather than report SourceMissing; source-context stale
// registration tests exercise the opposite link direction.
#[cfg(unix)]
#[test]
fn outpost_status_preserves_state_when_the_source_parent_is_now_a_file() {
    let mut fixture = AbcFixture::new();
    let parent = fixture.root.join("source-parent");
    fs::create_dir(&parent).expect("source parent");
    let source_path = parent.join("B");
    fs::rename(&fixture.source, &source_path).expect("place source inside its parent");
    fixture.source = source_path;
    let path = fixture
        .add_outpost("C")
        .expect("outpost with real reverse link");
    let source = fixture.source_repo().expect("source repository");
    let configured_source = source.work_tree().to_path_buf();
    let outpost = Outpost::at_with(&path, &fixture.git_env).expect("open outpost");
    let registry = fs::read(source.registry_path()).expect("registry");
    let registry_relative = source
        .registry_path()
        .strip_prefix(fs::canonicalize(&parent).expect("canonical source parent"))
        .expect("registry inside source parent")
        .to_path_buf();
    let metadata = fs::read(outpost.metadata_path()).expect("metadata");
    let source_head = fixture
        .rev_parse(&fixture.source, "HEAD")
        .expect("source HEAD");
    let outpost_head = fixture.rev_parse(&path, "HEAD").expect("outpost HEAD");
    fs::write(path.join("draft.txt"), "uncommitted outpost work\n").expect("outpost work");
    let relocated = fixture.root.join("relocated-source-parent");
    fs::rename(&parent, &relocated).expect("external source-parent relocation");
    fs::write(&parent, "user file occupies the old source parent\n")
        .expect("ordinary replacement file");

    let error = status::run_with(&path, &fixture.git_env)
        .expect_err("a source parent that is a file is an inspection error");

    assert!(matches!(
        error,
        OutpostError::IoAt { path: failed_path, source: error }
            if failed_path == configured_source
                && error.kind() == std::io::ErrorKind::NotADirectory
    ));
    assert_eq!(
        fs::read(relocated.join(registry_relative)).unwrap(),
        registry
    );
    assert_eq!(
        fixture.rev_parse(&relocated.join("B"), "HEAD").unwrap(),
        source_head
    );
    assert_eq!(fixture.rev_parse(&path, "HEAD").unwrap(), outpost_head);
    assert_eq!(fs::read(outpost.metadata_path()).unwrap(), metadata);
    assert_eq!(
        fs::read_to_string(&parent).unwrap(),
        "user file occupies the old source parent\n"
    );
    assert_eq!(
        fs::read_to_string(path.join("draft.txt")).unwrap(),
        "uncommitted outpost work\n"
    );
}

// A legal full ref can have a suffix that Git rejects as a short branch name.
// Analysis must preserve the InvalidRefName reason in unavailable relationship
// probes while retaining known local state. Existing tests cover malformed full
// refs, non-branch refs, and malformed remote names instead.
#[test]
fn analyze_degrades_relationship_probes_for_an_unsupported_short_upstream_branch() {
    let fixture = AbcFixture::new();
    let path = fixture.add_outpost("C").expect("outpost");
    let source = fixture.source_repo().expect("source repository");
    let outpost = Outpost::at_with(&path, &fixture.git_env).expect("open outpost");
    fixture
        .invoker(&fixture.source)
        .run_check(["check-ref-format", "refs/heads/-unsupported"])
        .expect("the full upstream reference is valid Git syntax");
    fixture
        .invoker(&fixture.source)
        .run_check([
            "config",
            "--local",
            "branch.main.merge",
            "refs/heads/-unsupported",
        ])
        .expect("configure the unsupported short upstream branch");
    let main = BranchName::parse("main").expect("main");
    assert_eq!(
        source
            .upstream_for(&main)
            .expect("valid full upstream ref")
            .unwrap()
            .short_branch(),
        Some("-unsupported")
    );
    let reason = OutpostError::InvalidRefName {
        name: "-unsupported".to_owned(),
    }
    .to_string();
    let source_head = fixture
        .rev_parse(&fixture.source, "HEAD")
        .expect("source HEAD");
    let outpost_head = fixture.rev_parse(&path, "HEAD").expect("outpost HEAD");
    let registry = fs::read(source.registry_path()).expect("registry");
    let metadata = fs::read(outpost.metadata_path()).expect("metadata");
    let config_path = source.git_dir().join("config");
    let config = fs::read(&config_path).expect("source config");
    fs::write(path.join("draft.txt"), "uncommitted outpost work\n").expect("outpost work");

    let report = analyze::run(
        &source,
        AnalyzeOptions {
            selector: OutpostSelector::from_path(path.clone()),
        },
        None,
    )
    .expect("analysis retains a partial report for invalid upstream branch syntax");

    assert_eq!(report.branch, Some(main.clone()));
    assert!(report.outpost_dirty);
    assert_eq!(
        report.outpost_vs_source,
        Probe::Known(AheadBehind {
            ahead: 0,
            behind: 0
        })
    );
    assert_eq!(report.upstream_remote, Probe::Unavailable(reason.clone()));
    assert_eq!(report.upstream_branch, Probe::Unavailable(reason.clone()));
    assert_eq!(
        report.upstream_default_branch,
        Probe::Unavailable(reason.clone())
    );
    assert_eq!(
        report.source_vs_upstream,
        Probe::Unavailable(reason.clone())
    );
    assert_eq!(
        report.source_vs_upstream_default,
        Probe::Unavailable(reason)
    );
    assert_eq!(
        report.safe_delete,
        BranchDeleteSafety::No {
            branch: Some(main.clone()),
            reason: BranchCleanupSkipReason::BranchCheckedOut
        }
    );
    assert_eq!(
        report.safe_delete_findings,
        vec![BranchCleanupFinding::Skipped {
            branch: Some(main),
            reason: BranchCleanupSkipReason::BranchCheckedOut
        }]
    );
    assert_eq!(
        fixture.rev_parse(&fixture.source, "HEAD").unwrap(),
        source_head
    );
    assert_eq!(fixture.rev_parse(&path, "HEAD").unwrap(), outpost_head);
    assert_eq!(fs::read(source.registry_path()).unwrap(), registry);
    assert_eq!(fs::read(outpost.metadata_path()).unwrap(), metadata);
    assert_eq!(fs::read(config_path).unwrap(), config);
    assert_eq!(
        fs::read_to_string(path.join("draft.txt")).unwrap(),
        "uncommitted outpost work\n"
    );
}
