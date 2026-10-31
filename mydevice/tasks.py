import threading

from PySide6.QtCore import QObject, QThreadPool, Signal, Slot


class Bg(QObject):
    """Runs blocking calls off the UI thread and delivers (result, error) back on it."""

    _done = Signal(object, object, object)

    def __init__(self, parent=None):
        super().__init__(parent)
        self._done.connect(self._dispatch)

    def run(self, fn, callback, long_running=False):
        def task():
            try:
                result, error = fn(), None
            except Exception as e:
                result, error = None, e
            try:
                self._done.emit(callback, result, error)
            except RuntimeError:
                pass  # window closed while the task was running

        # Long-running tasks (scrcpy sessions) get their own thread so they can't starve the pool.
        if long_running:
            threading.Thread(target=task, daemon=True).start()
        else:
            QThreadPool.globalInstance().start(task)

    @Slot(object, object, object)
    def _dispatch(self, callback, result, error):
        callback(result, error)
