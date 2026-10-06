#!/usr/bin/env python3
"""Validates packaging/winget against Microsoft's published manifest schemas.

    python3 scripts/validate-winget.py

`winget validate` is a Windows program; this is the check that can run anywhere, and it runs
before a release rather than waiting for the Windows workflow to say a checksum is the wrong
length. It needs `pyyaml` and `jsonschema`, and it fetches the three schemas (the manifests
declare which version they follow in their first line).
"""
import datetime
import json
import pathlib
import re
import sys
import urllib.request

try:
    import jsonschema
    import yaml
except ModuleNotFoundError as e:
    sys.exit(f"{e.name} is needed: pip3 install --user pyyaml jsonschema")

ROOT = pathlib.Path(__file__).resolve().parent.parent / "packaging" / "winget"

# manifest file -> (the schema's kind, as it appears in its URL)
KINDS = {
    "PruneContributors.Prune.yaml": "version",
    "PruneContributors.Prune.installer.yaml": "installer",
    "PruneContributors.Prune.locale.en-US.yaml": "defaultLocale",
}


def plain(x):
    """YAML turns an unquoted date into a date object; the schema wants the text."""
    if isinstance(x, (datetime.date, datetime.datetime)):
        return x.isoformat()
    if isinstance(x, dict):
        return {k: plain(v) for k, v in x.items()}
    if isinstance(x, list):
        return [plain(v) for v in x]
    return x


def schema_for(kind: str, text: str) -> dict:
    version = re.search(r"manifest\.\w+\.(\d+\.\d+\.\d+)\.schema", text)
    if not version:
        sys.exit(f"no schema header on the first line of the {kind} manifest")
    url = f"https://aka.ms/winget-manifest.{kind}.{version.group(1)}.schema.json"
    with urllib.request.urlopen(url, timeout=30) as r:
        return json.load(r)


failed = 0
for name, kind in KINDS.items():
    text = (ROOT / name).read_text()
    errors = sorted(
        jsonschema.Draft7Validator(schema_for(kind, text)).iter_errors(plain(yaml.safe_load(text))),
        key=lambda e: list(e.path),
    )
    print(("FAIL " if errors else "ok   ") + name)
    for e in errors:
        print(f"     {list(e.path)}: {e.message}")
    failed += bool(errors)
sys.exit(1 if failed else 0)
