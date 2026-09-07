use lucid_url::{
    Url, UrlAggregator, can_parse, get_max_input_length, href_from_file, set_max_input_length,
};

const SMALL_LIMIT: u32 = 1024;

trait LimitedUrl: Sized {
    fn parse(input: &str) -> lucid_url::Result<Self>;
    fn parse_with_base(input: &str, base: &Self) -> lucid_url::Result<Self>;
    fn href(&self) -> String;
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

macro_rules! impl_limited_url {
    ($type:ty) => {
        impl LimitedUrl for $type {
            fn parse(input: &str) -> lucid_url::Result<Self> {
                <$type>::parse(input)
            }

            fn parse_with_base(input: &str, base: &Self) -> lucid_url::Result<Self> {
                <$type>::parse_with_base(input, base)
            }

            fn href(&self) -> String {
                self.get_href().to_string()
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

impl_limited_url!(Url);
impl_limited_url!(UrlAggregator);

struct RestoreLimit(u32);

impl Drop for RestoreLimit {
    fn drop(&mut self) {
        set_max_input_length(self.0);
    }
}

fn run_limited_url_cases<T: LimitedUrl>() {
    assert_eq!(get_max_input_length(), SMALL_LIMIT);

    let long_url = format!("https://example.com/{}", "a".repeat(SMALL_LIMIT as usize));
    assert!(T::parse(&long_url).is_err());
    assert!(T::parse("https://example.com/ok").is_ok());

    let mut url = T::parse("https://example.com/").unwrap();
    let original = url.href();
    let over_limit = "x".repeat(SMALL_LIMIT as usize + 1);
    assert!(!url.set_href(&long_url));
    assert!(!url.set_host(&over_limit));
    assert!(!url.set_hostname(&over_limit));
    assert!(!url.set_username(&over_limit));
    assert!(!url.set_password(&over_limit));
    assert!(!url.set_port(&"1".repeat(SMALL_LIMIT as usize + 1)));
    assert!(!url.set_pathname(&format!("/{over_limit}")));
    url.set_search(&over_limit);
    url.set_hash(&over_limit);
    assert_eq!(url.href(), original);

    let mut accepted = T::parse("https://example.com/").unwrap();
    assert!(accepted.set_username("user"));
    assert!(accepted.set_password("pass"));
    assert!(accepted.set_pathname("/path"));
    assert!(accepted.set_port("8080"));
    accepted.set_search("?q=1");
    accepted.set_hash("#frag");
    assert_eq!(
        accepted.href(),
        "https://user:pass@example.com:8080/path?q=1#frag"
    );

    let spaces = " ".repeat(340);
    let mut expansion = T::parse("https://example.com/").unwrap();
    let original = expansion.href();
    assert!(!expansion.set_pathname(&spaces));
    assert!(!expansion.set_username(&spaces));
    assert!(!expansion.set_password(&spaces));
    expansion.set_search(&spaces);
    expansion.set_hash(&spaces);
    assert_eq!(expansion.href(), original);

    let exceeds_after_normalization = format!("http://x/{}y", " ".repeat(339));
    assert!(T::parse(&exceeds_after_normalization).is_err());
    assert!(!can_parse(&exceeds_after_normalization, None));

    for prefix in ["file://x/?", "http://u@x#"] {
        let input = format!("{prefix}{}y", " ".repeat(339));
        assert!(input.len() <= SMALL_LIMIT as usize);
        assert!(T::parse(&input).is_err());
        assert!(!can_parse(&input, None));
    }

    let under_after_normalization = format!("http://x/{}y", " ".repeat(337));
    let parsed = T::parse(&under_after_normalization).unwrap();
    assert!(parsed.href().len() <= SMALL_LIMIT as usize);

    let idna_host = std::iter::repeat_n("\u{337f}", 60)
        .collect::<Vec<_>>()
        .join(".");
    let idna_input = format!("ws://{idna_host}/");
    assert!(T::parse(&idna_input).is_err());
    assert!(!can_parse(&idna_input, None));

    let relative = format!("a{}y", " ".repeat(339));
    let base = T::parse("http://x/").unwrap();
    assert!(T::parse_with_base(&relative, &base).is_err());
    assert!(!can_parse(&relative, Some("http://x/")));

    let mut non_special = T::parse("foo://example.com/path").unwrap();
    let original = non_special.href();
    assert!(!non_special.set_protocol(&"z".repeat(SMALL_LIMIT as usize)));
    assert_eq!(non_special.href(), original);
}

fn href_from_file_long_way<T: LimitedUrl>(path: &str) -> String {
    let mut url = T::parse("file://").unwrap();
    url.set_pathname(path);
    url.href()
}

#[test]
fn ada_max_input_length_compatibility() {
    let _restore = RestoreLimit(get_max_input_length());
    set_max_input_length(SMALL_LIMIT);

    run_limited_url_cases::<Url>();
    run_limited_url_cases::<UrlAggregator>();

    assert!(href_from_file(&"a".repeat(SMALL_LIMIT as usize + 1)).is_empty());

    let expanded = format!("{}y", " ".repeat(339));
    assert!(href_from_file(&expanded).is_empty());
    let short = format!("{}y", " ".repeat(100));
    let short_href = href_from_file(&short);
    assert!(short_href.starts_with("file://"));
    assert!(short_href.len() <= SMALL_LIMIT as usize);

    assert_eq!(
        href_from_file("/home/user/file.txt"),
        "file:///home/user/file.txt"
    );

    for path in [
        "",
        "fsfds",
        r"C:\\blabala\fdfds\back.txt",
        "/home/user/txt.txt",
        "/%2e.bar",
        "/foo/%2e%2",
        "/foo/..bar",
        "foo\t%91",
    ] {
        let expected = href_from_file(path);
        assert_eq!(href_from_file_long_way::<Url>(path), expected);
        assert_eq!(href_from_file_long_way::<UrlAggregator>(path), expected);
    }
}
