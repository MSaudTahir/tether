//! Shared logic for the Tether apps, exposed to Swift/C# through UniFFI.
//!
//! All methods block (they run adb, scrcpy or downloads), so UIs call them off the main thread.

mod adb;
mod apps;
mod buddy;
mod files;
mod notifications;
mod photos;
mod session;
mod toolchain;
mod util;
mod window;

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use adb::Adb;
use serde::{Deserialize, Serialize};
pub use session::Session;
use toolchain::Toolchain;
use util::text;
pub use window::*;

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
    /// The serial for display: Wi-Fi pairings look like "adb-SERIAL-xxxx._adb-tls-connect._tcp".
    pub short_serial: String,
    pub wireless: bool,
}

/// Details for a device's page. Empty strings and zeros mean the device didn't say.
#[derive(Debug, Clone, uniffi::Record)]
pub struct DeviceInfo {
    pub name: String,
    pub model: String,
    pub android: String,
    pub sdk: u32,
    /// "2023-03-01"
    pub security_patch: String,
    /// "Qualcomm SM8250"
    pub chipset: String,
    pub battery: u32,
    /// "charging (AC)", "full", "on battery"
    pub power: String,
    /// "good", "overheating", ...
    pub battery_health: String,
    /// °C
    pub battery_temperature: f32,
    /// Bytes.
    pub memory_total: u64,
    pub memory_available: u64,
    pub storage_total: u64,
    pub storage_used: u64,
    /// "1080 × 2400 · 440 dpi"
    pub screen: String,
    pub uptime_seconds: u64,
    pub wifi_address: Option<String>,
    pub dark_mode: bool,
    /// Screen stays on while charging (developer option).
    pub stay_awake: bool,
    pub screen_on: bool,
    pub do_not_disturb: bool,
}

/// What a phone's Android version supports, so UIs can hide or explain what won't work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct Capabilities {
    /// Apps in their own windows (virtual displays): Android 10.
    pub app_windows: bool,
    /// Phone audio on the computer: Android 11.
    pub audio: bool,
    /// Pairing over Wi-Fi with a code: Android 11.
    pub wifi_pairing: bool,
    /// System dark theme: Android 10.
    pub dark_mode: bool,
    /// Do Not Disturb from adb: Android 9.
    pub do_not_disturb: bool,
}

#[uniffi::export]
pub fn device_capabilities(sdk: u32) -> Capabilities {
    Capabilities { app_windows: sdk >= 29, audio: sdk >= 30, wifi_pairing: sdk >= 30, dark_mode: sdk >= 29, do_not_disturb: sdk >= 28 }
}

/// A notification showing on the phone.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PhoneNotification {
    /// Unique while the notification is showing.
    pub key: String,
    pub package: String,
    /// From the app list cache; empty if the app isn't in it (e.g. system notifications).
    pub app_name: String,
    pub icon_path: Option<String>,
    pub title: String,
    pub text: String,
    /// Milliseconds since 1970, or 0 if the notification doesn't say.
    pub time: u64,
}

/// A file or folder on the phone.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PhoneFile {
    pub name: String,
    pub path: String,
    pub folder: bool,
    /// Bytes (folders: the folder entry's own size, not its contents).
    pub size: u64,
    /// Seconds since 1970.
    pub modified: i64,
}

/// A recent photo or video, with a small JPEG thumbnail if one could be made.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Photo {
    pub file: PhoneFile,
    pub thumbnail_path: Option<String>,
}

/// Where the phone's own files (Download, DCIM, Pictures, ...) live.
pub const PHONE_STORAGE: &str = "/sdcard";

/// Whether Tether Buddy (the optional companion app) is on the phone and enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BuddyState {
    NotInstalled,
    /// Installed, but older than the one Tether carries.
    Outdated,
    /// Installed, but not allowed to read notifications.
    Disabled,
    Ready,
}

/// Buttons a phone has, for remote control.
#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum DeviceKey {
    Power,
    VolumeUp,
    VolumeDown,
    Home,
    Back,
    RecentApps,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum RebootMode {
    Normal,
    Recovery,
    Bootloader,
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
    /// App window size in points and density per point: "WIDTHxHEIGHT/DPI", or "" for the phone's size.
    pub window_size: String,
    /// Resizing an app window resizes its display, so the app relayouts (window_size is the starting size).
    pub resizable_windows: bool,
    /// Play the phone's audio on the computer (Android 11+).
    pub forward_audio: bool,
    /// Turn the phone's screen off while it's mirrored or shows app windows (it stays on for them).
    pub screen_off: bool,
    /// Send the mouse as a real (UHID) mouse, captured in relative mode: for games and apps that use
    /// pointer capture (Android 8+). Left Alt or Cmd gives the mouse back to the computer.
    /// App windows turn on the device's "Force desktop mode" so the mouse lands on their display.
    /// Off at every launch.
    pub capture_mouse: bool,
    pub setup_done: bool,
    /// Wi-Fi addresses (ip:port) to reconnect to at launch.
    pub known_addresses: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            adb: "auto".into(),
            scrcpy: String::new(),
            window_size: "1280x720/160".into(),
            resizable_windows: true,
            forward_audio: true,
            screen_off: false,
            capture_mouse: false,
            setup_done: false,
            known_addresses: Vec::new(),
        }
    }
}

/// Android 10 is needed for scrcpy's virtual displays.
pub const VIRTUAL_DISPLAY_MIN_SDK: u32 = 29;

#[derive(uniffi::Object)]
pub struct Core {
    data_dir: PathBuf,
    tools: Toolchain,
    config: Mutex<Config>,
    sessions: Mutex<Vec<Arc<Session>>>,
    /// The open mirror window per phone (serial).
    mirrors: Mutex<Vec<(String, Arc<Session>)>>,
}

#[uniffi::export]
impl Core {
    /// `data_dir` is the app's data folder (scrcpy, the app cache and config live there).
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> Arc<Self> {
        let data_dir = PathBuf::from(data_dir);
        let mut config: Config = fs::read(data_dir.join("config.json"))
            .ok()
            .and_then(|json| serde_json::from_slice(&json).ok())
            .unwrap_or_default();
        // A per-session switch (it changes how the mouse behaves), so every launch starts with it off.
        config.capture_mouse = false;
        Arc::new(Core {
            tools: Toolchain { root: data_dir.join("scrcpy") },
            data_dir,
            config: Mutex::new(config),
            sessions: Mutex::new(Vec::new()),
            mirrors: Mutex::new(Vec::new()),
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

    /// The phone's current notifications, newest first (ongoing ones, like music players, are left out).
    pub fn notifications(&self, serial: String) -> Result<Vec<PhoneNotification>> {
        let adb = self.adb()?;
        let out = adb.text(Some(&serial), &["shell", "dumpsys notification --noredact"])?;
        let mut list = notifications::parse(&out);
        let mut apps = apps::cached(&self.data_dir, &serial).unwrap_or_default();
        // Senders that aren't launchable apps (Android System, System UI, ...) are looked up once.
        let others: Vec<String> = list.iter().map(|n| n.package.clone()).filter(|p| !apps.iter().any(|a| &a.package == p)).collect();
        if !others.is_empty() {
            apps.extend(apps::other_packages(&adb, &self.data_dir, &serial, &others));
        }
        for notification in &mut list {
            if let Some(app) = apps.iter().find(|a| a.package == notification.package) {
                notification.app_name = app.name.clone();
                notification.icon_path = app.icon_path.clone();
            }
        }
        Ok(list)
    }

    /// Opens a notification: with Tether Buddy, runs its own action on the phone (e.g. opens that
    /// chat); without, pulls the notification shade down to tap it. Either way the UI then mirrors
    /// the phone. Returns whether the action ran.
    pub fn open_notification(&self, serial: String, key: String) -> Result<bool> {
        let adb = self.adb()?;
        if buddy::open_notification(&adb, &self.data_dir, &serial, &key).unwrap_or(false) {
            return Ok(true);
        }
        adb.shell(&serial, "cmd statusbar expand-notifications")?;
        Ok(false)
    }

    pub fn buddy_state(&self, serial: String) -> Result<BuddyState> {
        buddy::state(&self.adb()?, &serial)
    }

    /// Installs (or updates) Tether Buddy and enables it. Some phones ask to confirm on screen, and
    /// Xiaomi phones need "Install via USB" turned on in Developer options.
    pub fn install_buddy(&self, serial: String) -> Result<()> {
        buddy::install(&self.adb()?, &self.data_dir, &serial)
    }

    // --- files -------------------------------------------------------------

    /// A folder's contents, folders first. `hidden` includes names starting with a dot.
    pub fn list_files(&self, serial: String, path: String, hidden: bool) -> Result<Vec<PhoneFile>> {
        files::list(&self.adb()?, &serial, &path, hidden)
    }

    /// The newest photos and videos from the camera and screenshots, newest first, with thumbnails
    /// (made on the phone the first time, then cached).
    pub fn recent_photos(&self, serial: String, count: u32) -> Result<Vec<Photo>> {
        photos::recent(&self.adb()?, &self.data_dir, &serial, count as usize)
    }

    /// Copies a file or folder from the phone into `local_dir` (as "name 2" etc. if taken); returns
    /// the copy's path.
    pub fn download_file(&self, serial: String, path: String, local_dir: String) -> Result<String> {
        files::download(&self.adb()?, &serial, &path, std::path::Path::new(&local_dir))
    }

    /// Copies a local file or folder into a folder on the phone.
    pub fn upload_file(&self, serial: String, local_path: String, folder: String) -> Result<()> {
        files::upload(&self.adb()?, &serial, &local_path, &folder)
    }

    pub fn make_folder(&self, serial: String, path: String) -> Result<()> {
        files::make_folder(&self.adb()?, &serial, &path)
    }

    pub fn rename_file(&self, serial: String, from: String, to: String) -> Result<()> {
        files::rename(&self.adb()?, &serial, &from, &to)
    }

    /// Deletes files and folders (with everything in them).
    pub fn delete_files(&self, serial: String, paths: Vec<String>) -> Result<()> {
        files::delete(&self.adb()?, &serial, &paths)
    }

    /// The phone's clipboard text; None if it's empty or the phone doesn't let adb read it in the
    /// background (some, e.g. Xiaomi's, don't).
    pub fn phone_clipboard(&self, serial: String) -> Result<Option<String>> {
        let out = apps::helper(&self.adb()?, &self.data_dir, &serial, &["clipboard"])?;
        // The answer is the last line that's valid base64; other lines are OEM log noise.
        let text = out
            .lines()
            .rev()
            .filter_map(|line| base64::Engine::decode(&base64::engine::general_purpose::STANDARD, line.trim()).ok())
            .next()
            .and_then(|bytes| String::from_utf8(bytes).ok());
        Ok(text.filter(|t| !t.is_empty()))
    }

    pub fn set_phone_clipboard(&self, serial: String, text: String) -> Result<()> {
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, text.as_bytes());
        let out = apps::helper(&self.adb()?, &self.data_dir, &serial, &["clipboard", &encoded])?;
        if !out.lines().any(|l| l.trim() == "ok") {
            return Err(CoreError::Failed("Couldn't set the phone's clipboard.".into()));
        }
        Ok(())
    }

    pub fn pair(&self, address: String, code: String) -> Result<()> {
        self.adb()?.pair(&address, &code)
    }

    pub fn connect(&self, address: String) -> Result<()> {
        self.adb()?.connect(&address)?;
        self.remember_address(&address);
        Ok(())
    }

    /// Switches a USB device to Wi-Fi. Returns the new address, which is remembered for reconnecting.
    pub fn go_wireless(&self, serial: String) -> Result<String> {
        let address = self.adb()?.go_wireless(&serial)?;
        self.remember_address(&address);
        Ok(address)
    }

    /// Reconnects to phones connected over Wi-Fi before (by Go Wireless or Connect) that aren't
    /// connected now. Phones paired with a code reconnect by themselves.
    pub fn reconnect(&self) -> Result<()> {
        let adb = self.adb()?;
        let connected: Vec<String> = adb.devices()?.into_iter().map(|d| d.serial).collect();
        for address in self.config().known_addresses {
            if !connected.contains(&address) {
                let _ = adb.connect(&address); // the phone may be away; that's fine
            }
        }
        Ok(())
    }

    // --- apps ----------------------------------------------------------------

    /// Opens the app's page in the phone's Settings (e.g. to change its permissions).
    pub fn show_app_info(&self, serial: String, package: String) -> Result<()> {
        check_package(&package)?;
        self.adb()?.shell(&serial, &format!("am start -a android.settings.APPLICATION_DETAILS_SETTINGS -d package:{package} >/dev/null"))
    }

    pub fn force_stop(&self, serial: String, package: String) -> Result<()> {
        check_package(&package)?;
        self.adb()?.shell(&serial, &format!("am force-stop {package}"))
    }

    /// Deletes the app's data, like "Clear storage" in Settings.
    pub fn clear_app_data(&self, serial: String, package: String) -> Result<()> {
        check_package(&package)?;
        self.adb()?.shell(&serial, &format!("pm clear {package}"))
    }

    pub fn uninstall_app(&self, serial: String, package: String) -> Result<()> {
        check_package(&package)?;
        let out = text(&self.adb()?.run(Some(&serial), &["uninstall", &package], std::time::Duration::from_secs(60))?);
        if !out.contains("Success") {
            return Err(CoreError::Failed(format!("Couldn't uninstall: {}", out.trim())));
        }
        Ok(())
    }

    /// Opens a web link (or any URI) on the phone.
    pub fn open_link(&self, serial: String, link: String) -> Result<()> {
        let quoted = format!("'{}'", link.trim().replace('\'', r"'\''"));
        self.adb()?.shell(&serial, &format!("am start -a android.intent.action.VIEW -d {quoted} >/dev/null"))
    }

    // --- control -----------------------------------------------------------

    /// Saves a PNG of the screen to `path`.
    pub fn screenshot(&self, serial: String, path: String) -> Result<()> {
        self.adb()?.screenshot(&serial, &path)
    }

    pub fn install_apk(&self, serial: String, path: String) -> Result<()> {
        self.adb()?.install(&serial, &path)
    }

    pub fn press_key(&self, serial: String, key: DeviceKey) -> Result<()> {
        self.adb()?.press_key(&serial, key)
    }

    pub fn set_dark_mode(&self, serial: String, on: bool) -> Result<()> {
        self.adb()?.shell(&serial, &format!("cmd uimode night {}", if on { "yes" } else { "no" }))
    }

    /// Keeps the screen on while charging (the "Stay awake" developer option).
    pub fn set_stay_awake(&self, serial: String, on: bool) -> Result<()> {
        // 7 = on AC, USB and wireless charging.
        self.adb()?.shell(&serial, &format!("settings put global stay_on_while_plugged_in {}", if on { 7 } else { 0 }))
    }

    pub fn set_do_not_disturb(&self, serial: String, on: bool) -> Result<()> {
        self.adb()?.shell(&serial, &format!("cmd notification set_dnd {}", if on { "on" } else { "off" }))
    }

    pub fn reboot(&self, serial: String, mode: RebootMode) -> Result<()> {
        self.adb()?.reboot(&serial, mode)
    }

    // --- scrcpy sessions --------------------------------------------------

    /// Mirrors the device's screen in a scrcpy window.
    /// A phone has at most one mirror window: if it's open already, that window is returned (to bring
    /// to the front) instead of a new one. App windows can be as many as you like.
    pub fn mirror(&self, serial: String, title: String) -> Result<Arc<Session>> {
        if let Some(open) = self.mirrors.lock().unwrap().iter().find(|(s, session)| *s == serial && !session.finished()) {
            return Ok(open.1.clone());
        }
        let session = self.start_scrcpy(vec!["-s".into(), serial.clone(), format!("--window-title={title}")])?;
        let mut mirrors = self.mirrors.lock().unwrap();
        mirrors.retain(|(_, s)| !s.finished());
        mirrors.push((serial, session.clone()));
        Ok(session)
    }

    /// Opens an app in its own window, on a new virtual display sized by the window_size config and
    /// fitted to `screen` (the usable screen area the window opens on).
    pub fn open_app(&self, serial: String, package: String, title: String, screen: Screen) -> Result<Arc<Session>> {
        let config = self.config();
        let Some(size) = parse_window_size(config.window_size.clone()) else {
            return Err(CoreError::Failed("Window size must look like 1280x720/160.".into()));
        };
        let adb = self.adb()?;
        if config.capture_mouse {
            adb.mouse_follows_virtual_displays(&serial)?;
        }
        let phone = if size.width == 0 { adb.display_size(&serial) } else { None };
        let mut args = vec!["-s".into(), serial, format!("--start-app={package}"), format!("--window-title={title}")];
        let window = app_window(size, phone, screen);
        let display = if config.resizable_windows {
            // scrcpy opens a flex display's window at the display's size, and doesn't allow setting the
            // window size. Where windows are sized in points (macOS), the display has to be too: no
            // cropping, but not Retina-sharp. Where they're in pixels (Windows), it's sharp anyway.
            args.push("--flex-display".into());
            if screen.pixel_windows { window.display } else { app_window(size, phone, Screen { scale: 1.0, ..screen }).display }
        } else {
            let (width, height) = if screen.pixel_windows { (window.display.width, window.display.height) } else { (window.width, window.height) };
            args.extend([format!("--window-width={width}"), format!("--window-height={height}")]);
            window.display
        };
        args.push(format!("--new-display={}", format_window_size(display)));
        if display.width > 1920 || display.height > 1920 {
            // H.264 encoders often stop at 1080p; H.265 handles more and needs less bandwidth for it.
            args.extend(["--video-codec=h265".into(), "--video-bit-rate=20M".into()]);
        }
        self.start_scrcpy(args)
    }

    /// Closes all scrcpy windows (call this when the app quits).
    pub fn stop_sessions(&self) {
        for session in self.sessions.lock().unwrap().drain(..) {
            session.stop();
        }
    }
}

impl Core {
    fn remember_address(&self, address: &str) {
        let mut config = self.config();
        // Newest last; Wireless debugging changes its port each time, so keep only the last few.
        config.known_addresses.retain(|a| a != address);
        config.known_addresses.push(address.to_string());
        let excess = config.known_addresses.len().saturating_sub(10);
        config.known_addresses.drain(..excess);
        let _ = self.save_config(config);
    }

    fn adb(&self) -> Result<Adb> {
        Ok(Adb { path: self.adb_path().ok_or_else(no_adb)? })
    }

    fn start_scrcpy(&self, mut args: Vec<String>) -> Result<Arc<Session>> {
        let scrcpy = self.scrcpy_path().ok_or_else(|| CoreError::Failed("scrcpy isn't installed. Open Setup.".into()))?;
        let adb = self.adb_path().ok_or_else(no_adb)?;
        let config = self.config();
        if config.capture_mouse {
            args.push("--mouse=uhid".into());
        }
        if !config.forward_audio {
            args.push("--no-audio".into());
        }
        if config.screen_off {
            args.push("--turn-screen-off".into());
        }
        let session = Session::spawn(&scrcpy, &adb, &args)?;
        let mut sessions = self.sessions.lock().unwrap();
        sessions.retain(|s| !s.finished());
        sessions.push(session.clone());
        Ok(session)
    }
}

/// Package names go into phone shell commands: make sure that's all they are.
fn check_package(package: &str) -> Result<()> {
    if package.is_empty() || !package.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_') {
        return Err(CoreError::Failed(format!("Not a package name: {package}")));
    }
    Ok(())
}

fn no_adb() -> CoreError {
    CoreError::Failed("No adb found. Open Setup to choose one.".into())
}

#[uniffi::export]
pub fn phone_storage() -> String {
    PHONE_STORAGE.into()
}

#[uniffi::export]
pub fn virtual_display_min_sdk() -> u32 {
    VIRTUAL_DISPLAY_MIN_SDK
}
