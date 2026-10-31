//! Shared logic for the MyDevice apps, exposed to Swift/C# through UniFFI.
//!
//! All methods block (they run adb, scrcpy or downloads), so UIs call them off the main thread.

mod adb;
mod apps;
mod toolchain;
mod util;

use std::fs;
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use adb::Adb;
use serde::{Deserialize, Serialize};
use toolchain::Toolchain;
use util::{command, read_in_background, text};

uniffi::setup_scaffolding!();

pub type Result<T, E = CoreError> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum CoreError {
    #[error("{0}")]
    Failed(String),
}

macro_rules! from_error {
    ($($t:ty),*) => {$(
        impl From<$t> for CoreError {
            fn from(e: $t) -> Self {
                CoreError::Failed(e.to_string())
            }
        }
    )*};
}
from_error!(std::io::Error, serde_json::Error, zip::result::ZipError);

impl From<ureq::Error> for CoreError {
    fn from(e: ureq::Error) -> Self {
        CoreError::Failed(format!("Download failed: {e}"))
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Device {
    pub serial: String,
    /// "device", "unauthorized", "offline", "no permissions", ...
    pub state: String,
    pub model: String,
    /// The model, or the serial if adb doesn't report one.
    pub label: String,
    pub wireless: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct DeviceInfo {
    pub name: String,
    pub model: String,
    pub android: String,
    pub sdk: u32,
    pub battery: u32,
    pub power: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct DeviceApp {
    pub name: String,
    pub package: String,
    pub system: bool,
    /// A cached PNG, if the device provided an icon.
    pub icon_path: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Release {
    pub version: String,
    pub url: String,
    pub asset: String,
    pub sha256: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AdbCandidate {
    /// "studio", "system" or "bundled".
    pub source: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
#[serde(default)]
pub struct Config {
    /// "auto", "studio", "system", "bundled" or a path.
    pub adb: String,
    /// "" for the bundled build, or a path.
    pub scrcpy: String,
    /// Virtual display size for app windows: "WIDTHxHEIGHT/DPI", or "" for the device's size.
    pub window_size: String,
    pub setup_done: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config { adb: "auto".into(), scrcpy: String::new(), window_size: "1920x1080/280".into(), setup_done: false }
    }
}

/// Android 10 is needed for scrcpy's virtual displays.
pub const VIRTUAL_DISPLAY_MIN_SDK: u32 = 29;

#[derive(uniffi::Object)]
pub struct Core {
    data_dir: PathBuf,
    tools: Toolchain,
    config: Mutex<Config>,
    sessions: Mutex<Vec<Arc<Mutex<Child>>>>,
}

#[uniffi::export]
impl Core {
    /// `data_dir` is the app's data folder (scrcpy, the app cache and config live there).
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> Arc<Self> {
        let data_dir = PathBuf::from(data_dir);
        let config = fs::read(data_dir.join("config.json"))
            .ok()
            .and_then(|json| serde_json::from_slice(&json).ok())
            .unwrap_or_default();
        Arc::new(Core {
            tools: Toolchain { root: data_dir.join("scrcpy") },
            data_dir,
            config: Mutex::new(config),
            sessions: Mutex::new(Vec::new()),
        })
    }

    // --- config & toolchain ---------------------------------------------

    pub fn config(&self) -> Config {
        self.config.lock().unwrap().clone()
    }

    pub fn save_config(&self, config: Config) -> Result<()> {
        fs::create_dir_all(&self.data_dir)?;
        fs::write(self.data_dir.join("config.json"), serde_json::to_vec_pretty(&config)?)?;
        *self.config.lock().unwrap() = config;
        Ok(())
    }

    /// True until setup has been completed and a scrcpy is available.
    pub fn needs_setup(&self) -> bool {
        !self.config().setup_done || self.scrcpy_path().is_none()
    }

    pub fn adb_path(&self) -> Option<String> {
        self.tools.resolve_adb(&self.config().adb)
    }

    pub fn scrcpy_path(&self) -> Option<String> {
        self.tools.resolve_scrcpy(&self.config().scrcpy).map(|p| p.to_string_lossy().into())
    }

    /// The downloaded scrcpy's version, if one is installed.
    pub fn bundled_scrcpy_version(&self) -> Option<String> {
        self.tools.installed_version()
    }

    pub fn adb_candidates(&self) -> Vec<AdbCandidate> {
        self.tools.adb_candidates()
    }

    pub fn resolve_adb(&self, choice: String) -> Option<String> {
        self.tools.resolve_adb(&choice)
    }

    pub fn latest_release(&self) -> Result<Option<Release>> {
        self.tools.latest_release()
    }

    /// A newer scrcpy than the downloaded one, if the downloaded one is in use.
    pub fn available_update(&self) -> Result<Option<Release>> {
        let Some(current) = self.tools.installed_version() else { return Ok(None) };
        if !self.config().scrcpy.is_empty() {
            return Ok(None);
        }
        Ok(self.tools.latest_release()?.filter(|release| release.version != current))
    }

    /// Downloads and installs a scrcpy release (checksum-verified), replacing older ones.
    pub fn install_scrcpy(&self, release: Release) -> Result<()> {
        self.tools.install(&release)
    }

    // --- devices ----------------------------------------------------------

    pub fn devices(&self) -> Result<Vec<Device>> {
        self.adb()?.devices()
    }

    pub fn device_info(&self, serial: String) -> Result<DeviceInfo> {
        self.adb()?.info(&serial)
    }

    /// Launchable apps with icons: from the disk cache, or from the device if `force` or not cached yet.
    pub fn apps(&self, serial: String, force: bool) -> Result<Vec<DeviceApp>> {
        let scrcpy = self.scrcpy_path().map(PathBuf::from);
        apps::load(&self.adb()?, scrcpy.as_deref(), &self.data_dir, &serial, force)
    }

    pub fn pair(&self, address: String, code: String) -> Result<()> {
        self.adb()?.pair(&address, &code)
    }

    pub fn connect(&self, address: String) -> Result<()> {
        self.adb()?.connect(&address)
    }

    /// Switches a USB device to Wi-Fi. Returns the new address.
    pub fn go_wireless(&self, serial: String) -> Result<String> {
        self.adb()?.go_wireless(&serial)
    }

    // --- scrcpy sessions --------------------------------------------------

    /// Mirrors the device's screen in a scrcpy window. Blocks until the window closes.
    pub fn mirror(&self, serial: String, title: String) -> Result<()> {
        self.run_scrcpy(vec!["-s".into(), serial, format!("--window-title={title}")])
    }

    /// Opens an app on a new virtual display (sized by the window_size setting) in its own window.
    /// Blocks until the window closes.
    pub fn open_app(&self, serial: String, package: String, title: String) -> Result<()> {
        let size = self.config().window_size;
        if !valid_window_size(&size) {
            return Err(CoreError::Failed("Window size must look like 1920x1080/280.".into()));
        }
        let display = if size.is_empty() { "--new-display".into() } else { format!("--new-display={size}") };
        self.run_scrcpy(vec!["-s".into(), serial, display, format!("--start-app={package}"), format!("--window-title={title}")])
    }

    /// Closes all scrcpy windows (call this when the app quits).
    pub fn stop_sessions(&self) {
        for session in self.sessions.lock().unwrap().drain(..) {
            let _ = session.lock().unwrap().kill();
        }
    }
}

impl Core {
    fn adb(&self) -> Result<Adb> {
        Ok(Adb { path: self.adb_path().ok_or_else(no_adb)? })
    }

    fn run_scrcpy(&self, args: Vec<String>) -> Result<()> {
        let scrcpy = self.scrcpy_path().ok_or_else(|| CoreError::Failed("scrcpy isn't installed. Open Setup.".into()))?;
        let adb = self.adb_path().ok_or_else(no_adb)?;
        let mut child = command(&scrcpy).args(&args).env("ADB", adb).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
        let stdout = read_in_background(child.stdout.take());
        let stderr = read_in_background(child.stderr.take());
        let session = Arc::new(Mutex::new(child));
        self.sessions.lock().unwrap().push(session.clone());

        let status = loop {
            if let Some(status) = session.lock().unwrap().try_wait()? {
                break status;
            }
            thread::sleep(Duration::from_millis(200));
        };
        self.sessions.lock().unwrap().retain(|s| !Arc::ptr_eq(s, &session));
        if status.success() {
            return Ok(());
        }
        let output = text(&stdout.join().unwrap_or_default()) + &text(&stderr.join().unwrap_or_default());
        let errors: Vec<&str> = output.lines().filter(|l| l.contains("ERROR")).collect();
        Err(CoreError::Failed(if errors.is_empty() { format!("scrcpy exited with {status}") } else { errors.join("\n") }))
    }
}

fn no_adb() -> CoreError {
    CoreError::Failed("No adb found. Open Setup to choose one.".into())
}

/// "" (device size), "WxH", "/DPI" or "WxH/DPI".
#[uniffi::export]
pub fn valid_window_size(size: &str) -> bool {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let (dims, dpi) = size.split_once('/').map_or((size, None), |(d, p)| (d, Some(p)));
    let dims_ok = dims.is_empty() || dims.split_once('x').is_some_and(|(w, h)| digits(w) && digits(h));
    dims_ok && dpi.is_none_or(digits)
}

#[uniffi::export]
pub fn virtual_display_min_sdk() -> u32 {
    VIRTUAL_DISPLAY_MIN_SDK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_sizes() {
        for ok in ["", "1920x1080", "1920x1080/280", "/240"] {
            assert!(valid_window_size(ok), "{ok}");
        }
        for bad in ["1920", "1920x", "x1080", "1920x1080/", "abc", "1920x1080/2a"] {
            assert!(!valid_window_size(bad), "{bad}");
        }
    }
}
