#! /usr/bin/env sh
# Convenience script to run the wikiparser with the maps generator as a drop-in replacement for the scraper.
# For more information on the map generator, see
# <https://github.com/organicmaps/organicmaps/blob/b52b42bd746fdb8daf05cc048f0b22654cfb9b8e/tools/python/maps_generator/README.md>.

USAGE="./run.sh DUMP_FILE.json.tar.gz [DUMP_FILE.json.tar.gz...]"

set -eu
# set -x

# Write printf-style message to stderr with a timestamp and line ending.
log () {
    printf "%s " "$(date '+%Y-%m-%dT%H:%M:%SZ')" >&2
    # shellcheck disable=2059
    printf "$@" >&2 # Forward all arguments to printf.
    printf "\n" >&2
}

if [ -z "${1}" ]
then
    printf "Usage:\t%s\n" "$USAGE" >&2
    exit 1
fi

# Ensure we're running in the directory of this script.
SCRIPT_PATH=$(dirname "$0")
cd "$SCRIPT_PATH"
SCRIPT_PATH=$(pwd)

# Get latest maps build folder

if [ -z "${MAPS_DIR+}" ]
then
    : "${MAPS_BUILD_ROOT:=$HOME/maps_build}"

    # Use latest date-format folder.
    # Folder format is only numbers and underscores, so ls | grep is fine.
    # shellcheck disable=2010
    MAPS_DIR="$MAPS_BUILD_ROOT"/$(ls "$MAPS_BUILD_ROOT" | grep -E '^[0-9_]+$' | tail -n1)
    if [ -z "$MAPS_DIR" ]
    then
        printf "No map build found in '%s'\n" "$MAPS_BUILD_ROOT" >&2
        exit 1
    fi
fi

log "Using maps build directory '%s'" "$MAPS_DIR"

if ! command -v "cargo" > /dev/null
then
    printf "'cargo' is not installed, cannot build wikiparser.\nSee <https://www.rust-lang.org/>.\n" >&2
    exit 1
fi

log "Building wikiparser"
cargo build --release
wikiparser=$(pwd)/target/release/om-wikiparser

log "Changing to maps build dir '%s'" "$MAPS_DIR"
cd "$MAPS_DIR"/intermediate_data

log "Transforming intermediate data"
cut -f 2 id_to_wikidata.csv > wikidata_ids.txt
tail -n +2 wiki_urls.txt | cut -f 3 > wikipedia_urls.txt

# Enable backtraces in errors and panics.
export RUST_BACKTRACE=1
# Set log level.
export RUST_LOG=om_wikiparser=info

# Begin extraction.
OUTPUT_DIR=$(pwd)/descriptions
if [ ! -e "$OUTPUT_DIR" ]
then
    mkdir "$OUTPUT_DIR"
fi
log "Extracting articles to '%s'" "$OUTPUT_DIR"

for dump in "$@"
do
  log "Extracting '%s'" "$dump"
  tar xzf "$dump" | $wikiparser \
    --wikidata-ids wikidata_ids.txt \
    --wikipedia-urls wikipedia_urls.txt \
    --write-new-ids new_qids.txt \
    "$OUTPUT_DIR"
done

log "Beginning extraction of discovered QIDs"

# Extract new qids from other dumps
for dump in "$@"
do
  log "Extracting '%s'\n" "$dump"
  tar xzf "$dump" | $wikiparser \
    --wikidata-ids new_qids.txt \
    "$OUTPUT_DIR"
done

log "Finished"
