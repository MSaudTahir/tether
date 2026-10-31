"""Thin wrappers around the adb and scrcpy command lines. Everything here blocks."""

import os
import re
import subprocess
import sys
import time
from dataclasses import dataclass

# Don't flash a console window for every command on Windows.
NO_WINDOW = subprocess.CREATE_NO_WINDOW if sys.platform == "win32" else 0


@dataclass
class Device:
    serial: str
    state: str  # "device", "unauthorized", "offline", ...
    model: str

    @property
    def wireless(self):
        return ":" in self.serial or "._adb-tls-connect." in self.serial

    @property
    def label(self):
        return self.model or self.serial


@dataclass
class DeviceInfo:
    name: str
    model: str
    android: str
    sdk: int
    battery: int
    power: str


class Adb:
    def __init__(self, path):
        self.path = path

    def text(self, *args, serial=None, timeout=15):
        return self.run(*args, serial=serial, timeout=timeout).decode(errors="replace")

    def run(self, *args, serial=None, timeout=15):
        cmd = [self.path, *(["-s", serial] if serial else []), *args]
        r = subprocess.run(cmd, capture_output=True, timeout=timeout, creationflags=NO_WINDOW)
        if r.returncode != 0:
            raise RuntimeError((r.stderr or r.stdout).decode(errors="replace").strip())
        return r.stdout

    def devices(self):
        devices = []
        for line in self.text("devices", "-l").splitlines():
            parts = line.split()
            if len(parts) < 2 or line.startswith(("List of", "*")):
                continue
            state = "no permissions" if parts[1] == "no" else parts[1]
            props = dict(p.split(":", 1) for p in parts[2:] if ":" in p)
            devices.append(Device(parts[0], state, props.get("model", "").replace("_", " ")))
        return devices

    def info(self, serial):
        out = self.text(
            "shell",
            "settings get global device_name; getprop ro.product.manufacturer; getprop ro.product.model;"
            " getprop ro.build.version.release; getprop ro.build.version.sdk; dumpsys battery",
            serial=serial,
        )
        lines = out.splitlines()
        name, maker, model, release, sdk = (lines + [""] * 5)[:5]
        battery = dict(line.strip().split(": ", 1) for line in lines[5:] if ": " in line)
        status = battery.get("status")
        power = "full" if status == "5" else "charging" if status == "2" else "on battery"
        sources = [s for s in ("AC", "USB", "Wireless") if battery.get(f"{s} powered") == "true"]
        if sources:
            power += f" ({', '.join(sources)})"
        return DeviceInfo(
            name=name if name not in ("", "null") else model,
            model=f"{maker} {model}".strip(),
            android=release,
            sdk=int(sdk) if sdk.isdigit() else 0,
            battery=int(battery.get("level", "0")),
            power=power,
        )

    def screenshot(self, serial):
        """PNG bytes of the current screen."""
        return self.run("exec-out", "screencap", "-p", serial=serial, timeout=20)

    def pair(self, address, code):
        out = self.text("pair", address, code, timeout=30)
        if "Successfully paired" not in out:
            raise RuntimeError(out.strip() or "Pairing failed")

    def connect(self, address):
        out = self.text("connect", address, timeout=30)
        if "connected to" not in out:
            raise RuntimeError(out.strip())
        return address

    def go_wireless(self, serial, port=5555):
        """Switch a USB device to adb over TCP/IP and connect to it. Returns the new serial."""
        route = self.text("shell", "ip route", serial=serial)
        ip = re.search(r"dev wlan\S*.*?src (\d+\.\d+\.\d+\.\d+)", route) or re.search(
            r"src (\d+\.\d+\.\d+\.\d+)", route
        )
        if not ip:
            raise RuntimeError("Couldn't find the device's Wi-Fi IP address. Is Wi-Fi on?")
        self.text("tcpip", str(port), serial=serial)
        time.sleep(1.5)  # adbd restarts in TCP mode
        return self.connect(f"{ip[1]}:{port}")


def scrcpy_env(adb_path):
    """scrcpy uses the adb in $ADB, so it talks to the same adb server we do."""
    return {**os.environ, "ADB": adb_path}


def list_apps(scrcpy, adb_path, serial):
    """Launchable apps as (name, package, is_system) tuples, via `scrcpy --list-apps`."""
    r = subprocess.run(
        [scrcpy, "-s", serial, "--list-apps"],
        capture_output=True,
        env=scrcpy_env(adb_path),
        timeout=90,
        creationflags=NO_WINDOW,
    )
    out = r.stdout.decode(errors="replace") + r.stderr.decode(errors="replace")
    apps = parse_app_list(out)
    if r.returncode != 0 and not apps:
        errors = [line for line in out.splitlines() if "ERROR" in line] or out.splitlines()[-3:]
        raise RuntimeError("\n".join(errors))
    return sorted(apps, key=lambda app: app[0].lower())


def parse_app_list(out):
    # Lines look like " * Name<padding> com.package" (* = system app, - = user app).
    # Names of 30+ chars put the package on the next line, indented.
    apps, pending = [], None
    for line in out.splitlines():
        if pending and line.startswith("    ") and line.strip():
            apps.append((pending[1], line.strip(), pending[0] == "*"))
            pending = None
        elif m := re.match(r" ([*-]) (.+)$", line):
            flag, rest = m.groups()
            if pair := re.match(r"(.*\S)\s{2,}(\S+)$", rest):
                apps.append((pair[1], pair[2], flag == "*"))
            else:
                pending = (flag, rest.strip())
    return apps
