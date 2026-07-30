//! From-scratch IDNA (UTS-46) domain-to-ASCII, built on the generated Unicode tables
//! ([`crate::unicode_tables`]): UTS-46 mapping, NFC normalization (canonical decomposition +
//! ordering + composition, including the algorithmic Hangul handling), and Punycode (RFC 3492). No
//! external `idna`/`url` dependency.
//!
//! Processing options match the WHATWG URL spec / WPT IDNA suites: non-transitional,
//! UseSTD3ASCIIRules=false, CheckHyphens=false, CheckBidi=true, CheckJoiners=true. We validate the
//! disallowed/NFC/leading-combining-mark/ContextJ/Bidi criteria.

use crate::unicode_tables::{BIDI, CCC, COMPOSE, DECOMP, JOINING, MARK, UTS46, UTS46_MAP};

const BUCKET_SHIFT: u32 = 10;
const BUCKET_LENGTH: usize = (0x110000 >> BUCKET_SHIFT) + 1;

const fn build_range3_buckets<const N: usize>(table: &[(u32, u32, u8)], shift: u32) -> [u16; N] {
    let mut buckets = [0u16; N];
    let mut bucket = 0usize;
    let mut index = 0usize;
    while bucket < N {
        let code_point = (bucket as u32) << shift;
        while index < table.len() && table[index].1 < code_point {
            index += 1;
        }
        buckets[bucket] = index as u16;
        bucket += 1;
    }
    buckets
}

const fn build_point_u8_buckets<const N: usize>(table: &[(u32, u8)], shift: u32) -> [u16; N] {
    let mut buckets = [0u16; N];
    let mut bucket = 0usize;
    let mut index = 0usize;
    while bucket < N {
        let code_point = (bucket as u32) << shift;
        while index < table.len() && table[index].0 < code_point {
            index += 1;
        }
        buckets[bucket] = index as u16;
        bucket += 1;
    }
    buckets
}

const fn build_point_str_buckets<const N: usize>(table: &[(u32, &str)], shift: u32) -> [u16; N] {
    let mut buckets = [0u16; N];
    let mut bucket = 0usize;
    let mut index = 0usize;
    while bucket < N {
        let code_point = (bucket as u32) << shift;
        while index < table.len() && table[index].0 < code_point {
            index += 1;
        }
        buckets[bucket] = index as u16;
        bucket += 1;
    }
    buckets
}

const fn build_compose_buckets<const N: usize>(table: &[(u32, u32, u32)], shift: u32) -> [u16; N] {
    let mut buckets = [0u16; N];
    let mut bucket = 0usize;
    let mut index = 0usize;
    while bucket < N {
        let code_point = (bucket as u32) << shift;
        while index < table.len() && table[index].0 < code_point {
            index += 1;
        }
        buckets[bucket] = index as u16;
        bucket += 1;
    }
    buckets
}

static UTS46_BUCKETS: [u16; BUCKET_LENGTH] = build_range3_buckets(UTS46, BUCKET_SHIFT);
static UTS46_MAP_BUCKETS: [u16; BUCKET_LENGTH] = build_point_str_buckets(UTS46_MAP, BUCKET_SHIFT);
static CCC_BUCKETS: [u16; BUCKET_LENGTH] = build_point_u8_buckets(CCC, BUCKET_SHIFT);
static DECOMP_BUCKETS: [u16; BUCKET_LENGTH] = build_point_str_buckets(DECOMP, BUCKET_SHIFT);
static COMPOSE_BUCKETS: [u16; BUCKET_LENGTH] = build_compose_buckets(COMPOSE, BUCKET_SHIFT);
static BIDI_BUCKETS: [u16; BUCKET_LENGTH] = build_range3_buckets(BIDI, BUCKET_SHIFT);

#[inline]
fn point_bucket<const N: usize>(buckets: &[u16; N], shift: u32, code_point: u32) -> (usize, usize) {
    let bucket = (code_point >> shift) as usize;
    (buckets[bucket] as usize, buckets[bucket + 1] as usize)
}

#[inline]
fn range_bucket<const N: usize>(buckets: &[u16; N], shift: u32, code_point: u32) -> (usize, usize) {
    let (start, end) = point_bucket(buckets, shift, code_point);
    (start, end.saturating_add(1))
}

fn is_mark(c: char) -> bool {
    let u = c as u32;
    let idx = MARK.partition_point(|&(_, end)| end < u);
    idx < MARK.len() && {
        let (start, end) = MARK[idx];
        u >= start && u <= end
    }
}

// Bidi_Class codes (see the BIDI table): 1=L 2=R 3=AL 4=AN 5=EN 6=ES 7=CS 8=ET 9=ON 10=BN 11=NSM.
fn bidi_class(c: char) -> u8 {
    let u = c as u32;
    let (start_index, end_index) = range_bucket(&BIDI_BUCKETS, BUCKET_SHIFT, u);
    let table = &BIDI[start_index..end_index.min(BIDI.len())];
    let idx = table.partition_point(|&(_, end, _)| end < u);
    if idx < table.len() {
        let (start, end, t) = table[idx];
        if u >= start && u <= end {
            return t;
        }
    }
    1 // default L (unlisted assigned code points are otherwise disallowed before this point)
}

/// IDNA CheckBidi rule (RFC 5893) for one label of a bidi domain.
fn label_bidi_ok(chars: impl IntoIterator<Item = char>) -> bool {
    let mut chars = chars.into_iter();
    let Some(first_character) = chars.next() else {
        return false;
    };
    let first = bidi_class(first_character);
    if !matches!(first, 1..=3) {
        return false;
    }

    let mut last_non_nsm = None;
    let mut has_an = false;
    let mut has_en = false;
    for character in std::iter::once(first_character).chain(chars) {
        let class = bidi_class(character);
        let allowed = match first {
            2 | 3 => matches!(class, 2..=11),
            1 => matches!(class, 1 | 5..=11),
            _ => unreachable!(),
        };
        if !allowed {
            return false;
        }
        if class != 11 {
            last_non_nsm = Some(class);
        }
        has_an |= class == 4;
        has_en |= class == 5;
    }

    match first {
        2 | 3 => matches!(last_non_nsm, Some(2..=5)) && !(has_an && has_en),
        1 => matches!(last_non_nsm, Some(1 | 5)),
        _ => unreachable!(),
    }
}

const VIRAMA: u8 = 9;
// Joining_Type codes (see the generated JOINING table): 1=L 2=R 3=D 4=C 5=T; unlisted = U(0).
fn joining_type(c: char) -> u8 {
    let u = c as u32;
    let idx = JOINING.partition_point(|&(_, end, _)| end < u);
    if idx < JOINING.len() {
        let (start, end, t) = JOINING[idx];
        if u >= start && u <= end {
            return t;
        }
    }
    0
}

/// IDNA ContextJ rule for ZWNJ (U+200C): valid after a Virama, or inside an (L|D) T* _ T* (R|D)
/// joining sequence (RFC 5892 A.1).
fn zwnj_ok(chars: &[char], idx: usize) -> bool {
    if idx > 0 && ccc(chars[idx - 1]) == VIRAMA {
        return true;
    }
    let mut j = idx;
    while j > 0 && joining_type(chars[j - 1]) == 5 {
        j -= 1;
    }
    if j == 0 || !matches!(joining_type(chars[j - 1]), 1 | 3) {
        return false;
    }
    let mut k = idx + 1;
    while k < chars.len() && joining_type(chars[k]) == 5 {
        k += 1;
    }
    k < chars.len() && matches!(joining_type(chars[k]), 2 | 3)
}

// Hangul syllable composition constants (UAX #15).
const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT; // 588
const S_COUNT: u32 = L_COUNT * N_COUNT; // 11172

enum Status {
    Valid,
    Mapped(&'static str),
    Ignored,
    Disallowed,
}

fn uts46_status(c: char) -> Status {
    let u = c as u32;
    let (start_index, end_index) = range_bucket(&UTS46_BUCKETS, BUCKET_SHIFT, u);
    let table = &UTS46[start_index..end_index.min(UTS46.len())];
    let idx = table.partition_point(|&(_, end, _)| end < u);
    if idx < table.len() {
        let (start, end, kind) = table[idx];
        if u >= start && u <= end {
            return match kind {
                0 => Status::Valid,
                1 => {
                    let (map_start, map_end) =
                        point_bucket(&UTS46_MAP_BUCKETS, BUCKET_SHIFT, start);
                    let mappings = &UTS46_MAP[map_start..map_end];
                    let mi = mappings
                        .binary_search_by_key(&start, |&(code_point, _)| code_point)
                        .expect("mapped UTS-46 ranges have mapping data");
                    Status::Mapped(mappings[mi].1)
                }
                2 => Status::Ignored,
                _ => Status::Disallowed,
            };
        }
    }
    Status::Disallowed
}

fn ccc(c: char) -> u8 {
    let u = c as u32;
    if u < 0x300 {
        return 0;
    }
    let (start, end) = point_bucket(&CCC_BUCKETS, BUCKET_SHIFT, u);
    let table = &CCC[start..end];
    match table.binary_search_by_key(&u, |&(code_point, _)| code_point) {
        Ok(i) => table[i].1,
        Err(_) => 0,
    }
}

fn canonical_decomposition(code_point: u32) -> Option<&'static str> {
    let (start, end) = point_bucket(&DECOMP_BUCKETS, BUCKET_SHIFT, code_point);
    let table = &DECOMP[start..end];
    table
        .binary_search_by_key(&code_point, |&(candidate, _)| candidate)
        .ok()
        .map(|index| table[index].1)
}

fn decompose(c: char, out: &mut Vec<char>) {
    let u = c as u32;
    if (S_BASE..S_BASE + S_COUNT).contains(&u) {
        let s = u - S_BASE;
        out.push(char::from_u32(L_BASE + s / N_COUNT).unwrap());
        out.push(char::from_u32(V_BASE + (s % N_COUNT) / T_COUNT).unwrap());
        let t = s % T_COUNT;
        if t != 0 {
            out.push(char::from_u32(T_BASE + t).unwrap());
        }
        return;
    }
    match canonical_decomposition(u) {
        Some(decomposition) => out.extend(decomposition.chars()),
        None => out.push(c),
    }
}

fn canonical_order(chars: &mut [char]) {
    // Stable bubble of adjacent combining marks by combining class.
    if chars.len() < 2 {
        return;
    }
    let mut i = 1;
    while i < chars.len() {
        let a = ccc(chars[i - 1]);
        let b = ccc(chars[i]);
        if b != 0 && a != 0 && a > b {
            chars.swap(i - 1, i);
            if i > 1 {
                i -= 1;
                continue;
            }
        }
        i += 1;
    }
}

fn compose_pair(a: char, b: char) -> Option<char> {
    let (au, bu) = (a as u32, b as u32);
    // Hangul L + V
    if (L_BASE..L_BASE + L_COUNT).contains(&au) && (V_BASE..V_BASE + V_COUNT).contains(&bu) {
        let l = au - L_BASE;
        let v = bu - V_BASE;
        return char::from_u32(S_BASE + (l * V_COUNT + v) * T_COUNT);
    }
    // Hangul LV + T
    if (S_BASE..S_BASE + S_COUNT).contains(&au)
        && (au - S_BASE).is_multiple_of(T_COUNT)
        && (T_BASE + 1..T_BASE + T_COUNT).contains(&bu)
    {
        return char::from_u32(au + (bu - T_BASE));
    }
    let (start, end) = point_bucket(&COMPOSE_BUCKETS, BUCKET_SHIFT, au);
    let table = &COMPOSE[start..end];
    match table.binary_search_by(|&(x, y, _)| (x, y).cmp(&(au, bu))) {
        Ok(i) => char::from_u32(table[i].2),
        Err(_) => None,
    }
}

fn definitely_nfc(input: &str) -> bool {
    let mut previous = None;
    for character in input.chars() {
        let code_point = character as u32;
        if ccc(character) != 0
            || (S_BASE..S_BASE + S_COUNT).contains(&code_point)
            || canonical_decomposition(code_point).is_some()
        {
            return false;
        }
        if previous.is_some_and(|starter| compose_pair(starter, character).is_some()) {
            return false;
        }
        previous = Some(character);
    }
    true
}

fn nfc(input: &str) -> String {
    let mut chars: Vec<char> = Vec::with_capacity(input.len());
    for c in input.chars() {
        decompose(c, &mut chars);
    }
    canonical_order(&mut chars);
    // Canonical composition. Keep the current starter in the output so
    // combining marks can be composed without allocating a temporary vector
    // for every starter sequence.
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut starter = None;
    let mut last_class = 0u8;
    for character in chars {
        let class = ccc(character);
        if let Some(starter_index) = starter {
            if (last_class == 0 || last_class < class)
                && let Some(composed) = compose_pair(out[starter_index], character)
            {
                out[starter_index] = composed;
                continue;
            }
        }
        if class == 0 {
            starter = Some(out.len());
        }
        last_class = class;
        out.push(character);
    }
    out.into_iter().collect()
}

/// A UTS-46 label is valid if it is NFC, doesn't begin with a combining mark, and every code point
/// has "valid" status (CheckHyphens/CheckBidi off, per the WPT options).
fn valid_normalized_label(chars: &[char], check_bidi: bool) -> bool {
    if chars.is_empty() {
        return false;
    }
    // A label must not begin with a combining mark (General_Category Mark).
    if is_mark(chars[0]) {
        return false;
    }
    if check_bidi && !label_bidi_ok(chars.iter().copied()) {
        return false;
    }
    for (idx, &c) in chars.iter().enumerate() {
        // ZWNJ/ZWJ are valid only in their ContextJ join contexts (RFC 5892 A.1/A.2).
        if c == '\u{200c}' {
            if !zwnj_ok(chars, idx) {
                return false;
            }
        } else if c == '\u{200d}' {
            if idx == 0 || ccc(chars[idx - 1]) != VIRAMA {
                return false;
            }
        } else if !matches!(uts46_status(c), Status::Valid) {
            return false;
        }
    }
    true
}

fn valid_normalized_ascii_label(label: &str, check_bidi: bool) -> bool {
    if label.is_empty() {
        return false;
    }
    if check_bidi && !label_bidi_ok(label.chars()) {
        return false;
    }
    label
        .chars()
        .all(|character| matches!(uts46_status(character), Status::Valid))
}

/// UTS-46 ToASCII (the host parser's domain-to-ASCII step).
pub(crate) fn domain_to_ascii(domain: &str) -> Result<String, ()> {
    let validate_alabels = !domain.is_ascii();
    // 1. Map (and reject disallowed code points).
    let mut mapped = String::with_capacity(domain.len());
    for c in domain.chars() {
        match uts46_status(c) {
            Status::Valid => mapped.push(c),
            Status::Mapped(s) => mapped.push_str(s),
            Status::Ignored => {}
            Status::Disallowed => return Err(()),
        }
    }
    // 2. Normalize (NFC).
    let normalized = if mapped.is_ascii() || definitely_nfc(&mapped) {
        mapped
    } else {
        nfc(&mapped)
    };
    // 3. WHATWG host parsing preserves ASCII A-label text verbatim after
    // UTS-46 mapping, including labels whose `xn--` suffix is not decodable
    // Punycode.
    // 4. A domain is a "bidi domain" if any label (in its Unicode form) has an R/AL/AN code point;
    //    CheckBidi then applies to every label.
    let check_bidi = normalized
        .chars()
        .any(|character| matches!(bidi_class(character), 2..=4));
    // 5. Validate each non-empty label and assemble the ASCII output.
    let mut out = String::with_capacity(normalized.len());
    for (i, label) in normalized.split('.').enumerate() {
        if i > 0 {
            out.push('.');
        }
        if label.is_empty() {
            // Empty label (e.g. a trailing dot) — allowed.
            continue;
        }
        if label.is_ascii() {
            if !valid_normalized_ascii_label(label, check_bidi) {
                return Err(());
            }
            if validate_alabels && let Some(encoded) = label.strip_prefix("xn--") {
                let decoded = punycode_decode(encoded).ok_or(())?;
                if !valid_normalized_label(&decoded, check_bidi) {
                    return Err(());
                }
            }
            out.push_str(label);
            continue;
        }
        if label.starts_with("xn--") {
            return Err(());
        }
        let characters: Vec<char> = label.chars().collect();
        if !valid_normalized_label(&characters, check_bidi) {
            return Err(());
        }
        out.push_str("xn--");
        out.push_str(&punycode_encode(&characters).ok_or(())?);
    }
    if out.is_empty() {
        return Err(());
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Punycode (RFC 3492)
// ---------------------------------------------------------------------------------------------

const BASE: u32 = 36;
const TMIN: u32 = 1;
const TMAX: u32 = 26;
const SKEW: u32 = 38;
const DAMP: u32 = 700;
const INITIAL_BIAS: u32 = 72;
const INITIAL_N: u32 = 128;

fn adapt(mut delta: u32, num_points: u32, first_time: bool) -> u32 {
    delta = if first_time { delta / DAMP } else { delta / 2 };
    delta += delta / num_points;
    let mut k = 0;
    while delta > ((BASE - TMIN) * TMAX) / 2 {
        delta /= BASE - TMIN;
        k += BASE;
    }
    k + (((BASE - TMIN + 1) * delta) / (delta + SKEW))
}

fn punycode_encode(input: &[char]) -> Option<String> {
    fn digit_to_basic(d: u32) -> char {
        if d < 26 {
            (b'a' + d as u8) as char
        } else {
            (b'0' + (d - 26) as u8) as char
        }
    }
    let mut output = String::new();
    let mut n = INITIAL_N;
    let mut delta: u32 = 0;
    let mut bias = INITIAL_BIAS;
    let basics: Vec<u32> = input
        .iter()
        .map(|&c| c as u32)
        .filter(|&c| c < 0x80)
        .collect();
    let b = basics.len();
    for &c in &basics {
        output.push(char::from_u32(c)?);
    }
    if b > 0 {
        output.push('-');
    }
    let mut h = b as u32;
    let total = input.len() as u32;
    while h < total {
        let m = input.iter().map(|&c| c as u32).filter(|&c| c >= n).min()?;
        delta = delta.checked_add((m - n).checked_mul(h + 1)?)?;
        n = m;
        for &cc in input {
            let c = cc as u32;
            if c < n {
                delta = delta.checked_add(1)?;
            }
            if c == n {
                let mut q = delta;
                let mut k = BASE;
                loop {
                    let t = if k <= bias {
                        TMIN
                    } else if k >= bias + TMAX {
                        TMAX
                    } else {
                        k - bias
                    };
                    if q < t {
                        break;
                    }
                    output.push(digit_to_basic(t + (q - t) % (BASE - t)));
                    q = (q - t) / (BASE - t);
                    k += BASE;
                }
                output.push(digit_to_basic(q));
                bias = adapt(delta, h + 1, h == b as u32);
                delta = 0;
                h += 1;
            }
        }
        delta += 1;
        n += 1;
    }
    Some(output)
}

/// Write a conservative, already-normalized left-to-right domain directly to
/// `output`. Returns false without changing `output` when the full UTS-46 path
/// is required.
pub(crate) fn push_simple_domain_to_ascii(output: &mut String, domain: &str) -> bool {
    let original_length = output.len();
    let validate_alabels = !domain.is_ascii();
    if domain.is_empty() {
        return false;
    }

    let mut labels = domain.split('.').peekable();
    let mut first = true;
    while let Some(label) = labels.next() {
        if !first {
            output.push('.');
        }
        first = false;
        if label.is_empty() {
            if labels.peek().is_none() {
                continue;
            }
            output.truncate(original_length);
            return false;
        }

        let mut has_non_ascii = false;
        for character in label.chars() {
            if character.is_ascii() {
                if !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_')) {
                    output.truncate(original_length);
                    return false;
                }
                continue;
            }
            has_non_ascii = true;
            let latin = matches!(character, '\u{00df}'..='\u{00f6}' | '\u{00f8}'..='\u{02af}');
            if !latin
                || is_mark(character)
                || bidi_class(character) != 1
                || !matches!(uts46_status(character), Status::Valid)
            {
                output.truncate(original_length);
                return false;
            }
        }

        if has_non_ascii {
            if label.starts_with("xn--") {
                output.truncate(original_length);
                return false;
            }
            output.push_str("xn--");
            if !punycode_encode_str_into(output, label) {
                output.truncate(original_length);
                return false;
            }
        } else {
            if validate_alabels
                && let Some(encoded) = label.strip_prefix("xn--")
                && punycode_decode(encoded)
                    .is_none_or(|decoded| !valid_normalized_label(&decoded, false))
            {
                output.truncate(original_length);
                return false;
            }
            let start = output.len();
            output.push_str(label);
            output[start..].make_ascii_lowercase();
        }
    }
    true
}

fn punycode_encode_str_into(output: &mut String, input: &str) -> bool {
    fn digit_to_basic(digit: u32) -> char {
        if digit < 26 {
            char::from(b'a' + digit as u8)
        } else {
            char::from(b'0' + (digit - 26) as u8)
        }
    }
    let mapped = |character: char| {
        if character.is_ascii_uppercase() {
            character.to_ascii_lowercase() as u32
        } else {
            character as u32
        }
    };

    let mut code_point = INITIAL_N;
    let mut delta = 0u32;
    let mut bias = INITIAL_BIAS;
    let mut basics = 0u32;
    let mut total = 0u32;
    for character in input.chars() {
        total += 1;
        let value = mapped(character);
        if value < 0x80 {
            let Some(character) = char::from_u32(value) else {
                return false;
            };
            output.push(character);
            basics += 1;
        }
    }
    if basics > 0 {
        output.push('-');
    }

    let mut handled = basics;
    while handled < total {
        let Some(next) = input
            .chars()
            .map(mapped)
            .filter(|&value| value >= code_point)
            .min()
        else {
            return false;
        };
        let Some(increment) = (next - code_point).checked_mul(handled + 1) else {
            return false;
        };
        let Some(next_delta) = delta.checked_add(increment) else {
            return false;
        };
        delta = next_delta;
        code_point = next;

        for character in input.chars() {
            let value = mapped(character);
            if value < code_point {
                let Some(next_delta) = delta.checked_add(1) else {
                    return false;
                };
                delta = next_delta;
            }
            if value == code_point {
                let mut quotient = delta;
                let mut k = BASE;
                loop {
                    let threshold = if k <= bias {
                        TMIN
                    } else if k >= bias + TMAX {
                        TMAX
                    } else {
                        k - bias
                    };
                    if quotient < threshold {
                        break;
                    }
                    output.push(digit_to_basic(
                        threshold + (quotient - threshold) % (BASE - threshold),
                    ));
                    quotient = (quotient - threshold) / (BASE - threshold);
                    k += BASE;
                }
                output.push(digit_to_basic(quotient));
                bias = adapt(delta, handled + 1, handled == basics);
                delta = 0;
                handled += 1;
            }
        }
        delta += 1;
        code_point += 1;
    }
    true
}

pub(crate) fn punycode_decode(input: &str) -> Option<Vec<char>> {
    fn basic_to_digit(c: u8) -> Option<u32> {
        match c {
            b'a'..=b'z' => Some((c - b'a') as u32),
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'0'..=b'9' => Some((c - b'0' + 26) as u32),
            _ => None,
        }
    }
    let bytes = input.as_bytes();
    let mut output: Vec<u32> = Vec::new();
    let (basic, rest) = match input.rfind('-') {
        Some(idx) => (&bytes[..idx], &bytes[idx + 1..]),
        None => (&bytes[..0], bytes),
    };
    for &b in basic {
        if b >= 0x80 {
            return None;
        }
        output.push(b as u32);
    }
    let mut n = INITIAL_N;
    let mut i: u32 = 0;
    let mut bias = INITIAL_BIAS;
    let mut pos = 0usize;
    while pos < rest.len() {
        let oldi = i;
        let mut w = 1u32;
        let mut k = BASE;
        loop {
            if pos >= rest.len() {
                return None;
            }
            let digit = basic_to_digit(rest[pos])?;
            pos += 1;
            i = i.checked_add(digit.checked_mul(w)?)?;
            let t = if k <= bias {
                TMIN
            } else if k >= bias + TMAX {
                TMAX
            } else {
                k - bias
            };
            if digit < t {
                break;
            }
            w = w.checked_mul(BASE - t)?;
            k += BASE;
        }
        let out_len = output.len() as u32 + 1;
        bias = adapt(i - oldi, out_len, oldi == 0);
        n = n.checked_add(i / out_len)?;
        i %= out_len;
        char::from_u32(n)?;
        output.insert(i as usize, n);
        i += 1;
    }
    output.into_iter().map(char::from_u32).collect()
}
