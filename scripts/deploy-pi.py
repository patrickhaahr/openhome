"""Executed over SSH; private runtime configuration stays on the Pi."""

import datetime
import json
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys

root = Path.home() / "d/openhome"
areas = ("api", "mcp", "dashboard")
versions = dict(zip(areas, sys.argv[1:4], strict=True))
verify_only = sys.argv[4:] == ["--verify-only"]


def run(*args, **kwargs):
    return subprocess.run(args, cwd=root, check=True, text=True, **kwargs)


def inspect(kind, name):
    return json.loads(run("docker", kind, "inspect", name, capture_output=True).stdout)[0]


# Resolve every release before downtime; a missing tag leaves production running.
expected_images = {}
for area, version in versions.items():
    image = f"patrickhaahr/openhome-{area}:{version}"
    run("docker", "pull", image, stdout=subprocess.DEVNULL)
    expected_images[area] = inspect("image", image)["Id"]


if not verify_only:
    backup = root / "backups" / ("pre-release-" + datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%SZ"))
    backup.mkdir(parents=True, mode=0o700)
    for name in ("app.db", "noop.db"):
        with sqlite3.connect(f'file:{root / "data" / name}?mode=ro', uri=True) as source:
            with sqlite3.connect(backup / name) as destination:
                source.backup(destination)
                if destination.execute("PRAGMA quick_check").fetchall() != [("ok",)]:
                    raise RuntimeError(f"Backup validation failed for {name}")
        print(f"{name}: backup verified", flush=True)
    for name in ("compose.yml", "compose.yaml", "docker-compose.yml", "docker-compose.yaml"):
        if (root / name).exists():
            shutil.copy2(root / name, backup / name)
    images = {area: inspect("container", f"openhome-{area}")["Image"] for area in areas}
    (backup / "previous-images.json").write_text(json.dumps(images, indent=2) + "\n")
    print(f"Backup: {backup}", flush=True)
    run("docker", "compose", "down")
    run("docker", "compose", "pull")

# Verify the immutable release tags against the latest images Compose will use.
for area, version in versions.items():
    image = f"patrickhaahr/openhome-{area}"
    if expected_images[area] != inspect("image", f"{image}:latest")["Id"]:
        raise RuntimeError(f"{image}:latest differs from release {version}")

if not verify_only:
    run("docker", "compose", "up", "-d", "--wait", "--wait-timeout", "180")

for area, version in versions.items():
    container = inspect("container", f"openhome-{area}")
    expected = expected_images[area]
    if (container["Image"] != expected or not container["State"]["Running"]
            or container["State"].get("Health", {}).get("Status") != "healthy"
            or container["RestartCount"] != 0):
        raise RuntimeError(f"openhome-{area} is not a healthy, stable instance of {version}")
    print(f"openhome-{area}: {version}, correct image, healthy, zero restarts", flush=True)

environment = inspect("container", "openhome-mcp")["Config"]["Env"]
host = next(value.split("=", 1)[1].split(",")[0].strip() for value in environment if value.startswith("MCP_ALLOWED_HOSTS="))
run("docker", "exec", "-i", "-e", f"MCP_SMOKE_HOST={host}", "-e", f'MCP_EXPECTED_VERSION={versions["mcp"]}',
    "openhome-dashboard", "bun", "-", input=Path("/tmp/openhome-deploy-smoke.mjs").read_text(), timeout=60)
run("docker", "compose", "ps")
