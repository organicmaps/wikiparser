#! /usr/bin/env bash
# Download the latest Wikipedia Enterprise dumps.
# Exit codes:
# - 0: The lastest dumps are already present or were downloaded successfully.
# - No new dumps available
# - Dump not complete
USAGE="download.sh DOWNLOAD_DIR"

set -euo pipefail
# set -x

if [ -z "${1:-}" ]; then
    echo -e "Usage:\t$USAGE\n" >&2
    exit 1
fi

DOWNLOAD_DIR=$(readlink -f "$1")

# Ensure we're running in the directory of this script.
SCRIPT_PATH=$(dirname "$0")
cd "$SCRIPT_PATH"
SCRIPT_PATH=$(pwd)

# Only load library after changing to script directory.
source lib.sh

if [ -z "${LANGUAGES:-}" ]; then
    # Load languages from config.
    LANGUAGES=$(jq -r '(.sections_to_remove | keys | .[])' article_processing_config.json)
fi
# shellcheck disable=SC2086 # LANGUAGES is intentionally expanded.
log "Selected languages:" $LANGUAGES

TMP=$(mktemp --tmpdir -d wikiparser-download-XXXX)
trap 'rm -rf $TMP' EXIT INT HUP

log "Fetching run index"
# Call wget outside of pipeline for errors to be caught by set -e.
wget 'https://dumps.wikimedia.org/other/enterprise_html/runs/' --no-verbose  -O "$TMP/runs.html"

LATEST_DUMP=$(grep -Po '(?<=href=")[^"]*' "$TMP/runs.html" | grep -P '\d{8}' | sort -r | head -n1)
LATEST_DUMP="${LATEST_DUMP%/}"

log "Checking latest dump $LATEST_DUMP"

URLS=
MISSING_DUMPS=0
for lang in $LANGUAGES; do
    url="https://dumps.wikimedia.org/other/enterprise_html/runs/${LATEST_DUMP}/${lang}wiki-NS0-${LATEST_DUMP}-ENTERPRISE-HTML.json.tar.gz"
    if ! wget --no-verbose --method=HEAD "$url"; then
        MISSING_DUMPS=$(( MISSING_DUMPS + 1 ))
        log "Dump for '$lang' does not exist at '$url'"
        continue
    fi
    URLS="$URLS $url"
done

if [ -z "$URLS" ]; then
    log "No dumps available"
    exit 1
fi

log "Downloading available dumps"
# shellcheck disable=SC2086 # URLS should be expanded on spaces.
wget --directory-prefix "$DOWNLOAD_DIR" --continue $URLS

if [ $MISSING_DUMPS -gt 0 ]; then
    log "$MISSING_DUMPS dumps not available yet"
    exit 1
fi
