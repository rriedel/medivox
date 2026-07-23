#!/usr/bin/env python3
"""
Test-Harness für Streaming-Transkription.

Lädt ein WAV-File und speist es in Echtzeit-Geschwindigkeit in die Engine ein,
simuliert dabei die Fenster-Transkription und Stabilisierung des Shim.

Damit lassen sich verschiedene Parameter reproduzierbar testen:
- STREAM_WINDOW_MS: Fenster-Größe
- STREAM_HOLDBACK_TOKENS: Wie viele Tokens am Ende noch nicht committed
- STREAM_STABLE_PASSES: Wie viele Hypothesen müssen gleich sein zum Committen
- STREAM_TICK_MS: Wie oft neue Fenster transkribiert werden

Beispiel:
    python test_streaming_transcription.py test_recording.wav \\
        --window-ms 12000 \\
        --holdback 4 \\
        --stable-passes 2 \\
        --tick-ms 1200

Ausgabe: Zeigt Preview-Text bei jedem Tick und finales Ergebnis.
"""

import argparse
import time
import wave
from collections import deque
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

import numpy as np

from medivox_engine.config import config
from medivox_engine.transcription import TranscriptionEngine

# Versuche soundfile zu nutzen (besser für WAVE_FORMAT_EXTENSIBLE)
try:
    import soundfile as sf
    HAS_SOUNDFILE = True
except ImportError:
    HAS_SOUNDFILE = False


@dataclass
class Hypothesis:
    raw: list[str]
    norm: list[str]


class StabilizationEngine:
    """Simuliert die Stabilisierungs-Logik des macOS Shim."""

    def __init__(self, stable_passes: int, holdback_tokens: int, verbose: bool = False):
        self.committed_raw: list[str] = []
        self.committed_norm: list[str] = []
        self.hypotheses: deque[Hypothesis] = deque()
        self.stable_passes = stable_passes
        self.holdback_tokens = holdback_tokens
        self.verbose = verbose

    def push_hypothesis(self, text: str) -> None:
        """Verarbeite eine neue Hypothesis vom Engine."""
        hyp = self._tokenize_words(text)
        if not hyp.raw:
            return

        # Finde committed Frontier und schneide es raus
        skip = self._find_committed_frontier(self.committed_norm, hyp.norm)
        if skip > 0:
            print(f"    [FRONTIER] skip={skip}, removed: {' '.join(hyp.raw[:skip])}")
            hyp.raw = hyp.raw[skip:]
            hyp.norm = hyp.norm[skip:]
        elif self.committed_raw:
            # Kein Overlap gefunden, aber wir haben committed Text
            committed_end = " ".join(self.committed_raw[-5:])
            hyp_start = " ".join(hyp.raw[:5])
            print(f"    [NO_FRONTIER] committed_end='{committed_end}' vs hyp_start='{hyp_start}'")

        if not hyp.raw:
            return

        self.hypotheses.append(hyp)
        while len(self.hypotheses) > max(self.stable_passes, 1):
            self.hypotheses.popleft()

        stable_prefix_len = self._longest_common_prefix_len_all()
        commit_count = max(0, stable_prefix_len - self.holdback_tokens)

        if self.verbose and commit_count > 0:
            print(f"    [COMMIT] stable_prefix={stable_prefix_len}, holdback={self.holdback_tokens}, commit_count={commit_count}")

        if commit_count == 0:
            return

        if self.hypotheses:
            latest = self.hypotheses[-1]
            committed_text = " ".join(latest.raw[:commit_count])
            print(f"    [COMMITTING] {commit_count} tokens: {committed_text}")
            self.committed_raw.extend(latest.raw[:commit_count])
            self.committed_norm.extend(latest.norm[:commit_count])

        for hyp in self.hypotheses:
            n = min(commit_count, len(hyp.raw))
            hyp.raw = hyp.raw[n:]
            hyp.norm = hyp.norm[n:]

    def preview_text(self) -> str:
        """Zeige bisherigen committed Text + aktuelle Hypothesis."""
        out = []
        if self.committed_raw:
            out.append(" ".join(self.committed_raw))
        if self.hypotheses:
            latest = self.hypotheses[-1]
            if latest.raw:
                out.append(" ".join(latest.raw))
        return " ".join(out)

    def finalize_text(self) -> str:
        """Finalisiere alle noch nicht committed Tokens."""
        if self.hypotheses:
            latest = self.hypotheses[-1]
            self.committed_raw.extend(latest.raw)
            self.committed_norm.extend(latest.norm)
        self.hypotheses.clear()
        return " ".join(self.committed_raw)

    @staticmethod
    def _tokenize_words(text: str) -> Hypothesis:
        raw = []
        norm = []
        for token in text.split():
            normalized = StabilizationEngine._normalize_token(token)
            if normalized:
                raw.append(token)
                norm.append(normalized)
        return Hypothesis(raw, norm)

    @staticmethod
    def _normalize_token(token: str) -> str:
        """Normalisiere Token: trim punctuation, lowercase."""
        import string

        normalized = token.strip(string.punctuation)
        return normalized.lower()

    def _find_committed_frontier(self, committed: list[str], candidate: list[str]) -> int:
        """Finde wo committed-Suffix im candidate auftaucht."""
        if not committed or not candidate:
            return 0

        max_anchor = min(len(committed), 12)
        min_anchor = min(3, len(committed))

        # Phase 1: Exakter Match
        for anchor_len in range(max_anchor, min_anchor - 1, -1):
            tail = committed[-anchor_len:]
            for pos in range(len(candidate) - anchor_len + 1):
                if candidate[pos : pos + anchor_len] == tail:
                    return pos + anchor_len

        # Phase 2: Fuzzy Match
        for anchor_len in range(max(max_anchor, 4), min_anchor + 3, -1):
            tail = committed[-anchor_len:]
            threshold = 0.75 if anchor_len >= 8 else 0.80

            for pos in range(len(candidate) - anchor_len + 1):
                window = candidate[pos : pos + anchor_len]
                matches = sum(1 for a, b in zip(tail, window) if a == b)
                if matches / anchor_len >= threshold:
                    print(
                        f"  [DEBUG] fuzzy_frontier: anchor_len={anchor_len}, "
                        f"pos={pos}, similarity={matches / anchor_len:.2f}"
                    )
                    return pos + anchor_len

        return 0

    def _longest_common_prefix_len_all(self) -> int:
        if not self.hypotheses:
            return 0

        first = self.hypotheses[0]
        result = len(first.norm)

        for hyp in list(self.hypotheses)[1:]:
            i = 0
            while i < len(first.norm) and i < len(hyp.norm) and first.norm[i] == hyp.norm[i]:
                i += 1
            result = min(result, i)

        return result


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
        description="Test Streaming-Transkription mit WAV-Datei"
    )
    parser.add_argument("wav_file", type=Path, help="WAV-Datei zum Testen")
    parser.add_argument("--window-ms", type=int, default=12000, help="Fenster-Größe (ms)")
    parser.add_argument("--tick-ms", type=int, default=1200, help="Tick-Intervall (ms)")
    parser.add_argument("--holdback", type=int, default=4, help="Holdback-Tokens")
    parser.add_argument("--stable-passes", type=int, default=2, help="Stabile Passes")
    parser.add_argument("--min-transcribe-ms", type=int, default=3000, help="Min Transcribe (ms)")
    parser.add_argument("--verbose", action="store_true", help="Verbose Ausgabe")
    args = parser.parse_args()

    if not args.wav_file.exists():
        print(f"ERROR: {args.wav_file} nicht gefunden")
        return 1

    # Lade Audio
    print(f"Lade {args.wav_file}...")
    audio = load_wav(args.wav_file)
    duration_s = len(audio) / config.sample_rate

    print(f"Audio: {duration_s:.2f}s, {len(audio)} Samples")
    print(
        f"Parameter: window={args.window_ms}ms, tick={args.tick_ms}ms, "
        f"holdback={args.holdback}, stable_passes={args.stable_passes}"
    )
    print("-" * 80)

    # Initialisiere Engine und Stabilisierung
    engine = TranscriptionEngine()
    stabilizer = StabilizationEngine(args.stable_passes, args.holdback, verbose=args.verbose)

    # Berechne Parameter in Samples
    window_samples = int((config.sample_rate * args.window_ms) / 1000)
    tick_samples = int((config.sample_rate * args.tick_ms) / 1000)
    min_transcribe_samples = int((config.sample_rate * args.min_transcribe_ms) / 1000)

    # Ringpuffer
    ring = deque(maxlen=window_samples)

    # Transkriptions-Loop
    tick_count = 0
    pos = 0
    start_time = time.time()
    next_tick_time = 0.0

    while True:
        # Berechne wie viel Zeit seit dem Start verstrichen ist
        elapsed = time.time() - start_time

        # Berechne wie viele Samples bis zur aktuellen Zeit vorhanden sein sollten
        target_pos_samples = int(elapsed * config.sample_rate)

        # Wenn wir über das Ende der Audio hinaus sind, stoppe
        if target_pos_samples >= len(audio):
            break

        # Prüfe ob Zeit für neuen Tick
        if elapsed < next_tick_time:
            time.sleep(0.001)  # Kurz schlafen, dann weitermachen
            continue

        next_tick_time = elapsed + (args.tick_ms / 1000.0)

        # Neuer Tick: Schneide die letzten window_samples aus der Audio
        tick_count += 1
        window_start = max(0, target_pos_samples - window_samples)
        window_end = min(len(audio), target_pos_samples)
        ring_audio = audio[window_start:window_end].copy()

        if len(ring_audio) < min_transcribe_samples:
            if args.verbose:
                ring_duration_s = len(ring_audio) / config.sample_rate
                print(
                    f"Tick {tick_count:2d} ({elapsed:6.2f}s): ring={ring_duration_s:.2f}s, "
                    f"zu wenig Audio ({len(ring_audio)} < {min_transcribe_samples})"
                )
            continue

        # Transkribiere Fenster
        text = engine.transcribe(ring_audio)
        ring_duration_s = len(ring_audio) / config.sample_rate
        if args.verbose:
            print(
                f"Tick {tick_count:2d} ({elapsed:6.2f}s): ring={ring_duration_s:.2f}s, "
                f"engine_output: {text} --- ({len(text)} chars"
            )

        # Verarbeite mit Stabilisierung
        stabilizer.push_hypothesis(text)
        preview = stabilizer.preview_text()

        if preview:
            print(f"Tick {tick_count:2d} ({elapsed:6.2f}s): {preview[:120]}")

    # Finalisiere
    print("-" * 80)
    final_text = stabilizer.finalize_text()
    print(f"FINALES ERGEBNIS: {final_text}")
    return 0


if __name__ == "__main__":
    exit(main())
