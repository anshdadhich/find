use std::collections::HashMap;
use rayon::prelude::*;
use crate::index::store::IndexEntry;

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
) -> Vec<SearchResult> {
    if query.is_empty() {
        return Vec::new();
    }

    let q = query.to_lowercase();

    let mut results: Vec<SearchResult> = entries
        .par_iter()
        .filter_map(|entry| {
            let rank = if entry.name_lower == q { 0 }
                else if entry.name_lower.starts_with(&q) { 1 }
                else if entry.name_lower.contains(&q) { 2 }
                else { return None; };

            let full_path = build_path(entry.file_ref, names, parents, drive_root, 0);

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

fn build_path(
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