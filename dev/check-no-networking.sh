#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

tmp_tree="$(mktemp)"
cleanup() {
    rm -f "$tmp_tree"
}
trap cleanup EXIT

echo "==> Checking dependency graph for networking and telemetry crates"
cargo tree --locked -e normal > "$tmp_tree"

crate_names=(
    'reqwest'
    'ureq'
    'surf'
    'isahc'
    'attohttpc'
    'awc'
    'hyper'
    'hyper-util'
    'hyper-rustls'
    'hyper-tls'
    'tower-http'
    'tonic'
    'tonic-web'
    'axum'
    'warp'
    'actix-web'
    'actix-http'
    'rouille'
    'tiny_http'
    'curl'
    'curl-sys'
    'native-tls'
    'tokio-native-tls'
    'async-native-tls'
    'rustls'
    'rustls-native-certs'
    'tokio-rustls'
    'tungstenite'
    'tokio-tungstenite'
    'websocket'
    'ws'
    'opentelemetry'
    'opentelemetry_sdk'
    'opentelemetry-otlp'
    'tracing-opentelemetry'
    'segment'
    'mixpanel'
    'posthog-rs'
    'sentry'
    'sentry-core'
    'sentry-tracing'
    'rudderanalytics-rust'
)

found_crates=()
for crate_name in "${crate_names[@]}"; do
    while IFS= read -r line; do
        found_crates+=("$line")
    done < <(grep -E "(^|[^A-Za-z0-9_-])${crate_name} v" "$tmp_tree" | sed 's/^[^A-Za-z0-9]*//')
done

if ((${#found_crates[@]} > 0)); then
    printf 'error: dependency graph contains suspicious crates:\n' >&2
    printf '  %s\n' "${found_crates[@]}" | sort -u >&2
    exit 1
fi

echo "==> Checking product code for socket, HTTP client, and telemetry APIs"

source_patterns=(
    'std::net::TcpStream'
    'std::net::TcpListener'
    'std::net::UdpSocket'
    'std::os::unix::net::UnixStream'
    'std::os::unix::net::UnixListener'
    'tokio::net::'
    'reqwest::'
    'ureq::'
    'hyper::client'
    'hyper_util::client'
    'attohttpc::'
    'isahc::'
    'surf::'
    'tonic::transport'
    'opentelemetry'
    'sentry'
    'mixpanel'
    'posthog'
)

found_source=()
for pattern in "${source_patterns[@]}"; do
    while IFS= read -r line; do
        found_source+=("$line")
    done < <(rg -n -F "$pattern" src || true)
done

while IFS= read -r line; do
    found_source+=("$line")
done < <(rg -n 'Command::new\("([^"]*(curl|wget|nc|netcat|telnet|ssh|scp|ftp|tftp|powershell|pwsh))"' src || true)

if ((${#found_source[@]} > 0)); then
    printf 'error: product code contains suspicious networking or telemetry usage:\n' >&2
    printf '  %s\n' "${found_source[@]}" | sort -u >&2
    exit 1
fi

echo "No-networking policy check passed."
echo "No common outbound-networking or telemetry crates/APIs were found in the shipped product code."
echo "This is a policy guardrail, not a security audit or proof against malicious behavior."
