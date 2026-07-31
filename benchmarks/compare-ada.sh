#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ada_commit="${ADA_COMMIT:-0a371d6b82c282948597d80f63e856862c8ce667}"
dataset_commit="${ADA_DATASET_COMMIT:-9749b92c13e970e70409948fa862461191504ccc}"
ada_simdutf="${ADA_USE_SIMDUTF:-OFF}"
ada_root="${repo_root}/target/ada-benchmark"
ada_build="${ada_root}/build"
ada_binary="${repo_root}/target/ada-benchmark-runner"
ada_real_world_binary="${repo_root}/target/ada-real-world-runner"
dataset_root="${repo_root}/target/ada-url-dataset"
cxx="${CXX:-c++}"
host_os="$(uname -s)"

# C++ LTO off on Darwin: Apple Clang aborts the linker (Unexistent dir .../cc-XXXX.o).
if [[ -n "${ADA_ENABLE_LTO:-}" ]]; then
  enable_lto="${ADA_ENABLE_LTO}"
elif [[ "${host_os}" == "Darwin" ]]; then
  enable_lto="OFF"
else
  enable_lto="ON"
fi

lto_flag=""
cmake_ipo="OFF"
cmake_linker_flags=""
if [[ "${enable_lto}" == "ON" ]]; then
  cmake_ipo="ON"
  if [[ -n "${ADA_LTO_MODE:-}" ]]; then
    case "${ADA_LTO_MODE}" in
      full) lto_flag="-flto=full" ;;
      thin) lto_flag="-flto=thin" ;;
      auto) lto_flag="-flto=auto" ;;
      *)
        echo "ADA_LTO_MODE must be full, thin, or auto (got: ${ADA_LTO_MODE})" >&2
        exit 2
        ;;
    esac
  elif [[ "${host_os}" == "Darwin" ]]; then
    lto_flag="-flto=thin"
  elif "${cxx}" --version 2>/dev/null | head -n 1 | grep -Eqi 'clang'; then
    lto_flag="-flto=full"
  else
    lto_flag="-flto=auto"
  fi
  cmake_linker_flags="${lto_flag}"
fi

if [[ -n "${ADA_CXX_FLAGS:-}" ]]; then
  # shellcheck disable=SC2206
  cxx_opt_flags=(${ADA_CXX_FLAGS})
else
  if [[ "${host_os}" == "Darwin" ]]; then
    cxx_opt_flags=(
      -O3
      -DNDEBUG
      -mcpu=native
      -fomit-frame-pointer
    )
  else
    cxx_opt_flags=(
      -O3
      -DNDEBUG
      -march=native
      -mtune=native
      -fomit-frame-pointer
    )
  fi
  if [[ -n "${lto_flag}" ]]; then
    cxx_opt_flags+=("${lto_flag}")
  fi
fi
cmake_cxx_flags="${cxx_opt_flags[*]}"

if [[ "${enable_lto}" != "ON" && -f "${ada_build}/CMakeCache.txt" ]]; then
  if grep -Eq 'INTERPROCEDURAL_OPTIMIZATION(:.*)?=ON|flto' \
    "${ada_build}/CMakeCache.txt" 2>/dev/null; then
    echo "Clearing ${ada_build} (previous build used LTO; rebuilding without it)"
    rm -rf "${ada_build}"
  fi
fi

if [[ " ${RUSTFLAGS:-} " != *" target-cpu="* ]]; then
  export RUSTFLAGS="${RUSTFLAGS:+${RUSTFLAGS} }-C target-cpu=native"
fi

if [[ "${enable_lto}" == "ON" ]]; then
  export CARGO_PROFILE_RELEASE_LTO=true
  export CARGO_PROFILE_BENCH_LTO=true
else
  export CARGO_PROFILE_RELEASE_LTO=false
  export CARGO_PROFILE_BENCH_LTO=false
fi

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

echo "Ada commit: ${ada_commit}"
echo "Dataset commit: ${dataset_commit}"
echo "ADA_USE_SIMDUTF: ${ada_simdutf}"
echo "ADA_ENABLE_LTO: ${enable_lto}"
echo "Rust LTO: ${enable_lto}"
echo "Ada amalgamation: ON"
echo "Ada/C++ opt flags: ${cmake_cxx_flags}"
echo "RUSTFLAGS: ${RUSTFLAGS:-<empty>}"

cmake \
  -S "${ada_root}" \
  -B "${ada_build}" \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INTERPROCEDURAL_OPTIMIZATION="${cmake_ipo}" \
  -DCMAKE_CXX_FLAGS="${cmake_cxx_flags}" \
  -DCMAKE_EXE_LINKER_FLAGS="${cmake_linker_flags}" \
  -DCMAKE_SHARED_LINKER_FLAGS="${cmake_linker_flags}" \
  -DCMAKE_MODULE_LINKER_FLAGS="${cmake_linker_flags}" \
  -DADA_BENCHMARKS=OFF \
  -DADA_INCLUDE_URL_PATTERN=OFF \
  -DADA_TESTING=OFF \
  -DADA_TOOLS=OFF \
  -DADA_USE_SIMDUTF="${ada_simdutf}"

extra_libs=()
ada_defines=(
  -DLUCID_URL_AMALGAMATE_ADA
  -DLUCID_URL_ADA_AMALGAMATION="\"${ada_root}/src/ada.cpp\""
  -DADA_INCLUDE_URL_PATTERN=0
)
include_args=(
  -I"${ada_root}/include"
  -I"${ada_root}/src"
)
if [[ "${ada_simdutf}" == "ON" ]]; then
  cmake --build "${ada_build}" --config Release --target simdutf --parallel
  ada_defines+=(-DADA_USE_SIMDUTF)
  simdutf_lib="$(find "${ada_build}" -name 'libsimdutf.a' -print 2>/dev/null | sort | head -n 1)"
  simdutf_include="$(find "${ada_build}/_deps" -type d -path '*/simdutf-src/include' 2>/dev/null | sort | head -n 1)"
  if [[ -z "${simdutf_lib}" || -z "${simdutf_include}" ]]; then
    echo "ADA_USE_SIMDUTF=ON but simdutf was not produced under ${ada_build}" >&2
    exit 1
  fi
  extra_libs+=("${simdutf_lib}")
  include_args+=(-I"${simdutf_include}")
  echo "Linking simdutf: ${simdutf_lib}"
fi

compile_ada_harness() {
  local source="$1"
  local output="$2"
  shift 2
  "${cxx}" \
    -std=c++20 \
    ${cxx_opt_flags[@]+"${cxx_opt_flags[@]}"} \
    -Wno-deprecated-declarations \
    -Wno-deprecated-volatile \
    -Wno-unused-parameter \
    -Wno-sign-conversion \
    ${ada_defines[@]+"${ada_defines[@]}"} \
    ${include_args[@]+"${include_args[@]}"} \
    "${source}" \
    ${extra_libs[@]+"${extra_libs[@]}"} \
    -o "${output}" \
    "$@"
}

compile_ada_harness \
  "${repo_root}/benchmarks/ada.cpp" \
  "${ada_binary}"

compile_ada_harness \
  "${repo_root}/benchmarks/ada_real_world.cpp" \
  "${ada_real_world_binary}"

export ADA_DATASET_COMMIT="${dataset_commit}"
export ADA_COMMIT="${ada_commit}"

echo "lucid-url microbenchmarks"
cargo bench --manifest-path "${repo_root}/Cargo.toml" --bench parse

echo
echo "Ada ${ada_commit} microbenchmarks"
"${ada_binary}"

echo
echo "lucid-url real-world corpora"
echo "(mixed top sites = parse only; clean HTTP = can_parse; benchdata = both)"
echo "dataset: ${dataset_root}/out.txt @ ${dataset_commit}"
cargo bench \
  --manifest-path "${repo_root}/Cargo.toml" \
  --bench real_world \
  -- \
  "${dataset_root}/out.txt"

echo
echo "Ada ${ada_commit} real-world corpora"
echo "(mixed top sites = parse only; clean HTTP = can_parse; benchdata = both)"
echo "dataset: ${dataset_root}/out.txt @ ${dataset_commit}"
"${ada_real_world_binary}" "${dataset_root}/out.txt"
