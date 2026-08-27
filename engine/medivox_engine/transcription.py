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

    def transcribe(
        self,
        audio: np.ndarray,
        *,
        temperature=None,
        vad_filter: bool | None = None,
        vad_min_silence_duration_ms: int | None = None,
        without_timestamps: bool | None = None,
        beam_size: int | None = None,
        patience: float | None = None,
        repetition_penalty: float | None = None,
    ) -> str:
        start = time.perf_counter()
        logger.info(f"Transcribing audio of length {len(audio)} samples...")
        segments, _ = self._model.transcribe(
            audio,
            language=config.language,
            #initial_prompt=self._initial_prompt or None,
            beam_size=beam_size if beam_size is not None else config.beam_size,
            best_of=config.best_of,
            patience=patience if patience is not None else 1.0,
            repetition_penalty=(
                repetition_penalty if repetition_penalty is not None else config.repetition_penalty
            ),
            temperature=temperature if temperature is not None else config.temperature,
            without_timestamps=(
                without_timestamps if without_timestamps is not None else config.without_timestamps
            ),
            condition_on_previous_text=config.condition_on_previous_text,
            vad_filter=vad_filter if vad_filter is not None else config.vad_filter,
            vad_parameters={
                "min_silence_duration_ms": (
                    vad_min_silence_duration_ms
                    if vad_min_silence_duration_ms is not None
                    else config.vad_min_silence_duration_ms
                )
            },
        )
        texts = []
        for segment in segments:
            logger.info(
                "segment [%.2fs-%.2fs] no_speech=%.2f avg_logprob=%.2f compression_ratio=%.2f: %s",
                segment.start,
                segment.end,
                segment.no_speech_prob,
                segment.avg_logprob,
                segment.compression_ratio,
                segment.text,
            )
            texts.append(segment.text)
        result = "".join(texts).strip()
        elapsed = time.perf_counter() - start
        logger.info("transkription (%.3fs): %s", elapsed, result)
        return result
