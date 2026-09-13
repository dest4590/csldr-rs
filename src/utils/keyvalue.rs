#[derive(Debug, Clone)]
pub enum KeyValueValue {
    String(String),
    Subkeys(Vec<KeyValueNode>),
}

#[derive(Debug, Clone)]
pub struct KeyValueNode {
    pub name: String,
    pub value: KeyValueValue,
}

impl KeyValueNode {
    pub fn parse(input: &str) -> Option<Vec<KeyValueNode>> {
        let mut chars = input.chars().peekable();
        Self::parse_block(&mut chars)
    }

    fn skip_white<I: Iterator<Item = char>>(chars: &mut std::iter::Peekable<I>) {
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() {
                chars.next();
            } else if c == '/' {
                chars.next();
                if chars.peek() == Some(&'/') {
                    chars.next();
                    while let Some(&ch) = chars.peek() {
                        chars.next();
                        if ch == '\n' || ch == '\r' {
                            break;
                        }
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    fn parse_string<I: Iterator<Item = char>>(
        chars: &mut std::iter::Peekable<I>,
    ) -> Option<String> {
        Self::skip_white(chars);
        let first = chars.peek()?;
        if *first == '"' {
            chars.next();
            let mut s = String::new();
            for c in chars.by_ref() {
                if c == '"' {
                    return Some(s);
                }
                s.push(c);
            }
            Some(s)
        } else {
            let mut s = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() || c == '{' || c == '}' || c == '/' {
                    break;
                }
                s.push(c);
                chars.next();
            }
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
    }

    fn parse_block<I: Iterator<Item = char>>(
        chars: &mut std::iter::Peekable<I>,
    ) -> Option<Vec<KeyValueNode>> {
        let mut nodes = Vec::new();

        loop {
            Self::skip_white(chars);
            if let Some(&c) = chars.peek() {
                if c == '}' {
                    chars.next();
                    break;
                }
            } else {
                break;
            }

            let name = match Self::parse_string(chars) {
                Some(name) => name,
                None => break,
            };

            Self::skip_white(chars);
            if let Some(&'{') = chars.peek() {
                chars.next();
                let subkeys = Self::parse_block(chars)?;
                nodes.push(KeyValueNode {
                    name,
                    value: KeyValueValue::Subkeys(subkeys),
                });
            } else {
                let val_str = Self::parse_string(chars)?;
                nodes.push(KeyValueNode {
                    name,
                    value: KeyValueValue::String(val_str),
                });
            }
        }

        Some(nodes)
    }
}
