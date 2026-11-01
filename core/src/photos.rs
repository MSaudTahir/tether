//! The phone's newest photos and videos, with small thumbnails made on the phone and cached here.

use std::fs;
use std::path::Path;

use base64::Engine;

use crate::adb::Adb;
use crate::{apps, files, Photo, PhoneFile, Result};

/// Where cameras and screenshots usually go.
const FOLDERS: [&str; 3] = ["/sdcard/DCIM/Camera", "/sdcard/Pictures/Screenshots", "/sdcard/DCIM/Screenshots"];
const EXTENSIONS: [&str; 11] = ["jpg", "jpeg", "png", "webp", "heic", "gif", "mp4", "mkv", "webm", "3gp", "mov"];

pub fn recent(adb: &Adb, data_dir: &Path, serial: &str, count: usize) -> Result<Vec<Photo>> {
    let mut found: Vec<PhoneFile> = FOLDERS
        .iter()
        .filter_map(|folder| files::list(adb, serial, folder, false).ok())
        .flatten()
        .filter(|f| !f.folder && f.name.rsplit_once('.').is_some_and(|(_, e)| EXTENSIONS.contains(&e.to_lowercase().as_str())))
        .collect();
    found.sort_by(|a, b| b.modified.cmp(&a.modified));
    found.truncate(count);

    let safe: String = serial.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
    let cache = data_dir.join("thumbs").join(safe);
    fs::create_dir_all(&cache)?;
    let thumb = |f: &PhoneFile| cache.join(format!("{:016x}.jpg", fnv(&format!("{}|{}", f.path, f.modified))));

    // Make the missing thumbnails on the phone, a few at a time (they go on one command line).
    let missing: Vec<&PhoneFile> = found.iter().filter(|f| !thumb(f).exists()).collect();
    for batch in missing.chunks(16) {
        let mut args = vec!["thumbs".to_string()];
        args.extend(batch.iter().map(|f| format!("'{}'", f.path.replace('\'', r"'\''"))));
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = apps::helper(adb, data_dir, serial, &refs)?;
        for line in out.lines() {
            let Some((path, data)) = line.split_once('\t') else { continue };
            let (Some(file), Ok(jpeg)) = (batch.iter().find(|f| f.path == path), base64::engine::general_purpose::STANDARD.decode(data)) else { continue };
            if !jpeg.is_empty() {
                fs::write(thumb(file), jpeg)?;
            }
        }
    }
    Ok(found
        .into_iter()
        .map(|file| {
            let path = thumb(&file);
            Photo { thumbnail_path: path.exists().then(|| path.to_string_lossy().into_owned()), file }
        })
        .collect())
}

/// FNV-1a: a stable name for a thumbnail.
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf29ce484222325, |hash, byte| (hash ^ byte as u64).wrapping_mul(0x100000001b3))
}
