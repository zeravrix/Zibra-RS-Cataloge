#!/usr/bin/env python3
"""Generate pure Rust catalog definitions for crates/zibra-sources/catalog/sources.json.

Usage: python3 tools/extract_catalog.py <parsers-dir> > crates/zibra-sources/catalog/sources.json
"""
import json
import re
import sys
from pathlib import Path

ANNOTATION = re.compile(
    r'@MangaSourceParser\(\s*"(\w+)"\s*,\s*"([^"]+)"\s*,\s*"([^"]*)"\s*(?:,\s*ContentType\.(\w+))?\s*\)')
SUPER_CALL = re.compile(r'(\w+Parser)\(\s*context\s*,\s*MangaParserSource\.(\w+)\s*,\s*"([^"]+)"')
OVERRIDE = re.compile(r'override\s+(?:suspend\s+)?(?:val|var|fun)\s+(\w+)')
LIST_URL = re.compile(r'override\s+val\s+listUrl\s*=\s*"([^"]*)"')
PRIVATE_MEMBER = re.compile(r'^\s*private\s+(?:val|var|fun)\s', re.M)
ADULT_WORDS = re.compile(r'hentai|porn|xxx|adult|erotica|18\+', re.I)

HARMLESS = {"listUrl", "tagPrefix", "datePattern", "sourceLocale"}
FAMILIES = {"madara", "mangareader"}


def classify(text):
    """Return (record, None) for a usable site, else (None, reason)."""
    ann = ANNOTATION.search(text)
    call = SUPER_CALL.search(text)
    if not ann or not call or ann.group(1) != call.group(2):
        return None, "unparsed"
    name, title, lang, ctype = ann.groups()
    if "@Broken" in text:
        return None, "broken"
    if ctype == "HENTAI" or "isNsfwSource" in text or ADULT_WORDS.search(title):
        return None, "adult"
    family = call.group(1)[: -len("Parser")].lower()
    if family not in FAMILIES:
        return None, "family"
    if set(OVERRIDE.findall(text)) - HARMLESS or PRIVATE_MEMBER.search(text):
        return None, "custom"
    rec = {"name": name, "title": title, "lang": lang, "domain": call.group(3), "family": family}
    m = LIST_URL.search(text)
    if m and m.group(1).strip("/") not in ("", "manga"):
        rec["list_path"] = m.group(1).strip("/")
    return rec, None


def extract(root):
    records, skipped, seen = [], {}, set()
    for f in sorted(Path(root).rglob("*.kt")):
        text = f.read_text(encoding="utf-8", errors="replace")
        if "@MangaSourceParser" not in text:
            continue
        rec, why = classify(text)
        if rec and rec["domain"] in seen:
            rec, why = None, "duplicate-domain"
        if rec:
            seen.add(rec["domain"])
            records.append(rec)
        else:
            skipped[why] = skipped.get(why, 0) + 1
    return records, skipped


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    records, skipped = extract(sys.argv[1])
    print(f"kept {len(records)}, skipped {skipped}", file=sys.stderr)
    json.dump(records, sys.stdout, indent=1, ensure_ascii=False)
    print()


if __name__ == "__main__":
    main()
