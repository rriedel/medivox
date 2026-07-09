import ctypes
import threading
from ctypes import wintypes
from typing import Callable

user32 = ctypes.windll.user32
kernel32 = ctypes.windll.kernel32

WM_HOTKEY = 0x0312
WM_QUIT = 0x0012
HOTKEY_ID = 1


class GlobalHotkey:
    """Registriert einen system-weiten Toggle-Hotkey und ruft on_trigger bei jedem Druck auf."""

    def __init__(self, modifiers: int, vk: int, on_trigger: Callable[[], None]) -> None:
        self._modifiers = modifiers
        self._vk = vk
        self._on_trigger = on_trigger
        self._thread = threading.Thread(target=self._run, daemon=True)
        self._thread_id: int | None = None

    def start(self) -> None:
        self._thread.start()

    def join(self) -> None:
        self._thread.join()

    def stop(self) -> None:
        if self._thread_id is not None:
            user32.PostThreadMessageW(self._thread_id, WM_QUIT, 0, 0)

    def _run(self) -> None:
        self._thread_id = kernel32.GetCurrentThreadId()
        if not user32.RegisterHotKey(None, HOTKEY_ID, self._modifiers, self._vk):
            raise OSError(
                "Hotkey konnte nicht registriert werden (evtl. bereits von einer anderen Anwendung belegt)."
            )
        try:
            msg = wintypes.MSG()
            while user32.GetMessageW(ctypes.byref(msg), None, 0, 0) != 0:
                if msg.message == WM_HOTKEY and msg.wParam == HOTKEY_ID:
                    self._on_trigger()
        finally:
            user32.UnregisterHotKey(None, HOTKEY_ID)
