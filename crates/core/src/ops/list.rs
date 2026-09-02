use std::fs;
use std::path::{Path, PathBuf};
use std::thread;

use crate::metadata::{self, MetadataState};
use crate::outpost_id::{OutpostId, shortest_unique_prefixes};
use crate::source_repo::{canonicalize_path, invoker_at};
use crate::{BranchName, OutpostResult, RegistryEntry, SourceRepo};

const MAX_LIST_WORKERS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutpostSummary {
    pub display_id: String,
    pub path: PathBuf,
    pub state: OutpostState,
    pub locked: bool,
    pub lock_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutpostState {
    Present { head_oid: String, head: OutpostHead },
    Missing,
    NotManaged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutpostHead {
    Attached(BranchName),
    Detached,
}

pub fn run(source: &SourceRepo) -> OutpostResult<Vec<OutpostSummary>> {
    let registry = source.registry()?;
    let ids = registry
        .entries()
        .iter()
        .map(|entry| OutpostId::derive(source.work_tree(), &entry.path))
        .collect::<Vec<_>>();
    let prefixes = shortest_unique_prefixes(ids.iter());
    let jobs = registry
        .entries()
        .iter()
        .cloned()
        .zip(prefixes)
        .collect::<Vec<_>>();

    Ok(summarize_entries(source, &jobs))
}

fn summarize_entries(source: &SourceRepo, jobs: &[(RegistryEntry, String)]) -> Vec<OutpostSummary> {
    if jobs.is_empty() {
        return Vec::new();
    }

    let workers = thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .min(MAX_LIST_WORKERS)
        .min(jobs.len());
    let chunk_size = jobs.len().div_ceil(workers);

    thread::scope(|scope| {
        let handles = jobs
            .chunks(chunk_size)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|(entry, display_id)| {
                            summarize_entry(source, entry, display_id.clone())
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>();

        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("list worker panicked"))
            .collect()
    })
}

fn summarize_entry(
    source: &SourceRepo,
    entry: &RegistryEntry,
    display_id: String,
) -> OutpostSummary {
    let state = match fs::metadata(&entry.path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => OutpostState::Missing,
        Ok(metadata) if metadata.is_dir() => {
            inspect_present_entry(source, entry).unwrap_or(OutpostState::NotManaged)
        }
        Ok(_) | Err(_) => OutpostState::NotManaged,
    };

    OutpostSummary {
        display_id,
        path: entry.path.clone(),
        state,
        locked: entry.locked,
        lock_reason: entry.lock_reason.clone(),
    }
}

fn inspect_present_entry(source: &SourceRepo, entry: &RegistryEntry) -> Option<OutpostState> {
    let git = invoker_at(&entry.path, source.env());
    let output = git
        .run_capture([
            "rev-parse",
            "--path-format=absolute",
            "--show-toplevel",
            "--absolute-git-dir",
            "HEAD",
            "--abbrev-ref",
            "HEAD",
        ])
        .ok()?;
    let snapshot = parse_snapshot(&output)?;
    let registered_path = canonicalize_path(&entry.path).ok()?;
    if registered_path != entry.path || snapshot.work_tree != registered_path {
        return None;
    }

    let metadata = match metadata::read(&snapshot.git_dir).ok()? {
        MetadataState::Valid(metadata) => metadata,
        MetadataState::Absent | MetadataState::Invalid(_) => return None,
    };
    let recorded_source = canonicalize_path(&metadata.source_repo).ok()?;
    if recorded_source != source.work_tree() || metadata.remote_name != entry.remote_name {
        return None;
    }

    Some(OutpostState::Present {
        head_oid: snapshot.head_oid,
        head: snapshot.head,
    })
}

struct RepositorySnapshot {
    work_tree: PathBuf,
    git_dir: PathBuf,
    head_oid: String,
    head: OutpostHead,
}

fn parse_snapshot(output: &str) -> Option<RepositorySnapshot> {
    let mut lines = output.lines();
    let work_tree = canonicalize_path(Path::new(lines.next()?)).ok()?;
    let git_dir = canonicalize_path(Path::new(lines.next()?)).ok()?;
    let head_oid = lines.next()?.to_owned();
    if !valid_object_id(&head_oid) {
        return None;
    }
    let head = match lines.next()? {
        "HEAD" => OutpostHead::Detached,
        branch => OutpostHead::Attached(BranchName::from_validated_git_output(branch.to_owned())),
    };
    if lines.next().is_some() {
        return None;
    }

    Some(RepositorySnapshot {
        work_tree,
        git_dir,
        head_oid,
        head,
    })
}

fn valid_object_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
