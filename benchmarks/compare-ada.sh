#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ada_commit="30f3f3020c5a979b62f90dc9c37fd45de3cc84d7"
ada_root="${repo_root}/target/ada-benchmark"
ada_build="${ada_root}/build"
ada_binary="${repo_root}/target/ada-benchmark-runner"

if [[ ! -d "${ada_root}/.git" ]]; then
  git clone https://github.com/ada-url/ada.git "${ada_root}"
fi

git -C "${ada_root}" fetch origin "${ada_commit}"
git -C "${ada_root}" checkout --detach "${ada_commit}"

cmake \
  -S "${ada_root}" \
  -B "${ada_build}" \
  -DCMAKE_BUILD_TYPE=Release \
  -DADA_BENCHMARKS=OFF \
  -DADA_INCLUDE_URL_PATTERN=OFF \
  -DADA_TESTING=OFF \
  -DADA_TOOLS=OFF
cmake --build "${ada_build}" --config Release --parallel

c++ \
  -std=c++20 \
  -O3 \
  -DNDEBUG \
  -I"${ada_root}/include" \
  "${repo_root}/benchmarks/ada.cpp" \
  "${ada_build}/src/libada.a" \
  -o "${ada_binary}"

echo "lucid-url"
cargo bench --manifest-path "${repo_root}/Cargo.toml" --bench parse

echo
echo "Ada ${ada_commit}"
"${ada_binary}"
