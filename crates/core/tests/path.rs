#[allow(dead_code)]
mod common;

use common::fixture::AbcFixture;
use outpost_core::ops;
use outpost_core::selector::OutpostSelector;

#[test]
fn path_source_returns_source_work_tree() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");

    let report = ops::path::run(&source, ops::path::PathTarget::Source).expect("path source");

    assert_eq!(
        report.path,
        std::fs::canonicalize(&fixture.source).expect("canonical source")
    );
}

#[test]
fn path_outpost_resolves_live_selector() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let outpost = fixture.add_outpost("C").expect("outpost");
    let selector = OutpostSelector::from_cli_arg(&fixture.source, "../C".into());

    let report =
        ops::path::run(&source, ops::path::PathTarget::Outpost(selector)).expect("path outpost");

    assert_eq!(
        report.path,
        std::fs::canonicalize(outpost).expect("canonical outpost")
    );
}

#[test]
fn path_outpost_rejects_stale_registered_selector() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source");
    let outpost = fixture.add_outpost("C").expect("outpost");
    std::fs::remove_dir_all(&outpost).expect("remove outpost directory");
    let selector = OutpostSelector::from_cli_arg(&fixture.source, "../C".into());

    let err = ops::path::run(&source, ops::path::PathTarget::Outpost(selector))
        .expect_err("stale outpost path should fail");

    assert!(
        matches!(
            err,
            outpost_core::OutpostError::IoAt { .. }
                | outpost_core::OutpostError::RegistryEntryNotManaged(_)
        ),
        "stale registered path should not be printed for cd workflows: {err}"
    );
}
