import ctypes
from ctypes import wintypes

INPUT_KEYBOARD = 1
KEYEVENTF_UNICODE = 0x0004
KEYEVENTF_KEYUP = 0x0002

user32 = ctypes.WinDLL("user32", use_last_error=True)


class MOUSEINPUT(ctypes.Structure):
    _fields_ = [
        ("dx", wintypes.LONG),
        ("dy", wintypes.LONG),
        ("mouseData", wintypes.DWORD),
        ("dwFlags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ctypes.POINTER(wintypes.ULONG)),
    ]


class KEYBDINPUT(ctypes.Structure):
    _fields_ = [
        ("wVk", wintypes.WORD),
        ("wScan", wintypes.WORD),
        ("dwFlags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ctypes.POINTER(wintypes.ULONG)),
    ]


class HARDWAREINPUT(ctypes.Structure):
    _fields_ = [
        ("uMsg", wintypes.DWORD),
        ("wParamL", wintypes.WORD),
        ("wParamH", wintypes.WORD),
    ]


class _InputUnion(ctypes.Union):
    # mi/hi werden nie benutzt, muessen aber Teil des Union sein: Windows berechnet
    # sizeof(INPUT) anhand der echten API-Struktur (inkl. MOUSEINPUT/HARDWAREINPUT) und
    # verweigert den Aufruf mit ERROR_INVALID_PARAMETER, wenn cbSize davon abweicht.
    _fields_ = [
        ("mi", MOUSEINPUT),
        ("ki", KEYBDINPUT),
        ("hi", HARDWAREINPUT),
    ]


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
    inputs: list[INPUT] = []
    for char in text:
        if char == "\n":
            char = "\r"
        inputs.append(_char_input(char, key_up=False))
        inputs.append(_char_input(char, key_up=True))
    array = (INPUT * len(inputs))(*inputs)
    ctypes.set_last_error(0)
    sent = user32.SendInput(len(inputs), array, ctypes.sizeof(INPUT))
    if sent != len(inputs):
        error = ctypes.WinError(ctypes.get_last_error())
        raise OSError(
            f"SendInput hat nur {sent} von {len(inputs)} Eingaben zugestellt: {error}"
        )
