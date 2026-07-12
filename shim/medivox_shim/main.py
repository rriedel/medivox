import logging

import pystray
from pystray._base import Icon as PystrayIcon

from . import client
from .audio import Recorder
from .config import config
from .hotkey import GlobalHotkey
from .inject import type_text
from .logging_config import configure_logging
from .tray import TrayIcon

logger = logging.getLogger(__name__)


def main() -> None:
    configure_logging(config.log_level)
    recorder = Recorder()
    state: dict[str, bool] = {"recording": False}

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
        except Exception:  # Netzwerk-/Engine-Fehler nicht die Anwendung abstuerzen lassen
            logger.exception("Transkription fehlgeschlagen")
            return
        logger.info("Transkriptionsergebnis: %s", text)
        if not text:
            return
        try:
            type_text(text)
        except OSError:
            logger.exception("Texteingabe fehlgeschlagen")

    def on_quit(icon: PystrayIcon, item: pystray.MenuItem) -> None:
        hotkey.stop()
        tray.stop()

    hotkey = GlobalHotkey(config.hotkey_modifiers, config.hotkey_vk, on_toggle)
    tray = TrayIcon(on_quit)

    hotkey.start()
    tray.run_detached()
    hotkey.join()


if __name__ == "__main__":
    main()
