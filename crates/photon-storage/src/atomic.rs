//! Writing files so a crash or power loss never leaves one half-written.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Tells apart temporary files written at the same time, as when a delayed
/// save and the save at quit overlap.
static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

/// Creates `path` and ensures the directory is private on Unix systems.
pub(crate) fn ensure_private_dir(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists && path.is_dir() => {}
        Err(error) => return Err(error),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn create_private_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

fn create_temporary(path: &Path) -> io::Result<(PathBuf, File)> {
    for _ in 0..100 {
        let mut temporary = path.as_os_str().to_owned();
        temporary.push(format!(
            ".{}-{}.tmp",
            std::process::id(),
            NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
        ));
        let temporary = PathBuf::from(temporary);
        match create_private_file(&temporary) {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a private temporary profile file",
    ))
}

/// Replaces `path` with `contents` by writing a sibling file, flushing it to
/// disk, and renaming it over the original. Readers see the old contents or
/// the new, never part of either.
pub(crate) fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        ensure_private_dir(parent)?;
    }
    let (temporary, mut file) = create_temporary(path)?;
    let write_result = (|| {
        file.write_all(contents)?;
        file.sync_all()
    })();
    drop(file);
    let result = write_result.and_then(|()| fs::rename(&temporary, path));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_file_and_leaves_no_temporary_behind() {
        let directory = std::env::temp_dir().join(format!("photon-atomic-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("file.json");
        write(&path, b"first").unwrap();
        write(&path, b"second").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
        let leftovers = fs::read_dir(&directory).unwrap().count();
        fs::remove_dir_all(&directory).unwrap();
        assert_eq!(leftovers, 1);
    }
}
