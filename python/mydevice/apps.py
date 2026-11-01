"""Installed apps and their icons, cached on disk per device until a forced refresh."""

import base64
import json
import re
import shutil
from dataclasses import asdict, dataclass
from pathlib import Path

from mydevice.adb import list_apps

HELPER = Path(__file__).with_name("helper.dex")  # built from helper/Helper.java
DEVICE_HELPER = "/data/local/tmp/tether-helper.dex"


@dataclass
class App:
    name: str
    package: str
    system: bool
    icon: str | None  # path to a cached PNG


def load_apps(adb, scrcpy, cache_root, serial, force=False):
    """Apps on the device, from the cache unless force is set. Runs blocking adb calls."""
    folder = cache_root / re.sub(r"[^\w.-]", "_", serial)
    index = folder / "apps.json"
    if not force and index.exists():
        return [App(**app) for app in json.loads(index.read_text())]

    shutil.rmtree(folder, ignore_errors=True)
    try:
        apps = fetch_with_icons(adb, serial, folder / "icons")
    except Exception:
        # The helper relies on Android internals; scrcpy's list still works, just without icons.
        apps = [App(name, package, system, None) for name, package, system in list_apps(scrcpy, adb.path, serial)]
    if not apps:
        raise RuntimeError("No apps found on the device.")

    apps.sort(key=lambda app: app.name.lower())
    folder.mkdir(parents=True, exist_ok=True)
    index.write_text(json.dumps([asdict(app) for app in apps]))
    return apps


def fetch_with_icons(adb, serial, icon_dir):
    adb.run("push", str(HELPER), DEVICE_HELPER, serial=serial)
    out = adb.text("exec-out", f"CLASSPATH={DEVICE_HELPER} app_process / tether.Helper", serial=serial, timeout=120)
    icon_dir.mkdir(parents=True, exist_ok=True)
    apps = []
    # Lines are "package \t system \t label \t base64 png". Anything else is OEM log noise.
    for line in out.splitlines():
        fields = line.split("\t")
        if len(fields) != 4:
            continue
        package, system, name, icon = fields
        path = None
        if icon:
            path = icon_dir / f"{package}.png"
            path.write_bytes(base64.b64decode(icon))
        apps.append(App(name, package, system == "1", str(path) if path else None))
    return apps
