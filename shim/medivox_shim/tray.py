from typing import Callable

import pystray
from PIL import Image, ImageDraw
from pystray._base import Icon as PystrayIcon


def _make_icon(color: str) -> Image.Image:
    img = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw.ellipse((8, 8, 56, 56), fill=color)
    return img


class TrayIcon:
    def __init__(self, on_quit: Callable[[PystrayIcon, pystray.MenuItem], None]) -> None:
        self._icon = pystray.Icon(
            "medivox",
            _make_icon("gray"),
            "medivox -- bereit",
            menu=pystray.Menu(pystray.MenuItem("Beenden", on_quit)),
        )

    def run_detached(self) -> None:
        self._icon.run_detached()

    def set_recording(self, active: bool) -> None:
        self._icon.icon = _make_icon("red" if active else "gray")
        self._icon.title = "medivox -- Aufnahme laeuft" if active else "medivox -- bereit"

    def stop(self) -> None:
        self._icon.stop()
