#!/usr/bin/env python3
import json
import re
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BASELINE = ROOT / "scripts" / "comment-baseline.json"

EXCLUDED = ("/vendor/", "dashboard/.intent/")
C_LIKE = {".rs", ".ts", ".tsx", ".js", ".mjs", ".cjs", ".kt", ".kts", ".css", ".qml", ".cpp", ".c", ".h", ".java"}
HASH = {".py", ".sh"}
SQL = {".sql"}

DIRECTIVE = re.compile(
    r"^\s*(?:"
    r"(?:eslint|oxlint|biome|prettier|stylelint|ktlint|shellcheck|istanbul|c8|v8|jshint|tslint)[- ]|"
    r"@ts-(?:expect-error|ignore|nocheck|check)|"
    r"@vite-ignore|@jsxImportSource|@refresh|"
    r"/ ?<reference |"
    r"noqa|type: ?ignore|pyright:|mypy:|fmt: ?(?:off|on|skip)|pylint:|"
    r"NOLINT|clang-format |"
    r"SAFETY:|"
    r"language=|region\b|endregion\b|"
    r"-\*-.*-\*-"
    r")"
)


@dataclass
class Comment:
    path: str
    line: int
    end_line: int
    kind: str
    own_lines: bool
    text: str


def _scan_c_like(src: str, ext: str):
    i, n, line = 0, len(src), 1
    rust = ext == ".rs"
    templ = ext in {".ts", ".tsx", ".js", ".mjs", ".cjs"}
    kotlin = ext in {".kt", ".kts"}
    brace_stack: list[str] = []
    while i < n:
        c = src[i]
        if c == "\n":
            line += 1
            i += 1
            continue
        if c == "/" and i + 1 < n and src[i + 1] == "/":
            j = src.find("\n", i)
            j = n if j == -1 else j
            yield ("line", i, j, line, line)
            i = j
            continue
        if c == "/" and i + 1 < n and src[i + 1] == "*":
            depth, j, start_line = 1, i + 2, line
            while j < n and depth:
                if rust and src.startswith("/*", j):
                    depth += 1
                    j += 2
                elif src.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    if src[j] == "\n":
                        line += 1
                    j += 1
            yield ("block", i, j, start_line, line)
            i = j
            continue
        if rust and c == "r" and re.match(r'r#*"', src[i : i + 260]) and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_")):
            m = re.match(r'r(#*)"', src[i:])
            close = '"' + m.group(1)
            j = src.find(close, i + len(m.group(0)))
            j = n if j == -1 else j + len(close)
            line += src.count("\n", i, j)
            i = j
            continue
        if rust and c == "'":
            m = re.match(r"'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F]+\}|.)|[^\\'\n])'", src[i:])
            if m:
                i += len(m.group(0))
            else:
                i += 1
            continue
        if kotlin and src.startswith('"""', i):
            j = src.find('"""', i + 3)
            j = n if j == -1 else j + 3
            while j < n and src[j] == '"':
                j += 1
            line += src.count("\n", i, j)
            i = j
            continue
        if c == "`" and templ:
            i += 1
            while i < n and src[i] != "`":
                if src[i] == "\\":
                    i += 2
                    continue
                if src.startswith("${", i):
                    brace_stack.append("`")
                    i += 2
                    break
                if src[i] == "\n":
                    line += 1
                i += 1
            else:
                i += 1
            continue
        if templ and c == "}" and brace_stack and brace_stack[-1] == "`":
            brace_stack.pop()
            i += 1
            while i < n and src[i] != "`":
                if src[i] == "\\":
                    i += 2
                    continue
                if src.startswith("${", i):
                    brace_stack.append("`")
                    i += 2
                    break
                if src[i] == "\n":
                    line += 1
                i += 1
            else:
                i += 1
            continue
        if templ and c == "{" and brace_stack:
            brace_stack.append("{")
            i += 1
            continue
        if templ and c == "}" and brace_stack and brace_stack[-1] == "{":
            brace_stack.pop()
            i += 1
            continue
        if c == '"' or (c == "'" and not rust and ext != ".kt"):
            q, j = c, i + 1
            while j < n and src[j] != q:
                if src[j] == "\\":
                    j += 1
                elif src[j] == "\n" and not rust:
                    break
                j += 1
            line += src.count("\n", i, j)
            i = j + 1
            continue
        if c == "'" and ext in {".kt", ".kts"}:
            m = re.match(r"'(?:\\.|[^\\'\n])+'", src[i:])
            i += len(m.group(0)) if m else 1
            continue
        if templ and c == "/" and _regex_allowed(src, i):
            j = i + 1
            in_class = False
            while j < n and src[j] != "\n":
                if src[j] == "\\":
                    j += 2
                    continue
                if src[j] == "[":
                    in_class = True
                elif src[j] == "]":
                    in_class = False
                elif src[j] == "/" and not in_class:
                    break
                j += 1
            i = j + 1
            continue
        i += 1


def _regex_allowed(src: str, i: int) -> bool:
    k = i - 1
    while k >= 0 and src[k] in " \t":
        k -= 1
    if k < 0:
        return True
    prev = src[k]
    if prev in "(,=:[!&|?{};+-*%<>~^\n":
        return True
    m = re.search(r"(?:return|typeof|case|in|of|void|throw|yield|await)$", src[max(0, k - 10) : k + 1])
    return bool(m)


def _scan_prefix(src: str, prefix: str, ext: str):
    line = 0
    offset = 0
    in_triple = None
    for s in src.split("\n"):
        line += 1
        start, offset = offset, offset + len(s) + 1
        if ext == ".py":
            if in_triple:
                if in_triple in s:
                    in_triple = None
                continue
            stripped = s.lstrip()
            for q in ('"""', "'''"):
                if stripped.startswith(q) or re.search(r"=\s*[rbfu]?" + q, s):
                    if s.count(q) == 1:
                        in_triple = q
                    break
            if in_triple:
                continue
        idx = _find_prefix(s, prefix, ext)
        if idx is None:
            continue
        yield ("line", start + idx, start + len(s), line, line)


def _find_prefix(s: str, prefix: str, ext: str):
    q = None
    i = 0
    while i < len(s):
        ch = s[i]
        if q:
            if ch == "\\" and ext != ".sql":
                i += 2
                continue
            if ch == q:
                q = None
        elif ch in ("'", '"') and not (ext == ".sh" and ch == "'" and i > 0 and s[i - 1].isalnum()):
            q = ch
        elif s.startswith(prefix, i):
            if prefix == "#" and ext == ".sh" and i > 0 and not s[i - 1].isspace():
                i += 1
                continue
            if prefix == "#" and ext == ".sh" and s.startswith("#!", i) and i == 0:
                return None
            return i
        i += 1
    return None


def scan_source(src: str, ext: str, name: str = "") -> list[Comment]:
    if ext in C_LIKE:
        spans = _scan_c_like(src, ext)
    elif ext in HASH:
        spans = _scan_prefix(src, "#", ext)
    elif ext in SQL:
        spans = _scan_prefix(src, "--", ext)
    else:
        return []
    out = []
    for kind, a, b, l1, l2 in spans:
        text = src[a:b]
        body = re.sub(r"^(?://[/!]?|/\*+!?|#|--)", "", text).rstrip("*/").strip()
        if DIRECTIVE.match(body) or text.startswith("#!"):
            continue
        if (ext == ".rs" and text.startswith(("///", "//!"))) or (text.startswith("/**") and ext != ".css"):
            kind = "doc"
        line_end = src.find("\n", b)
        own = src[src.rfind("\n", 0, a) + 1 : a].strip() == "" and src[b : len(src) if line_end == -1 else line_end].strip() == ""
        out.append(Comment(name, l1, l2, kind, own, text))
    return out


def scan(path: Path) -> list[Comment]:
    try:
        src = path.read_text()
    except (UnicodeDecodeError, FileNotFoundError):
        return []
    return scan_source(src, path.suffix, str(path.relative_to(ROOT)))


def strip_source(src: str, ext: str, lines: set[int]) -> str:
    out = src.split("\n")
    drop: set[int] = set()
    for c in scan_source(src, ext):
        if c.line not in lines:
            continue
        if c.own_lines:
            drop.update(range(c.line - 1, c.end_line))
        else:
            row = out[c.line - 1]
            at = row.rfind(c.text)
            if at >= 0:
                out[c.line - 1] = row[:at].rstrip()
    tidy: list[str] = []
    removed_here = False
    for i, row in enumerate(out):
        if i in drop:
            removed_here = True
            continue
        if removed_here and row.strip() == "" and (not tidy or tidy[-1].strip() == ""):
            continue
        removed_here = False
        tidy.append(row)
    return "\n".join(tidy).lstrip("\n")


def strip(locations: list[str]) -> None:
    by_file: dict[str, set[int]] = {}
    for location in locations:
        path, line = location.rsplit(":", 1)
        by_file.setdefault(path, set()).add(int(line))
    for path, lines in by_file.items():
        file = ROOT / path
        file.write_text(strip_source(file.read_text(), file.suffix, lines))


@dataclass
class Verdict:
    grown: dict[str, tuple[int, int]]
    shrunk: list[str]


def verdict(baseline: dict[str, int], current: dict[str, int]) -> Verdict:
    grown = {p: (k, baseline.get(p, 0)) for p, k in sorted(current.items()) if k > baseline.get(p, 0)}
    shrunk = sorted(p for p, k in baseline.items() if current.get(p, 0) < k)
    return Verdict(grown, shrunk)


def tracked_files(paths: list[str]) -> list[Path]:
    exts = C_LIKE | HASH | SQL
    out = subprocess.run(["git", "ls-files", "--cached", "--others", "--exclude-standard", "--", *paths], cwd=ROOT, capture_output=True, text=True, check=True).stdout
    return [
        ROOT / p
        for p in out.splitlines()
        if Path(p).suffix in exts and not any(x in p for x in EXCLUDED) and (ROOT / p).is_file()
    ]


def counts(files: list[Path]) -> dict[str, int]:
    result = {}
    for f in files:
        k = len(scan(f))
        if k:
            result[str(f.relative_to(ROOT))] = k
    return result


def main(argv: list[str]) -> int:
    cmd = argv[1] if len(argv) > 1 else "check"
    rest = argv[2:]
    if cmd == "list":
        for f in tracked_files(rest):
            for c in scan(f):
                print(json.dumps(asdict(c)))
        return 0
    if cmd == "strip":
        strip(rest)
        return 0
    if cmd == "update":
        BASELINE.write_text(json.dumps(counts(tracked_files([])), indent=1, sort_keys=True) + "\n")
        return 0
    if cmd == "check":
        result = verdict(json.loads(BASELINE.read_text()), counts(tracked_files([])))
        for path, (k, allowed) in result.grown.items():
            print(f"{path}: {k} comments, baseline allows {allowed}")
        if result.grown:
            print("Comments are banned (docs/adr/0018-no-code-comments.md): put the why in names, types or tests.")
            print("Record decisions in docs/adr/ and hacks as GitHub issues; see .agents/skills/delete-comment/SKILL.md.")
            return 1
        if result.shrunk:
            print(f"{len(result.shrunk)} files hold fewer comments than the baseline; run `just comment-baseline`.")
            return 1
        return 0
    print(f"usage: {argv[0]} [check|update|list [paths...]|strip path:line...]", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
