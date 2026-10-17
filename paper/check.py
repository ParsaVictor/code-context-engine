"""Static checks for the paper source, for machines without a TeX installation.

Checks: balanced braces and \\begin/\\end pairs per file, every \\input file exists, every
\\cite key is in refs.bib, every \\ref has a \\label, no duplicate labels or bib keys, bib entries
brace-balanced, no non-ASCII characters in .tex files. Lists every \\todo. Exit code 1 on any error.

    python paper/check.py            # from the repository root or from paper/
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent


def strip_comments(text):
    # A % starts a comment unless escaped as \%.
    return "\n".join(re.sub(r"(?<!\\)%.*", "", line) for line in text.splitlines())


def check_tex(path, errors):
    raw = path.read_text(encoding="utf-8")
    for n, line in enumerate(raw.splitlines(), 1):
        for ch in line:
            if ord(ch) > 127:
                errors.append(f"{path.name}:{n}: non-ASCII character {ch!r}")
                break
    text = strip_comments(raw)
    body = re.sub(r"\\[{}]", "", text)  # escaped braces
    depth = 0
    for n, line in enumerate(body.splitlines(), 1):
        for ch in line:
            depth += ch == "{"
            depth -= ch == "}"
            if depth < 0:
                errors.append(f"{path.name}:{n}: unmatched '}}'")
                depth = 0
    if depth:
        errors.append(f"{path.name}: {depth} unclosed '{{'")
    stack = []
    for m in re.finditer(r"\\(begin|end)\{([^}]+)\}", text):
        line = text.count("\n", 0, m.start()) + 1
        if m.group(1) == "begin":
            stack.append((m.group(2), line))
        elif not stack or stack[-1][0] != m.group(2):
            errors.append(f"{path.name}:{line}: \\end{{{m.group(2)}}} does not close "
                          f"{stack[-1] if stack else 'anything'}")
        else:
            stack.pop()
    for env, line in stack:
        errors.append(f"{path.name}:{line}: \\begin{{{env}}} never closed")
    return text


def bib_keys(path, errors):
    raw = path.read_text(encoding="utf-8")
    keys = []
    for m in re.finditer(r"^@(\w+)\{([^,\s]+),", raw, re.M):
        keys.append(m.group(2))
        start, depth, i = m.end(), 1, m.end()
        while i < len(raw) and depth:
            depth += raw[i] == "{"
            depth -= raw[i] == "}"
            i += 1
        if depth:
            errors.append(f"refs.bib: entry {m.group(2)} not brace-balanced")
    seen = set()
    for k in keys:
        if k in seen:
            errors.append(f"refs.bib: duplicate key {k}")
        seen.add(k)
    return seen


def main():
    errors = []
    texts = {}
    main_tex = ROOT / "main.tex"
    queue = [main_tex]
    while queue:
        path = queue.pop()
        if path in texts:
            continue
        if not path.exists():
            errors.append(f"missing input file {path.relative_to(ROOT)}")
            continue
        texts[path] = check_tex(path, errors)
        for m in re.finditer(r"\\input\{([^}]+)\}", texts[path]):
            name = m.group(1)
            queue.append(ROOT / (name if name.endswith(".tex") else name + ".tex"))
    all_text = "\n".join(texts.values())

    keys = bib_keys(ROOT / "refs.bib", errors)
    cited = set()
    for m in re.finditer(r"\\cite\w*\*?(?:\[[^\]]*\])*\{([^}]+)\}", all_text):
        cited.update(k.strip() for k in m.group(1).split(","))
    for k in sorted(cited - keys):
        errors.append(f"cited but not in refs.bib: {k}")
    unused = sorted(keys - cited)

    labels = re.findall(r"\\label\{([^}]+)\}", all_text)
    for lab in {l for l in labels if labels.count(l) > 1}:
        errors.append(f"duplicate label {lab}")
    for ref in sorted(set(re.findall(r"\\ref\{([^}]+)\}", all_text)) - set(labels)):
        errors.append(f"\\ref to undefined label {ref}")

    todos = []
    for path, text in texts.items():
        for n, line in enumerate(text.splitlines(), 1):
            for m in re.finditer(r"\\todo\{([^}]*)", line):
                todos.append(f"{path.name}:{n}: {m.group(1)}")

    print(f"files: {', '.join(sorted(p.name for p in texts))}")
    print(f"citations: {len(cited)} keys used, {len(keys)} in refs.bib"
          + (f" (unused: {', '.join(unused)})" if unused else ""))
    print(f"labels: {len(labels)}")
    print(f"TODOs ({len(todos)}):")
    for t in todos:
        print("  " + t)
    if errors:
        print(f"ERRORS ({len(errors)}):")
        for e in errors:
            print("  " + e)
        return 1
    print("OK: no syntax errors found")
    return 0


if __name__ == "__main__":
    sys.exit(main())
