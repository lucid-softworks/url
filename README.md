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
- URL setters: 278/278
- `IdnaTestV2` through the host parser: 2671/2671

The parser has direct paths for validated canonical URLs and common
normalization work, including percent encoding, path normalization, IPv4,
IPv6, and IDNA. Inputs outside those conservative paths fall through to the
complete WHATWG state machine.

## Benchmarks

Run the Rust benchmark:

```sh
cargo bench --bench parse
```

For a pinned comparison against Ada:

```sh
./benchmarks/compare-ada.sh
```

The harness uses the same URL corpora, operation, warm-up policy, sample count,
and median calculation for both implementations. Ada is pinned to commit
`30f3f3020c5a979b62f90dc9c37fd45de3cc84d7`.

Results on an Apple M4 running macOS 26.5.2, Rust 1.97.0, and Apple Clang 21:

| Corpus | Implementation | ns/URL | URLs/s |
| --- | --- | ---: | ---: |
| Canonical ASCII | lucid `UrlAggregator` | 94.74 | 10,555,436 |
| Canonical ASCII | Ada `url_aggregator` | 94.39 | 10,594,374 |
| Canonical ASCII | lucid `Url` | 93.73 | 10,668,823 |
| Canonical ASCII | Ada `url` | 69.55 | 14,377,759 |
| Normalization-heavy | lucid `UrlAggregator` | 123.08 | 8,124,654 |
| Normalization-heavy | Ada `url_aggregator` | 173.86 | 5,751,804 |
| Normalization-heavy | lucid `Url` | 123.76 | 8,080,410 |
| Normalization-heavy | Ada `url` | 130.38 | 7,669,859 |

On the normalization-heavy corpus, lucid is about 29% faster than Ada's
`url_aggregator` and about 5% faster than Ada's `url`. The canonical corpus is
shown separately because combining the two workloads would hide their
different performance characteristics.

## License

MIT
