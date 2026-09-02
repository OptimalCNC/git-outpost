#[allow(dead_code)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use outpost_core::ops::status::{ConfigProblem, SourceLocation, StatusReport, run_with};
use outpost_core::safety;
use outpost_core::selector::{OutpostSelector, resolve_entry};
use outpost_core::{
    BranchName, Outpost, OutpostError, RefName, RegistryEntry, RemoteName, UpstreamRef,
};

#[test]
fn selector_resolves_an_absolute_cli_path_without_rebasing_it() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let source = fixture.source_repo().expect("source repo");
    let absolute = fs::canonicalize(&outpost).expect("absolute outpost path");
    let selector = OutpostSelector::from_cli_arg(&fixture.root, absolute.clone());

    let resolved = resolve_entry(&source, &selector).expect("absolute path selector");

    assert_eq!(resolved.path, absolute);
}

#[test]
fn divergence_safety_rejects_an_absent_tracking_ref_after_remote_branch_probe() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("outpost");
    fixture
        .invoker(&outpost_path)
        .run_check(["update-ref", "-d", "refs/remotes/local/main"])
        .expect("remove local tracking ref");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("open outpost");
    let branch = BranchName::parse("main").expect("branch");
    let upstream = UpstreamRef {
        remote: RemoteName::parse("local").expect("remote"),
        merge_ref: RefName::parse("refs/heads/main").expect("branch ref"),
    };

    let error = safety::check_no_divergence_after_fetch(&outpost, &branch, &upstream)
        .expect_err("missing tracking ref should fail closed");

    assert!(matches!(
        error,
        OutpostError::BranchNotFound { branch, repo }
            if branch == "main" && repo == outpost.work_tree()
    ));
}

#[test]
fn registry_readd_preserves_an_existing_lock_invariant() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let source = fixture.source_repo().expect("source repo");
    let canonical = fs::canonicalize(&outpost).expect("canonical outpost path");
    let mut registry = source.registry_mut().expect("mutable registry");

    registry
        .lock(&outpost, Some("release freeze".to_owned()))
        .expect("lock");
    let mut replacement = registry.entries()[0].clone();
    replacement.remote_name = RemoteName::parse("replacement").expect("remote");
    replacement.locked = false;
    replacement.lock_reason = None;
    replacement.locked_at = None;
    registry
        .add(RegistryEntry {
            path: canonical,
            ..replacement
        })
        .expect("re-add entry");

    let entry = &registry.entries()[0];
    assert_eq!(entry.remote_name.as_str(), "replacement");
    assert!(entry.locked);
    assert_eq!(entry.lock_reason.as_deref(), Some("release freeze"));
    assert!(entry.locked_at.is_some());
    registry.save().expect("save re-added entry");
}

#[test]
fn status_keeps_an_absolute_source_path_when_its_parent_is_missing() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let configured = fixture.root.join("missing-parent").join("source");
    edit_metadata(&outpost, |metadata| {
        metadata["source_repo"] =
            serde_json::Value::String(configured.to_string_lossy().into_owned());
    });

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("status report"));

    assert_eq!(report.source, SourceLocation::Missing(configured));
}

#[test]
fn status_ignores_an_outpost_tracking_merge_ref_outside_heads() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    set_local_config(
        &fixture,
        &outpost,
        "branch.main.merge",
        "refs/remotes/local/main",
    );

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("status report"));

    assert_eq!(report.outpost_ahead_behind_source, None);
    assert!(
        report
            .problems
            .contains(&ConfigProblem::OutpostSourceTrackingUnavailable {
                branch: BranchName::parse("main").expect("branch"),
            })
    );
}

#[test]
fn destination_safety_normalizes_parent_dir_in_a_missing_destination() {
    let fixture = AbcFixture::new();
    let parent = fixture.root.join("outside");
    fs::create_dir(&parent).expect("parent");
    let child = parent.join("child");
    fs::create_dir(&child).expect("child directory");
    fs::write(child.join("marker"), "content").expect("non-empty child");

    let error = safety::check_destination_clean(&parent, Path::new("missing/../child"))
        .expect_err("normalized non-empty destination");

    assert!(matches!(
        error,
        OutpostError::DestinationExists(path) if path == PathBuf::from("missing/../child")
    ));
}

fn expect_outpost(report: StatusReport) -> outpost_core::ops::status::OutpostStatus {
    match report {
        StatusReport::Outpost(report) => report,
        StatusReport::Source(_) => panic!("expected outpost report"),
    }
}

fn edit_metadata(outpost: &Path, edit: impl FnOnce(&mut serde_json::Value)) {
    let metadata = Outpost::at(outpost).expect("managed outpost");
    let path = metadata.metadata_path();
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("metadata contents"))
            .expect("metadata JSON");
    edit(&mut value);
    fs::write(
        path,
        serde_json::to_string_pretty(&value).expect("serialize metadata"),
    )
    .expect("write metadata");
}

fn set_local_config(fixture: &AbcFixture, repo: &Path, key: &str, value: &str) {
    fixture
        .invoker(repo)
        .run_check(["config", "--local", key, value])
        .expect("set config");
}
