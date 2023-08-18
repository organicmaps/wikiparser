#! /usr/bin/env bash
USAGE="Usage: ./download.sh [-hD] <DUMP_DIR>

Download the latest Wikipedia Enterprise HTML dumps.

Arguments:
    <DUMP_DIR>  An existing directory to store dumps in. Dumps will be grouped
                into subdirectories by date, and a link 'latest' will point to
                the latest complete dump subdirectory, if it exists.

Options:
    -h      Print this help screen
    -D      Delete all old dump subdirectories if the latest is downloaded

Environment Variables:
    LANGUAGES   A whitespace-separated list of wikipedia language codes to
                download dumps of.
                Defaults to the languages in 'article_processing_config.json'.
                See <https://meta.wikimedia.org/wiki/List_of_Wikipedias>.

Exit codes:
    0   The lastest dumps are already present or were downloaded successfully.
    1   Argument error.
    16  Some of languages were not available to download. The latest dump may
        be in progress, or some of the specified languages may not exist.
    _   Subprocess error.
"

set -euo pipefail
# set -x

# Parse options.
DELETE_OLD_DUMPS=false
while getopts "hD" opt
do
    case $opt in
    h)  echo -n "$USAGE"; exit 0;;
    D)  DELETE_OLD_DUMPS=true;;
    ?)  echo "$USAGE" | head -n1 >&2; exit 1;;
    esac
done
shift $((OPTIND - 1))

if [ -z "${1:-}" ]; then
    echo "DUMP_DIR is required" >&2
    echo -n "$USAGE" >&2
    exit 1
fi

# The parent directory to store groups of dumps in.
DUMP_DIR=$(readlink -f "$1")
shift

if [ -n "${1:-}" ]; then
    echo "Unexpected extra argument: '$1'" >&2
    echo "$USAGE" | head -n1 >&2
    exit 1
fi

if [ ! -d "$DUMP_DIR" ]; then
    echo "DUMP_DIR '$DUMP_DIR' does not exist" >&2
    exit 1
fi

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

log "Fetching run index"
# The date of the latest dump, YYYYMMDD.
LATEST_DUMP=$(wget 'https://dumps.wikimedia.org/other/enterprise_html/runs/' --no-verbose -O - \
            | grep -Po '(?<=href=")[^"]*' | grep -P '\d{8}' | sort -r | head -n1)
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
    exit 16
fi

# The subdir to store the latest dump in.
DOWNLOAD_DIR="$DUMP_DIR/$LATEST_DUMP"
if [ ! -e "$DOWNLOAD_DIR" ]; then
    mkdir "$DOWNLOAD_DIR"
fi

log "Downloading available dumps"
if type wget2 > /dev/null; then
    # NOTE: Wikipedia requests no more than 2 concurrent downloads.
    # See https://dumps.wikimedia.org/ for more info.

    # shellcheck disable=SC2086 # URLS should be expanded on spaces.
    wget2 --max-threads 2 --verbose --progress=bar --directory-prefix "$DOWNLOAD_DIR" --continue $URLS
else
    log "WARN: wget2 is not available, falling back to sequential downloads"
    # shellcheck disable=SC2086 # URLS should be expanded on spaces.
    wget --directory-prefix "$DOWNLOAD_DIR" --continue $URLS
fi

if [ $MISSING_DUMPS -gt 0 ]; then
    log "$MISSING_DUMPS dumps not available yet"
    exit 16
fi

log "Linking 'latest' to '$LATEST_DUMP'"
LATEST_LINK="$DUMP_DIR/latest"
ln -sf -T "$LATEST_DUMP" "$LATEST_LINK"

if [ "$DELETE_OLD_DUMPS" = true ]; then
    # shellcheck disable=SC2010 # Only matching files with numeric names are used.
    mapfile -t OLD_DUMPS < <(ls "$DUMP_DIR" | grep -P '^\d{8}$' | grep -vF "$LATEST_DUMP")
    if [ "${#OLD_DUMPS[@]}" -gt 0 ]; then
        log "Deleting old dumps" "${OLD_DUMPS[@]}"
        for old_dump in "${OLD_DUMPS[@]}"; do
            rm -r "${DUMP_DIR:?}/${old_dump:?}/"
        done
    else
        log "No old dumps to delete"
    fi
fi
