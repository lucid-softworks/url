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
| Canonical ASCII | lucid `UrlAggregator` | 72.26 | 13,839,137 |
| Canonical ASCII | Ada `url_aggregator` | 93.07 | 10,744,112 |
| Canonical ASCII | lucid `Url` | 73.18 | 13,664,172 |
| Canonical ASCII | Ada `url` | 70.52 | 14,179,391 |
| Normalization-heavy | lucid `UrlAggregator` | 115.64 | 8,647,830 |
| Normalization-heavy | Ada `url_aggregator` | 173.65 | 5,758,705 |
| Normalization-heavy | lucid `Url` | 115.54 | 8,655,046 |
| Normalization-heavy | Ada `url` | 130.81 | 7,644,627 |

The real-world corpora exercise parsing plus `get_href_size`, as well as the
construction-free `can_parse` API:

| Corpus | Implementation | ns/URL | URLs/s |
| --- | --- | ---: | ---: |
| Top sites | lucid `UrlAggregator` | 72.95 | 13,708,544 |
| Top sites | Ada `url_aggregator` | 87.43 | 11,437,794 |
| Top sites | lucid `Url` | 72.58 | 13,777,480 |
| Top sites | Ada `url` | 89.05 | 11,229,505 |
| Top sites | lucid `can_parse` | 29.71 | 33,658,651 |
| Top sites | Ada `can_parse` | 50.30 | 19,881,109 |
| 100,025 URLs | lucid `UrlAggregator` | 84.48 | 11,836,695 |
| 100,025 URLs | Ada `url_aggregator` | 85.39 | 11,710,671 |
| 100,025 URLs | lucid `Url` | 84.78 | 11,795,808 |
| 100,025 URLs | Ada `url` | 98.24 | 10,179,108 |
| 100,025 URLs | lucid `can_parse` | 26.92 | 37,149,350 |
| 100,025 URLs | Ada `can_parse` | 35.48 | 28,182,357 |

Lucid is faster across the 100,025-URL corpus, all three top-sites operations,
and both normalization-heavy representations. Ada remains faster for the
setter-oriented `url` representation on the focused canonical ASCII corpus.
The corpora remain separate so a combined number cannot hide
workload-specific behavior.

## License

MIT
