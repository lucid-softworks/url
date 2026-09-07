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
matching its upstream default too. Ada retains its normal Release compiler
settings and architecture dispatch. On this machine its native NEON paths are
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
apply to the documented query/fragment workloads, not general parsing or Ada.

The retained setter implementation replaces affected ranges and checks the
resulting size before mutation. Differential tests cover general-parser
agreement, ASCII and Unicode encoding, component offsets, empty components,
and fallback URL forms. The pinned Ada fixture and length-limit suites remain
mandatory. No parser result cache or corpus-specific production branch is used.

Hostname SWAR, wider scan blocks, lookup-based escape writing, and parser-stack
experiments were discarded because they did not demonstrate dependable gains.
