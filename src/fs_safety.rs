//! Safety checks shared by destructive filesystem builtins.
//!
//! `remove_dir_all` is intentionally powerful: it removes a whole tree without
//! confirmation. Two root spellings are refused before any filesystem access
//! because they are almost always mistakes and can destroy far more than the
//! caller intended:
//!
//! * a final `.` or `..` component (including `link/.`, `link/..`, and their
//!   trailing-separator forms), and
//! * the filesystem root `/` (and repeated-separator spellings such as `//`).
//!
//! Refusing these spellings does not make the builtin a sandbox: intermediate
//! symbolic links and check-then-act races remain caller concerns.
//!
//! `copy_file` also refuses a source and target that resolve to the same file.
//! Opening the target for writing would truncate the source before any bytes
//! are copied, so an accidental `copy_file(path, path)` silently destroys the
//! file. The check follows symbolic links and compares device/inode identity on
//! Unix, so hard links and symlink aliases are caught as well. It is a
//! best-effort guard against mistakes, not an atomic transaction: a caller can
//! still race the check, and non-Unix builds fall back to canonical paths.

use std::path::Path;

/// Why a recursive-removal root was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemovalRefusal {
    /// The final component is `.` or `..`.
    DotOrDotDot,
    /// The path names the filesystem root.
    FilesystemRoot,
}

impl RemovalRefusal {
    /// Stable explanation used in the `R0015` runtime message.
    pub fn message(self) -> &'static str {
        match self {
            RemovalRefusal::DotOrDotDot => {
                "refusing to remove a directory whose final component is `.` or `..`"
            }
            RemovalRefusal::FilesystemRoot => "refusing to remove the filesystem root",
        }
    }
}

/// Returns a refusal when `path` is a destructive recursive-removal root.
///
/// Trailing `/` separators are ignored. An empty path is not classified here;
/// the filesystem layer reports it as a normal `R0015` failure. The check is
/// lexical and intentionally does not resolve symbolic links.
pub fn refused_removal_root(path: &str) -> Option<RemovalRefusal> {
    if path.is_empty() {
        return None;
    }
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        // One or more separators and nothing else: the filesystem root.
        return Some(RemovalRefusal::FilesystemRoot);
    }
    let name = trimmed.rsplit('/').next().unwrap_or(trimmed);
    if name == "." || name == ".." {
        return Some(RemovalRefusal::DotOrDotDot);
    }
    None
}

/// Returns whether `source` and `target` currently resolve to the same file.
///
/// Both paths are followed through symbolic links. On Unix the comparison uses
/// the device and inode, which also catches hard links; other platforms fall
/// back to canonical paths. Any metadata failure returns `false` so the copy
/// itself can report the more useful error.
pub fn paths_refer_to_same_file(source: &str, target: &str) -> bool {
    paths_refer_to_same_file_paths(Path::new(source), Path::new(target))
}

/// Path-typed form of [`paths_refer_to_same_file`] for callers that already
/// hold `Path` values, such as the build-output guard.
pub fn paths_refer_to_same_file_paths(source: &Path, target: &Path) -> bool {
    let (Ok(source_metadata), Ok(target_metadata)) =
        (std::fs::metadata(source), std::fs::metadata(target))
    else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        source_metadata.dev() == target_metadata.dev()
            && source_metadata.ino() == target_metadata.ino()
    }
    #[cfg(not(unix))]
    {
        let _ = (source_metadata, target_metadata);
        match (std::fs::canonicalize(source), std::fs::canonicalize(target)) {
            (Ok(source_path), Ok(target_path)) => source_path == target_path,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn workspace(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "vantyx-fs-safety-{}-{}-{name}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("create fs-safety workspace");
        directory
    }

    #[test]
    fn allows_ordinary_roots() {
        for path in [
            "tree",
            "tree/",
            "tree//",
            "./tree",
            "/tmp/tree",
            "foo/.hidden",
            "foo/..bar",
            "..foo",
            "",
        ] {
            assert_eq!(
                refused_removal_root(path),
                None,
                "expected `{path}` to be allowed"
            );
        }
    }

    #[test]
    fn refuses_dot_and_dot_dot_final_components() {
        for path in [
            ".",
            "./",
            ".//",
            "..",
            "../",
            "..//",
            "tree/.",
            "tree/./",
            "tree/..",
            "tree/../",
            "tree/.//",
            "tree/..//",
            "/.",
            "/..",
            "/tree/.",
            "/tree/..",
        ] {
            assert_eq!(
                refused_removal_root(path),
                Some(RemovalRefusal::DotOrDotDot),
                "expected `{path}` to be refused"
            );
        }
    }

    #[test]
    fn refuses_the_filesystem_root() {
        for path in ["/", "//", "///"] {
            assert_eq!(
                refused_removal_root(path),
                Some(RemovalRefusal::FilesystemRoot),
                "expected `{path}` to be refused"
            );
        }
    }

    #[test]
    fn empty_paths_are_left_to_the_filesystem_layer() {
        assert_eq!(refused_removal_root(""), None);
    }

    #[test]
    fn messages_are_stable() {
        assert!(
            RemovalRefusal::DotOrDotDot
                .message()
                .contains("`.` or `..`")
        );
        assert!(
            RemovalRefusal::FilesystemRoot
                .message()
                .contains("filesystem root")
        );
    }

    #[test]
    fn detects_the_same_path_and_distinct_files() {
        let directory = workspace("same-path");
        let file = directory.join("file.txt");
        std::fs::write(&file, "contents").expect("write file");
        let other = directory.join("other.txt");
        std::fs::write(&other, "other").expect("write other");

        assert!(paths_refer_to_same_file(
            file.to_str().expect("utf-8 file"),
            file.to_str().expect("utf-8 file")
        ));
        assert!(!paths_refer_to_same_file(
            file.to_str().expect("utf-8 file"),
            other.to_str().expect("utf-8 other")
        ));
        assert!(!paths_refer_to_same_file(
            file.to_str().expect("utf-8 file"),
            directory
                .join("missing.txt")
                .to_str()
                .expect("utf-8 missing")
        ));
        let _ = std::fs::remove_dir_all(directory);
    }

    #[cfg(unix)]
    #[test]
    fn detects_symbolic_and_hard_link_aliases() {
        use std::os::unix::fs::symlink;

        let directory = workspace("aliases");
        let file = directory.join("file.txt");
        std::fs::write(&file, "contents").expect("write file");
        let symlink_path = directory.join("link.txt");
        symlink("file.txt", &symlink_path).expect("create symlink");
        let hard_link_path = directory.join("hard.txt");
        std::fs::hard_link(&file, &hard_link_path).expect("create hard link");

        let file_text = file.to_str().expect("utf-8 file");
        for alias in [&symlink_path, &hard_link_path] {
            assert!(
                paths_refer_to_same_file(file_text, alias.to_str().expect("utf-8 alias")),
                "{} should resolve to the source",
                alias.display()
            );
            assert!(
                paths_refer_to_same_file(alias.to_str().expect("utf-8 alias"), file_text),
                "the comparison should be symmetric"
            );
        }
        let _ = std::fs::remove_dir_all(directory);
    }
}
