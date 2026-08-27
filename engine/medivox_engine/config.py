from dataclasses import dataclass
from pathlib import Path


@dataclass
class EngineConfig:
    model_size: str = "medium"
    device: str = "cpu"
    compute_type: str = "int8"
    cpu_threads: int = 8
    beam_size: int = 3
    best_of: int = 1
    temperature: tuple[float, ...] = (0.0, 0.2, 0.4, 0.6, 0.8, 1.0)
    repetition_penalty: float = 1.1
    without_timestamps: bool = False
    condition_on_previous_text: bool = False
    vad_filter: bool = True
    vad_min_silence_duration_ms: int = 500
    language: str = "de"
    host: str = "127.0.0.1"
    port: int = 8123
    sample_rate: int = 16000
    glossary_path: Path = Path(__file__).resolve().parent.parent / "glossary.txt"
    log_level: str = "INFO"


config = EngineConfig()
