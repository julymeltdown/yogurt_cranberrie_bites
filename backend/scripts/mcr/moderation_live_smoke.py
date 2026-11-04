#!/usr/bin/env python3
"""Run BE39 native fixtures only under a fresh explicitly accepted target lease."""
from __future__ import annotations
import argparse
import json
import signal
from pathlib import Path
from moderation_live_support import run

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ("test-binary","accepted-target","frozen-bundle","pg-bin","report"):
        parser.add_argument("--"+name,type=Path,required=True)
    parser.add_argument("--accepted-lease-id",required=True)
    args=parser.parse_args()
    if not args.report.is_absolute() or args.report.exists():parser.error("report must be a new absolute file")
    report={"task":"BE39","result":"NOT_RUN"}
    def interrupted(_signal,_frame):raise KeyboardInterrupt()
    prior=signal.signal(signal.SIGTERM,interrupted)
    try:
        run(args.test_binary,args.accepted_target,args.frozen_bundle,args.pg_bin,args.accepted_lease_id,report)
    finally:
        signal.signal(signal.SIGTERM,prior)
        with args.report.open("x") as stream:json.dump(report,stream,indent=2);stream.write("\n")
    print(json.dumps({k:report.get(k) for k in ("task","result","native_tests","cleanup","failure")}))
    return 0 if report.get("result")=="PASS" and report.get("cleanup")=="PASS" else 1

if __name__=="__main__":raise SystemExit(main())
