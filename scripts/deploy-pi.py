"""Executed over SSH; private runtime configuration stays on the Pi."""

import datetime
import json
from pathlib import Path
import pwd
import shutil
import sqlite3
import subprocess
import sys
from uuid import UUID

root = Path.home() / "d/openhome"


def run(*args, **kwargs):
    return subprocess.run(args, cwd=root, check=True, text=True, **kwargs)


def inspect(kind, name):
    return json.loads(run("docker", kind, "inspect", name, capture_output=True).stdout)[0]


def back_up_data(data, backup):
    """Snapshot databases and referenced photos while the only database writer is stopped."""
    run("docker", "stop", "--time", "30", "openhome-api", stdout=subprocess.DEVNULL)
    try:
        for name in ("app.db", "noop.db"):
            with sqlite3.connect(f'file:{data / name}?mode=ro', uri=True) as source:
                with sqlite3.connect(backup / name) as destination:
                    source.backup(destination)
                    if destination.execute("PRAGMA quick_check").fetchall() != [("ok",)]:
                        raise RuntimeError(f"Backup validation failed for {name}")
            (backup / name).chmod(0o600)
            print(f"{name}: backup verified", flush=True)
        with sqlite3.connect(f'file:{backup / "app.db"}?mode=ro', uri=True) as database:
            # The first release with nutrition backs up a schema without Meal Logs or photos.
            columns = {row[1] for row in database.execute("PRAGMA table_info(meal_logs)")}
            named = [] if "photo_id" not in columns else [row[0] for row in database.execute(
                "SELECT photo_id FROM meal_logs WHERE photo_id IS NOT NULL")]
        photos = backup / "meal-photos"
        photos.mkdir(mode=0o700)
        for photo in named:
            # UUIDs name the stored files. Reject malformed database references instead of
            # allowing backup paths outside the photo directory.
            if str(UUID(photo)) != photo:
                raise RuntimeError("Invalid meal photo reference in app.db")
            for suffix in (".avif", ".thumbnail.avif"):
                target = photos / f"{photo}{suffix}"
                shutil.copy2(data / "meal-photos" / target.name, target)
                target.chmod(0o600)
        print(f"meal-photos: {len(named)} photos backed up", flush=True)
    finally:
        # Resume the previous API even when a missing photo or failed disk aborts the release.
        run("docker", "start", "openhome-api", stdout=subprocess.DEVNULL)


def main():
    areas = ("api", "mcp", "dashboard")
    versions = dict(zip(areas, sys.argv[1:4], strict=True))
    released = sys.argv[4].removeprefix("--areas=").split(",")
    verify_only = sys.argv[5:] == ["--verify-only"]
    if not sys.argv[4].startswith("--areas=") or not set(released) <= set(areas):
        raise SystemExit("Usage: deploy-pi.py API MCP DASHBOARD --areas=a,b [--verify-only]")


    # Resolve every release before downtime; a missing tag leaves production running.
    expected_images = {}
    for area, version in versions.items():
        image = f"patrickhaahr/openhome-{area}:{version}"
        run("docker", "pull", image, stdout=subprocess.DEVNULL)
        expected_images[area] = inspect("image", image)["Id"]


    if not verify_only:
        backup = root / "backups" / ("pre-release-" + datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%SZ"))
        backup.mkdir(parents=True, mode=0o700)
        back_up_data(root / "data", backup)
        for name in ("compose.yml", "compose.yaml", "docker-compose.yml", "docker-compose.yaml",
                     "compose.bluetooth.yml", ".env"):
            if (root / name).exists():
                shutil.copy2(root / name, backup / name)
                (backup / name).chmod(0o600)
        images = {area: inspect("container", f"openhome-{area}")["Image"] for area in areas}
        (backup / "previous-images.json").write_text(json.dumps(images, indent=2) + "\n")
        print(f"Backup: {backup}", flush=True)
        # COMPOSE_FILE in the private .env opts into the Bluetooth overlay. Install
        # release-owned configuration only after preserving the previous stack.
        for name in ("compose.yml", "compose.bluetooth.yml"):
            shutil.copy2(Path("/tmp") / ("openhome-" + name), root / name)
        configuration = json.loads(run("docker", "compose", "config", "--format", "json",
                                       capture_output=True).stdout)
        api_environment = configuration["services"]["openhome-api"].get("environment", {})
        if api_environment.get("BACKLIGHT_BLUETOOTH_ADDRESS") and api_environment.get("DBUS_SYSTEM_BUS_ADDRESS"):
            # System D-Bus authenticates the container's numeric UID against the host
            # user database before its existing BlueZ policy can apply.
            try:
                pwd.getpwuid(10001)
            except KeyError:
                run("sudo", "-n", "useradd", "--system", "--uid", "10001", "--no-create-home",
                    "--no-user-group", "--gid", "nogroup", "--shell", "/usr/sbin/nologin", "openhome-api")
                run("sudo", "-n", "busctl", "--system", "call", "org.freedesktop.DBus",
                    "/org/freedesktop/DBus", "org.freedesktop.DBus", "ReloadConfig")
        # Only released services are pulled; `up` recreates just the containers whose image or
        # configuration changed and leaves the rest running.
        run("docker", "compose", "pull", *[f"openhome-{area}" for area in released])

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
        # Containers left running keep their history; only a fresh one must have no restarts.
        if (container["Image"] != expected or not container["State"]["Running"]
                or container["State"].get("Health", {}).get("Status") != "healthy"
                or (area in released and container["RestartCount"] != 0)):
            raise RuntimeError(f"openhome-{area} is not a healthy, stable instance of {version}")
        print(f"openhome-{area}: {version}, correct image, healthy, zero restarts", flush=True)

    environment = inspect("container", "openhome-mcp")["Config"]["Env"]
    host = next(value.split("=", 1)[1].split(",")[0].strip() for value in environment if value.startswith("MCP_ALLOWED_HOSTS="))
    run("docker", "exec", "-i", "-e", f"MCP_SMOKE_HOST={host}", "-e", f'MCP_EXPECTED_VERSION={versions["mcp"]}',
        "openhome-dashboard", "bun", "-", input=Path("/tmp/openhome-deploy-smoke.mjs").read_text(), timeout=60)
    run("docker", "compose", "ps")


if __name__ == "__main__":
    main()
