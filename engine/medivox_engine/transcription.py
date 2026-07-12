import logging

import numpy as np
from faster_whisper import WhisperModel

from .config import config
from .glossary import build_initial_prompt, load_glossary

logger = logging.getLogger(__name__)


class TranscriptionEngine:
    def __init__(self) -> None:
        self._model = WhisperModel(
            config.model_size,
            device=config.device,
            compute_type=config.compute_type,
            cpu_threads=config.cpu_threads,
        )
        self._initial_prompt = build_initial_prompt(load_glossary(config.glossary_path))

    def reload_glossary(self) -> None:
        self._initial_prompt = build_initial_prompt(load_glossary(config.glossary_path))

    def transcribe(self, audio: np.ndarray) -> str:
        segments, _ = self._model.transcribe(
            audio,
            language=config.language,
            initial_prompt=self._initial_prompt or None,
            vad_filter=False,
        )
        result = "".join(segment.text for segment in segments).strip()
        logger.info("Transkriptionsergebnis: %s", result)
        return result
