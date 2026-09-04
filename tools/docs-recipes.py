#!/usr/bin/env python3
"""Regenerate the canonical runnable-recipe sections in user documentation."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys


START = b"<!-- BEGIN GENERATED OOXML RECIPES -->"
END = b"<!-- END GENERATED OOXML RECIPES -->"
DOCUMENTS = (Path("README.md"), Path("skills/ooxml/SKILL.md"))


def normalized_lf(data: bytes) -> bytes:
    return data.replace(b"\r\n", b"\n").replace(b"\r", b"\n")


def generated_catalog(binary: Path) -> bytes:
    completed = subprocess.run(
        [str(binary), "robot-docs", "recipes", "--format", "text"],
        check=True,
        stdout=subprocess.PIPE,
    )
    catalog = normalized_lf(completed.stdout)
    if not catalog.endswith(b"\n"):
        catalog += b"\n"
    return catalog


def replace_section(document: bytes, catalog: bytes, path: Path) -> bytes:
    document = normalized_lf(document)
    if document.count(START) != 1 or document.count(END) != 1:
        raise ValueError(f"{path}: expected exactly one generated recipe marker pair")
    start = document.index(START)
    end = document.index(END, start)
    if end < start:
        raise ValueError(f"{path}: generated recipe end marker precedes start marker")
    replacement = START + b"\n" + catalog + END
    return document[:start] + replacement + document[end + len(END) :]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check",
        action="store_true",
        help="report drift without rewriting documents",
    )
    args = parser.parse_args()

    repo = Path(__file__).resolve().parent.parent
    configured = os.environ.get("OOXML_BIN")
    if not configured:
        parser.error("OOXML_BIN must point to the freshly built ooxml executable")
    binary = Path(configured).resolve()
    catalog = generated_catalog(binary)

    drifted: list[Path] = []
    for relative in DOCUMENTS:
        path = repo / relative
        current = path.read_bytes()
        expected = replace_section(current, catalog, relative)
        if current != expected:
            drifted.append(relative)
            if not args.check:
                path.write_bytes(expected)

    if args.check and drifted:
        print(
            "generated recipe docs are stale: "
            + ", ".join(str(path) for path in drifted),
            file=sys.stderr,
        )
        print("run: make docs-recipes", file=sys.stderr)
        return 1
    if not args.check:
        for path in DOCUMENTS:
            print(f"updated {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
