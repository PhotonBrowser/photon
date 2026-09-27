use std::path::Path;

use crate::support::{invoke, with_progress};

pub(crate) fn run(root: &Path, verbose: bool) -> Result<(), String> {
    with_progress("Run Rust workspace tests", !verbose, || {
        invoke("cargo", &["test", "--workspace"], root, verbose)
    })
}
