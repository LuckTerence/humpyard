#!/usr/bin/env python3
"""Builds the MkDocs input tree in build/docs from docs/ (including its index.md landing page) and the examples.

Links in the Markdown that leave docs/ (tests, examples, OpenSpec, AGENTS.md ...) cannot resolve
on the site, so they are rewritten to the matching GitHub URL. Run from the repository root.
"""
import posixpath
import re
import shutil
from pathlib import Path

REPO = "https://github.com/gschaetz/humpyard"
SRC, OUT = Path("docs"), Path("build/docs")
LINK = re.compile(r"(\]\()([^)\s#]+)(#[^)\s]*)?(\))")


def rewrite(text: str, rel: str) -> str:
    base = posixpath.dirname(rel)

    def fix(m: re.Match) -> str:
        target = m.group(2)
        if re.match(r"^[a-z]+:", target) or target.startswith("/"):
            return m.group(0)
        resolved = posixpath.normpath(posixpath.join("docs", base, target))
        if resolved.startswith("docs/") or resolved == "docs":
            return m.group(0)
        kind = "tree" if not posixpath.splitext(resolved)[1] else "blob"
        return f"{m.group(1)}{REPO}/{kind}/main/{resolved}{m.group(3) or ''}{m.group(4)}"

    return LINK.sub(fix, text)


def main() -> None:
    if OUT.parent.exists():
        shutil.rmtree(OUT.parent)
    for path in SRC.rglob("*.md"):
        rel = path.relative_to(SRC).as_posix()
        dest = OUT / rel
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text(rewrite(path.read_text(), rel))
    (OUT / "CNAME").write_text("humpyard.dev\n")  # the custom domain, kept with the site
    # The example configs as pages, so they are readable and searchable on the site.
    examples = OUT / "examples.md"
    parts = ["# Example configs\n\nCopy and adapt. Both load and validate as shipped.\n"]
    for name, blurb in (
        ("config.toml", "The general example: providers, targets, routes, keys, budgets, health."),
        ("auto-routing.toml", "One `auto` model whose rules pick the pool by intent. See the [recipe](routing.md#auto-routing-recipe)."),
        ("budgets.toml", "Virtual keys with daily and monthly limits."),
    ):
        src = Path("examples") / name
        if src.exists():
            parts.append(f"\n## {name}\n\n{blurb}\n\n```toml\n{src.read_text()}```\n")
    examples.write_text("".join(parts))
    print(f"prepared {len(list(OUT.rglob('*.md')))} pages in {OUT}")


if __name__ == "__main__":
    main()
