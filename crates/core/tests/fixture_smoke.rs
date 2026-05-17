mod common;

use common::fixture::AbcFixture;

#[test]
fn abc_fixture_builds_a_b_with_hermetic_git_env() {
    let fixture = AbcFixture::new();

    assert_eq!(
        fixture
            .invoker(&fixture.upstream)
            .run_capture(["symbolic-ref", "HEAD"])
            .expect("bare upstream should point HEAD at main"),
        "refs/heads/main"
    );
    assert_eq!(
        fixture
            .invoker(&fixture.source)
            .run_capture(["config", "core.autocrlf"])
            .expect("source should disable autocrlf"),
        "false"
    );
    assert_eq!(
        fixture
            .invoker(&fixture.source)
            .run_capture(["log", "-1", "--format=%s"])
            .expect("source should have initial commit"),
        "initial"
    );
    let source = fixture
        .source_repo()
        .expect("source repo should open with hermetic env");
    assert_eq!(
        source.work_tree(),
        std::fs::canonicalize(&fixture.source).expect("canonical source")
    );
    source
        .current_branch()
        .expect("source should have current branch");
    assert!(source
        .test_invoker()
        .argv_log()
        .iter()
        .any(|argv| argv.iter().any(|arg| arg == "symbolic-ref")));

    let source_oid = fixture
        .commit_in_source("source commit")
        .expect("source commit should succeed");
    assert_eq!(source_oid.len(), 40);

    let upstream_oid = fixture
        .commit_in_upstream("main", "upstream commit")
        .expect("upstream commit should succeed");
    assert_eq!(upstream_oid.len(), 40);

    let err = fixture
        .invoker(&fixture.source)
        .run_capture(["config", "--global", "user.name"])
        .expect_err("empty fixture global config should not contain user.name");
    match err {
        outpost_core::OutpostError::GitFailed { args, code, stderr } => {
            assert_eq!(args, r#"["config", "--global", "user.name"]"#);
            assert_eq!(code, 1);
            assert!(stderr.is_empty());
        }
        other => panic!("expected GitFailed, got {other:?}"),
    }
}
