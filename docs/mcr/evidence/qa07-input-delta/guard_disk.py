"""Allocated disk snapshots tolerate only ENOENT races inside this job's private TMP."""
from pathlib import Path
import subprocess
import time


def measured_du(path, ephemeral, races, runner=subprocess.run):
    path, ephemeral = Path(path), Path(ephemeral)
    if path == ephemeral and not path.exists():
        return 0
    result = runner(['du', '-sk', str(path)], text=True, capture_output=True)
    lines = result.stdout.splitlines()
    total = None
    if lines:
        try:
            total = int(lines[-1].split()[0]) * 1024
        except (ValueError, IndexError):
            pass
    if result.returncode == 0 and total is not None and total >= 0:
        return total
    errors = result.stderr.splitlines()
    missing_only = bool(errors) and all(line.endswith(': No such file or directory') for line in errors)
    if path == ephemeral and result.returncode == 1 and missing_only:
        if total is None and not path.exists():
            total = 0
        if total is not None and total >= 0:
            if len(races) < 32:
                races.append({'epoch': time.time(), 'detail': result.stderr[:1200], 'partialAllocatedBytes': total})
            return total
    raise subprocess.CalledProcessError(result.returncode or 1, result.args, output=result.stdout, stderr=result.stderr)
