#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ada_commit="16a5772360d4b901fc3b35ee1ee6947782ab9491"
dataset_commit="9749b92c13e970e70409948fa862461191504ccc"
ada_root="${repo_root}/target/ada-benchmark"
ada_build="${repo_root}/target/ada-official-benchmark"
dataset_root="${repo_root}/target/ada-url-dataset"
bridge_target="${repo_root}/target/official-benchmark-bridge"
runner="${repo_root}/target/ada-official-compare"

if [[ ! -d "${ada_root}/.git" ]]; then
  git clone https://github.com/ada-url/ada.git "${ada_root}"
fi
git -C "${ada_root}" fetch origin "${ada_commit}"
git -C "${ada_root}" checkout --detach "${ada_commit}"

if [[ ! -d "${dataset_root}/.git" ]]; then
  git clone https://github.com/ada-url/url-dataset.git "${dataset_root}"
fi
git -C "${dataset_root}" fetch origin "${dataset_commit}"
git -C "${dataset_root}" checkout --detach "${dataset_commit}"

cmake \
  -S "${ada_root}" \
  -B "${ada_build}" \
  -DCMAKE_BUILD_TYPE=Release \
  -DADA_BENCHMARKS=ON \
  -DADA_INCLUDE_URL_PATTERN=OFF \
  -DADA_TESTING=OFF \
  -DADA_TOOLS=OFF
cmake --build "${ada_build}" --config Release --target ada benchmark --parallel

cargo build \
  --release \
  --manifest-path "${repo_root}/benchmarks/official_bridge/Cargo.toml" \
  --target-dir "${bridge_target}"

c++ \
  -std=c++20 \
  -O3 \
  -DNDEBUG \
  -Wno-deprecated-declarations \
  -Wno-deprecated-volatile \
  -DLUCID_URL_DATASET="\"${dataset_root}/out.txt\"" \
  -I"${ada_root}/include" \
  -I"${ada_build}/_deps/benchmark-src/include" \
  "${repo_root}/benchmarks/official_compare.cpp" \
  "${bridge_target}/release/liblucid_url_official_benchmark_bridge.a" \
  "${ada_build}/src/libada.a" \
  "${ada_build}/_deps/benchmark-build/src/libbenchmark.a" \
  -framework Security \
  -framework CoreFoundation \
  -o "${runner}"

"${runner}" \
  --benchmark_repetitions=5 \
  --benchmark_report_aggregates_only=true
