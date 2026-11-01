//! adb commands and output parsing. Everything here blocks.

use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::util::{command, output, text};
use crate::{CoreError, Device, DeviceInfo, DeviceKey, RebootMode, Result, WindowSize};

const TIMEOUT: Duration = Duration::from_secs(15);

pub struct Adb {
    pub path: String,
}

impl Adb {
    pub fn command(&self, serial: Option<&str>, args: &[&str]) -> Command {
        let mut cmd = command(&self.path);
        if let Some(serial) = serial {
            cmd.args(["-s", serial]);
        }
        cmd.args(args);
        cmd
    }

    /// Runs adb and returns stdout, or fails with its error output.
    pub fn run(&self, serial: Option<&str>, args: &[&str], timeout: Duration) -> Result<Vec<u8>> {
        let out = output(self.command(serial, args), timeout)?;
        if !out.status.success() {
            let message = if out.stderr.is_empty() { &out.stdout } else { &out.stderr };
            return Err(CoreError::Failed(text(message).trim().to_string()));
        }
        Ok(out.stdout)
    }

    pub fn text(&self, serial: Option<&str>, args: &[&str]) -> Result<String> {
        Ok(text(&self.run(serial, args, TIMEOUT)?))
    }

    pub fn devices(&self) -> Result<Vec<Device>> {
        Ok(parse_devices(&self.text(None, &["devices", "-l"])?))
    }

    pub fn info(&self, serial: &str) -> Result<DeviceInfo> {
        Ok(parse_info(&self.text(Some(serial), &["shell", INFO_SCRIPT])?))
    }

    pub fn pair(&self, address: &str, code: &str) -> Result<()> {
        let out = text(&self.run(None, &["pair", address, code], Duration::from_secs(30))?);
        if !out.contains("Successfully paired") {
            return Err(CoreError::Failed(if out.trim().is_empty() { "Pairing failed".into() } else { out.trim().into() }));
        }
        Ok(())
    }

    pub fn connect(&self, address: &str) -> Result<()> {
        let out = text(&self.run(None, &["connect", address], Duration::from_secs(30))?);
        // adb exits 0 even when it fails to connect; "failed to connect to" doesn't contain "connected to".
        if !out.contains("connected to") {
            return Err(CoreError::Failed(out.trim().into()));
        }
        Ok(())
    }

    /// Switches a USB device to adb over TCP/IP and connects to it. Returns the new serial.
    pub fn go_wireless(&self, serial: &str) -> Result<String> {
        let ip = wifi_ip(&self.text(Some(serial), &["shell", "ip route"])?)
            .ok_or_else(|| CoreError::Failed("Couldn't find the device's Wi-Fi IP address. Is Wi-Fi on?".into()))?;
        self.text(Some(serial), &["tcpip", "5555"])?;
        thread::sleep(Duration::from_millis(1500)); // adbd restarts in TCP mode
        let address = format!("{ip}:5555");
        self.connect(&address)?;
        Ok(address)
    }

    /// Runs a shell command, failing with its output if it fails.
    pub fn shell(&self, serial: &str, command: &str) -> Result<()> {
        // adb shell returns the command's exit status (Android 7+).
        self.text(Some(serial), &["shell", command])?;
        Ok(())
    }

    pub fn screenshot(&self, serial: &str, path: &str) -> Result<()> {
        let png = self.run(Some(serial), &["exec-out", "screencap -p"], Duration::from_secs(30))?;
        if !png.starts_with(b"\x89PNG") {
            return Err(CoreError::Failed("The phone didn't return a screenshot. Is the screen on?".into()));
        }
        std::fs::write(path, png)?;
        Ok(())
    }

    pub fn install(&self, serial: &str, path: &str) -> Result<()> {
        // -r: replace an installed version, -d: allow a lower version.
        let out = text(&self.run(Some(serial), &["install", "-r", "-d", path], Duration::from_secs(300))?);
        if !out.contains("Success") {
            return Err(CoreError::Failed(out.trim().into()));
        }
        Ok(())
    }

    pub fn press_key(&self, serial: &str, key: DeviceKey) -> Result<()> {
        let code = match key {
            DeviceKey::Power => "KEYCODE_POWER",
            DeviceKey::VolumeUp => "KEYCODE_VOLUME_UP",
            DeviceKey::VolumeDown => "KEYCODE_VOLUME_DOWN",
            DeviceKey::Home => "KEYCODE_HOME",
            DeviceKey::Back => "KEYCODE_BACK",
            DeviceKey::RecentApps => "KEYCODE_APP_SWITCH",
        };
        self.shell(serial, &format!("input keyevent {code}"))
    }

    pub fn reboot(&self, serial: &str, mode: RebootMode) -> Result<()> {
        let mut args = vec!["reboot"];
        match mode {
            RebootMode::Normal => {}
            RebootMode::Recovery => args.push("recovery"),
            RebootMode::Bootloader => args.push("bootloader"),
        }
        self.run(Some(serial), &args, TIMEOUT)?;
        Ok(())
    }

    /// The screen's size in pixels and its density (the overridden ones if set), if adb says.
    pub fn display_size(&self, serial: &str) -> Option<WindowSize> {
        parse_display_size(&self.text(Some(serial), &["shell", "wm size; wm density"]).ok()?)
    }

    /// Android sends a hardware mouse to the main display unless "Force desktop mode" (developer
    /// option) is on; then it goes to the newest virtual display. Takes effect without a reboot.
    pub fn mouse_follows_virtual_displays(&self, serial: &str) -> Result<()> {
        self.shell(serial, "settings put global force_desktop_mode_on_external_displays 1")
    }
}

pub fn parse_devices(out: &str) -> Vec<Device> {
    let mut devices = Vec::new();
    for line in out.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 || line.starts_with("List of") || line.starts_with('*') {
            continue;
        }
        let serial = parts[0].to_string();
        let state = if parts[1] == "no" { "no permissions".to_string() } else { parts[1].to_string() };
        let model = parts[2..]
            .iter()
            .find_map(|p| p.strip_prefix("model:"))
            .map(|m| m.replace('_', " "))
            .unwrap_or_default();
        let wireless = serial.contains(':') || serial.contains("._adb-tls-connect.");
        let label = if model.is_empty() { serial.clone() } else { model.clone() };
        let short_serial = serial
            .strip_prefix("adb-")
            .and_then(|rest| rest.split_once("._adb-tls-connect."))
            .and_then(|(name, _)| name.rsplit_once('-'))
            .map_or(serial.clone(), |(serial, _)| serial.to_string());
        devices.push(Device { serial, state, model, label, short_serial, wireless });
    }
    devices
}

/// One round trip for everything DeviceInfo shows; each section starts with "==name".
const INFO_SCRIPT: &str = "echo ==props; settings get global device_name; getprop ro.product.manufacturer; \
    getprop ro.product.model; getprop ro.build.version.release; getprop ro.build.version.sdk; \
    getprop ro.build.version.security_patch; getprop ro.soc.manufacturer; getprop ro.soc.model; \
    getprop ro.board.platform; settings get global stay_on_while_plugged_in; \
    echo ==battery; dumpsys battery; echo ==memory; cat /proc/meminfo; echo ==storage; df /data; \
    echo ==display; wm size; wm density; echo ==uptime; cat /proc/uptime; echo ==night; cmd uimode night; \
    echo ==ip; ip -4 addr show wlan0; \
    echo ==power; dumpsys power | grep -m1 mWakefulness= 2>/dev/null; echo ==zen; settings get global zen_mode";

pub fn parse_info(out: &str) -> DeviceInfo {
    let section = |name: &str| -> Vec<&str> {
        out.lines()
            .skip_while(|l| l.trim() != format!("=={name}"))
            .skip(1)
            .take_while(|l| !l.starts_with("=="))
            .map(str::trim)
            .collect()
    };
    let props = section("props");
    let prop = |i: usize| props.get(i).copied().filter(|p| !p.is_empty() && *p != "null").unwrap_or_default();
    let (name, maker, model) = (prop(0), prop(1), prop(2));

    let battery = section("battery");
    let get = |key: &str| battery.iter().filter_map(|l| l.split_once(": ")).find(|(k, _)| *k == key).map(|(_, v)| v);
    let mut power = match get("status") {
        Some("5") => "full",
        Some("2") => "charging",
        _ => "on battery",
    }
    .to_string();
    let sources: Vec<&str> =
        ["AC", "USB", "Wireless"].into_iter().filter(|s| get(&format!("{s} powered")) == Some("true")).collect();
    if !sources.is_empty() {
        power += &format!(" ({})", sources.join(", "));
    }
    // BatteryManager.BATTERY_HEALTH_*
    let battery_health = match get("health") {
        Some("2") => "good",
        Some("3") => "overheating",
        Some("4") => "dead",
        Some("5") => "over voltage",
        Some("7") => "cold",
        _ => "",
    };

    // "MemTotal:  7793688 kB"
    let memory = section("memory");
    let kb = |key: &str| -> u64 {
        memory.iter().find_map(|l| l.strip_prefix(key)?.trim().strip_suffix("kB")?.trim().parse().ok()).unwrap_or(0) * 1024
    };
    // "/dev/block/dm-10  112203740 110262804 1809864 99% /data"
    let storage: Vec<u64> = section("storage")
        .get(1)
        .map(|l| l.split_whitespace().skip(1).take(2).filter_map(|n| n.parse::<u64>().ok()).map(|n| n * 1024).collect())
        .unwrap_or_default();

    // "Physical size: 1080x2400", maybe followed by "Override size: ..."; same for density.
    let display = section("display");
    let last = |key: &str| display.iter().rev().find_map(|l| l.split_once(": ").filter(|(k, _)| k.ends_with(key)).map(|(_, v)| v));
    let screen = match (last("size"), last("density")) {
        (Some(size), Some(dpi)) => format!("{} · {dpi} dpi", size.replace('x', " × ")),
        (Some(size), None) => size.replace('x', " × "),
        _ => String::new(),
    };

    let soc_model = prop(7);
    let chipset = match (prop(6), soc_model) {
        (_, "") => prop(8).to_string(),
        ("QTI" | "Qualcomm", soc) => format!("Qualcomm {soc}"),
        (soc_maker, soc) => format!("{soc_maker} {soc}").trim().to_string(),
    };

    DeviceInfo {
        name: if name.is_empty() { model.to_string() } else { name.to_string() },
        model: format!("{maker} {model}").trim().to_string(),
        android: prop(3).to_string(),
        sdk: prop(4).parse().unwrap_or(0),
        security_patch: prop(5).to_string(),
        chipset,
        battery: get("level").and_then(|l| l.parse().ok()).unwrap_or(0),
        power,
        battery_health: battery_health.into(),
        battery_temperature: get("temperature").and_then(|t| t.parse::<f32>().ok()).map_or(0.0, |t| t / 10.0),
        memory_total: kb("MemTotal:"),
        memory_available: kb("MemAvailable:"),
        storage_total: storage.first().copied().unwrap_or(0),
        storage_used: storage.get(1).copied().unwrap_or(0),
        screen,
        uptime_seconds: section("uptime").first().and_then(|l| l.split('.').next()?.parse().ok()).unwrap_or(0),
        wifi_address: section("ip").iter().find_map(|l| l.strip_prefix("inet ")?.split('/').next()).map(String::from),
        dark_mode: section("night").first() == Some(&"Night mode: yes"),
        stay_awake: prop(9).parse::<u32>().unwrap_or(0) != 0,
        screen_on: section("power").first().is_some_and(|l| *l == "mWakefulness=Awake"),
        // Settings.Global.ZEN_MODE: 0 is off; 1-3 are kinds of Do Not Disturb.
        do_not_disturb: section("zen").first().and_then(|z| z.parse::<u32>().ok()).is_some_and(|z| z != 0),
    }
}

/// "Physical size: 1080x2400" / "Physical density: 440", each maybe followed by an "Override" line.
pub fn parse_display_size(out: &str) -> Option<WindowSize> {
    let last = |key: &str| out.lines().rev().find_map(|l| l.trim().split_once(": ").filter(|(k, _)| k.ends_with(key)).map(|(_, v)| v));
    let (width, height) = last("size")?.split_once('x')?;
    Some(WindowSize { width: width.parse().ok()?, height: height.parse().ok()?, dpi: last("density")?.parse().ok()? })
}

fn wifi_ip(route: &str) -> Option<String> {
    let src = |line: &str| {
        let mut words = line.split_whitespace();
        words.find(|w| *w == "src").and_then(|_| words.next()).map(str::to_string)
    };
    route.lines().filter(|l| l.contains("wlan")).find_map(src).or_else(|| route.lines().find_map(src))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices() {
        let out = "List of devices attached\n\
                   3acb3aa                device usb:34603008X product:apollo_global model:M2007J3SY device:apollo transport_id:9\n\
                   192.168.1.5:5555       unauthorized transport_id:2\n";
        let devices = parse_devices(out);
        assert_eq!(devices.len(), 2);
        assert_eq!((devices[0].label.as_str(), devices[0].wireless), ("M2007J3SY", false));
        assert_eq!((devices[1].state.as_str(), devices[1].wireless), ("unauthorized", true));
        let tls = parse_devices("adb-3acb3aa-eBd7lh._adb-tls-connect._tcp device model:M2007J3SY\n");
        assert_eq!((tls[0].short_serial.as_str(), tls[0].wireless), ("3acb3aa", true));
    }

    #[test]
    fn info() {
        let out = "==props\nnull\nXiaomi\nM2007J3SY\n12\n31\n2023-03-01\nQTI\nSM8250\nkona\n7\n\
            ==battery\nCurrent Battery Service state:\n  AC powered: true\n  USB powered: false\n  status: 2\n  health: 2\n  level: 82\n  temperature: 370\n\
            ==memory\nMemTotal:        7793688 kB\nMemFree:          165700 kB\nMemAvailable:    1856336 kB\n\
            ==storage\nFilesystem       1K-blocks      Used Available Use% Mounted on\n/dev/block/dm-10 112203740 110262804   1809864  99% /data/user/0\n\
            ==display\nPhysical size: 1080x2400\nPhysical density: 440\nOverride density: 400\n\
            ==uptime\n22623.83 136384.28\n==night\nNight mode: yes\n\
            ==power\nmWakefulness=Awake\n==zen\n1\n\
            ==ip\n47: wlan0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500\n    inet 192.168.100.85/24 brd 192.168.100.255 scope global dynamic wlan0\n";
        let info = parse_info(out);
        assert_eq!(info.name, "M2007J3SY");
        assert_eq!(info.model, "Xiaomi M2007J3SY");
        assert_eq!((info.sdk, info.battery), (31, 82));
        assert_eq!(info.power, "charging (AC)");
        assert_eq!((info.battery_health.as_str(), info.battery_temperature), ("good", 37.0));
        assert_eq!((info.chipset.as_str(), info.security_patch.as_str()), ("Qualcomm SM8250", "2023-03-01"));
        assert_eq!((info.memory_total, info.memory_available), (7793688 * 1024, 1856336 * 1024));
        assert_eq!((info.storage_total, info.storage_used), (112203740 * 1024, 110262804 * 1024));
        assert_eq!(info.screen, "1080 × 2400 · 400 dpi");
        assert_eq!(info.uptime_seconds, 22623);
        assert_eq!(info.wifi_address.as_deref(), Some("192.168.100.85"));
        assert!(info.dark_mode && info.stay_awake && info.screen_on && info.do_not_disturb);

        let empty = parse_info("");
        assert_eq!((empty.memory_total, empty.storage_total, empty.wifi_address), (0, 0, None));
    }

    #[test]
    fn display_size() {
        let out = "Physical size: 1080x2400\nPhysical density: 440\nOverride density: 400\n";
        assert_eq!(parse_display_size(out), Some(WindowSize { width: 1080, height: 2400, dpi: 400 }));
        assert_eq!(parse_display_size("error"), None);
    }

    #[test]
    fn ip() {
        let route = "10.0.0.0/8 dev rmnet0 proto kernel scope link src 10.1.2.3\n\
                     192.168.1.0/24 dev wlan0 proto kernel scope link src 192.168.1.20\n";
        assert_eq!(wifi_ip(route).as_deref(), Some("192.168.1.20"));
    }
}
