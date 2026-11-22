use std::fs;
use std::io::Write;
use std::path::Path;

use serde::Serialize;

use crate::{OutpostError, OutpostResult};

#[derive(Debug, Clone, Copy)]
pub(crate) enum Publish {
    Replace,
    NoClobber,
}

/// Publish one pretty JSON document atomically in an Outpost state directory.
///
/// The document is encoded before the file system is touched. It is written
/// to a temporary file in `state_dir` and renamed over `path`, so any later
/// failure leaves the previous document in place. Every failure reports `path`.
pub(crate) fn publish_json(
    state_dir: &Path,
    path: &Path,
    value: &impl Serialize,
    mode: Publish,
) -> OutpostResult<()> {
    let mut contents = serde_json::to_vec_pretty(value).map_err(|source| OutpostError::IoAt {
        path: path.to_path_buf(),
        source: std::io::Error::other(source),
    })?;
    contents.push(b'\n');
    write_atomically(state_dir, path, &contents, mode).map_err(|source| OutpostError::IoAt {
        path: path.to_path_buf(),
        source,
    })
}

fn write_atomically(
    state_dir: &Path,
    path: &Path,
    contents: &[u8],
    mode: Publish,
) -> std::io::Result<()> {
    fs::create_dir_all(state_dir)?;
    let mut temp = tempfile::NamedTempFile::new_in(state_dir)?;
    temp.write_all(contents)?;
    match mode {
        Publish::Replace => temp.persist(path),
        Publish::NoClobber => temp.persist_noclobber(path),
    }
    .map_err(|error| error.error)?;
    Ok(())
}
