#[allow(dead_code)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{TimeZone, Utc};
use common::fixture::AbcFixture;
use outpost_core::safety;
use outpost_core::{
    BranchName, Outpost, OutpostError, RefName, RegistryEntry, RemoteName, SourceRepo, UpstreamRef,
};

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("canonical path")
}

fn upstream_main() -> UpstreamRef {
    UpstreamRef {
        remote: RemoteName::parse("local").expect("remote"),
        merge_ref: RefName::parse("refs/heads/main").expect("branch ref"),
    }
}

fn entry(path: &Path) -> RegistryEntry {
    RegistryEntry {
        path: path.to_path_buf(),
        created_at: Utc.timestamp_opt(1, 0).single().expect("timestamp"),
        remote_name: RemoteName::parse("local").expect("remote"),
        locked: false,
        lock_reason: None,
        locked_at: None,
    }
}

fn write_registry(source: &SourceRepo, contents: &str) {
    let path = source.registry_path();
    fs::create_dir_all(path.parent().expect("registry parent")).expect("registry parent");
    fs::write(path, contents).expect("registry contents");
}

#[test]
fn registry_rejects_invalid_json_and_unsupported_version() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");

    write_registry(&source, "{not-json");
    let malformed = source
        .registry()
        .expect_err("malformed registry should fail");
    assert!(
        matches!(malformed, OutpostError::BadRegistry { path, .. } if path == source.registry_path())
    );

    write_registry(&source, r#"{"version":99,"outposts":[]}"#);
    let version = source
        .registry()
        .expect_err("unsupported version should fail");
    assert!(
        matches!(version, OutpostError::BadRegistry { path, reason } if path == source.registry_path() && reason == "unsupported registry version 99")
    );
}

#[test]
fn registry_rejects_malformed_entry_timestamp_and_remote() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let path = fixture.root.join("C");
    fs::create_dir_all(&path).expect("entry path");
    let timestamp =
        serde_json::to_string(&Utc.timestamp_opt(1, 0).single().unwrap()).expect("timestamp json");

    write_registry(
        &source,
        &format!(
            r#"{{"version":1,"outposts":[{{"path":{},"created_at":"not-a-time","remote_name":"local","locked":false,"lock_reason":null,"locked_at":null}}]}}"#,
            serde_json::to_string(&path).expect("path json")
        ),
    );
    let timestamp_error = source.registry().expect_err("bad timestamp should fail");
    assert!(
        matches!(timestamp_error, OutpostError::BadRegistry { path: error_path, .. } if error_path == source.registry_path())
    );

    write_registry(
        &source,
        &format!(
            r#"{{"version":1,"outposts":[{{"path":{},"created_at":{},"remote_name":"bad/name","locked":false,"lock_reason":null,"locked_at":null}}]}}"#,
            serde_json::to_string(&path).expect("path json"),
            timestamp
        ),
    );
    let remote_error = source.registry().expect_err("bad remote should fail");
    assert!(
        matches!(remote_error, OutpostError::BadRegistry { path: error_path, reason } if error_path == source.registry_path() && reason.contains("invalid ref name: bad/name"))
    );
}

#[test]
fn registry_entry_and_mutation_paths_are_canonicalized() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let parent = fixture.root.join("nested");
    let outpost = parent.join("C");
    fs::create_dir_all(&outpost).expect("outpost path");

    let entry = RegistryEntry::new(
        parent.join(".").join("C"),
        RemoteName::parse("local").unwrap(),
    )
    .expect("entry path canonicalizes");
    assert_eq!(entry.path, canonical(&outpost));

    let mut registry = source.registry_mut().expect("registry mut");
    registry
        .add(RegistryEntry {
            path: parent.join(".").join("C"),
            ..entry
        })
        .expect("add canonicalizes");
    assert_eq!(registry.entries()[0].path, canonical(&outpost));
    registry.save().expect("save registry");
    assert_eq!(
        source.registry().expect("reload registry").entries()[0].path,
        canonical(&outpost)
    );
}

#[test]
fn failed_registry_mutations_leave_persisted_entries_unchanged() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let source = fixture.source_repo().expect("source repo");
    let before = fs::read_to_string(source.registry_path()).expect("registry before");
    let missing = fixture.root.join("missing");
    let unknown = fixture.root.join("unknown");
    fs::create_dir(&unknown).expect("unknown path");
    let unknown_canonical = canonical(&unknown);

    let mut registry = source.registry_mut().expect("registry mut");
    let lock_error = registry
        .lock(&unknown, Some("reason".to_owned()))
        .expect_err("lock unregistered");
    assert!(
        matches!(lock_error, OutpostError::RegistryEntryNotFound(path) if path == unknown_canonical)
    );
    let unlock_error = registry.unlock(&unknown).expect_err("unlock unregistered");
    assert!(
        matches!(unlock_error, OutpostError::RegistryEntryNotFound(path) if path == unknown_canonical)
    );
    let update_error = registry
        .update_path(&unknown, outpost.clone())
        .expect_err("update unregistered");
    assert!(
        matches!(update_error, OutpostError::RegistryEntryNotFound(path) if path == unknown_canonical)
    );
    let add_error = registry
        .add(RegistryEntry {
            path: missing.clone(),
            ..entry(&missing)
        })
        .expect_err("add missing");
    assert!(
        matches!(add_error, OutpostError::IoAt { path, source } if path == missing && source.kind() == std::io::ErrorKind::NotFound)
    );
    assert!(
        !registry
            .remove_by_path(&unknown)
            .expect("remove unknown existing path")
    );
    assert_eq!(registry.entries().len(), 1);
    drop(registry);

    assert_eq!(
        fs::read_to_string(source.registry_path()).expect("registry after"),
        before
    );
}

#[test]
fn remove_by_path_can_remove_a_recorded_path_after_checkout_disappears() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("outpost");
    let source = fixture.source_repo().expect("source repo");
    let recorded = canonical(&outpost);
    fs::remove_dir_all(&outpost).expect("remove checkout");

    let mut registry = source.registry_mut().expect("registry mut");
    assert!(
        registry
            .remove_by_path(&recorded)
            .expect("remove recorded path")
    );
    registry.save().expect("save removal");
    assert!(source.registry().expect("reload").entries().is_empty());
}

#[test]
fn registry_save_preserves_local_exclude_and_is_idempotent() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("source repo");
    let exclude = source.local_exclude_path_for_tests();
    fs::create_dir_all(exclude.parent().expect("exclude parent")).expect("exclude parent");
    fs::write(&exclude, "keep-this").expect("seed exclude");

    source
        .registry()
        .expect("load empty registry")
        .save()
        .expect("first save");
    source
        .registry()
        .expect("reload registry")
        .save()
        .expect("second save");
    let contents = fs::read_to_string(exclude).expect("exclude contents");
    assert!(contents.lines().any(|line| line == "keep-this"));
    assert_eq!(
        contents
            .lines()
            .filter(|line| line.trim() == ".outpost/")
            .count(),
        1
    );
}

#[test]
fn clean_safety_accepts_clean_tree_and_rejects_dirty_tree() {
    let fixture = AbcFixture::new();
    let git = fixture.invoker(&fixture.source);
    safety::check_clean(&fixture.source, &git).expect("clean source");
    fs::write(fixture.source.join("untracked.txt"), "dirty").expect("dirty file");
    let error = safety::check_clean(&fixture.source, &git).expect_err("dirty source");
    assert!(
        matches!(error, OutpostError::DirtyTree { repo, hint } if repo == fixture.source && hint == "pass --force")
    );
}

#[test]
fn unpushed_safety_accepts_synced_outpost_and_rejects_local_commit() {
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("outpost");
    let source = fixture.source_repo().expect("source repo");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("outpost");

    safety::check_no_unpushed(&outpost, &source).expect("synced outpost");
    fixture
        .commit_in_outpost(&outpost_path, "local only")
        .expect("outpost commit");
    let error = safety::check_no_unpushed(&outpost, &source).expect_err("unpushed outpost");
    assert!(
        matches!(error, OutpostError::UnpushedCommits { repo, branch, hint } if repo == canonical(&outpost_path) && branch == "main" && hint == "pass --force")
    );
}

#[test]
fn divergence_safety_accepts_equal_ahead_only_and_behind_only_histories() {
    let branch = BranchName::parse("main").expect("branch");

    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("ahead").expect("ahead outpost");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("ahead outpost");
    fixture
        .commit_in_outpost(&outpost_path, "ahead")
        .expect("ahead commit");
    safety::check_no_divergence_after_fetch(&outpost, &branch, &upstream_main())
        .expect("ahead only");

    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("behind").expect("behind outpost");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("behind outpost");
    fixture.commit_in_source("behind").expect("source commit");
    safety::check_no_divergence(&outpost, &branch, &upstream_main()).expect("behind only");

    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("equal").expect("equal outpost");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("equal outpost");
    safety::check_no_divergence(&outpost, &branch, &upstream_main()).expect("equal histories");
}

#[test]
fn divergence_safety_rejects_diverged_history_and_invalid_upstreams() {
    let branch = BranchName::parse("main").expect("branch");
    let fixture = AbcFixture::new();
    let outpost_path = fixture.add_outpost("C").expect("outpost");
    let outpost = Outpost::at_with(&outpost_path, &fixture.git_env).expect("outpost");
    fixture
        .commit_in_source("source side")
        .expect("source commit");
    fixture
        .commit_in_outpost(&outpost_path, "outpost side")
        .expect("outpost commit");

    let error = safety::check_no_divergence(&outpost, &branch, &upstream_main())
        .expect_err("diverged history");
    assert!(matches!(error, OutpostError::Divergence { branch } if branch == "main"));

    let missing = UpstreamRef {
        remote: RemoteName::parse("local").unwrap(),
        merge_ref: RefName::parse("refs/heads/missing").unwrap(),
    };
    let missing_error = safety::check_no_divergence_after_fetch(&outpost, &branch, &missing)
        .expect_err("missing upstream");
    assert!(
        matches!(missing_error, OutpostError::BranchNotFound { branch, repo } if branch == "missing" && repo == canonical(&outpost_path))
    );

    let tag = UpstreamRef {
        remote: RemoteName::parse("local").unwrap(),
        merge_ref: RefName::parse("refs/tags/v1").unwrap(),
    };
    let tag_error =
        safety::check_no_divergence_after_fetch(&outpost, &branch, &tag).expect_err("tag upstream");
    assert!(
        matches!(tag_error, OutpostError::UpstreamNotABranch { merge_ref } if merge_ref == "refs/tags/v1")
    );
}

#[test]
fn destination_safety_matrix_has_expected_successes_and_errors() {
    let fixture = AbcFixture::new();
    let outside = fixture.root.join("outside");
    fs::create_dir_all(&outside).expect("outside dir");
    safety::check_destination_clean(&outside, Path::new("missing")).expect("missing destination");
    fs::create_dir(outside.join("empty")).expect("empty destination");
    safety::check_destination_clean(&outside, Path::new("empty")).expect("empty destination");

    fs::write(outside.join("file"), "content").expect("destination file");
    let file_error =
        safety::check_destination_clean(&outside, Path::new("file")).expect_err("file destination");
    assert!(
        matches!(file_error, OutpostError::DestinationExists(path) if path == PathBuf::from("file"))
    );
    fs::create_dir(outside.join("non-empty")).expect("non-empty destination");
    fs::write(outside.join("non-empty").join("child"), "content").expect("child");
    let dir_error = safety::check_destination_clean(&outside, Path::new("non-empty"))
        .expect_err("non-empty destination");
    assert!(
        matches!(dir_error, OutpostError::DestinationExists(path) if path == PathBuf::from("non-empty"))
    );

    let inside_error = safety::check_destination_clean(&fixture.source, Path::new("new-outpost"))
        .expect_err("unignored in-repo destination");
    assert!(
        matches!(inside_error, OutpostError::DestinationInsideRepo(path) if path == PathBuf::from("new-outpost"))
    );
    fs::write(fixture.source.join(".git/info/exclude"), "new-outpost/\n")
        .expect("ignore destination");
    safety::check_destination_clean(&fixture.source, Path::new("new-outpost"))
        .expect("ignored in-repo destination");
}
