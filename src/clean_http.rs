//! Conservative HTTP fast-path validation and component scanning.
use crate::host_normalization::may_be_ipv4;
use crate::{
    BYTE_FRAGMENT, BYTE_HOST, BYTE_PATH_NO_DOT, BYTE_SPECIAL_QUERY, Error, FastHttpComponents,
    FastHttpScan, UrlAggregator, UrlComponents, canonical_ipv4, dot_path_segment_at,
    eight_bytes_have_class, four_bytes_have_class, fragment_byte_is_canonical,
    host_byte_is_canonical, legacy, path_byte_is_canonical, special_query_byte_is_canonical,
    two_bytes_have_class,
};

pub(crate) fn try_can_parse_clean_http(input: &str) -> Option<bool> {
    let bytes = input.as_bytes();
    let authority_start = if bytes.starts_with(b"https://") {
        8usize
    } else if bytes.starts_with(b"http://") {
        7usize
    } else {
        return None;
    };
    if authority_start >= bytes.len() {
        return Some(false);
    }
    if bytes[authority_start] == b'[' {
        let closing = bytes[authority_start + 1..]
            .iter()
            .position(|&byte| byte == b']')
            .map(|offset| authority_start + 1 + offset)?;
        if legacy::parse_ipv6(&input[authority_start + 1..closing]).is_err() {
            return Some(false);
        }
        let mut cursor = closing + 1;
        if bytes.get(cursor) == Some(&b':') {
            cursor += 1;
            let port_start = cursor;
            let mut port = 0u32;
            while cursor < bytes.len() && !matches!(bytes[cursor], b'/' | b'?' | b'#') {
                let byte = bytes[cursor];
                if !byte.is_ascii_digit() {
                    return Some(false);
                }
                port = port
                    .checked_mul(10)
                    .and_then(|value| value.checked_add(u32::from(byte - b'0')))
                    .unwrap_or(u32::MAX);
                if port > u32::from(u16::MAX) {
                    return Some(false);
                }
                cursor += 1;
            }
            if cursor == port_start {
                return None;
            }
        } else if !matches!(bytes.get(cursor), None | Some(b'/' | b'?' | b'#')) {
            return Some(false);
        }
        return Some(true);
    }

    let mut cursor = authority_start;
    while cursor + 4 <= bytes.len() && four_bytes_have_class(bytes, cursor, BYTE_HOST) {
        cursor += 4;
    }
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if matches!(byte, b'/' | b'?' | b'#') {
            break;
        }
        if byte == b':' {
            break;
        }
        if !host_byte_is_canonical(byte) {
            return None;
        }
        cursor += 1;
    }

    let host_end = cursor;
    if bytes.get(cursor) == Some(&b':') {
        cursor += 1;
        let port_start = cursor;
        let mut port = 0u32;
        while cursor < bytes.len() && !matches!(bytes[cursor], b'/' | b'?' | b'#') {
            let byte = bytes[cursor];
            if !byte.is_ascii_digit() {
                return None;
            }
            port = port
                .checked_mul(10)
                .and_then(|value| value.checked_add(u32::from(byte - b'0')))
                .unwrap_or(u32::MAX);
            if port > u32::from(u16::MAX) {
                return Some(false);
            }
            cursor += 1;
        }
        if cursor == port_start {
            return None;
        }
    }
    if host_end == authority_start {
        return Some(false);
    }
    let last = bytes[host_end - 1];
    if last == b'.' {
        return None;
    }
    let hostname = &input[authority_start..host_end];
    if last.is_ascii_digit() {
        if !hostname
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
            || !canonical_ipv4(hostname)
        {
            return None;
        }
    } else if may_be_ipv4(hostname) {
        return None;
    }
    Some(true)
}

impl UrlAggregator {
    #[inline(always)]
    pub(crate) fn try_fast_clean_http_components(input: &str) -> Option<FastHttpScan> {
        let bytes = input.as_bytes();
        let protocol_end = if bytes.starts_with(b"https://") {
            5usize
        } else if bytes.starts_with(b"http://") {
            4usize
        } else {
            return None;
        };
        let authority_start = protocol_end + 3;
        if authority_start >= bytes.len() {
            return Some(FastHttpScan::Parsed(Err(Error::TypeError)));
        }

        let mut cursor = authority_start;
        while cursor + 8 <= bytes.len() && eight_bytes_have_class(bytes, cursor, BYTE_HOST) {
            cursor += 8;
        }
        while cursor < bytes.len() {
            let byte = bytes[cursor];
            if matches!(byte, b'/' | b'?' | b'#') {
                break;
            }
            if !host_byte_is_canonical(byte) {
                if matches!(byte, b':' | b'@') {
                    return Some(FastHttpScan::ComplexAuthority);
                }
                return None;
            }
            cursor += 1;
        }

        let host_end = cursor;
        if host_end == authority_start {
            if matches!(bytes.get(authority_start), Some(b'/' | b'\\')) {
                return None;
            }
            return Some(FastHttpScan::Parsed(Err(Error::TypeError)));
        }
        let last = bytes[host_end - 1];
        if last == b'.' {
            return None;
        }

        if may_be_ipv4(&input[authority_start..host_end]) {
            return None;
        }

        let pathname_start = cursor;
        let mut search_start = UrlComponents::OMITTED;
        let mut hash_start = UrlComponents::OMITTED;
        if bytes.get(cursor) == Some(&b'/') {
            cursor += 1;
            while cursor < bytes.len() && !matches!(bytes[cursor], b'?' | b'#') {
                if cursor + 4 <= bytes.len()
                    && four_bytes_have_class(bytes, cursor, BYTE_PATH_NO_DOT)
                {
                    cursor += 4;
                    continue;
                }
                if cursor + 2 <= bytes.len()
                    && two_bytes_have_class(bytes, cursor, BYTE_PATH_NO_DOT)
                {
                    cursor += 2;
                    continue;
                }
                let byte = bytes[cursor];
                if !path_byte_is_canonical(byte) {
                    return None;
                }
                if matches!(byte, b'.' | b'%')
                    && (cursor == pathname_start + 1 || bytes[cursor - 1] == b'/')
                    && dot_path_segment_at(bytes, cursor)
                {
                    return None;
                }
                cursor += 1;
            }
        }
        if bytes.get(cursor) == Some(&b'?') {
            search_start = cursor as u32;
            cursor += 1;
            while cursor < bytes.len() && bytes[cursor] != b'#' {
                if cursor + 8 <= bytes.len()
                    && eight_bytes_have_class(bytes, cursor, BYTE_SPECIAL_QUERY)
                {
                    cursor += 8;
                    continue;
                }
                if !special_query_byte_is_canonical(bytes[cursor]) {
                    return None;
                }
                cursor += 1;
            }
        }
        if bytes.get(cursor) == Some(&b'#') {
            hash_start = cursor as u32;
            cursor += 1;
            while cursor < bytes.len() {
                if cursor + 8 <= bytes.len() && eight_bytes_have_class(bytes, cursor, BYTE_FRAGMENT)
                {
                    cursor += 8;
                    continue;
                }
                if !fragment_byte_is_canonical(bytes[cursor]) {
                    return None;
                }
                cursor += 1;
            }
        }
        if cursor != bytes.len() {
            return None;
        }

        Some(FastHttpScan::Parsed(Ok(FastHttpComponents {
            protocol_end,
            authority_start,
            host_end,
            pathname_start,
            search_start,
            hash_start,
        })))
    }
}
