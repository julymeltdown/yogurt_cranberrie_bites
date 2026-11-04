#!/usr/bin/env python3
"""Original QA05 actual-public-World failure driver; source/fixture proof is separate.

Root owns any serving-process restart. --restart-checkpoint emits a fixed request
and waits for a fixed stdin acknowledgment; it never sends a signal or executes SQL.
"""
from __future__ import annotations

import argparse
import asyncio
import json
import sys

from world_capacity_support.report import Failure, check, json_value, read_private
from world_capacity_support.scenarios import run


async def restart_checkpoint():
    print(json.dumps({"event": "world-restart-requested", "owner": "external-producer"}), flush=True)
    reader = asyncio.StreamReader(limit=1024)
    protocol = asyncio.StreamReaderProtocol(reader)
    transport, _ = await asyncio.get_running_loop().connect_read_pipe(lambda: protocol, sys.stdin.buffer)
    try:
        line = await reader.readline()
        check(len(line) <= 512 and json_value(line) == {"checkpoint": "world-restart-complete"}, "restart-checkpoint")
    finally:
        transport.close()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", help="absolute owned mode0600 private input; otherwise stdin")
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--restart-checkpoint", action="store_true",
                        help="request external producer action using sanitized stdout/stdin checkpoint")
    args = parser.parse_args(argv)
    if not args.execute:
        print(json.dumps({"result": "NOT_RUN", "reason": "explicit-execute-required"}))
        return 2
    try:
        check(not args.restart_checkpoint or args.input is not None, "restart-needs-private-file-input")
        config = read_private(args.input)
        report = asyncio.run(run(config, capacity=False,
                                 restart_hook=restart_checkpoint if args.restart_checkpoint else None))
    except Failure as error:
        report = {"result": "FAIL", "failure": error.label}
    except BaseException:
        report = {"result": "FAIL", "failure": "private-input-or-runtime-unavailable"}
    print(json.dumps(report, allow_nan=False, sort_keys=True))
    return 0 if report["result"] == "COMPLETED" else 1


if __name__ == "__main__":
    sys.exit(main())
