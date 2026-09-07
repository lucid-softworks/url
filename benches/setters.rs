use lucid_url::{Url, UrlAggregator};
use std::hint::black_box;
use std::time::{Duration, Instant};

fn measure(mut operation: impl FnMut(usize) -> usize) -> f64 {
    let duration = Duration::from_millis(200);
    let mut samples = [0.0; 5];
    for sample in &mut samples {
        let mut iterations = 1024;
        loop {
            let start = Instant::now();
            for i in 0..iterations {
                black_box(operation(black_box(i)));
            }
            let elapsed = start.elapsed();
            if elapsed >= duration {
                *sample = elapsed.as_nanos() as f64 / iterations as f64;
                break;
            }
            iterations *= 2;
        }
    }
    samples.sort_by(f64::total_cmp);
    samples[2]
}

macro_rules! run {
    ($url:ty) => {
        for (name, values, query) in [
            ("ASCII query", ["?a=1", "?longer=value&another=two"], true),
            (
                "Unicode query",
                ["?search=日本語&x='y'", "?search=hello world&x=😀"],
                true,
            ),
            ("fragment", ["#a", "#longer fragment 日本語"], false),
        ] {
            let mut url =
                <$url>::parse("https://user:pass@example.com/a/long/path?q=initial#fragment")
                    .unwrap();
            let ns = measure(|i| {
                let value = values[i % values.len()];
                if query {
                    url.set_search(value);
                } else {
                    url.set_hash(value);
                }
                black_box(&url);
                black_box(url.get_href_size())
            });
            println!("{} {name}: {ns:.2} ns/setter", stringify!($url));
        }
    };
}

fn main() {
    run!(UrlAggregator);
    run!(Url);
}
