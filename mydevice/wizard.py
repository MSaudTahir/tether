"""First-run setup: get scrcpy, pick an adb, connect a phone."""

from pathlib import Path

from PySide6.QtCore import QSettings, Qt, QTimer
from PySide6.QtWidgets import (
    QFileDialog,
    QLabel,
    QListWidget,
    QListWidgetItem,
    QProgressBar,
    QPushButton,
    QVBoxLayout,
    QWizard,
    QWizardPage,
)

from mydevice.adb import Adb
from mydevice.tasks import Bg

ADB_NAMES = {"studio": "Android Studio", "system": "System (PATH)", "bundled": "Bundled with scrcpy"}


class SetupWizard(QWizard):
    def __init__(self, tools, parent=None):
        super().__init__(parent)
        self.setWindowTitle("MyDevice setup")
        self.setMinimumSize(620, 440)
        self.setAttribute(Qt.WA_DeleteOnClose)  # stops the device polling when closed
        self.tools = tools
        self.settings = QSettings()
        self.bg = Bg(self)
        self.addPage(ScrcpyPage(self))
        self.addPage(AdbPage(self))
        self.addPage(ConnectPage(self))

    def accept(self):
        self.settings.setValue("setup_done", True)
        super().accept()


class ScrcpyPage(QWizardPage):
    def __init__(self, wizard):
        super().__init__()
        self.w = wizard
        self.downloading = False
        self.setTitle("Get scrcpy")
        self.setSubTitle("MyDevice uses scrcpy to show your phone's screen. It's free and open source.")

        self.status = QLabel(wordWrap=True)
        self.progress = QProgressBar(visible=False)
        self.progress.setRange(0, 0)
        self.retry = QPushButton("Try again", visible=False, clicked=self.download)
        own = QPushButton("Use my own scrcpy…", clicked=self.choose_own)

        layout = QVBoxLayout(self)
        layout.addWidget(self.status)
        layout.addWidget(self.progress)
        layout.addWidget(self.retry, alignment=Qt.AlignLeft)
        layout.addStretch()
        layout.addWidget(own, alignment=Qt.AlignLeft)

    def scrcpy(self):
        custom = self.w.settings.value("scrcpy", "")
        return custom if custom and Path(custom).is_file() else self.w.tools.bundled_file("scrcpy")

    def initializePage(self):
        if self.scrcpy():
            self.show_ready()
        else:
            self.download()

    def isComplete(self):
        return not self.downloading and bool(self.scrcpy())

    def download(self):
        def install():
            release = self.w.tools.latest_release()
            if not release:
                raise RuntimeError("There's no prebuilt scrcpy for this platform. Install it and choose it below.")
            self.w.tools.install(*release)

        self.downloading = True
        self.retry.hide()
        self.progress.show()
        self.status.setText("Downloading the latest scrcpy…")
        self.w.bg.run(install, self.downloaded)

    def downloaded(self, _, error):
        self.downloading = False
        self.progress.hide()
        if error:
            self.status.setText(f"Couldn't download scrcpy:\n{error}")
            self.retry.show()
        else:
            self.w.settings.setValue("scrcpy", "")
            self.show_ready()
        self.completeChanged.emit()

    def choose_own(self):
        path, _ = QFileDialog.getOpenFileName(self, "Choose scrcpy")
        if path:
            self.w.settings.setValue("scrcpy", path)
            self.show_ready()
            self.completeChanged.emit()

    def show_ready(self):
        custom = self.w.settings.value("scrcpy", "")
        version = self.w.tools.installed_version()
        self.status.setText(f"✓ Using {custom}" if custom else f"✓ scrcpy {version} is ready.")


class AdbPage(QWizardPage):
    def __init__(self, wizard):
        super().__init__()
        self.w = wizard
        self.setTitle("Choose adb")
        self.setSubTitle("adb is how your computer talks to the phone. If you use Android Studio, sharing its adb avoids conflicts.")

        self.list = QListWidget()
        self.list.currentItemChanged.connect(self.completeChanged)
        browse = QPushButton("Browse…", clicked=self.browse)

        layout = QVBoxLayout(self)
        layout.addWidget(self.list)
        layout.addWidget(browse, alignment=Qt.AlignLeft)

    def initializePage(self):
        self.list.clear()
        auto = self.w.tools.resolve_adb("auto")
        self.add("auto", f"Auto (recommended): {auto}" if auto else "Auto: no adb found")
        for key, path in self.w.tools.adb_candidates().items():
            self.add(key, f"{ADB_NAMES[key]}: {path}")
        current = self.w.settings.value("adb", "auto")
        if current not in ("auto", *ADB_NAMES):
            self.add(current, f"Custom: {current}")
        self.select(current)

    def add(self, key, label):
        item = QListWidgetItem(label)
        item.setData(Qt.UserRole, key)
        self.list.addItem(item)

    def select(self, key):
        for i in range(self.list.count()):
            if self.list.item(i).data(Qt.UserRole) == key:
                self.list.setCurrentRow(i)

    def browse(self):
        path, _ = QFileDialog.getOpenFileName(self, "Choose adb")
        if path:
            self.add(path, f"Custom: {path}")
            self.select(path)

    def isComplete(self):
        item = self.list.currentItem()
        return bool(item and self.w.tools.resolve_adb(item.data(Qt.UserRole)))

    def validatePage(self):
        self.w.settings.setValue("adb", self.list.currentItem().data(Qt.UserRole))
        return True


class ConnectPage(QWizardPage):
    def __init__(self, wizard):
        super().__init__()
        self.w = wizard
        self.adb = None
        self.polling = False
        self.setTitle("Connect your phone")
        self.setSubTitle("You can skip this and connect, or pair over Wi-Fi, later.")

        steps = QLabel(
            "<ol>"
            "<li>On the phone, open <b>Settings → About phone</b> and tap <b>Build number</b> 7 times.</li>"
            "<li>Go to <b>Settings → System → Developer options</b> and turn on <b>USB debugging</b>.</li>"
            "<li>Plug the phone in and tap <b>Allow</b> on the “Allow USB debugging?” prompt.</li>"
            "</ol>",
            wordWrap=True,
        )
        self.status = QLabel()
        self.status.setStyleSheet("font-weight: 600;")
        self.timer = QTimer(self, interval=2000, timeout=self.poll)

        layout = QVBoxLayout(self)
        layout.addWidget(steps)
        layout.addWidget(self.status)
        layout.addStretch()

    def initializePage(self):
        self.adb = Adb(self.w.tools.resolve_adb(self.w.settings.value("adb", "auto")))
        self.status.setText("Waiting for a device…")
        self.timer.start()
        self.poll()

    def cleanupPage(self):
        self.timer.stop()

    def poll(self):
        if not self.polling:
            self.polling = True
            self.w.bg.run(self.adb.devices, self.on_devices)

    def on_devices(self, devices, error):
        self.polling = False
        if error:
            self.status.setText(f"adb: {error}")
        elif ready := next((d for d in devices if d.state == "device"), None):
            self.status.setText(f"✓ {ready.label} is connected. Click Finish.")
        elif devices:
            self.status.setText(f"Found {devices[0].label}. Tap Allow on the phone.")
        else:
            self.status.setText("Waiting for a device…")
