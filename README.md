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
| Canonical ASCII | lucid `UrlAggregator` | 56.24 | 17,780,475 |
| Canonical ASCII | Ada `url_aggregator` | 91.38 | 10,943,511 |
| Canonical ASCII | lucid `Url` | 57.44 | 17,410,418 |
| Canonical ASCII | Ada `url` | 71.91 | 13,905,844 |
| Normalization-heavy | lucid `UrlAggregator` | 103.49 | 9,662,428 |
| Normalization-heavy | Ada `url_aggregator` | 176.38 | 5,669,445 |
| Normalization-heavy | lucid `Url` | 104.07 | 9,608,782 |
| Normalization-heavy | Ada `url` | 129.96 | 7,694,925 |

The real-world corpora exercise parsing plus `get_href_size`, as well as the
construction-free `can_parse` API:

| Corpus | Implementation | ns/URL | URLs/s |
| --- | --- | ---: | ---: |
| Top sites | lucid `UrlAggregator` | 58.83 | 16,997,802 |
| Top sites | Ada `url_aggregator` | 88.82 | 11,258,325 |
| Top sites | lucid `Url` | 58.82 | 17,001,702 |
| Top sites | Ada `url` | 87.73 | 11,398,369 |
| Top sites | lucid `can_parse` | 13.40 | 74,624,541 |
| Top sites | Ada `can_parse` | 50.45 | 19,822,733 |
| 100,025 URLs | lucid `UrlAggregator` | 69.81 | 14,325,422 |
| 100,025 URLs | Ada `url_aggregator` | 84.88 | 11,781,605 |
| 100,025 URLs | lucid `Url` | 69.65 | 14,356,574 |
| 100,025 URLs | Ada `url` | 98.99 | 10,101,873 |
| 100,025 URLs | lucid `can_parse` | 11.14 | 89,760,951 |
| 100,025 URLs | Ada `can_parse` | 35.37 | 28,275,480 |

Lucid is faster in every measured operation across the focused, top-sites,
and 100,025-URL corpora. The corpora remain separate so a combined number
cannot hide workload-specific behavior.

## License

MIT
