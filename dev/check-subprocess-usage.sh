#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

echo "==> Checking subprocess usage in product code"

current_sites=()
while IFS= read -r site; do
    current_sites+=("$site")
done < <(
    rg -n 'Command::new\(|std::process::Command::new\(' src \
        | sed -E 's/^([^:]+):[0-9]+:/\1:/'
)

allowed_sites=(
    'src/config_file.rs:        match Command::new(&editor).arg(&config_path).status() {'
    'src/interactive.rs:    let status = Command::new(&exe_path).args(cmd_args).status()?;'
    'src/decompression.rs:        let compress_result = Command::new("zstd")'
)

unexpected_sites=()
for site in "${current_sites[@]}"; do
    allowed=false
    for expected in "${allowed_sites[@]}"; do
        if [[ "$site" == "$expected" ]]; then
            allowed=true
            break
        fi
    done

    if [[ "$allowed" != true ]]; then
        unexpected_sites+=("$site")
    fi
done

if ((${#unexpected_sites[@]} > 0)); then
    printf 'error: unapproved subprocess usage found in src/:\n' >&2
    printf '  %s\n' "${unexpected_sites[@]}" >&2
    printf 'review the new call site and update dev/check-subprocess-usage.sh if it is intentional.\n' >&2
    exit 1
fi

echo "Subprocess usage check passed."
echo "Only the reviewed subprocess call sites are present in src/."
