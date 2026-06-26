use std::path::PathBuf;

use thiserror::Error;

pub type OutpostResult<T> = Result<T, OutpostError>;

#[derive(Debug, Error)]
pub enum OutpostError {
    #[error("not inside a Git repository: {}", .0.display())]
    NotARepo(PathBuf),

    #[error("not inside a managed outpost: {}", .0.display())]
    NotAnOutpost(PathBuf),

    #[error("source repository not found at {}", .0.display())]
    SourceMissing(PathBuf),

    #[error("{command} must be run from {expected}; effective cwd is {}", .cwd.display())]
    WrongContext {
        command: &'static str,
        expected: &'static str,
        cwd: PathBuf,
    },

    #[error("{command} requires <outpost> when run from source repository {}", .cwd.display())]
    MissingOutpostPath { command: &'static str, cwd: PathBuf },

    #[error("unknown config key: {key}")]
    UnknownConfigKey { key: String },

    #[error("config key is unset: {key}")]
    ConfigKeyUnset { key: String },

    #[error("invalid config value for {key}: {} ({reason})", .value.display())]
    InvalidConfigValue {
        key: String,
        value: PathBuf,
        reason: String,
    },

    #[error("destination already exists: {}", .0.display())]
    DestinationExists(PathBuf),

    #[error("destination {} is inside an existing Git repository", .0.display())]
    DestinationInsideRepo(PathBuf),

    #[error("working tree is dirty in {}; {hint}", .repo.display())]
    DirtyTree { repo: PathBuf, hint: &'static str },

    #[error("branch {branch} has unpushed commits in {}; {hint}", .repo.display())]
    UnpushedCommits {
        repo: PathBuf,
        branch: String,
        hint: &'static str,
    },

    #[error("history diverges from source repository on branch {branch}")]
    Divergence { branch: String },

    #[error("branch not found: {branch} in {}", .repo.display())]
    BranchNotFound { branch: String, repo: PathBuf },

    #[error("no upstream tracking configured for branch {branch}")]
    NoUpstreamTracking { branch: String },

    #[error(
        "upstream is not a branch ref (got {merge_ref}); cannot synchronize from a non-branch upstream"
    )]
    UpstreamNotABranch { merge_ref: String },

    #[error("invalid ref name: {name}")]
    InvalidRefName { name: String },

    #[error(
        "source repository {} has {branch} checked out; cannot push to a non-bare checked-out branch (configure receive.denyCurrentBranch=updateInstead on the source, or check out a different branch in the source)",
        .r#source.display()
    )]
    PushIntoCheckedOutBranch { r#source: PathBuf, branch: String },

    #[error("branch {branch} does not exist on the source repository")]
    AmbiguousBranchCreation { branch: String },

    #[error("outpost is locked: {}{reason}", .path.display())]
    OutpostLocked { path: PathBuf, reason: String },

    #[error("registry entry path is not a managed outpost of this source: {}", .0.display())]
    RegistryEntryNotManaged(PathBuf),

    #[error("registry entry not found: {}", .0.display())]
    RegistryEntryNotFound(PathBuf),

    #[error("outpost id prefix not found: {0}")]
    OutpostIdPrefixNotFound(String),

    #[error("outpost id prefix is ambiguous: {0}")]
    OutpostIdPrefixAmbiguous(String),

    #[error("outpost selector is ambiguous: {0}")]
    OutpostSelectorAmbiguous(String),

    #[error("invalid registry file at {}: {reason}", .path.display())]
    BadRegistry { path: PathBuf, reason: String },

    #[error("invalid config file at {}: {reason}", .path.display())]
    BadConfig { path: PathBuf, reason: String },

    #[error("invalid outpost metadata at {}: {reason}", .outpost.display())]
    BadMetadata { outpost: PathBuf, reason: String },

    #[error("git command failed: `git {args}` (exit {code}): {stderr}")]
    GitFailed {
        args: String,
        code: i32,
        stderr: String,
    },

    #[error("git command terminated by signal: `git {args}`{signal_str}")]
    GitTerminatedBySignal { args: String, signal_str: String },

    #[error("io error at {}: {source}", .path.display())]
    IoAt {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl OutpostError {
    pub fn exit_code(&self) -> u8 {
        use OutpostError::*;
        match self {
            NotARepo(_)
            | NotAnOutpost(_)
            | SourceMissing(_)
            | WrongContext { .. }
            | MissingOutpostPath { .. }
            | UnknownConfigKey { .. }
            | ConfigKeyUnset { .. } => 2,
            DestinationExists(_)
            | DestinationInsideRepo(_)
            | DirtyTree { .. }
            | UnpushedCommits { .. }
            | OutpostLocked { .. } => 3,
            Divergence { .. }
            | PushIntoCheckedOutBranch { .. }
            | AmbiguousBranchCreation { .. } => 4,
            BranchNotFound { .. }
            | NoUpstreamTracking { .. }
            | InvalidRefName { .. }
            | UpstreamNotABranch { .. } => 5,
            BadRegistry { .. }
            | BadMetadata { .. }
            | BadConfig { .. }
            | InvalidConfigValue { .. }
            | OutpostIdPrefixNotFound(_)
            | OutpostIdPrefixAmbiguous(_)
            | OutpostSelectorAmbiguous(_)
            | RegistryEntryNotManaged(_)
            | RegistryEntryNotFound(_) => 6,
            GitFailed { code, .. } => (*code).clamp(1, 125) as u8,
            GitTerminatedBySignal { .. } => 137,
            IoAt { .. } => 70,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(value: &str) -> PathBuf {
        PathBuf::from(value)
    }

    #[test]
    fn display_strings_match_snapshot() {
        let cases = [
            (
                OutpostError::NotARepo(path("/repo")),
                "not inside a Git repository: /repo",
            ),
            (
                OutpostError::NotAnOutpost(path("/outpost")),
                "not inside a managed outpost: /outpost",
            ),
            (
                OutpostError::SourceMissing(path("/source")),
                "source repository not found at /source",
            ),
            (
                OutpostError::WrongContext {
                    command: "pull",
                    expected: "a managed outpost",
                    cwd: path("/source"),
                },
                "pull must be run from a managed outpost; effective cwd is /source",
            ),
            (
                OutpostError::MissingOutpostPath {
                    command: "lock",
                    cwd: path("/source"),
                },
                "lock requires <outpost> when run from source repository /source",
            ),
            (
                OutpostError::UnknownConfigKey {
                    key: "unknown".to_owned(),
                },
                "unknown config key: unknown",
            ),
            (
                OutpostError::ConfigKeyUnset {
                    key: "outpost-container".to_owned(),
                },
                "config key is unset: outpost-container",
            ),
            (
                OutpostError::InvalidConfigValue {
                    key: "outpost-container".to_owned(),
                    value: path("/file"),
                    reason: "path is not an existing directory".to_owned(),
                },
                "invalid config value for outpost-container: /file (path is not an existing directory)",
            ),
            (
                OutpostError::DestinationExists(path("/dest")),
                "destination already exists: /dest",
            ),
            (
                OutpostError::DestinationInsideRepo(path("/dest")),
                "destination /dest is inside an existing Git repository",
            ),
            (
                OutpostError::DirtyTree {
                    repo: path("/repo"),
                    hint: "pass --force",
                },
                "working tree is dirty in /repo; pass --force",
            ),
            (
                OutpostError::UnpushedCommits {
                    repo: path("/repo"),
                    branch: "main".to_owned(),
                    hint: "push first",
                },
                "branch main has unpushed commits in /repo; push first",
            ),
            (
                OutpostError::Divergence {
                    branch: "main".to_owned(),
                },
                "history diverges from source repository on branch main",
            ),
            (
                OutpostError::BranchNotFound {
                    branch: "feature".to_owned(),
                    repo: path("/repo"),
                },
                "branch not found: feature in /repo",
            ),
            (
                OutpostError::NoUpstreamTracking {
                    branch: "feature".to_owned(),
                },
                "no upstream tracking configured for branch feature",
            ),
            (
                OutpostError::UpstreamNotABranch {
                    merge_ref: "refs/tags/v1".to_owned(),
                },
                "upstream is not a branch ref (got refs/tags/v1); cannot synchronize from a non-branch upstream",
            ),
            (
                OutpostError::InvalidRefName {
                    name: "-evil".to_owned(),
                },
                "invalid ref name: -evil",
            ),
            (
                OutpostError::PushIntoCheckedOutBranch {
                    source: path("/source"),
                    branch: "main".to_owned(),
                },
                "source repository /source has main checked out; cannot push to a non-bare checked-out branch (configure receive.denyCurrentBranch=updateInstead on the source, or check out a different branch in the source)",
            ),
            (
                OutpostError::AmbiguousBranchCreation {
                    branch: "feature".to_owned(),
                },
                "branch feature does not exist on the source repository",
            ),
            (
                OutpostError::OutpostLocked {
                    path: path("/outpost"),
                    reason: ": release".to_owned(),
                },
                "outpost is locked: /outpost: release",
            ),
            (
                OutpostError::RegistryEntryNotManaged(path("/outpost")),
                "registry entry path is not a managed outpost of this source: /outpost",
            ),
            (
                OutpostError::RegistryEntryNotFound(path("/missing")),
                "registry entry not found: /missing",
            ),
            (
                OutpostError::OutpostIdPrefixNotFound("abcde".to_owned()),
                "outpost id prefix not found: abcde",
            ),
            (
                OutpostError::OutpostIdPrefixAmbiguous("abcde".to_owned()),
                "outpost id prefix is ambiguous: abcde",
            ),
            (
                OutpostError::OutpostSelectorAmbiguous("abcde".to_owned()),
                "outpost selector is ambiguous: abcde",
            ),
            (
                OutpostError::BadRegistry {
                    path: path("/repo/.outpost/registry.json"),
                    reason: "invalid json".to_owned(),
                },
                "invalid registry file at /repo/.outpost/registry.json: invalid json",
            ),
            (
                OutpostError::BadConfig {
                    path: path("/repo/.outpost/config.json"),
                    reason: "invalid json".to_owned(),
                },
                "invalid config file at /repo/.outpost/config.json: invalid json",
            ),
            (
                OutpostError::BadMetadata {
                    outpost: path("/outpost"),
                    reason: "missing source".to_owned(),
                },
                "invalid outpost metadata at /outpost: missing source",
            ),
            (
                OutpostError::GitFailed {
                    args: "status --short".to_owned(),
                    code: 1,
                    stderr: "fatal".to_owned(),
                },
                "git command failed: `git status --short` (exit 1): fatal",
            ),
            (
                OutpostError::GitTerminatedBySignal {
                    args: "fetch".to_owned(),
                    signal_str: " (signal 9)".to_owned(),
                },
                "git command terminated by signal: `git fetch` (signal 9)",
            ),
            (
                OutpostError::IoAt {
                    path: path("/repo/.outpost/registry.json"),
                    source: std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
                },
                "io error at /repo/.outpost/registry.json: missing",
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn exit_code_maps_each_variant() {
        let cases = [
            (OutpostError::NotARepo(path("/repo")), 2),
            (OutpostError::NotAnOutpost(path("/outpost")), 2),
            (OutpostError::SourceMissing(path("/source")), 2),
            (
                OutpostError::WrongContext {
                    command: "pull",
                    expected: "a managed outpost",
                    cwd: path("/source"),
                },
                2,
            ),
            (
                OutpostError::MissingOutpostPath {
                    command: "lock",
                    cwd: path("/source"),
                },
                2,
            ),
            (
                OutpostError::UnknownConfigKey {
                    key: "unknown".to_owned(),
                },
                2,
            ),
            (
                OutpostError::ConfigKeyUnset {
                    key: "outpost-container".to_owned(),
                },
                2,
            ),
            (OutpostError::DestinationExists(path("/dest")), 3),
            (OutpostError::DestinationInsideRepo(path("/dest")), 3),
            (
                OutpostError::DirtyTree {
                    repo: path("/repo"),
                    hint: "pass --force",
                },
                3,
            ),
            (
                OutpostError::UnpushedCommits {
                    repo: path("/repo"),
                    branch: "main".to_owned(),
                    hint: "push first",
                },
                3,
            ),
            (
                OutpostError::Divergence {
                    branch: "main".to_owned(),
                },
                4,
            ),
            (
                OutpostError::BranchNotFound {
                    branch: "feature".to_owned(),
                    repo: path("/repo"),
                },
                5,
            ),
            (
                OutpostError::NoUpstreamTracking {
                    branch: "feature".to_owned(),
                },
                5,
            ),
            (
                OutpostError::UpstreamNotABranch {
                    merge_ref: "refs/tags/v1".to_owned(),
                },
                5,
            ),
            (
                OutpostError::InvalidRefName {
                    name: "-evil".to_owned(),
                },
                5,
            ),
            (
                OutpostError::PushIntoCheckedOutBranch {
                    source: path("/source"),
                    branch: "main".to_owned(),
                },
                4,
            ),
            (
                OutpostError::AmbiguousBranchCreation {
                    branch: "feature".to_owned(),
                },
                4,
            ),
            (
                OutpostError::OutpostLocked {
                    path: path("/outpost"),
                    reason: ": release".to_owned(),
                },
                3,
            ),
            (OutpostError::RegistryEntryNotManaged(path("/outpost")), 6),
            (OutpostError::RegistryEntryNotFound(path("/missing")), 6),
            (OutpostError::OutpostIdPrefixNotFound("abcde".to_owned()), 6),
            (
                OutpostError::OutpostIdPrefixAmbiguous("abcde".to_owned()),
                6,
            ),
            (
                OutpostError::OutpostSelectorAmbiguous("abcde".to_owned()),
                6,
            ),
            (
                OutpostError::BadRegistry {
                    path: path("/repo/.outpost/registry.json"),
                    reason: "invalid json".to_owned(),
                },
                6,
            ),
            (
                OutpostError::BadConfig {
                    path: path("/repo/.outpost/config.json"),
                    reason: "invalid json".to_owned(),
                },
                6,
            ),
            (
                OutpostError::InvalidConfigValue {
                    key: "outpost-container".to_owned(),
                    value: path("/file"),
                    reason: "path is not an existing directory".to_owned(),
                },
                6,
            ),
            (
                OutpostError::BadMetadata {
                    outpost: path("/outpost"),
                    reason: "missing source".to_owned(),
                },
                6,
            ),
            (
                OutpostError::GitFailed {
                    args: "status".to_owned(),
                    code: 42,
                    stderr: "fatal".to_owned(),
                },
                42,
            ),
            (
                OutpostError::GitTerminatedBySignal {
                    args: "fetch".to_owned(),
                    signal_str: " (signal 9)".to_owned(),
                },
                137,
            ),
            (
                OutpostError::IoAt {
                    path: path("/repo/.outpost/registry.json"),
                    source: std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
                },
                70,
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(error.exit_code(), expected);
        }

        assert_eq!(
            OutpostError::GitFailed {
                args: "status".to_owned(),
                code: -1,
                stderr: "fatal".to_owned(),
            }
            .exit_code(),
            1
        );
        assert_eq!(
            OutpostError::GitFailed {
                args: "status".to_owned(),
                code: 256,
                stderr: "fatal".to_owned(),
            }
            .exit_code(),
            125
        );
    }
}
