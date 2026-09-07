# Comparison audit

The reference is Ada `b2c2d7f6b5723a4b924409f9d80ce517d6db8226`.
The reviewed anonrig (Yagiz Nizipli) changes include:

- [#1214: SIMD scanners and host preservation](https://github.com/ada-url/ada/commit/0b88932)
- [#1216: nibble-table SIMD classification](https://github.com/ada-url/ada/commit/2705334)
- [#1221: absolute-path instruction reductions](https://github.com/ada-url/ada/commit/18c65ce)
- [#1222: scheme matching and SWAR](https://github.com/ada-url/ada/commit/2dd9194)
- [#1230: SIMD percent encoding](https://github.com/ada-url/ada/commit/fa9a175)

Ada's `CMakeLists.txt` defaults `ADA_USE_SIMDUTF` to **OFF**. This optional
Unicode/IDNA dependency is separate from the URL parser's built-in SIMD
scanners. The comparison preserves that default and enables URLPattern,
matching its upstream default too. The merged harness preserves main’s native CPU compilation, Ada amalgamation,
and matched LTO policy (off on macOS, on for Linux). On this machine its native NEON paths are
available. `ADA_USE_SIMDUTF=ON` remains supported, including linking simdutf;
the selected setting is recorded in the benchmark JSON.

The primary comparison uses the pinned 100,025-URL dataset, the upstream
parse-plus-href and `can_parse` workloads, and five Google Benchmark repetitions.
Both owned href results pass an optimization barrier. Before timing, the runner
checks validity and serialization between implementations, between both URL
representations, and between parsing and `can_parse`. Any disagreement stops
the run. This stronger check found and fixed Lucid's extra-authority-slash
validation regression.

The setter benchmark is a separate Lucid-before/after measurement. It includes
encoding and mutation, alternates values, and makes the complete resulting URL
observable. Initial parsing is outside the timed operation. Its reported gains
apply to the documented query/fragment and credential workloads, not general parsing or Ada.

The retained setter implementation replaces affected ranges and checks the
resulting size before mutation. Differential tests cover general-parser
agreement, ASCII and Unicode encoding, component offsets, empty components,
and fallback URL forms. The pinned Ada fixture and length-limit suites remain
mandatory. No parser result cache or corpus-specific production branch is used.

Hostname SWAR, wider scan blocks, lookup-based escape writing, and parser-stack
experiments were discarded because they did not demonstrate dependable gains.

## Credential setter follow-up

Re-fetching Ada main on 2026-09-07 still resolved to `b2c2d7f6`.
Ada [#1228](https://github.com/ada-url/ada/commit/4b8b4bfe5b79cc71539593261e43eed0379364d8)
reduces credential setter tail movement. Inspired by that change, Lucid now
replaces special-URL credentials in one buffer edit and adjusts the offsets,
without reparsing the unchanged host, path, query, or fragment. File and
non-special URLs retain the general setter. Credential controls are escaped,
including tabs and newlines. Resulting-size checks precede any mutation.

The credential baseline is `970dedc`, built from a separate source archive with
the identical expanded `benches/setters.rs`. Before and after run sequentially,
with five samples of at least 200 ms per workload and the median reported.
The retained implementation is `6e92ef4`. Raw results are `results/credentials-before.txt` and
`results/credentials-after.txt`. These measure Lucid before/after, not Ada.

The post-change corpus comparison is retained in
`results/ada-b2c2d7f6-credentials.json`: zero validity, serialization, or operation
disagreements on 100,025 inputs. Lucid/Ada mean ns per URL were 86.37/112.90
for owned parse-plus-href, 61.71/59.21 for aggregator parse-plus-href, and
13.21/11.43 for validation. This run recorded substantial background system
load (19.41 one-minute load average); small differences between runs should
not be attributed to the credential change. No parsing speedup is claimed.

## Merge with main

The merge retains main's benchmark result barriers, dataset reporting, native
CPU flags, Ada amalgamation, and matching LTO settings. It also retains this
branch's current Ada fixtures, parser fixes, and cross-operation preflight.
URLPattern stays enabled and SIMDUTF stays at its upstream OFF default.
Earlier setter comparisons remain historical measurements using identical fat
LTO builds before and after each optimization; the merged default release
profile follows main and disables LTO on this macOS machine.
