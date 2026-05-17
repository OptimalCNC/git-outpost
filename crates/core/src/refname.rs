use std::fmt;
use std::process::{Command, Stdio};

use crate::{OutpostError, OutpostResult};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BranchName(String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RefName(String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RemoteName(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRemoteRef {
    pub remote: RemoteName,
    pub branch: BranchName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamRef {
    pub remote: RemoteName,
    pub merge_ref: RefName,
}

impl BranchName {
    pub fn parse(name: impl Into<String>) -> OutpostResult<Self> {
        let name = name.into();
        reject_empty_or_leading_dash(&name)?;
        if git_check_ref_format(["check-ref-format", "--branch", name.as_str()]) {
            Ok(Self(name))
        } else {
            invalid_ref(name)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl RefName {
    pub fn parse(name: impl Into<String>) -> OutpostResult<Self> {
        let name = name.into();
        reject_empty_or_leading_dash(&name)?;
        if git_check_ref_format(["check-ref-format", name.as_str()]) {
            Ok(Self(name))
        } else {
            invalid_ref(name)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl RemoteName {
    pub fn parse(name: impl Into<String>) -> OutpostResult<Self> {
        let name = name.into();
        reject_empty_or_leading_dash(&name)?;
        if name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            Ok(Self(name))
        } else {
            invalid_ref(name)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl SourceRemoteRef {
    pub fn parse(value: impl Into<String>) -> OutpostResult<Self> {
        let value = value.into();
        let Some((remote, branch)) = value.split_once('/') else {
            return invalid_ref(value);
        };

        Ok(Self {
            remote: RemoteName::parse(remote.to_owned())?,
            branch: BranchName::parse(branch.to_owned())?,
        })
    }
}

impl UpstreamRef {
    pub fn short_branch(&self) -> Option<&str> {
        self.merge_ref.as_str().strip_prefix("refs/heads/")
    }
}

impl fmt::Display for BranchName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Display for RefName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Display for RemoteName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

fn reject_empty_or_leading_dash(name: &str) -> OutpostResult<()> {
    if name.is_empty() || name.starts_with('-') {
        invalid_ref(name.to_owned())
    } else {
        Ok(())
    }
}

fn invalid_ref<T>(name: String) -> OutpostResult<T> {
    Err(OutpostError::InvalidRefName { name })
}

fn git_check_ref_format<const N: usize>(args: [&str; N]) -> bool {
    Command::new("git")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_parse_rejects_leading_dash_and_accepts_feature_branch() {
        let err = BranchName::parse("-evil").expect_err("leading dash should be rejected");
        assert!(matches!(
            err,
            OutpostError::InvalidRefName { name } if name == "-evil"
        ));

        let branch = BranchName::parse("feature/foo").expect("feature branch should parse");
        assert_eq!(branch.as_str(), "feature/foo");
    }

    #[test]
    fn remote_parse_rejects_shell_like_value() {
        let err =
            RemoteName::parse("origin --upload-pack=evil").expect_err("spaces should be rejected");
        assert!(matches!(
            err,
            OutpostError::InvalidRefName { name } if name == "origin --upload-pack=evil"
        ));
    }

    #[test]
    fn ref_parse_uses_full_ref_validation() {
        let heads = RefName::parse("refs/heads/main").expect("full branch ref should parse");
        assert_eq!(heads.as_str(), "refs/heads/main");

        let err = RefName::parse("main").expect_err("bare branch is not a full ref");
        assert!(matches!(
            err,
            OutpostError::InvalidRefName { name } if name == "main"
        ));
    }

    #[test]
    fn source_remote_ref_parses_remote_and_branch() {
        let source_ref = SourceRemoteRef::parse("local/feature/foo").expect("source ref parses");
        assert_eq!(source_ref.remote.as_str(), "local");
        assert_eq!(source_ref.branch.as_str(), "feature/foo");

        let err = SourceRemoteRef::parse("feature").expect_err("missing slash is invalid");
        assert!(matches!(
            err,
            OutpostError::InvalidRefName { name } if name == "feature"
        ));
    }

    #[test]
    fn upstream_short_branch_returns_only_heads_refs() {
        let upstream = UpstreamRef {
            remote: RemoteName::parse("local").expect("remote parses"),
            merge_ref: RefName::parse("refs/heads/main").expect("head ref parses"),
        };
        assert_eq!(upstream.short_branch(), Some("main"));

        let tag = UpstreamRef {
            remote: RemoteName::parse("origin").expect("remote parses"),
            merge_ref: RefName::parse("refs/tags/v1.0").expect("tag ref parses"),
        };
        assert_eq!(tag.short_branch(), None);
    }
}
