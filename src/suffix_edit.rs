//! Query and fragment replacement without reparsing unaffected components.
use crate::percent_encoding::{EncodeSet, push_encoded_runs};
use std::borrow::Cow;

use crate::{
    BYTE_CLASSES, BYTE_FRAGMENT, BYTE_SPECIAL_QUERY, UrlAggregator, UrlComponents,
    get_max_input_length,
};

impl UrlAggregator {
    pub fn set_search(&mut self, input: &str) {
        if !self.is_special() || self.has_opaque_path() {
            let _ = self.mutate("search", input);
            return;
        }
        let end = self.offset(self.components.hash_start);
        let start = if self.has_search() {
            self.components.search_start as usize
        } else {
            end
        };
        let encoded = encode_suffix(input, '?', EncodeSet::SpecialQuery, BYTE_SPECIAL_QUERY);
        if !self.replace_suffix_range(start, end, &encoded) {
            return;
        }
        self.components.search_start = if input.is_empty() {
            UrlComponents::OMITTED
        } else {
            start as u32
        };
        if self.has_hash() {
            self.components.hash_start = (start + encoded.len()) as u32;
        }
    }

    pub fn set_hash(&mut self, input: &str) {
        if !self.is_special() || self.has_opaque_path() {
            let _ = self.mutate("hash", input);
            return;
        }
        let start = self.offset(self.components.hash_start);
        let encoded = encode_suffix(input, '#', EncodeSet::Fragment, BYTE_FRAGMENT);
        if !self.replace_suffix_range(start, self.buffer.len(), &encoded) {
            return;
        }
        self.components.hash_start = if input.is_empty() {
            UrlComponents::OMITTED
        } else {
            start as u32
        };
    }

    fn replace_suffix_range(&mut self, start: usize, end: usize, encoded: &str) -> bool {
        let Some(length) = (self.buffer.len() - (end - start)).checked_add(encoded.len()) else {
            return false;
        };
        if length > get_max_input_length() as usize {
            return false;
        }
        self.buffer.replace_range(start..end, encoded);
        true
    }
}

fn encode_suffix(input: &str, delimiter: char, set: EncodeSet, class: u8) -> Cow<'_, str> {
    if input.is_empty() {
        return Cow::Borrowed("");
    }
    if let Some(content) = input.strip_prefix(delimiter)
        && content
            .bytes()
            .all(|byte| BYTE_CLASSES[usize::from(byte)] & class != 0)
    {
        return Cow::Borrowed(input);
    }
    let mut output = String::new();
    if !input.is_empty() {
        output.reserve(input.len().saturating_add(1));
        output.push(delimiter);
        push_encoded_runs(
            &mut output,
            input.strip_prefix(delimiter).unwrap_or(input),
            set,
            class,
        );
    }
    Cow::Owned(output)
}

#[cfg(test)]
mod tests {
    use crate::{UrlAggregator, legacy};

    #[test]
    fn suffix_edits_match_general_parser_and_preserve_offsets() {
        for initial in [
            "https://user:pass@example.com:8080/path?q=old#fragment",
            "http://example.com/",
            "file:///C:/path?old#fragment",
            "ftp://example.com/path?#",
            "ws://[::1]/socket",
            "custom://host/path?q=old#fragment",
            "data:opaque path  ?old#fragment",
        ] {
            let mut actual = UrlAggregator::parse(initial).unwrap();
            let mut expected = legacy::Url::parse(initial).unwrap();
            for query in [
                "",
                "?",
                "??",
                "?a=1",
                "longer query='日本語'",
                "\t\n\r",
                "a#b",
                "?a=%20",
                "?😀",
            ] {
                actual.set_search(query);
                expected.set("search", query);
                assert_eq!(
                    actual.get_href(),
                    expected.href(),
                    "{initial} query={query:?}"
                );
                for fragment in [
                    "",
                    "#",
                    "##",
                    "short",
                    "long fragment 日本語",
                    "#😀\t\n\r",
                    "#a`b\"<c>",
                ] {
                    actual.set_hash(fragment);
                    expected.set("hash", fragment);
                    assert_eq!(
                        actual.get_href(),
                        expected.href(),
                        "{initial} fragment={fragment:?}"
                    );
                    let indexed = UrlAggregator::from_record(&expected).unwrap();
                    assert_eq!(actual.get_components(), indexed.get_components());
                    assert_eq!(actual.get_pathname(), indexed.get_pathname());
                    assert_eq!(actual.get_search(), indexed.get_search());
                    assert_eq!(actual.get_hash(), indexed.get_hash());
                }
            }
        }
    }

    #[test]
    fn every_ascii_byte_and_unicode_preserves_query_and_fragment_encoding() {
        for character in (0..=127)
            .filter_map(char::from_u32)
            .chain(['é', '日', '😀', '\u{2028}'])
        {
            let input = format!("prefix{character}tail");
            for property in ["search", "hash"] {
                let initial = "https://example.com/path?q=old#fragment";
                let mut expected = legacy::Url::parse(initial).unwrap();
                expected.set(property, &input);
                let mut actual = UrlAggregator::parse(initial).unwrap();
                if property == "search" {
                    actual.set_search(&input);
                } else {
                    actual.set_hash(&input);
                }
                assert_eq!(actual.get_href(), expected.href(), "{property} {input:?}");
            }
        }
    }
}
