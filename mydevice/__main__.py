import sys
import traceback
from pathlib import Path

from PySide6.QtCore import QSettings, QStandardPaths
from PySide6.QtWidgets import QApplication, QDialog

# Absolute imports so this file also works as the PyInstaller entry point.
from mydevice.toolchain import Toolchain
from mydevice.ui import STYLE, MainWindow
from mydevice.wizard import SetupWizard


def log_errors(path):
    """Also write uncaught errors to a file: packaged GUI apps have nowhere to print them."""

    def hook(*exc_info):
        with open(path, "a") as f:
            traceback.print_exception(*exc_info, file=f)
        sys.__excepthook__(*exc_info)

    sys.excepthook = hook


def main():
    app = QApplication(sys.argv)
    app.setOrganizationName("mydevice")
    app.setApplicationName("MyDevice")
    app.setStyle("Fusion")  # consistent across platforms; STYLE builds on it
    app.setStyleSheet(STYLE)
    data_dir = Path(QStandardPaths.writableLocation(QStandardPaths.AppDataLocation))
    data_dir.mkdir(parents=True, exist_ok=True)
    log_errors(data_dir / "errors.log")
    tools = Toolchain(data_dir)

    settings = QSettings()
    if not settings.value("setup_done", False, type=bool) or not tools.resolve_scrcpy(settings.value("scrcpy", "")):
        if SetupWizard(tools).exec() != QDialog.Accepted:
            return

    window = MainWindow(tools, data_dir)
    window.show()
    sys.exit(app.exec())


if __name__ == "__main__":
    main()
