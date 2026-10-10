//! Writing files so a crash never leaves one half-written.

use std::fs;
use std::io;
use std::path::Path;

/// Replaces `path` with `contents` by writing a sibling file and renaming it
/// over the original.
pub(crate) fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    fs::write(&temporary, contents)?;
    fs::rename(&temporary, path)
}
