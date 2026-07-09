from dataclasses import dataclass
from pathlib import Path


@dataclass
class EngineConfig:
    model_size: str = "medium"
    device: str = "cpu"
    compute_type: str = "int8"
    cpu_threads: int = 4
    language: str = "de"
    host: str = "127.0.0.1"
    port: int = 8123
    sample_rate: int = 16000
    glossary_path: Path = Path(__file__).resolve().parent.parent / "glossary.txt"


config = EngineConfig()
