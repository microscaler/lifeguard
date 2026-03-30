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
    """Set kubeconfig context to kind cluster."""
    log_info("Setting kubeconfig context...")
    result = subprocess.run(
        ["kubectl", "config", "use-context", "kind-lifeguard-test"],
        capture_output=True,
        text=True
    )
    if result.returncode != 0:
        log_info("⚠️  Warning: Could not set kind context, using current context")
    else:
        log_info("✅ Context set to kind-lifeguard-test")


def start_tilt():
    """Start Tilt development environment."""
    log_info("🎯 Starting Tilt...")
    log_info("   Tilt UI: http://localhost:10350")
    log_info("   Postgres primary :6543 | replica-0 :6544 | Redis :6545 | replica-1 :6546 (CI-parity ports)")
    log_info("   Observability: Grafana :3000 | Prometheus :9090 | Loki :3100 | OTEL :4317 gRPC / :4318 HTTP / :9464 metrics")
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
