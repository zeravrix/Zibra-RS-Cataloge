#!/bin/sh
# One command: fetch the Kotatsu parsers and regenerate the bundled catalog.
set -e
cd "$(dirname "$0")/.."
tmp=$(mktemp -d)
git clone --depth 1 https://github.com/KotatsuApp/kotatsu-parsers "$tmp/kp"
python3 tools/extract_kotatsu.py "$tmp/kp/src/main/kotlin" > crates/zibra-sources/catalog/sources.json
echo "catalog updated: $(grep -c '"name"' crates/zibra-sources/catalog/sources.json) sources"
