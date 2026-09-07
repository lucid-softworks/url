//! Public parser regressions adapted from Ada's basic_tests.cpp at the pinned
//! fixture revision. Ada's internal SIMD/helper tests are not Rust API tests.
use lucid_url::{Url, UrlAggregator, can_parse};

#[test]
fn normalized_hosts_and_fast_path_boundaries() {
    for (input, expected) in [
        ("http://%C3%A1%CC%A3/", "http://xn--lsa752l/"),
        (
            "https://%C5%9A%CC%A7.example/",
            "https://xn--nga05f.example/",
        ),
        ("http://@19%2E68.1.10.", "http://19.68.1.10/"),
        ("http://19%2E68.1.10./x", "http://19.68.1.10/x"),
        ("http://%31%2e%32%2e%33%2e%34/", "http://1.2.3.4/"),
        ("http://0xffffffff.", "http://255.255.255.255/"),
        ("http://12.34.56.78/", "http://12.34.56.78/"),
        ("http://[1::2:3:4:5:6:7]/", "http://[1:0:2:3:4:5:6:7]/"),
        ("http://[::abcd]/", "http://[::abcd]/"),
        ("http://[AB::CD]/", "http://[ab::cd]/"),
        (
            "http://[ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff]/",
            "http://[ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff]/",
        ),
        ("https://ex.com/FooBar", "https://ex.com/FooBar"),
        (
            "https://example.com/xn--path",
            "https://example.com/xn--path",
        ),
        ("https://example.com:0080/x", "https://example.com:80/x"),
        ("http://example.com:0080/x", "http://example.com/x"),
        (
            "https://example.com/a%20b/c%2Fd",
            "https://example.com/a%20b/c%2Fd",
        ),
        ("wss://ab?x\n9", "wss://ab/?x9"),
        ("http://ab?a\tb", "http://ab/?ab"),
        ("wss://ab?x9 ", "wss://ab/?x9"),
        ("http://ab#f\ng", "http://ab/#fg"),
        ("http://ab#f ", "http://ab/#f"),
        ("http://ab/p ", "http://ab/p"),
        ("http://ab:81?x\n9", "http://ab:81/?x9"),
        ("http://ab:81/p?x\ty#h", "http://ab:81/p?xy#h"),
        ("http://ab:81#f ", "http://ab:81/#f"),
        ("http://ab?x y", "http://ab/?x%20y"),
    ] {
        assert_eq!(Url::parse(input).unwrap().get_href(), expected, "{input}");
        let url = UrlAggregator::parse(input).unwrap();
        assert_eq!(url.get_href(), expected, "{input}");
        assert!(url.validate(), "{input}");
        assert!(can_parse(input, None), "{input}");
    }
    for input in [
        "http://1234.5.6.7/",
        "http://1.2345.6.7/",
        "http://1..2.34/",
        "http://12.3..4/",
        "http://[1::2:3:4:5:6:7:8]/",
        "http://[::abcde]/",
        "http://[:1::2]/",
        "http://[1::2:]/",
        "http://[1::2::3]/",
        "http://[fffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff]/",
        "https://foo_^bar.com/",
        "http://example.com:65536/",
        "http://1.2.3.999/",
        "http://foo.0xffffffff/",
        "http://foo.0xfffffffff/",
        "http://0xfffffffff/",
        "http://example.0x/",
        "http://foo.0x1/",
    ] {
        assert!(Url::parse(input).is_err(), "{input}");
        assert!(UrlAggregator::parse(input).is_err(), "{input}");
        assert!(!can_parse(input, None), "{input}");
    }
}

macro_rules! setter_regressions {
    ($name:ident, $url:ty) => {
        #[test]
        fn $name() {
            let mut url =
                <$url>::parse("https://initial:secret@example.com/path?before=yes#fragment")
                    .unwrap();
            for (user, password) in [
                ("changed", "planet"),
                ("x", "y"),
                ("a-much-longer-username", "a-much-longer-password"),
            ] {
                assert!(url.set_username(user));
                assert!(url.set_password(password));
                assert_eq!(
                    url.get_href(),
                    format!("https://{user}:{password}@example.com/path?before=yes#fragment")
                );
            }
            let mut url = <$url>::parse("https://example.com/path?q=1#fragment").unwrap();
            assert!(url.set_username("a b"));
            assert!(url.set_password("p@ss"));
            assert_eq!(
                url.get_href(),
                "https://a%20b:p%40ss@example.com/path?q=1#fragment"
            );
            assert!(url.set_username(""));
            assert!(url.set_password(""));
            for (query, expected) in [
                ("?same=value", "?same=value"),
                ("?a=1", "?a=1"),
                ("?longer query=value", "?longer%20query=value"),
                ("?value='x y'", "?value=%27x%20y%27"),
                ("", ""),
            ] {
                url.set_search(query);
                assert_eq!(
                    url.get_href(),
                    format!("https://example.com/path{expected}#fragment")
                );
            }
        }
    };
}
setter_regressions!(url_setters_preserve_tail, Url);
setter_regressions!(aggregator_setters_preserve_tail, UrlAggregator);
