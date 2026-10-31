//! Exercises the core against a real device: cargo run --example smoke -- <data dir>
use std::time::Instant;

fn main() {
    let dir = std::env::args().nth(1).expect("data dir");
    let core = mydevice_core::Core::new(dir);
    let t = Instant::now();
    if core.bundled_scrcpy_version().is_none() {
        let release = core.latest_release().unwrap().expect("no release for this platform");
        println!("installing scrcpy {}", release.version);
        core.install_scrcpy(release).unwrap();
    }
    println!("scrcpy {:?} at {:?}", core.bundled_scrcpy_version(), core.scrcpy_path());
    println!("adb candidates: {:?}", core.adb_candidates());
    println!("adb: {:?}", core.adb_path());
    for device in core.devices().unwrap() {
        println!("{device:?}");
        if device.state == "device" {
            println!("{:?}", core.device_info(device.serial.clone()).unwrap());
            for force in [true, false] {
                let t = Instant::now();
                let apps = core.apps(device.serial.clone(), force).unwrap();
                let icons = apps.iter().filter(|a| a.icon_path.is_some()).count();
                println!("apps (force={force}): {} with {icons} icons in {:?}", apps.len(), t.elapsed());
            }
        }
    }
    println!("total {:?}", t.elapsed());
}
