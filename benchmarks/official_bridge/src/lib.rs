use std::fs;
use std::slice;
use std::str;
use std::sync::OnceLock;

use lucid_url::{Url, UrlAggregator, can_parse, parse};

static URLS: OnceLock<Vec<String>> = OnceLock::new();

fn urls() -> &'static [String] {
    URLS.get().map_or(&[], Vec::as_slice)
}

fn volatile_add(target: &mut usize, value: usize) {
    // Match the volatile checksum updates in Ada's official benchmark.
    unsafe {
        let current = std::ptr::read_volatile(target);
        std::ptr::write_volatile(target, current.wrapping_add(value));
    }
}

/// Load the URL corpus used by the C++ Google Benchmark runner.
///
/// # Safety
///
/// `path` must point to `length` readable bytes containing a UTF-8 filesystem
/// path for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lucid_bench_initialize(path: *const u8, length: usize) -> usize {
    if path.is_null() {
        return 0;
    }
    let path = unsafe { slice::from_raw_parts(path, length) };
    let Ok(path) = str::from_utf8(path) else {
        return 0;
    };
    let Ok(dataset) = fs::read_to_string(path) else {
        return 0;
    };
    let parsed = dataset
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    let _ = URLS.set(parsed);
    urls().len()
}

#[unsafe(no_mangle)]
pub extern "C" fn lucid_bench_url() -> usize {
    let mut success = 0usize;
    let mut href_size = 0usize;
    for input in urls() {
        if let Ok(url) = parse::<Url>(input, None) {
            volatile_add(&mut success, 1);
            let href = std::hint::black_box(url.get_href());
            volatile_add(&mut href_size, href.len());
        }
    }
    success.wrapping_add(href_size)
}

#[unsafe(no_mangle)]
pub extern "C" fn lucid_bench_url_aggregator() -> usize {
    let mut success = 0usize;
    let mut href_size = 0usize;
    for input in urls() {
        if let Ok(url) = parse::<UrlAggregator>(input, None) {
            volatile_add(&mut success, 1);
            volatile_add(&mut href_size, url.get_href().len());
        }
    }
    success.wrapping_add(href_size)
}

#[unsafe(no_mangle)]
pub extern "C" fn lucid_bench_can_parse() -> usize {
    let mut success = 0usize;
    for input in urls() {
        if can_parse(input, None) {
            volatile_add(&mut success, 1);
        }
    }
    success
}

#[unsafe(no_mangle)]
pub extern "C" fn lucid_bench_count_invalid() -> usize {
    urls()
        .iter()
        .filter(|input| parse::<UrlAggregator>(input, None).is_err())
        .count()
}

#[unsafe(no_mangle)]
pub extern "C" fn lucid_bench_is_valid(index: usize) -> bool {
    urls()
        .get(index)
        .is_some_and(|input| parse::<UrlAggregator>(input, None).is_ok())
}
