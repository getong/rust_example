# /// script
# requires-python = ">=3.11,<3.14"
# dependencies = [
#   "sentence-transformers>=3.0,<6",
#   "transformers>=4.51,<5",
#   "torch>=2.6,<3",
# ]
# ///
"""Local, single-request-at-a-time Qwen embedding server for the Rust demo.
Run: uv run --script embedding_server.py
Model reference: https://huggingface.co/Qwen/Qwen3-Embedding-0.6B
"""
import argparse
import json
from http.server import BaseHTTPRequestHandler, HTTPServer

MODEL = "Qwen/Qwen3-Embedding-0.6B"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=8000)
    parser.add_argument("--device", default="cpu", help="cpu (default), mps or cuda")
    args = parser.parse_args()

    from sentence_transformers import SentenceTransformer

    class Handler(BaseHTTPRequestHandler):
        def reply(self, status, data):
            body = json.dumps(data).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self):
            if self.path == "/health":
                self.reply(200, {"status": "ok", "model": MODEL, "dimensions": 1024})
            elif self.path == "/v1/models":
                self.reply(200, {"object": "list", "data": [{"id": MODEL, "object": "model"}]})
            else:
                self.reply(404, {"error": "not found"})

        def do_POST(self):
            if self.path != "/v1/embeddings":
                self.reply(404, {"error": "not found"})
                return
            try:
                size = int(self.headers.get("Content-Length", "0"))
                if not 0 < size <= 131072:
                    raise ValueError("request must be between 1 and 131072 bytes")
                body = json.loads(self.rfile.read(size))
                if not isinstance(body, dict):
                    raise ValueError("request must be a JSON object")
                if body.get("model", MODEL) != MODEL:
                    raise ValueError(f"only {MODEL} is loaded")
                if body.get("encoding_format", "float") != "float":
                    raise ValueError("only float encoding is supported")
                text = body.get("input")
                if not isinstance(text, str) or not text.strip():
                    raise ValueError("input must be a non-empty string")
                # Never silently truncate a query. Prefix is supplied by Rust.
                if len(model.tokenizer.encode(text)) > model.max_seq_length:
                    raise ValueError(f"input exceeds {model.max_seq_length} tokens")
            except (ValueError, TypeError) as error:
                self.reply(400, {"error": str(error)})
                return
            try:
                vector = model.encode([text], prompt="", normalize_embeddings=True)[0].tolist()
                self.reply(200, {"object": "list", "model": MODEL,
                    "data": [{"object": "embedding", "index": 0, "embedding": vector}]})
            except Exception as error:
                self.log_error("inference failed: %s", error)
                self.reply(500, {"error": "model inference failed; see server log"})

    # Bind before downloading the model, to detect port conflicts immediately.
    with HTTPServer(("127.0.0.1", args.port), Handler) as server:
        print(f"Loading {MODEL} on {args.device}; first run downloads model weights...", flush=True)
        model = SentenceTransformer(MODEL, device=args.device)
        model.max_seq_length = 8192
        print(f"Ready: http://127.0.0.1:{args.port}/health", flush=True)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass


if __name__ == "__main__":
    main()
