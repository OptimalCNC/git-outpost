#[allow(dead_code)]
mod common;

#[cfg(unix)]
use std::collections::BTreeSet;
#[cfg(unix)]
use std::ffi::OsString;
use std::fs;
use std::path::Path;

use common::fixture::AbcFixture;
use outpost_core::ops::add::{AddCheckout, AddOptions, run as add_run};
use outpost_core::{OutpostError, RemoteName, SourceRepo};

#[cfg(unix)]
#[test]
fn registry_publication_preserves_a_directory_obstruction_and_registered_work() {
    let fixture = AbcFixture::new();
    let outpost = fixture.add_outpost("C").expect("add outpost");
    let work = outpost.join("work.txt");
    fs::write(&work, "uncommitted checkout contents").expect("outpost work");
    let source = fixture.source_repo().expect("source repo");
    let registry = source.registry().expect("load registry snapshot");
    let registry_path = source.registry_path();
    let registry_before = fs::read(&registry_path).expect("original registry");
    let state_dir = registry_path.parent().expect("registry parent");
    let backup = state_dir.join("registry.saved.json");
    fs::rename(&registry_path, &backup).expect("move registry document");
    fs::create_dir(&registry_path).expect("replace registry path with directory");
    let marker = registry_path.join("keep.txt");
    fs::write(&marker, "external directory contents").expect("directory contents");
    let state_before = directory_names(state_dir);

    let error = registry
        .save()
        .expect_err("directory must block atomic publication");

    assert!(matches!(
        error,
        OutpostError::IoAt { path, source }
            if path == registry_path && source.kind() == std::io::ErrorKind::IsADirectory
    ));
    assert_eq!(fs::read(&backup).expect("saved registry"), registry_before);
    assert_eq!(
        fs::read_to_string(&marker).expect("preserved directory contents"),
        "external directory contents"
    );
    assert_eq!(
        fs::read_to_string(&work).expect("preserved outpost work"),
        "uncommitted checkout contents"
    );
    assert_eq!(directory_names(state_dir), state_before);
}

#[test]
fn add_metadata_failure_preserves_template_state_and_source_registration() {
    let fixture = AbcFixture::new();
    let initial_source = fixture.source_repo().expect("source repo");
    initial_source
        .registry()
        .expect("empty registry")
        .save()
        .expect("save empty registry");
    let registry_path = initial_source.registry_path();
    let registry_before = fs::read(&registry_path).expect("source registry before add");
    let config_path = initial_source.git_dir().join("config");
    let config_before = fs::read(&config_path).expect("source Git config before add");
    let refs_before = refs_snapshot(&fixture, &fixture.source);
    let source_head = fixture
        .rev_parse(&fixture.source, "HEAD")
        .expect("source HEAD");
    let template = fixture.root.join("git-template");
    fs::create_dir(&template).expect("template directory");
    let template_state = template.join("outpost");
    fs::write(&template_state, "user template state").expect("template state file");
    let mut env = fixture.git_env.clone();
    env.insert("GIT_TEMPLATE_DIR".into(), template.into_os_string());
    let source = SourceRepo::at_with(&fixture.source, &env).expect("source with template");
    let destination = fixture.root.join("C");
    let expected_state = fs::canonicalize(&fixture.root)
        .expect("canonical fixture root")
        .join("C")
        .join(".git")
        .join("outpost");
    let expected_metadata = expected_state.join("metadata.json");
    let mut reporter = common::fixture::CapturingReporter::default();

    let error = match add_run(
        &source,
        AddOptions {
            destination: destination.clone(),
            checkout: AddCheckout::CheckoutExisting {
                target_branch: None,
            },
            remote_name: RemoteName::parse("local").expect("remote name"),
        },
        &mut reporter,
    ) {
        Ok(_) => panic!("template state file must block metadata initialization"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        OutpostError::IoAt { path, source }
            if path == expected_metadata && source.kind() == std::io::ErrorKind::AlreadyExists
    ));
    assert_eq!(
        fs::read_to_string(&template_state).expect("original template state"),
        "user template state"
    );
    if destination.exists() {
        let partial_clone =
            SourceRepo::at_with(&destination, &env).expect("partial clone repository");
        assert_eq!(
            fs::read_to_string(partial_clone.git_dir().join("outpost"))
                .expect("copied template state"),
            "user template state"
        );
        assert_eq!(
            fixture
                .rev_parse(&destination, "HEAD")
                .expect("partial clone HEAD"),
            source_head
        );
    }
    assert_eq!(
        fs::read(&registry_path).expect("source registry after add"),
        registry_before
    );
    assert!(
        source
            .registry()
            .expect("source registry")
            .entries()
            .is_empty()
    );
    assert_eq!(
        fs::read(&config_path).expect("source Git config after add"),
        config_before
    );
    assert_eq!(refs_snapshot(&fixture, &fixture.source), refs_before);
}

#[cfg(unix)]
fn directory_names(path: &Path) -> BTreeSet<OsString> {
    fs::read_dir(path)
        .expect("state directory")
        .map(|entry| entry.expect("state entry").file_name())
        .collect()
}

fn refs_snapshot(fixture: &AbcFixture, repository: &Path) -> String {
    fixture
        .invoker(repository)
        .run_capture(["for-each-ref", "--format=%(refname) %(objectname)"])
        .expect("repository refs")
}

#[cfg(target_os = "linux")]
#[test]
fn add_cannot_record_a_source_path_that_metadata_json_cannot_represent() {
    use std::os::unix::ffi::OsStrExt;

    let mut fixture = AbcFixture::new();
    let source_path = fixture
        .root
        .join(std::ffi::OsStr::from_bytes(b"projet-\xe9"));
    fs::rename(&fixture.source, &source_path).expect("rename source checkout");
    fixture.source = source_path;
    let git = fixture.invoker(&fixture.source);
    git.run_check(["config", "receive.denyCurrentBranch", "refuse"])
        .expect("set source push policy");
    let metadata = fs::canonicalize(&fixture.root)
        .expect("fixture root")
        .join("C/.git/outpost/metadata.json");

    let error = fixture
        .add_outpost("C")
        .expect_err("unrepresentable source path");

    assert!(matches!(error, OutpostError::IoAt { path, .. } if path == metadata));
    assert!(!metadata.try_exists().expect("check unpublished metadata"));
    let source = fixture.source_repo().expect("source repository");
    assert!(source.registry().expect("registry").entries().is_empty());
    assert_eq!(
        git.run_capture(["config", "--get", "receive.denyCurrentBranch"])
            .expect("source push policy"),
        "refuse"
    );
}

// Unix can clone to a non-UTF-8 path, but JSON cannot represent that registry
// entry. Failed publication must preserve prior registrations and their work;
// config serialization and early storage obstructions do not prove this.
#[cfg(unix)]
#[test]
fn add_to_a_non_utf8_path_preserves_the_existing_registry_on_serialization_failure() {
    use std::os::unix::ffi::OsStringExt;

    let fixture = AbcFixture::new();
    let existing = fixture.add_outpost("existing").expect("registered outpost");
    fs::write(existing.join("draft.txt"), "existing uncommitted work\n").expect("existing work");
    let head = fixture.rev_parse(&existing, "HEAD").expect("existing HEAD");
    let source = fixture.source_repo().expect("source repo");
    let registry = fs::read(source.registry_path()).expect("registry before add");
    let entries = source
        .registry()
        .expect("registry entries")
        .entries()
        .to_vec();
    let state_dir = source.registry_path().parent().unwrap().to_path_buf();
    let names = directory_names(&state_dir);
    let refs = refs_snapshot(&fixture, &fixture.source);
    let destination = fixture
        .root
        .join(OsString::from_vec(b"checkout-\xff".to_vec()));
    let mut reporter = common::fixture::CapturingReporter::default();

    let error = match add_run(
        &source,
        AddOptions {
            destination,
            checkout: AddCheckout::CheckoutExisting {
                target_branch: None,
            },
            remote_name: RemoteName::parse("local").expect("remote name"),
        },
        &mut reporter,
    ) {
        Ok(_) => panic!("the registry cannot serialize a non-UTF-8 path"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        OutpostError::IoAt { path, .. } if path == source.registry_path()
    ));
    assert_eq!(fs::read(source.registry_path()).unwrap(), registry);
    assert_eq!(source.registry().unwrap().entries(), entries.as_slice());
    assert_eq!(directory_names(&state_dir), names);
    assert_eq!(refs_snapshot(&fixture, &fixture.source), refs);
    assert_eq!(fixture.rev_parse(&existing, "HEAD").unwrap(), head);
    assert_eq!(
        fs::read_to_string(existing.join("draft.txt")).unwrap(),
        "existing uncommitted work\n"
    );
}
