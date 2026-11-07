"""Extract a tagged version's release notes from CHANGELOG.md."""

import argparse
from pathlib import Path
import re


def extract_notes(changelog: str, tag: str) -> str:
    if not tag.startswith("v") or tag == "vUnreleased":
        raise ValueError("expected a version tag with a lowercase v prefix")
    version = tag[1:]
    sections = list(re.finditer(r"^## \[([^\]\n]+)\][^\n]*$", changelog, re.MULTILINE))
    matches = [index for index, section in enumerate(sections) if section[1] == version]
    if len(matches) != 1:
        raise ValueError(f"expected exactly one CHANGELOG.md entry for {version}")
    index = matches[0]
    end = sections[index + 1].start() if index + 1 < len(sections) else len(changelog)
    notes = changelog[sections[index].end():end].strip()
    # Comments and subsection headings alone are not release content.
    content = re.sub(r"<!--.*?-->", "", notes, flags=re.DOTALL)
    content = re.sub(r"^#{1,6}\s+.*$", "", content, flags=re.MULTILINE)
    if not content.strip():
        raise ValueError(f"CHANGELOG.md entry for {version} is empty")
    return notes + "\n"


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tag")
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    try:
        notes = extract_notes(Path("CHANGELOG.md").read_text(encoding="utf-8"), args.tag)
    except ValueError as error:
        parser.exit(1, f"error: {error}\n")
    args.output.write_text(notes, encoding="utf-8")
