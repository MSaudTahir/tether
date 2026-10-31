"""Main window: device list on the left, the selected device and its apps on the right."""

import re
import subprocess
import zlib
from functools import lru_cache

from PySide6.QtCore import QSettings, QSize, Qt, QTimer
from PySide6.QtGui import QColor, QIcon, QPainter, QPixmap
from PySide6.QtWidgets import (
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QFormLayout,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QListView,
    QListWidget,
    QListWidgetItem,
    QMainWindow,
    QMessageBox,
    QPushButton,
    QSplitter,
    QStackedWidget,
    QVBoxLayout,
    QWidget,
)

from mydevice.adb import NO_WINDOW, Adb, scrcpy_env
from mydevice.apps import load_apps
from mydevice.tasks import Bg
from mydevice.wizard import SetupWizard

VD_PRESETS = ["1280x800/200", "1920x1080/280", "2560x1440/360", "Device size"]
VIRTUAL_DISPLAY_MIN_SDK = 29  # Android 10
NO_DEVICE = "No device connected.\n\nPlug in a phone with USB debugging enabled,\nor use “Pair wireless…”."
STATE_HINTS = {
    "unauthorized": "Unlock the phone and accept the “Allow USB debugging?” prompt.",
    "offline": "The device is offline. Try reconnecting it.",
    "no permissions": "No permission to access this device.\nOn Linux, add a udev rule for it.",
}


@lru_cache(maxsize=None)
def avatar(name, size=48):
    """Placeholder app icon: first letter on a color derived from the name."""
    pixmap = QPixmap(size, size)
    pixmap.fill(Qt.transparent)
    p = QPainter(pixmap)
    p.setRenderHint(QPainter.Antialiasing)
    p.setPen(Qt.NoPen)
    p.setBrush(QColor.fromHsv(zlib.crc32(name.encode()) % 360, 120, 200))
    p.drawRoundedRect(0, 0, size, size, size / 4, size / 4)
    font = p.font()
    font.setPixelSize(size // 2)
    font.setBold(True)
    p.setFont(font)
    p.setPen(Qt.white)
    p.drawText(pixmap.rect(), Qt.AlignCenter, name[:1].upper())
    p.end()
    return QIcon(pixmap)


class MainWindow(QMainWindow):
    def __init__(self, tools, data_dir):
        super().__init__()
        self.setWindowTitle("MyDevice")
        self.resize(1000, 680)
        self.settings = QSettings()
        self.tools = tools
        self.app_cache = data_dir / "apps"
        self.bg = Bg()
        self.adb = None
        self.devices = {}  # serial -> Device
        self.device_key = None  # last device list, to skip redundant redraws
        self.serial = None  # selected device
        self.apps = {}  # serial -> [App]
        self.loading_apps = set()  # serials
        self.sessions = []  # running scrcpy processes
        self.latest = None  # newest scrcpy release, if newer than ours
        self.polling = False

        self.build_ui()
        self.apply_adb()
        self.check_updates()
        self.timer = QTimer(self, interval=2000, timeout=self.poll)
        self.timer.start()
        self.poll()

    def build_ui(self):
        toolbar = self.addToolBar("Main")
        toolbar.setMovable(False)
        toolbar.addAction("Refresh", self.reload).setToolTip("Reload device info and the app list")
        toolbar.addAction("Pair wireless…", self.pair_dialog)
        toolbar.addAction("Setup…", self.run_setup)

        self.device_list = QListWidget()
        self.device_list.setMinimumWidth(220)
        self.device_list.currentItemChanged.connect(
            lambda item: self.show_device(item.data(Qt.UserRole) if item else None)
        )

        self.name_label = QLabel()
        self.name_label.setStyleSheet("font-size: 22px; font-weight: 600;")
        self.details = QLabel()
        self.details.setTextInteractionFlags(Qt.TextSelectableByMouse)
        self.mirror_btn = QPushButton("Mirror screen", clicked=self.mirror)
        self.wireless_btn = QPushButton("Go wireless", clicked=self.go_wireless)
        self.vd_size = QComboBox(editable=True)
        self.vd_size.addItems(VD_PRESETS)
        self.vd_size.setCurrentText(self.settings.value("vd_size", VD_PRESETS[1]))
        self.vd_size.setToolTip("App window size as WIDTHxHEIGHT/DPI, e.g. 1600x1000/240")

        buttons = QHBoxLayout()
        buttons.addWidget(self.mirror_btn)
        buttons.addWidget(self.wireless_btn)
        buttons.addStretch()
        size_row = QHBoxLayout()
        size_row.addWidget(QLabel("App window size:"))
        size_row.addWidget(self.vd_size)
        size_row.addStretch()
        header = QVBoxLayout()
        header.addWidget(self.name_label)
        header.addWidget(self.details)
        header.addSpacing(8)
        header.addLayout(buttons)
        header.addLayout(size_row)

        self.app_hint = QLabel()
        self.filter = QLineEdit(placeholderText="Filter apps…", clearButtonEnabled=True)
        self.filter.textChanged.connect(self.show_apps)
        self.app_grid = QListWidget()
        self.app_grid.setViewMode(QListView.IconMode)
        self.app_grid.setIconSize(QSize(48, 48))
        self.app_grid.setGridSize(QSize(100, 92))
        self.app_grid.setResizeMode(QListView.Adjust)
        self.app_grid.setMovement(QListView.Static)
        self.app_grid.setWordWrap(True)
        self.app_grid.itemActivated.connect(self.launch_app)
        apps_row = QHBoxLayout()
        apps_row.addWidget(self.app_hint, 1)
        apps_row.addWidget(self.filter)

        detail = QWidget()
        layout = QVBoxLayout(detail)
        layout.addLayout(header)
        layout.addSpacing(12)
        layout.addLayout(apps_row)
        layout.addWidget(self.app_grid, 1)

        self.empty = QLabel(NO_DEVICE, alignment=Qt.AlignCenter)
        self.stack = QStackedWidget()
        self.stack.addWidget(self.empty)
        self.stack.addWidget(detail)

        splitter = QSplitter()
        splitter.addWidget(self.device_list)
        splitter.addWidget(self.stack)
        splitter.setStretchFactor(1, 1)
        self.setCentralWidget(splitter)

        self.tool_label = QLabel()
        self.update_btn = QPushButton(visible=False, clicked=lambda: self.install_scrcpy(self.latest))
        self.statusBar().addPermanentWidget(self.tool_label)
        self.statusBar().addPermanentWidget(self.update_btn)

    # --- scrcpy & adb -------------------------------------------------------

    def scrcpy(self):
        return self.tools.resolve_scrcpy(self.settings.value("scrcpy", ""))

    def using_bundled_scrcpy(self):
        return not self.settings.value("scrcpy", "") and self.tools.installed_version()

    def apply_adb(self):
        path = self.tools.resolve_adb(self.settings.value("adb", "auto"))
        self.adb = Adb(path) if path else None
        self.device_key = None
        if self.using_bundled_scrcpy():
            scrcpy = f"scrcpy {self.tools.installed_version()}"
        else:
            scrcpy = f"scrcpy: {self.scrcpy() or 'not found'}"
        self.tool_label.setText(f"{scrcpy}  ·  adb: {path or 'not found'}")

    def check_updates(self):
        def done(latest, error):
            if not error and latest and self.using_bundled_scrcpy() and latest[0] != self.tools.installed_version():
                self.latest = latest
                self.update_btn.setText(f"Update scrcpy to {latest[0]}")
                self.update_btn.show()

        self.update_btn.hide()
        if self.using_bundled_scrcpy():
            self.bg.run(self.tools.latest_release, done)

    def install_scrcpy(self, release):
        self.update_btn.hide()
        self.statusBar().showMessage(f"Downloading scrcpy {release[0]}…")

        def done(_, error):
            if error:
                self.statusBar().clearMessage()
                QMessageBox.warning(self, "scrcpy", f"Couldn't install scrcpy {release[0]}:\n{error}")
                return
            self.statusBar().showMessage(f"scrcpy {release[0]} installed", 5000)
            self.apply_adb()  # the bundled adb may be the only one available

        self.bg.run(lambda: self.tools.install(*release), done)

    def run_setup(self):
        SetupWizard(self.tools, self).exec()
        self.apps.clear()  # scrcpy may have changed
        self.apply_adb()
        self.check_updates()
        self.poll()

    # --- devices ------------------------------------------------------------

    def poll(self):
        if self.polling or not self.adb:
            return
        self.polling = True
        self.bg.run(self.adb.devices, self.on_devices)

    def on_devices(self, devices, error):
        self.polling = False
        if error:
            self.statusBar().showMessage(f"adb: {error}", 4000)
            return
        key = [(d.serial, d.state, d.model) for d in devices]
        if key == self.device_key:
            return
        self.device_key = key
        self.devices = {d.serial: d for d in devices}

        target = self.serial if self.serial in self.devices else next(iter(self.devices), None)
        self.device_list.blockSignals(True)
        self.device_list.clear()
        for d in devices:
            item = QListWidgetItem(f"{d.label}\n{'Wi-Fi' if d.wireless else 'USB'} · {d.state}")
            item.setData(Qt.UserRole, d.serial)
            item.setSizeHint(QSize(0, 48))
            self.device_list.addItem(item)
            if d.serial == target:
                self.device_list.setCurrentItem(item)
        self.device_list.blockSignals(False)
        self.show_device(target)

    def show_device(self, serial):
        self.serial = serial
        device = self.devices.get(serial)
        if not device or device.state != "device":
            self.empty.setText(STATE_HINTS.get(device.state, f"Device is {device.state}.") if device else NO_DEVICE)
            self.stack.setCurrentIndex(0)
            return
        self.stack.setCurrentIndex(1)
        self.name_label.setText(device.label)
        self.details.setText(serial)
        self.wireless_btn.setVisible(not device.wireless)
        self.show_apps()
        self.refresh()
        self.fetch_apps()

    def selected(self):
        device = self.devices.get(self.serial)
        return device if device and device.state == "device" else None

    def reload(self):
        self.refresh()
        self.fetch_apps(force=True)

    def refresh(self):
        """Load info about the selected device."""
        device = self.selected()
        if not device or not self.adb:
            return
        serial, adb = device.serial, self.adb
        self.bg.run(lambda: adb.info(serial), lambda info, error: self.on_info(serial, info, error))

    def fetch_apps(self, force=False):
        """Load the selected device's apps: from the disk cache, or from the device if forced or uncached."""
        device = self.selected()
        if not device or not self.adb or device.serial in self.loading_apps:
            return
        if device.serial in self.apps and not force:
            return
        serial, adb, scrcpy = device.serial, self.adb, self.scrcpy()
        self.loading_apps.add(serial)
        self.show_apps()
        self.bg.run(
            lambda: load_apps(adb, scrcpy, self.app_cache, serial, force),
            lambda apps, error: self.on_apps(serial, apps, error),
        )

    def on_info(self, serial, info, error):
        if serial != self.serial:
            return
        if error:
            self.details.setText(f"{serial}\nCouldn't read device info: {error}")
            return
        self.name_label.setText(info.name)
        self.details.setText(
            f"{info.model}\nAndroid {info.android}\nBattery {info.battery}% · {info.power}\n{serial}"
        )
        supported = info.sdk >= VIRTUAL_DISPLAY_MIN_SDK
        self.app_grid.setEnabled(supported)
        self.vd_size.setEnabled(supported)
        if not supported:
            self.app_hint.setText("App windows need Android 10 or newer.")

    def on_apps(self, serial, apps, error):
        self.loading_apps.discard(serial)
        if not error:
            self.apps[serial] = apps
        if serial == self.serial:
            self.show_apps()
            if error:
                self.app_hint.setText(f"Couldn't list apps: {error}\nPress Refresh to try again.")

    def show_apps(self):
        self.app_grid.clear()
        text = self.filter.text().lower()
        apps = self.apps.get(self.serial, [])
        for app in apps:
            if text in app.name.lower() or text in app.package:
                item = QListWidgetItem(QIcon(app.icon) if app.icon else avatar(app.name), app.name)
                item.setData(Qt.UserRole, app.package)
                item.setToolTip(app.package)
                self.app_grid.addItem(item)
        if self.serial in self.loading_apps:
            self.app_hint.setText("Loading apps…")
        elif apps:
            self.app_hint.setText("Double-click an app to open it in its own window.")
        else:
            self.app_hint.clear()

    # --- wireless -----------------------------------------------------------

    def go_wireless(self):
        device = self.selected()
        if not device:
            return
        adb = self.adb
        self.statusBar().showMessage("Switching to Wi-Fi…")

        def done(address, error):
            self.statusBar().clearMessage()
            if error:
                QMessageBox.warning(self, "Go wireless", str(error))
            else:
                self.statusBar().showMessage(f"Connected over Wi-Fi at {address}. You can unplug the cable.", 8000)
                self.poll()

        self.bg.run(lambda: adb.go_wireless(device.serial), done)

    def pair_dialog(self):
        if not self.adb:
            return
        dialog = QDialog(self, windowTitle="Pair over Wi-Fi")
        form = QFormLayout(dialog)
        form.addRow(
            QLabel(
                "On the phone: Developer options → Wireless debugging → Pair device with pairing code.\n"
                "The phone and this computer must be on the same network.\n"
                "Already paired? Just fill in the connect address.",
                wordWrap=True,
            )
        )
        pair_address = QLineEdit(placeholderText="192.168.1.20:37099")
        code = QLineEdit(placeholderText="123456")
        connect_address = QLineEdit(placeholderText="192.168.1.20:41235 (from the Wireless debugging screen)")
        form.addRow("Pairing IP:port", pair_address)
        form.addRow("Pairing code", code)
        form.addRow("Connect IP:port", connect_address)
        form.addRow(
            QDialogButtonBox(
                QDialogButtonBox.Ok | QDialogButtonBox.Cancel, accepted=dialog.accept, rejected=dialog.reject
            )
        )
        if dialog.exec() != QDialog.Accepted:
            return

        adb = self.adb
        pair, pin, address = pair_address.text().strip(), code.text().strip(), connect_address.text().strip()

        def job():
            if pair and pin:
                adb.pair(pair, pin)
            if address:
                adb.connect(address)

        def done(_, error):
            if error:
                QMessageBox.warning(self, "Pair over Wi-Fi", str(error))
            else:
                self.statusBar().showMessage("Done. The device will show up in the list shortly.", 6000)
                self.poll()

        self.bg.run(job, done)

    # --- mirroring ----------------------------------------------------------

    def mirror(self):
        device = self.selected()
        if device:
            self.launch(["-s", device.serial, f"--window-title={self.name_label.text()}"])

    def launch_app(self, item):
        device = self.selected()
        size = self.vd_size.currentText().strip()
        if not device:
            return
        if size != "Device size" and not re.fullmatch(r"(\d+x\d+)?(/\d+)?", size):
            QMessageBox.warning(self, "App window size", "Use WIDTHxHEIGHT/DPI, e.g. 1920x1080/280.")
            return
        self.settings.setValue("vd_size", size)
        display = "--new-display" if size == "Device size" else f"--new-display={size}"
        self.launch(
            [
                "-s",
                device.serial,
                display,
                f"--start-app={item.data(Qt.UserRole)}",
                f"--window-title={item.text()} · {self.name_label.text()}",
            ]
        )

    def launch(self, args):
        scrcpy = self.scrcpy()
        if not scrcpy or not self.adb:
            QMessageBox.warning(self, "MyDevice", "scrcpy isn't installed yet.")
            return
        proc = subprocess.Popen(
            [scrcpy, *args],
            env=scrcpy_env(self.adb.path),
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            creationflags=NO_WINDOW,
        )
        self.sessions.append(proc)

        def done(output, error):
            self.sessions.remove(proc)
            if proc.returncode != 0 and output is not None:
                lines = output.decode(errors="replace").splitlines()
                errors = [line for line in lines if "ERROR" in line] or lines[-5:]
                QMessageBox.warning(self, "scrcpy", "\n".join(errors) or f"scrcpy exited with {proc.returncode}")

        self.bg.run(lambda: proc.communicate()[0], done, long_running=True)

    def closeEvent(self, event):
        for proc in self.sessions:
            proc.terminate()
        super().closeEvent(event)
