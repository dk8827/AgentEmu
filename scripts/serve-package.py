"""Loopback-only acceptance server for the packaged app under a URL prefix."""

import argparse
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote, urlsplit

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--port", type=int, default=8879)
args = parser.parse_args()
ROOT = Path(__file__).resolve().parents[1] / "dist/web"


class Handler(SimpleHTTPRequestHandler):
    def translate_path(self, path):
        path = unquote(urlsplit(path).path)
        if not path.startswith("/AgentEmu/"):
            return str(ROOT / "__missing__")
        target = (ROOT / path[len("/AgentEmu/") :]).resolve()
        return str(target if target.is_relative_to(ROOT.resolve()) else ROOT / "__missing__")


ThreadingHTTPServer(("127.0.0.1", args.port), Handler).serve_forever()
