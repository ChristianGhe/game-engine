//! `cargo xtask size-check` (spec 8.2): Rust files over 500 lines warn,
//! over 1,000 fail. Small files keep the code navigable for agents.

use std::fs;
use std::path::Path;

use crate::util;

pub const WARN_LINES: usize = 500;
pub const FAIL_LINES: usize = 1000;

pub fn check(root: &Path) -> Result<String, String> {
    let mut warnings = Vec::new();
    let mut failures = Vec::new();
    let mut files = 0;
    for dir in ["crates", "xtask"] {
        for file in util::walk(&root.join(dir)) {
            if file.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            files += 1;
            let lines = fs::read_to_string(&file).map_or(0, |t| t.lines().count());
            let path = util::rel(root, &file);
            if lines > FAIL_LINES {
                failures.push(format!(
                    "[E-SIZE] {path}: {lines} lines (max {FAIL_LINES}); split it"
                ));
            } else if lines > WARN_LINES {
                warnings.push(format!(
                    "[W-SIZE] {path}: {lines} lines (warn over {WARN_LINES})"
                ));
            }
        }
    }
    let mut report = warnings;
    if failures.is_empty() {
        report.push(format!(
            "size-check: {files} files within {FAIL_LINES} lines"
        ));
        Ok(report.join("\n"))
    } else {
        report.extend(failures);
        Err(report.join("\n"))
    }
}
