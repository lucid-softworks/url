use std::fs;
use std::hint::black_box;
use std::time::{Duration, Instant};

use lucid_url::{Url, UrlAggregator, can_parse, parse};

/// Mixed realistic inputs for parse+href. Includes IPv4, IPv6, and a non-special
/// scheme so averages are not limited to the clean-HTTP fast path.
const MIXED_TOP_SITES: &[&str] = &[
    "https://www.google.com/webhp?hl=en&amp;ictx=2&amp;sa=X&amp;ved=0ahUKEwil_oSxzJj8AhVtEFkFHTHnCGQQPQgI",
    "https://support.google.com/websearch/?p=ws_results_help&amp;hl=en-CA&amp;fg=1",
    "https://en.wikipedia.org/wiki/Dog#Roles_with_humans",
    "https://www.tiktok.com/@aguyandagolden/video/7133277734310038830",
    "https://business.twitter.com/en/help/troubleshooting/how-twitter-ads-work.html?ref=web-twc-ao-gbl-adsinfo&utm_source=twc&utm_medium=web&utm_campaign=ao&utm_content=adsinfo",
    "https://images-na.ssl-images-amazon.com/images/I/41Gc3C8UysL.css?AUIClients/AmazonGatewayAuiAssets",
    "https://www.reddit.com/?after=t3_zvz1ze",
    "https://www.reddit.com/login/?dest=https%3A%2F%2Fwww.reddit.com%2F",
    "postgresql://other:9818274x1!!@localhost:5432/otherdb?connect_timeout=10&application_name=myapp",
    "http://192.168.1.1",
    "http://[2606:4700:4700::1111]",
];

/// Already-canonical special URLs that both libraries' `can_parse` fast paths
/// target. Used so a handful of slow-path URLs cannot dominate a tiny mean.
const CLEAN_HTTP: &[&str] = &[
    "https://www.google.com/",
    "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
    "https://www.facebook.com/",
    "https://www.amazon.com/dp/B08N5WRWNW",
    "https://en.wikipedia.org/wiki/URL",
    "https://www.reddit.com/r/rust/",
    "https://github.com/ada-url/ada",
    "https://stackoverflow.com/questions/tagged/url",
    "https://www.nytimes.com/",
    "https://www.bbc.com/news",
    "https://www.apple.com/iphone/",
    "https://developer.mozilla.org/en-US/docs/Web/API/URL",
    "https://crates.io/crates/url",
    "https://docs.rs/url/latest/url/",
    "http://example.com/path?query=1#frag",
    "https://cdn.example.com/static/app.js",
    "https://api.example.com/v1/users/42",
    "https://subdomain.example.co.uk/products/12345",
    "https://news.ycombinator.com/item?id=1",
    "https://www.linkedin.com/in/example/",
    "https://twitter.com/yagiznizipli",
    "https://www.instagram.com/",
    "https://www.netflix.com/browse",
    "https://www.microsoft.com/en-us/",
];

fn sample_duration() -> Duration {
    std::env::var("LUCID_URL_BENCH_SAMPLE_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .map_or(Duration::from_millis(300), Duration::from_millis)
}

fn sample(urls: &[&str], operation: &mut impl FnMut(&str) -> usize) -> (f64, usize) {
    let minimum_duration = sample_duration();

    // Warm one pass so timed samples are not dominated by cold I-cache.
    {
        let mut checksum = 0usize;
        for _ in 0..64 {
            for input in urls {
                checksum = checksum.wrapping_add(black_box(operation(black_box(*input))));
            }
        }
        black_box(checksum);
    }

    let mut iterations = 1usize;
    loop {
        let start = Instant::now();
        let mut checksum = 0usize;
        for _ in 0..iterations {
            for input in urls {
                checksum = checksum.wrapping_add(black_box(operation(black_box(*input))));
            }
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

fn print_row(label: &str, nanoseconds: f64, rate: f64) {
    println!("lucid {label:<18} {nanoseconds:10.2}  {rate:12.0}");
}

/// Print the corpus under test, Ada-style (`# Loading …`, `# urls=…`, samples).
fn print_dataset(name: &str, urls: &[&str], source: &str) {
    let bytes: usize = urls.iter().map(|url| url.len()).sum();
    println!("\n# {name}");
    println!("# source: {source}");
    if source.starts_with("file:") {
        if let Ok(commit) = std::env::var("ADA_DATASET_COMMIT") {
            println!("# dataset commit: {commit}");
        }
    }
    println!("# urls={} bytes={}", urls.len(), bytes);
    let max_print = std::env::var("LUCID_URL_BENCH_DATASET_PRINT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(if urls.len() <= 64 { urls.len() } else { 8 });
    for (index, url) in urls.iter().enumerate().take(max_print) {
        println!("#   [{index}] {url}");
    }
    if urls.len() > max_print {
        println!("#   ... {} more", urls.len() - max_print);
    }
}

fn run_parse(name: &str, urls: &[&str], source: &str) {
    print_dataset(name, urls, source);
    println!("implementation             ns/url        URLs/s");
    let operation = std::env::var("LUCID_URL_BENCH_ONLY").ok();
    if operation
        .as_deref()
        .is_none_or(|value| value == "aggregate")
    {
        let (nanoseconds, rate, checksum) = measure(urls, |input| {
            parse::<UrlAggregator>(input, None).map_or(0, |url| black_box(url.get_href()).len())
        });
        black_box(checksum);
        print_row("UrlAggregator", nanoseconds, rate);
    }
    if operation.as_deref().is_none_or(|value| value == "url") {
        let (nanoseconds, rate, checksum) = measure(urls, |input| {
            parse::<Url>(input, None).map_or(0, |url| black_box(url.get_href()).len())
        });
        black_box(checksum);
        print_row("Url", nanoseconds, rate);
    }
}

fn run_can_parse(name: &str, urls: &[&str], source: &str) {
    let operation = std::env::var("LUCID_URL_BENCH_ONLY").ok();
    if operation
        .as_deref()
        .is_some_and(|value| value != "can_parse")
    {
        return;
    }
    print_dataset(name, urls, source);
    println!("implementation             ns/url        URLs/s");
    let (nanoseconds, rate, checksum) = measure(urls, |input| usize::from(can_parse(input, None)));
    black_box(checksum);
    print_row("can_parse", nanoseconds, rate);
}

fn run_all(name: &str, urls: &[&str], source: &str) {
    let operation = std::env::var("LUCID_URL_BENCH_ONLY").ok();
    let want_parse = operation
        .as_deref()
        .is_none_or(|value| value == "aggregate" || value == "url");
    let want_can_parse = operation
        .as_deref()
        .is_none_or(|value| value == "can_parse");

    if want_parse {
        run_parse(name, urls, source);
    }
    if want_can_parse {
        if want_parse {
            let (nanoseconds, rate, checksum) =
                measure(urls, |input| usize::from(can_parse(input, None)));
            black_box(checksum);
            print_row("can_parse", nanoseconds, rate);
        } else {
            run_can_parse(name, urls, source);
        }
    }
}

fn main() {
    println!(
        "note: mixed top sites measure parse+href only (includes IPv4/IPv6/non-special).\n\
         note: clean HTTP measures can_parse on already-canonical special URLs.\n\
         note: benchdata reports parse+href and can_parse on the full corpus.\n\
         note: each section prints the dataset (urls/bytes/source); set\n\
               LUCID_URL_BENCH_DATASET_PRINT=N to cap listed URLs."
    );

    if std::env::var_os("LUCID_URL_BENCH_SKIP_TOP_SITES").is_none() {
        run_parse(
            "mixed top sites parse",
            MIXED_TOP_SITES,
            "inline (ada-url/ada bench.cpp url_examples_default)",
        );
        run_can_parse(
            "clean HTTP can_parse",
            CLEAN_HTTP,
            "inline clean-HTTP can_parse corpus",
        );
    }

    let dataset_path = std::env::args()
        .skip(1)
        .find(|argument| argument != "--bench")
        .unwrap_or_else(|| "target/ada-url-dataset/out.txt".to_owned());
    println!("\n# Loading {dataset_path}");
    let Ok(dataset) = fs::read_to_string(&dataset_path) else {
        eprintln!(
            "\nbenchdata skipped: {dataset_path} is unavailable; run benchmarks/compare-ada.sh to fetch the pinned dataset"
        );
        return;
    };
    let owned: Vec<String> = dataset
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    let urls: Vec<&str> = owned.iter().map(String::as_str).collect();
    let source = format!("file:{dataset_path} (ada-url/url-dataset out.txt)");
    run_all("benchdata", &urls, &source);
}
