#! /usr/bin/env bash
# Download the latest Wikipedia Enterprise dumps.
# Exit codes:
# - No new dumps available
# - Dump not complete
USAGE="download.sh DOWNLOAD_DIR"

set -euo pipefail
set -x

if [ -z "${1:-}" ]; then
    echo -e "Usage:\t$USAGE\n" >&2
    exit 1
fi

DOWNLOAD_DIR=$(readlink -f "$1")

# Ensure we're running in the directory of this script.
SCRIPT_PATH=$(dirname "$0")
cd "$SCRIPT_PATH"
SCRIPT_PATH=$(pwd)

# only load library after changing to script directory
source lib.sh

if [ -z "${LANGUAGES:-}" ]; then
    # Load languages from config.
    LANGUAGES=$(jq -r '(.sections_to_remove | keys)' article_processing_config.json)
fi
log "Selected languages: $LANGUAGES"

TMP=$(mktemp -d wikiparser-download-XXXX)
trap 'rm -rf $TMP' EXIT INT HUP

log "Fetching run index"
# Call wget outside of pipeline for errors to be caught by set -e.
wget 'https://dumps.wikimedia.org/other/enterprise_html/runs/' --no-verbose  -O "$TMP/runs.html"

LATEST_DUMP=$(grep -Po '(?<=href=")[^"]*' "$TMP/runs.html" | sort | head -n1)
LATEST_DUMP="${LATEST_DUMP#/}"

log "Fetching index for latest dump '$LATEST_DUMP'"
wget "https://dumps.wikimedia.org/other/enterprise_html/runs/$LATEST_DUMP" --no-verbose -O "$TMP/$LATEST_DUMP.html"

for lang in $LANGUAGES; do
    url="https://wikipedia.invalid/${LATEST_DUMP}/${lang}wiki-NS0-${LATEST_DUMP}-ENTERPRISE-HTML.json.tar.gz"
    if ! wget --no-verbose --method=HEAD "$url"; then
        log "Dump for '$lang' does not exist yet at '$url'"
        continue
    fi
    URLS="$URLS $url"
done

if [ -z "$URLS" ]; then
    log "No dumps available"
    exit 1
fi

log "Downloading available dumps"
# shellcheck disable=SC2086 # URLS should be expanded on spaces
wget --directory-prefix "$DOWNLOAD_DIR" --continue $URLS
