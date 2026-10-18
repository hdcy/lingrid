//! Small bounded parser for Steam's text KeyValues files (not binary VDF).
//! Preserve duplicate keys, UTF-8 paths and unknown backslash escapes.
#[derive(Debug)]
pub enum Value {
    Text(String),
    Object(Vec<(String, Value)>),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries()?
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v)
    }
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            _ => None,
        }
    }
    pub fn entries(&self) -> Option<&[(String, Value)]> {
        match self {
            Self::Object(o) => Some(o),
            _ => None,
        }
    }
}

pub fn parse(text: &str) -> Result<Value, String> {
    let mut parser = Parser {
        chars: text.trim_start_matches('\u{feff}').chars().peekable(),
    };
    parser.object(0)
}

struct Parser<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl Parser<'_> {
    fn skip(&mut self) {
        loop {
            while self.chars.peek().is_some_and(|c| c.is_whitespace()) {
                self.chars.next();
            }
            let mut look = self.chars.clone();
            if look.next() != Some('/') || look.next() != Some('/') {
                break;
            }
            for c in self.chars.by_ref() {
                if c == '\n' {
                    break;
                }
            }
        }
    }

    fn token(&mut self) -> Result<String, String> {
        self.skip();
        let mut out = String::new();
        if self.chars.peek() == Some(&'"') {
            self.chars.next();
            while let Some(c) = self.chars.next() {
                match c {
                    '"' => return Ok(out),
                    '\\' => match self.chars.next() {
                        Some(c @ ('"' | '\\')) => out.push(c),
                        Some(c) => {
                            out.push('\\');
                            out.push(c);
                        }
                        None => return Err("Unterminated escape".into()),
                    },
                    _ => out.push(c),
                }
            }
            return Err("Unterminated quoted string".into());
        }
        while let Some(&c) = self.chars.peek() {
            if c.is_whitespace() || matches!(c, '{' | '}' | '"') {
                break;
            }
            out.push(c);
            self.chars.next();
        }
        if out.is_empty() {
            Err("Expected a key or value".into())
        } else {
            Ok(out)
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, String> {
        if depth > 32 {
            return Err("KeyValues nesting limit exceeded".into());
        }
        let mut entries = Vec::new();
        loop {
            self.skip();
            match self.chars.peek() {
                None if depth == 0 => return Ok(Value::Object(entries)),
                None => return Err("Missing closing brace".into()),
                Some('}') if depth > 0 => {
                    self.chars.next();
                    return Ok(Value::Object(entries));
                }
                Some('}') => return Err("Unexpected closing brace".into()),
                _ => {}
            }
            let key = self.token()?;
            self.skip();
            let value = if self.chars.peek() == Some(&'{') {
                self.chars.next();
                self.object(depth + 1)?
            } else {
                Value::Text(self.token()?)
            };
            entries.push((key, value));
        }
    }
}
