#!/bin/sh
# Regenerate the bundled pure Rust catalog.
set -e
cd "$(dirname "$0")/.."
python3 tools/extract_catalog.py "$@" > crates/zibra-sources/catalog/sources.json
echo "Catalog updated: $(grep -c '"name"' crates/zibra-sources/catalog/sources.json) sources"
