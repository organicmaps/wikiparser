#! /usr/bin/env bash
# shellcheck disable=SC2016 # Backticks not used as expansions in documentation.
USAGE='Usage: ./run.sh [-h] <BUILD_DIR> <OSM_FILE.osm.pbf> <DUMP_FILE.json.tar.gz> [<DUMP_FILE.json.tar.gz>...]

A convenience script to run the wikiparser with the maps generator as a drop-in replacement for the descriptions scraper.

Arguments:
    <BUILD_DIR> An existing directory to place descriptions in.
                The extracted articles will be placed in a `descriptions`
                subdirectory within this directory.
                The `intermediate_data` subfolder of a maps build directory may
                be used for this. The same folder may be used for multiple runs.
    <OSM_FILE>  An OpenStreetMap dump in PBF format to extract tags from.
    <DUMP_FILE> A wikipedia enterprise html dump. These take the form of
                `enwiki-NS0-20230401-ENTERPRISE-HTML.json.tar.gz`. Multiple
                dumps in the same language SHOULD NOT be provided, and will
                result in inconsistent data.

Options:
    -h      Print this help screen

1. Builds wikiparser.
2. Extracts wikidata qids and wikipedia urls from OpenStreetMap pbf file (NOTE: this spawns as many threads as there are cores).
3. Runs wikiparser in parallel for all input dump files (NOTE: this currently starts 2 processes for each dump files).

For information on running the wikiparser manually, see README.md.

For more information on the map generator, see
<https://github.com/organicmaps/organicmaps/blob/b52b42bd746fdb8daf05cc048f0b22654cfb9b8e/tools/python/maps_generator/README.md>.
'

set -euo pipefail
# set -x

# Parse options.
while getopts "h" opt
do
    case $opt in
    h)  echo -n "$USAGE"; exit 0;;
    ?)  echo "$USAGE" | head -n1 >&2; exit 1;;
    esac
done
shift $((OPTIND - 1))

if [ -z "${3-}" ]; then
    echo "BUILD_DIR, OSM_FILE, and at least one DUMP_FILE are required" >&2
    echo -n "$USAGE" >&2
    exit 1
fi

# Process and canonicalize all path arguments before changing directories.

BUILD_DIR=$(readlink -f -- "$1")
shift
if [ ! -d "$BUILD_DIR" ]; then
    echo "BUILD_DIR '$BUILD_DIR' does not exist or is not a directory" >&2
    exit 1
fi

OSM_FILE=$(readlink -f -- "$1")
shift
if [ ! -f "$OSM_FILE" ]; then
    echo "OSM_FILE '$OSM_FILE' does not exist or is not a file" >&2
    exit 1
fi

DUMP_FILES=()
while (( $# > 0 )); do
    dump_file="$(readlink -f -- "$1")"
    if [ ! -f "$dump_file" ]; then
        echo "DUMP_FILE '$dump_file' does not exist or is not a file" >&2
        exit 1
    fi
    DUMP_FILES+=("$dump_file")
    shift
done

# Ensure we're running in the directory of this script.
SCRIPT_PATH=$(dirname "$0")
cd "$SCRIPT_PATH"
SCRIPT_PATH=$(pwd)

# only load library after changing to script directory
source lib.sh

log "Using maps build directory '$BUILD_DIR'"

if ! command -v "cargo" > /dev/null; then
    echo -e "'cargo' is not installed, cannot build wikiparser.\nSee <https://www.rust-lang.org/>." >&2
    exit 1
fi

log "Building wikiparser"
cargo build --release
wikiparser=$(pwd)/target/release/om-wikiparser

log "Changing to maps build dir '$BUILD_DIR'"
cd "$BUILD_DIR"

log "Extracting tags from '$OSM_FILE'"
"$wikiparser" get-tags "$OSM_FILE" > osm_tags.tsv

# Enable backtraces in errors and panics.
export RUST_BACKTRACE=1
# Set log level.
export RUST_LOG=om_wikiparser=info

# Begin extraction.
OUTPUT_DIR=$(pwd)/descriptions
if [ ! -e "$OUTPUT_DIR" ]; then
    mkdir "$OUTPUT_DIR"
fi
log "Extracting articles to '$OUTPUT_DIR'"

kill_jobs() {
    pids=$(jobs -p)
    if [ -n "$pids" ]; then
        log "Killing background jobs"
        # shellcheck disable=SC2086 # PIDs are intentionally expanded.
        kill $pids
        log "Waiting for background jobs to stop"
        wait
    fi
}

trap 'kill_jobs' SIGINT SIGTERM EXIT

for dump in "${DUMP_FILES[@]}"; do
  log "Extracting '$dump'"
  tar xzOf "$dump" | "$wikiparser" get-articles \
    --osm-tags osm_tags.tsv \
    --write-new-qids new_qids.txt \
    "$OUTPUT_DIR" &
done

wait

log "Beginning extraction of discovered QIDs"

# Extract new qids from other dumps in parallel.
for dump in "${DUMP_FILES[@]}"; do
  tar xzOf "$dump" | "$wikiparser" get-articles \
    --wikidata-qids new_qids.txt \
    "$OUTPUT_DIR" &
done

wait

log "Finished"
