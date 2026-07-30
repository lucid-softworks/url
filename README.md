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

The parser was checked against the Web Platform Tests available during
development:

- URL parsing and serialization: 891/891
- `can_parse`: 891/891
- URL setters: 278/278
- `IdnaTestV2` through the host parser: 2671/2671

The parser has direct paths for validated canonical URLs and common
normalization work, including percent encoding, path normalization, IPv4,
IPv6, and IDNA. Inputs outside those conservative paths fall through to the
complete WHATWG state machine.

## Benchmarks

Run the focused Rust benchmark:

```sh
cargo bench --bench parse
```

For a pinned comparison against Ada:

```sh
./benchmarks/compare-ada.sh
```

The harness uses the same URL corpora, operation, warm-up policy, sample count,
and median calculation for both implementations. The comparison script also
fetches Ada's 100,025-URL dataset at commit
`9749b92c13e970e70409948fa862461191504ccc`. Ada is pinned to the exact
[Ada v4 release benchmark](https://www.yagiz.co/release-of-ada-v4) commit,
`16a5772360d4b901fc3b35ee1ee6947782ab9491`.

Results on an Apple M4 running macOS 26.5.2, Rust 1.97.0, and Apple Clang 21:

| Corpus | Implementation | ns/URL | URLs/s |
| --- | --- | ---: | ---: |
| Canonical ASCII | lucid `UrlAggregator` | 57.39 | 17,424,025 |
| Canonical ASCII | Ada `url_aggregator` | 90.48 | 11,051,891 |
| Canonical ASCII | lucid `Url` | 55.70 | 17,952,303 |
| Canonical ASCII | Ada `url` | 71.33 | 14,019,760 |
| Normalization-heavy | lucid `UrlAggregator` | 101.74 | 9,828,899 |
| Normalization-heavy | Ada `url_aggregator` | 171.74 | 5,822,726 |
| Normalization-heavy | lucid `Url` | 100.56 | 9,943,961 |
| Normalization-heavy | Ada `url` | 128.82 | 7,762,962 |

The real-world corpora exercise parsing plus `get_href_size`, as well as the
construction-free `can_parse` API:

| Corpus | Implementation | ns/URL | URLs/s |
| --- | --- | ---: | ---: |
| Top sites | lucid `UrlAggregator` | 55.29 | 18,086,416 |
| Top sites | Ada `url_aggregator` | 88.10 | 11,350,958 |
| Top sites | lucid `Url` | 56.73 | 17,627,083 |
| Top sites | Ada `url` | 88.14 | 11,344,956 |
| Top sites | lucid `can_parse` | 13.12 | 76,192,874 |
| Top sites | Ada `can_parse` | 50.07 | 19,973,953 |
| 100,025 URLs | lucid `UrlAggregator` | 57.77 | 17,310,493 |
| 100,025 URLs | Ada `url_aggregator` | 84.76 | 11,798,569 |
| 100,025 URLs | lucid `Url` | 57.72 | 17,325,112 |
| 100,025 URLs | Ada `url` | 96.79 | 10,331,350 |
| 100,025 URLs | lucid `can_parse` | 11.02 | 90,762,733 |
| 100,025 URLs | Ada `can_parse` | 35.21 | 28,404,000 |

Lucid is faster in every measured operation across the focused, top-sites,
and 100,025-URL corpora. The corpora remain separate so a combined number
cannot hide workload-specific behavior.

### Official Ada benchmark protocol

The repository can also run both libraries through the Google Benchmark
protocol used for the [Ada v4 release
benchmark](https://www.yagiz.co/release-of-ada-v4):

```sh
./benchmarks/compare-ada-official.sh
```

This pins Ada and its dataset to the commits above, runs the official
parse-plus-href and `can_parse` operations, and reports the mean of five
repetitions. Results from the same Apple M4 system:

| Operation | Lucid ns/URL | Ada ns/URL | Lucid speedup |
| --- | ---: | ---: | ---: |
| `Url` parse + href | 78.93 | 131.38 | 1.66× |
| `UrlAggregator` parse + href | 56.96 | 83.50 | 1.47× |
| `can_parse` | 10.15 | 33.89 | 3.34× |

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
| Native object sections | 449.8 KiB | 318.4 KiB |
| Machine-code `__text` | 92.9 KiB | 226.9 KiB |

Lucid's complete native object is 41% larger, while its machine code is 59%
smaller. Most of Lucid's remaining footprint is its dependency-free Unicode and
IDNA data. Raw compiler archives are not comparable: Rust's `.rlib` also
contains 1.48 MiB of compiler metadata and LLVM input used for downstream
generic compilation and LTO, while Ada's `.a` is a conventional native archive.

## License

MIT
