import sys
from pathlib import Path

from PySide6.QtCore import QSettings, QStandardPaths
from PySide6.QtWidgets import QApplication, QDialog

# Absolute imports so this file also works as the PyInstaller entry point.
from mydevice.toolchain import Toolchain
from mydevice.ui import MainWindow
from mydevice.wizard import SetupWizard


def main():
    app = QApplication(sys.argv)
    app.setOrganizationName("mydevice")
    app.setApplicationName("MyDevice")
    tools = Toolchain(Path(QStandardPaths.writableLocation(QStandardPaths.AppDataLocation)))

    settings = QSettings()
    if not settings.value("setup_done", False, type=bool) or not tools.resolve_scrcpy(settings.value("scrcpy", "")):
        if SetupWizard(tools).exec() != QDialog.Accepted:
            return

    window = MainWindow(tools)
    window.show()
    sys.exit(app.exec())


if __name__ == "__main__":
    main()
