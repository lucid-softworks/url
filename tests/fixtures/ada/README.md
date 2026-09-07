# Ada compatibility fixtures

These fixtures are copied from
[`ada-url/ada`](https://github.com/ada-url/ada) at commit
`b2c2d7f6b5723a4b924409f9d80ce517d6db8226`.

They are committed so a fresh checkout can reproduce the conformance results
without network access. `ADA_TEST_ROOT` may point the test harness at another
Ada checkout's `tests/wpt` directory, which is used by the scheduled
compatibility workflow to detect upstream changes.

Included parser fixtures:

- `IdnaTestV2.json`
- `percent-encoding.json`
- `setters_tests.json`
- `ada_extra_setters_tests.json`
- `toascii.json`
- `urltestdata.json`
- `ada_extra_urltestdata.json`
- `verifydnslength_tests.json`

Ada's MIT and Apache-2.0 license files are included alongside the fixtures.
