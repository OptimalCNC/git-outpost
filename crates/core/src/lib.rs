pub mod error;
pub mod git;
pub mod metadata;
pub mod outpost;
pub mod refname;
pub mod registry;
pub mod reporter;
pub mod safety;
pub mod source_repo;

pub use error::{OutpostError, OutpostResult};
pub use git::GitInvoker;
pub use metadata::{Metadata, RawMetadata};
pub use outpost::{AheadBehind, Outpost};
pub use refname::{BranchName, RefName, RemoteName, SourceRemoteRef, UpstreamRef};
pub use registry::{Registry, RegistryEntry, RegistryMut};
pub use reporter::{Reporter, StepKind};
pub use source_repo::SourceRepo;
