//! Count source lines in the Rust project.
//!
//! Run from this package with `cargo loc`.

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

const SOURCE_EXTENSIONS: &[&str] = &[
    "c", "cc", "cpp", "h", "hpp", "js", "jsx", "m", "mm", "py", "rs", "sh", "ts", "tsx", "zig",
];

const SKIPPED_DIRECTORIES: &[&str] = &[
    ".git",
    ".zig-cache",
    "node_modules",
    "target",
    "test-videos",
    "vendor",
    "zig-out",
    "zig-pkg",
];

const LARGE_FILE_THRESHOLD: usize = 1000;

#[derive(Default)]
struct Count {
    files: usize,
    lines: usize,
    non_blank: usize,
    large_files: Vec<PathBuf>,
}

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut counts = BTreeMap::<String, Count>::new();

    if let Err(error) = count_directory(&root, &mut counts) {
        eprintln!("Could not count {}: {error}", root.display());
        std::process::exit(1);
    }

    println!("Source lines under {}\n", root.display());
    println!(
        "{:<10} {:>8} {:>12} {:>12}",
        "Language", "Files", "Lines", "Non-blank"
    );
    println!("{:-<10} {:->8} {:->12} {:->12}", "", "", "", "");

    let mut total = Count::default();
    let mut total_large_files = 0usize;
    for (extension, count) in &counts {
        println!(
            "{:<10} {:>8} {:>12} {:>12}",
            extension, count.files, count.lines, count.non_blank
        );
        total.files += count.files;
        total.lines += count.lines;
        total.non_blank += count.non_blank;
        total_large_files += count.large_files.len();
    }

    println!("{:-<10} {:->8} {:->12} {:->12}", "", "", "", "");
    println!(
        "{:<10} {:>8} {:>12} {:>12}",
        "Total", total.files, total.lines, total.non_blank
    );
    println!();
    println!("Files with more than {LARGE_FILE_THRESHOLD} lines:");
    if total_large_files == 0 {
        println!("  (none)");
    } else {
        for count in counts.values() {
            for path in &count.large_files {
                if let Ok(relative) = path.strip_prefix(&root) {
                    println!("  {}", relative.display());
                } else {
                    println!("  {}", path.display());
                }
            }
        }
    }
}

fn count_directory(path: &Path, counts: &mut BTreeMap<String, Count>) -> io::Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            let name = entry.file_name();
            if !SKIPPED_DIRECTORIES.iter().any(|skipped| name == *skipped) {
                count_directory(&path, counts)?;
            }
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
            continue;
        };
        let extension = extension.to_ascii_lowercase();
        if !SOURCE_EXTENSIONS.contains(&extension.as_str()) {
            continue;
        }

        let bytes = fs::read(&path)?;
        let source = String::from_utf8_lossy(&bytes);
        let line_count = source.lines().count();
        let count = counts.entry(extension).or_default();
        count.files += 1;
        count.lines += line_count;
        if line_count > LARGE_FILE_THRESHOLD {
            count.large_files.push(path);
        }
        count.non_blank += source
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count();
    }

    Ok(())
}
