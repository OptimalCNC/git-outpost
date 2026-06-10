use std::path::PathBuf;

use crate::outpost_id::{OutpostId, shortest_unique_prefixes};
use crate::{AheadBehind, BranchName, OutpostError, OutpostResult, SourceRepo};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutpostSummary {
    pub display_id: String,
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
    let registry = source.registry()?;
    let ids = registry
        .entries()
        .iter()
        .map(|entry| OutpostId::derive(source.work_tree(), &entry.path))
        .collect::<Vec<_>>();
    let prefixes = shortest_unique_prefixes(ids.iter());
    registry
        .entries()
        .iter()
        .zip(prefixes)
        .map(|(entry, display_id)| summarize_entry(source, entry, display_id))
        .collect()
}

fn summarize_entry(
    source: &SourceRepo,
    entry: &crate::RegistryEntry,
    display_id: String,
) -> OutpostResult<OutpostSummary> {
    let mut summary = OutpostSummary {
        display_id,
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

    let outpost = match crate::safety::check_entry_is_managed_outpost_of(source, entry) {
        Ok(outpost) => outpost,
        Err(OutpostError::RegistryEntryNotManaged(_)) => {
            summary.state = OutpostState::NotManaged;
            return Ok(summary);
        }
        Err(err) => return Err(err),
    };

    summary.current_branch = outpost.current_branch().ok();
    summary.ahead_behind = outpost.ahead_behind_source().ok();
    summary.state = if outpost.is_dirty()? {
        OutpostState::Dirty
    } else {
        OutpostState::Clean
    };

    Ok(summary)
}
