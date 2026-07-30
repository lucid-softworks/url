use std::fs;
use std::hint::black_box;
use std::time::{Duration, Instant};

use lucid_url::{Url, UrlAggregator, can_parse, parse};

const TOP_SITES: &[&str] = &[
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
            for input in urls {
                checksum = checksum.wrapping_add(black_box(operation(black_box(input))));
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

fn run(name: &str, urls: &[&str]) {
    println!("\n{name} ({} URLs)", urls.len());
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
        println!("lucid UrlAggregator    {nanoseconds:10.2}  {rate:12.0}");
    }
    if operation.as_deref().is_none_or(|value| value == "url") {
        let (nanoseconds, rate, checksum) = measure(urls, |input| {
            parse::<Url>(input, None).map_or(0, |url| black_box(url.get_href()).len())
        });
        black_box(checksum);
        println!("lucid Url              {nanoseconds:10.2}  {rate:12.0}");
    }
    if operation
        .as_deref()
        .is_none_or(|value| value == "can_parse")
    {
        let (nanoseconds, rate, checksum) =
            measure(urls, |input| usize::from(can_parse(input, None)));
        black_box(checksum);
        println!("lucid can_parse        {nanoseconds:10.2}  {rate:12.0}");
    }
}

fn main() {
    if std::env::var_os("LUCID_URL_BENCH_SKIP_TOP_SITES").is_none() {
        run("top sites", TOP_SITES);
    }

    let dataset_path = std::env::args()
        .skip(1)
        .find(|argument| argument != "--bench")
        .unwrap_or_else(|| "target/ada-url-dataset/out.txt".to_owned());
    let Ok(dataset) = fs::read_to_string(&dataset_path) else {
        eprintln!(
            "\nbenchdata skipped: {dataset_path} is unavailable; run benchmarks/compare-ada.sh to fetch the pinned dataset"
        );
        return;
    };
    let urls: Vec<_> = dataset
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    run("benchdata", &urls);
}
