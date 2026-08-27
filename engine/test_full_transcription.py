#!/usr/bin/env python3
"""
Test-Harness für vollständige (nicht-streamende) Transkription.

Lädt ein WAV-File komplett und schickt es in einem Rutsch an die Engine,
ohne Fenster-Streaming oder Stabilisierung. Dient als Baseline-Vergleich
zu test_streaming_transcription.py.

Beispiel:
    python test_full_transcription.py test_recording.wav

Ausgabe: Transkriptionszeit und finaler Text.

Jedes transkribierte Segment wird geloggt mit no_speech/avg_logprob/
compression_ratio (siehe TranscriptionEngine.transcribe) - das sind die
Werte, mit denen sich ein Textverlust an einer bestimmten Stelle
eingrenzen lässt.

Hintergrund: Whisper hat gelegentlich ganze Wörter bis Sätze im Ergebnis
komplett ausgelassen, obwohl sie im Audio enthalten waren. Die folgenden
Flags spiegeln Engine-Parameter, mit denen sich das eingrenzen bzw.
beheben ließ. Sie überschreiben testweise die Defaults aus config.py -
die dortigen Defaults spiegeln bereits die unten beschriebenen Fixes
wider, die Flags sind zum Zurückschalten/Vergleichen gedacht.

Flags und die damit behobenen Probleme:

--vad-filter
    Aktiviert faster-whisper's Silero-VAD statt Whisper's eigener
    no_speech-Heuristik. Behebt: ganze Passagen wurden fälschlich als
    "keine Sprache" verworfen (v.a. leise/genuschelte Stellen).
    -> Jetzt Default (config.vad_filter=True).

--temperature-fallback
    Aktiviert Temperature-Fallback (0.0, 0.2, 0.4, 0.6, 0.8, 1.0) statt
    festem temperature=0.0. Ohne Fallback gab es nur einen einzigen
    Decoding-Versuch pro Segment; unsichere Segmente wurden dadurch
    eher als "keine Sprache" gewertet statt neu versucht.
    -> Jetzt Default (config.temperature ist ein Tupel).

--with-timestamps
    Deaktiviert without_timestamps. Ohne Timestamp-Tokens fehlte
    Whisper der interne Fortschrittsmarker innerhalb eines bis zu
    30s-Blocks; das Modell konnte nach einer kurzen Äußerung vorzeitig
    abbrechen und den Rest des Blocks (mehrere Sekunden bis über 30s)
    nie nachdekodieren. Behebt: lange Blöcke mit auffällig wenig Text.
    -> Jetzt Default (config.without_timestamps=False).

--beam-size / --patience
    Breitere bzw. geduldigere Beam-Search. Kandidaten gegen
    Attention-Skips bei ähnlichen, aufeinanderfolgenden Phrasen; im
    konkreten Testfall hat aber repetition-penalty allein ausgereicht.

--repetition-penalty
    Erschwert dem Modell, eine kurz zuvor gesagte, sehr ähnliche Phrase
    als "schon transkribiert" zu behandeln und dabei den Text danach zu
    überspringen. Behebt: fehlender Text nach einer Selbstkorrektur/
    Wiederholung durch den Sprecher (z.B. zwei fast identische Sätze
    hintereinander, wobei nur der erste im Ergebnis auftauchte).
    -> Jetzt Default (config.repetition_penalty=1.1).
"""

import argparse
import time
from pathlib import Path

import numpy as np

from medivox_engine.config import config
from medivox_engine.logging_config import configure_logging
from medivox_engine.transcription import TranscriptionEngine

# Versuche soundfile zu nutzen (besser für WAVE_FORMAT_EXTENSIBLE)
try:
    import soundfile as sf
    HAS_SOUNDFILE = True
except ImportError:
    HAS_SOUNDFILE = False

import wave


def load_wav(path: Path) -> np.ndarray:
    """Lade WAV-Datei mit erwarteter Sample-Rate."""

    # Versuche soundfile zu nutzen (bessere Format-Unterstützung)
    if HAS_SOUNDFILE:
        try:
            audio, sr = sf.read(str(path), dtype=np.float32)
            if sr != config.sample_rate:
                raise ValueError(f"Sample-Rate muss {config.sample_rate} Hz sein, aber {sr} Hz")
            if len(audio.shape) > 1 and audio.shape[1] > 1:
                raise ValueError(f"Nur mono WAV, aber {audio.shape[1]} Kanäle")
            if len(audio.shape) > 1:
                audio = audio[:, 0]
            return audio
        except Exception as e:
            print(f"soundfile Fehler: {e}, fallback zu wave Modul...")

    # Fallback zu standard wave Modul
    with wave.open(str(path), "rb") as wf:
        channels = wf.getnchannels()
        sample_width = wf.getsampwidth()
        sample_rate = wf.getframerate()
        frames = wf.readframes(wf.getnframes())

    if channels != 1:
        raise ValueError(f"Nur mono WAV, aber {channels} Kanäle")
    if sample_rate != config.sample_rate:
        raise ValueError(f"Sample-Rate muss {config.sample_rate} Hz sein, aber {sample_rate} Hz")

    if sample_width == 2:
        audio_i16 = np.frombuffer(frames, dtype=np.int16)
        return (audio_i16.astype(np.float32) / 32768.0).copy()
    elif sample_width == 4:
        return np.frombuffer(frames, dtype=np.float32).astype(np.float32, copy=True)
    else:
        raise ValueError(f"Nicht unterstützte Bit-Breite: {sample_width * 8} bit")


def main():
    parser = argparse.ArgumentParser(
        description="Test vollständige (Batch-)Transkription mit WAV-Datei"
    )
    parser.add_argument("wav_file", type=Path, help="WAV-Datei zum Testen")
    parser.add_argument(
        "--vad-filter", action="store_true",
        help="Aktiviere faster-whisper VAD-Filter (statt engine-interner no_speech-Heuristik)",
    )
    parser.add_argument(
        "--temperature-fallback", action="store_true",
        help="Aktiviere Temperature-Fallback (0.0, 0.2, 0.4, 0.6, 0.8, 1.0) statt festem temperature=0.0",
    )
    parser.add_argument(
        "--with-timestamps", action="store_true",
        help="Deaktiviere without_timestamps, damit lange Blöcke intern korrekt in Sub-Segmente zerlegt werden",
    )
    parser.add_argument(
        "--beam-size", type=int, default=None,
        help="Überschreibe beam_size (Default aus Config, aktuell 3)",
    )
    parser.add_argument(
        "--patience", type=float, default=None,
        help="Beam-Search-Patience (Default 1.0), höher = gründlichere Suche vor Abbruch",
    )
    parser.add_argument(
        "--repetition-penalty", type=float, default=None,
        help="Repetition-Penalty (Default 1.0), >1.0 erschwert das Überspringen ähnlicher Phrasen",
    )
    parser.add_argument("--log-level", default="INFO", help="Log-Level (z.B. DEBUG, INFO)")
    args = parser.parse_args()

    configure_logging(args.log_level)

    if not args.wav_file.exists():
        print(f"ERROR: {args.wav_file} nicht gefunden")
        return 1

    print(f"Lade {args.wav_file}...")
    audio = load_wav(args.wav_file)
    duration_s = len(audio) / config.sample_rate
    print(f"Audio: {duration_s:.2f}s, {len(audio)} Samples")
    print(
        f"Parameter: vad_filter={args.vad_filter}, "
        f"temperature_fallback={args.temperature_fallback}, "
        f"with_timestamps={args.with_timestamps}, "
        f"beam_size={args.beam_size}, patience={args.patience}, "
        f"repetition_penalty={args.repetition_penalty}"
    )
    print("-" * 80)

    engine = TranscriptionEngine()

    start = time.time()
    text = engine.transcribe(
        audio,
        vad_filter=True if args.vad_filter else None,
        temperature=(0.0, 0.2, 0.4, 0.6, 0.8, 1.0) if args.temperature_fallback else None,
        without_timestamps=False if args.with_timestamps else None,
        beam_size=args.beam_size,
        patience=args.patience,
        repetition_penalty=args.repetition_penalty,
    )
    elapsed = time.time() - start

    print("-" * 80)
    print(f"Transkriptionszeit: {elapsed:.2f}s (RTF={elapsed / duration_s:.2f}x)")
    print(f"FINALES ERGEBNIS: {text}")
    return 0


if __name__ == "__main__":
    exit(main())
