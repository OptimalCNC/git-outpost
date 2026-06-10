use std::fmt;
use std::path::Path;

use sha2::{Digest, Sha256};

const ID_LEN: usize = 64;
pub const MIN_PREFIX_LEN: usize = 5;
const DERIVED_OUTPOST_ID_HASH_NAMESPACE: &[u8] =
    b"git-outpost derived outpost id from source path and outpost path v1";

/// Deterministic display and selector alias for one registered outpost.
///
/// IDs are scoped to a single source registry and are derived from the source
/// path and outpost path. They are not stored; moving an outpost changes its
/// derived ID because the path is part of the outpost identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OutpostId(String);

/// Validated ID prefix accepted from human selectors.
///
/// Prefixes are accepted only when they are at least five hex characters and
/// uniquely identify one entry within the current source registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutpostIdPrefix(String);

impl OutpostId {
    pub fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.len() != ID_LEN {
            return Err(format!(
                "outpost id must be {ID_LEN} lowercase hex characters"
            ));
        }
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("outpost id must contain only lowercase hex characters".to_owned());
        }
        Ok(Self(value))
    }

    pub fn derive(source: &Path, outpost: &Path) -> Self {
        let mut hasher = Sha256::new();
        update_field(&mut hasher, DERIVED_OUTPOST_ID_HASH_NAMESPACE);
        update_field(&mut hasher, path_bytes(source).as_ref());
        update_field(&mut hasher, path_bytes(outpost).as_ref());
        Self(hex_lower(hasher.finalize().as_slice()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn starts_with(&self, prefix: &OutpostIdPrefix) -> bool {
        self.0.starts_with(prefix.as_str())
    }
}

impl OutpostIdPrefix {
    pub fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into().to_ascii_lowercase();
        if value.len() < MIN_PREFIX_LEN {
            return Err(format!(
                "outpost id prefix must be at least {MIN_PREFIX_LEN} hex characters"
            ));
        }
        if value.len() > ID_LEN {
            return Err(format!(
                "outpost id prefix must be at most {ID_LEN} hex characters"
            ));
        }
        if !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("outpost id prefix must contain only hex characters".to_owned());
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OutpostId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Display for OutpostIdPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

pub(crate) fn shortest_unique_prefixes<'a>(
    ids: impl IntoIterator<Item = &'a OutpostId>,
) -> Vec<String> {
    let ids = ids.into_iter().collect::<Vec<_>>();
    ids.iter()
        .map(|id| shortest_unique_prefix(id, &ids))
        .collect()
}

fn shortest_unique_prefix(id: &OutpostId, ids: &[&OutpostId]) -> String {
    for len in MIN_PREFIX_LEN..=ID_LEN {
        let prefix = &id.as_str()[..len];
        if ids
            .iter()
            .filter(|candidate| candidate.as_str().starts_with(prefix))
            .count()
            == 1
        {
            return prefix.to_owned();
        }
    }
    id.as_str().to_owned()
}

fn update_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update(value.len().to_le_bytes());
    hasher.update(value);
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> std::borrow::Cow<'_, [u8]> {
    use std::os::unix::ffi::OsStrExt;

    std::borrow::Cow::Borrowed(path.as_os_str().as_bytes())
}

#[cfg(not(unix))]
fn path_bytes(path: &Path) -> std::borrow::Cow<'_, [u8]> {
    path.to_string_lossy().as_bytes().to_vec().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortest_unique_prefixes_expand_from_minimum_only_when_needed() {
        let first =
            OutpostId::parse("abcde00000000000000000000000000000000000000000000000000000000000")
                .expect("first id");
        let second =
            OutpostId::parse("abcdf00000000000000000000000000000000000000000000000000000000000")
                .expect("second id");
        let third =
            OutpostId::parse("1234500000000000000000000000000000000000000000000000000000000000")
                .expect("third id");

        let prefixes = shortest_unique_prefixes([&first, &second, &third]);

        assert_eq!(prefixes, vec!["abcde", "abcdf", "12345"]);
    }

    #[test]
    fn shortest_unique_prefixes_expand_for_collision() {
        let first =
            OutpostId::parse("abcde00000000000000000000000000000000000000000000000000000000000")
                .expect("first id");
        let second =
            OutpostId::parse("abcde10000000000000000000000000000000000000000000000000000000000")
                .expect("second id");

        let prefixes = shortest_unique_prefixes([&first, &second]);

        assert_eq!(prefixes, vec!["abcde0", "abcde1"]);
    }
}
