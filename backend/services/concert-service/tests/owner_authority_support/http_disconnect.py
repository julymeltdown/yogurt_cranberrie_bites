"""Fixture-only real loopback TCP reset, with the login token supplied by pipe."""
import json
import socket
import struct
import sys

request = json.loads(sys.stdin.readline())
host, port = request["address"].rsplit(":", 1)
if (host != "127.0.0.1" or not 0 < int(port) < 65536
        or any(c in request["bearer"] for c in "\r\n")
        or any(c in request["target"] for c in "\r\n")):
    raise SystemExit("invalid owned fixture request")
with socket.create_connection((host, int(port)), timeout=5) as stream:
    # Python supplies platform constants; no handwritten native ABI or extra
    # dependency enters Concert. Closing this socket sends an actual TCP reset.
    stream.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, struct.pack("@ii", 1, 0))
    stream.sendall(("GET " + request["target"] + " HTTP/1.1\r\nHost: " + request["address"]
                    + "\r\nAuthorization: Bearer " + request["bearer"]
                    + "\r\nConnection: close\r\n\r\n").encode())
    if sys.stdin.readline() != "DROP\n":
        raise SystemExit("missing fixture drop command")
