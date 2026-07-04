use std::path::PathBuf;

use crate::selector::{OutpostSelector, resolve_live_entry};
use crate::{OutpostResult, SourceRepo};

pub enum PathTarget {
    Source,
    Outpost(OutpostSelector),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathReport {
    pub path: PathBuf,
}

pub fn run(source: &SourceRepo, target: PathTarget) -> OutpostResult<PathReport> {
    let path = match target {
        PathTarget::Source => source.work_tree().to_path_buf(),
        PathTarget::Outpost(selector) => resolve_live_entry(source, &selector)?.path,
    };

    Ok(PathReport { path })
}
