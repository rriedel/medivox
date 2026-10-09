from transformers import AutoProcessor, AutoModelForMultimodalLM
from datetime import datetime

model_id = "Qwen/Qwen3-ASR-1.7B-hf"
processor = AutoProcessor.from_pretrained(model_id)
model = AutoModelForMultimodalLM.from_pretrained(model_id, device_map="auto")
print(f"{datetime.now()}: Model loaded on {model.device} with dtype {model.dtype}")

inputs = processor.apply_transcription_request(
    audio="../data/SCHM6771.wav",
).to(model.device, model.dtype)

print(f"{datetime.now()}: Inputs: {inputs}")

output_ids = model.generate(**inputs, max_new_tokens=8000)
print(f"{datetime.now()}: output_ids: {output_ids}")

generated_ids = output_ids[:, inputs["input_ids"].shape[1]:]
print(f"{datetime.now()}: generated_ids: {generated_ids}")

# Raw output includes language tag and <asr_text> marker
#raw = processor.decode(generated_ids)[0]
#print(f"Raw: {raw}")

# Parsed output: dict with "language" and "transcription"
#parsed = processor.decode(generated_ids, return_format="parsed")[0]
#print(f"Parsed: {parsed}")

# Extract only the transcription text
transcription = processor.decode(generated_ids, return_format="transcription_only")
print(f"{datetime.now()}: Transcription: {transcription}")

"""
Raw: language English<asr_text>Mr. Quilter is the apostle of the middle classes, and we are glad to welcome his gospel.
Parsed: {'language': 'English', 'transcription': 'Mr. Quilter is the apostle of the middle classes, and we are glad to welcome his gospel.'}
Transcription: Mr. Quilter is the apostle of the middle classes, and we are glad to welcome his gospel.
"""