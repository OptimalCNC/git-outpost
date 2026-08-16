#[allow(dead_code)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::fixture::AbcFixture;
use outpost_core::ops::add::{AddCheckout, AddOptions, run as add_run};
use outpost_core::ops::status::{
    ConfigProblem, OutpostHeadStatus, OutpostStatus, RemoteRoutes, RemoteUrlList,
    RouteAvailability, SourceHead, SourceLocation, SourceUpstreamStatus, StatusReport,
    TrackedUpstream, run_with,
};
use outpost_core::{
    AheadBehind, OutpostError, OutpostId, RegistryEntry, RemoteName, Reporter, StepKind,
};

#[test]
fn s01_run_with_from_inside_outpost_reports_canonical_source_path() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let nested = outpost.join("nested");
    fs::create_dir(&nested).expect("create nested dir");

    let report = expect_outpost(run_with(&nested, &fixture.git_env).expect("status report"));

    assert_eq!(
        report.source,
        SourceLocation::Present(canonical(&fixture.source))
    );
}

#[test]
fn s02_run_with_reports_local_remote_name() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("status report"));

    assert_eq!(
        report.remote_name.as_ref().map(|remote| remote.as_str()),
        Some("local")
    );
}

#[test]
fn s03_run_with_reports_current_branch_and_detached_head() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");

    let branch_report =
        expect_outpost(run_with(&outpost, &fixture.git_env).expect("branch status report"));

    assert!(matches!(
        branch_report.head,
        OutpostHeadStatus::Attached { ref branch, .. } if branch.as_str() == "main"
    ));

    fixture
        .invoker(&outpost)
        .run_check(["checkout", "--detach"])
        .expect("detach HEAD");

    let detached_report =
        expect_outpost(run_with(&outpost, &fixture.git_env).expect("detached status report"));

    assert_eq!(detached_report.head, OutpostHeadStatus::Detached);
}

#[test]
fn s04_run_with_reports_dirty_state_for_untracked_files() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");

    let clean_report =
        expect_outpost(run_with(&outpost, &fixture.git_env).expect("clean status report"));

    assert!(!clean_report.outpost_dirty);

    fs::write(outpost.join("untracked.txt"), "new").expect("write untracked file");

    let dirty_report =
        expect_outpost(run_with(&outpost, &fixture.git_env).expect("dirty status report"));

    assert!(dirty_report.outpost_dirty);
}

#[test]
fn s05_run_with_reports_outpost_ahead_behind_source_from_existing_refs() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let source_seen = fixture
        .commit_in_source("source seen by outpost")
        .expect("source seen commit");
    update_remote_tracking_ref(&fixture, &outpost, "local", "main", &source_seen);
    fixture
        .commit_in_outpost(&outpost, "outpost commit")
        .expect("outpost commit");
    fixture
        .commit_in_source("source not fetched by status")
        .expect("source unseen commit");
    let remote_ref_before = rev_parse(&fixture, &outpost, "refs/remotes/local/main");

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("status report"));

    assert_eq!(
        report.outpost_ahead_behind_source,
        Some(AheadBehind {
            ahead: 1,
            behind: 1
        })
    );
    assert_eq!(
        rev_parse(&fixture, &outpost, "refs/remotes/local/main"),
        remote_ref_before
    );
}

#[test]
fn s06_run_with_reports_source_ahead_behind_upstream_from_existing_refs() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    fixture
        .commit_in_source("source commit")
        .expect("source commit");
    let upstream_seen = fixture
        .commit_in_upstream("main", "upstream seen by source")
        .expect("upstream seen commit");
    set_branch_upstream(&fixture, &fixture.source, "main", "origin");
    update_remote_tracking_ref(&fixture, &fixture.source, "origin", "main", &upstream_seen);
    fixture
        .commit_in_upstream("main", "upstream not fetched by status")
        .expect("upstream unseen commit");
    let remote_ref_before = rev_parse(&fixture, &fixture.source, "refs/remotes/origin/main");

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("status report"));

    assert_eq!(
        report.source_ahead_behind_upstream,
        Some(AheadBehind {
            ahead: 1,
            behind: 1
        })
    );
    assert_eq!(
        rev_parse(&fixture, &fixture.source, "refs/remotes/origin/main"),
        remote_ref_before
    );
}

#[test]
fn s10_run_with_reports_missing_source_problem() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let moved_source = fixture.root.join("B.moved");
    fs::rename(&fixture.source, &moved_source).expect("move source repo");

    let report =
        expect_outpost(run_with(&outpost, &fixture.git_env).expect("degraded status report"));

    assert_eq!(
        report.source,
        SourceLocation::Missing(canonical_missing(&fixture.source))
    );
    assert!(
        report
            .problems
            .contains(&ConfigProblem::SourceMissing(canonical_missing(
                &fixture.source
            )))
    );
}

#[test]
fn s11_run_with_flags_local_remote_mismatch() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    set_remote_url(&fixture, &outpost, "local", &fixture.upstream);

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("status report"));

    assert!(
        report
            .problems
            .contains(&ConfigProblem::LocalRemoteMismatch {
                configured: canonical(&fixture.source),
                actual: canonical(&fixture.upstream),
            })
    );
}

#[test]
fn s12_run_with_uses_metadata_remote_name_for_custom_remote() {
    let fixture = AbcFixture::new();
    let outpost = add_outpost_with_remote(&fixture, "C", "custom");
    let source_seen = fixture
        .commit_in_source("source seen by custom outpost")
        .expect("source seen commit");
    update_remote_tracking_ref(&fixture, &outpost, "custom", "main", &source_seen);
    fixture
        .commit_in_outpost(&outpost, "custom outpost commit")
        .expect("outpost commit");
    assert!(matches!(
        fixture
            .invoker(&outpost)
            .run_capture(["remote", "get-url", "local"]),
        Err(OutpostError::GitFailed { .. })
    ));

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("status report"));

    assert_eq!(
        report.remote_name.as_ref().map(|remote| remote.as_str()),
        Some("custom")
    );
    assert_eq!(
        report.outpost_ahead_behind_source,
        Some(AheadBehind {
            ahead: 1,
            behind: 1
        })
    );
    assert!(
        !report
            .problems
            .iter()
            .any(|problem| matches!(problem, ConfigProblem::LocalRemoteMismatch { .. }))
    );
}

#[test]
fn run_with_flags_not_in_registry_when_outpost_entry_is_missing() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    remove_from_registry(&fixture, &outpost);

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("status report"));

    assert!(report.problems.contains(&ConfigProblem::NotInRegistry));
}

#[test]
fn run_with_flags_push_would_fail_when_source_refuses_checked_out_branch_update() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    set_local_config(
        &fixture,
        &fixture.source,
        "receive.denyCurrentBranch",
        "refuse",
    );

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("status report"));

    assert!(report.problems.contains(&ConfigProblem::PushWouldFail {
        branch: outpost_core::BranchName::parse("main").expect("branch"),
    }));
}

#[test]
fn s07_run_with_accepts_explicit_outpost_target_path() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let cwd = std::env::current_dir().expect("current dir");
    assert!(!cwd.starts_with(&outpost));

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("status report"));

    assert_eq!(report.outpost_path, canonical(&outpost));
}

#[test]
fn source_context_succeeds_from_root_and_nested_directory() {
    let fixture = AbcFixture::new();
    let nested = fixture.source.join("nested");
    fs::create_dir(&nested).expect("create nested");

    let root_report = run_with(&fixture.source, &fixture.git_env).expect("root source status");
    let nested_report = run_with(&nested, &fixture.git_env).expect("nested source status");

    for report in [root_report, nested_report] {
        let StatusReport::Source(report) = report else {
            panic!("expected source report");
        };
        assert_eq!(report.source_path, canonical(&fixture.source));
    }
}

#[test]
fn explicit_false_marker_is_source_and_invalid_marker_is_an_error() {
    let fixture = AbcFixture::new();
    set_local_config(&fixture, &fixture.source, "outpost.managed", "false");
    assert!(matches!(
        run_with(&fixture.source, &fixture.git_env).expect("source report"),
        StatusReport::Source(_)
    ));

    set_local_config(&fixture, &fixture.source, "outpost.managed", "maybe");
    assert!(matches!(
        expect_error(
            run_with(&fixture.source, &fixture.git_env),
            "invalid marker"
        ),
        OutpostError::BadMetadata { .. }
    ));
}

#[test]
fn source_dirty_includes_staged_unstaged_and_untracked_changes_but_excludes_ignored_files() {
    let fixture = AbcFixture::new();
    let tracked = fixture.source.join("tracked.txt");
    fs::write(&tracked, "initial\n").expect("write tracked file");
    fixture
        .invoker(&fixture.source)
        .run_check(["add", "tracked.txt"])
        .expect("stage tracked file");
    fixture
        .invoker(&fixture.source)
        .run_check(["commit", "-m", "add tracked file"])
        .expect("commit tracked file");

    fs::write(fixture.source.join("staged.txt"), "staged\n").expect("write staged file");
    fixture
        .invoker(&fixture.source)
        .run_check(["add", "staged.txt"])
        .expect("stage new file");
    assert!(
        expect_source(run_with(&fixture.source, &fixture.git_env).expect("staged status"))
            .source_dirty
    );
    fixture
        .invoker(&fixture.source)
        .run_check(["reset", "--hard"])
        .expect("reset staged file");

    fs::write(&tracked, "modified\n").expect("modify tracked file");
    assert!(
        expect_source(run_with(&fixture.source, &fixture.git_env).expect("unstaged status"))
            .source_dirty
    );
    fixture
        .invoker(&fixture.source)
        .run_check(["checkout", "--", "tracked.txt"])
        .expect("restore tracked file");

    fs::write(fixture.source.join("untracked.txt"), "untracked\n").expect("write untracked file");
    assert!(
        expect_source(run_with(&fixture.source, &fixture.git_env).expect("untracked status"))
            .source_dirty
    );
    fs::remove_file(fixture.source.join("untracked.txt")).expect("remove untracked file");

    fs::write(fixture.source.join(".git/info/exclude"), "ignored.txt\n")
        .expect("exclude ignored file");
    fs::write(fixture.source.join("ignored.txt"), "ignored\n").expect("write ignored file");
    assert!(
        !expect_source(run_with(&fixture.source, &fixture.git_env).expect("ignored status"))
            .source_dirty
    );
}

#[test]
fn source_upstream_keeps_local_and_target_branch_names_separate() {
    let fixture = AbcFixture::new();
    fixture
        .invoker(&fixture.source)
        .run_check(["branch", "-m", "release-prep"])
        .expect("rename source branch");
    set_branch_tracking(&fixture, &fixture.source, "release-prep", "origin", "main");
    set_remote_url_text(
        &fixture,
        &fixture.source,
        "origin",
        "https://example.test/widget.git",
    );
    set_remote_push_url_text(
        &fixture,
        &fixture.source,
        "origin",
        "ssh://git@example.test/widget.git",
    );

    let report = expect_source(run_with(&fixture.source, &fixture.git_env).expect("source status"));

    assert_eq!(
        report.head,
        SourceHead::Attached {
            branch: branch("release-prep"),
            upstream: Some(TrackedUpstream::Remote {
                remote: remote("origin"),
                branch: branch("main"),
                routes: RemoteRoutes {
                    fetch: RouteAvailability::Known(urls(["https://example.test/widget.git",])),
                    push: RouteAvailability::Known(urls(["ssh://git@example.test/widget.git",])),
                },
            }),
        }
    );
}

#[test]
fn source_upstream_deduplicates_multiple_urls_in_first_seen_order() {
    let fixture = AbcFixture::new();
    set_branch_tracking(&fixture, &fixture.source, "main", "origin", "main");
    unset_all_local_config(&fixture, &fixture.source, "remote.origin.url");
    for url in [
        "https://first.example/widget.git",
        "https://second.example/widget.git",
        "https://first.example/widget.git",
    ] {
        add_local_config(&fixture, &fixture.source, "remote.origin.url", url);
    }

    let report = expect_source(run_with(&fixture.source, &fixture.git_env).expect("source status"));
    let SourceHead::Attached {
        upstream: Some(TrackedUpstream::Remote { routes, .. }),
        ..
    } = report.head
    else {
        panic!("expected remote upstream");
    };

    let expected = urls([
        "https://first.example/widget.git",
        "https://second.example/widget.git",
    ]);
    assert_eq!(routes.fetch, RouteAvailability::Known(expected.clone()));
    assert_eq!(routes.push, RouteAvailability::Known(expected));
}

#[test]
fn source_upstream_reports_missing_named_remote_as_unavailable() {
    let fixture = AbcFixture::new();
    set_branch_tracking(&fixture, &fixture.source, "main", "missing", "main");

    let report = expect_source(run_with(&fixture.source, &fixture.git_env).expect("source status"));

    assert_eq!(
        report.head,
        SourceHead::Attached {
            branch: branch("main"),
            upstream: Some(TrackedUpstream::Remote {
                remote: remote("missing"),
                branch: branch("main"),
                routes: RemoteRoutes {
                    fetch: RouteAvailability::Unavailable,
                    push: RouteAvailability::Unavailable,
                },
            }),
        }
    );
}

#[test]
fn source_upstream_without_effective_url_maps_git_exit_two_to_unavailable_routes() {
    let fixture = AbcFixture::new();
    set_branch_tracking(&fixture, &fixture.source, "main", "origin", "main");
    fixture
        .invoker(&fixture.source)
        .run_check(["config", "--local", "--remove-section", "remote.origin"])
        .expect("remove configured remote URLs");

    let report = expect_source(run_with(&fixture.source, &fixture.git_env).expect("source status"));

    assert_eq!(
        report.head,
        SourceHead::Attached {
            branch: branch("main"),
            upstream: Some(TrackedUpstream::Remote {
                remote: remote("origin"),
                branch: branch("main"),
                routes: RemoteRoutes {
                    fetch: RouteAvailability::Unavailable,
                    push: RouteAvailability::Unavailable,
                },
            }),
        }
    );
}

#[cfg(unix)]
#[test]
fn source_upstream_empty_successful_route_output_is_invalid_git_output() {
    let fixture = AbcFixture::new();
    set_branch_tracking(&fixture, &fixture.source, "main", "origin", "main");
    let env = git_env_with_empty_route_output(&fixture);

    let error = expect_error(
        run_with(&fixture.source, &env),
        "empty successful route output should be invalid Git output",
    );

    assert!(matches!(
        error,
        OutpostError::IoAt { source, .. }
            if source.kind() == std::io::ErrorKind::InvalidData
                && source.to_string() == "git remote get-url returned no URLs"
    ));
}

#[test]
fn source_upstream_is_unset_for_incomplete_tracking_and_not_applicable_when_detached() {
    let fixture = AbcFixture::new();
    unset_local_config(&fixture, &fixture.source, "branch.main.merge");
    let unset = expect_source(run_with(&fixture.source, &fixture.git_env).expect("source status"));
    assert_eq!(
        unset.head,
        SourceHead::Attached {
            branch: branch("main"),
            upstream: None,
        }
    );

    fixture
        .invoker(&fixture.source)
        .run_check(["checkout", "--detach"])
        .expect("detach source");
    let detached =
        expect_source(run_with(&fixture.source, &fixture.git_env).expect("detached status"));
    assert_eq!(detached.head, SourceHead::Detached);
}

#[test]
fn source_upstream_dot_remote_is_local_repository() {
    let fixture = AbcFixture::new();
    set_branch_tracking(&fixture, &fixture.source, "main", ".", "main");

    let report = expect_source(run_with(&fixture.source, &fixture.git_env).expect("source status"));

    assert_eq!(
        report.head,
        SourceHead::Attached {
            branch: branch("main"),
            upstream: Some(TrackedUpstream::LocalRepository {
                branch: branch("main"),
            }),
        }
    );
}

#[cfg(unix)]
#[test]
fn source_upstream_propagates_non_two_git_route_failure() {
    let fixture = AbcFixture::new();
    set_branch_tracking(&fixture, &fixture.source, "main", "origin", "main");
    let env = git_env_with_route_failure(&fixture, 7);

    let error = expect_error(
        run_with(&fixture.source, &env),
        "route failure should propagate",
    );

    assert!(matches!(error, OutpostError::GitFailed { code: 7, .. }));
}

#[test]
fn source_registry_absent_is_an_empty_inventory() {
    let fixture = AbcFixture::new();

    let report = expect_source(run_with(&fixture.source, &fixture.git_env).expect("source status"));

    assert!(report.outposts.is_empty());
    assert!(report.stale_registrations.is_empty());
}

#[test]
fn source_registry_live_rows_preserve_order_and_local_state() {
    let fixture = AbcFixture::new();
    let first = fixture.add_outpost("C").expect("add C");
    let second = fixture.add_outpost("D").expect("add D");
    fs::write(first.join("dirty.txt"), "dirty").expect("dirty first outpost");
    fixture
        .invoker(&second)
        .run_check(["checkout", "--detach"])
        .expect("detach second outpost");
    let source = fixture.source_repo().expect("source repo");
    let mut registry = source.registry_mut().expect("registry mut");
    registry
        .lock(&second, Some("retain".to_owned()))
        .expect("lock second");
    registry.save().expect("save registry");

    let report = expect_source(run_with(&fixture.source, &fixture.git_env).expect("source status"));

    assert_eq!(report.outposts.len(), 2);
    assert_eq!(report.outposts[0].path, canonical(&first));
    assert_eq!(report.outposts[0].head, registered_branch("main"));
    assert!(report.outposts[0].dirty);
    assert!(!report.outposts[0].locked);
    assert_eq!(report.outposts[1].path, canonical(&second));
    assert_eq!(
        report.outposts[1].head,
        outpost_core::ops::status::RegisteredOutpostHead::Detached
    );
    assert!(!report.outposts[1].dirty);
    assert!(report.outposts[1].locked);
    assert!(
        report
            .outposts
            .iter()
            .all(|row| row.display_id.as_str().len() >= 5)
    );
    assert_ne!(report.outposts[0].display_id, report.outposts[1].display_id);
}

#[test]
fn source_registry_id_prefixes_include_stale_entries() {
    let fixture = AbcFixture::new();
    let (live_name, stale_name) = colliding_names(&fixture);
    let live = fixture.add_outpost(&live_name).expect("add live collision");
    let stale = fixture.root.join(&stale_name);
    fs::create_dir(&stale).expect("create stale path");
    let source = fixture.source_repo().expect("source repo");
    let mut registry = source.registry_mut().expect("registry mut");
    registry
        .add(RegistryEntry::new(stale.clone(), remote("local")).expect("stale registry entry"))
        .expect("add stale entry");
    registry.save().expect("save registry");
    fs::remove_dir(&stale).expect("remove stale path");

    let report = expect_source(run_with(&fixture.source, &fixture.git_env).expect("source status"));

    assert_eq!(report.outposts[0].path, canonical(&live));
    assert_eq!(
        report.stale_registrations[0].path,
        canonical_missing(&stale)
    );
    assert!(report.outposts[0].display_id.as_str().len() > 5);
    assert!(report.stale_registrations[0].display_id.as_str().len() > 5);
    assert_ne!(
        report.outposts[0].display_id,
        report.stale_registrations[0].display_id
    );
}

#[test]
fn source_registry_not_found_is_stale() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    fs::remove_dir_all(&outpost).expect("remove outpost checkout");

    let report = expect_source(run_with(&fixture.source, &fixture.git_env).expect("source status"));

    assert!(report.outposts.is_empty());
    assert_eq!(report.stale_registrations.len(), 1);
    assert_eq!(
        report.stale_registrations[0].path,
        canonical_missing(&outpost)
    );
}

#[cfg(unix)]
#[test]
fn source_registry_metadata_io_error_is_not_stale() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = AbcFixture::new();
    let sealed = fixture.root.join("sealed");
    let path = sealed.join("C");
    fs::create_dir_all(&path).expect("create registered path");
    let source = fixture.source_repo().expect("source repo");
    let mut registry = source.registry_mut().expect("registry mut");
    registry
        .add(RegistryEntry::new(path.clone(), remote("local")).expect("entry"))
        .expect("add entry");
    registry.save().expect("save registry");
    fs::set_permissions(&sealed, fs::Permissions::from_mode(0o000)).expect("seal parent");

    let result = run_with(&fixture.source, &fixture.git_env);

    fs::set_permissions(&sealed, fs::Permissions::from_mode(0o755)).expect("restore parent");
    assert!(matches!(
        expect_error(result, "metadata error must propagate"),
        OutpostError::IoAt { path: error_path, source }
            if error_path == path && source.kind() == std::io::ErrorKind::PermissionDenied
    ));
}

#[test]
fn source_registry_duplicate_paths_are_bad_registry() {
    let fixture = AbcFixture::new();
    fixture.add_outpost("C").expect("add C");
    duplicate_first_registry_entry(&fixture);

    assert!(matches!(
        expect_error(
            run_with(&fixture.source, &fixture.git_env),
            "duplicate registry path"
        ),
        OutpostError::BadRegistry { .. }
    ));
}

#[test]
fn source_registry_malformed_required_entry_fields_are_bad_registry() {
    let fixture = AbcFixture::new();
    fixture.add_outpost("C").expect("add C");
    edit_first_registry_entry(&fixture, |entry| {
        entry["created_at"] = serde_json::Value::String("not-a-timestamp".to_owned());
    });

    assert!(matches!(
        expect_error(
            run_with(&fixture.source, &fixture.git_env),
            "malformed registry entry"
        ),
        OutpostError::BadRegistry { .. }
    ));
}

#[test]
fn source_registry_path_replaced_by_file_is_an_integrity_error() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    fs::remove_dir_all(&outpost).expect("remove checkout");
    fs::write(&outpost, "replacement file").expect("replace checkout with file");

    assert_integrity(&fixture, &outpost);
}

#[cfg(unix)]
#[test]
fn source_registry_path_rebound_to_same_source_outpost_is_an_integrity_error() {
    use std::os::unix::fs::symlink;

    let fixture = AbcFixture::new();
    let recorded = fixture.add_outpost("C").expect("add C");
    let target = fixture.add_outpost("D").expect("add D");
    let recorded_path = canonical(&recorded);
    fs::remove_dir_all(&recorded).expect("remove recorded checkout");
    symlink(&target, &recorded).expect("replace recorded checkout with symlink");

    let error = expect_error(
        run_with(&fixture.source, &fixture.git_env),
        "symlink replacement should be an integrity error",
    );

    assert!(matches!(
        error,
        OutpostError::RegisteredOutpostIntegrity { source, outpost }
            if source == canonical(&fixture.source) && outpost == recorded_path
    ));
}

#[test]
fn source_registry_outpost_container_supports_unset_configured_and_malformed() {
    let unset_fixture = AbcFixture::new();
    let unset = expect_source(
        run_with(&unset_fixture.source, &unset_fixture.git_env).expect("unset config status"),
    );
    assert_eq!(unset.outpost_container, None);

    let configured_fixture = AbcFixture::new();
    let container = configured_fixture.root.join("outposts");
    fs::create_dir(&container).expect("create container");
    configured_fixture
        .source_repo()
        .expect("source repo")
        .set_outpost_container(&container)
        .expect("set container");
    let configured = expect_source(
        run_with(&configured_fixture.source, &configured_fixture.git_env)
            .expect("configured status"),
    );
    assert_eq!(configured.outpost_container, Some(canonical(&container)));

    let malformed_fixture = AbcFixture::new();
    write_source_config(
        &malformed_fixture,
        r#"{"version":1,"outpost_container":"relative"}"#,
    );
    assert!(matches!(
        expect_error(
            run_with(&malformed_fixture.source, &malformed_fixture.git_env),
            "malformed config"
        ),
        OutpostError::BadConfig { .. }
    ));
}

#[test]
fn source_registry_existing_contradictions_are_integrity_errors() {
    let missing_marker = AbcFixture::new();
    let outpost = missing_marker.add_outpost("C").expect("add C");
    unset_local_config(&missing_marker, &outpost, "outpost.managed");
    assert_integrity(&missing_marker, &outpost);

    let false_marker = AbcFixture::new();
    let outpost = false_marker.add_outpost("C").expect("add C");
    set_local_config(&false_marker, &outpost, "outpost.managed", "false");
    assert_integrity(&false_marker, &outpost);

    let wrong_source = AbcFixture::new();
    let outpost = wrong_source.add_outpost("C").expect("add C");
    set_local_config(
        &wrong_source,
        &outpost,
        "outpost.sourceRepo",
        wrong_source.upstream.to_str().expect("upstream path"),
    );
    assert_integrity(&wrong_source, &outpost);

    let wrong_metadata_remote = AbcFixture::new();
    let outpost = wrong_metadata_remote.add_outpost("C").expect("add C");
    set_local_config(
        &wrong_metadata_remote,
        &outpost,
        "outpost.remoteName",
        "other",
    );
    assert_integrity(&wrong_metadata_remote, &outpost);

    let missing_recorded_remote = AbcFixture::new();
    let outpost = missing_recorded_remote.add_outpost("C").expect("add C");
    missing_recorded_remote
        .invoker(&outpost)
        .run_check(["remote", "remove", "local"])
        .expect("remove recorded remote");
    assert_integrity(&missing_recorded_remote, &outpost);

    let redirected_remote = AbcFixture::new();
    let outpost = redirected_remote.add_outpost("C").expect("add C");
    set_remote_url(
        &redirected_remote,
        &outpost,
        "local",
        &redirected_remote.upstream,
    );
    assert_integrity(&redirected_remote, &outpost);
}

#[test]
fn outpost_source_upstream_is_reported_even_without_outpost_remote_metadata() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    unset_local_config(&fixture, &outpost, "outpost.remoteName");

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("outpost status"));

    assert!(
        report
            .problems
            .contains(&ConfigProblem::MissingRemoteNameConfig)
    );
    let OutpostHeadStatus::Attached {
        branch: current_branch,
        source_upstream,
    } = report.head
    else {
        panic!("expected attached outpost");
    };
    assert_eq!(current_branch, branch("main"));
    assert_eq!(
        source_upstream,
        SourceUpstreamStatus::Configured(TrackedUpstream::Remote {
            remote: remote("origin"),
            branch: branch("main"),
            routes: RemoteRoutes {
                fetch: RouteAvailability::Known(urls([canonical(&fixture.upstream)
                    .to_str()
                    .expect("upstream path"),])),
                push: RouteAvailability::Known(urls([canonical(&fixture.upstream)
                    .to_str()
                    .expect("upstream path"),])),
            },
        })
    );
}

#[test]
fn outpost_source_upstream_unset_names_source_relationship() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    unset_local_config(&fixture, &fixture.source, "branch.main.remote");
    unset_local_config(&fixture, &fixture.source, "branch.main.merge");

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("outpost status"));

    assert!(matches!(
        report.head,
        OutpostHeadStatus::Attached {
            source_upstream: SourceUpstreamStatus::Unset,
            ..
        }
    ));
    assert!(
        report
            .problems
            .contains(&ConfigProblem::SourceUpstreamTrackingUnset {
                branch: branch("main"),
            })
    );
}

#[test]
fn outpost_source_branch_missing_is_unavailable_without_using_stale_config() {
    let fixture = AbcFixture::new();
    let feature = fixture
        .create_source_branch("feature")
        .expect("feature branch");
    let outpost = fixture
        .add_outpost_on_branch("C", Some(feature.clone()))
        .expect("add feature outpost");
    set_branch_tracking(&fixture, &fixture.source, "feature", "origin", "main");
    fixture
        .delete_source_branch(&feature)
        .expect("delete source branch");

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("outpost status"));

    assert!(matches!(
        report.head,
        OutpostHeadStatus::Attached {
            source_upstream: SourceUpstreamStatus::Unavailable,
            ..
        }
    ));
    assert!(
        report
            .problems
            .contains(&ConfigProblem::SourceBranchMissing { branch: feature })
    );
}

#[test]
fn outpost_source_upstream_unavailable_route_is_typed_and_diagnostic() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    set_branch_tracking(&fixture, &fixture.source, "main", "missing", "main");

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("outpost status"));

    assert!(matches!(
        report.head,
        OutpostHeadStatus::Attached {
            source_upstream: SourceUpstreamStatus::Configured(TrackedUpstream::Remote {
                routes: RemoteRoutes {
                    fetch: RouteAvailability::Unavailable,
                    push: RouteAvailability::Unavailable,
                },
                ..
            }),
            ..
        }
    ));
    assert!(
        report
            .problems
            .contains(&ConfigProblem::SourceUpstreamRouteUnavailable {
                remote: remote("missing"),
            })
    );
}

#[test]
fn outpost_to_source_tracking_unavailable_names_outpost_relationship() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    unset_local_config(&fixture, &outpost, "branch.main.merge");

    let report = expect_outpost(run_with(&outpost, &fixture.git_env).expect("outpost status"));

    assert_eq!(report.outpost_ahead_behind_source, None);
    assert!(
        report
            .problems
            .contains(&ConfigProblem::OutpostSourceTrackingUnavailable {
                branch: branch("main"),
            })
    );
}

#[cfg(unix)]
#[test]
fn outpost_source_path_access_error_is_not_reported_missing() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let sealed = fixture.root.join("sealed-source-parent");
    let configured = sealed.join("source");
    fs::create_dir_all(&configured).expect("create configured source path");
    set_local_config(
        &fixture,
        &outpost,
        "outpost.sourceRepo",
        configured.to_str().expect("configured path"),
    );
    fs::set_permissions(&sealed, fs::Permissions::from_mode(0o000)).expect("seal source parent");

    let result = run_with(&outpost, &fixture.git_env);

    fs::set_permissions(&sealed, fs::Permissions::from_mode(0o755)).expect("restore source parent");
    assert!(matches!(
        expect_error(result, "source metadata error must propagate"),
        OutpostError::IoAt { path, source }
            if path == configured && source.kind() == std::io::ErrorKind::PermissionDenied
    ));
}

#[cfg(unix)]
#[test]
fn status_from_source_and_outpost_is_local_and_does_not_change_refs() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    let source_refs_before = refs_snapshot(&fixture, &fixture.source);
    let outpost_refs_before = refs_snapshot(&fixture, &outpost);
    let env = git_env_rejecting_forbidden_status_commands(&fixture);

    run_with(&fixture.source, &env).expect("local source status");
    run_with(&outpost, &env).expect("local outpost status");

    assert_eq!(refs_snapshot(&fixture, &fixture.source), source_refs_before);
    assert_eq!(refs_snapshot(&fixture, &outpost), outpost_refs_before);
}

#[test]
fn s09_missing_source_repo_config_is_reported_as_problem() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    unset_local_config(&fixture, &outpost, "outpost.sourceRepo");

    let report =
        expect_outpost(run_with(&outpost, &fixture.git_env).expect("degraded status report"));

    assert!(
        report
            .problems
            .contains(&ConfigProblem::MissingSourceRepoConfig)
    );
}

#[test]
fn s13_missing_source_repo_config_keeps_degraded_report_available() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add C");
    unset_local_config(&fixture, &outpost, "outpost.sourceRepo");

    let report =
        expect_outpost(run_with(&outpost, &fixture.git_env).expect("degraded status report"));

    assert_eq!(report.source, SourceLocation::Unconfigured);
    assert_eq!(
        report.remote_name.as_ref().map(|remote| remote.as_str()),
        Some("local")
    );
    assert!(
        report
            .problems
            .contains(&ConfigProblem::MissingSourceRepoConfig)
    );
}

fn unset_local_config(fixture: &AbcFixture, repo: &Path, key: &str) {
    fixture
        .invoker(repo)
        .run_check(["config", "--local", "--unset", key])
        .expect("unset local config");
}

fn set_branch_upstream(fixture: &AbcFixture, repo: &Path, branch: &str, remote: &str) {
    set_branch_tracking(fixture, repo, branch, remote, branch);
}

fn set_branch_tracking(
    fixture: &AbcFixture,
    repo: &Path,
    branch: &str,
    remote: &str,
    target_branch: &str,
) {
    let remote_key = format!("branch.{branch}.remote");
    set_local_config(fixture, repo, &remote_key, remote);
    let merge_key = format!("branch.{branch}.merge");
    let merge_ref = format!("refs/heads/{target_branch}");
    set_local_config(fixture, repo, &merge_key, &merge_ref);
}

fn set_local_config(fixture: &AbcFixture, repo: &Path, key: &str, value: &str) {
    fixture
        .invoker(repo)
        .run_check(["config", "--local", key, value])
        .expect("set local config");
}

fn set_remote_url(fixture: &AbcFixture, repo: &Path, remote: &str, url: &Path) {
    fixture
        .invoker(repo)
        .run_check([
            "remote".into(),
            "set-url".into(),
            remote.into(),
            url.as_os_str().to_os_string(),
        ])
        .expect("set remote url");
}

fn set_remote_url_text(fixture: &AbcFixture, repo: &Path, remote: &str, url: &str) {
    fixture
        .invoker(repo)
        .run_check(["remote", "set-url", remote, url])
        .expect("set remote URL");
}

fn set_remote_push_url_text(fixture: &AbcFixture, repo: &Path, remote: &str, url: &str) {
    fixture
        .invoker(repo)
        .run_check(["remote", "set-url", "--push", remote, url])
        .expect("set remote push URL");
}

fn add_local_config(fixture: &AbcFixture, repo: &Path, key: &str, value: &str) {
    fixture
        .invoker(repo)
        .run_check(["config", "--local", "--add", key, value])
        .expect("add local config");
}

fn unset_all_local_config(fixture: &AbcFixture, repo: &Path, key: &str) {
    let result = fixture
        .invoker(repo)
        .run_check(["config", "--local", "--unset-all", key]);
    if let Err(error) = result {
        assert!(matches!(error, OutpostError::GitFailed { code: 5, .. }));
    }
}

fn remove_from_registry(fixture: &AbcFixture, outpost: &Path) {
    let source = fixture.source_repo().expect("source repo");
    let mut registry = source.registry_mut().expect("registry mut");
    assert!(
        registry
            .remove_by_path(outpost)
            .expect("remove registry entry")
    );
    registry.save().expect("save registry");
}

fn update_remote_tracking_ref(
    fixture: &AbcFixture,
    repo: &Path,
    remote: &str,
    branch: &str,
    oid: &str,
) {
    let ref_name = format!("refs/remotes/{remote}/{branch}");
    let fetch_refspec = format!("refs/heads/{branch}:{ref_name}");
    fixture
        .invoker(repo)
        .run_check(["fetch", remote, &fetch_refspec])
        .expect("fetch remote tracking ref");
    assert_eq!(rev_parse(fixture, repo, &ref_name), oid);
}

fn rev_parse(fixture: &AbcFixture, repo: &Path, rev: &str) -> String {
    fixture
        .invoker(repo)
        .run_capture(["rev-parse", rev])
        .expect("rev-parse")
}

fn add_outpost_with_remote(fixture: &AbcFixture, name: &str, remote_name: &str) -> PathBuf {
    let source = fixture.source_repo().expect("source repo");
    let destination = fixture.root.join(name);
    let mut reporter = NoopReporter;
    add_run(
        &source,
        AddOptions {
            destination: destination.clone(),
            checkout: AddCheckout::CheckoutExisting {
                target_branch: None,
            },
            remote_name: RemoteName::parse(remote_name).expect("remote name"),
        },
        &mut reporter,
    )
    .expect("add outpost");
    destination
}

struct NoopReporter;

impl Reporter for NoopReporter {
    fn step(&mut self, _kind: StepKind, _message: &str) {}

    fn warn(&mut self, _message: &str) {}
}

fn expect_outpost(report: StatusReport) -> OutpostStatus {
    match report {
        StatusReport::Outpost(report) => report,
        StatusReport::Source(report) => {
            panic!(
                "expected outpost report, got source {}",
                report.source_path.display()
            )
        }
    }
}

fn expect_source(report: StatusReport) -> outpost_core::ops::status::SourceStatus {
    match report {
        StatusReport::Source(report) => report,
        StatusReport::Outpost(report) => {
            panic!(
                "expected source report, got outpost {}",
                report.outpost_path.display()
            )
        }
    }
}

fn branch(value: &str) -> outpost_core::BranchName {
    outpost_core::BranchName::parse(value).expect("branch")
}

fn remote(value: &str) -> RemoteName {
    RemoteName::parse(value).expect("remote")
}

fn urls<const N: usize>(values: [&str; N]) -> RemoteUrlList {
    RemoteUrlList::for_tests(values)
}

#[cfg(unix)]
fn git_env_with_route_failure(
    fixture: &AbcFixture,
    exit_code: u8,
) -> std::collections::BTreeMap<std::ffi::OsString, std::ffi::OsString> {
    use std::os::unix::fs::PermissionsExt;

    let shim_dir = fixture.root.join("git-shim");
    fs::create_dir(&shim_dir).expect("create shim dir");
    let real_git = find_git_executable();
    let shim = shim_dir.join("git");
    fs::write(
        &shim,
        format!(
            "#!/bin/sh\nif [ \"$1\" = remote ] && [ \"$2\" = get-url ]; then exit {exit_code}; fi\nexec '{}' \"$@\"\n",
            real_git.display()
        ),
    )
    .expect("write git shim");
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).expect("chmod git shim");

    let mut env = fixture.git_env.clone();
    env.insert("PATH".into(), shim_dir.into_os_string());
    env
}

#[cfg(unix)]
fn git_env_with_empty_route_output(
    fixture: &AbcFixture,
) -> std::collections::BTreeMap<std::ffi::OsString, std::ffi::OsString> {
    use std::os::unix::fs::PermissionsExt;

    let shim_dir = fixture.root.join("empty-route-git-shim");
    fs::create_dir(&shim_dir).expect("create shim dir");
    let real_git = find_git_executable();
    let shim = shim_dir.join("git");
    fs::write(
        &shim,
        format!(
            "#!/bin/sh\nif [ \"$1\" = remote ] && [ \"$2\" = get-url ] && [ \"$3\" = --all ] && [ \"$4\" = origin ]; then exit 0; fi\nexec '{}' \"$@\"\n",
            real_git.display()
        ),
    )
    .expect("write git shim");
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).expect("chmod git shim");

    let mut env = fixture.git_env.clone();
    env.insert("PATH".into(), shim_dir.into_os_string());
    env
}

#[cfg(unix)]
fn git_env_rejecting_forbidden_status_commands(
    fixture: &AbcFixture,
) -> std::collections::BTreeMap<std::ffi::OsString, std::ffi::OsString> {
    use std::os::unix::fs::PermissionsExt;

    let shim_dir = fixture.root.join("status-local-git-shim");
    fs::create_dir(&shim_dir).expect("create local-only shim dir");
    let real_git = find_git_executable();
    let shim = shim_dir.join("git");
    fs::write(
        &shim,
        format!(
            "#!/bin/sh\ncase \"$1\" in fetch|pull|push|ls-remote|update-ref) exit 97 ;; esac\nexec '{}' \"$@\"\n",
            real_git.display()
        ),
    )
    .expect("write local-only git shim");
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755))
        .expect("chmod local-only git shim");

    let mut env = fixture.git_env.clone();
    env.insert("PATH".into(), shim_dir.into_os_string());
    env
}

#[cfg(unix)]
fn find_git_executable() -> PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").expect("PATH"))
        .map(|directory| directory.join("git"))
        .find(|candidate| candidate.is_file())
        .expect("git executable")
}

fn registered_branch(value: &str) -> outpost_core::ops::status::RegisteredOutpostHead {
    outpost_core::ops::status::RegisteredOutpostHead::Attached(branch(value))
}

fn colliding_names(fixture: &AbcFixture) -> (String, String) {
    let source = canonical(&fixture.source);
    let root = canonical(&fixture.root);
    let mut prefixes = std::collections::HashMap::new();
    for index in 0..10_000 {
        let name = format!("collision-{index}");
        let id = OutpostId::derive(&source, &root.join(&name));
        let prefix = id.as_str()[..5].to_owned();
        if let Some(previous) = prefixes.insert(prefix, name.clone()) {
            return (previous, name);
        }
    }
    panic!("expected a five-character ID collision");
}

fn duplicate_first_registry_entry(fixture: &AbcFixture) {
    let path = fixture.source.join(".outpost/registry.json");
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("registry contents"))
            .expect("registry JSON");
    let outposts = value["outposts"].as_array_mut().expect("outposts array");
    outposts.push(outposts[0].clone());
    fs::write(
        path,
        serde_json::to_string_pretty(&value).expect("serialize registry"),
    )
    .expect("write duplicate registry");
}

fn edit_first_registry_entry(fixture: &AbcFixture, edit: impl FnOnce(&mut serde_json::Value)) {
    let path = fixture.source.join(".outpost/registry.json");
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("registry contents"))
            .expect("registry JSON");
    let outposts = value["outposts"].as_array_mut().expect("outposts array");
    edit(&mut outposts[0]);
    fs::write(
        path,
        serde_json::to_string_pretty(&value).expect("serialize registry"),
    )
    .expect("write edited registry");
}

fn write_source_config(fixture: &AbcFixture, contents: &str) {
    let directory = fixture.source.join(".outpost");
    fs::create_dir_all(&directory).expect("create outpost config directory");
    fs::write(directory.join("config.json"), contents).expect("write source config");
}

fn assert_integrity(fixture: &AbcFixture, outpost: &Path) {
    let error = expect_error(
        run_with(&fixture.source, &fixture.git_env),
        "contradiction should be an integrity error",
    );
    assert_eq!(error.exit_code(), 6);
    assert!(matches!(
        error,
        OutpostError::RegisteredOutpostIntegrity { source, outpost: actual }
            if source == canonical(&fixture.source) && actual == canonical(outpost)
    ));
}

fn refs_snapshot(fixture: &AbcFixture, repo: &Path) -> String {
    fixture
        .invoker(repo)
        .run_capture([
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            "refs/heads",
            "refs/remotes",
        ])
        .expect("refs snapshot")
}

fn expect_error<T>(result: outpost_core::OutpostResult<T>, message: &str) -> OutpostError {
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
