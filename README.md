# lucid-url

A dependency-free WHATWG URL parser written in Rust, with a Rust adaptation of
Ada's URL interface.

The implementation uses only Rust's standard library. Ada is not linked,
vendored, or used by the parser; its current public interface is the
compatibility target and its parser is used only by the optional comparison
benchmark.

## Usage

```rust
use lucid_url::{UrlAggregator, parse};

let url = parse::<UrlAggregator>(
    "https://user:pass@example.com:8080/path?query=yes#fragment",
    None,
)?;

assert_eq!(url.get_protocol(), "https:");
assert_eq!(url.get_hostname(), "example.com");
assert_eq!(url.get_pathname(), "/path");
```

Parse into `Url` when the Ada-style owned getter surface is useful:

```rust
use lucid_url::{Url, parse};

let mut url = parse::<Url>("https://example.com/", None)?;
url.set_hostname("example.org");
url.set_pathname("/account");

assert_eq!(url.get_href(), "https://example.org/account");
```

Relative URLs take a parsed base of the same representation:

```rust
use lucid_url::{UrlAggregator, parse};

let base = parse::<UrlAggregator>("https://example.com/a/b", None)?;
let url = parse::<UrlAggregator>("../c", Some(&base))?;

assert_eq!(url.get_href(), "https://example.com/c");
```

## Interface

The parser mirrors Ada's URL-facing concepts:

- `parse::<UrlAggregator>(input, base)` and `parse::<Url>(input, base)`
- `can_parse(input, base)`
- `UrlAggregator`, backed by one serialized buffer and component offsets
- `Url`, with Ada-compatible owned and borrowed getter return types
- component getters, setters, presence checks, `get_origin`, and
  `get_components`
- `href_from_file`, `set_max_input_length`, and `get_max_input_length`

`UrlSearchParams` and `UrlPattern` are not part of the initial parser crate.

## Conformance

The repository includes Ada's parser fixtures at a pinned test revision. A
fresh checkout runs them through both `Url` and `UrlAggregator` as part of
ordinary `cargo test`; missing or malformed fixtures are hard failures.

- URL parsing, serialization, getters, and `can_parse`: 921 cases
- URL setters: 296 cases
- `IdnaTestV2` through the host parser: 2670 representable cases
- Ada ToASCII success cases: 68 cases
- percent encoding: 7 cases
- DNS/domain-length validation: 17 cases

Run the mandatory suite directly with:

```sh
cargo test --test ada_conformance --locked
```

The fixtures can be refreshed to another Ada revision with
`./scripts/update-ada-fixtures.sh <commit>`. A daily workflow also runs the
suite against Ada's current `main` and fails when upstream parser tests change.
The exact coverage and justified exclusions are documented in
[`tests/ADA_TEST_SCOPE.md`](tests/ADA_TEST_SCOPE.md).

The parser has direct paths for validated canonical URLs and common
normalization work, including percent encoding, path normalization, IPv4,
IPv6, and IDNA. Inputs outside those conservative paths fall through to the
complete WHATWG state machine.

## Benchmarks

Run the default, pinned comparison against Ada:

```sh
./benchmarks/run.sh
```

This command always fetches and builds both implementations, then runs them in
the same Google Benchmark executable over the same corpus. It prints the
operating system, compiler versions, Ada options, Ada revision, and dataset
revision before reporting results. A missing compiler or failed Ada build is a
hard failure rather than a Lucid-only fallback.

The comparison fetches Ada's 100,025-URL dataset at commit
`9749b92c13e970e70409948fa862461191504ccc`. Ada is pinned to
`b2c2d7f6b5723a4b924409f9d80ce517d6db8226`, the latest `main` revision
checked on September 7, 2026. The compatibility fixtures use the same revision.

Ada's optional simdutf path is off by default, matching Ada's default build.
It can be measured explicitly with:

```sh
ADA_USE_SIMDUTF=ON ./benchmarks/run.sh
```

For Lucid-only development regressions, without making a comparison claim:

```sh
cargo bench --bench parse
```

These microbenchmarks exercise focused canonical, normalization, Unicode,
IDNA, and long-input paths. They are regression aids rather than published
Ada comparisons.

### Official Ada benchmark protocol

The default benchmark uses the Google Benchmark protocol from the
[Ada v4 release benchmark](https://www.yagiz.co/release-of-ada-v4). It runs
the official parse-plus-href and `can_parse` operations and reports the mean
of five repetitions. It also refuses to run if the parsers disagree about
which corpus inputs are valid or how any accepted input is serialized.

Results from `./benchmarks/run.sh` on September 7, 2026, on an Apple M4
running macOS 26.5, Rust 1.98.0, and Apple Clang 21, with
`ADA_USE_SIMDUTF=OFF`. Values are mean CPU time per URL over five repetitions.
[Raw Google Benchmark results](benchmarks/results/ada-b2c2d7f6.json) are retained
for reproducibility; the benchmarked Lucid source is commit `23d349f`.

| Operation | Lucid ns/URL | Ada ns/URL | Lucid speedup |
| --- | ---: | ---: | ---: |
| `Url` parse + href | 83.06 | 109.49 | 1.32× |
| `UrlAggregator` parse + href | 58.58 | 58.54 | 1.00× (tie) |
| `can_parse` | 9.71 | 11.00 | 1.13× |

Both parsers accept, reject, and serialize the same inputs in this 100,025-URL
run (26 rejected, zero validity or serialization disagreements). The aggregator
difference is below measurement variability: its coefficient of variation was
0.52% for Lucid and 1.25% for Ada. The `Url` benchmark forces the owned href
through an optimization barrier so Rust cannot replace materialization with a
length lookup.

### Historical release artifact size

These size measurements predate the September 2026 refresh and use Ada
`16a5772360d4b901fc3b35ee1ee6947782ab9491`; they have not been rerun
against the current pin.

For a native-code comparison, both libraries were built at optimization level
3 without LTO so the Apple Mach-O `size` tool could inspect their complete
objects:

| Release object | Lucid | Ada |
| --- | ---: | ---: |
| Native object sections | 462.1 KiB | 318.4 KiB |
| Machine-code `__text` | 92.3 KiB | 226.9 KiB |

Lucid's complete native object is 45% larger, while its machine code is 59%
smaller. Most of Lucid's remaining footprint is its dependency-free Unicode and
IDNA data. Raw compiler archives are not comparable: Rust's `.rlib` also
contains 1.52 MiB of compiler metadata and LLVM input used for downstream
generic compilation and LTO, while Ada's `.a` is a conventional native archive.

## License

MIT
