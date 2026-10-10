//! Runtime source readers keep guards on the React implementation, not just its HTML shell.

use std::fs;
use std::path::Path;

pub fn dashboard_file(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/web/dashboard/src")
        .join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// Include Views, ViewModels and their helpers, but never test fixtures or locale JSON.
pub fn dashboard_sources() -> String {
    fn collect(directory: &Path, output: &mut String) {
        let mut entries: Vec<_> = fs::read_dir(directory)
            .expect("dashboard source directory must exist")
            .map(|entry| {
                entry
                    .expect("dashboard source entry must be readable")
                    .path()
            })
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                if path
                    .file_name()
                    .is_some_and(|name| name == "__tests__" || name == "test")
                {
                    continue;
                }
                collect(&path, output);
            } else if matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("ts" | "tsx")
            ) {
                let name = path.file_name().unwrap().to_string_lossy();
                if !name.contains(".test.")
                    && !name.contains(".spec.")
                    && !name.contains(".type-test.")
                {
                    output.push_str(
                        &fs::read_to_string(&path).expect("dashboard source must be readable"),
                    );
                    output.push('\n');
                }
            }
        }
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/web/dashboard/src");
    let mut output = String::new();
    collect(&root, &mut output);
    assert!(!output.is_empty(), "dashboard source must not be empty");
    output
}

/// Fail closed when a named section moves instead of silently checking another section.
pub fn section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    source
        .split_once(start)
        .unwrap_or_else(|| panic!("source section start missing: {start}"))
        .1
        .split_once(end)
        .unwrap_or_else(|| panic!("source section end missing: {end}"))
        .0
}
