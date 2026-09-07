//! Normalization and serialization of special URL hosts.
use crate::{Error, Result, UrlHostType, idna, legacy};

// A conservative gate: every numeric last label starts with a digit, including
// hexadecimal forms ending in a-f. False positives go to the general parser.
#[inline]
pub(crate) fn may_be_ipv4(hostname: &str) -> bool {
    let hostname = hostname.trim_end_matches('.');
    hostname
        .as_bytes()
        .last()
        .is_some_and(|byte| byte.is_ascii_hexdigit() || matches!(byte, b'x' | b'X'))
        && hostname
            .rsplit('.')
            .next()
            .and_then(|label| label.as_bytes().first())
            .is_some_and(u8::is_ascii_digit)
}

#[inline]
pub(crate) fn push_normalized_special_host(
    output: &mut String,
    input: &str,
) -> Result<UrlHostType> {
    if let Some(segments) = input
        .strip_prefix('[')
        .and_then(|input| input.strip_suffix(']'))
        .and_then(|input| legacy::parse_ipv6(input).ok())
    {
        push_ipv6_address(output, &segments);
        return Ok(UrlHostType::Ipv6);
    }

    let ends_in_number = may_be_ipv4(input);
    let contains_percent = input.contains('%');
    let simple_ascii = !input.starts_with('[')
        && !contains_percent
        && !ends_in_number
        && input
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'));

    if simple_ascii {
        let start = output.len();
        output.push_str(input);
        output[start..].make_ascii_lowercase();
        return Ok(UrlHostType::Default);
    }

    if ends_in_number && input.is_ascii() && !contains_percent {
        if let Some(address) = parse_ipv4_address(input) {
            push_decimal_u8(output, (address >> 24) as u8);
            output.push('.');
            push_decimal_u8(output, (address >> 16) as u8);
            output.push('.');
            push_decimal_u8(output, (address >> 8) as u8);
            output.push('.');
            push_decimal_u8(output, address as u8);
            return Ok(UrlHostType::Ipv4);
        }
    }

    if !input.is_ascii()
        && !contains_percent
        && !ends_in_number
        && idna::push_simple_domain_to_ascii(output, input)
    {
        return Ok(UrlHostType::Default);
    }

    let normalized = if input.is_ascii() || contains_percent || ends_in_number {
        legacy::normalize_special_host(input).map_err(|()| Error::TypeError)?
    } else {
        idna::domain_to_ascii(input).map_err(|()| Error::TypeError)?
    };
    output.push_str(&normalized);
    if normalized.starts_with('[') {
        Ok(UrlHostType::Ipv6)
    } else if normalized
        .bytes()
        .all(|byte| byte.is_ascii_digit() || byte == b'.')
        && normalized.bytes().any(|byte| byte == b'.')
    {
        Ok(UrlHostType::Ipv4)
    } else {
        Ok(UrlHostType::Default)
    }
}

#[inline]
fn push_decimal_u8(output: &mut String, value: u8) {
    if value >= 100 {
        output.push(char::from(b'0' + value / 100));
        output.push(char::from(b'0' + (value / 10) % 10));
    } else if value >= 10 {
        output.push(char::from(b'0' + value / 10));
    }
    output.push(char::from(b'0' + value % 10));
}

fn push_ipv6_address(output: &mut String, segments: &[u16; 8]) {
    let mut best_start = 0usize;
    let mut best_length = 0usize;
    let mut cursor = 0usize;
    while cursor < segments.len() {
        if segments[cursor] != 0 {
            cursor += 1;
            continue;
        }
        let start = cursor;
        while cursor < segments.len() && segments[cursor] == 0 {
            cursor += 1;
        }
        let length = cursor - start;
        if length > best_length {
            best_start = start;
            best_length = length;
        }
    }
    if best_length < 2 {
        best_length = 0;
    }

    const HEX: &[u8; 16] = b"0123456789abcdef";
    output.push('[');
    let mut index = 0usize;
    while index < segments.len() {
        if best_length > 0 && index == best_start {
            output.push_str("::");
            index += best_length;
            if index == segments.len() {
                break;
            }
        } else {
            if index > 0 && (best_length == 0 || index != best_start + best_length) {
                output.push(':');
            }
            let value = segments[index];
            let mut shift = 12u32;
            while shift > 0 && ((value >> shift) & 0xf) == 0 {
                shift -= 4;
            }
            loop {
                output.push(char::from(HEX[((value >> shift) & 0xf) as usize]));
                if shift == 0 {
                    break;
                }
                shift -= 4;
            }
            index += 1;
        }
    }
    output.push(']');
}

#[inline]
fn parse_ipv4_address(input: &str) -> Option<u32> {
    let input = input.strip_suffix('.').unwrap_or(input);
    let mut parts = [0u64; 4];
    let mut count = 0usize;
    for part in input.split('.') {
        if count == parts.len() || part.is_empty() {
            return None;
        }
        let (digits, radix) =
            if let Some(hex) = part.strip_prefix("0x").or_else(|| part.strip_prefix("0X")) {
                (hex, 16)
            } else if part.len() > 1 && part.starts_with('0') {
                (&part[1..], 8)
            } else {
                (part, 10)
            };
        parts[count] = if digits.is_empty() {
            0
        } else {
            u64::from_str_radix(digits, radix).ok()?
        };
        count += 1;
    }
    if count == 0 {
        return None;
    }
    if parts[..count.saturating_sub(1)]
        .iter()
        .any(|&part| part > 255)
    {
        return None;
    }
    let last_limit = 1u64.checked_shl(8 * (5 - count) as u32)?;
    if parts[count - 1] >= last_limit {
        return None;
    }
    let mut address = parts[count - 1];
    for (index, &part) in parts[..count - 1].iter().enumerate() {
        address = address.checked_add(part << (8 * (3 - index)))?;
    }
    u32::try_from(address).ok()
}
