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

fn sample(urls: &[&str], operation: &mut impl FnMut(&str) -> usize) -> (f64, usize) {
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
        if elapsed >= Duration::from_millis(300) {
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
        parse::<UrlAggregator>(input, None).unwrap().get_href_size()
    });
    let (url_ns, url_rate, url_sum) = measure(urls, |input| {
        parse::<Url>(input, None).unwrap().get_href_size()
    });

    black_box((aggregate_sum, url_sum));
    println!("\n{name}");
    println!("implementation             ns/url        URLs/s");
    println!("lucid UrlAggregator    {aggregate_ns:10.2}  {aggregate_rate:12.0}");
    println!("lucid Url              {url_ns:10.2}  {url_rate:12.0}");
}

fn main() {
    run("canonical ASCII", CANONICAL);
    run("normalization-heavy", NORMALIZATION_HEAVY);
}
