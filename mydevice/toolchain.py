"""Finds adb binaries and downloads/updates the bundled scrcpy build."""

import hashlib
import io
import json
import os
import platform
import shutil
import ssl
import sys
import tarfile
import urllib.request
import zipfile
from pathlib import Path

import certifi

RELEASES_API = "https://api.github.com/repos/Genymobile/scrcpy/releases/latest"
EXE = ".exe" if sys.platform == "win32" else ""
# Auto mode prefers Android Studio's adb so we don't kill the adb server it is using.
ADB_PREFERENCE = ("studio", "system", "bundled")


def _get(url, timeout=30):
    req = urllib.request.Request(url, headers={"User-Agent": "mydevice"})
    ctx = ssl.create_default_context(cafile=certifi.where())
    with urllib.request.urlopen(req, timeout=timeout, context=ctx) as r:
        return r.read()


def asset_platform():
    """The scrcpy release asset name for this OS/CPU, or None if there is no prebuilt one."""
    machine = platform.machine().lower()
    arm = machine in ("arm64", "aarch64")
    if sys.platform == "darwin":
        return "macos-aarch64" if arm else "macos-x86_64"
    if sys.platform == "win32":
        return "winarm64" if arm else "win64"
    if sys.platform.startswith("linux") and machine in ("x86_64", "amd64"):
        return "linux-x86_64"
    return None


def android_sdk_dirs():
    dirs = [os.environ.get("ANDROID_HOME"), os.environ.get("ANDROID_SDK_ROOT")]
    if sys.platform == "darwin":
        dirs.append(Path.home() / "Library/Android/sdk")
    elif sys.platform == "win32":
        dirs.append(Path(os.environ.get("LOCALAPPDATA", "")) / "Android/Sdk")
    else:
        dirs.append(Path.home() / "Android/Sdk")
    return [Path(d) for d in dirs if d]


class Toolchain:
    def __init__(self, data_dir: Path):
        self.root = data_dir / "scrcpy"

    def installed_version(self):
        current = self.root / "current"
        return current.read_text().strip() if current.exists() else None

    def bundled_file(self, name):
        version = self.installed_version()
        path = self.root / version / f"{name}{EXE}" if version else None
        return str(path) if path and path.exists() else None

    def scrcpy_path(self):
        return self.bundled_file("scrcpy") or shutil.which("scrcpy")

    def adb_candidates(self):
        """Available adb binaries keyed by source."""
        found = {}
        for sdk in android_sdk_dirs():
            adb = sdk / "platform-tools" / f"adb{EXE}"
            if adb.exists():
                found["studio"] = str(adb)
                break
        if system := shutil.which("adb"):
            found["system"] = system
        if bundled := self.bundled_file("adb"):
            found["bundled"] = bundled
        return found

    def resolve_adb(self, choice="auto"):
        """choice is "auto", a source key from adb_candidates(), or a custom path."""
        found = self.adb_candidates()
        if choice in found:
            return found[choice]
        if choice not in ("auto", *ADB_PREFERENCE) and Path(choice).is_file():
            return choice
        return next((found[k] for k in ADB_PREFERENCE if k in found), None)

    def latest_release(self):
        """(version, url, asset name, sha256) of the newest scrcpy for this platform, or None."""
        target = asset_platform()
        if not target:
            return None
        release = json.loads(_get(RELEASES_API))
        assets = {a["name"]: a["browser_download_url"] for a in release["assets"]}
        name = next((n for n in assets if n.startswith(f"scrcpy-{target}-")), None)
        if not name:
            return None
        sums = _get(assets["SHA256SUMS.txt"]).decode()
        sha = next(line.split()[0] for line in sums.splitlines() if line.endswith(name))
        return release["tag_name"].lstrip("v"), assets[name], name, sha

    def install(self, version, url, name, sha):
        data = _get(url, timeout=300)
        if hashlib.sha256(data).hexdigest() != sha:
            raise RuntimeError(f"Checksum mismatch for {name}")

        tmp = self.root / f".{version}.tmp"
        shutil.rmtree(tmp, ignore_errors=True)
        tmp.mkdir(parents=True)
        if name.endswith(".zip"):
            zipfile.ZipFile(io.BytesIO(data)).extractall(tmp)
        else:
            with tarfile.open(fileobj=io.BytesIO(data)) as archive:
                archive.extractall(tmp, filter="data")

        # Archives contain a single top-level folder, e.g. scrcpy-macos-aarch64-v5.0.1/
        dest = self.root / version
        shutil.rmtree(dest, ignore_errors=True)
        next(tmp.iterdir()).rename(dest)
        shutil.rmtree(tmp)
        (self.root / "current").write_text(version)

        for old in self.root.iterdir():
            if old.is_dir() and old != dest:
                shutil.rmtree(old, ignore_errors=True)
