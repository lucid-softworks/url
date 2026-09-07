//! Canonical special URLs with credentials or ports.
use crate::{
    Error, FLAG_HOSTNAME, FLAG_PASSWORD, Result, UrlAggregator, UrlComponents, UrlHostType,
    canonical_ipv4, canonical_path, fragment_byte_is_canonical, host_byte_is_canonical,
    special_query_byte_is_canonical, userinfo_byte_is_canonical,
};

impl UrlAggregator {
    /// Parse an already-canonical ASCII special URL in one pass and one
    /// allocation. Inputs needing normalization fall through to the complete
    /// WHATWG state machine.
    pub(crate) fn try_fast_absolute(input: &str) -> Option<Result<Self>> {
        let bytes = input.as_bytes();
        if bytes.len() < 8 {
            return None;
        }

        let (protocol_end, default_port) = if bytes.starts_with(b"https://") {
            (5, 443)
        } else if bytes.starts_with(b"http://") {
            (4, 80)
        } else if bytes.starts_with(b"ftp://") {
            (3, 21)
        } else if bytes.starts_with(b"wss://") {
            (3, 443)
        } else if bytes.starts_with(b"ws://") {
            (2, 80)
        } else {
            return None;
        };

        let authority_start = protocol_end + 3;
        let suffix_start = bytes[authority_start..]
            .iter()
            .position(|&byte| matches!(byte, b'/' | b'?' | b'#'))
            .map_or(bytes.len(), |offset| authority_start + offset);
        if suffix_start == authority_start {
            if matches!(bytes.get(authority_start), Some(b'/' | b'\\')) {
                return None;
            }
            return Some(Err(Error::TypeError));
        }

        let authority = &bytes[authority_start..suffix_start];
        let at = authority.iter().rposition(|&byte| byte == b'@');
        if let Some(offset) = at {
            if authority[..offset].contains(&b'@') {
                return None;
            }
        }
        let userinfo_end = at.map(|offset| authority_start + offset);
        let host_start = userinfo_end.map_or(authority_start, |end| end + 1);
        if host_start == suffix_start {
            return Some(Err(Error::TypeError));
        }
        if userinfo_end.is_some_and(|end| {
            end == authority_start || bytes.get(end.wrapping_sub(1)) == Some(&b':')
        }) {
            return None;
        }

        let username_end = userinfo_end.map_or(host_start, |end| {
            bytes[authority_start..end]
                .iter()
                .position(|&byte| byte == b':')
                .map_or(end, |offset| authority_start + offset)
        });
        if let Some(end) = userinfo_end {
            if bytes[authority_start..end]
                .iter()
                .any(|&byte| !userinfo_byte_is_canonical(byte))
            {
                return None;
            }
        }

        let host_and_port = &bytes[host_start..suffix_start];
        if host_and_port.starts_with(b"[") {
            return None;
        }
        let port_colon = host_and_port.iter().rposition(|&byte| byte == b':');
        let host_end = port_colon.map_or(suffix_start, |offset| host_start + offset);
        if host_end == host_start
            || bytes[host_start..host_end]
                .iter()
                .any(|&byte| !host_byte_is_canonical(byte))
        {
            return None;
        }
        let hostname = &input[host_start..host_end];
        let last_label = hostname
            .trim_end_matches('.')
            .rsplit('.')
            .next()
            .unwrap_or("");
        let ends_in_number = last_label
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_digit);
        let host_type = if ends_in_number {
            if !hostname
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'.')
                || !canonical_ipv4(hostname)
            {
                return None;
            }
            UrlHostType::Ipv4
        } else {
            UrlHostType::Default
        };

        let port = if port_colon.is_some() {
            let port_input = &input[host_end + 1..suffix_start];
            let Ok(port) = port_input.parse::<u16>() else {
                return None;
            };
            if port == default_port || (port_input.len() > 1 && port_input.starts_with('0')) {
                return None;
            }
            u32::from(port)
        } else {
            UrlComponents::OMITTED
        };

        let pathname_start = suffix_start;
        let mut search_start = UrlComponents::OMITTED;
        let mut hash_start = UrlComponents::OMITTED;
        let mut cursor = suffix_start;
        if bytes.get(cursor) == Some(&b'/') {
            let path_end = bytes[cursor..]
                .iter()
                .position(|&byte| matches!(byte, b'?' | b'#'))
                .map_or(bytes.len(), |offset| cursor + offset);
            if !canonical_path(&bytes[cursor..path_end]) {
                return None;
            }
            cursor = path_end;
        }
        if bytes.get(cursor) == Some(&b'?') {
            search_start = cursor as u32;
            cursor += 1;
            while cursor < bytes.len() && bytes[cursor] != b'#' {
                if !special_query_byte_is_canonical(bytes[cursor]) {
                    return None;
                }
                cursor += 1;
            }
        }
        if bytes.get(cursor) == Some(&b'#') {
            hash_start = cursor as u32;
            cursor += 1;
            if bytes[cursor..]
                .iter()
                .any(|&byte| !fragment_byte_is_canonical(byte))
            {
                return None;
            }
            cursor = bytes.len();
        }
        if cursor != bytes.len() {
            return None;
        }

        let mut buffer;
        if bytes.get(pathname_start) == Some(&b'/') {
            buffer = input.to_owned();
        } else {
            buffer = String::with_capacity(input.len() + 1);
            buffer.push_str(&input[..pathname_start]);
            buffer.push('/');
            buffer.push_str(&input[pathname_start..]);
            if search_start != UrlComponents::OMITTED {
                search_start += 1;
            }
            if hash_start != UrlComponents::OMITTED {
                hash_start += 1;
            }
        }

        let flags = FLAG_HOSTNAME
            | if userinfo_end.is_some() && username_end < userinfo_end.unwrap_or(username_end) {
                FLAG_PASSWORD
            } else {
                0
            };
        Some(Ok(Self {
            components: UrlComponents {
                protocol_end: protocol_end as u32,
                username_end: username_end as u32,
                host_start: host_start as u32,
                host_end: host_end as u32,
                port,
                pathname_start: pathname_start as u32,
                search_start,
                hash_start,
            },
            buffer,
            flags,
            host_type,
        }))
    }
}
