// #![allow(dead_code)]

// mod index;
// mod mft;
// mod utils;

// use std::sync::Arc;
// use std::io::{self, Write};
// use parking_lot::RwLock;
// use crossbeam_channel::unbounded;

// use crate::index::store::IndexStore;
// use crate::index::search::search;
// use crate::mft::reader::MftReader;
// use crate::mft::watcher::UsnWatcher;
// use crate::mft::types::IndexEvent;
// use crate::utils::drives::get_ntfs_drives;

// fn main() {
//     println!("╔══════════════════════════════════╗");
//     println!("║       FastSeek - File Search      ║");
//     println!("╚══════════════════════════════════╝");
//     println!();

//     let drives = get_ntfs_drives();
//     if drives.is_empty() {
//         eprintln!("No NTFS drives found. Are you running as Administrator?");
//         std::process::exit(1);
//     }

//     let index: Arc<RwLock<IndexStore>> = Arc::new(RwLock::new(IndexStore::new()));
//     let (tx, rx) = unbounded();
//     let cache_path = std::env::temp_dir().join("fastseek_cache.bin");

//     // --- Try loading from cache ---
//     let cache_loaded = if cache_path.exists() {
//         print!("Loading cached index... ");
//         io::stdout().flush().unwrap();
//         match std::fs::read(&cache_path) {
//             Ok(compressed) => {
//                 match lz4_flex::decompress_size_prepended(&compressed) {
//                     Ok(bytes) => {
//                         match bincode::deserialize::<crate::index::store::CacheData>(&bytes) {
//                             Ok(cache) => {
//                                 let count = cache.entries.len();
//                                 *index.write() = IndexStore::from_cache(cache);
//                                 println!("{} files loaded instantly", count);
//                                 println!();
//                                 true
//                             }
//                             Err(_) => { println!("cache corrupt, rescanning..."); false }
//                         }
//                     }
//                     Err(_) => { println!("cache corrupt, rescanning..."); false }
//                 }
//             }
//             Err(_) => { println!("cache unreadable, rescanning..."); false }
//         }
//     } else {
//         false
//     };

//     // --- Full MFT scan if no cache ---
//     if !cache_loaded {
//         println!("Found drives: {}", drives.iter().map(|d| format!("{}:", d.letter)).collect::<Vec<_>>().join(", "));
//         println!("Building index...");

//         let index_clone: Arc<RwLock<IndexStore>> = Arc::clone(&index);
//         let drives_clone = drives.clone();

//         let scan_thread = std::thread::spawn(move || {
//             let mut total = 0usize;
//             for drive in &drives_clone {
//                 print!("  Scanning {}:  ... ", drive.letter);
//                 io::stdout().flush().unwrap();

//                 let reader: MftReader = match MftReader::open(drive) {
//                     Ok(r) => r,
//                     Err(e) => { println!("FAILED ({:?})", e); continue; }
//                 };

//                 let t1 = std::time::Instant::now();
//                 let records = reader.scan();
//                 let count = records.len();
//                 println!("{} files  (scan {:.2}s)", count, t1.elapsed().as_secs_f64());

//                 {
//                     let mut store = index_clone.write();
//                     store.populate(records, &drive.root);
//                 }
//                 total += count;
//             }

//             {
//                 let mut store = index_clone.write();
//                 store.finalize();
//             }

//             println!();
//             println!("Index ready — {} total files", total);
//             println!();
//             total
//         });

//         scan_thread.join().unwrap();

//         // Save — compress before writing
//         {
//             let store = index.read();
//             let cache = store.to_cache();
//             match bincode::serialize(&cache) {
//                 Ok(bytes) => {
//                     let compressed = lz4_flex::compress_prepend_size(&bytes);
//                     let mb = compressed.len() as f64 / 1_048_576.0;
//                     match std::fs::write(&cache_path, &compressed) {
//                         Ok(_) => println!("Cache saved ({:.1}MB compressed) — next launch will be instant", mb),
//                         Err(e) => eprintln!("Could not save cache: {}", e),
//                     }
//                 }
//                 Err(e) => eprintln!("Could not serialize: {}", e),
//             }
//         }
//         println!();
//     }

//     // --- USN watchers ---
//     for drive in drives {
//         let tx_clone = tx.clone();
//         std::thread::spawn(move || {
//             if let Ok(mut watcher) = UsnWatcher::new(&drive, tx_clone) {
//                 watcher.run();
//             }
//         });
//     }

//     // --- Live index updates ---
//     let index_watcher: Arc<RwLock<IndexStore>> = Arc::clone(&index);
//     std::thread::spawn(move || {
//         for event in rx {
//             let mut store = index_watcher.write();
//             match event {
//                 IndexEvent::Created(r) => store.insert(r),
//                 IndexEvent::Deleted(id) => store.remove(id),
//                 IndexEvent::Renamed { old_ref, new_record } => store.rename(old_ref, new_record),
//                 IndexEvent::Moved { file_ref, new_parent_ref, name, kind } => {
//                 store.remove(file_ref);
//             }
//             }
//         }
//     });

//     search_loop(index);
// }

// fn search_loop(index: Arc<RwLock<IndexStore>>) {
//     println!("Commands:");
//     println!("  <query>        search files");
//     println!("  :<query>       directories only");
//     println!("  !<query>       files only");
//     println!("  *.ext          by extension e.g. *.pdf");
//     println!("  count          total indexed files");
//     println!("  rescan         clear cache and rescan");
//     println!("  quit           exit");
//     println!();

//     loop {
//         print!("search> ");
//         io::stdout().flush().unwrap();

//         let mut input = String::new();
//         match io::stdin().read_line(&mut input) {
//             Ok(0) | Err(_) => break,
//             Ok(_) => {}
//         }

//         let input = input.trim();
//         if input.is_empty() { continue; }

//         match input {
//             "quit" | "exit" | "q" => { println!("Bye."); break; }

//             "count" => {
//                 let store = index.read();
//                 println!("  {} files in index\n", store.len());
//             }

//             "rescan" => {
//                 let cache_path = std::env::temp_dir().join("fastseek_cache.bin");
//                 let _ = std::fs::remove_file(&cache_path);
//                 println!("Cache cleared. Restart fastseek to rescan.\n");
//             }

//             _ => {
//                 let (query, filter) = parse_query(input);

//                 let ext_filter: Option<String> = if query.starts_with("*.") {
//                     Some(query[2..].to_lowercase())
//                 } else {
//                     None
//                 };

//                 let actual_query = if ext_filter.is_some() { "" } else { query };

//                 let store = index.read();
//                 let start = std::time::Instant::now();
//                 let results = search(
//                     &store.entries,
//                     &store.names,
//                     &store.parents,
//                     &store.drive_root,
//                     actual_query,
//                     200,
//                 );
//                 let elapsed = start.elapsed();

//                 let results: Vec<_> = results.iter().filter(|r| {
//                     let kind_ok = match filter {
//                         Filter::All   => true,
//                         Filter::Dirs  => r.is_dir,
//                         Filter::Files => !r.is_dir,
//                     };
//                     let ext_ok = match &ext_filter {
//                         None => true,
//                         Some(ext) => r.full_path
//                             .extension()
//                             .map(|e| e.to_string_lossy().to_lowercase() == *ext)
//                             .unwrap_or(false),
//                     };
//                     kind_ok && ext_ok
//                 }).take(50).collect();

//                 if results.is_empty() {
//                     println!("  no results for \"{}\"\n", query);
//                 } else {
//                     println!();
//                     for (i, r) in results.iter().enumerate() {
//                         let kind = if r.is_dir { "DIR " } else { "FILE" };
//                         println!("  [{:>3}] [{}] {}", i + 1, kind, r.full_path.display());
//                     }
//                     println!();
//                     println!("  {} result(s) in {:.2}ms\n", results.len(), elapsed.as_secs_f64() * 1000.0);
//                 }
//             }
//         }
//     }
// }

// enum Filter { All, Dirs, Files }

// fn parse_query(input: &str) -> (&str, Filter) {
//     if let Some(q) = input.strip_prefix(':') { (q, Filter::Dirs) }
//     else if let Some(q) = input.strip_prefix('!') { (q, Filter::Files) }
//     else { (input, Filter::All) }
// }


#![allow(dead_code)]

mod index;
mod mft;
mod utils;

use std::sync::Arc;
use std::io::{self, Write};
use parking_lot::RwLock;
use crossbeam_channel::unbounded;

use crate::index::store::IndexStore;
use crate::index::search::search;
use crate::mft::reader::MftReader;
use crate::mft::watcher::UsnWatcher;
use crate::mft::types::IndexEvent;
use crate::utils::drives::get_ntfs_drives;

fn main() {
    println!("╔══════════════════════════════════╗");
    println!("║       FastSeek - File Search      ║");
    println!("╚══════════════════════════════════╝");
    println!();

    let drives = get_ntfs_drives();
    if drives.is_empty() {
        eprintln!("No NTFS drives found. Are you running as Administrator?");
        std::process::exit(1);
    }

    let index: Arc<RwLock<IndexStore>> = Arc::new(RwLock::new(IndexStore::new()));
    let (tx, rx) = unbounded();
    let cache_path = std::env::temp_dir().join("fastseek_cache.bin");

    // --- Try loading from cache ---
    let cache_loaded = if cache_path.exists() {
        print!("Loading cached index... ");
        io::stdout().flush().unwrap();
        match std::fs::read(&cache_path) {
            Ok(compressed) => {
                match lz4_flex::decompress_size_prepended(&compressed) {
                    Ok(bytes) => {
                        match bincode::deserialize::<crate::index::store::CacheData>(&bytes) {
                            Ok(cache) => {
                                let count = cache.entries.len();
                                let checkpoints = cache.checkpoints.clone();
                                *index.write() = IndexStore::from_cache(cache);
                                println!("{} files", count);

                                // --- Delta catch-up ---
                                if !checkpoints.is_empty() {
                                    print!("Catching up on changes since last run... ");
                                    io::stdout().flush().unwrap();

                                    let (delta_tx, delta_rx) = unbounded::<IndexEvent>();
                                    let mut journal_ok = true;

                                    for drive in &drives {
                                        let cp = checkpoints.iter()
                                            .find(|c| c.drive_letter == drive.letter);

                                        if let Some(cp) = cp {
                                            match UsnWatcher::new_from(drive, delta_tx.clone(), Some(cp)) {
                                                Ok(mut watcher) => {
                                                    watcher.drain();
                                                    let new_cp = watcher.checkpoint();
                                                    let mut store = index.write();
                                                    store.checkpoints.retain(|c| c.drive_letter != drive.letter);
                                                    store.checkpoints.push(new_cp);
                                                }
                                                Err(_) => {
                                                    println!("journal reset, full rescan needed.");
                                                    let _ = std::fs::remove_file(&cache_path);
                                                    journal_ok = false;
                                                    break;
                                                }
                                            }
                                        } else {
                                            // No checkpoint for this drive — cache is incomplete
                                            println!("missing checkpoint for {}:, full rescan needed.", drive.letter);
                                            let _ = std::fs::remove_file(&cache_path);
                                            journal_ok = false;
                                            break;
                                        }
                                    }

                                    drop(delta_tx);

                                    if journal_ok {
                                        let mut applied = 0usize;
                                        let mut store = index.write();
                                        for event in delta_rx {
                                            match event {
                                                IndexEvent::Created(r) => store.insert(r),
                                                IndexEvent::Deleted(id) => store.remove(id),
                                                IndexEvent::Renamed { old_ref, new_record } => {
                                                    store.rename(old_ref, new_record)
                                                }
                                                IndexEvent::Moved { file_ref, new_parent_ref, name, kind } => {
                                                    store.apply_move(file_ref, new_parent_ref, name, kind);
                                                }
                                            }
                                            applied += 1;
                                        }
                                        println!("{} change(s) applied", applied);
                                        println!();
                                        true
                                    } else {
                                        false
                                    }
                                } else {
                                    println!();
                                    true
                                }
                            }
                            Err(_) => { println!("cache corrupt, rescanning..."); false }
                        }
                    }
                    Err(_) => { println!("cache corrupt, rescanning..."); false }
                }
            }
            Err(_) => { println!("cache unreadable, rescanning..."); false }
        }
    } else {
        false
    };

    // --- Full MFT scan if no cache ---
    if !cache_loaded {
        println!("Found drives: {}", drives.iter().map(|d| format!("{}:", d.letter)).collect::<Vec<_>>().join(", "));
        println!("Building index...");

        // Capture checkpoints BEFORE scan so changes during scan aren't lost
        {
            let mut store = index.write();
            for drive in &drives {
                let (dummy_tx, _) = unbounded::<IndexEvent>();
                if let Ok(w) = UsnWatcher::new(drive, dummy_tx) {
                    store.checkpoints.push(w.checkpoint());
                }
            }
        }

        let index_clone: Arc<RwLock<IndexStore>> = Arc::clone(&index);
        let drives_clone = drives.clone();

        let scan_thread = std::thread::spawn(move || {
            let mut total = 0usize;
            for drive in &drives_clone {
                print!("  Scanning {}:  ... ", drive.letter);
                io::stdout().flush().unwrap();

                let reader: MftReader = match MftReader::open(drive) {
                    Ok(r) => r,
                    Err(e) => { println!("FAILED ({:?})", e); continue; }
                };

                let t1 = std::time::Instant::now();
                let (scan, method) = match reader.scan_direct() {
                    Some(s) => (s, "direct"),
                    None => (reader.scan(), "ioctl"),
                };
                let count = scan.records.len();
                let scan_time = t1.elapsed();

                let t2 = std::time::Instant::now();
                {
                    let mut store = index_clone.write();
                    store.populate_from_scan(scan, &drive.root);
                }
                let index_time = t2.elapsed();

                println!("{} files  (scan {:.2}s {}, index {:.2}s)",
                    count, scan_time.as_secs_f64(), method, index_time.as_secs_f64());

                total += count;
            }

            {
                let mut store = index_clone.write();
                store.finalize();
            }

            println!();
            println!("Index ready — {} total files", total);
            println!();
            total
        });

        scan_thread.join().unwrap();

        // Save cache
        {
            let store = index.read();
            let cache = store.to_cache();
            match bincode::serialize(&cache) {
                Ok(bytes) => {
                    let compressed = lz4_flex::compress_prepend_size(&bytes);
                    let mb = compressed.len() as f64 / 1_048_576.0;
                    match std::fs::write(&cache_path, &compressed) {
                        Ok(_) => println!("Cache saved ({:.1}MB) — next launch will be instant", mb),
                        Err(e) => eprintln!("Could not save cache: {}", e),
                    }
                }
                Err(e) => eprintln!("Could not serialize: {}", e),
            }
        }
        println!();
    }

    // --- USN watchers for live updates while running ---
    let live_checkpoints: Arc<parking_lot::Mutex<Vec<crate::mft::types::JournalCheckpoint>>> =
        Arc::new(parking_lot::Mutex::new(index.read().checkpoints.clone()));

    for drive in &drives {
        let tx_clone = tx.clone();
        let drive_clone = drive.clone();
        let cps = Arc::clone(&live_checkpoints);
        std::thread::spawn(move || {
            if let Ok(mut watcher) = UsnWatcher::new(&drive_clone, tx_clone) {
                watcher.run_shared(cps);
            }
        });
    }

    // --- Live index updates ---
    let index_live: Arc<RwLock<IndexStore>> = Arc::clone(&index);
    std::thread::spawn(move || {
        for event in rx {
            let mut store = index_live.write();
            match event {
                IndexEvent::Created(r) => store.insert(r),
                IndexEvent::Deleted(id) => store.remove(id),
                IndexEvent::Renamed { old_ref, new_record } => store.rename(old_ref, new_record),
                IndexEvent::Moved { file_ref, new_parent_ref, name, kind } => {
                    store.apply_move(file_ref, new_parent_ref, name, kind);
                }
            }
        }
    });

    // Save updated cache on exit with latest checkpoints from live watchers
    let index_for_save = Arc::clone(&index);
    let cps_for_save = Arc::clone(&live_checkpoints);
    ctrlc::set_handler(move || {
        let mut store = index_for_save.write();
        store.checkpoints = cps_for_save.lock().clone();
        let cache = store.to_cache();
        if let Ok(bytes) = bincode::serialize(&cache) {
            let compressed = lz4_flex::compress_prepend_size(&bytes);
            let _ = std::fs::write(
                std::env::temp_dir().join("fastseek_cache.bin"),
                &compressed,
            );
        }
        std::process::exit(0);
    }).ok();

    search_loop(index);
}

fn search_loop(index: Arc<RwLock<IndexStore>>) {
    println!("Commands:");
    println!("  <query>        search files");
    println!("  :<query>       directories only");
    println!("  !<query>       files only");
    println!("  *.ext          by extension e.g. *.pdf");
    println!("  count          total indexed files");
    println!("  rescan         clear cache and rescan");
    println!("  quit           exit");
    println!();

    loop {
        print!("search> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }

        let input = input.trim();
        if input.is_empty() { continue; }

        match input {
            "quit" | "exit" | "q" => {
                println!("Bye.");
                break;
            }

            "count" => {
                let store = index.read();
                println!("  {} files in index\n", store.len());
            }

            "rescan" => {
                let cache_path = std::env::temp_dir().join("fastseek_cache.bin");
                let _ = std::fs::remove_file(&cache_path);
                println!("Cache cleared. Restart fastseek to rescan.\n");
            }

            _ => {
                let (query, filter) = parse_query(input);

                let ext_filter: Option<String> = if query.starts_with("*.") {
                    Some(query[2..].to_lowercase())
                } else {
                    None
                };

                let store = index.read();
                let start = std::time::Instant::now();

                let results: Vec<_> = if let Some(ref ext) = ext_filter {
                    // Extension-only search: scan all entries directly
                    use crate::index::search::SearchResult;
                    store.entries.iter().filter_map(|entry| {
                        let name = &entry.name_lower;
                        if !name.ends_with(&format!(".{}", ext)) {
                            return None;
                        }
                        let kind_ok = match filter {
                            Filter::All   => true,
                            Filter::Dirs  => matches!(entry.kind, crate::mft::types::FileKind::Directory),
                            Filter::Files => !matches!(entry.kind, crate::mft::types::FileKind::Directory),
                        };
                        if !kind_ok { return None; }

                        let full_path = crate::index::search::build_path(
                            entry.file_ref, &store.names, &store.parents, &store.drive_root, 0,
                        );
                        Some(SearchResult {
                            full_path,
                            name: entry.name_original.clone(),
                            rank: 0,
                            is_dir: matches!(entry.kind, crate::mft::types::FileKind::Directory),
                        })
                    }).take(50).collect()
                } else {
                    let raw = search(
                        &store.entries,
                        &store.names,
                        &store.parents,
                        &store.drive_root,
                        query,
                        200,
                    );
                    raw.into_iter().filter(|r| {
                        match filter {
                            Filter::All   => true,
                            Filter::Dirs  => r.is_dir,
                            Filter::Files => !r.is_dir,
                        }
                    }).take(50).collect()
                };
                let elapsed = start.elapsed();

                if results.is_empty() {
                    println!("  no results for \"{}\"\n", query);
                } else {
                    println!();
                    for (i, r) in results.iter().enumerate() {
                        let kind = if r.is_dir { "DIR " } else { "FILE" };
                        println!("  [{:>3}] [{}] {}", i + 1, kind, r.full_path.display());
                    }
                    println!();
                    println!("  {} result(s) in {:.2}ms\n",
                        results.len(), elapsed.as_secs_f64() * 1000.0);
                }
            }
        }
    }
}

enum Filter { All, Dirs, Files }

fn parse_query(input: &str) -> (&str, Filter) {
    if let Some(q) = input.strip_prefix(':') { (q, Filter::Dirs) }
    else if let Some(q) = input.strip_prefix('!') { (q, Filter::Files) }
    else { (input, Filter::All) }
}




///////////////////////////////////////////////////////////////////////

// 6 sec reader.rs

// #![allow(dead_code)]

// use std::mem;
// use windows::{
//     core::PCWSTR,
//     Win32::Foundation::HANDLE,
//     Win32::Storage::FileSystem::{
//         CreateFileW, ReadFile, SetFilePointerEx,
//         FILE_BEGIN, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_SEQUENTIAL_SCAN,
//         FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
//     },
//     Win32::System::Ioctl::{
//         FSCTL_ENUM_USN_DATA, MFT_ENUM_DATA_V0, USN_RECORD_V2,
//     },
//     Win32::System::IO::DeviceIoControl,
// };

// use crate::mft::types::NtfsDrive;

// const FALLBACK_BUF: usize = 4 * 1024 * 1024;
// const DIRECT_BUF: usize = 4 * 1024 * 1024;

// /// Compact MFT record — no heap allocations per file.
// pub struct CompactRecord {
//     pub file_ref: u64,
//     pub parent_ref: u64,
//     pub name_off: u32,
//     pub name_len: u16,
//     pub is_dir: bool,
// }

// /// Result of a full MFT scan.
// pub struct ScanResult {
//     pub records: Vec<CompactRecord>,
//     pub name_data: Vec<u16>,
// }

// pub struct MftReader {
//     handle: HANDLE,
//     pub drive: NtfsDrive,
// }

// impl MftReader {
//     pub fn open(drive: &NtfsDrive) -> windows::core::Result<Self> {
//         let path: Vec<u16> = drive
//             .device_path
//             .encode_utf16()
//             .chain(Some(0))
//             .collect();

//         let handle = unsafe {
//             CreateFileW(
//                 PCWSTR(path.as_ptr()),
//                 0x80000000u32,
//                 FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
//                 None,
//                 OPEN_EXISTING,
//                 FILE_FLAG_BACKUP_SEMANTICS,
//                 None,
//             )?
//         };

//         Ok(Self {
//             handle,
//             drive: drive.clone(),
//         })
//     }

//     // ---------------------------------------------------------------
//     //  Primary: direct $MFT file read  (falls back to FSCTL if fails)
//     // ---------------------------------------------------------------

//     /// Try direct sequential read of $MFT for maximum speed.
//     /// Returns None if direct access is unavailable.
//     pub fn scan_direct(&self) -> Option<ScanResult> {
//         let record_size = self.read_mft_record_size()?;

//         let mft_path = format!("{}$MFT", self.drive.root);
//         let mft_wide: Vec<u16> = mft_path.encode_utf16().chain(Some(0)).collect();

//         let mft_handle = unsafe {
//             CreateFileW(
//                 PCWSTR(mft_wide.as_ptr()),
//                 0x80000000u32,
//                 FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
//                 None,
//                 OPEN_EXISTING,
//                 FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_SEQUENTIAL_SCAN,
//                 None,
//             )
//             .ok()?
//         };

//         let mut records: Vec<CompactRecord> = Vec::with_capacity(3_000_000);
//         let mut name_data: Vec<u16> = Vec::with_capacity(40_000_000);
//         let mut buffer = vec![0u8; DIRECT_BUF];
//         let mut mft_index: u64 = 0;
//         let mut leftover = 0usize;

//         loop {
//             let mut bytes_read = 0u32;
//             let ok = unsafe {
//                 ReadFile(
//                     mft_handle,
//                     Some(&mut buffer[leftover..]),
//                     Some(&mut bytes_read),
//                     None,
//                 )
//             };
//             if ok.is_err() || bytes_read == 0 {
//                 break;
//             }

//             let total = leftover + bytes_read as usize;
//             let mut offset = 0usize;

//             use rayon::prelude::*;

//             while offset + record_size <= total {
//                 let applied =
//                     Self::apply_fixup(&mut buffer[offset..offset + record_size], record_size);
            
//                 if applied {
//                     Self::parse_file_record(
//                         &buffer[offset..offset + record_size],
//                         mft_index,
//                         &mut records,
//                         &mut name_data,
//                     );
//                 }
            
//                 mft_index += 1;
//                 offset += record_size;
//             }

//             offset = (total / record_size) * record_size;

//             leftover = total - offset;
//             if leftover > 0 {
//                 buffer.copy_within(offset..total, 0);
//             }

//             // for (mut recs, mut names) in results {
//             //     records.append(&mut recs);
//             //     name_data.append(&mut names);
//             // }
    
//             leftover = total - offset;
//             if leftover > 0 {
//                 buffer.copy_within(offset..total, 0);
//             }
//         }

//         unsafe {
//             windows::Win32::Foundation::CloseHandle(mft_handle).ok();
//         }

//         Some(ScanResult { records, name_data })
//     }

//     // ---------------------------------------------------------------
//     //  Fallback: FSCTL_ENUM_USN_DATA  (4 MB buffer)
//     // ---------------------------------------------------------------

//     pub fn scan(&self) -> ScanResult {
//         let mut records: Vec<CompactRecord> = Vec::with_capacity(3_000_000);
//         let mut name_data: Vec<u16> = Vec::with_capacity(40_000_000);

//         let mut enum_data = MFT_ENUM_DATA_V0 {
//             StartFileReferenceNumber: 0,
//             LowUsn: 0,
//             HighUsn: i64::MAX,
//         };

//         let mut buffer = vec![0u8; FALLBACK_BUF];

//         loop {
//             let mut bytes_returned: u32 = 0;

//             let ok = unsafe {
//                 DeviceIoControl(
//                     self.handle,
//                     FSCTL_ENUM_USN_DATA,
//                     Some(&enum_data as *const _ as *const _),
//                     mem::size_of::<MFT_ENUM_DATA_V0>() as u32,
//                     Some(buffer.as_mut_ptr() as *mut _),
//                     FALLBACK_BUF as u32,
//                     Some(&mut bytes_returned),
//                     None,
//                 )
//             };

//             if let Err(e) = ok {
//                 let code = e.code().0 as u32;
//                 if code == 0x80070026 {
//                     break;
//                 }
//                 eprintln!("MFT error on {}: {:?}", self.drive.letter, e);
//                 break;
//             }

//             if bytes_returned <= 8 {
//                 break;
//             }

//             let next_ref = u64::from_ne_bytes(buffer[0..8].try_into().unwrap());
//             enum_data.StartFileReferenceNumber = next_ref;

//             let mut offset = 8usize;
//             while offset + mem::size_of::<USN_RECORD_V2>() <= bytes_returned as usize {
//                 let record = unsafe {
//                     &*(buffer.as_ptr().add(offset) as *const USN_RECORD_V2)
//                 };

//                 let rec_len = record.RecordLength as usize;
//                 if rec_len == 0 || offset + rec_len > bytes_returned as usize {
//                     break;
//                 }

//                 let name_offset = record.FileNameOffset as usize;
//                 let name_len = record.FileNameLength as usize / 2;
//                 let name_ptr = unsafe {
//                     buffer.as_ptr().add(offset + name_offset) as *const u16
//                 };
//                 let name_slice = unsafe { std::slice::from_raw_parts(name_ptr, name_len) };

//                 let arena_off = name_data.len() as u32;
//                 name_data.extend_from_slice(name_slice);

//                 records.push(CompactRecord {
//                     file_ref: record.FileReferenceNumber as u64,
//                     parent_ref: record.ParentFileReferenceNumber as u64,
//                     name_off: arena_off,
//                     name_len: name_len as u16,
//                     is_dir: (record.FileAttributes & 0x10) != 0,
//                 });

//                 offset += rec_len;
//             }
//         }

//         ScanResult { records, name_data }
//     }

//     // ---------------------------------------------------------------
//     //  NTFS helpers
//     // ---------------------------------------------------------------

//     /// Read MFT record size from the NTFS boot sector.
//     fn read_mft_record_size(&self) -> Option<usize> {
//         unsafe {
//             SetFilePointerEx(self.handle, 0, None, FILE_BEGIN).ok()?;
//         }
//         let mut boot = [0u8; 512];
//         let mut br = 0u32;
//         unsafe {
//             ReadFile(self.handle, Some(&mut boot), Some(&mut br), None).ok()?;
//         }
//         if br < 512 || &boot[3..7] != b"NTFS" {
//             return None;
//         }

//         let bytes_per_sector = u16::from_le_bytes([boot[0x0B], boot[0x0C]]) as usize;
//         let sectors_per_cluster = boot[0x0D] as usize;
//         let cluster_size = bytes_per_sector * sectors_per_cluster;

//         let raw = boot[0x40] as i8;
//         let record_size = if raw > 0 {
//             raw as usize * cluster_size
//         } else {
//             1usize << (-(raw as i32) as usize)
//         };

//         Some(record_size)
//     }

//     /// Apply NTFS multi-sector fixup. Returns false if the record is invalid.
//     fn apply_fixup(record: &mut [u8], record_size: usize) -> bool {
//         if record.len() < 48 || &record[0..4] != b"FILE" {
//             return false;
//         }

//         let fixup_off = u16::from_le_bytes([record[4], record[5]]) as usize;
//         let fixup_cnt = u16::from_le_bytes([record[6], record[7]]) as usize;

//         if fixup_cnt < 2 || fixup_off + fixup_cnt * 2 > record_size {
//             return false;
//         }

//         let check = [record[fixup_off], record[fixup_off + 1]];

//         for i in 1..fixup_cnt {
//             let end = i * 512 - 2;
//             if end + 1 >= record_size {
//                 break;
//             }
//             if record[end] != check[0] || record[end + 1] != check[1] {
//                 return false;
//             }
//             record[end] = record[fixup_off + i * 2];
//             record[end + 1] = record[fixup_off + i * 2 + 1];
//         }

//         true
//     }

//     /// Parse one MFT FILE record, extracting name + parent into the arena.
//     fn parse_file_record(
//         record: &[u8],
//         mft_index: u64,
//         records: &mut Vec<CompactRecord>,
//         name_data: &mut Vec<u16>,
//     ) {
//         let flags = u16::from_le_bytes([record[0x16], record[0x17]]);
//         if flags & 0x01 == 0 {
//             return; // not in use
//         }

//         let is_dir = flags & 0x02 != 0;
//         let seq = u16::from_le_bytes([record[0x10], record[0x11]]) as u64;
//         let file_ref = mft_index | (seq << 48);

//         let first_attr = u16::from_le_bytes([record[0x14], record[0x15]]) as usize;
//         let used_size = u32::from_le_bytes(
//             record[0x18..0x1C].try_into().unwrap_or([0; 4]),
//         ) as usize;

//         let mut aoff = first_attr;
//         while aoff + 4 <= record.len() {
//             let atype = u32::from_le_bytes(
//                 record[aoff..aoff + 4].try_into().unwrap_or([0xFF; 4]),
//             );
//             if atype == 0xFFFF_FFFF {
//                 break;
//             }
//             if aoff + 8 > record.len() {
//                 break;
//             }
//             let alen = u32::from_le_bytes(
//                 record[aoff + 4..aoff + 8].try_into().unwrap_or([0; 4]),
//             ) as usize;
//             if alen == 0 || aoff + alen > record.len() {
//                 break;
//             }

//             if atype == 0x30 && record[aoff + 8] == 0 {
//                 if aoff + 22 <= record.len() {
//                     let vlen = u32::from_le_bytes(
//                         record[aoff + 16..aoff + 20].try_into().unwrap_or([0; 4]),
//                     ) as usize;
            
//                     let voff = u16::from_le_bytes([record[aoff + 20], record[aoff + 21]]) as usize;
//                     let vs = aoff + voff;
            
//                     if vs + 66 <= record.len() && vlen >= 66 {
//                         let parent = u64::from_le_bytes(record[vs..vs + 8].try_into().unwrap());
            
//                         let nlen = record[vs + 64] as usize;
//                         let ns = record[vs + 65];
            
//                         if vs + 66 + nlen * 2 <= record.len() {
            
//                             // skip pure DOS if a better name exists
//                             if ns == 2 && nlen <= 12 {
//                                 continue; // skip short 8.3 duplicates
//                             }
                                        
//                             let arena_off = name_data.len() as u32;
            
//                             let name_slice =
//                                 &record[vs + 66..vs + 66 + nlen * 2];
            
//                                 let arena_off = name_data.len() as u32;

//                                 for i in 0..nlen {
//                                     let p = vs + 66 + i * 2;
//                                     name_data.push(u16::from_le_bytes([record[p], record[p + 1]]));
//                                 }
                                
//                                 records.push(CompactRecord {
//                                     file_ref,
//                                     parent_ref: parent,
//                                     name_off: arena_off,
//                                     name_len: nlen as u16,
//                                     is_dir,
//                                 });
//                         }
//                     }
//                 }
//             }
//             aoff += alen;
//         }
//     }
// }

// impl Drop for MftReader {
//     fn drop(&mut self) {
//         unsafe { windows::Win32::Foundation::CloseHandle(self.handle).ok() };
//     }
// }
