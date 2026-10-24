"""Release the three OpenHome images through the repository's just recipes."""

import argparse
import fcntl
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tomllib
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
AREAS = ("api", "mcp", "dashboard")


def run(*args, **kwargs):
    return subprocess.run(args, check=True, text=True, **kwargs)


def versions(root):
    result = {
        area: tomllib.loads((root / area / "Cargo.toml").read_text())["package"]["version"]
        for area in ("api", "mcp")
    }
    result["dashboard"] = json.loads((root / "dashboard/package.json").read_text())["version"]
    if any(not re.fullmatch(r"\d+\.\d+\.\d+", version) for version in result.values()):
        raise ValueError("Release versions must use major.minor.patch")
    return result


def bump_versions(root):
    before = versions(root)
    after = {}
    edits = {}
    for area, version in before.items():
        major, minor, patch = version.split(".")
        after[area] = f"{major}.{minor}.{int(patch) + 1}"
        path = root / area / ("package.json" if area == "dashboard" else "Cargo.toml")
        content = path.read_text()
        pattern = (rf'("version"\s*:\s*"){re.escape(version)}(")' if area == "dashboard"
                   else rf'(?m)^(version\s*=\s*"){re.escape(version)}(")$')
        content, count = re.subn(pattern, lambda match: match[1] + after[area] + match[2], content)
        if count != 1:
            raise ValueError(f"Expected one package version in {path}")
        edits[path] = content
    for area in ("api", "mcp"):
        path = root / area / "Cargo.lock"
        content = path.read_text()
        for dependency in (("api",) if area == "api" else ("api", "mcp")):
            old = f'name = "openhome-{dependency}"\nversion = "{before[dependency]}"'
            if content.count(old) != 1:
                raise ValueError(f"Version mismatch for openhome-{dependency} in {path}")
            content = content.replace(old, f'name = "openhome-{dependency}"\nversion = "{after[dependency]}"')
        edits[path] = content
    # Validate every manifest and lockfile before writing any of them.
    for path, content in edits.items():
        path.write_text(content)
    return after


def builder():
    command = ["docker", "buildx", "inspect", "openhome-local"]
    inspected = subprocess.run(command, text=True, capture_output=True)
    if inspected.returncode:
        run("docker", "buildx", "create", "--name", "openhome-local", "--driver", "docker-container",
            "unix:///var/run/docker.sock")
        inspected = run(*command, capture_output=True)
    endpoints = re.findall(r"^Endpoint:\s*(\S+)$", inspected.stdout, re.MULTILINE)
    if endpoints != ["unix:///var/run/docker.sock"]:
        raise ValueError("openhome-local must contain exactly one local Docker socket node")
    if not Path("/proc/sys/fs/binfmt_misc/qemu-aarch64").exists():
        run("docker", "--host", "unix:///var/run/docker.sock", "run", "--privileged", "--rm",
            "tonistiigi/binfmt", "--install", "arm64")
    run("docker", "buildx", "inspect", "openhome-local", "--bootstrap")


def verify_https():
    origin = "https://openhome.haahr.me"
    with urllib.request.urlopen(origin + "/connection", timeout=15) as response:
        html = response.read().decode()
        if response.status != 200 or "Your home is connected." not in html:
            raise ValueError("HTTPS dashboard is not connected")
    assets = set(re.findall(r'(?:src|href)="(/assets/[^"\s]+)"', html))
    if not any(path.endswith(".css") for path in assets) or not any(path.endswith(".js") for path in assets):
        raise ValueError("HTTPS dashboard is missing its CSS/JS assets")
    for path in assets:
        with urllib.request.urlopen(origin + path, timeout=15) as response:
            if response.status != 200:
                raise ValueError(f"HTTPS asset failed: {path}")
    for path in ("/api/health", "/mcp"):
        try:
            urllib.request.urlopen(origin + path, timeout=15)
        except urllib.error.HTTPError as error:
            if error.code != 401:
                raise
        else:
            raise ValueError(f"HTTPS {path} accepted an unauthenticated request")
    print(f"HTTPS verified: connected dashboard, {len(assets)} assets, API/MCP authentication.", flush=True)


def activate(release, verify_only=False):
    run("scp", "scripts/deploy-smoke.mjs", "pi:/tmp/openhome-deploy-smoke.mjs")
    if not verify_only:
        run("scp", "api/compose.yml", "pi:/tmp/openhome-compose.yml")
        run("scp", "api/compose.bluetooth.yml", "pi:/tmp/openhome-compose.bluetooth.yml")
    arguments = [release[area] for area in AREAS]
    if verify_only:
        arguments.append("--verify-only")
    run("ssh", "pi", shlex.join(["python3", "-", *arguments]), input=(ROOT / "scripts/deploy-pi.py").read_text())
    verify_https()


def main(mode):
    if mode == "builder":
        builder()
        return
    release = versions(ROOT)
    pending = ROOT / ".devenv/release.json"
    if mode in ("deploy", "current"):
        if mode == "deploy" and pending.exists():
            raise ValueError("An unfinished release exists; use just deploy-current")
        if mode == "current" and (not pending.exists() or json.loads(pending.read_text()) != release):
            raise ValueError("No unfinished release matches the current versions; use just deploy")
        # A failed release keeps its bumped versions; `deploy-current` resumes them.
        run("ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "pi", "true")
        run("just", "docker-builder")
        if mode == "deploy":
            release = bump_versions(ROOT)
            pending.write_text(json.dumps(release) + "\n")
        print("Release: " + json.dumps(release), flush=True)
        run("just", "api-check", "mcp-check", "dashboard-check", "deployment-check")
        for area in AREAS:
            run("just", f"{area}-docker-push", release[area])
        run("just", "dashboard-docker-smoke", f'patrickhaahr/openhome-dashboard:{release["dashboard"]}')
    activate(release, verify_only=mode == "verify")
    if mode in ("deploy", "current"):
        pending.unlink()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("builder", "deploy", "current", "activate", "verify"))
    mode = parser.parse_args().mode
    lock_dir = ROOT / ".devenv"
    lock_dir.mkdir(exist_ok=True)
    with (lock_dir / "deploy.lock").open("w") as lock:
        try:
            # Builder setup is also called by recipes inside the locked release.
            if mode != "builder":
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            main(mode)
        except (OSError, ValueError, subprocess.CalledProcessError) as error:
            retry = "deploy-current" if (ROOT / ".devenv/release.json").exists() else "deploy"
            sys.exit(f"Deployment stopped: {error}. Fix the failure before retrying with just {retry}.")
