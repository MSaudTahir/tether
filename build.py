"""Build a standalone MyDevice for the current platform into dist/.

    pip install -e ".[build]" && python build.py

PyInstaller can't cross-compile, so run this on each OS (CI does it on tag push).
"""

import platform
import shutil
import subprocess
import sys
import tarfile
from pathlib import Path

NAME = "MyDevice"
ARCH = {"amd64": "x86_64", "aarch64": "arm64"}.get(platform.machine().lower(), platform.machine().lower())
DIST = Path("dist")

args = ["--noconfirm", "--clean", "--windowed", "--name", NAME, "--paths", "."]
if sys.platform == "darwin":
    args += ["--osx-bundle-identifier", "app.mydevice"]  # a .app folder; onefile .apps start slowly
else:
    args += ["--onefile"]
subprocess.run([sys.executable, "-m", "PyInstaller", *args, "mydevice/__main__.py"], check=True)

if sys.platform == "darwin":
    out = DIST / f"{NAME}-macos-{ARCH}.zip"
    out.unlink(missing_ok=True)
    # ditto keeps the symlinks inside Qt's frameworks, which zipfile would break.
    subprocess.run(["ditto", "-c", "-k", "--keepParent", DIST / f"{NAME}.app", out], check=True)
elif sys.platform == "win32":
    out = DIST / f"{NAME}-windows-{ARCH}.exe"
    out.unlink(missing_ok=True)
    shutil.move(DIST / f"{NAME}.exe", out)
else:
    out = DIST / f"{NAME}-linux-{ARCH}.tar.gz"
    with tarfile.open(out, "w:gz") as archive:  # tar keeps the executable bit
        archive.add(DIST / NAME, arcname=NAME)

print(f"Built {out}")
