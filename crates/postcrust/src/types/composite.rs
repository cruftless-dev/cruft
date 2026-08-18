
#[derive(Debug, Clone)]
pub struct CompositeInfo {
    pub name: String,
    pub fields: Vec<(String, u32, i32)>,
}

impl CompositeInfo {

    pub fn field_index(&self, field: &str) -> Option<usize> {
        self.fields.iter().position(|(n, _, _)| n == field)
    }

    pub fn field_oid(&self, field: &str) -> Option<u32> {
        self.field_index(field).map(|i| self.fields[i].1)
    }
}

fn is_pg_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r')
}

fn quote_field(s: &str) -> String {
    let needs = s.is_empty()
        || s.chars()
            .any(|c| matches!(c, ',' | '(' | ')' | '"' | '\\') || is_pg_space(c));
    if !needs {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

pub fn encode(fields: &[Option<String>]) -> String {
    let mut out = String::from("(");
    for (i, f) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        match f {
            None => {}
            Some(s) => out.push_str(&quote_field(s)),
        }
    }
    out.push(')');
    out
}

pub fn decode(lit: &str) -> Result<Vec<Option<String>>, ()> {
    let chars: Vec<char> = lit.chars().collect();
    let mut i = 0usize;

    while i < chars.len() && is_pg_space(chars[i]) {
        i += 1;
    }
    if chars.get(i) != Some(&'(') {
        return Err(());
    }
    i += 1;

    let mut fields: Vec<Option<String>> = Vec::new();

    loop {

        let mut buf = String::new();
        let mut quoted = false;
        let mut any = false;
        loop {
            match chars.get(i) {
                None => return Err(()),
                Some('"') => {
                    quoted = true;
                    any = true;
                    i += 1;

                    loop {
                        match chars.get(i) {
                            None => return Err(()),
                            Some('\\') => {
                                i += 1;
                                match chars.get(i) {
                                    None => return Err(()),
                                    Some(c) => {
                                        buf.push(*c);
                                        i += 1;
                                    }
                                }
                            }
                            Some('"') => {
                                if chars.get(i + 1) == Some(&'"') {
                                    buf.push('"');
                                    i += 2;
                                } else {
                                    i += 1;
                                    break;
                                }
                            }
                            Some(c) => {
                                buf.push(*c);
                                i += 1;
                            }
                        }
                    }
                }
                Some(',') | Some(')') => break,
                Some('\\') => {
                    any = true;
                    i += 1;
                    match chars.get(i) {
                        None => return Err(()),
                        Some(c) => {
                            buf.push(*c);
                            i += 1;
                        }
                    }
                }
                Some(c) => {
                    any = true;
                    buf.push(*c);
                    i += 1;
                }
            }
        }

        if !any && !quoted {
            fields.push(None);
        } else {
            fields.push(Some(buf));
        }
        match chars.get(i) {
            Some(',') => {
                i += 1;
                continue;
            }
            Some(')') => {
                i += 1;
                break;
            }
            _ => return Err(()),
        }
    }

    while i < chars.len() && is_pg_space(chars[i]) {
        i += 1;
    }
    if i != chars.len() {
        return Err(());
    }
    Ok(fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_plain() {
        assert_eq!(encode(&[Some("1".into()), Some("x".into())]), "(1,x)");
    }

    #[test]
    fn encode_null_field_is_empty() {
        assert_eq!(encode(&[Some("1".into()), None]), "(1,)");
        assert_eq!(encode(&[None, None]), "(,)");
    }

    #[test]
    fn encode_quotes_when_needed() {
        assert_eq!(encode(&[Some("a,b".into())]), "(\"a,b\")");
        assert_eq!(encode(&[Some("a b".into())]), "(\"a b\")");
        assert_eq!(encode(&[Some(String::new())]), "(\"\")");
        assert_eq!(encode(&[Some("a\"b".into())]), "(\"a\\\"b\")");
        assert_eq!(encode(&[Some("a\\b".into())]), "(\"a\\\\b\")");
        assert_eq!(encode(&[Some("(x)".into())]), "(\"(x)\")");
    }

    #[test]
    fn decode_roundtrip() {
        assert_eq!(
            decode("(1,x)").unwrap(),
            vec![Some("1".into()), Some("x".into())]
        );
        assert_eq!(decode("(1,)").unwrap(), vec![Some("1".into()), None]);
        assert_eq!(decode("(,)").unwrap(), vec![None, None]);
        assert_eq!(decode("(\"a,b\")").unwrap(), vec![Some("a,b".into())]);
        assert_eq!(decode("(\"\")").unwrap(), vec![Some(String::new())]);
        assert_eq!(decode("(\"a\\\"b\")").unwrap(), vec![Some("a\"b".into())]);
        assert_eq!(decode("(\"a\"\"b\")").unwrap(), vec![Some("a\"b".into())]);
    }

    #[test]
    fn decode_malformed() {
        assert!(decode("1,x").is_err());
        assert!(decode("(1,x").is_err());
        assert!(decode("(1,x) junk").is_err());
    }
}
