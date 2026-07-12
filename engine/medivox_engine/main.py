from collections.abc import AsyncIterator
from contextlib import asynccontextmanager

import numpy as np
import uvicorn
from fastapi import FastAPI, Request
from fastapi.responses import PlainTextResponse

from .config import config
from .logging_config import configure_logging
from .transcription import TranscriptionEngine

engine: TranscriptionEngine | None = None


@asynccontextmanager
async def lifespan(app: FastAPI) -> AsyncIterator[None]:
    global engine
    configure_logging(config.log_level)
    engine = TranscriptionEngine()
    yield


app = FastAPI(title="medivox-engine", lifespan=lifespan)


def _get_engine() -> TranscriptionEngine:
    if engine is None:
        raise RuntimeError("TranscriptionEngine ist noch nicht initialisiert.")
    return engine


@app.post("/transcribe", response_class=PlainTextResponse)
async def transcribe(request: Request) -> str:
    raw = await request.body()
    audio = np.frombuffer(raw, dtype=np.float32)
    return _get_engine().transcribe(audio)


@app.post("/reload-glossary")
def reload_glossary() -> dict[str, str]:
    _get_engine().reload_glossary()
    return {"status": "reloaded"}


def run() -> None:
    uvicorn.run(app, host=config.host, port=config.port)


if __name__ == "__main__":
    run()
