pub mod error;
pub mod git;
pub mod metadata;
pub mod refname;
pub mod registry;
pub mod reporter;
pub mod source_repo;

pub use error::{OutpostError, OutpostResult};
pub use git::GitInvoker;
pub use metadata::{Metadata, RawMetadata};
pub use refname::{BranchName, RefName, RemoteName, SourceRemoteRef, UpstreamRef};
pub use registry::{Registry, RegistryEntry, RegistryMut};
pub use reporter::{Reporter, StepKind};
pub use source_repo::SourceRepo;
