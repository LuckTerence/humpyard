#!/usr/bin/env python3
"""A tiny OpenAI-compatible provider for the humpyard demo. No network, no API keys.

It answers any POST .../chat/completions with a canned reply and plausible token counts, so the
demo can show routing and cost without calling a real model. Usage: mock_provider.py [port]
"""
import json
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

# model -> (completion tokens, seconds it takes): small models are quick, big ones are not.
MODELS = {
    "fast-small": (60, 0.03),
    "capable-large": (400, 0.35),
    "long-context": (250, 0.25),
    "local-coder": (120, 0.06),
    "vision": (150, 0.12),
}


def prompt_tokens(body: dict) -> int:
    text = json.dumps(body.get("messages", [])) + json.dumps(body.get("tools", []))
    return max(8, len(text) // 4)


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("content-length", 0))
        body = json.loads(self.rfile.read(length) or b"{}")
        model = body.get("model", "fast-small")
        completion, delay = MODELS.get(model, (100, 0.05))
        time.sleep(delay)
        reply = {
            "id": "mock-1",
            "object": "chat.completion",
            "created": int(time.time()),
            "model": model,
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": f"[answered by {model}]"},
                "finish_reason": "stop",
            }],
            "usage": {
                "prompt_tokens": prompt_tokens(body),
                "completion_tokens": completion,
                "total_tokens": prompt_tokens(body) + completion,
            },
        }
        data = json.dumps(reply).encode()
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *_):
        pass


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 9100
    ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()
