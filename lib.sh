# Shared functions for scripts
# shellcheck shell=bash

# Write message to stderr with a timestamp and line ending.
log () {
    echo -e "$(date '+%Y-%m-%dT%H:%M:%SZ')" "$@" >&2
}
