//! A small JSON reader and writer.
//!
//! The language server speaks JSON, and the compiler has no other need for
//! it, so the reading and the writing of it live here.

/// A value of JSON.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    Text(String),
    List(Vec<Value>),
    /// The keys of an object are kept in the order they were written, so
    /// that a message may be read as it was sent.
    Object(Vec<(String, Value)>),
}

impl Value {
    /// The value of a key, when the value is an object that has it.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(items) => items
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    /// The value of a key, as a number.
    pub fn number(&self, key: &str) -> Option<f64> {
        match self.get(key) {
            Some(Value::Number(value)) => Some(*value),
            _ => None,
        }
    }

    /// The value of a key, as a text.
    pub fn text(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(Value::Text(value)) => Some(value.as_str()),
            _ => None,
        }
    }

    /// The value of a key, as a list.
    pub fn list(&self, key: &str) -> Option<&Vec<Value>> {
        match self.get(key) {
            Some(Value::List(values)) => Some(values),
            _ => None,
        }
    }

    /// Write the value as JSON.
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            Value::Null => out.push_str("null"),
            Value::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
            Value::Number(value) => {
                if value.fract() == 0.0 && value.abs() < 9_007_199_254_740_991.0 {
                    out.push_str(&format!("{}", *value as i64));
                } else {
                    out.push_str(&format!("{}", value));
                }
            }
            Value::Text(value) => write_text(value, out),
            Value::List(values) => {
                out.push('[');
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    value.write(out);
                }
                out.push(']');
            }
            Value::Object(items) => {
                out.push('{');
                for (index, (key, value)) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    write_text(key, out);
                    out.push(':');
                    value.write(out);
                }
                out.push('}');
            }
        }
    }
}

/// A text of JSON, with the marks that it holds.
fn write_text(value: &str, out: &mut String) {
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// An object of JSON, written from the pairs.
pub fn object(items: Vec<(&str, Value)>) -> Value {
    Value::Object(
        items
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

/// A text of JSON.
pub fn text(value: &str) -> Value {
    Value::Text(value.to_string())
}

/// A number of JSON.
pub fn number(value: f64) -> Value {
    Value::Number(value)
}

/// Read a text of JSON.
///
/// The value that is read is the first one of the text, and the text that is
/// left is returned with it.
pub fn read(source: &str) -> Option<(Value, &str)> {
    let mut rest = source;
    read_value(&mut rest)
}

/// Read four marks of sixteen, and the number they make.
///
/// The marks are read one by one, because a mark of a text may be more
/// than one byte. A text that is not four marks of sixteen is refused.
fn read_hex(text: &str) -> Option<(u32, usize)> {
    let mut digits = String::new();
    let mut taken = 0;
    for c in text.chars() {
        if digits.len() >= 4 {
            break;
        }
        if !c.is_ascii_hexdigit() {
            break;
        }
        digits.push(c);
        taken += c.len_utf8();
    }
    if digits.len() < 4 {
        return None;
    }
    let code = u32::from_str_radix(&digits, 16).ok()?;
    Some((code, taken))
}

/// Read one value of JSON, and the text that is left.
fn read_value<'a>(chars: &mut &'a str) -> Option<(Value, &'a str)> {
    let trimmed = chars.trim_start();
    let mut iter = trimmed.chars();
    let c = iter.next()?;
    match c {
        '{' => {
            let mut items = Vec::new();
            let mut rest = iter.as_str();
            loop {
                rest = rest.trim_start();
                if rest.starts_with('}') {
                    *chars = &rest[1..];
                    break;
                }
                let (key, tail) = read_value(&mut rest)?;
                let Value::Text(key) = key else {
                    return None;
                };
                let tail = tail.trim_start();
                if !tail.starts_with(':') {
                    return None;
                }
                let mut tail = &tail[1..];
                let (value, tail) = read_value(&mut tail)?;
                items.push((key, value));
                rest = tail.trim_start();
                if rest.starts_with(',') {
                    rest = &rest[1..];
                } else if rest.starts_with('}') {
                    *chars = &rest[1..];
                    break;
                } else {
                    return None;
                }
            }
            Some((Value::Object(items), chars))
        }
        '[' => {
            let mut values = Vec::new();
            let mut rest = iter.as_str();
            loop {
                rest = rest.trim_start();
                if rest.starts_with(']') {
                    *chars = &rest[1..];
                    break;
                }
                let (value, tail) = read_value(&mut rest)?;
                values.push(value);
                rest = tail.trim_start();
                if rest.starts_with(',') {
                    rest = &rest[1..];
                } else if rest.starts_with(']') {
                    *chars = &rest[1..];
                    break;
                } else {
                    return None;
                }
            }
            Some((Value::List(values), chars))
        }
        '"' => {
            let mut text = String::new();
            let mut rest = iter.as_str();
            loop {
                let Some(c) = rest.chars().next() else {
                    return None;
                };
                rest = &rest[c.len_utf8()..];
                match c {
                    '"' => {
                        *chars = rest;
                        break;
                    }
                    '\\' => {
                        let Some(escaped) = rest.chars().next() else {
                            return None;
                        };
                        rest = &rest[escaped.len_utf8()..];
                        match escaped {
                            'n' => text.push('\n'),
                            't' => text.push('\t'),
                            'r' => text.push('\r'),
                            'b' => text.push('\u{8}'),
                            'f' => text.push('\u{c}'),
                            'u' => {
                                // The number is four marks of sixteen. A
                                // mark may be more than one byte, so the
                                // marks are read one by one and a text
                                // that is not a number is refused instead
                                // of taking the server down.
                                let Some((code, taken)) = read_hex(rest) else {
                                    return None;
                                };
                                rest = &rest[taken..];
                                // A pair of marks that is a letter outside
                                // the plane of the text is one letter.
                                if (0xd800..0xdc00).contains(&code) {
                                    if !rest.starts_with("\\u") {
                                        return None;
                                    }
                                    let Some((low, more)) = read_hex(&rest[2..]) else {
                                        return None;
                                    };
                                    if !(0xdc00..0xe000).contains(&low) {
                                        return None;
                                    }
                                    rest = &rest[2 + more..];
                                    let together = 0x10000 + ((code - 0xd800) << 10) + (low - 0xdc00);
                                    text.push(char::from_u32(together)?);
                                } else {
                                    text.push(char::from_u32(code)?);
                                }
                            }
                            other => text.push(other),
                        }
                    }
                    other => text.push(other),
                }
            }
            Some((Value::Text(text), chars))
        }
        't' => {
            if !trimmed.starts_with("true") {
                return None;
            }
            *chars = &trimmed[4..];
            Some((Value::Bool(true), chars))
        }
        'f' => {
            if !trimmed.starts_with("false") {
                return None;
            }
            *chars = &trimmed[5..];
            Some((Value::Bool(false), chars))
        }
        'n' => {
            if !trimmed.starts_with("null") {
                return None;
            }
            *chars = &trimmed[4..];
            Some((Value::Null, chars))
        }
        _ => {
            let mut end = 0usize;
            for (index, c) in trimmed.char_indices() {
                if c.is_ascii_digit() || c == '-' || c == '+' || c == '.' || c == 'e' || c == 'E' {
                    end = index + c.len_utf8();
                } else {
                    break;
                }
            }
            let text = &trimmed[..end];
            let value = text.parse().ok().map(Value::Number)?;
            *chars = &trimmed[end..];
            Some((value, chars))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A text with a mark of sixteen that is followed by a letter of more
    /// than one byte must not take the reader down.
    #[test]
    fn a_broken_mark_of_sixteen_is_refused() {
        assert!(read("{\"x\":\"\\uab€\"}").is_none());
        assert!(read("{\"x\":\"\\u12\"}").is_none());
        assert!(read("{\"x\":\"\\uZZZZ\"}").is_none());
    }

    /// A mark of sixteen is read, and so is a pair of them that is a
    /// letter outside the plane of the text.
    #[test]
    fn a_mark_of_sixteen_is_read() {
        let (value, _) = read("{\"x\":\"\\u00e9\"}").expect("the text is read");
        match value {
            Value::Object(items) => match items.iter().find(|(key, _)| *key == "x") {
                Some((_, Value::Text(text))) => assert_eq!(text, "é"),
                other => panic!("the value is not there: {other:?}"),
            },
            other => panic!("the value is not an object: {other:?}"),
        }
        let (value, _) = read("{\"x\":\"\\uD83D\\uDE00\"}").expect("the text is read");
        match value {
            Value::Object(items) => match items.iter().find(|(key, _)| *key == "x") {
                Some((_, Value::Text(text))) => assert_eq!(text, "😀"),
                other => panic!("the value is not there: {other:?}"),
            },
            other => panic!("the value is not an object: {other:?}"),
        }
    }
}
