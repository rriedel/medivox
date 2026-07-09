import ctypes
from ctypes import wintypes

INPUT_KEYBOARD = 1
KEYEVENTF_UNICODE = 0x0004
KEYEVENTF_KEYUP = 0x0002

user32 = ctypes.windll.user32


class KEYBDINPUT(ctypes.Structure):
    _fields_ = [
        ("wVk", wintypes.WORD),
        ("wScan", wintypes.WORD),
        ("dwFlags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ctypes.POINTER(wintypes.ULONG)),
    ]


class _InputUnion(ctypes.Union):
    _fields_ = [("ki", KEYBDINPUT)]


class INPUT(ctypes.Structure):
    _fields_ = [("type", wintypes.DWORD), ("union", _InputUnion)]


def _char_input(char: str, key_up: bool) -> INPUT:
    flags = KEYEVENTF_UNICODE | (KEYEVENTF_KEYUP if key_up else 0)
    # wVk muss 0 sein, wenn KEYEVENTF_UNICODE gesetzt ist; wScan traegt den Unicode-Codepoint.
    ki = KEYBDINPUT(0, ord(char), flags, 0, None)
    return INPUT(type=INPUT_KEYBOARD, union=_InputUnion(ki=ki))


def type_text(text: str) -> None:
    """Injiziert Text als Unicode-Tastatureingaben in das aktuell fokussierte Fenster.

    Funktioniert nicht, wenn das Zielfenster mit hoeheren Rechten (erhoeht/Administrator)
    laeuft als dieser Prozess -- Windows UIPI blockiert das dann.
    """
    if not text:
        return
    inputs = []
    for char in text:
        if char == "\n":
            char = "\r"
        inputs.append(_char_input(char, key_up=False))
        inputs.append(_char_input(char, key_up=True))
    array = (INPUT * len(inputs))(*inputs)
    sent = user32.SendInput(len(inputs), array, ctypes.sizeof(INPUT))
    if sent != len(inputs):
        raise OSError("SendInput konnte nicht alle Eingaben zustellen.")
