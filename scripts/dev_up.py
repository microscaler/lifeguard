#!/usr/bin/env python3
"""
Development environment startup script.

Starts Kind cluster and Tilt for local development.
Replaces embedded shell script in justfile.
"""

import os
import subprocess
import sys
from pathlib import Path

def log_info(msg):
    """Print info message."""
    print(f"[INFO] {msg}")


def log_error(msg):
    """Print error message."""
    print(f"[ERROR] {msg}", file=sys.stderr)


def log_warn(msg):
    """Print warning message."""
    print(f"[WARN] {msg}")


def check_command(cmd):
    """Check if a command exists."""
    import shutil
    if not shutil.which(cmd):
        log_error(f"{cmd} is not installed. Please install it first.")
        sys.exit(1)


def check_docker():
    """Check if Docker is running."""
    log_info("Checking Docker daemon...")
    result = subprocess.run(
        ["docker", "info"],
        capture_output=True,
        text=True
    )
    if result.returncode != 0:
        log_error("Docker daemon is not running")
        print("   Please start Docker Desktop and try again")
        sys.exit(1)
    log_info("✅ Docker daemon is running")


def start_kind():
    """Start or create Kind cluster."""
    log_info("Setting up Kind cluster...")
    
    # Run setup script (it already waits for database)
    setup_script = Path(__file__).parent / "setup_kind_cluster.sh"
    result = subprocess.run(
        ["bash", str(setup_script)],
        capture_output=False
    )
    if result.returncode != 0:
        log_error("Failed to setup Kind cluster")
        sys.exit(1)


def set_kubeconfig_context():
    """Use the shared Kind cluster context (kind-kind) or legacy kind-lifeguard-test."""
    log_info("Setting kubeconfig context...")
    for ctx in ("kind-kind", "kind-lifeguard-test"):
        result = subprocess.run(
            ["kubectl", "config", "use-context", ctx],
            capture_output=True,
            text=True,
        )
        if result.returncode == 0:
            log_info(f"✅ Context set to {ctx}")
            return
    log_info(
        "⚠️  Warning: Neither kind-kind nor kind-lifeguard-test is available; "
        "using current kubectl context"
    )


def start_tilt():
    """Start Tilt development environment."""
    log_info("🎯 Starting Lifeguard Tilt (cargo builds/tests — no DB manifests here)...")
    log_info("   Tilt UI: http://localhost:10350")
    log_info("   Postgres/Redis/Grafana: run shared-kind-cluster `tilt up` (UI often :10348), context kind-kind")
    # Run tilt up in foreground (will block until user stops it)
    # KeyboardInterrupt will be caught by main() handler
    subprocess.run(["tilt", "up"], check=False)


def main():
    """Main development environment startup."""
    log_info("🚀 Starting Lifeguard development environment (Kind)...")
    
    # Check prerequisites
    check_command("docker")
    check_command("kind")
    check_command("kubectl")
    check_command("tilt")
    
    # Check Docker is running
    check_docker()
    
    # Start Kind cluster
    start_kind()
    
    # Set kubeconfig context
    set_kubeconfig_context()
    
    # Start Tilt
    start_tilt()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print()
        log_info("🛑 Shutting down gracefully...")
        log_info("   Tilt has been stopped")
        log_info("   Kind cluster is still running (use 'just dev-down' to stop it)")
        print()
        log_info("✅ Shutdown complete")
        sys.exit(0)
