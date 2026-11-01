//! Tether Buddy: the optional companion app (see buddy/). Installed and enabled over adb; it lets
//! notifications run their own action, which adb alone can't.

use std::fs;
use std::path::Path;

use crate::adb::Adb;
use crate::{apps, BuddyState, Result};

const APK: &[u8] = include_bytes!("../../buddy/buddy.apk");
/// buddy/AndroidManifest.xml's versionCode.
const VERSION: u32 = 1;
const PACKAGE: &str = "io.github.msaudtahir.tether.buddy";
const LISTENER: &str = "io.github.msaudtahir.tether.buddy/.Listener";

pub fn state(adb: &Adb, serial: &str) -> Result<BuddyState> {
    let out = adb.text(
        Some(serial),
        &["shell", &format!("dumpsys package {PACKAGE} | grep -m1 versionCode=; echo; settings get secure enabled_notification_listeners")],
    )?;
    Ok(parse_state(&out))
}

/// `out` is dumpsys's "versionCode=1 minSdk=21 ..." line (none if not installed), then the enabled
/// notification listeners ("pkg/cls:pkg/cls").
fn parse_state(out: &str) -> BuddyState {
    let installed = out.split_whitespace().find_map(|w| w.strip_prefix("versionCode=")).and_then(|v| v.parse::<u32>().ok());
    let enabled = out.lines().last().unwrap_or("").split(':').any(|c| c.trim().starts_with(&format!("{PACKAGE}/")));
    match installed {
        None => BuddyState::NotInstalled,
        Some(v) if v < VERSION => BuddyState::Outdated,
        Some(_) if !enabled => BuddyState::Disabled,
        Some(_) => BuddyState::Ready,
    }
}

/// Installs (or updates) Buddy and enables it as a notification listener.
pub fn install(adb: &Adb, data_dir: &Path, serial: &str) -> Result<()> {
    let apk = data_dir.join("buddy.apk");
    fs::create_dir_all(data_dir)?;
    fs::write(&apk, APK)?;
    adb.install(serial, &apk.to_string_lossy())?;
    adb.shell(serial, &format!("cmd notification allow_listener {LISTENER}"))
}

/// Runs the notification's own action through Buddy; false if Buddy isn't ready or the
/// notification is gone.
pub fn open_notification(adb: &Adb, data_dir: &Path, serial: &str, key: &str) -> Result<bool> {
    // The key goes through the device shell: quote it.
    let quoted = format!("'{}'", key.replace('\'', r"'\''"));
    let out = apps::helper(adb, data_dir, serial, &["notification", &quoted])?;
    Ok(out.lines().any(|l| l.trim() == "ok"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states() {
        let installed = "    versionCode=1 minSdk=21 targetSdk=34\n\n";
        assert_eq!(parse_state("\n\ncom.other/.L"), BuddyState::NotInstalled);
        assert_eq!(parse_state(&format!("{installed}com.other/.L")), BuddyState::Disabled);
        assert_eq!(parse_state(&format!("{installed}com.other/.L:{LISTENER}")), BuddyState::Ready);
        assert_eq!(parse_state("    versionCode=0 minSdk=21\n\nnull"), BuddyState::Outdated);
    }
}
