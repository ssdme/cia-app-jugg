import sys
import os
import io
import json

# Ensure UTF-8 output streams regardless of Windows console codepage
try:
    if hasattr(sys.stdout, 'reconfigure'):
        sys.stdout.reconfigure(encoding='utf-8')
    if hasattr(sys.stderr, 'reconfigure'):
        sys.stderr.reconfigure(encoding='utf-8')
except Exception:
    pass

from faster_whisper import WhisperModel

DEFAULT_RAP_PROMPT = (
    "Paroles de musique rap, trap, hip-hop freestyle francais, argot, verlan, punchlines, "
    "Un million cash pour au moins ca, rooftop, reuf, quetru, audimat, Hermes, Chanel, lamelles, cancesva."
)

def transcribe_file(audio_path, model_size="large-v3-turbo"):
    if not os.path.exists(audio_path):
        err_payload = {"error": f"Audio file not found: {audio_path}"}
        print("RESULT:" + json.dumps(err_payload, ensure_ascii=True), flush=True)
        return

    print("PROGRESS:20:Loading Whisper Speech Model (large-v3-turbo)...", flush=True)
    
    # Try large-v3-turbo first for best rap/slang accuracy, fallback to small then base
    model = None
    for candidate in [model_size, "large-v3-turbo", "small", "base"]:
        try:
            model = WhisperModel(candidate, device="cpu", compute_type="int8")
            break
        except Exception as e:
            print(f"PROGRESS:25:Model {candidate} init error: {e}", flush=True)
            continue

    if model is None:
        model = WhisperModel("base", device="cpu", compute_type="int8")

    print("PROGRESS:50:Transcribing audio speech...", flush=True)
    segments, info = model.transcribe(
        audio_path,
        beam_size=5,
        word_timestamps=True,
        vad_filter=True,
        initial_prompt=DEFAULT_RAP_PROMPT
    )

    full_text_parts = []
    output_segments = []

    for seg in segments:
        seg_text = seg.text.strip()
        if seg_text:
            full_text_parts.append(seg_text)
        
        words_list = []
        if seg.words:
            for w in seg.words:
                clean_word = w.word.strip()
                if clean_word:
                    words_list.append({
                        "word": clean_word,
                        "start": round(float(w.start), 3),
                        "end": round(float(w.end), 3),
                        "probability": round(float(w.probability), 2)
                    })

        output_segments.append({
            "start": round(float(seg.start), 3),
            "end": round(float(seg.end), 3),
            "text": seg_text,
            "words": words_list
        })

    print("PROGRESS:100:Transcription complete", flush=True)

    result = {
        "language": getattr(info, "language", "fr"),
        "language_probability": round(float(getattr(info, "language_probability", 1.0)), 2),
        "duration": round(float(getattr(info, "duration", 0.0)), 2),
        "text": " ".join(full_text_parts),
        "segments": output_segments
    }

    print("RESULT:" + json.dumps(result, ensure_ascii=True), flush=True)

if __name__ == "__main__":
    if len(sys.argv) > 1:
        audio = sys.argv[1]
        model = sys.argv[2] if len(sys.argv) > 2 else "large-v3-turbo"
        transcribe_file(audio, model)
    else:
        err_payload = {"error": "No audio path provided"}
        print("RESULT:" + json.dumps(err_payload, ensure_ascii=True), flush=True)
