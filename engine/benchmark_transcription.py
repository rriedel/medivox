import argparse
import statistics
import time
import wave
from pathlib import Path

import numpy as np

from medivox_engine.config import config
from medivox_engine.transcription import TranscriptionEngine


def _load_wav(path: Path) -> np.ndarray:
    with wave.open(str(path), "rb") as wf:
        channels = wf.getnchannels()
        sample_width = wf.getsampwidth()
        sample_rate = wf.getframerate()
        frames = wf.readframes(wf.getnframes())

    if channels != 1:
        raise ValueError(f"Nur mono WAV wird unterstuetzt, gefunden: {channels} Kanaele")

    if sample_rate != config.sample_rate:
        raise ValueError(
            f"Sample-Rate muss {config.sample_rate} Hz sein, gefunden: {sample_rate} Hz"
        )

    if sample_width == 2:
        audio_i16 = np.frombuffer(frames, dtype=np.int16)
        return (audio_i16.astype(np.float32) / 32768.0).copy()

    if sample_width == 4:
        audio_f32 = np.frombuffer(frames, dtype=np.float32)
        return audio_f32.astype(np.float32, copy=True)

    raise ValueError(f"Nicht unterstuetzte WAV-Bitbreite: {sample_width * 8} bit")


def _load_audio(path: Path) -> np.ndarray:
    suffix = path.suffix.lower()
    if suffix == ".wav":
        return _load_wav(path)
    if suffix == ".npy":
        data = np.load(path)
        return np.asarray(data, dtype=np.float32)
    if suffix == ".f32":
        data = np.fromfile(path, dtype=np.float32)
        return np.asarray(data, dtype=np.float32)
    raise ValueError("Unterstuetzte Formate: .wav, .npy, .f32")


def _run_benchmark(audio: np.ndarray, repeats: int) -> tuple[list[float], str]:
    engine = TranscriptionEngine()

    # Warmup reduziert JIT-/Model-Initialisierungsrauschen in der Messung.
    engine.transcribe(audio)

    durations: list[float] = []
    text = ""
    for _ in range(repeats):
        start = time.perf_counter()
        text = engine.transcribe(audio)
        durations.append(time.perf_counter() - start)

    return durations, text


def main() -> None:
    parser = argparse.ArgumentParser(description="Benchmark fuer medivox Transkription")
    parser.add_argument("audio", type=Path, help="Audio-Datei (.wav, .npy oder .f32)")
    parser.add_argument("--repeats", type=int, default=5, help="Anzahl Messwiederholungen")
    args = parser.parse_args()

    if args.repeats < 1:
        raise ValueError("--repeats muss >= 1 sein")

    audio = _load_audio(args.audio)
    audio_seconds = len(audio) / config.sample_rate

    durations, text = _run_benchmark(audio, args.repeats)

    mean_s = statistics.fmean(durations)
    min_s = min(durations)
    max_s = max(durations)
    p95_s = np.percentile(np.array(durations, dtype=np.float64), 95)

    print(f"Datei: {args.audio}")
    print(f"Dauer Audio: {audio_seconds:.3f}s")
    print(f"Messungen: {args.repeats}")
    print(f"Latenz min/mean/p95/max: {min_s:.3f}s / {mean_s:.3f}s / {p95_s:.3f}s / {max_s:.3f}s")
    print(f"RTF mean: {mean_s / audio_seconds:.3f}")
    print("---")
    print(text)


if __name__ == "__main__":
    main()
