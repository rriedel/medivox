import numpy as np
import requests

from .config import config


def transcribe(audio: np.ndarray) -> str:
    url = f"http://{config.engine_host}:{config.engine_port}/transcribe"
    response = requests.post(
        url,
        data=audio.astype(np.float32).tobytes(),
        headers={"Content-Type": "application/octet-stream"},
        timeout=config.request_timeout_seconds,
    )
    response.raise_for_status()
    return response.text
