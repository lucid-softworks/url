use std::path::PathBuf;

use lucid_url::{Url, UrlAggregator, can_parse};
use serde_json::Value;

trait TestUrl: Sized {
    fn parse(input: &str) -> lucid_url::Result<Self>;
    fn parse_with_base(input: &str, base: &Self) -> lucid_url::Result<Self>;

    fn href(&self) -> String;
    fn protocol(&self) -> String;
    fn username(&self) -> String;
    fn password(&self) -> String;
    fn host(&self) -> String;
    fn hostname(&self) -> String;
    fn port(&self) -> String;
    fn pathname(&self) -> String;
    fn search(&self) -> String;
    fn hash(&self) -> String;
    fn origin(&self) -> String;
    fn has_valid_domain(&self) -> bool;

    fn set_href(&mut self, value: &str) -> bool;
    fn set_protocol(&mut self, value: &str) -> bool;
    fn set_username(&mut self, value: &str) -> bool;
    fn set_password(&mut self, value: &str) -> bool;
    fn set_host(&mut self, value: &str) -> bool;
    fn set_hostname(&mut self, value: &str) -> bool;
    fn set_port(&mut self, value: &str) -> bool;
    fn set_pathname(&mut self, value: &str) -> bool;
    fn set_search(&mut self, value: &str);
    fn set_hash(&mut self, value: &str);
}

macro_rules! impl_test_url {
    ($type:ty) => {
        impl TestUrl for $type {
            fn parse(input: &str) -> lucid_url::Result<Self> {
                <$type>::parse(input)
            }

            fn parse_with_base(input: &str, base: &Self) -> lucid_url::Result<Self> {
                <$type>::parse_with_base(input, base)
            }

            fn href(&self) -> String {
                self.get_href().to_string()
            }

            fn protocol(&self) -> String {
                self.get_protocol().to_string()
            }

            fn username(&self) -> String {
                self.get_username().to_string()
            }

            fn password(&self) -> String {
                self.get_password().to_string()
            }

            fn host(&self) -> String {
                self.get_host().to_string()
            }

            fn hostname(&self) -> String {
                self.get_hostname().to_string()
            }

            fn port(&self) -> String {
                self.get_port().to_string()
            }

            fn pathname(&self) -> String {
                self.get_pathname().to_string()
            }

            fn search(&self) -> String {
                self.get_search().to_string()
            }

            fn hash(&self) -> String {
                self.get_hash().to_string()
            }

            fn origin(&self) -> String {
                self.get_origin()
            }

            fn has_valid_domain(&self) -> bool {
                self.has_valid_domain()
            }

            fn set_href(&mut self, value: &str) -> bool {
                self.set_href(value)
            }

            fn set_protocol(&mut self, value: &str) -> bool {
                self.set_protocol(value)
            }

            fn set_username(&mut self, value: &str) -> bool {
                self.set_username(value)
            }

            fn set_password(&mut self, value: &str) -> bool {
                self.set_password(value)
            }

            fn set_host(&mut self, value: &str) -> bool {
                self.set_host(value)
            }

            fn set_hostname(&mut self, value: &str) -> bool {
                self.set_hostname(value)
            }

            fn set_port(&mut self, value: &str) -> bool {
                self.set_port(value)
            }

            fn set_pathname(&mut self, value: &str) -> bool {
                self.set_pathname(value)
            }

            fn set_search(&mut self, value: &str) {
                self.set_search(value);
            }

            fn set_hash(&mut self, value: &str) {
                self.set_hash(value);
            }
        }
    };
}

impl_test_url!(Url);
impl_test_url!(UrlAggregator);

fn fixture_root() -> PathBuf {
    std::env::var_os("ADA_TEST_ROOT").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("fixtures")
                .join("ada")
                .join("wpt")
        },
        PathBuf::from,
    )
}

fn fixture(name: &str) -> Value {
    let path = fixture_root().join(name);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("required Ada fixture {}: {error}", path.display()));
    serde_json::from_str(&sanitize_lone_surrogates(&raw))
        .unwrap_or_else(|error| panic!("parse Ada fixture {}: {error}", path.display()))
}

fn sanitize_lone_surrogates(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = String::with_capacity(input.len());
    let mut index = 0;
    let hex = |position: usize| -> Option<u32> {
        if position + 6 <= bytes.len() && bytes[position] == b'\\' && bytes[position + 1] == b'u' {
            std::str::from_utf8(&bytes[position + 2..position + 6])
                .ok()
                .and_then(|value| u32::from_str_radix(value, 16).ok())
        } else {
            None
        }
    };

    while index < bytes.len() {
        if let Some(code_point) = hex(index) {
            if (0xd800..0xdc00).contains(&code_point) {
                if matches!(hex(index + 6), Some(low) if (0xdc00..0xe000).contains(&low)) {
                    output.push_str(&input[index..index + 12]);
                    index += 12;
                    continue;
                }
                output.push_str("\\ufffd");
                index += 6;
                continue;
            }
            if (0xdc00..0xe000).contains(&code_point) {
                output.push_str("\\ufffd");
                index += 6;
                continue;
            }
        }
        let character = input[index..]
            .chars()
            .next()
            .expect("index remains on a UTF-8 boundary");
        output.push(character);
        index += character.len_utf8();
    }
    output
}

fn parse_case<T: TestUrl>(input: &str, base: Option<&str>) -> lucid_url::Result<T> {
    match base {
        Some(base) => {
            let base = T::parse(base)?;
            T::parse_with_base(input, &base)
        }
        None => T::parse(input),
    }
}

fn getter<T: TestUrl>(url: &T, name: &str) -> String {
    match name {
        "href" => url.href(),
        "protocol" => url.protocol(),
        "username" => url.username(),
        "password" => url.password(),
        "host" => url.host(),
        "hostname" => url.hostname(),
        "port" => url.port(),
        "pathname" => url.pathname(),
        "search" => url.search(),
        "hash" => url.hash(),
        "origin" => url.origin(),
        _ => panic!("unsupported fixture getter {name}"),
    }
}

fn apply_setter<T: TestUrl>(url: &mut T, name: &str, value: &str) {
    match name {
        "href" => {
            url.set_href(value);
        }
        "protocol" => {
            url.set_protocol(value);
        }
        "username" => {
            url.set_username(value);
        }
        "password" => {
            url.set_password(value);
        }
        "host" => {
            url.set_host(value);
        }
        "hostname" => {
            url.set_hostname(value);
        }
        "port" => {
            url.set_port(value);
        }
        "pathname" => {
            url.set_pathname(value);
        }
        "search" => url.set_search(value),
        "hash" => url.set_hash(value),
        _ => panic!("unsupported fixture setter {name}"),
    }
}

fn run_url_cases<T: TestUrl>(fixture_name: &str) {
    let cases = fixture(fixture_name);
    for (index, case) in cases.as_array().unwrap().iter().enumerate() {
        let Some(case) = case.as_object() else {
            continue;
        };
        let input = case["input"].as_str().unwrap();
        let base = case.get("base").and_then(Value::as_str);
        let expected_failure = case
            .get("failure")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let parsed = parse_case::<T>(input, base);
        assert_eq!(
            parsed.is_err(),
            expected_failure,
            "{fixture_name} case {index}: input={input:?} base={base:?}"
        );
        assert_eq!(
            can_parse(input, base),
            !expected_failure,
            "{fixture_name} can_parse case {index}: input={input:?} base={base:?}"
        );
        let Ok(url) = parsed else {
            continue;
        };
        for field in [
            "href", "protocol", "username", "password", "host", "hostname", "port", "pathname",
            "search", "hash", "origin",
        ] {
            if let Some(expected) = case.get(field).and_then(Value::as_str) {
                assert_eq!(
                    getter(&url, field),
                    expected,
                    "{fixture_name} case {index} field {field}: input={input:?} base={base:?}"
                );
            }
        }
    }
}

fn run_setter_cases<T: TestUrl>(fixture_name: &str) {
    let cases = fixture(fixture_name);
    for (setter, cases) in cases.as_object().unwrap() {
        if setter == "comment" {
            continue;
        }
        for (index, case) in cases.as_array().unwrap().iter().enumerate() {
            let href = case["href"].as_str().unwrap();
            let new_value = case["new_value"].as_str().unwrap();
            let mut url = T::parse(href)
                .unwrap_or_else(|error| panic!("{fixture_name} {setter} case {index}: {error}"));
            apply_setter(&mut url, setter, new_value);
            for (field, expected) in case["expected"].as_object().unwrap() {
                let Some(expected) = expected.as_str() else {
                    continue;
                };
                assert_eq!(
                    getter(&url, field),
                    expected,
                    "{fixture_name} {setter} case {index} field {field}: href={href:?}, value={new_value:?}"
                );
            }
        }
    }
}

fn run_to_ascii_cases<T: TestUrl>(fixture_name: &str, assert_rejections: bool) {
    let cases = fixture(fixture_name);
    for (index, case) in cases.as_array().unwrap().iter().enumerate() {
        let Some(case) = case.as_object() else {
            continue;
        };
        let input = case["input"].as_str().unwrap();
        let expected = case.get("output").and_then(Value::as_str);
        let parsed = T::parse(&format!("https://{input}/x"));
        match expected {
            Some(expected) if !expected.is_empty() => {
                let url = parsed.unwrap_or_else(|error| {
                    panic!("{fixture_name} case {index}: input={input:?}: {error}")
                });
                assert_eq!(
                    url.hostname(),
                    expected,
                    "{fixture_name} case {index}: input={input:?}"
                );
                assert_eq!(url.pathname(), "/x");

                let mut host_setter = T::parse("https://x/x").unwrap();
                assert!(host_setter.set_host(input));
                assert_eq!(host_setter.hostname(), expected);

                let mut hostname_setter = T::parse("https://x/x").unwrap();
                assert!(hostname_setter.set_hostname(input));
                assert_eq!(hostname_setter.hostname(), expected);
            }
            _ if assert_rejections => assert!(
                parsed.is_err(),
                "{fixture_name} case {index} should reject input={input:?}"
            ),
            _ => {}
        }
    }
}

fn run_idna_cases<T: TestUrl>() {
    run_to_ascii_cases::<T>("IdnaTestV2.json", true);
}

fn run_percent_encoding_cases<T: TestUrl>() {
    let cases = fixture("percent-encoding.json");
    for (index, case) in cases.as_array().unwrap().iter().enumerate() {
        let Some(case) = case.as_object() else {
            continue;
        };
        let input = case["input"].as_str().unwrap();
        let expected = case["output"]["utf-8"].as_str().unwrap();
        let url = T::parse(&format!("https://example/?{input}A")).unwrap();
        assert_eq!(
            url.search(),
            format!("?{expected}A"),
            "percent-encoding.json case {index}: input={input:?}"
        );

        let mut setter = T::parse("https://example/").unwrap();
        setter.set_search(&format!("{input}A"));
        assert_eq!(setter.search(), format!("?{expected}A"));
    }
}

fn run_dns_length_cases<T: TestUrl>() {
    let cases = fixture("verifydnslength_tests.json");
    for (index, case) in cases.as_array().unwrap().iter().enumerate() {
        let Some(case) = case.as_object() else {
            continue;
        };
        let input = case["input"].as_str().unwrap();
        let failure = case["failure"].as_bool().unwrap();
        let url = T::parse(input).unwrap_or_else(|error| {
            panic!("verifydnslength_tests.json case {index}: input={input:?}: {error}")
        });
        assert_eq!(
            url.has_valid_domain(),
            !failure,
            "verifydnslength_tests.json case {index}: input={input:?}"
        );
    }
}

fn run_all<T: TestUrl>() {
    run_url_cases::<T>("urltestdata.json");
    run_url_cases::<T>("ada_extra_urltestdata.json");
    run_setter_cases::<T>("setters_tests.json");
    run_setter_cases::<T>("ada_extra_setters_tests.json");
    run_idna_cases::<T>();
    // Ada's test validates successful ToASCII results through the URL parser
    // and setters. Rejected ToASCII inputs are covered by its direct Unicode
    // API, which this crate does not expose publicly.
    run_to_ascii_cases::<T>("toascii.json", false);
    run_percent_encoding_cases::<T>();
    run_dns_length_cases::<T>();
}

#[test]
fn ada_url_conformance() {
    run_all::<Url>();
}

#[test]
fn ada_url_aggregator_conformance() {
    run_all::<UrlAggregator>();
}
