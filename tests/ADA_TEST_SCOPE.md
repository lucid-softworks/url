# Ada test compatibility scope

The conformance harness runs every data-driven URL-parser fixture used by
Ada's `wpt_url_tests` suite at the pinned revision, against both `Url` and
`UrlAggregator`.

Covered:

- URL parsing, relative resolution, serialization, getters, and `can_parse`
- every standard and Ada-specific URL setter fixture
- 2670 representable UTS-46/IDNA cases and Ada's additional ToASCII successes
- percent encoding through URL parsing and setters
- valid-domain/DNS-length cases
- Ada-compatible maximum-input-length and `href_from_file` behavior in the
  crate's unit tests

Not applicable to this crate:

- `url_search_params.cpp`: `UrlSearchParams` is not part of this crate
- `wpt_urlpattern_tests.cpp`: `UrlPattern` is not part of this crate
- `ada_c.cpp`: this crate does not expose Ada's C ABI
- `installation/` and `wasm/`: build-system and binding tests, not parser
  behavior
- fuzzers: these are continuous fuzzing entry points rather than finite test
  cases; Rust fuzz targets should be maintained separately
- Ada-internal diagnostics such as `validate()` and `to_diagram()`, which have
  no public Rust equivalent

The exclusions are about absent interfaces, not unsupported parser cases. Any
new applicable fixture added to Ada's `tests/wpt` suite should be added to the
pinned set and made mandatory here.

The empty-host `IdnaTestV2` case cannot be isolated through the URL-wrapper
adaptation: `https:///x` correctly reparses `x` as the host in both Ada and
WHATWG. Testing that case directly would require Ada's standalone Unicode API,
which this crate does not expose.

## September 2026 refresh

Fixtures are pinned to `b2c2d7f6b5723a4b924409f9d80ce517d6db8226`.
The refreshed WPT corpus adds two IPv4-in-IPv6 rejection cases.
`ada_parser_regressions.rs` additionally adapts public-behavior regressions
from the upstream C++ tests for NFC host normalization, hexadecimal IPv4,
IPv6 boundaries, ports, URL whitespace, and credential/query replacement.
The length-limit suite includes expanded queries and fragments on slow paths.
These are selected public API regressions, not an execution of Ada's complete
C++ test suite; its internal SIMD and percent-encoding helper tests are not
ported directly.
