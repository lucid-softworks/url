//! A dependency-free WHATWG URL parser with a Rust adaptation of Ada's public
//! URL interface.
//!
//! The crate exposes the same two representations as Ada:
//!
//! - [`Url`] exposes Ada's setter-oriented, owned-getter surface.
//! - [`UrlAggregator`] owns one serialized buffer and component offsets, making
//!   getters allocation-free.

use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};

mod encoding;
mod encoding_tables;
mod idna;
mod legacy;
mod unicode_tables;

/// The single parse error exposed by Ada.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The input is not a valid URL.
    TypeError,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("invalid URL")
    }
}

impl std::error::Error for Error {}

/// Result returned by URL parsing operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Ada-compatible host classification.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UrlHostType {
    /// A domain name or opaque hostname.
    #[default]
    Default,
    /// An IPv4 address.
    Ipv4,
    /// An IPv6 address.
    Ipv6,
}

/// Offsets into a [`UrlAggregator`]'s serialized buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UrlComponents {
    pub protocol_end: u32,
    pub username_end: u32,
    pub host_start: u32,
    pub host_end: u32,
    pub port: u32,
    pub pathname_start: u32,
    pub search_start: u32,
    pub hash_start: u32,
}

impl UrlComponents {
    /// Sentinel used by Ada for an omitted component.
    pub const OMITTED: u32 = u32::MAX;
}

const FLAG_HOSTNAME: u8 = 1 << 0;
const FLAG_PASSWORD: u8 = 1 << 1;
const FLAG_OPAQUE_PATH: u8 = 1 << 2;

/// Setter-oriented URL representation with Ada-compatible owned getters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Url {
    aggregate: UrlAggregator,
}

/// Getter-optimised URL representation backed by one allocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UrlAggregator {
    components: UrlComponents,
    buffer: String,
    flags: u8,
    host_type: UrlHostType,
}

/// A URL representation accepted by [`parse`].
pub trait ParseTarget: Sized {
    #[doc(hidden)]
    fn parse_target(input: &str, base: Option<&Self>) -> Result<Self>;
}

static MAX_INPUT_LENGTH: AtomicU32 = AtomicU32::new(u32::MAX);

/// Parse a URL into either [`Url`] or [`UrlAggregator`].
///
/// This is the Rust equivalent of `ada::parse<result_type>(input, base)`.
pub fn parse<T: ParseTarget>(input: &str, base: Option<&T>) -> Result<T> {
    let limit = get_max_input_length() as usize;
    if input.len() > limit {
        return Err(Error::TypeError);
    }
    let parsed = T::parse_target(input, base)?;
    Ok(parsed)
}

/// Return whether `input` parses, optionally resolving it against `base`.
pub fn can_parse(input: &str, base: Option<&str>) -> bool {
    if input.len() > get_max_input_length() as usize {
        return false;
    }
    match base {
        None => legacy::Url::parse(input).is_ok(),
        Some(base_input) => {
            let Ok(base_url) = legacy::Url::parse(base_input) else {
                return false;
            };
            legacy::Url::parse_with_base(input, &base_url).is_ok()
        }
    }
}

/// Resolve `input` against an absolute base URL.
pub fn resolve(input: &str, base: &str) -> Option<String> {
    legacy::resolve(input, base)
}

/// Set the maximum accepted raw and serialized URL length.
pub fn set_max_input_length(length: u32) {
    MAX_INPUT_LENGTH.store(length, Ordering::Relaxed);
}

/// Return the current maximum URL length.
pub fn get_max_input_length() -> u32 {
    MAX_INPUT_LENGTH.load(Ordering::Relaxed)
}

/// Convert a filesystem path to a `file:` URL.
pub fn href_from_file(path: &str) -> String {
    if path.len() > get_max_input_length() as usize {
        return String::new();
    }
    let mut result = String::with_capacity(path.len() + 8);
    result.push_str("file://");
    if !path.starts_with('/') {
        result.push('/');
    }
    for character in path.chars() {
        match character {
            '\\' => result.push('/'),
            ' ' => result.push_str("%20"),
            '#' => result.push_str("%23"),
            '?' => result.push_str("%3F"),
            '%' => result.push_str("%25"),
            character => result.push(character),
        }
    }
    if result.len() > get_max_input_length() as usize {
        String::new()
    } else {
        result
    }
}

impl ParseTarget for Url {
    fn parse_target(input: &str, base: Option<&Self>) -> Result<Self> {
        let aggregate = UrlAggregator::parse_target(input, base.map(|base| &base.aggregate))?;
        Ok(Self { aggregate })
    }
}

impl ParseTarget for UrlAggregator {
    fn parse_target(input: &str, base: Option<&Self>) -> Result<Self> {
        if base.is_none() {
            if let Some(parsed) = Self::try_fast_absolute(input) {
                return parsed;
            }
        }
        let record = match base {
            Some(base) => {
                let base_record =
                    legacy::Url::parse(base.get_href()).map_err(|()| Error::TypeError)?;
                legacy::Url::parse_with_base(input, &base_record)
            }
            None => legacy::Url::parse(input),
        }
        .map_err(|()| Error::TypeError)?;
        Self::from_record(&record)
    }
}

impl UrlAggregator {
    /// Parse an absolute URL.
    pub fn parse(input: &str) -> Result<Self> {
        parse(input, None)
    }

    /// Parse a URL relative to `base`.
    pub fn parse_with_base(input: &str, base: &Self) -> Result<Self> {
        parse(input, Some(base))
    }

    fn from_record(record: &legacy::Url) -> Result<Self> {
        Self::from_normalized(record.href(), record.cannot_be_a_base())
    }

    /// Parse an already-canonical ASCII special URL in one pass and one
    /// allocation. Inputs needing normalization fall through to the complete
    /// WHATWG state machine.
    fn try_fast_absolute(input: &str) -> Option<Result<Self>> {
        let bytes = input.as_bytes();
        if bytes.len() > get_max_input_length() as usize
            || bytes.len() > u32::MAX as usize
            || bytes.len() < 8
        {
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
                return Some(Err(Error::TypeError));
            };
            if port == default_port {
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

    fn from_normalized(buffer: String, opaque_path: bool) -> Result<Self> {
        if buffer.len() > get_max_input_length() as usize || buffer.len() > u32::MAX as usize {
            return Err(Error::TypeError);
        }

        let bytes = buffer.as_bytes();
        let protocol_end = bytes
            .iter()
            .position(|&byte| byte == b':')
            .ok_or(Error::TypeError)?;
        let after_protocol = protocol_end + 1;
        let has_authority = bytes.get(after_protocol..after_protocol + 2) == Some(b"//");
        let hash_start = bytes
            .iter()
            .position(|&byte| byte == b'#')
            .unwrap_or(usize::MAX);
        let search_start = bytes[..hash_start.min(bytes.len())]
            .iter()
            .position(|&byte| byte == b'?')
            .unwrap_or(usize::MAX);
        let suffix_start = search_start.min(hash_start).min(bytes.len());

        let mut flags = if opaque_path { FLAG_OPAQUE_PATH } else { 0 };
        let (username_end, host_start, host_end, pathname_start, port, host_type) = if has_authority
        {
            let authority_start = after_protocol + 2;
            let pathname_start = bytes[authority_start..suffix_start]
                .iter()
                .position(|&byte| byte == b'/')
                .map_or(suffix_start, |offset| authority_start + offset);
            let authority = &bytes[authority_start..pathname_start];
            let at = authority.iter().rposition(|&byte| byte == b'@');
            let host_start = at.map_or(authority_start, |offset| authority_start + offset + 1);
            let userinfo_end = at.map(|offset| authority_start + offset);
            let username_end = userinfo_end.map_or(host_start, |end| {
                bytes[authority_start..end]
                    .iter()
                    .position(|&byte| byte == b':')
                    .map_or(end, |offset| authority_start + offset)
            });
            if let Some(end) = userinfo_end {
                if username_end < end {
                    flags |= FLAG_PASSWORD;
                }
            }
            flags |= FLAG_HOSTNAME;

            let host_and_port = &bytes[host_start..pathname_start];
            let (host_end, port) = if host_and_port.starts_with(b"[") {
                let closing = host_and_port
                    .iter()
                    .position(|&byte| byte == b']')
                    .ok_or(Error::TypeError)?;
                let end = host_start + closing + 1;
                let port = if bytes.get(end) == Some(&b':') {
                    parse_port_number(&buffer[end + 1..pathname_start])
                } else {
                    UrlComponents::OMITTED
                };
                (end, port)
            } else if let Some(colon) = host_and_port.iter().rposition(|&byte| byte == b':') {
                let end = host_start + colon;
                let port = parse_port_number(&buffer[end + 1..pathname_start]);
                (end, port)
            } else {
                (pathname_start, UrlComponents::OMITTED)
            };
            let hostname = &buffer[host_start..host_end];
            let host_type = if hostname.starts_with('[') {
                UrlHostType::Ipv6
            } else if hostname
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'.')
                && hostname.bytes().any(|byte| byte == b'.')
            {
                UrlHostType::Ipv4
            } else {
                UrlHostType::Default
            };
            (
                username_end,
                host_start,
                host_end,
                pathname_start,
                port,
                host_type,
            )
        } else {
            let pathname_start = if !opaque_path
                && bytes.get(after_protocol..after_protocol + 3) == Some(b"/./")
                && bytes.get(after_protocol + 3) == Some(&b'/')
            {
                after_protocol + 2
            } else {
                after_protocol
            };
            (
                after_protocol,
                after_protocol,
                after_protocol,
                pathname_start,
                UrlComponents::OMITTED,
                UrlHostType::Default,
            )
        };

        let component = |offset: usize| {
            if offset == usize::MAX {
                UrlComponents::OMITTED
            } else {
                offset as u32
            }
        };

        Ok(Self {
            components: UrlComponents {
                protocol_end: protocol_end as u32,
                username_end: username_end as u32,
                host_start: host_start as u32,
                host_end: host_end as u32,
                port,
                pathname_start: pathname_start as u32,
                search_start: component(search_start),
                hash_start: component(hash_start),
            },
            buffer,
            flags,
            host_type,
        })
    }

    /// Return the full serialized URL without allocating.
    #[inline]
    pub fn get_href(&self) -> &str {
        &self.buffer
    }

    #[inline]
    pub fn get_href_size(&self) -> usize {
        self.buffer.len()
    }

    #[inline]
    pub fn get_protocol(&self) -> &str {
        &self.buffer[..self.components.protocol_end as usize + 1]
    }

    #[inline]
    pub fn get_username(&self) -> &str {
        if !self.has_credentials() {
            ""
        } else {
            let start = self.components.protocol_end as usize + 3;
            &self.buffer[start..self.components.username_end as usize]
        }
    }

    #[inline]
    pub fn get_password(&self) -> &str {
        if self.flags & FLAG_PASSWORD == 0 {
            ""
        } else {
            &self.buffer
                [self.components.username_end as usize + 1..self.components.host_start as usize - 1]
        }
    }

    #[inline]
    pub fn get_host(&self) -> &str {
        if !self.has_hostname() {
            ""
        } else {
            &self.buffer
                [self.components.host_start as usize..self.components.pathname_start as usize]
        }
    }

    #[inline]
    pub fn get_hostname(&self) -> &str {
        if !self.has_hostname() {
            ""
        } else {
            &self.buffer[self.components.host_start as usize..self.components.host_end as usize]
        }
    }

    #[inline]
    pub fn get_port(&self) -> &str {
        if !self.has_port() {
            ""
        } else {
            &self.buffer
                [self.components.host_end as usize + 1..self.components.pathname_start as usize]
        }
    }

    #[inline]
    pub fn get_pathname(&self) -> &str {
        let end = self
            .offset(self.components.search_start)
            .min(self.offset(self.components.hash_start));
        &self.buffer[self.components.pathname_start as usize..end]
    }

    #[inline]
    pub fn get_pathname_length(&self) -> u32 {
        self.get_pathname().len() as u32
    }

    #[inline]
    pub fn get_search(&self) -> &str {
        if !self.has_search() {
            ""
        } else {
            let start = self.components.search_start as usize;
            let end = self.offset(self.components.hash_start);
            if start + 1 == end {
                ""
            } else {
                &self.buffer[start..end]
            }
        }
    }

    #[inline]
    pub fn get_hash(&self) -> &str {
        if !self.has_hash() {
            ""
        } else {
            let start = self.components.hash_start as usize;
            if start + 1 == self.buffer.len() {
                ""
            } else {
                &self.buffer[start..]
            }
        }
    }

    pub fn get_origin(&self) -> String {
        match self.get_protocol() {
            "ftp:" | "http:" | "https:" | "ws:" | "wss:" => {
                let mut origin =
                    String::with_capacity(self.get_protocol().len() + self.get_host().len() + 2);
                origin.push_str(self.get_protocol());
                origin.push_str("//");
                origin.push_str(self.get_host());
                origin
            }
            "blob:" => legacy::Url::parse(self.get_pathname())
                .map_or_else(|()| "null".to_owned(), |url| url.origin()),
            _ => "null".to_owned(),
        }
    }

    #[inline]
    pub fn get_components(&self) -> &UrlComponents {
        &self.components
    }

    #[inline]
    pub fn host_type(&self) -> UrlHostType {
        self.host_type
    }

    #[inline]
    pub fn is_special(&self) -> bool {
        matches!(
            self.get_protocol(),
            "ftp:" | "file:" | "http:" | "https:" | "ws:" | "wss:"
        )
    }

    #[inline]
    pub fn has_credentials(&self) -> bool {
        self.has_hostname()
            && self.components.protocol_end as usize + 3 < self.components.host_start as usize
    }

    #[inline]
    pub fn has_non_empty_username(&self) -> bool {
        !self.get_username().is_empty()
    }

    #[inline]
    pub fn has_non_empty_password(&self) -> bool {
        !self.get_password().is_empty()
    }

    #[inline]
    pub fn has_password(&self) -> bool {
        self.flags & FLAG_PASSWORD != 0
    }

    #[inline]
    pub fn has_hostname(&self) -> bool {
        self.flags & FLAG_HOSTNAME != 0
    }

    #[inline]
    pub fn has_empty_hostname(&self) -> bool {
        self.has_hostname() && self.components.host_start == self.components.host_end
    }

    #[inline]
    pub fn has_port(&self) -> bool {
        self.components.port != UrlComponents::OMITTED
    }

    #[inline]
    pub fn has_search(&self) -> bool {
        self.components.search_start != UrlComponents::OMITTED
    }

    #[inline]
    pub fn has_hash(&self) -> bool {
        self.components.hash_start != UrlComponents::OMITTED
    }

    #[inline]
    pub fn has_opaque_path(&self) -> bool {
        self.flags & FLAG_OPAQUE_PATH != 0
    }

    pub fn has_valid_domain(&self) -> bool {
        valid_domain(self.get_hostname())
    }

    pub fn validate(&self) -> bool {
        let length = self.buffer.len() as u32;
        let components = self.components;
        components.protocol_end < length
            && components.username_end <= components.host_start
            && components.host_start <= components.host_end
            && components.host_end <= components.pathname_start
            && components.pathname_start <= length
            && (components.search_start == UrlComponents::OMITTED
                || components.search_start >= components.pathname_start)
            && (components.hash_start == UrlComponents::OMITTED
                || components.hash_start >= components.pathname_start)
    }

    pub fn clear_port(&mut self) {
        if self.has_port() {
            let start = self.components.host_end as usize;
            self.buffer
                .replace_range(start..self.components.pathname_start as usize, "");
            self.reindex();
        }
    }

    pub fn clear_hash(&mut self) {
        if self.has_hash() {
            self.buffer.truncate(self.components.hash_start as usize);
            self.reindex();
        }
    }

    pub fn clear_search(&mut self) {
        if self.has_search() {
            let start = self.components.search_start as usize;
            let end = self.offset(self.components.hash_start);
            self.buffer.replace_range(start..end, "");
            self.reindex();
        }
    }

    pub fn set_href(&mut self, input: &str) -> bool {
        let Ok(next) = Self::parse(input) else {
            return false;
        };
        *self = next;
        true
    }

    pub fn set_protocol(&mut self, input: &str) -> bool {
        self.mutate("protocol", input)
    }

    pub fn set_username(&mut self, input: &str) -> bool {
        self.mutate("username", input)
    }

    pub fn set_password(&mut self, input: &str) -> bool {
        self.mutate("password", input)
    }

    pub fn set_host(&mut self, input: &str) -> bool {
        self.mutate("host", input)
    }

    pub fn set_hostname(&mut self, input: &str) -> bool {
        self.mutate("hostname", input)
    }

    pub fn set_port(&mut self, input: &str) -> bool {
        self.mutate("port", input)
    }

    pub fn set_pathname(&mut self, input: &str) -> bool {
        self.mutate("pathname", input)
    }

    pub fn set_search(&mut self, input: &str) {
        let _ = self.mutate("search", input);
    }

    pub fn set_hash(&mut self, input: &str) {
        let _ = self.mutate("hash", input);
    }

    /// Return a JSON diagnostic matching Ada's `to_string()` intent.
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        format!(
            concat!(
                "{{\"href\":{:?},\"protocol\":{:?},\"username\":{:?},",
                "\"password\":{:?},\"host\":{:?},\"hostname\":{:?},",
                "\"port\":{:?},\"pathname\":{:?},\"search\":{:?},\"hash\":{:?}}}"
            ),
            self.get_href(),
            self.get_protocol(),
            self.get_username(),
            self.get_password(),
            self.get_host(),
            self.get_hostname(),
            self.get_port(),
            self.get_pathname(),
            self.get_search(),
            self.get_hash()
        )
    }

    fn offset(&self, offset: u32) -> usize {
        if offset == UrlComponents::OMITTED {
            self.buffer.len()
        } else {
            offset as usize
        }
    }

    fn mutate(&mut self, property: &str, input: &str) -> bool {
        let Ok(mut record) = legacy::Url::parse(&self.buffer) else {
            return false;
        };
        let before = record.href();
        record.set(property, input);
        let after = record.href();
        if after == before && !setter_already_matches(&record, property, input) {
            return false;
        }
        let Ok(next) = Self::from_record(&record) else {
            return false;
        };
        *self = next;
        true
    }

    fn reindex(&mut self) {
        let opaque = self.has_opaque_path();
        let buffer = std::mem::take(&mut self.buffer);
        if let Ok(next) = Self::from_normalized(buffer, opaque) {
            *self = next;
        }
    }
}

impl Url {
    /// Parse an absolute URL.
    pub fn parse(input: &str) -> Result<Self> {
        parse(input, None)
    }

    /// Parse a URL relative to `base`.
    pub fn parse_with_base(input: &str, base: &Self) -> Result<Self> {
        parse(input, Some(base))
    }

    pub fn get_href(&self) -> String {
        self.aggregate.get_href().to_owned()
    }

    pub fn get_href_size(&self) -> usize {
        self.aggregate.get_href_size()
    }

    pub fn get_origin(&self) -> String {
        self.aggregate.get_origin()
    }

    pub fn get_protocol(&self) -> String {
        self.aggregate.get_protocol().to_owned()
    }

    pub fn get_username(&self) -> &str {
        self.aggregate.get_username()
    }

    pub fn get_password(&self) -> &str {
        self.aggregate.get_password()
    }

    pub fn get_host(&self) -> String {
        self.aggregate.get_host().to_owned()
    }

    pub fn get_hostname(&self) -> String {
        self.aggregate.get_hostname().to_owned()
    }

    pub fn get_port(&self) -> String {
        self.aggregate.get_port().to_owned()
    }

    pub fn get_pathname(&self) -> &str {
        self.aggregate.get_pathname()
    }

    pub fn get_pathname_length(&self) -> usize {
        self.aggregate.get_pathname().len()
    }

    pub fn get_search(&self) -> String {
        self.aggregate.get_search().to_owned()
    }

    pub fn get_hash(&self) -> String {
        self.aggregate.get_hash().to_owned()
    }

    pub fn has_credentials(&self) -> bool {
        self.aggregate.has_credentials()
    }

    pub fn has_hostname(&self) -> bool {
        self.aggregate.has_hostname()
    }

    pub fn has_empty_hostname(&self) -> bool {
        self.aggregate.has_empty_hostname()
    }

    pub fn has_port(&self) -> bool {
        self.aggregate.has_port()
    }

    pub fn has_search(&self) -> bool {
        self.aggregate.has_search()
    }

    pub fn has_hash(&self) -> bool {
        self.aggregate.has_hash()
    }

    pub fn has_valid_domain(&self) -> bool {
        self.aggregate.has_valid_domain()
    }

    pub fn get_components(&self) -> UrlComponents {
        *self.aggregate.get_components()
    }

    pub fn set_href(&mut self, input: &str) -> bool {
        self.aggregate.set_href(input)
    }

    pub fn set_protocol(&mut self, input: &str) -> bool {
        self.aggregate.set_protocol(input)
    }

    pub fn set_username(&mut self, input: &str) -> bool {
        self.aggregate.set_username(input)
    }

    pub fn set_password(&mut self, input: &str) -> bool {
        self.aggregate.set_password(input)
    }

    pub fn set_host(&mut self, input: &str) -> bool {
        self.aggregate.set_host(input)
    }

    pub fn set_hostname(&mut self, input: &str) -> bool {
        self.aggregate.set_hostname(input)
    }

    pub fn set_port(&mut self, input: &str) -> bool {
        self.aggregate.set_port(input)
    }

    pub fn set_pathname(&mut self, input: &str) -> bool {
        self.aggregate.set_pathname(input)
    }

    pub fn set_search(&mut self, input: &str) {
        self.aggregate.set_search(input);
    }

    pub fn set_hash(&mut self, input: &str) {
        self.aggregate.set_hash(input);
    }
}

fn parse_port_number(input: &str) -> u32 {
    input
        .parse::<u16>()
        .map_or(UrlComponents::OMITTED, u32::from)
}

fn setter_already_matches(url: &legacy::Url, property: &str, input: &str) -> bool {
    let input = input
        .strip_prefix(if property == "protocol" { ':' } else { '\0' })
        .unwrap_or(input);
    match property {
        "protocol" => url.scheme().trim_end_matches(':') == input.trim_end_matches(':'),
        "username" => url.username() == input,
        "password" => url.password() == input,
        "host" => url.host_str().eq_ignore_ascii_case(input),
        "hostname" => url.hostname().eq_ignore_ascii_case(input),
        "port" => url.port_str() == input,
        "pathname" => url.path_str() == input,
        "search" => url.query_str().trim_start_matches('?') == input.trim_start_matches('?'),
        "hash" => url.fragment_str().trim_start_matches('#') == input.trim_start_matches('#'),
        _ => false,
    }
}

fn valid_domain(hostname: &str) -> bool {
    if hostname.starts_with('[') {
        return true;
    }
    hostname.len() <= 255
        && hostname
            .trim_end_matches('.')
            .split('.')
            .all(|label| !label.is_empty() && label.len() <= 63)
}

#[inline]
fn userinfo_byte_is_canonical(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'-' | b'.'
                | b'_'
                | b'~'
                | b'!'
                | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b':'
                | b'%'
        )
}

#[inline]
fn host_byte_is_canonical(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

#[inline]
fn path_byte_is_canonical(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'/' | b'-'
                | b'.'
                | b'_'
                | b'~'
                | b'!'
                | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
                | b':'
                | b'@'
                | b'%'
        )
}

fn canonical_path(path: &[u8]) -> bool {
    let mut segment_start = 0usize;
    for (index, &byte) in path.iter().enumerate() {
        if !path_byte_is_canonical(byte) {
            return false;
        }
        if byte == b'/' {
            if dot_segment(&path[segment_start..index]) {
                return false;
            }
            segment_start = index + 1;
        }
    }
    !dot_segment(&path[segment_start..])
}

fn dot_segment(segment: &[u8]) -> bool {
    matches!(
        segment,
        b"." | b".."
            | b"%2e"
            | b"%2E"
            | b".%2e"
            | b".%2E"
            | b"%2e."
            | b"%2E."
            | b"%2e%2e"
            | b"%2E%2E"
            | b"%2e%2E"
            | b"%2E%2e"
    )
}

#[inline]
fn special_query_byte_is_canonical(byte: u8) -> bool {
    (0x20..=0x7e).contains(&byte) && !matches!(byte, b' ' | b'"' | b'#' | b'<' | b'>' | b'\'')
}

#[inline]
fn fragment_byte_is_canonical(byte: u8) -> bool {
    (0x20..=0x7e).contains(&byte) && !matches!(byte, b' ' | b'"' | b'<' | b'>' | b'`')
}

fn canonical_ipv4(hostname: &str) -> bool {
    let mut count = 0usize;
    for part in hostname.split('.') {
        count += 1;
        if part.is_empty()
            || (part.len() > 1 && part.starts_with('0'))
            || part.parse::<u8>().is_err()
        {
            return false;
        }
    }
    count == 4
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = "https://user:pass@example.com:8080/a/b?query=yes#fragment";

    #[test]
    fn aggregator_getters_match_ada() {
        let url = parse::<UrlAggregator>(EXAMPLE, None).unwrap();
        assert_eq!(url.get_href(), EXAMPLE);
        assert_eq!(url.get_protocol(), "https:");
        assert_eq!(url.get_username(), "user");
        assert_eq!(url.get_password(), "pass");
        assert_eq!(url.get_host(), "example.com:8080");
        assert_eq!(url.get_hostname(), "example.com");
        assert_eq!(url.get_port(), "8080");
        assert_eq!(url.get_pathname(), "/a/b");
        assert_eq!(url.get_search(), "?query=yes");
        assert_eq!(url.get_hash(), "#fragment");
        assert!(url.validate());
    }

    #[test]
    fn generic_parse_supports_both_representations() {
        let aggregate = parse::<UrlAggregator>("HTTPS://EXAMPLE.COM", None).unwrap();
        let mutable = parse::<Url>("HTTPS://EXAMPLE.COM", None).unwrap();
        assert_eq!(aggregate.get_href(), "https://example.com/");
        assert_eq!(mutable.get_href(), "https://example.com/");
    }

    #[test]
    fn resolves_against_a_base() {
        let base = parse::<UrlAggregator>("https://example.com/a/b", None).unwrap();
        let resolved = parse::<UrlAggregator>("../c?x", Some(&base)).unwrap();
        assert_eq!(resolved.get_href(), "https://example.com/c?x");
    }

    #[test]
    fn setters_update_offsets() {
        let mut url = UrlAggregator::parse(EXAMPLE).unwrap();
        assert!(url.set_hostname("example.org"));
        assert!(url.set_port("443"));
        url.set_hash("");
        assert_eq!(
            url.get_href(),
            "https://user:pass@example.org/a/b?query=yes"
        );
        assert_eq!(url.get_hostname(), "example.org");
        assert_eq!(url.get_port(), "");
        assert!(!url.has_hash());
        assert!(url.validate());
    }

    #[test]
    fn invalid_urls_are_rejected() {
        assert_eq!(
            parse::<UrlAggregator>("not a url", None),
            Err(Error::TypeError)
        );
        assert!(!can_parse("not a url", None));
    }

    #[test]
    fn fast_path_matches_the_general_state_machine() {
        let inputs = [
            "https://example.com/",
            "https://user:password@example.com:8080/path/to/resource?query=value#fragment",
            "http://www.example.org/a/b/c?one=1&two=2",
            "https://subdomain.example.co.uk/products/12345",
            "ftp://ftp.example.com/pub/file.txt",
            "ws://localhost:3000/socket",
            "https://127.0.0.1:8443/api/v1/health",
            "https://example.com/a%20path?q=already%20encoded",
        ];
        for input in inputs {
            let fast = UrlAggregator::parse(input).unwrap();
            let general = legacy::Url::parse(input).unwrap();
            assert_eq!(fast.get_href(), general.href());
            assert_eq!(fast.get_protocol(), format!("{}:", general.scheme()));
            assert_eq!(fast.get_username(), general.username());
            assert_eq!(fast.get_password(), general.password());
            assert_eq!(fast.get_host(), general.host_str());
            assert_eq!(fast.get_hostname(), general.hostname());
            assert_eq!(fast.get_port(), general.port_str());
            assert_eq!(fast.get_pathname(), general.path_str());
            assert_eq!(fast.get_search(), general.query_str());
            assert_eq!(fast.get_hash(), general.fragment_str());
        }
    }

    #[test]
    fn preserves_empty_component_semantics() {
        let url = UrlAggregator::parse("https://example.com/path?#").unwrap();
        assert_eq!(url.get_href(), "https://example.com/path?#");
        assert_eq!(url.get_search(), "");
        assert_eq!(url.get_hash(), "");
        assert!(url.has_search());
        assert!(url.has_hash());

        let credentials = UrlAggregator::parse("https://test:@example.com").unwrap();
        assert_eq!(credentials.get_href(), "https://test@example.com/");
        assert_eq!(credentials.get_username(), "test");
        assert_eq!(credentials.get_password(), "");
    }

    #[test]
    fn handles_wpt_fast_path_boundaries() {
        assert_eq!(
            UrlAggregator::parse("http://0x7f.1/").unwrap().get_href(),
            "http://127.0.0.1/"
        );
        assert!(UrlAggregator::parse("http://0999999999999999999/").is_err());
        assert_eq!(
            UrlAggregator::parse("non-spec:/a/..//path")
                .unwrap()
                .get_pathname(),
            "//path"
        );
        assert_eq!(
            UrlAggregator::parse("http://bücher.example/straße")
                .unwrap()
                .get_href(),
            "http://xn--bcher-kva.example/stra%C3%9Fe"
        );
        assert_eq!(
            UrlAggregator::parse("http://xn--/").unwrap().get_hostname(),
            "xn--"
        );
        assert!(UrlAggregator::parse("http://xn--a-ä.pt/").is_err());
    }
}
