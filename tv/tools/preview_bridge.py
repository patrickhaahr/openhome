"""Swap the TV's local bridge for a stand-in script and swap the real one back; never installed on the TV."""
import subprocess

from tv_paths import SSH

CLIENT_DIR = "/var/lib/openhome-tv"


def remote(command, **kwargs):
    return subprocess.run(SSH + [command], check=True, **kwargs)


def script_path(name):
    return "/tmp/openhome-tv-%s.js" % name


def upload(name, script, files=()):
    remote("cat > " + script_path(name), input=script)
    for path, content in files:
        remote("cat > " + path, input=content)


def launch(name):
    remote(
        "touch {dir}/client-disabled && {dir}/client-service.sh stop && "
        "{{ setsid node {script} </dev/null >/tmp/openhome-tv-{name}.log 2>&1 & }}".format(
            dir=CLIENT_DIR, script=script_path(name), name=name
        )
    )


def restore(name, files=()):
    leftovers = " ".join([script_path(name), "/tmp/openhome-tv-%s.log" % name] + [path for path, _ in files])
    remote(
        "for p in /proc/[0-9]*/cmdline; do [ -r \"$p\" ] || continue; "
        "command=$(tr '\\000' ' ' < \"$p\" 2>/dev/null) || continue; "
        "case \"$command\" in 'node {script} '*) pid=${{p#/proc/}}; kill \"${{pid%/cmdline}}\";; esac; done; "
        "rm -f {leftovers} {dir}/client-disabled; {dir}/client-service.sh start; /var/lib/pearwebos/pearwebos on".format(
            script=script_path(name), leftovers=leftovers, dir=CLIENT_DIR
        )
    )
