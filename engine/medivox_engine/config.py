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
    temperature: float = 0.0
    without_timestamps: bool = True
    condition_on_previous_text: bool = False
    vad_filter: bool = False
    vad_min_silence_duration_ms: int = 500
    language: str = "de"
    host: str = "127.0.0.1"
    port: int = 8123
    sample_rate: int = 16000
    glossary_path: Path = Path(__file__).resolve().parent.parent / "glossary.txt"
    log_level: str = "INFO"


config = EngineConfig()
