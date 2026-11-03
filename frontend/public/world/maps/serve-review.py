#!/usr/bin/env python3
"""Read-only asset review server; optional AR04 checkout resource override."""
import argparse
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlsplit

parser = argparse.ArgumentParser()
parser.add_argument('--port', type=int, default=8873)
parser.add_argument('--theme-root', type=Path)
args = parser.parse_args()
public = Path(__file__).resolve().parents[2]


class Handler(SimpleHTTPRequestHandler):
    def __init__(self, *a, **kw):
        super().__init__(*a, directory=str(public), **kw)

    def translate_path(self, path):
        pathname = urlsplit(path).path
        if args.theme_root and pathname in ['/world/themes/' + name + '.png' for name in ['revenge', 'parade', 'danger']]:
            return str(args.theme_root / Path(pathname).name)
        return super().translate_path(path)


print('AR02 read-only review http://127.0.0.1:' + str(args.port) + '/world/maps/review.html', flush=True)
ThreadingHTTPServer(('127.0.0.1', args.port), Handler).serve_forever()
