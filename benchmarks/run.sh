#!/usr/bin/env bash
set -euo pipefail

benchmark_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ada_commit="${ADA_COMMIT:-b2c2d7f6b5723a4b924409f9d80ce517d6db8226}"
dataset_commit="${ADA_DATASET_COMMIT:-9749b92c13e970e70409948fa862461191504ccc}"
ada_simdutf="${ADA_USE_SIMDUTF:-OFF}"
benchmark_repetitions="${BENCHMARK_REPETITIONS:-5}"
ada_root="${benchmark_root}/target/ada-benchmark"
ada_build="${benchmark_root}/target/ada-official-benchmark"
dataset_root="${benchmark_root}/target/ada-url-dataset"
bridge_target="${benchmark_root}/target/official-benchmark-bridge"
runner="${benchmark_root}/target/ada-official-compare"
cxx="${CXX:-c++}"

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

echo "System: $(uname -a)"
echo "Rust: $(rustc --version)"
echo "C++: $(${cxx} --version | head -n 1)"
echo "CMake: $(cmake --version | head -n 1)"
echo "Ada commit: ${ada_commit}"
echo "Dataset commit: ${dataset_commit}"
echo "ADA_USE_SIMDUTF: ${ada_simdutf}"

cmake \
  -S "${ada_root}" \
  -B "${ada_build}" \
  -DCMAKE_BUILD_TYPE=Release \
  -DADA_BENCHMARKS=ON \
  -DADA_INCLUDE_URL_PATTERN=OFF \
  -DADA_TESTING=OFF \
  -DADA_TOOLS=OFF \
  -DADA_USE_SIMDUTF="${ada_simdutf}"
cmake --build "${ada_build}" --config Release --target ada benchmark --parallel

cargo build \
  --release \
  --locked \
  --manifest-path "${benchmark_root}/benchmarks/official_bridge/Cargo.toml" \
  --target-dir "${bridge_target}"

platform_link_args=()
case "$(uname -s)" in
  Darwin)
    platform_link_args=(-framework Security -framework CoreFoundation)
    ;;
  Linux)
    platform_link_args=(-ldl -lpthread -lm -lrt -lutil)
    ;;
  *)
    echo "unsupported benchmark platform: $(uname -s)" >&2
    exit 2
    ;;
esac

"${cxx}" \
  -std=c++20 \
  -O3 \
  -DNDEBUG \
  -Wno-deprecated-declarations \
  -Wno-deprecated-volatile \
  -DLUCID_URL_ADA_COMMIT="\"${ada_commit}\"" \
  -DLUCID_URL_DATASET_COMMIT="\"${dataset_commit}\"" \
  -DLUCID_URL_DATASET="\"${dataset_root}/out.txt\"" \
  -I"${ada_root}/include" \
  -I"${ada_build}/_deps/benchmark-src/include" \
  "${benchmark_root}/benchmarks/official_compare.cpp" \
  "${bridge_target}/release/liblucid_url_official_benchmark_bridge.a" \
  "${ada_build}/src/libada.a" \
  "${ada_build}/_deps/benchmark-build/src/libbenchmark.a" \
  "${platform_link_args[@]}" \
  -o "${runner}"

"${runner}" \
  --benchmark_repetitions="${benchmark_repetitions}" \
  --benchmark_report_aggregates_only=true \
  "$@"
