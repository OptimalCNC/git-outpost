use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub(crate) fn git_path(path: &Path) -> PathBuf {
    dunce::simplified(path).to_path_buf()
}

pub(crate) fn git_path_arg(path: &Path) -> OsString {
    git_path(path).into_os_string()
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn git_path_strips_windows_verbatim_disk_prefix_when_safe() {
        use std::path::Path;

        let path = Path::new(r"\\?\C:\Users\runneradmin\AppData\Local\Temp\outpost");

        assert_eq!(
            super::git_path(path),
            Path::new(r"C:\Users\runneradmin\AppData\Local\Temp\outpost")
        );
    }
}
