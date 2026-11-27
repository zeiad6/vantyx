#!/bin/sh
# VANTYX_CC wrapper used by the CI sanitizer job and local sanitizer runs.
#
# The native backend invokes this script as its C compiler. AddressSanitizer
# and UndefinedBehaviorSanitizer instrument generated programs; leak detection
# is disabled by the caller because native allocations are intentionally
# leaked in v0.1.
set -eu
exec clang -fsanitize=address,undefined -fno-sanitize-recover=all \
    -fno-omit-frame-pointer -g "$@"
