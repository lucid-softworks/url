#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ada_commit="16a5772360d4b901fc3b35ee1ee6947782ab9491"
dataset_commit="9749b92c13e970e70409948fa862461191504ccc"
ada_root="${repo_root}/target/ada-benchmark"
ada_build="${ada_root}/build"
ada_binary="${repo_root}/target/ada-benchmark-runner"
ada_real_world_binary="${repo_root}/target/ada-real-world-runner"
dataset_root="${repo_root}/target/ada-url-dataset"

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

c++ \
  -std=c++20 \
  -O3 \
  -DNDEBUG \
  -I"${ada_root}/include" \
  "${repo_root}/benchmarks/ada_real_world.cpp" \
  "${ada_build}/src/libada.a" \
  -o "${ada_real_world_binary}"

echo "lucid-url"
cargo bench --manifest-path "${repo_root}/Cargo.toml" --bench parse

echo
echo "Ada ${ada_commit}"
"${ada_binary}"

echo
echo "lucid-url real-world corpora"
cargo bench \
  --manifest-path "${repo_root}/Cargo.toml" \
  --bench real_world \
  -- \
  "${dataset_root}/out.txt"

echo
echo "Ada ${ada_commit} real-world corpora"
"${ada_real_world_binary}" "${dataset_root}/out.txt"
