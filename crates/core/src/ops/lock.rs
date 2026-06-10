use std::path::PathBuf;

use crate::selector::{OutpostSelector, resolve_live_entry};
use crate::{OutpostResult, SourceRepo};

pub struct LockOptions {
    pub selector: OutpostSelector,
    pub reason: Option<String>,
}

pub fn run(source: &SourceRepo, opts: LockOptions) -> OutpostResult<PathBuf> {
    let resolved = resolve_live_entry(source, &opts.selector)?;
    let mut registry = source.registry_mut()?;
    registry.lock(&resolved.path, opts.reason)?;
    registry.save()?;
    Ok(resolved.path)
}
