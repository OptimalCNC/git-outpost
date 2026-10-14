#[allow(dead_code)]
mod common;

use std::fs;

use common::fixture::{AbcFixture, CapturingReporter};
use outpost_core::ops::branch_analysis::{
    BranchCleanupFinding, BranchCleanupSkipReason, analyze_branch_cleanup,
};
use outpost_core::ops::{list, lock, merge, pull, push, rebase};
use outpost_core::selector::OutpostSelector;
use outpost_core::{Outpost, SourceRemoteRef, SourceRepo};

// An explicit checkout path can name a missing parent. Preflight must preserve
// the source and avoid a partial clone; occupied destinations and failures after
// cloning are different contracts already tested elsewhere.
#[test]
fn add_with_a_missing_destination_parent_preserves_the_source() {
    use outpost_core::ops::add::{AddCheckout, AddOptions};
    use outpost_core::{OutpostError, RemoteName};

    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let parent = fixture.root.join("missing-parent");
    let destination = parent.join("C");
    let config_path = source.git_dir().join("config");
    let config = fs::read(&config_path).expect("source config");
    let refs = fixture
        .invoker(&fixture.source)
        .run_capture(["for-each-ref", "--format=%(refname) %(objectname)"])
        .expect("source refs");
    let mut reporter = CapturingReporter::default();

    let error = match outpost_core::ops::add::run(
        &source,
        AddOptions {
            destination: destination.clone(),
            checkout: AddCheckout::CheckoutExisting {
                target_branch: None,
            },
            remote_name: RemoteName::parse("local").expect("remote"),
        },
        &mut reporter,
    ) {
        Ok(_) => panic!("the destination parent must exist"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        OutpostError::IoAt { path, source: error }
            if path == parent && error.kind() == std::io::ErrorKind::NotFound
    ));
    assert!(reporter.steps.is_empty());
    assert!(!destination.exists());
    assert!(!parent.exists());
    assert!(!source.registry_path().exists());
    assert_eq!(fs::read(config_path).unwrap(), config);
    assert_eq!(
        fixture
            .invoker(&fixture.source)
            .run_capture(["for-each-ref", "--format=%(refname) %(objectname)"])
            .unwrap(),
        refs
    );
}

// An external move can invalidate a retained SourceRepo handle. Inspection must
// report unavailable source evidence rather than authorize cleanup; a deleted
// branch in an otherwise available source is already covered separately.
#[test]
fn cleanup_analysis_warns_when_a_retained_source_repository_was_relocated() {
    let fixture = AbcFixture::new();
    let branch = fixture.create_source_branch("feat").expect("source branch");
    let path = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("outpost");
    let source = fixture.source_repo().expect("source repository");
    let outpost = Outpost::at_with(&path, &fixture.git_env).expect("open outpost");
    let source_oid = source
        .branch_oid(&branch)
        .expect("source OID")
        .expect("source branch exists");
    let head = fixture.rev_parse(&path, "HEAD").expect("outpost HEAD");
    let metadata = fs::read(outpost.metadata_path()).expect("metadata");
    let registry = fs::read(source.registry_path()).expect("registry");
    let registry_relative = source
        .registry_path()
        .strip_prefix(source.work_tree())
        .expect("registry inside source")
        .to_path_buf();
    fs::write(path.join("notes.txt"), "work retained after source move\n")
        .expect("uncommitted work");
    let relocated = fixture.root.join("relocated-source");
    fs::rename(&fixture.source, &relocated).expect("external source relocation");

    let analysis = analyze_branch_cleanup(&source, &outpost, None);

    assert_eq!(analysis.candidate, None);
    assert_eq!(analysis.evidence, None);
    assert!(matches!(
        analysis.findings.as_slice(),
        [BranchCleanupFinding::Warning { branch: Some(found), message }]
            if found == &branch && message.starts_with("cannot inspect source branch: ")
    ));
    let reopened = SourceRepo::at_with(&relocated, &fixture.git_env)
        .expect("repository remains valid at its new location");
    assert_eq!(
        reopened.branch_oid(&branch).expect("retained branch"),
        Some(source_oid)
    );
    assert_eq!(
        fs::read(relocated.join(registry_relative)).unwrap(),
        registry
    );
    assert_eq!(fixture.rev_parse(&path, "HEAD").unwrap(), head);
    assert_eq!(fs::read(outpost.metadata_path()).unwrap(), metadata);
    assert_eq!(
        fs::read_to_string(path.join("notes.txt")).unwrap(),
        "work retained after source move\n"
    );
}

// Git's supported orphan checkout has a named branch without a HEAD commit.
// Cleanup must report missing commit evidence without treating it as detached
// or deleting the existing source branch with the same name.
#[test]
fn cleanup_analysis_warns_for_an_unborn_outpost_branch_with_valid_tracking() {
    let fixture = AbcFixture::new();
    let branch = fixture.create_source_branch("feat").expect("source branch");
    let path = fixture.add_outpost("C").expect("outpost on main");
    let source = fixture.source_repo().expect("source repository");
    let outpost = Outpost::at_with(&path, &fixture.git_env).expect("open outpost");
    fixture
        .invoker(&path)
        .run_check(["switch", "--orphan", branch.as_str()])
        .expect("create unborn outpost branch");
    fixture
        .invoker(&path)
        .run_check(["config", "branch.feat.remote", "local"])
        .expect("set tracking remote");
    fixture
        .invoker(&path)
        .run_check(["config", "branch.feat.merge", "refs/heads/feat"])
        .expect("set tracking branch");
    assert_eq!(
        outpost.current_branch().expect("named unborn branch"),
        branch
    );
    assert!(
        !fixture
            .invoker(&path)
            .run_status(["rev-parse", "--verify", "--quiet", "HEAD"])
            .expect("unborn HEAD status")
    );
    let source_oid = source
        .branch_oid(&branch)
        .expect("source OID")
        .expect("source branch exists");
    let registry = fs::read(source.registry_path()).expect("registry");
    let metadata = fs::read(outpost.metadata_path()).expect("metadata");
    fs::write(path.join("new-root.txt"), "new history in progress\n")
        .expect("uncommitted work on unborn branch");

    let analysis = analyze_branch_cleanup(&source, &outpost, None);

    assert_eq!(analysis.candidate, None);
    assert_eq!(analysis.evidence, None);
    assert!(matches!(
        analysis.findings.as_slice(),
        [BranchCleanupFinding::Warning { branch: Some(found), message }]
            if found == &branch && message.starts_with("cannot inspect outpost HEAD: ")
    ));
    assert_eq!(
        source.branch_oid(&branch).expect("preserved source branch"),
        Some(source_oid)
    );
    assert_eq!(
        outpost.current_branch().expect("preserved unborn branch"),
        branch
    );
    assert!(
        !fixture
            .invoker(&path)
            .run_status(["rev-parse", "--verify", "--quiet", "HEAD"])
            .expect("HEAD remains unborn")
    );
    assert_eq!(fs::read(source.registry_path()).unwrap(), registry);
    assert_eq!(fs::read(outpost.metadata_path()).unwrap(), metadata);
    assert_eq!(
        fs::read_to_string(path.join("new-root.txt")).unwrap(),
        "new history in progress\n"
    );
}

// A remote administrator can use symbolic-ref to point HEAD at an existing tag.
// Real Git then advertises a non-branch default identity. That is insufficient
// cleanup evidence, unlike the already-covered missing remote/default cases.
#[test]
fn cleanup_analysis_rejects_a_remote_default_identity_that_is_a_tag() {
    let fixture = AbcFixture::new();
    let branch = fixture.create_source_branch("feat").expect("source branch");
    fixture
        .push_source_branch(&branch)
        .expect("publish feature");
    let path = fixture
        .add_outpost_on_branch("C", Some(branch.clone()))
        .expect("outpost");
    let source = fixture.source_repo().expect("source repository");
    let outpost = Outpost::at_with(&path, &fixture.git_env).expect("open outpost");
    fixture
        .invoker(&fixture.upstream)
        .run_check(["tag", "release", "refs/heads/main"])
        .expect("remote tag");
    fixture
        .invoker(&fixture.upstream)
        .run_check(["symbolic-ref", "HEAD", "refs/tags/release"])
        .expect("remote HEAD targets a tag through Git's interface");
    let source_oid = source
        .branch_oid(&branch)
        .expect("source OID")
        .expect("source branch exists");
    let remote_oid = fixture
        .rev_parse(&fixture.upstream, "refs/heads/feat")
        .expect("upstream feature OID");
    let tag_oid = fixture
        .rev_parse(&fixture.upstream, "refs/tags/release")
        .expect("tag OID");
    let head = fixture.rev_parse(&path, "HEAD").expect("outpost HEAD");
    let registry = fs::read(source.registry_path()).expect("registry");
    let metadata = fs::read(outpost.metadata_path()).expect("metadata");

    let analysis = analyze_branch_cleanup(&source, &outpost, None);

    assert_eq!(analysis.candidate, None);
    assert_eq!(analysis.evidence, None);
    assert!(matches!(
        analysis.findings.as_slice(),
        [
            BranchCleanupFinding::Warning { branch: Some(found), message },
            BranchCleanupFinding::Skipped { branch: Some(skipped), reason: BranchCleanupSkipReason::DefaultBranchUnknown },
        ] if found == &branch && skipped == &branch
            && message.starts_with("cannot inspect upstream branches: ")
            && message.contains("default branch is not under refs/heads")
    ));
    assert_eq!(source.branch_oid(&branch).unwrap(), Some(source_oid));
    assert_eq!(
        fixture
            .rev_parse(&fixture.upstream, "refs/heads/feat")
            .unwrap(),
        remote_oid
    );
    assert_eq!(
        fixture
            .rev_parse(&fixture.upstream, "refs/tags/release")
            .unwrap(),
        tag_oid
    );
    assert_eq!(
        fixture
            .invoker(&fixture.upstream)
            .run_capture(["symbolic-ref", "HEAD"])
            .unwrap(),
        "refs/tags/release"
    );
    assert_eq!(fixture.rev_parse(&path, "HEAD").unwrap(), head);
    assert_eq!(fs::read(source.registry_path()).unwrap(), registry);
    assert_eq!(fs::read(outpost.metadata_path()).unwrap(), metadata);
}

// External replacement can leave a registered, locked path occupied by a file.
// Listing must distinguish that occupied path from Missing and retain its lock
// and data; neighboring list tests cover unmanaged directories instead.
#[test]
fn list_preserves_a_locked_registered_path_that_is_now_a_regular_file() {
    let fixture = AbcFixture::new();
    let path = fixture.add_outpost("C").expect("outpost");
    let source = fixture.source_repo().expect("source repository");
    lock::run(
        &source,
        lock::LockOptions {
            selector: OutpostSelector::from_path(path.clone()),
            reason: Some("retain this location".to_owned()),
        },
    )
    .expect("lock outpost before external replacement");
    let entries = source.registry().expect("registry").entries().to_vec();
    let registry = fs::read(source.registry_path()).expect("registry bytes");
    fs::remove_dir_all(&path).expect("external outpost removal");
    fs::write(&path, "user file now occupies this location\n").expect("replacement file");

    let summaries = list::run(&source).expect("list occupied registered path");

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].path, entries[0].path);
    assert_eq!(summaries[0].state, list::OutpostState::NotManaged);
    assert!(summaries[0].locked);
    assert_eq!(
        summaries[0].lock_reason.as_deref(),
        Some("retain this location")
    );
    assert_eq!(source.registry().unwrap().entries(), entries.as_slice());
    assert_eq!(fs::read(source.registry_path()).unwrap(), registry);
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "user file now occupies this location\n"
    );
}

// A retained Outpost handle can outlive an external directory relocation.
// Synchronization must preserve the filesystem failure rather than report
// missing tracking or change any repository; detached-HEAD and direct invoker
// tests do not establish each operation's error-remapping contract.
#[cfg(unix)]
#[test]
fn synchronization_preserves_io_failures_after_an_outpost_was_relocated() {
    let fixture = AbcFixture::new();
    let path = fixture.add_outpost("C").expect("outpost");
    let source = fixture.source_repo().expect("source repository");
    let outpost = Outpost::at_with(&path, &fixture.git_env).expect("open outpost");
    let path = outpost.work_tree().to_path_buf();
    fixture
        .commit_file_in_outpost(&path, "tracked work", "work.txt", "committed work\n")
        .expect("tracked work");
    fs::write(path.join("work.txt"), "uncommitted edits\n").expect("dirty tracked work");
    let head = fixture.rev_parse(&path, "HEAD").expect("outpost HEAD");
    let metadata = fs::read(outpost.metadata_path()).expect("metadata");
    let metadata_relative = outpost
        .metadata_path()
        .strip_prefix(&path)
        .expect("metadata inside outpost")
        .to_path_buf();
    let registry = fs::read(source.registry_path()).expect("registry");
    let source_refs = fixture
        .invoker(&fixture.source)
        .run_capture(["for-each-ref", "--format=%(refname) %(objectname)"])
        .expect("source refs");
    let upstream_refs = fixture
        .invoker(&fixture.upstream)
        .run_capture(["for-each-ref", "--format=%(refname) %(objectname)"])
        .expect("upstream refs");
    let relocated = fixture.root.join("relocated-outpost");
    fs::rename(&path, &relocated).expect("external outpost relocation");

    for operation in ["merge", "rebase", "pull", "push"] {
        let mut reporter = CapturingReporter::default();
        let result = match operation {
            "merge" => merge::run(
                &outpost,
                merge::MergeOptions {
                    source_ref: SourceRemoteRef::parse("local/main").expect("source ref"),
                },
                &mut reporter,
            )
            .map(|_| ()),
            "rebase" => rebase::run(
                &outpost,
                rebase::RebaseOptions {
                    source_ref: SourceRemoteRef::parse("local/main").expect("source ref"),
                },
                &mut reporter,
            )
            .map(|_| ()),
            "pull" => pull::run(&outpost, pull::PullOptions, &mut reporter).map(|_| ()),
            "push" => push::run(&outpost, push::PushOptions, &mut reporter).map(|_| ()),
            _ => unreachable!("the operation list is fixed above"),
        };
        let error = result.expect_err("retained handle points at a missing directory");

        assert!(
            matches!(
                error,
                outpost_core::OutpostError::IoAt { path: failed_path, source }
                    if failed_path == path && source.kind() == std::io::ErrorKind::NotFound
            ),
            "{operation} must preserve the missing-cwd I/O failure"
        );
        // Reporter steps describe user-visible work about to run. This failure
        // occurs before synchronization begins, so none should be announced.
        assert!(reporter.steps.is_empty(), "{operation}");
        assert!(reporter.warnings.is_empty(), "{operation}");
        assert_eq!(
            fixture
                .invoker(&fixture.source)
                .run_capture(["for-each-ref", "--format=%(refname) %(objectname)"])
                .unwrap(),
            source_refs,
            "{operation} source refs"
        );
        assert_eq!(
            fixture
                .invoker(&fixture.upstream)
                .run_capture(["for-each-ref", "--format=%(refname) %(objectname)"])
                .unwrap(),
            upstream_refs,
            "{operation} upstream refs"
        );
        assert_eq!(
            fs::read(source.registry_path()).unwrap(),
            registry,
            "{operation}"
        );
        assert_eq!(
            fixture.rev_parse(&relocated, "HEAD").unwrap(),
            head,
            "{operation}"
        );
        assert_eq!(
            fs::read(relocated.join(&metadata_relative)).unwrap(),
            metadata,
            "{operation}"
        );
        assert_eq!(
            fs::read_to_string(relocated.join("work.txt")).unwrap(),
            "uncommitted edits\n",
            "{operation}"
        );
        assert!(
            !path.exists(),
            "{operation} must not recreate the old location"
        );
    }
}

// An external move followed by a convenience symlink leaves a valid same-source
// repository at a different canonical identity. Listing must not advertise that
// target as the registered checkout; a regular-file replacement fails earlier.
#[cfg(unix)]
#[test]
fn list_preserves_a_moved_checkout_without_following_its_registered_symlink_alias() {
    use std::os::unix::fs::symlink;

    let fixture = AbcFixture::new();
    let path = fixture.add_outpost("C").expect("outpost");
    let source = fixture.source_repo().expect("source repository");
    lock::run(
        &source,
        lock::LockOptions {
            selector: OutpostSelector::from_path(path.clone()),
            reason: Some("retain work".to_owned()),
        },
    )
    .expect("lock original checkout");
    let registry = fs::read(source.registry_path()).expect("registry");
    let head = fixture.rev_parse(&path, "HEAD").expect("checkout HEAD");
    let original = Outpost::at_with(&path, &fixture.git_env).expect("original outpost");
    let registered_path = original.work_tree().to_path_buf();
    let metadata = fs::read(original.metadata_path()).expect("metadata");
    fs::write(
        path.join("draft.txt"),
        "work retained after external move\n",
    )
    .expect("outpost work");
    let moved = fixture.root.join("moved-checkout");
    fs::rename(&path, &moved).expect("external checkout move");
    symlink(&moved, &path).expect("convenience alias at the registered path");

    let summaries = list::run(&source).expect("list stale canonical identity");

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].state, list::OutpostState::NotManaged);
    assert_eq!(summaries[0].path, registered_path);
    assert!(summaries[0].locked);
    assert_eq!(summaries[0].lock_reason.as_deref(), Some("retain work"));
    assert_eq!(fs::read_link(&path).unwrap(), moved);
    assert_eq!(fs::read(source.registry_path()).unwrap(), registry);
    let reopened =
        Outpost::at_with(&moved, &fixture.git_env).expect("moved checkout remains valid");
    assert_eq!(fs::read(reopened.metadata_path()).unwrap(), metadata);
    assert_eq!(fixture.rev_parse(&moved, "HEAD").unwrap(), head);
    assert_eq!(
        fs::read_to_string(moved.join("draft.txt")).unwrap(),
        "work retained after external move\n"
    );
}

// Merging application-owned registry documents can duplicate a row. No hash
// collision is needed: listing must reject ambiguous IDs and preserve the
// document and checkout, beyond the pure helper's duplicate-ID rejection.
#[test]
fn list_rejects_duplicate_registry_rows_without_rewriting_state() {
    let fixture = AbcFixture::new();
    let path = fixture.add_outpost("C").expect("outpost");
    let source = fixture.source_repo().expect("source repository");
    let head = fixture.rev_parse(&path, "HEAD").expect("checkout HEAD");
    fs::write(path.join("draft.txt"), "uncommitted work\n").expect("outpost work");
    let mut document: serde_json::Value =
        serde_json::from_slice(&fs::read(source.registry_path()).expect("registry document"))
            .expect("registry JSON");
    let rows = document["outposts"].as_array_mut().expect("registry rows");
    rows.push(rows[0].clone());
    let bytes = serde_json::to_vec_pretty(&document).expect("duplicate-row document");
    fs::write(source.registry_path(), &bytes).expect("external registry merge");

    let error = list::run(&source).expect_err("duplicate identities cannot be listed");

    assert!(matches!(error,
        outpost_core::OutpostError::BadRegistry { path: failed_path, reason }
            if failed_path == source.registry_path()
                && reason == outpost_core::outpost_id::DuplicateOutpostIdError.to_string()
    ));
    assert_eq!(fs::read(source.registry_path()).unwrap(), bytes);
    assert_eq!(fixture.rev_parse(&path, "HEAD").unwrap(), head);
    assert_eq!(
        fs::read_to_string(path.join("draft.txt")).unwrap(),
        "uncommitted work\n"
    );
}

// A destination supplied through MoveOptions can exceed Linux's component
// limit. The operation must report the failure and leave the original outpost
// and registry intact even when force admits uncommitted work.
#[cfg(target_os = "linux")]
#[test]
fn failed_move_to_an_overlong_name_preserves_the_outpost_and_registry() {
    let fixture = AbcFixture::new();
    let path = fixture.add_outpost("C").expect("outpost");
    let source = fixture.source_repo().expect("source repository");
    let outpost = Outpost::at_with(&path, &fixture.git_env).expect("open outpost");
    fixture
        .commit_file_in_outpost(&path, "tracked work", "work.txt", "committed work\n")
        .expect("tracked work");
    fs::write(path.join("work.txt"), "uncommitted edits\n").expect("dirty tracked work");
    let head = fixture.rev_parse(&path, "HEAD").expect("outpost HEAD");
    let metadata = fs::read(outpost.metadata_path()).expect("metadata");
    let registry = fs::read(source.registry_path()).expect("registry");
    // Linux limits syscall pathnames to 4096 bytes including the terminating NUL,
    // independent of a mounted filesystem's component-length limit.
    let destination = fixture.root.join("d".repeat(4096));

    let error = outpost_core::ops::r#move::run(
        &source,
        outpost_core::ops::r#move::MoveOptions {
            selector: OutpostSelector::from_path(path.clone()),
            new_path: destination.clone(),
            force: true,
        },
    )
    .expect_err("the filesystem rejects the overlong destination component");

    assert!(matches!(
        error,
        outpost_core::OutpostError::IoAt { path: failed_path, source }
            if (failed_path == path || failed_path == destination)
                && source.kind() == std::io::ErrorKind::InvalidFilename
    ));
    assert!(path.is_dir());
    assert!(!destination.exists());
    assert_eq!(fixture.rev_parse(&path, "HEAD").unwrap(), head);
    assert_eq!(fs::read(outpost.metadata_path()).unwrap(), metadata);
    assert_eq!(fs::read(source.registry_path()).unwrap(), registry);
    assert_eq!(
        fs::read_to_string(path.join("work.txt")).unwrap(),
        "uncommitted edits\n"
    );
}
