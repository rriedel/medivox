import logging
import time

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
        start = time.perf_counter()
        segments, _ = self._model.transcribe(
            audio,
            language=config.language,
            initial_prompt=self._initial_prompt or None,
            beam_size=config.beam_size,
            best_of=config.best_of,
            temperature=config.temperature,
            without_timestamps=config.without_timestamps,
            condition_on_previous_text=config.condition_on_previous_text,
            vad_filter=config.vad_filter,
            vad_parameters={"min_silence_duration_ms": config.vad_min_silence_duration_ms},
        )
        result = "".join(segment.text for segment in segments).strip()
        elapsed = time.perf_counter() - start
        logger.info("transkription (%.3fs): %s", elapsed, result)
        return result
