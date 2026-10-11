//! Writing files so a crash or power loss never leaves one half-written.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Tells apart temporary files written at the same time, as when a delayed
/// save and the save at quit overlap.
static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

/// Replaces `path` with `contents` by writing a sibling file, flushing it to
/// disk, and renaming it over the original. Readers see the old contents or
/// the new, never part of either.
pub(crate) fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(format!(
        ".{}-{}.tmp",
        std::process::id(),
        NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = File::create(&temporary)?;
        file.write_all(contents)?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
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
