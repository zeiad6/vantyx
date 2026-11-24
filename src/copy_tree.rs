//! Recursive directory copying with explicit limits (ADR-0034).
//!
//! The walker never follows symbolic links below the source root, does not
//! preserve metadata, and copies entry names as raw bytes. Failures are fatal
//! in the language; this module returns a message for the interpreter to wrap
//! in `R0015`.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

pub const MAX_ENTRIES: u64 = 100_000;
pub const MAX_DEPTH: usize = 128;

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_entries: u64,
    pub max_depth: usize,
}

pub fn copy_dir_all(from: &str, to: &str) -> Result<(), String> {
    copy_dir_all_with_limits(
        from,
        to,
        Limits {
            max_entries: MAX_ENTRIES,
            max_depth: MAX_DEPTH,
        },
    )
}

pub(crate) fn copy_dir_all_with_limits(
    from: &str,
    to: &str,
    limits: Limits,
) -> Result<(), String> {
    if from.is_empty() {
        return Err("source path is empty".to_string());
    }
    let source = Path::new(from);
    let target = Path::new(to);

    let metadata = fs::metadata(source)
        .map_err(|error| format!("cannot copy directory `{from}`: {error}"))?;
    if !metadata.is_dir() {
        return Err(format!("cannot copy directory `{from}`: not a directory"));
    }

    let trimmed = to.trim_end_matches('/');
    let name = trimmed.rsplit('/').next().unwrap_or("");
    if name.is_empty() || name == "." || name == ".." {
        return Err(format!(
            "cannot copy directory `{from}` to `{to}`: target must name a new directory"
        ));
    }
    let parent = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    match fs::symlink_metadata(target) {
        Ok(_) => {
            return Err(format!(
                "cannot copy directory `{from}` to `{to}`: target already exists"
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "cannot inspect target `{to}` for directory copy: {error}"
            ));
        }
    }

    // Resolve the parent so `..` and symbolic links cannot hide a target that
    // lies inside the source tree. The target itself must not exist yet, so
    // resolving its parent plus the final name is the closest check available
    // without creating it first.
    let source_real = fs::canonicalize(source)
        .map_err(|error| format!("cannot resolve source `{from}`: {error}"))?;
    let parent_real = fs::canonicalize(parent).map_err(|error| {
        format!(
            "cannot resolve target parent `{}`: {error}",
            parent.display()
        )
    })?;
    let target_real = parent_real.join(name);
    if target_real.starts_with(&source_real) {
        return Err(format!(
            "cannot copy directory `{from}` to `{to}`: target is inside the source"
        ));
    }

    fs::create_dir(target)
        .map_err(|error| format!("cannot create target directory `{to}`: {error}"))?;
    let mut entries = 0_u64;
    copy_directory(source, target, 1, &mut entries, limits).map_err(|error| {
        format!("cannot copy directory `{from}` to `{to}`: {error}")
    })
}

fn copy_directory(
    source: &Path,
    target: &Path,
    depth: usize,
    entries: &mut u64,
    limits: Limits,
) -> Result<(), String> {
    let reader = fs::read_dir(source).map_err(|error| {
        format!("cannot read directory `{}`: {error}", source.display())
    })?;
    for entry in reader {
        let entry = entry.map_err(|error| {
            format!("cannot read directory `{}`: {error}", source.display())
        })?;
        *entries += 1;
        if *entries > limits.max_entries {
            return Err(format!(
                "directory tree exceeds {} entries",
                limits.max_entries
            ));
        }
        let child_source = entry.path();
        let child_target = target.join(entry.file_name());
        let file_type = entry.file_type().map_err(|error| {
            format!("cannot inspect `{}`: {error}", child_source.display())
        })?;
        if file_type.is_symlink() {
            return Err(format!(
                "symbolic link `{}` is not supported",
                child_source.display()
            ));
        }
        if file_type.is_dir() {
            if depth > limits.max_depth {
                return Err(format!(
                    "directory tree exceeds maximum depth {}",
                    limits.max_depth
                ));
            }
            fs::create_dir(&child_target).map_err(|error| {
                format!(
                    "cannot create directory `{}`: {error}",
                    child_target.display()
                )
            })?;
            copy_directory(&child_source, &child_target, depth + 1, entries, limits)?;
        } else if file_type.is_file() {
            copy_file(&child_source, &child_target)?;
        } else {
            return Err(format!(
                "`{}` is not a regular file or directory",
                child_source.display()
            ));
        }
    }
    Ok(())
}

fn copy_file(source: &Path, target: &Path) -> Result<(), String> {
    let mut input = fs::File::open(source)
        .map_err(|error| format!("cannot open `{}`: {error}", source.display()))?;
    let mut output = fs::File::create(target)
        .map_err(|error| format!("cannot create `{}`: {error}", target.display()))?;
    io::copy(&mut input, &mut output).map_err(|error| {
        format!(
            "cannot copy `{}` to `{}`: {error}",
            source.display(),
            target.display()
        )
    })?;
    output.flush().map_err(|error| {
        format!(
            "cannot copy `{}` to `{}`: {error}",
            source.display(),
            target.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn workspace(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "vantyx-copy-unit-{}-{}-{name}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("create copy workspace");
        directory
    }

    fn text(path: &Path) -> String {
        fs::read_to_string(path).expect("read copied text")
    }

    #[test]
    fn copies_a_tree_and_preserves_the_source() {
        let directory = workspace("roundtrip");
        let source = directory.join("tree");
        fs::create_dir_all(source.join("sub/empty")).expect("create source tree");
        fs::create_dir_all(source.join(".hidden")).expect("create hidden directory");
        fs::write(source.join("a.txt"), "alpha").expect("write a");
        fs::write(source.join("sub/b.bin"), [0_u8, 255, 1]).expect("write binary");
        fs::write(source.join(".hidden/dot"), "dot").expect("write dot");
        let target = directory.join("copy");

        copy_dir_all(
            source.to_str().expect("utf-8 source"),
            target.to_str().expect("utf-8 target"),
        )
        .expect("copy tree");

        assert_eq!(text(&target.join("a.txt")), "alpha");
        assert_eq!(fs::read(target.join("sub/b.bin")).expect("read binary"), [0, 255, 1]);
        assert_eq!(text(&target.join(".hidden/dot")), "dot");
        assert!(target.join("sub/empty").is_dir());
        assert_eq!(text(&source.join("a.txt")), "alpha");
        assert!(source.join("sub/b.bin").is_file());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn copies_bytes_only_without_preserving_permissions() {
        let directory = workspace("metadata");
        let source = directory.join("tree");
        fs::create_dir(&source).expect("create source");
        let file = source.join("script.sh");
        fs::write(&file, "echo hi\n").expect("write script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&file, fs::Permissions::from_mode(0o700))
                .expect("set permissions");
        }
        let target = directory.join("copy");
        copy_dir_all(
            source.to_str().expect("utf-8 source"),
            target.to_str().expect("utf-8 target"),
        )
        .expect("copy tree");
        assert_eq!(text(&target.join("script.sh")), "echo hi\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let source_mode = fs::metadata(&file).expect("source metadata").permissions().mode();
            let target_mode = fs::metadata(target.join("script.sh"))
                .expect("target metadata")
                .permissions()
                .mode();
            assert_ne!(source_mode, target_mode, "copy must not preserve the mode");
        }
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn rejects_invalid_sources_and_targets() {
        let directory = workspace("errors");
        let source = directory.join("tree");
        fs::create_dir(&source).expect("create source");
        fs::write(source.join("a.txt"), "a").expect("write file");
        fs::write(directory.join("file"), "x").expect("write file source");
        let source_text = source.to_str().expect("utf-8 source");
        let directory_text = directory.to_str().expect("utf-8 workspace");

        let missing = format!("{directory_text}/missing");
        let error = copy_dir_all(&missing, &format!("{directory_text}/copy"))
            .expect_err("missing source");
        assert!(error.contains("cannot copy directory"), "{error}");

        let error = copy_dir_all(&format!("{directory_text}/file"), &format!("{directory_text}/copy"))
            .expect_err("file source");
        assert!(error.contains("not a directory"), "{error}");

        fs::create_dir(directory.join("existing")).expect("create target");
        let error = copy_dir_all(source_text, &format!("{directory_text}/existing"))
            .expect_err("existing target");
        assert!(error.contains("already exists"), "{error}");

        let error = copy_dir_all(source_text, &format!("{directory_text}/missing/copy"))
            .expect_err("missing parent");
        assert!(error.contains("target parent"), "{error}");

        let error = copy_dir_all(source_text, &format!("{source_text}/inside"))
            .expect_err("target inside source");
        assert!(error.contains("inside the source"), "{error}");

        for target in ["", ".", "..", "/"] {
            let error = copy_dir_all(source_text, target).expect_err("invalid target name");
            assert!(
                error.contains("must name a new directory") || error.contains("already exists"),
                "target {target:?}: {error}"
            );
        }
        let _ = fs::remove_dir_all(directory);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_special_files() {
        use std::os::unix::net::UnixListener;

        let directory = workspace("special");
        let source = directory.join("tree");
        fs::create_dir(&source).expect("create special source");
        let _socket = UnixListener::bind(source.join("sock")).expect("bind unix socket");
        let error = copy_dir_all(
            source.to_str().expect("utf-8 source"),
            directory.join("copy").to_str().expect("utf-8 target"),
        )
        .expect_err("special file");
        assert!(error.contains("not a regular file or directory"), "{error}");
        let _ = fs::remove_dir_all(directory);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_inner_links_but_follows_a_linked_root() {
        use std::os::unix::fs::symlink;

        let directory = workspace("links");
        let real = directory.join("real");
        fs::create_dir_all(real.join("sub")).expect("create real tree");
        fs::write(real.join("sub/data.txt"), "data").expect("write data");
        symlink(&real, directory.join("root-link")).expect("create root link");
        symlink("missing", directory.join("dangling")).expect("create dangling link");
        let directory_text = directory.to_str().expect("utf-8 workspace");

        let target = directory.join("copy");
        copy_dir_all(&format!("{directory_text}/root-link"), target.to_str().expect("utf-8 target"))
            .expect("copy linked root");
        assert_eq!(text(&target.join("sub/data.txt")), "data");

        let linked = directory.join("linked");
        fs::create_dir(&linked).expect("create linked source");
        symlink("sub", linked.join("inner")).expect("create inner link");
        let error = copy_dir_all(
            linked.to_str().expect("utf-8 linked"),
            directory.join("copy-inner").to_str().expect("utf-8 target"),
        )
        .expect_err("inner link");
        assert!(error.contains("symbolic link"), "{error}");

        let error = copy_dir_all(
            directory.join("dangling").to_str().expect("utf-8 dangling"),
            directory.join("copy-dangling").to_str().expect("utf-8 target"),
        )
        .expect_err("dangling root");
        assert!(error.contains("cannot copy directory"), "{error}");
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn default_limits_are_pinned() {
        assert_eq!(MAX_ENTRIES, 100_000);
        assert_eq!(MAX_DEPTH, 128);
    }

    #[test]
    fn limits_are_enforced() {
        let directory = workspace("limits");
        let source = directory.join("tree");
        fs::create_dir(&source).expect("create source");
        for name in ["a", "b", "c"] {
            fs::write(source.join(name), "x").expect("write file");
        }
        let error = copy_dir_all_with_limits(
            source.to_str().expect("utf-8 source"),
            directory.join("copy").to_str().expect("utf-8 target"),
            Limits {
                max_entries: 2,
                max_depth: 8,
            },
        )
        .expect_err("entry cap");
        assert!(error.contains("exceeds 2 entries"), "{error}");

        let deep = directory.join("deep");
        let mut path = deep.clone();
        for index in 0..6 {
            path.push(format!("d{index}"));
        }
        fs::create_dir_all(&path).expect("create deep tree");
        let error = copy_dir_all_with_limits(
            deep.to_str().expect("utf-8 deep"),
            directory.join("copy-deep").to_str().expect("utf-8 target"),
            Limits {
                max_entries: 100,
                max_depth: 4,
            },
        )
        .expect_err("depth cap");
        assert!(error.contains("maximum depth 4"), "{error}");
        let _ = fs::remove_dir_all(directory);
    }
}
