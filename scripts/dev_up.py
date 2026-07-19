#!/usr/bin/env python3
"""
Development environment startup for Lifeguard.

Default (ms02): shared-k8s platform + systemd tilt-lifeguard.service (port 10355).
Kind fallback: set TILT_K8S_CLUSTER=kind (see shared-gitops-k8s-cluster/config/systemd-kind-override.example).
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
SHARED_K8S = Path.home() / "Workspace/microscaler/shared-gitops-k8s-cluster"
KUBECONFIG = SHARED_K8S / "kubeconfig/shared-k8s.yaml"
TILT_UNIT = "tilt-lifeguard.service"
TILT_PORT = "10355"


def log_info(msg: str) -> None:
    print(f"[INFO] {msg}")


def log_error(msg: str) -> None:
    print(f"[ERROR] {msg}", file=sys.stderr)


def k8s_mode() -> str:
    return os.environ.get("TILT_K8S_CLUSTER", "").strip().lower()


def use_shared_k8s() -> bool:
    mode = k8s_mode()
    if mode in ("kind", "kind-kind"):
        return False
    if mode in ("shared-k8s", "k3s"):
        return True
    return KUBECONFIG.is_file()


def check_command(cmd: str) -> None:
    if not shutil.which(cmd):
        log_error(f"{cmd} is not installed.")
        sys.exit(1)


def check_docker() -> None:
    result = subprocess.run(["docker", "info"], capture_output=True, text=True)
    if result.returncode != 0:
        log_error("Docker daemon is not running")
        sys.exit(1)


def run(cmd: list[str], *, cwd: Path | None = None, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=cwd, check=check, text=True)


def start_kind() -> None:
    log_info("Setting up Kind cluster (TILT_K8S_CLUSTER=kind)...")
    setup_script = REPO / "scripts/setup_kind_cluster.sh"
    result = subprocess.run(["bash", str(setup_script)])
    if result.returncode != 0:
        log_error("Failed to setup Kind cluster")
        sys.exit(1)
    for ctx in ("kind-kind", "kind-lifeguard-test"):
        if run(["kubectl", "config", "use-context", ctx], check=False).returncode == 0:
            log_info(f"Context set to {ctx}")
            break
    log_info("Starting Lifeguard Tilt on port 10350 (Kind mode)...")
    subprocess.run(["tilt", "up", "--host", "0.0.0.0", "--port", "10350"], check=False)


def ensure_platform() -> None:
    env = os.environ.copy()
    env["KUBECONFIG"] = str(KUBECONFIG)
    run(["just", "check-ready"], cwd=SHARED_K8S, check=True)
    result = subprocess.run(
        ["kubectl", "get", "svc", "-n", "data", "minio"],
        env=env,
        capture_output=True,
    )
    if result.returncode != 0:
        log_info("Platform Tilt not up — starting shared-k8s platform...")
        run(["just", "systemd-tilt-up"], cwd=SHARED_K8S, check=False)
        for _ in range(60):
            if subprocess.run(
                ["kubectl", "get", "svc", "-n", "data", "minio"],
                env=env,
                capture_output=True,
            ).returncode == 0:
                break
            time.sleep(2)
    run(["just", "dev-wait-db"], cwd=REPO, check=True)


def start_shared_k8s() -> None:
    log_info("Starting Lifeguard (shared-k8s)...")
    os.environ["KUBECONFIG"] = str(KUBECONFIG)
    check_docker()
    ensure_platform()
    run([sys.executable, str(REPO / "scripts/setup_data_port_forwards.py")], check=True)
    log_info(f"Starting {TILT_UNIT} via systemd (port {TILT_PORT})...")
    run(["systemctl", "--user", "start", TILT_UNIT], check=True)
    for _ in range(60):
        if subprocess.run(
            ["curl", "-sf", f"http://localhost:{TILT_PORT}/api/v1/info"],
            capture_output=True,
        ).returncode == 0:
            log_info(f"Tilt ready at http://0.0.0.0:{TILT_PORT}")
            return
        time.sleep(2)
    log_error(f"Tilt did not become ready on port {TILT_PORT}")
    sys.exit(1)


def main() -> None:
    check_command("kubectl")
    check_command("tilt")
    if use_shared_k8s():
        start_shared_k8s()
    else:
        check_command("kind")
        check_docker()
        start_kind()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print()
        log_info("Shutdown complete")
        sys.exit(0)
