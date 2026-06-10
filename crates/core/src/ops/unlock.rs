use std::path::PathBuf;

use crate::selector::{OutpostSelector, resolve_live_entry};
use crate::{OutpostResult, SourceRepo};

pub struct UnlockOptions {
    pub selector: OutpostSelector,
}

pub fn run(source: &SourceRepo, opts: UnlockOptions) -> OutpostResult<PathBuf> {
    let resolved = resolve_live_entry(source, &opts.selector)?;
    let mut registry = source.registry_mut()?;
    registry.unlock(&resolved.path)?;
    registry.save()?;
    Ok(resolved.path)
}
