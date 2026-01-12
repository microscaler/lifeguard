#!/usr/bin/env python3
"""
Development environment shutdown script.

Stops Tilt, Kind cluster for local development.
Replaces embedded shell script in justfile.
"""

import subprocess
import sys
from pathlib import Path


def log_info(msg):
    """Print info message."""
    print(f"[INFO] {msg}")


def log_warn(msg):
    """Print warning message."""
    print(f"[WARN] {msg}")


def run_command(cmd, check=False, capture_output=True):
    """Run a command and return the result."""
    result = subprocess.run(
        cmd,
        shell=isinstance(cmd, str),
        capture_output=capture_output,
        text=True,
        check=check
    )
    return result


def stop_tilt():
    """Stop Tilt processes."""
    log_info("Stopping Tilt...")
    # Try to stop tilt gracefully first
    result = run_command(["tilt", "down"], check=False, capture_output=True)
    if result.returncode == 0:
        log_info("✅ Tilt stopped")
    else:
        # Fallback: kill tilt processes
        result = run_command(["pkill", "-f", "tilt up"], check=False)
        if result.returncode == 0:
            log_info("✅ Tilt stopped (via pkill)")
        else:
            log_warn("No Tilt processes found (or already stopped)")


def stop_kind():
    """Stop Kind cluster."""
    log_info("Stopping Kind cluster...")
    result = run_command(
        ["kind", "delete", "cluster", "--name", "lifeguard-test"],
        check=False,
        capture_output=True
    )
    if result.returncode == 0:
        log_info("✅ Kind cluster deleted")
    else:
        # Check if cluster exists
        cluster_check = run_command("kind get clusters", check=False, capture_output=True)
        if "lifeguard-test" in cluster_check.stdout:
            log_warn("Cluster deletion had issues, but continuing with cleanup")
        else:
            log_info("Cluster already deleted or does not exist")


def main():
    """Main development environment shutdown."""
    log_info("🛑 Stopping Lifeguard development environment...")
    
    # Stop Tilt
    stop_tilt()
    
    # Stop Kind cluster
    stop_kind()
    
    log_info("✅ Development environment stopped and cleaned up")


if __name__ == "__main__":
    main()
