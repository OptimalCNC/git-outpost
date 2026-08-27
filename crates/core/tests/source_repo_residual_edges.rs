#[allow(dead_code)]
mod common;

use std::fs;

use common::fixture::AbcFixture;
use outpost_core::OutpostError;

#[test]
fn set_outpost_container_propagates_an_invalid_container_error() {
    let fixture = AbcFixture::new();
    let source = fixture.source_repo().expect("open source");
    let file = fixture.root.join("not-a-container");
    fs::write(&file, "not a directory\n").expect("write non-directory target");

    assert!(matches!(
        source
            .set_outpost_container(&file)
            .expect_err("a file cannot be an outpost container"),
        OutpostError::InvalidConfigValue { key, value, .. }
            if key == "outpost-container" && value == file
    ));
}
