//! A small strict JSON parser for the frame record's reader, so the probe
//! needs no third-party one: numbers keep their text until a field says
//! what it is, and a key twice in an object is refused. Then the typed
//! reads `read.rs` builds the record with, each failing with the JSON
//! pointer of the value it refused.

/// A parsed JSON value. Objects keep their order; a key twice is refused.
#[derive(Debug)]
pub(super) enum Json {
    Null,
    Bool(bool),
    Number(String),
    Str(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

/// How deep arrays and objects may nest: the record needs 6.
const MAX_DEPTH: usize = 16;

struct Parser<'a> {
    text: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn fail<T>(&self, what: &str) -> Result<T, String> {
        Err(format!("byte {}: {what}", self.at))
    }

    fn skip_space(&mut self) {
        while let Some(b' ' | b'\n' | b'\r' | b'\t') = self.text.get(self.at) {
            self.at += 1;
        }
    }

    fn eat(&mut self, byte: u8) -> bool {
        if self.text.get(self.at) == Some(&byte) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn word(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if self.text[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            self.fail("not a JSON value")
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > MAX_DEPTH {
            return self.fail("nested too deep");
        }
        self.skip_space();
        match self.text.get(self.at) {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => self.string().map(Json::Str),
            Some(b't') => self.word("true", Json::Bool(true)),
            Some(b'f') => self.word("false", Json::Bool(false)),
            Some(b'n') => self.word("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => self.fail("not a JSON value"),
        }
    }

    fn digits(&mut self) -> usize {
        let start = self.at;
        while self.text.get(self.at).is_some_and(u8::is_ascii_digit) {
            self.at += 1;
        }
        self.at - start
    }

    /// JSON's grammar: `-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?`.
    fn number(&mut self) -> Result<Json, String> {
        let start = self.at;
        self.eat(b'-');
        if self.eat(b'0') {
            if self.text.get(self.at).is_some_and(u8::is_ascii_digit) {
                return self.fail("a number with a leading zero");
            }
        } else if self.digits() == 0 {
            return self.fail("a number without digits");
        }
        if self.eat(b'.') && self.digits() == 0 {
            return self.fail("a number without digits after its point");
        }
        if self.eat(b'e') || self.eat(b'E') {
            let _ = self.eat(b'+') || self.eat(b'-');
            if self.digits() == 0 {
                return self.fail("a number without digits in its exponent");
            }
        }
        let text = std::str::from_utf8(&self.text[start..self.at]).expect("ASCII");
        Ok(Json::Number(text.to_owned()))
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self.text.get(self.at..self.at + 4);
        // Four hex digits exactly: `from_str_radix` alone takes a sign.
        let value = digits
            .filter(|d| d.iter().all(u8::is_ascii_hexdigit))
            .and_then(|d| std::str::from_utf8(d).ok())
            .and_then(|d| u32::from_str_radix(d, 16).ok());
        match value {
            Some(v) => {
                self.at += 4;
                Ok(v)
            }
            None => self.fail("a \\u escape without four hex digits"),
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.at += 1;
        let mut out = String::new();
        loop {
            let Some(&byte) = self.text.get(self.at) else {
                return self.fail("a string that does not end");
            };
            self.at += 1;
            match byte {
                b'"' => return Ok(out),
                b'\\' => {
                    let Some(&escape) = self.text.get(self.at) else {
                        return self.fail("a string that does not end");
                    };
                    self.at += 1;
                    let c = match escape {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let high = self.hex4()?;
                            let code = if (0xd800..0xdc00).contains(&high) {
                                if !(self.eat(b'\\') && self.eat(b'u')) {
                                    return self.fail("a lone surrogate");
                                }
                                let low = self.hex4()?;
                                if !(0xdc00..0xe000).contains(&low) {
                                    return self.fail("a lone surrogate");
                                }
                                0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00)
                            } else {
                                high
                            };
                            match char::from_u32(code) {
                                Some(c) => c,
                                None => return self.fail("a lone surrogate"),
                            }
                        }
                        _ => return self.fail("an unknown escape"),
                    };
                    out.push(c);
                }
                0..0x20 => return self.fail("a control character in a string"),
                _ => {
                    // The text is UTF-8, so a sequence starts here: copy it.
                    let start = self.at - 1;
                    let len = match byte {
                        0xf0.. => 4,
                        0xe0.. => 3,
                        0xc0.. => 2,
                        _ => 1,
                    };
                    let end = (start + len).min(self.text.len());
                    match std::str::from_utf8(&self.text[start..end]) {
                        Ok(s) => out.push_str(s),
                        Err(_) => return self.fail("a string that is not UTF-8"),
                    }
                    self.at = end;
                }
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Json, String> {
        self.at += 1;
        let mut items = Vec::new();
        self.skip_space();
        if self.eat(b']') {
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.skip_space();
            if self.eat(b']') {
                return Ok(Json::Array(items));
            }
            if !self.eat(b',') {
                return self.fail("an array missing a ',' or ']'");
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, String> {
        self.at += 1;
        let mut pairs: Vec<(String, Json)> = Vec::new();
        self.skip_space();
        if self.eat(b'}') {
            return Ok(Json::Object(pairs));
        }
        loop {
            self.skip_space();
            if self.text.get(self.at) != Some(&b'"') {
                return self.fail("an object key that is not a string");
            }
            let key = self.string()?;
            if pairs.iter().any(|(k, _)| *k == key) {
                return self.fail(&format!("the key {key:?} twice"));
            }
            self.skip_space();
            if !self.eat(b':') {
                return self.fail("an object missing a ':'");
            }
            let value = self.value(depth + 1)?;
            pairs.push((key, value));
            self.skip_space();
            if self.eat(b'}') {
                return Ok(Json::Object(pairs));
            }
            if !self.eat(b',') {
                return self.fail("an object missing a ',' or '}'");
            }
        }
    }
}

pub(super) fn parse_json(text: &str) -> Result<Json, String> {
    let mut parser = Parser {
        text: text.as_bytes(),
        at: 0,
    };
    let value = parser.value(0)?;
    parser.skip_space();
    if parser.at != parser.text.len() {
        return parser.fail("text after the record");
    }
    Ok(value)
}

// Typed reads of parsed values, each naming where it failed.

/// Where a value sits in the record, for errors: a JSON pointer.
pub(super) type At<'a> = &'a str;

pub(super) fn wrong<T>(at: At<'_>, what: &str) -> Result<T, String> {
    Err(format!("{at}: {what}"))
}

/// An object's fields, which must be exactly `keys` (sorted).
pub(super) fn fields<'j, const N: usize>(
    value: &'j Json,
    at: At<'_>,
    keys: [&str; N],
) -> Result<[&'j Json; N], String> {
    let Json::Object(pairs) = value else {
        return wrong(at, "not an object");
    };
    let names: Vec<&str> = pairs.iter().map(|(k, _)| k.as_str()).collect();
    if names != keys {
        return wrong(at, &format!("keys {names:?}, not {keys:?}"));
    }
    Ok(core::array::from_fn(|i| &pairs[i].1))
}

pub(super) fn array<'j>(value: &'j Json, at: At<'_>) -> Result<&'j [Json], String> {
    match value {
        Json::Array(items) => Ok(items),
        _ => wrong(at, "not an array"),
    }
}

fn number_text<'j>(value: &'j Json, at: At<'_>) -> Result<&'j str, String> {
    match value {
        Json::Number(text) => Ok(text),
        _ => wrong(at, "not a number"),
    }
}

/// A whole number, written without sign, point or exponent.
pub(super) fn uint(value: &Json, at: At<'_>, max: u64) -> Result<u64, String> {
    let text = number_text(value, at)?;
    match text.parse::<u64>() {
        Ok(n) if n <= max && text.bytes().all(|b| b.is_ascii_digit()) => Ok(n),
        _ => wrong(at, &format!("{text} is not a whole number from 0 to {max}")),
    }
}

pub(super) fn u32_of(value: &Json, at: At<'_>) -> Result<u32, String> {
    uint(value, at, u32::MAX.into()).map(|n| n as u32)
}

pub(super) fn f64_of(value: &Json, at: At<'_>) -> Result<f64, String> {
    let text = number_text(value, at)?;
    match text.parse::<f64>() {
        Ok(x) if x.is_finite() => Ok(x),
        _ => wrong(at, &format!("{text} is not a finite number")),
    }
}

pub(super) fn f32_of(value: &Json, at: At<'_>) -> Result<f32, String> {
    let text = number_text(value, at)?;
    match text.parse::<f32>() {
        Ok(x) if x.is_finite() => Ok(x),
        _ => wrong(at, &format!("{text} is not a finite f32")),
    }
}

/// A number from `low` to `high`.
pub(super) fn within(value: &Json, at: At<'_>, low: f64, high: f64) -> Result<f64, String> {
    let x = f64_of(value, at)?;
    if (low..=high).contains(&x) {
        Ok(x)
    } else {
        wrong(at, &format!("{x} is not from {low} to {high}"))
    }
}

/// An angle in degrees from `low` up to, not including, `low + 360`: what
/// a binary angle gives.
pub(super) fn angle(value: &Json, at: At<'_>, low: f64) -> Result<f64, String> {
    let x = f64_of(value, at)?;
    if (low..low + 360.0).contains(&x) {
        Ok(x)
    } else {
        wrong(at, &format!("{x} is not from {low} up to {}", low + 360.0))
    }
}

pub(super) fn nullable<T>(
    value: &Json,
    read: impl FnOnce(&Json) -> Result<T, String>,
) -> Result<Option<T>, String> {
    match value {
        Json::Null => Ok(None),
        other => read(other).map(Some),
    }
}

pub(super) fn boolean(value: &Json, at: At<'_>) -> Result<bool, String> {
    match value {
        Json::Bool(b) => Ok(*b),
        _ => wrong(at, "not true or false"),
    }
}

pub(super) fn string<'j>(value: &'j Json, at: At<'_>) -> Result<&'j str, String> {
    match value {
        Json::Str(s) => Ok(s),
        _ => wrong(at, "not a string"),
    }
}
