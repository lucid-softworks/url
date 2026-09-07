//! Credential replacement with one movement of the unchanged URL tail.
use crate::{
    BYTE_CLASSES, BYTE_USERINFO, FLAG_PASSWORD, UrlAggregator, UrlComponents, get_max_input_length,
};

impl UrlAggregator {
    pub fn set_username(&mut self, input: &str) -> bool {
        self.replace_credential(input, false)
    }

    pub fn set_password(&mut self, input: &str) -> bool {
        self.replace_credential(input, true)
    }

    fn replace_credential(&mut self, input: &str, password: bool) -> bool {
        if !self.is_special() || self.get_protocol() == "file:" || self.has_opaque_path() {
            return self.mutate(if password { "password" } else { "username" }, input);
        }
        let start = self.components.protocol_end as usize + 3;
        let end = self.components.host_start as usize;
        let mut credentials = String::with_capacity(input.len());
        if password {
            credentials.push_str(self.get_username());
        } else {
            encode_credential(&mut credentials, input);
        }
        let username_end = start + credentials.len();
        let has_password = if password {
            !input.is_empty()
        } else {
            !self.get_password().is_empty()
        };
        if has_password {
            credentials.push(':');
            if password {
                encode_credential(&mut credentials, input);
            } else {
                credentials.push_str(self.get_password());
            }
        }
        if !credentials.is_empty() {
            credentials.push('@');
        }
        let Some(length) = (self.buffer.len() - (end - start)).checked_add(credentials.len())
        else {
            return false;
        };
        if length > get_max_input_length() as usize {
            return false;
        }
        self.buffer.replace_range(start..end, &credentials);
        let shift = |offset: u32| offset - end as u32 + (start + credentials.len()) as u32;
        self.components.username_end = username_end as u32;
        self.components.host_start = shift(self.components.host_start);
        self.components.host_end = shift(self.components.host_end);
        self.components.pathname_start = shift(self.components.pathname_start);
        if self.components.search_start != UrlComponents::OMITTED {
            self.components.search_start = shift(self.components.search_start);
        }
        if self.components.hash_start != UrlComponents::OMITTED {
            self.components.hash_start = shift(self.components.hash_start);
        }
        self.flags = (self.flags & !FLAG_PASSWORD) | if has_password { FLAG_PASSWORD } else { 0 };
        true
    }
}

fn encode_credential(output: &mut String, input: &str) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    // Unlike parser input, setter credentials retain tabs/newlines as escapes.
    for byte in input.bytes() {
        if BYTE_CLASSES[byte as usize] & BYTE_USERINFO != 0 {
            output.push(char::from(byte));
        } else {
            output.push('%');
            output.push(char::from(HEX[(byte >> 4) as usize]));
            output.push(char::from(HEX[(byte & 15) as usize]));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::UrlAggregator;

    #[test]
    fn credentials_match_general_setters_through_repeated_edits() {
        let mut values = vec![
            String::new(),
            "alice".into(),
            "longer-user-name".into(),
            "%20".into(),
            "日本語😀".into(),
        ];
        values.extend((0..=127).map(|byte| format!("a{}z", char::from(byte))));
        values.push(String::new());
        for initial in [
            "https://example.com/",
            "https://u:p@example.com:8080/a?q#h",
            "http://:p@[::1]/?#",
            "ftp://u@example.com/a#h",
            "ws://host/a?q",
            "file:///path",
            "custom://u:p@host/path",
            "data:opaque",
            "custom:///path",
        ] {
            let mut actual = UrlAggregator::parse(initial).unwrap();
            let mut expected = actual.clone();
            for value in &values {
                for password in [false, true] {
                    let property = if password { "password" } else { "username" };
                    let result = if password {
                        actual.set_password(value)
                    } else {
                        actual.set_username(value)
                    };
                    assert_eq!(
                        result,
                        expected.mutate(property, value),
                        "{initial} {property} {value:?}"
                    );
                    assert_eq!(actual, expected, "{initial} {property} {value:?}");
                }
            }
            // Remove each side independently, including the password-only form.
            for (password, value) in [
                (true, "pass"),
                (false, ""),
                (true, ""),
                (false, "user"),
                (true, "pass"),
                (true, ""),
                (false, ""),
            ] {
                let property = if password { "password" } else { "username" };
                let result = if password {
                    actual.set_password(value)
                } else {
                    actual.set_username(value)
                };
                assert_eq!(result, expected.mutate(property, value));
                assert_eq!(actual, expected);
            }
        }
    }
}
