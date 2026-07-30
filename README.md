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

- URL parsing, serialization, getters, and `can_parse`: 919 cases
- URL setters: 296 cases
- `IdnaTestV2` through the host parser: 2671 cases
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
`9749b92c13e970e70409948fa862461191504ccc`. Ada is pinned to the exact
[Ada v4 release benchmark](https://www.yagiz.co/release-of-ada-v4) commit,
`16a5772360d4b901fc3b35ee1ee6947782ab9491`.

Ada's optional simdutf path is off by default, matching Ada's default build.
It can be measured explicitly with:

```sh
ADA_USE_SIMDUTF=ON ./benchmarks/run.sh
```

For Lucid-only development regressions, without making a comparison claim:

```sh
cargo bench --bench parse
```

Results on an Apple M4 running macOS 26.5.2, Rust 1.97.0, and Apple Clang 21:

| Corpus | Implementation | ns/URL | URLs/s |
| --- | --- | ---: | ---: |
| Canonical ASCII | lucid `UrlAggregator` | 55.37 | 18,060,205 |
| Canonical ASCII | Ada `url_aggregator` | 90.06 | 11,103,140 |
| Canonical ASCII | lucid `Url` | 55.16 | 18,127,717 |
| Canonical ASCII | Ada `url` | 68.78 | 14,539,609 |
| Normalization-heavy | lucid `UrlAggregator` | 101.31 | 9,870,648 |
| Normalization-heavy | Ada `url_aggregator` | 169.82 | 5,888,483 |
| Normalization-heavy | lucid `Url` | 102.44 | 9,761,850 |
| Normalization-heavy | Ada `url` | 127.44 | 7,846,607 |

Additional Lucid-specific regression corpora exercise internationalized domains
and longer canonical inputs:

| Corpus | Implementation | ns/URL | URLs/s |
| --- | --- | ---: | ---: |
| Unicode and IDNA | lucid `UrlAggregator` | 781.64 | 1,279,354 |
| Unicode and IDNA | lucid `Url` | 779.97 | 1,282,101 |
| Long canonical scans | lucid `UrlAggregator` | 59.44 | 16,824,240 |
| Long canonical scans | lucid `Url` | 59.60 | 16,779,507 |

The real-world corpora exercise parsing plus `get_href_size`, as well as the
construction-free `can_parse` API:

| Corpus | Implementation | ns/URL | URLs/s |
| --- | --- | ---: | ---: |
| Top sites | lucid `UrlAggregator` | 55.77 | 17,929,349 |
| Top sites | Ada `url_aggregator` | 88.51 | 11,297,710 |
| Top sites | lucid `Url` | 56.00 | 17,858,094 |
| Top sites | Ada `url` | 87.55 | 11,422,233 |
| Top sites | lucid `can_parse` | 13.18 | 75,865,062 |
| Top sites | Ada `can_parse` | 49.54 | 20,187,468 |
| 100,025 URLs | lucid `UrlAggregator` | 56.62 | 17,662,483 |
| 100,025 URLs | Ada `url_aggregator` | 84.08 | 11,892,930 |
| 100,025 URLs | lucid `Url` | 56.28 | 17,768,113 |
| 100,025 URLs | Ada `url` | 95.33 | 10,490,154 |
| 100,025 URLs | lucid `can_parse` | 9.47 | 105,582,000 |
| 100,025 URLs | Ada `can_parse` | 34.65 | 28,857,454 |

Lucid is faster in every measured operation across the focused, top-sites,
and 100,025-URL corpora. The corpora remain separate so a combined number
cannot hide workload-specific behavior.

### Official Ada benchmark protocol

The default benchmark uses the Google Benchmark protocol from the
[Ada v4 release benchmark](https://www.yagiz.co/release-of-ada-v4). It runs
the official parse-plus-href and `can_parse` operations and reports the mean
of five repetitions. `./benchmarks/compare-ada-official.sh` remains as a
backwards-compatible alias. Results from the same Apple M4 system:

| Operation | Lucid ns/URL | Ada ns/URL | Lucid speedup |
| --- | ---: | ---: | ---: |
| `Url` parse + href | 76.33 | 132.31 | 1.73× |
| `UrlAggregator` parse + href | 57.31 | 82.33 | 1.44× |
| `can_parse` | 9.05 | 34.74 | 3.84× |

Ada accepts three malformed doubled-scheme inputs in the corpus that Lucid
rejects. The runner reports every disagreement; they account for 0.003% of the
100,025 URLs. The `Url` benchmark forces the owned href through an optimization
barrier so Rust cannot replace materialization with a length lookup.

### Release artifact size

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
