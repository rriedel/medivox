import os
from dataclasses import dataclass, field
from pathlib import Path


def _env_str(name: str, default: str) -> str:
    return os.environ.get(name, default)


def _env_int(name: str, default: int) -> int:
    raw = os.environ.get(name)
    return int(raw) if raw else default


@dataclass
class EngineConfig:
    model_size: str = field(default_factory=lambda: _env_str("MEDIVOX_MODEL_SIZE", "medium"))
    device: str = field(default_factory=lambda: _env_str("MEDIVOX_DEVICE", "cpu"))
    compute_type: str = field(default_factory=lambda: _env_str("MEDIVOX_COMPUTE_TYPE", "int8"))
    cpu_threads: int = field(default_factory=lambda: _env_int("MEDIVOX_CPU_THREADS", 8))
    beam_size: int = 3
    best_of: int = 3
    temperature: tuple[float, ...] = (0.0, 0.2, 0.4, 0.6, 0.8, 1.0)
    repetition_penalty: float = 1.1
    without_timestamps: bool = False
    condition_on_previous_text: bool = False
    vad_filter: bool = True
    vad_min_silence_duration_ms: int = 1000
    vad_threshold: float = 0.3
    language: str = "de"
    host: str = field(default_factory=lambda: _env_str("MEDIVOX_HOST", "127.0.0.1"))
    port: int = field(default_factory=lambda: _env_int("MEDIVOX_PORT", 8123))
    sample_rate: int = 16000
    glossary_path: Path = Path(__file__).resolve().parent.parent / "glossary.txt"
    log_level: str = field(default_factory=lambda: _env_str("MEDIVOX_LOG_LEVEL", "INFO"))


config = EngineConfig()
