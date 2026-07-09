import sys

from . import client
from .audio import Recorder
from .config import config
from .hotkey import GlobalHotkey
from .inject import type_text
from .tray import TrayIcon


def main() -> None:
    recorder = Recorder()
    state = {"recording": False}

    def on_toggle() -> None:
        if not state["recording"]:
            state["recording"] = True
            tray.set_recording(True)
            recorder.start()
            return

        state["recording"] = False
        tray.set_recording(False)
        audio = recorder.stop()
        if audio.size == 0:
            return
        try:
            text = client.transcribe(audio)
        except Exception as exc:  # Netzwerk-/Engine-Fehler nicht die Anwendung abstuerzen lassen
            print(f"Transkription fehlgeschlagen: {exc}", file=sys.stderr)
            return
        if text:
            type_text(text)

    def on_quit(icon, item) -> None:
        hotkey.stop()
        tray.stop()

    hotkey = GlobalHotkey(config.hotkey_modifiers, config.hotkey_vk, on_toggle)
    tray = TrayIcon(on_quit)

    hotkey.start()
    tray.run_detached()
    hotkey.join()


if __name__ == "__main__":
    main()
