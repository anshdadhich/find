use std::collections::HashMap;
use rayon::prelude::*;
use crate::index::store::IndexEntry;

const APP_EXTENSIONS: &[&str] = &["exe", "lnk", "msi", "appx", "msix"];
const APP_PATH_MARKERS: &[&str] = &[
    "\\program files\\", "\\program files (x86)\\",
    "\\start menu\\", "\\desktop\\", "\\appdata\\",
];

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub full_path: std::path::PathBuf,
    pub name: String,
    pub rank: u8,
    pub is_dir: bool,
}

pub fn search(
    entries: &[IndexEntry],
    names: &HashMap<u64, String>,
    parents: &HashMap<u64, u64>,
    drive_root: &str,
    query: &str,
    limit: usize,
    case_sensitive: bool,
    excluded_dirs: &[String],
) -> Vec<SearchResult> {
    if query.is_empty() {
        return Vec::new();
    }

    let q = if case_sensitive { query.to_string() } else { query.to_lowercase() };

    let mut results: Vec<SearchResult> = entries
        .par_iter()
        .filter_map(|entry| {
            let name_cmp = if case_sensitive { &entry.name_original } else { &entry.name_lower };

            let base_rank = if *name_cmp == q { 1u8 }
                else if name_cmp.starts_with(&q) { 2 }
                else if name_cmp.contains(q.as_str()) { 3 }
                else { return None; };

            let full_path = build_path(entry.file_ref, names, parents, drive_root, 0);

            // Check exclusions
            if !excluded_dirs.is_empty() {
                let path_lower = full_path.to_string_lossy().to_lowercase();
                for ex in excluded_dirs {
                    if path_lower.starts_with(ex.as_str()) {
                        return None;
                    }
                }
            }

            // Promote apps (exe/lnk in known app dirs) to rank 0
            let is_app = if base_rank <= 2 {
                let ext_is_app = entry.name_lower
                    .rsplit('.')
                    .next()
                    .map(|e| APP_EXTENSIONS.contains(&e))
                    .unwrap_or(false);
                if ext_is_app {
                    let path_lower = full_path.to_string_lossy().to_lowercase();
                    APP_PATH_MARKERS.iter().any(|m| path_lower.contains(m))
                } else {
                    false
                }
            } else {
                false
            };

            let rank = if is_app { 0 } else { base_rank };

            Some(SearchResult {
                full_path,
                name: entry.name_original.clone(),
                rank,
                is_dir: matches!(entry.kind, crate::mft::types::FileKind::Directory),
            })
        })
        .collect();

    results.sort_unstable_by_key(|r| r.rank);
    results.truncate(limit);
    results
}

pub fn build_path(
    file_ref: u64,
    names: &HashMap<u64, String>,
    parents: &HashMap<u64, u64>,
    drive_root: &str,
    depth: usize,
) -> std::path::PathBuf {
    if depth > 64 {
        return std::path::PathBuf::from(drive_root);
    }
    let parent_ref = match parents.get(&file_ref) {
        Some(&p) => p,
        None => return std::path::PathBuf::from(drive_root),
    };
    if parent_ref == file_ref || !names.contains_key(&parent_ref) {
        return std::path::PathBuf::from(drive_root)
            .join(names.get(&file_ref).cloned().unwrap_or_default());
    }
    let mut path = build_path(parent_ref, names, parents, drive_root, depth + 1);
    if let Some(name) = names.get(&file_ref) {
        path.push(name);
    }
    path
}
