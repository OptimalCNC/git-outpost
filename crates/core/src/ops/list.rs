use std::path::PathBuf;

use crate::{AheadBehind, BranchName, OutpostError, OutpostResult, SourceRepo};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutpostSummary {
    pub path: PathBuf,
    pub current_branch: Option<BranchName>,
    pub state: OutpostState,
    pub ahead_behind: Option<AheadBehind>,
    pub locked: bool,
    pub lock_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutpostState {
    Clean,
    Dirty,
    Missing,
    NotManaged,
}

pub fn run(source: &SourceRepo) -> OutpostResult<Vec<OutpostSummary>> {
    source
        .registry()?
        .entries()
        .iter()
        .map(|entry| summarize_entry(source, entry))
        .collect()
}

fn summarize_entry(
    source: &SourceRepo,
    entry: &crate::RegistryEntry,
) -> OutpostResult<OutpostSummary> {
    let mut summary = OutpostSummary {
        path: entry.path.clone(),
        current_branch: None,
        state: OutpostState::Missing,
        ahead_behind: None,
        locked: entry.locked,
        lock_reason: entry.lock_reason.clone(),
    };

    if !entry.path.exists() {
        return Ok(summary);
    }

    let outpost = match source.outpost_at(&entry.path) {
        Ok(outpost) => outpost,
        Err(OutpostError::NotAnOutpost(_) | OutpostError::NotARepo(_)) => {
            summary.state = OutpostState::NotManaged;
            return Ok(summary);
        }
        Err(err) => return Err(err),
    };

    summary.current_branch = outpost.current_branch().ok();
    summary.state = if outpost.is_dirty()? {
        OutpostState::Dirty
    } else {
        OutpostState::Clean
    };

    Ok(summary)
}
