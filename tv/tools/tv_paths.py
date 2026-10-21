"""Shared workstation paths; this module is never installed on the TV."""
from pathlib import Path

TV_ROOT = Path(__file__).resolve().parent.parent
SSH = ["sh", str(TV_ROOT / "tools" / "ssh.sh")]
