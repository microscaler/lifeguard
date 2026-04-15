#!/usr/bin/env python3
"""
Development environment shutdown script.

Stops Tilt. Does not delete the Kind cluster (shared `kind` / context `kind-kind`).
"""

import subprocess


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


def main():
    """Main development environment shutdown."""
    log_info("🛑 Stopping Lifeguard development environment...")

    # Stop Tilt only — the Kind cluster is shared (default name `kind`, context `kind-kind`).
    # Deleting the cluster is a separate, destructive step: `kind delete cluster --name kind`.
    stop_tilt()

    log_info(
        "ℹ️  Kind cluster was not deleted (reusable shared cluster). "
        "Use `kind delete cluster --name kind` only if you intend to remove it."
    )
    log_info("✅ Development environment stopped")


if __name__ == "__main__":
    main()
