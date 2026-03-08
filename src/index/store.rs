// #![allow(dead_code)]
// use std::collections::HashMap;
// use serde::{Serialize, Deserialize};
// use crate::mft::types::{FileKind, FileRecord};

// // What gets cached to disk — compact
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct CachedEntry {
//     pub file_ref: u64,
//     pub parent_ref: u64,
//     pub name: String,   // original case only, lowercase computed on load
//     pub kind: FileKind,
//     pub size: u64,
// }

// // What lives in memory during search — has lowercase for fast compare
// #[derive(Debug, Clone)]
// pub struct IndexEntry {
//     pub file_ref: u64,
//     pub parent_ref: u64,
//     pub name_lower: String,
//     pub name_original: String,
//     pub kind: FileKind,
// }

// impl IndexEntry {
//     pub fn from_cached(c: CachedEntry) -> Self {
//         Self {
//             name_lower: c.name.to_lowercase(),
//             name_original: c.name,
//             file_ref: c.file_ref,
//             parent_ref: c.parent_ref,
//             kind: c.kind,
//             size: c.size,
//         }
//     }
// }

// #[derive(Serialize, Deserialize)]
// pub struct CacheData {
//     pub entries: Vec<CachedEntry>,
//     pub drive_root: String,
//     pub saved_at_usn: i64,      // ADD THIS
//     pub journal_id: u64,        // ADD THIS
// }

// pub struct IndexStore {
//     pub entries: Vec<IndexEntry>,
//     pub names: HashMap<u64, String>,
//     pub parents: HashMap<u64, u64>,
//     pub drive_root: String,
// }

// impl IndexStore {
//     pub fn new() -> Self {
//         Self {
//             entries: Vec::with_capacity(1_000_000),
//             names: HashMap::with_capacity(1_000_000),
//             parents: HashMap::with_capacity(1_000_000),
//             drive_root: String::new(),
//         }
//     }

//     pub fn populate(&mut self, records: Vec<FileRecord>, drive_root: &str) {
//         self.drive_root = drive_root.to_string();
//         for r in &records {
//             self.names.insert(r.file_ref, r.name.clone());
//             self.parents.insert(r.file_ref, r.parent_ref);
//         }
//         let new: Vec<IndexEntry> = records
//             .into_iter()
//             .map(|r| IndexEntry {
//                 file_ref: r.file_ref,
//                 parent_ref: r.parent_ref,
//                 name_lower: r.name.to_lowercase(),
//                 name_original: r.name.clone(),
//                 kind: r.kind,
//             })
//             .collect();
//         self.entries.extend(new);
//     }

//     pub fn finalize(&mut self) {
//         self.entries.sort_unstable_by(|a, b| a.name_lower.cmp(&b.name_lower));
//     }

//     pub fn to_cache(&self) -> CacheData {
//         CacheData {
//             entries: self.entries.iter().map(|e| CachedEntry {
//                 file_ref: e.file_ref,
//                 parent_ref: e.parent_ref,
//                 name: e.name_original.clone(),
//                 kind: e.kind.clone(),
//             }).collect(),
//             drive_root: self.drive_root.clone(),
//         }
//     }

//     pub fn from_cache(cache: CacheData) -> Self {
//       let mut names = HashMap::with_capacity(cache.entries.len());
//       let mut parents = HashMap::with_capacity(cache.entries.len());
      
//       for e in &cache.entries {
//           names.insert(e.file_ref, e.name.clone());
//           parents.insert(e.file_ref, e.parent_ref);
//       }
  
//       let entries = cache.entries
//           .into_iter()
//           .map(IndexEntry::from_cached)
//           .collect();
  
//       Self { entries, names, parents, drive_root: cache.drive_root }
// }

//     pub fn insert(&mut self, record: FileRecord) {
//         self.names.insert(record.file_ref, record.name.clone());
//         self.parents.insert(record.file_ref, record.parent_ref);
//         let entry = IndexEntry {
//             file_ref: record.file_ref,
//             parent_ref: record.parent_ref,
//             name_lower: record.name.to_lowercase(),
//             name_original: record.name.clone(),
//             kind: record.kind,
//         };
//         let pos = self.entries.partition_point(|e| e.name_lower < entry.name_lower);
//         self.entries.insert(pos, entry);
//     }

//     pub fn remove(&mut self, file_ref: u64) {
//         self.names.remove(&file_ref);
//         self.parents.remove(&file_ref);
//         self.entries.retain(|e| e.file_ref != file_ref);
//     }

//     pub fn rename(&mut self, old_ref: u64, new_record: FileRecord) {
//         self.remove(old_ref);
//         self.insert(new_record);
//     }

//     pub fn len(&self) -> usize {
//         self.entries.len()
//     }

//     pub fn move_entry(
//     &mut self,
//     file_ref: u64,
//     new_parent_ref: u64,
//     name: String,
//     kind: FileKind,
//     ) {
//         // update parent map
//         self.parents.insert(file_ref, new_parent_ref);
    
//         // update name map
//         self.names.insert(file_ref, name.clone());
    
//         // update entry in sorted list
//         if let Some(pos) = self.entries.iter().position(|e| e.file_ref == file_ref) {
//             let entry = &mut self.entries[pos];
    
//             entry.parent_ref = new_parent_ref;
//             entry.name_original = name.clone();
//             entry.name_lower = name.to_lowercase();
//             entry.kind = kind;
//         }
    
//         // re-sort because name may have changed
//         self.entries.sort_unstable_by(|a, b| a.name_lower.cmp(&b.name_lower));
//     }
// }


#![allow(dead_code)]
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::mft::types::{FileKind, FileRecord, JournalCheckpoint};
use crate::mft::reader::ScanResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedEntry {
    pub file_ref: u64,
    pub parent_ref: u64,
    pub name: String,
    pub kind: FileKind,
}

#[derive(Debug, Clone)]
pub struct IndexEntry {
    pub file_ref: u64,
    pub parent_ref: u64,
    pub name_lower: String,
    pub name_original: String,
    pub kind: FileKind,
}

impl IndexEntry {
    pub fn from_cached(c: CachedEntry) -> Self {
        Self {
            name_lower: c.name.to_lowercase(),
            name_original: c.name,
            file_ref: c.file_ref,
            parent_ref: c.parent_ref,
            kind: c.kind,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct CacheData {
    pub entries: Vec<CachedEntry>,
    pub drive_root: String,
    pub checkpoints: Vec<JournalCheckpoint>,
}

pub struct IndexStore {
    pub entries: Vec<IndexEntry>,
    pub names: HashMap<u64, String>,
    pub parents: HashMap<u64, u64>,
    pub drive_root: String,
    pub checkpoints: Vec<JournalCheckpoint>,
}

impl IndexStore {
    pub fn new() -> Self {
        Self {
            entries: Vec::with_capacity(3_000_000),
            names: HashMap::with_capacity(3_000_000),
            parents: HashMap::with_capacity(3_000_000),
            drive_root: String::new(),
            checkpoints: Vec::new(),
        }
    }

    /// Populate from compact arena-based scan result — single pass, minimal clones.
    pub fn populate_from_scan(&mut self, scan: ScanResult, drive_root: &str) {
        self.drive_root = drive_root.to_string();
        let count = scan.records.len();
        self.entries.reserve(count);
        self.names.reserve(count);
        self.parents.reserve(count);

        for r in &scan.records {
            let name_slice = &scan.name_data[r.name_off as usize..(r.name_off as usize + r.name_len as usize)];
            let name = String::from_utf16_lossy(name_slice);
            let name_lower = name.to_lowercase();
            let kind = if r.is_dir { FileKind::Directory } else { FileKind::File };

            self.names.insert(r.file_ref, name.clone());
            self.parents.insert(r.file_ref, r.parent_ref);
            self.entries.push(IndexEntry {
                file_ref: r.file_ref,
                parent_ref: r.parent_ref,
                name_lower,
                name_original: name,
                kind,
            });
        }
    }

    pub fn finalize(&mut self) {
        self.entries.sort_unstable_by(|a, b| a.name_lower.cmp(&b.name_lower));
    }

    pub fn apply_move(&mut self, file_ref: u64, new_parent_ref: u64, name: String, kind: FileKind) {
        self.remove(file_ref);
        self.insert(FileRecord {
            file_ref,
            parent_ref: new_parent_ref,
            name,
            kind,
        });
    }

    pub fn to_cache(&self) -> CacheData {
        CacheData {
            entries: self.entries.iter().map(|e| CachedEntry {
                file_ref: e.file_ref,
                parent_ref: e.parent_ref,
                name: e.name_original.clone(),
                kind: e.kind.clone(),
            }).collect(),
            drive_root: self.drive_root.clone(),
            checkpoints: self.checkpoints.clone(),
        }
    }

    pub fn from_cache(cache: CacheData) -> Self {
        let mut names = HashMap::with_capacity(cache.entries.len());
        let mut parents = HashMap::with_capacity(cache.entries.len());

        for e in &cache.entries {
            names.insert(e.file_ref, e.name.clone());
            parents.insert(e.file_ref, e.parent_ref);
        }

        let entries = cache.entries
            .into_iter()
            .map(IndexEntry::from_cached)
            .collect();

        Self {
            entries,
            names,
            parents,
            drive_root: cache.drive_root,
            checkpoints: cache.checkpoints,
        }
    }

    pub fn insert(&mut self, record: FileRecord) {
        self.names.insert(record.file_ref, record.name.clone());
        self.parents.insert(record.file_ref, record.parent_ref);
        let name_lower = record.name.to_lowercase();
        let entry = IndexEntry {
            file_ref: record.file_ref,
            parent_ref: record.parent_ref,
            name_lower,
            name_original: record.name,
            kind: record.kind,
        };
        let pos = self.entries.partition_point(|e| e.name_lower < entry.name_lower);
        self.entries.insert(pos, entry);
    }

    pub fn remove(&mut self, file_ref: u64) {
        self.names.remove(&file_ref);
        self.parents.remove(&file_ref);
        self.entries.retain(|e| e.file_ref != file_ref);
    }

    pub fn rename(&mut self, old_ref: u64, new_record: FileRecord) {
        self.remove(old_ref);
        self.insert(new_record);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}