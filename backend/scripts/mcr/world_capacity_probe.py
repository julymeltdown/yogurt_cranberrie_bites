#!/usr/bin/env python3
"""Original QA08 public 200-principal/four-room driver. Execute only in a named allocation.

Private JSON enters stdin or an owned mode0600 file. Sanitized JSON exits stdout.
No signup, credential manufacture, SQL access, installation or server control.
"""
from __future__ import annotations

import argparse
import asyncio
import json
import sys

from world_capacity_support.report import Failure, read_private
from world_capacity_support.scenarios import run


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", help="absolute owned mode0600 private input; otherwise stdin")
    parser.add_argument("--execute", action="store_true", help="run against the separately allocated public stack")
    args = parser.parse_args(argv)
    if not args.execute:
        print(json.dumps({"result": "NOT_RUN", "reason": "explicit-execute-required"}))
        return 2
    try:
        config = read_private(args.input)
        report = asyncio.run(run(config))
    except Failure as error:
        report = {"result": "FAIL", "failure": error.label}
    except BaseException:
        report = {"result": "FAIL", "failure": "private-input-or-runtime-unavailable"}
    print(json.dumps(report, allow_nan=False, sort_keys=True))
    return 0 if report["result"] == "COMPLETED" else 1


if __name__ == "__main__":
    sys.exit(main())
