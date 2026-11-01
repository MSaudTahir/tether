//! Installed apps and their icons, cached on disk per device until a forced refresh.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;

use crate::adb::Adb;
use crate::util::{command, output, text};
use crate::{DeviceApp, CoreError, Result};

/// Lists launchable apps with icons on the device; built from helper/Helper.java.
const HELPER: &[u8] = include_bytes!("../../helper/helper.dex");
const DEVICE_HELPER: &str = "/data/local/tmp/tether-helper.dex";

pub fn load(adb: &Adb, scrcpy: Option<&Path>, data_dir: &Path, serial: &str, force: bool) -> Result<Vec<DeviceApp>> {
    if !force {
        if let Some(apps) = cached(data_dir, serial) {
            return Ok(apps);
        }
    }
    let folder = cache_folder(data_dir, serial);
    let index = folder.join("apps.json");

    let _ = fs::remove_dir_all(&folder);
    let mut apps = match fetch_with_icons(adb, data_dir, serial, &folder.join("icons")) {
        Ok(apps) if !apps.is_empty() => apps,
        // The helper relies on Android internals; scrcpy's list still works, just without icons.
        _ => match scrcpy {
            Some(scrcpy) => list_with_scrcpy(scrcpy, &adb.path, serial)?,
            None => Vec::new(),
        },
    };
    if apps.is_empty() {
        return Err(CoreError::Failed("No apps found on the device.".into()));
    }

    apps.sort_by_key(|app| app.name.to_lowercase());
    fs::create_dir_all(&folder)?;
    fs::write(&index, serde_json::to_vec(&apps)?)?;
    Ok(apps)
}

/// The cached app list, if there is a readable one (an older format is simply fetched again).
pub fn cached(data_dir: &Path, serial: &str) -> Option<Vec<DeviceApp>> {
    let folder = cache_folder(data_dir, serial);
    let mut apps: Vec<DeviceApp> = serde_json::from_slice(&fs::read(folder.join("apps.json")).ok()?).ok()?;
    // Icon paths are rebuilt so the data folder can move.
    for app in &mut apps {
        let path = folder.join("icons").join(format!("{}.png", app.package));
        app.icon_path = path.exists().then(|| path.to_string_lossy().into_owned());
    }
    Some(apps)
}

fn cache_folder(data_dir: &Path, serial: &str) -> PathBuf {
    let safe: String = serial.chars().map(|c| if c.is_alphanumeric() || ".-_".contains(c) { c } else { '_' }).collect();
    data_dir.join("apps").join(safe)
}

fn fetch_with_icons(adb: &Adb, data_dir: &Path, serial: &str, icon_dir: &Path) -> Result<Vec<DeviceApp>> {
    run_helper(adb, data_dir, serial, &[], icon_dir)
}

/// Pushes the helper and runs it with `args`; returns its output (which may include OEM log noise).
pub fn helper(adb: &Adb, data_dir: &Path, serial: &str, args: &[&str]) -> Result<String> {
    let local = data_dir.join("helper.dex");
    fs::write(&local, HELPER)?;
    adb.run(Some(serial), &["push", &local.to_string_lossy(), DEVICE_HELPER], Duration::from_secs(30))?;
    let command = format!("CLASSPATH={DEVICE_HELPER} app_process / tether.Helper {}", args.join(" "));
    Ok(text(&adb.run(Some(serial), &["exec-out", command.trim()], Duration::from_secs(120))?))
}

/// Runs the helper with `args`, saving icons to `icon_dir`.
fn run_helper(adb: &Adb, data_dir: &Path, serial: &str, args: &[&str], icon_dir: &Path) -> Result<Vec<DeviceApp>> {
    let out = helper(adb, data_dir, serial, args)?;

    fs::create_dir_all(icon_dir)?;
    let mut apps = Vec::new();
    // Lines are "package \t system \t label \t base64 png". Anything else is OEM log noise.
    for line in out.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        let [package, system, name, icon] = fields[..] else { continue };
        let mut icon_path = None;
        if let Ok(png) = base64::engine::general_purpose::STANDARD.decode(icon) {
            if !png.is_empty() {
                let path: PathBuf = icon_dir.join(format!("{package}.png"));
                fs::write(&path, png)?;
                icon_path = Some(path.to_string_lossy().into_owned());
            }
        }
        apps.push(DeviceApp { name: name.into(), package: package.into(), system: system == "1", icon_path });
    }
    Ok(apps)
}

/// Names and icons for packages that aren't launchable apps (e.g. "android", which posts system
/// notifications). Fetched once per package and kept next to the app cache; a package the phone
/// doesn't know is remembered too, with an empty name, so it isn't asked for again.
pub fn other_packages(adb: &Adb, data_dir: &Path, serial: &str, packages: &[String]) -> Vec<DeviceApp> {
    let folder = cache_folder(data_dir, serial);
    let index = folder.join("other.json");
    let mut known: Vec<DeviceApp> = fs::read(&index).ok().and_then(|json| serde_json::from_slice(&json).ok()).unwrap_or_default();
    let missing: Vec<&str> = packages
        .iter()
        .filter(|p| !known.iter().any(|k| &k.package == *p) && p.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '_'))
        .map(String::as_str)
        .collect();
    if !missing.is_empty() {
        let args: Vec<&str> = ["packages"].into_iter().chain(missing.iter().copied()).collect();
        if let Ok(found) = run_helper(adb, data_dir, serial, &args, &folder.join("icons")) {
            for package in &missing {
                let app = found.iter().find(|a| a.package == *package).cloned();
                known.push(app.unwrap_or(DeviceApp { name: String::new(), package: package.to_string(), system: true, icon_path: None }));
            }
            let _ = fs::create_dir_all(&folder).and_then(|_| fs::write(&index, serde_json::to_vec(&known).unwrap_or_default()));
        }
    }
    for app in &mut known {
        let path = folder.join("icons").join(format!("{}.png", app.package));
        app.icon_path = path.exists().then(|| path.to_string_lossy().into_owned());
    }
    known
}

fn list_with_scrcpy(scrcpy: &Path, adb_path: &str, serial: &str) -> Result<Vec<DeviceApp>> {
    let mut cmd = command(&scrcpy.to_string_lossy());
    cmd.args(["-s", serial, "--list-apps"]).env("ADB", adb_path);
    let out = output(cmd, Duration::from_secs(90))?;
    let all = text(&out.stdout) + &text(&out.stderr);
    let apps = parse_scrcpy_list(&all);
    if !out.status.success() && apps.is_empty() {
        let errors: Vec<&str> = all.lines().filter(|l| l.contains("ERROR")).collect();
        return Err(CoreError::Failed(errors.join("\n")));
    }
    Ok(apps)
}

/// Lines look like " * Name<padding> com.package" (* = system app, - = user app).
/// Names of 30+ chars put the package on the next line, indented.
fn parse_scrcpy_list(out: &str) -> Vec<DeviceApp> {
    let mut apps = Vec::new();
    let mut pending: Option<(bool, String)> = None;
    for line in out.lines() {
        if line.starts_with("    ") && !line.trim().is_empty() {
            if let Some((system, name)) = pending.take() {
                apps.push(DeviceApp { name, package: line.trim().into(), system, icon_path: None });
            }
        } else if let Some(rest) = line.strip_prefix(" * ").or_else(|| line.strip_prefix(" - ")) {
            let system = line.starts_with(" * ");
            match rest.rfind("  ") {
                Some(i) => apps.push(DeviceApp {
                    name: rest[..i].trim().into(),
                    package: rest[i..].trim().into(),
                    system,
                    icon_path: None,
                }),
                None => pending = Some((system, rest.trim().into())),
            }
        }
    }
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrcpy_list() {
        let out = "[server] INFO: List of apps:\n \
                   * Camera                         com.android.camera2\n \
                   - Slack                          com.Slack\n \
                   - A Very Long Application Name Here\n                                  com.example.longname\n";
        let apps = parse_scrcpy_list(out);
        let got: Vec<(&str, &str, bool)> = apps.iter().map(|a| (a.name.as_str(), a.package.as_str(), a.system)).collect();
        assert_eq!(
            got,
            [
                ("Camera", "com.android.camera2", true),
                ("Slack", "com.Slack", false),
                ("A Very Long Application Name Here", "com.example.longname", false)
            ]
        );
    }
}
