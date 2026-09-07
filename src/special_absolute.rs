//! Normalization of absolute special URLs into the serialized buffer.
use crate::host_normalization::push_normalized_special_host;
use crate::percent_encoding::{EncodeSet, push_encoded_runs};
use crate::{
    BYTE_FRAGMENT, BYTE_SPECIAL_QUERY, BYTE_USERINFO, Error, FLAG_HOSTNAME, FLAG_PASSWORD, Result,
    SpecialScheme, UrlAggregator, UrlComponents, UrlHostType, get_max_input_length, legacy,
    push_normalized_path, split_host_port, split_path_query_fragment,
};

impl UrlAggregator {
    pub(crate) fn normalize_special_absolute(
        input: &str,
        colon: usize,
        scheme: SpecialScheme,
        authority_end: Option<usize>,
    ) -> Result<Self> {
        let is_file = scheme == SpecialScheme::File;
        let bytes = input.as_bytes();
        let mut authority_start = colon + 1;
        if !matches!(bytes.get(authority_start), Some(b'/' | b'\\'))
            || !matches!(bytes.get(authority_start + 1), Some(b'/' | b'\\'))
        {
            return Err(Error::TypeError);
        }
        authority_start += 2;
        if !is_file {
            while matches!(bytes.get(authority_start), Some(b'/' | b'\\')) {
                authority_start += 1;
            }
        }

        let authority_end = authority_end.unwrap_or_else(|| {
            bytes[authority_start..]
                .iter()
                .position(|&byte| matches!(byte, b'/' | b'\\' | b'?' | b'#'))
                .map_or(bytes.len(), |offset| authority_start + offset)
        });
        let authority = &input[authority_start..authority_end];
        let at = authority.as_bytes().iter().rposition(|&byte| byte == b'@');
        let (userinfo, host_port) = at.map_or((None, authority), |offset| {
            (
                Some(&authority[..offset]),
                &authority[offset.saturating_add(1)..],
            )
        });
        if host_port.is_empty() && !is_file {
            return Err(Error::TypeError);
        }

        let (host_input, port_input) = split_host_port(host_port)?;
        if is_file && port_input.is_some() {
            return Err(Error::TypeError);
        }
        if is_file && !host_input.is_empty() {
            return legacy::Url::parse(input)
                .map_err(|()| Error::TypeError)
                .and_then(|record| Self::from_record(&record));
        }

        let mut buffer = String::with_capacity(input.len().saturating_add(16));
        buffer.push_str(scheme.name());
        let protocol_end = buffer.len() as u32;
        buffer.push_str("://");

        let mut password_present = false;
        let mut username_end = buffer.len();
        if let Some(userinfo) = userinfo {
            let (username, password) = userinfo
                .split_once(':')
                .map_or((userinfo, None), |(username, password)| {
                    (username, Some(password))
                });
            if !username.is_empty() || password.is_some_and(|password| !password.is_empty()) {
                push_encoded_runs(&mut buffer, username, EncodeSet::UserInfo, BYTE_USERINFO);
                username_end = buffer.len();
                if let Some(password) = password.filter(|password| !password.is_empty()) {
                    password_present = true;
                    buffer.push(':');
                    push_encoded_runs(&mut buffer, password, EncodeSet::UserInfo, BYTE_USERINFO);
                }
                buffer.push('@');
            }
        }
        let host_start = buffer.len();
        if username_end < protocol_end as usize + 3 {
            username_end = host_start;
        }

        let host_type = if host_input.is_empty() {
            UrlHostType::Default
        } else {
            push_normalized_special_host(&mut buffer, host_input)?
        };
        let host_end = buffer.len();

        let mut port = UrlComponents::OMITTED;
        if let Some(port_input) = port_input {
            let parsed_port = port_input.parse::<u16>().map_err(|_| Error::TypeError)?;
            let default_port = scheme.default_port();
            if parsed_port != default_port {
                buffer.push(':');
                let digits = port_input.trim_start_matches('0');
                buffer.push_str(if digits.is_empty() { "0" } else { digits });
                port = u32::from(parsed_port);
            }
        }

        let (path, query, fragment) = split_path_query_fragment(&input[authority_end..]);
        if path
            .as_bytes()
            .windows(2)
            .any(|pair| matches!(pair[0], b'/' | b'\\') && matches!(pair[1], b'/' | b'\\'))
        {
            return legacy::Url::parse(input)
                .map_err(|()| Error::TypeError)
                .and_then(|record| Self::from_record(&record));
        }
        let pathname_start = buffer.len() as u32;
        push_normalized_path(&mut buffer, path, is_file)?;
        let mut search_start = UrlComponents::OMITTED;
        if let Some(query) = query {
            search_start = buffer.len() as u32;
            buffer.push('?');
            push_encoded_runs(
                &mut buffer,
                query,
                EncodeSet::SpecialQuery,
                BYTE_SPECIAL_QUERY,
            );
        }
        let mut hash_start = UrlComponents::OMITTED;
        if let Some(fragment) = fragment {
            hash_start = buffer.len() as u32;
            buffer.push('#');
            push_encoded_runs(&mut buffer, fragment, EncodeSet::Fragment, BYTE_FRAGMENT);
        }

        if buffer.len() > get_max_input_length() as usize || buffer.len() > u32::MAX as usize {
            return Err(Error::TypeError);
        }
        Ok(Self {
            components: UrlComponents {
                protocol_end,
                username_end: username_end as u32,
                host_start: host_start as u32,
                host_end: host_end as u32,
                port,
                pathname_start,
                search_start,
                hash_start,
            },
            buffer,
            flags: FLAG_HOSTNAME | if password_present { FLAG_PASSWORD } else { 0 },
            host_type,
        })
    }
}
