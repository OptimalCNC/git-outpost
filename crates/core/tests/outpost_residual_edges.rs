use outpost_core::{Outpost, OutpostError};

#[test]
fn discover_propagates_a_missing_working_directory_io_error() {
    let temp = tempfile::tempdir().expect("tempdir");
    let missing = temp.path().join("missing-outpost");

    assert!(matches!(
        Outpost::discover(&missing)
            .err()
            .expect("missing cwd should fail"),
        OutpostError::IoAt { path, .. } if path == missing
    ));
}
