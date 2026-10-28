"""Release the changed OpenHome images through the repository's just recipes."""

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
MANIFESTS = {"api": "api/Cargo.toml", "mcp": "mcp/Cargo.toml", "dashboard": "dashboard/package.json"}


def run(*args, **kwargs):
    return subprocess.run(args, check=True, text=True, **kwargs)


def version_of(area, text):
    return json.loads(text)["version"] if area == "dashboard" else tomllib.loads(text)["package"]["version"]


def versions(root):
    result = {area: version_of(area, (root / MANIFESTS[area]).read_text()) for area in AREAS}
    if any(not re.fullmatch(r"\d+\.\d+\.\d+", version) for version in result.values()):
        raise ValueError("Release versions must use major.minor.patch")
    return result


def git(root, *args):
    return subprocess.run(["git", *args], cwd=root, check=True, text=True, capture_output=True).stdout


def released_at(root, area, version):
    """The commit that set the area's current version: its last release."""
    found = None
    for commit in git(root, "log", "--format=%H", "--", MANIFESTS[area]).split():
        if version_of(area, git(root, "show", f"{commit}:{MANIFESTS[area]}")) != version:
            break
        found = commit
    if found is None:
        raise ValueError(f"No commit records {area} {version}; name the areas to release")
    return found


def changed_areas(root):
    """Areas with tracked or untracked changes since their last release, in release order."""
    current = versions(root)
    changed = set()
    for area in AREAS:
        since = released_at(root, area, current[area])
        if (subprocess.run(["git", "diff", "--quiet", since, "--", area], cwd=root).returncode
                or git(root, "ls-files", "--others", "--exclude-standard", "--", area)):
            changed.add(area)
    return with_dependents(changed)


def with_dependents(areas):
    # The MCP image compiles the API crate and locks its version.
    areas = set(areas) | ({"mcp"} if "api" in areas else set())
    return [area for area in AREAS if area in areas]


def bump_versions(root, areas=AREAS):
    before = versions(root)
    after = dict(before)
    edits = {}
    for area in with_dependents(areas):
        version = before[area]
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
    for area in [area for area in ("api", "mcp") if area in after and after[area] != before[area]]:
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


def activate(release, areas=AREAS, verify_only=False):
    run("scp", "scripts/deploy-smoke.mjs", "pi:/tmp/openhome-deploy-smoke.mjs")
    if not verify_only:
        run("scp", "api/compose.yml", "pi:/tmp/openhome-compose.yml")
        run("scp", "api/compose.bluetooth.yml", "pi:/tmp/openhome-compose.bluetooth.yml")
    arguments = [release[area] for area in AREAS] + ["--areas=" + ",".join(areas)]
    if verify_only:
        arguments.append("--verify-only")
    run("ssh", "pi", shlex.join(["python3", "-", *arguments]), input=(ROOT / "scripts/deploy-pi.py").read_text())
    verify_https()


def main(mode, requested=()):
    if mode == "builder":
        builder()
        return
    release = versions(ROOT)
    areas = list(AREAS)
    pending = ROOT / ".devenv/release.json"
    if mode in ("deploy", "current"):
        if mode == "deploy" and pending.exists():
            raise ValueError("An unfinished release exists; use just deploy-current")
        if mode == "current":
            unfinished = json.loads(pending.read_text()) if pending.exists() else {}
            if unfinished.get("versions") != release:
                raise ValueError("No unfinished release matches the current versions; use just deploy")
            areas = unfinished["areas"]
        else:
            areas = with_dependents(requested) if requested else changed_areas(ROOT)
            if not areas:
                raise ValueError("Nothing changed since the last release; name the areas to force one")
        # A failed release keeps its bumped versions; `deploy-current` resumes them.
        run("ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "pi", "true")
        run("just", "docker-builder")
        if mode == "deploy":
            release = bump_versions(ROOT, areas)
            pending.write_text(json.dumps({"versions": release, "areas": areas}) + "\n")
        print("Release: " + json.dumps({area: release[area] for area in areas}), flush=True)
        run("just", *[f"{area}-check" for area in areas], "deployment-check")
        for area in areas:
            run("just", f"{area}-docker-push", release[area])
        if "dashboard" in areas:
            run("just", "dashboard-docker-smoke", f'patrickhaahr/openhome-dashboard:{release["dashboard"]}')
    activate(release, areas, verify_only=mode == "verify")
    if mode in ("deploy", "current"):
        pending.unlink()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("builder", "deploy", "current", "activate", "verify"))
    parser.add_argument("areas", nargs="*", choices=AREAS,
                        help="deploy only: release these areas instead of the changed ones")
    arguments = parser.parse_args()
    mode = arguments.mode
    lock_dir = ROOT / ".devenv"
    lock_dir.mkdir(exist_ok=True)
    with (lock_dir / "deploy.lock").open("w") as lock:
        try:
            # Builder setup is also called by recipes inside the locked release.
            if mode != "builder":
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            main(mode, arguments.areas)
        except (OSError, ValueError, subprocess.CalledProcessError) as error:
            retry = "deploy-current" if (ROOT / ".devenv/release.json").exists() else "deploy"
            sys.exit(f"Deployment stopped: {error}. Fix the failure before retrying with just {retry}.")
