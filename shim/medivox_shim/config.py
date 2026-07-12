from dataclasses import dataclass

MOD_ALT = 0x0001
MOD_CONTROL = 0x0002
MOD_SHIFT = 0x0004
MOD_WIN = 0x0008

VK_SPACE = 0x20


@dataclass
class ShimConfig:
    engine_host: str = "127.0.0.1"
    engine_port: int = 8123
    sample_rate: int = 16000
    # Standard-Hotkey zum Umschalten: Strg+Alt+Leertaste
    hotkey_modifiers: int = MOD_SHIFT | MOD_CONTROL
    hotkey_vk: int = VK_SPACE
    request_timeout_seconds: int = 60
    log_level: str = "INFO"


config = ShimConfig()
