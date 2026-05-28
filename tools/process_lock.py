from __future__ import annotations

import os
from pathlib import Path


_ACTIVE_LOCK_PATHS: set[Path] = set()


class InstanceLock:
    def __init__(self, path: Path, handle):
        self.path = path.resolve()
        self._handle = handle

    def release(self) -> None:
        if self._handle is None:
            return
        try:
            _unlock_handle(self._handle)
        finally:
            self._handle.close()
            self._handle = None
            _ACTIVE_LOCK_PATHS.discard(self.path)

    def __enter__(self):
        return self

    def __exit__(self, _exc_type, _exc, _traceback):
        self.release()


def acquire_instance_lock(path: Path) -> InstanceLock | None:
    path = path.resolve()
    if path in _ACTIVE_LOCK_PATHS:
        return None

    path.parent.mkdir(parents=True, exist_ok=True)
    path.touch(exist_ok=True)
    handle = path.open("r+b")
    try:
        _lock_handle(handle)
    except OSError:
        handle.close()
        return None

    _ACTIVE_LOCK_PATHS.add(path)
    handle.seek(0)
    handle.truncate()
    handle.write(f"{os.getpid()}\n".encode("ascii"))
    handle.flush()
    return InstanceLock(path, handle)


def _lock_handle(handle) -> None:
    handle.seek(0)
    if os.name == "nt":
        import msvcrt

        msvcrt.locking(handle.fileno(), msvcrt.LK_NBLCK, 1)
        return

    import fcntl

    fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)


def _unlock_handle(handle) -> None:
    handle.seek(0)
    if os.name == "nt":
        import msvcrt

        msvcrt.locking(handle.fileno(), msvcrt.LK_UNLCK, 1)
        return

    import fcntl

    fcntl.flock(handle.fileno(), fcntl.LOCK_UN)
