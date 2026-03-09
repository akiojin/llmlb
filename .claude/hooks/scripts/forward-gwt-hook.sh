#!/bin/bash

# Claude Code Hook: Forward event payload to gwt hook handler.
# Best-effort only: this script never blocks Claude execution.

event="$1"
if [ -z "$event" ]; then
    exit 0
fi

payload=$(cat || true)

run_hook() {
    local executable="$1"
    if [ -z "$executable" ]; then
        return 1
    fi

    printf '%s' "$payload" | "$executable" hook "$event" >/dev/null 2>&1
}

# Optional explicit override.
if [ -n "${GWT_HOOK_EXECUTABLE:-}" ]; then
    if run_hook "$GWT_HOOK_EXECUTABLE"; then
        exit 0
    fi
fi

# PATH candidates.
if command -v gwt-tauri >/dev/null 2>&1; then
    if run_hook "$(command -v gwt-tauri)"; then
        exit 0
    fi
fi
if command -v gwt >/dev/null 2>&1; then
    if run_hook "$(command -v gwt)"; then
        exit 0
    fi
fi

# Common app-install locations.
for candidate in \
    "$HOME/Applications/gwt.app/Contents/MacOS/gwt-tauri" \
    "/Applications/gwt.app/Contents/MacOS/gwt-tauri"
do
    if [ -x "$candidate" ] && run_hook "$candidate"; then
        exit 0
    fi
done

exit 0
