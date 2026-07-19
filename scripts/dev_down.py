#!/usr/bin/env python3
"""Stop Lifeguard Tilt (systemd on shared-k8s, or foreground tilt down on Kind)."""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

KUBECONFIG = (
    Path.home()
    / "Workspace/microscaler/shared-gitops-k8s-cluster/kubeconfig/shared-k8s.yaml"
)


def log_info(msg: str) -> None:
    print(f"[INFO] {msg}")


def use_shared_k8s() -> bool:
    mode = os.environ.get("TILT_K8S_CLUSTER", "").strip().lower()
    if mode in ("kind", "kind-kind"):
        return False
    if mode in ("shared-k8s", "k3s"):
        return True
    return KUBECONFIG.is_file()


def main() -> None:
    log_info("Stopping Lifeguard development environment...")
    if use_shared_k8s():
        subprocess.run(
            ["systemctl", "--user", "stop", "tilt-lifeguard.service"],
            check=False,
        )
        log_info("Stopped tilt-lifeguard.service (shared-k8s cluster unchanged)")
    else:
        subprocess.run(["tilt", "down", "--port", "10350"], check=False)
        log_info("Tilt stopped (Kind cluster unchanged)")


if __name__ == "__main__":
    main()
