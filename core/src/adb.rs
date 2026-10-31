//! adb commands and output parsing. Everything here blocks.

use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::util::{command, output, text};
use crate::{CoreError, Device, DeviceInfo, Result};

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
        let out = self.text(
            Some(serial),
            &[
                "shell",
                "settings get global device_name; getprop ro.product.manufacturer; getprop ro.product.model; \
                 getprop ro.build.version.release; getprop ro.build.version.sdk; dumpsys battery",
            ],
        )?;
        Ok(parse_info(&out))
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

pub fn parse_info(out: &str) -> DeviceInfo {
    let lines: Vec<&str> = out.lines().collect();
    let line = |i: usize| lines.get(i).map(|l| l.trim()).unwrap_or_default();
    let (name, maker, model, release, sdk) = (line(0), line(1), line(2), line(3), line(4));
    let battery: Vec<(&str, &str)> = lines.iter().skip(5).filter_map(|l| l.trim().split_once(": ")).collect();
    let get = |key: &str| battery.iter().find(|(k, _)| *k == key).map(|(_, v)| *v);

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
    DeviceInfo {
        name: if name.is_empty() || name == "null" { model.to_string() } else { name.to_string() },
        model: format!("{maker} {model}").trim().to_string(),
        android: release.to_string(),
        sdk: sdk.parse().unwrap_or(0),
        battery: get("level").and_then(|l| l.parse().ok()).unwrap_or(0),
        power,
    }
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
        let out = "null\nXiaomi\nM2007J3SY\n12\n31\nCurrent Battery Service state:\n  AC powered: true\n  USB powered: false\n  status: 2\n  level: 82\n";
        let info = parse_info(out);
        assert_eq!(info.name, "M2007J3SY");
        assert_eq!(info.model, "Xiaomi M2007J3SY");
        assert_eq!((info.sdk, info.battery), (31, 82));
        assert_eq!(info.power, "charging (AC)");
    }

    #[test]
    fn ip() {
        let route = "10.0.0.0/8 dev rmnet0 proto kernel scope link src 10.1.2.3\n\
                     192.168.1.0/24 dev wlan0 proto kernel scope link src 192.168.1.20\n";
        assert_eq!(wifi_ip(route).as_deref(), Some("192.168.1.20"));
    }
}
