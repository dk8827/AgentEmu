"""Development-only loopback server. Exposes only web assets and test fixtures."""

from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import argparse

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument("--port", type=int, default=8765)
parser.add_argument("--isolated", action="store_true")
args = parser.parse_args()


class Handler(SimpleHTTPRequestHandler):
    def __init__(self, *a, **kw):
        super().__init__(*a, directory=str(ROOT / "web"), **kw)

    def translate_path(self, path):
        if path.split("?")[0].startswith("/fixtures/"):
            base = ROOT / "fixtures"
            target = (base / path.split("?")[0][10:]).resolve()
            if target.is_relative_to(base):
                return str(target)
            return str(base / "not-found")
        return super().translate_path(path)

    def end_headers(self):
        if args.isolated:
            self.send_header("Cross-Origin-Opener-Policy", "same-origin")
            self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


print(f"AgentEmu: http://127.0.0.1:{args.port}", flush=True)
ThreadingHTTPServer(("127.0.0.1", args.port), Handler).serve_forever()
