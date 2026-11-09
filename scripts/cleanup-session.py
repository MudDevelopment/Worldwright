"""Linux launcher session cleanup using pidfds, not reusable PID signaling.
The launcher starts Studio with setsid; preview groups remain in that session.
"""
import os
from pathlib import Path
import select
import signal
import sys
import time


def session_members(session_id: int) -> list[int]:
    found = []
    for entry in Path('/proc').iterdir():
        if not entry.name.isdecimal(): continue
        try:
            if entry.stat().st_uid != os.geteuid(): continue
            text = (entry / 'stat').read_text()
            fields = text[text.rfind(')') + 2:].split()
            if int(fields[3]) == session_id: found.append(int(entry.name))
        except (OSError, ValueError, IndexError):
            continue
    return found


def cleanup(session_id: int, seconds: float = 10) -> bool:
    if session_id <= 1 or session_id == os.getsid(0):
        raise ValueError('refusing launcher/invalid session')
    if not hasattr(os, 'pidfd_open') or not hasattr(signal, 'pidfd_send_signal'):
        raise RuntimeError('Linux pidfd support required; cleanup cannot silently degrade')
    descriptors = {}
    try:
        deadline = time.monotonic() + seconds
        signaled = set()
        while True:
            for pid in session_members(session_id):
                if pid in descriptors: continue
                try:
                    fd = os.pidfd_open(pid)
                    # Recheck membership after pinning lifetime to avoid PID reuse.
                    if pid not in session_members(session_id): os.close(fd); continue
                    descriptors[pid] = fd
                except ProcessLookupError: continue
            for pid, fd in descriptors.items():
                if pid == session_id and pid not in signaled:
                    try: signal.pidfd_send_signal(fd, signal.SIGTERM)
                    except ProcessLookupError: pass
                    signaled.add(pid)
            alive = [fd for fd in descriptors.values() if not select.select([fd], [], [], 0)[0]]
            if not alive and not session_members(session_id): return True
            if time.monotonic() >= deadline:
                for fd in alive:
                    try: signal.pidfd_send_signal(fd, signal.SIGKILL)
                    except ProcessLookupError: pass
                end = time.monotonic() + 3
                while time.monotonic() < end:
                    if all(select.select([fd], [], [], 0)[0] for fd in descriptors.values()): return False
                    time.sleep(0.02)
                raise RuntimeError('session processes survived forced cleanup')
            # Exited zombies are not executable; the owning parent/init reaps them.
            if not alive: return True
            time.sleep(0.05)
    finally:
        for fd in descriptors.values(): os.close(fd)


if __name__ == '__main__':
    if len(sys.argv) != 2: raise SystemExit('one Studio session ID required')
    try:
        graceful = cleanup(int(sys.argv[1]))
        if not graceful: print('Studio session required forced cleanup', file=sys.stderr)
        raise SystemExit(0 if graceful else 1)
    except (OSError, ValueError, RuntimeError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
