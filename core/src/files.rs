//! Browsing and moving files on the phone's storage over adb.

use std::path::Path;
use std::time::Duration;

use crate::adb::Adb;
use crate::{CoreError, PhoneFile, Result};

/// Copies can be big (videos, backups).
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(3600);

/// Quotes a path for the phone's shell.
fn quote(path: &str) -> String {
    format!("'{}'", path.replace('\'', r"'\''"))
}

pub fn list(adb: &Adb, serial: &str, dir: &str, hidden: bool) -> Result<Vec<PhoneFile>> {
    let dir = dir.trim_end_matches('/');
    let dir = if dir.is_empty() { "/" } else { dir };
    // `cd` first so a missing or unreadable folder is an error, not an empty list.
    let base = if dir == "/" { String::new() } else { quote(dir) };
    let globs = if hidden { format!("{base}/* {base}/.[!.]*") } else { format!("{base}/*") };
    let command = format!("cd {} || exit 1; stat -L -c '%s|%Y|%F|%n' -- {globs} 2>/dev/null; true", quote(dir));
    let out = adb.text(Some(serial), &["shell", &command]).map_err(|e| {
        // "/system/bin/sh: cd: /path: No such file or directory" → "No such file or directory"
        let message = e.to_string();
        let reason = message.rsplit(": ").next().unwrap_or(&message).to_string();
        CoreError::Failed(format!("Couldn't open {dir}: {reason}"))
    })?;
    Ok(parse_list(&out))
}

/// Lines of `stat -L -c '%s|%Y|%F|%n'`: size, modified (seconds), type, path. Folders first, then by name.
pub fn parse_list(out: &str) -> Vec<PhoneFile> {
    let mut files: Vec<PhoneFile> = out
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(4, '|');
            let (size, modified, kind, path) = (fields.next()?, fields.next()?, fields.next()?, fields.next()?);
            let name = path.rsplit('/').next()?.to_string();
            if name.is_empty() || name.contains('*') {
                return None; // an empty folder's unmatched glob
            }
            Some(PhoneFile {
                name,
                path: path.to_string(),
                folder: kind == "directory",
                size: size.parse().unwrap_or(0),
                modified: modified.parse().unwrap_or(0),
            })
        })
        .collect();
    files.sort_by(|a, b| b.folder.cmp(&a.folder).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    files
}

/// Copies a file or folder from the phone into `local_dir`; returns the local path.
pub fn download(adb: &Adb, serial: &str, remote: &str, local_dir: &Path) -> Result<String> {
    std::fs::create_dir_all(local_dir)?;
    let name = remote.trim_end_matches('/').rsplit('/').next().unwrap_or("download");
    let target = unique(&local_dir.join(name));
    adb.run(Some(serial), &["pull", remote, &target.to_string_lossy()], TRANSFER_TIMEOUT)?;
    Ok(target.to_string_lossy().into_owned())
}

/// A path that doesn't exist yet: "name", else "name 2", "name 3", ...
fn unique(path: &Path) -> std::path::PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..).map(|n| path.with_file_name(format!("{stem} {n}{ext}"))).find(|p| !p.exists()).unwrap()
}

/// Copies a local file or folder into `remote_dir` on the phone.
pub fn upload(adb: &Adb, serial: &str, local: &str, remote_dir: &str) -> Result<()> {
    let target = format!("{}/", remote_dir.trim_end_matches('/'));
    adb.run(Some(serial), &["push", local, &target], TRANSFER_TIMEOUT)?;
    // Let the phone's gallery and file apps see new media.
    let name = Path::new(local).file_name().unwrap_or_default().to_string_lossy();
    let _ = adb.shell(serial, &format!("am broadcast -a android.intent.action.MEDIA_SCANNER_SCAN_FILE -d {} >/dev/null", quote(&format!("file://{target}{name}"))));
    Ok(())
}

pub fn make_folder(adb: &Adb, serial: &str, path: &str) -> Result<()> {
    adb.shell(serial, &format!("mkdir -p -- {}", quote(path)))
}

pub fn rename(adb: &Adb, serial: &str, from: &str, to: &str) -> Result<()> {
    adb.shell(serial, &format!("[ ! -e {to} ] && mv -- {from} {to}", from = quote(from), to = quote(to)))
        .map_err(|_| CoreError::Failed("Couldn't rename: something with that name may already exist.".into()))
}

pub fn delete(adb: &Adb, serial: &str, paths: &[String]) -> Result<()> {
    let quoted: Vec<String> = paths.iter().map(|p| quote(p)).collect();
    adb.shell(serial, &format!("rm -rf -- {}", quoted.join(" ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listing() {
        let out = "130437515|1656877299|regular file|/sdcard/a b.mpkg\n3488|1678726354|directory|/sdcard/Zed\n3488|1647954380|directory|/sdcard/alarms\n10|1|regular file|/sdcard/x|y.txt\n";
        let files = parse_list(out);
        let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["alarms", "Zed", "a b.mpkg", "x|y.txt"]);
        assert!(files[0].folder && !files[2].folder);
        assert_eq!((files[2].size, files[2].modified, files[2].path.as_str()), (130437515, 1656877299, "/sdcard/a b.mpkg"));
        assert!(parse_list("0|0|regular file|/sdcard/empty/*\n").is_empty());
        assert_eq!(quote("it's"), r"'it'\''s'");
    }
}
