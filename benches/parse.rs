use std::hint::black_box;
use std::time::{Duration, Instant};

use lucid_url::{Url, UrlAggregator, parse};

const CANONICAL: &[&str] = &[
    "https://example.com/",
    "https://user:password@example.com:8080/path/to/resource?query=value#fragment",
    "http://www.example.org/a/b/c?one=1&two=2",
    "https://subdomain.example.co.uk/products/12345",
    "ftp://ftp.example.com/pub/file.txt",
    "ws://localhost:3000/socket",
    "https://127.0.0.1:8443/api/v1/health",
    "https://example.com/a%20path?q=already%20encoded",
];

const NORMALIZATION_HEAVY: &[&str] = &[
    "HTTPS://EXAMPLE.COM/a/../b?x=hello world#frag ment",
    "http://bücher.example/straße",
    "http://[2001:db8::1]:8080/a/./b",
    "file:///C|/Program Files/test.txt",
    "mailto:User@Example.com",
    "http:\\\\example.com\\a\\b",
    "http://0x7f.1/",
    "https://user name:pass word@example.com/",
];

const UNICODE_IDNA: &[&str] = &[
    "https://bücher.example/straße",
    "https://mañana.example/café",
    "https://例え.テスト/パス",
    "https://παράδειγμα.δοκιμή/",
    "https://مثال.إختبار/",
    "https://उदाहरण.भारत/",
    "https://한국어.example/",
    "https://cafe\u{301}.example/résumé",
    "https://ＥＸＡＭＰＬＥ.com/",
];

const LONG_SCANS: &[&str] = &[
    "https://assets.example.com/packages/catalogue/components/react/dialog-manager/examples/controlled-dialog/source/index.tsx?framework=react&bundler=vite&render=client&theme=system#interactive-example",
    "https://api.example.com/v1/organizations/lucid-softworks/repositories/url/commits/306db12a15e0d5ed3428934622a187b720ae5741/check-runs?filter=latest&per_page=100",
    "https://cdn.example.com/assets/0123456789abcdefghijklmnopqrstuvwxyz/0123456789abcdefghijklmnopqrstuvwxyz/0123456789abcdefghijklmnopqrstuvwxyz/module.min.js?cache=0123456789abcdefghijklmnopqrstuvwxyz",
];

fn sample(urls: &[&str], operation: &mut impl FnMut(&str) -> usize) -> (f64, usize) {
    let minimum_duration = std::env::var("LUCID_URL_BENCH_SAMPLE_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .map_or(Duration::from_millis(300), Duration::from_millis);
    let mut iterations = 1usize;
    loop {
        let start = Instant::now();
        let mut checksum = 0usize;
        for _ in 0..iterations {
            checksum ^= urls
                .iter()
                .map(|input| black_box(operation(black_box(input))))
                .sum::<usize>();
        }
        let elapsed = start.elapsed();
        if elapsed >= minimum_duration {
            let count = iterations * urls.len();
            return (elapsed.as_nanos() as f64 / count as f64, checksum);
        }
        iterations = iterations.saturating_mul(2);
    }
}

fn measure(urls: &[&str], mut operation: impl FnMut(&str) -> usize) -> (f64, f64, usize) {
    let mut samples = [0.0; 5];
    let mut checksum = 0usize;
    for result in &mut samples {
        let (nanoseconds, sample_checksum) = sample(urls, &mut operation);
        *result = nanoseconds;
        checksum ^= sample_checksum;
    }
    samples.sort_by(f64::total_cmp);
    let median = samples[samples.len() / 2];
    (median, 1_000_000_000.0 / median, checksum)
}

fn run(name: &str, urls: &[&str]) {
    let (aggregate_ns, aggregate_rate, aggregate_sum) = measure(urls, |input| {
        let url = parse::<UrlAggregator>(input, None).unwrap();
        black_box(url.get_href()).len()
    });
    let (url_ns, url_rate, url_sum) = measure(urls, |input| {
        let url = parse::<Url>(input, None).unwrap();
        black_box(url.get_href()).len()
    });

    black_box((aggregate_sum, url_sum));
    println!("\n{name}");
    println!("implementation             ns/url        URLs/s");
    println!("lucid UrlAggregator    {aggregate_ns:10.2}  {aggregate_rate:12.0}");
    println!("lucid Url              {url_ns:10.2}  {url_rate:12.0}");
}

fn main() {
    if let Some(index) = std::env::var("LUCID_URL_BENCH_UNICODE_INDEX")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
    {
        let input = UNICODE_IDNA[index];
        run(input, std::slice::from_ref(&input));
        return;
    }
    if let Some(index) = std::env::var("LUCID_URL_BENCH_LONG_INDEX")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
    {
        let input = LONG_SCANS[index];
        run(input, std::slice::from_ref(&input));
        return;
    }
    if let Some(index) = std::env::var("LUCID_URL_BENCH_CANONICAL_INDEX")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
    {
        let input = CANONICAL[index];
        run(input, std::slice::from_ref(&input));
        return;
    }
    if let Some(index) = std::env::var("LUCID_URL_BENCH_INDEX")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
    {
        let input = NORMALIZATION_HEAVY[index];
        run(input, std::slice::from_ref(&input));
        return;
    }
    run("canonical ASCII", CANONICAL);
    run("normalization-heavy", NORMALIZATION_HEAVY);
    run("Unicode and IDNA", UNICODE_IDNA);
    run("long canonical scans", LONG_SCANS);
    if std::env::var_os("LUCID_URL_BENCH_INDIVIDUAL").is_some() {
        for input in NORMALIZATION_HEAVY {
            run(input, std::slice::from_ref(input));
        }
    }
}
