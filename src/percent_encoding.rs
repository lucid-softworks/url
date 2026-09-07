//! Percent encoding for normalized URL components.

use crate::BYTE_CLASSES;

#[derive(Clone, Copy)]
pub(crate) enum EncodeSet {
    C0,
    Fragment,
    Query,
    SpecialQuery,
    Path,
    UserInfo,
}

#[inline(always)]
pub(crate) fn push_encoded(output: &mut String, character: char, set: EncodeSet) {
    if matches!(character, '\t' | '\n' | '\r') {
        return;
    }
    let encode = match set {
        EncodeSet::C0 => character <= '\u{1f}' || character > '\u{7e}',
        EncodeSet::Fragment => {
            character <= '\u{1f}'
                || character > '\u{7e}'
                || matches!(character, ' ' | '"' | '<' | '>' | '`')
        }
        EncodeSet::Query => {
            character <= '\u{1f}'
                || character > '\u{7e}'
                || matches!(character, ' ' | '"' | '#' | '<' | '>')
        }
        EncodeSet::SpecialQuery => {
            character <= '\u{1f}'
                || character > '\u{7e}'
                || matches!(character, ' ' | '"' | '#' | '<' | '>' | '\'')
        }
        EncodeSet::Path => {
            character <= '\u{1f}'
                || character > '\u{7e}'
                || matches!(
                    character,
                    ' ' | '"' | '#' | '<' | '>' | '?' | '`' | '{' | '}' | '^'
                )
        }
        EncodeSet::UserInfo => {
            character <= '\u{1f}'
                || character > '\u{7e}'
                || matches!(
                    character,
                    ' ' | '"'
                        | '#'
                        | '/'
                        | ':'
                        | ';'
                        | '<'
                        | '='
                        | '>'
                        | '?'
                        | '@'
                        | '['
                        | '\\'
                        | ']'
                        | '^'
                        | '`'
                        | '{'
                        | '|'
                        | '}'
                )
        }
    };
    if !encode {
        output.push(character);
        return;
    }

    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut utf8 = [0u8; 4];
    for byte in character.encode_utf8(&mut utf8).bytes() {
        output.push('%');
        output.push(char::from(HEX[(byte >> 4) as usize]));
        output.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
}

pub(crate) fn push_encoded_runs(output: &mut String, input: &str, set: EncodeSet, class: u8) {
    // Non-ASCII bytes are always escaped. Copied spans therefore contain only
    // ASCII, so their endpoints are also valid UTF-8 boundaries.
    let mut run_start = 0usize;
    for (index, byte) in input.bytes().enumerate() {
        let skipped = matches!(byte, b'\t' | b'\n' | b'\r');
        if !skipped && BYTE_CLASSES[byte as usize] & class != 0 {
            continue;
        }
        if run_start < index {
            output.push_str(&input[run_start..index]);
        }
        if !skipped {
            if byte.is_ascii() {
                push_encoded(output, char::from(byte), set);
            } else {
                const HEX: &[u8; 16] = b"0123456789ABCDEF";
                output.push('%');
                output.push(char::from(HEX[(byte >> 4) as usize]));
                output.push(char::from(HEX[(byte & 0x0f) as usize]));
            }
        }
        run_start = index + 1;
    }
    if run_start < input.len() {
        output.push_str(&input[run_start..]);
    }
}

#[cfg(test)]
mod tests {
    use super::{EncodeSet, push_encoded, push_encoded_runs};
    use crate::{BYTE_FRAGMENT, BYTE_SPECIAL_QUERY, BYTE_USERINFO};

    #[test]
    fn byte_runs_match_scalar_encoding_for_every_unicode_scalar() {
        for (set, class) in [
            (EncodeSet::Fragment, BYTE_FRAGMENT),
            (EncodeSet::SpecialQuery, BYTE_SPECIAL_QUERY),
            (EncodeSet::UserInfo, BYTE_USERINFO),
        ] {
            let mut input = String::new();
            let mut expected = String::new();
            let mut actual = String::new();
            for character in (0..=0x10ffff).filter_map(char::from_u32) {
                input.clear();
                input.push_str("ascii:");
                input.push(character);
                input.push_str("/tail");
                expected.clear();
                actual.clear();
                for character in input.chars() {
                    push_encoded(&mut expected, character, set);
                }
                push_encoded_runs(&mut actual, &input, set, class);
                assert_eq!(actual, expected, "U+{:04X}", character as u32);
            }
        }
    }

    #[test]
    fn mixed_runs_preserve_prefix_and_skip_url_whitespace() {
        let mut output = String::from("prefix:");
        push_encoded_runs(
            &mut output,
            "abcé日本😀\t\n\rxyz 'end",
            EncodeSet::SpecialQuery,
            BYTE_SPECIAL_QUERY,
        );
        assert_eq!(
            output,
            "prefix:abc%C3%A9%E6%97%A5%E6%9C%AC%F0%9F%98%80xyz%20%27end"
        );
    }
}
