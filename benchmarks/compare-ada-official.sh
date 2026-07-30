#!/usr/bin/env bash
set -euo pipefail

benchmark_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "${benchmark_root}/run.sh" "$@"
