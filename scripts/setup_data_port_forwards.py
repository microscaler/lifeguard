#!/usr/bin/env python3
"""Start kubectl port-forwards to shared platform Services (namespace data)."""

from __future__ import annotations

import os
import socket
import subprocess
import sys
import time
from pathlib import Path

FORWARDS: list[tuple[str, int, str, int]] = [
    ("postgres", 5432, "svc/postgres", 5432),
    ("postgres-replica-0", 6544, "svc/postgres-replica-0", 5432),
    ("postgres-replica-1", 6546, "svc/postgres-replica-1", 5432),
    ("redis", 6545, "svc/redis", 6379),
]


def port_open(host: str, port: int) -> bool:
    try:
        with socket.create_connection((host, port), timeout=0.5):
            return True
    except OSError:
        return False


def ensure_kubeconfig() -> None:
    kcfg = os.environ.get("KUBECONFIG")
    if kcfg and Path(kcfg).is_file():
        return
    default = (
        Path.home()
        / "Workspace/microscaler/shared-k8s-cluster/kubeconfig/shared-k8s.yaml"
    )
    if default.is_file():
        os.environ["KUBECONFIG"] = str(default)


def start_forward(name: str, local_port: int, target: str, remote_port: int) -> None:
    if port_open("127.0.0.1", local_port):
        print(f"[OK] localhost:{local_port} already listening ({name})")
        return
    cmd = [
        "kubectl",
        "port-forward",
        "-n",
        "data",
        target,
        f"{local_port}:{remote_port}",
    ]
    subprocess.Popen(
        cmd,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    for _ in range(30):
        if port_open("127.0.0.1", local_port):
            print(f"[OK] port-forward {name} -> localhost:{local_port}")
            return
        time.sleep(0.5)
    print(f"[WARN] port-forward {name} did not bind localhost:{local_port}", file=sys.stderr)


def main() -> int:
    ensure_kubeconfig()
    for name, local_port, target, remote_port in FORWARDS:
        start_forward(name, local_port, target, remote_port)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
