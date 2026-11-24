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

#[cfg(test)]
mod tests {
    use super::*;

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
}
