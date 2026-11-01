//! Finds adb binaries and downloads/updates the bundled scrcpy build.

use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::util::{home, which, EXE};
use crate::{AdbCandidate, CoreError, Release, Result};

const RELEASES_API: &str = "https://api.github.com/repos/Genymobile/scrcpy/releases/latest";
/// Auto mode prefers Android Studio's adb so we don't kill the adb server it is using.
const ADB_PREFERENCE: [&str; 3] = ["studio", "system", "bundled"];

pub struct Toolchain {
    pub root: PathBuf,
}

impl Toolchain {
    pub fn installed_version(&self) -> Option<String> {
        fs::read_to_string(self.root.join("current")).ok().map(|v| v.trim().to_string())
    }

    pub fn bundled_file(&self, name: &str) -> Option<PathBuf> {
        let path = self.root.join(self.installed_version()?).join(format!("{name}{EXE}"));
        path.is_file().then_some(path)
    }

    /// `choice` is "" for the bundled build, or a custom path.
    pub fn resolve_scrcpy(&self, choice: &str) -> Option<PathBuf> {
        if !choice.is_empty() && Path::new(choice).is_file() {
            return Some(choice.into());
        }
        self.bundled_file("scrcpy").or_else(|| which("scrcpy"))
    }

    /// Available adb binaries keyed by source: "studio", "system", "bundled".
    pub fn adb_candidates(&self) -> Vec<AdbCandidate> {
        let mut found = Vec::new();
        let studio = android_sdk_dirs().into_iter().map(|sdk| sdk.join("platform-tools").join(format!("adb{EXE}")));
        if let Some(path) = studio.into_iter().find(|p| p.is_file()) {
            found.push(("studio", path));
        }
        if let Some(path) = which("adb") {
            found.push(("system", path));
        }
        if let Some(path) = self.bundled_file("adb") {
            found.push(("bundled", path));
        }
        found
            .into_iter()
            .map(|(source, path)| AdbCandidate { source: source.into(), path: path.to_string_lossy().into() })
            .collect()
    }

    /// `choice` is "auto", a source from adb_candidates(), or a custom path.
    pub fn resolve_adb(&self, choice: &str) -> Option<String> {
        let found = self.adb_candidates();
        let by_source = |source: &str| found.iter().find(|c| c.source == source).map(|c| c.path.clone());
        if let Some(path) = by_source(choice) {
            return Some(path);
        }
        if choice != "auto" && !ADB_PREFERENCE.contains(&choice) && Path::new(choice).is_file() {
            return Some(choice.into());
        }
        ADB_PREFERENCE.into_iter().find_map(by_source)
    }

    /// The newest scrcpy release for this platform, or None if there's no prebuilt one.
    pub fn latest_release(&self) -> Result<Option<Release>> {
        let Some(target) = asset_platform() else { return Ok(None) };
        let release: serde_json::Value = serde_json::from_slice(&get(RELEASES_API)?)?;
        let assets: Vec<(&str, &str)> = release["assets"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|a| Some((a["name"].as_str()?, a["browser_download_url"].as_str()?)))
            .collect();
        let Some(&(asset, url)) = assets.iter().find(|(name, _)| name.starts_with(&format!("scrcpy-{target}-"))) else {
            return Ok(None);
        };
        let sums_url = assets.iter().find(|(name, _)| *name == "SHA256SUMS.txt").map(|(_, url)| *url);
        let sums = String::from_utf8_lossy(&get(sums_url.ok_or_else(|| fail("Release has no checksums"))?)?).into_owned();
        let sha256 = sums
            .lines()
            .find(|line| line.ends_with(asset))
            .and_then(|line| line.split_whitespace().next())
            .ok_or_else(|| fail("Release has no checksum for this platform"))?;
        Ok(Some(Release {
            version: release["tag_name"].as_str().unwrap_or_default().trim_start_matches('v').into(),
            url: url.into(),
            asset: asset.into(),
            sha256: sha256.into(),
        }))
    }

    pub fn install(&self, release: &Release) -> Result<()> {
        let data = get(&release.url)?;
        if format!("{:x}", Sha256::digest(&data)) != release.sha256 {
            return Err(fail(&format!("Checksum mismatch for {}", release.asset)));
        }

        let tmp = self.root.join(format!(".{}.tmp", release.version));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp)?;
        if release.asset.ends_with(".zip") {
            zip::ZipArchive::new(Cursor::new(data))?.extract(&tmp)?;
        } else {
            tar::Archive::new(flate2::read::GzDecoder::new(Cursor::new(data))).unpack(&tmp)?;
        }

        // Archives contain a single top-level folder, e.g. scrcpy-macos-aarch64-v5.0.1/
        let inner = fs::read_dir(&tmp)?.next().ok_or_else(|| fail("The scrcpy archive is empty"))??.path();
        let dest = self.root.join(&release.version);
        let _ = fs::remove_dir_all(&dest);
        fs::rename(inner, &dest)?;
        fs::remove_dir_all(&tmp)?;
        fs::write(self.root.join("current"), &release.version)?;

        for old in fs::read_dir(&self.root)?.flatten() {
            if old.path().is_dir() && old.path() != dest {
                let _ = fs::remove_dir_all(old.path());
            }
        }
        Ok(())
    }
}

fn get(url: &str) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    ureq::get(url).set("User-Agent", "tether").call()?.into_reader().read_to_end(&mut data)?;
    Ok(data)
}

fn fail(message: &str) -> CoreError {
    CoreError::Failed(message.into())
}

/// The scrcpy release asset name for this OS/CPU, or None if there's no prebuilt one.
fn asset_platform() -> Option<&'static str> {
    let arm = std::env::consts::ARCH == "aarch64";
    match std::env::consts::OS {
        "macos" => Some(if arm { "macos-aarch64" } else { "macos-x86_64" }),
        "windows" => Some(if arm { "winarm64" } else { "win64" }),
        "linux" if std::env::consts::ARCH == "x86_64" => Some("linux-x86_64"),
        _ => None,
    }
}

fn android_sdk_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> =
        ["ANDROID_HOME", "ANDROID_SDK_ROOT"].iter().filter_map(|v| std::env::var_os(v)).map(PathBuf::from).collect();
    if cfg!(target_os = "macos") {
        dirs.extend(home().map(|h| h.join("Library/Android/sdk")));
    } else if cfg!(windows) {
        dirs.extend(std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Android/Sdk")));
    } else {
        dirs.extend(home().map(|h| h.join("Android/Sdk")));
    }
    dirs
}
