Import("env")

from pathlib import Path


def load_dotenv(path):
    values = {}
    if not path.is_file():
        return values

    for line in path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue

        key, value = line.split("=", 1)
        value = value.strip()
        if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
            value = value[1:-1]
        values[key.strip()] = value

    return values


credentials = load_dotenv(Path(env.subst("$PROJECT_DIR")) / ".env")
for name in ("WIFI_SSID", "WIFI_PASSWORD"):
    if not credentials.get(name):
        raise RuntimeError(f"Missing required {name} in .env")
    env.Append(CPPDEFINES=[(name, env.StringifyMacro(credentials[name]))])
