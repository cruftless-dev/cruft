
use super::PgError;
use sql_core::SqlValue;

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    match validate(text) {
        Ok(()) => Ok(SqlValue::Text(text.to_string())),
        Err(()) => Err(PgError::InvalidInputSyntax {
            typ: super::type_name(oid),
            input: text.to_string(),
        }),
    }
}

pub fn output(_oid: u32, v: &SqlValue) -> String {
    match v {
        SqlValue::Text(s) => s.clone(),
        _ => String::new(),
    }
}

fn validate(text: &str) -> Result<(), ()> {
    let mut p = Parser {
        bytes: text.as_bytes(),
        pos: 0,
    };
    p.skip_ws();
    p.value()?;
    p.skip_ws();

    if p.pos == p.bytes.len() {
        Ok(())
    } else {
        Err(())
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek();
        if b.is_some() {
            self.pos += 1;
        }
        b
    }

    fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            match b {
                b' ' | b'\t' | b'\n' | b'\r' => self.pos += 1,
                _ => break,
            }
        }
    }

    fn value(&mut self) -> Result<(), ()> {
        match self.peek().ok_or(())? {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => self.string(),
            b't' => self.literal(b"true"),
            b'f' => self.literal(b"false"),
            b'n' => self.literal(b"null"),
            b'-' | b'0'..=b'9' => self.number(),
            _ => Err(()),
        }
    }

    fn literal(&mut self, word: &[u8]) -> Result<(), ()> {
        if self.bytes[self.pos..].starts_with(word) {
            self.pos += word.len();
            Ok(())
        } else {
            Err(())
        }
    }

    fn object(&mut self) -> Result<(), ()> {
        self.bump();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.bump();
            return Ok(());
        }
        loop {
            self.skip_ws();

            if self.peek() != Some(b'"') {
                return Err(());
            }
            self.string()?;
            self.skip_ws();
            if self.bump() != Some(b':') {
                return Err(());
            }
            self.skip_ws();
            self.value()?;
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b'}') => return Ok(()),
                _ => return Err(()),
            }
        }
    }

    fn array(&mut self) -> Result<(), ()> {
        self.bump();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.bump();
            return Ok(());
        }
        loop {
            self.skip_ws();
            self.value()?;
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => return Ok(()),
                _ => return Err(()),
            }
        }
    }

    fn string(&mut self) -> Result<(), ()> {
        self.bump();
        loop {
            match self.bump().ok_or(())? {
                b'"' => return Ok(()),
                b'\\' => match self.bump().ok_or(())? {
                    b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {}
                    b'u' => {
                        for _ in 0..4 {
                            let h = self.bump().ok_or(())?;
                            if !h.is_ascii_hexdigit() {
                                return Err(());
                            }
                        }
                    }
                    _ => return Err(()),
                },

                c if c < 0x20 => return Err(()),

                _ => {}
            }
        }
    }

    fn number(&mut self) -> Result<(), ()> {

        if self.peek() == Some(b'-') {
            self.bump();
        }

        match self.peek().ok_or(())? {
            b'0' => {
                self.bump();
            }
            b'1'..=b'9' => {
                self.bump();
                self.skip_digits();
            }
            _ => return Err(()),
        }

        if self.peek() == Some(b'.') {
            self.bump();
            if !self.at_digit() {
                return Err(());
            }
            self.skip_digits();
        }

        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.bump();
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.bump();
            }
            if !self.at_digit() {
                return Err(());
            }
            self.skip_digits();
        }
        Ok(())
    }

    fn at_digit(&self) -> bool {
        matches!(self.peek(), Some(b'0'..=b'9'))
    }

    fn skip_digits(&mut self) {
        while self.at_digit() {
            self.pos += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::oid::JSON;
    use super::*;

    fn ok(s: &str) -> SqlValue {
        input(JSON, s).unwrap_or_else(|e| panic!("expected {s:?} accepted: {}", e.message()))
    }

    fn is_ok_verbatim(s: &str) {

        let v = ok(s);
        assert_eq!(
            v,
            SqlValue::Text(s.to_string()),
            "{s:?} should store verbatim"
        );
        assert_eq!(output(JSON, &v), s, "{s:?} should output verbatim");
    }

    fn is_err(s: &str) {
        match input(JSON, s) {
            Ok(v) => panic!("{s:?} should be rejected, got {v:?}"),
            Err(e) => {

                assert!(
                    matches!(e, PgError::InvalidInputSyntax { typ: "json", .. }),
                    "{s:?} should be InvalidInputSyntax(json), got {e:?}"
                );
            }
        }
    }

    #[test]
    fn scalars_accepted() {

        for s in [
            r#""""#,
            r#""abc""#,
            "1",
            "0",
            "0.1",
            "9223372036854775808",
            "1e100",
            "1.3e100",
            "-5",
            "-0",
            "-1.5e-10",
            "true",
            "false",
            "null",
        ] {
            is_ok_verbatim(s);
        }
    }

    #[test]
    fn legal_string_escapes() {

        is_ok_verbatim(r#""\n\"\\""#);

        is_ok_verbatim(r#"".............abc\n""#);

        is_ok_verbatim(r#""\" \\ \/ \b \f \n \r \t""#);
    }

    #[test]
    fn unicode_escapes_accepted() {
        is_ok_verbatim(r#""é""#);
        is_ok_verbatim(r#""𝄞""#);
        is_ok_verbatim(r#""café ￿""#);
    }

    #[test]
    fn raw_utf8_in_string_accepted() {
        is_ok_verbatim("\"café — déjà\"");
    }

    #[test]
    fn arrays_accepted() {
        for s in [
            "[]",
            "[1,2]",
            "[1, 2, 3]",
            "[[1],[2,[3]]]",
            r#"["a",true,null,1.5]"#,
        ] {
            is_ok_verbatim(s);
        }

        let deep = format!("{}{}", "[".repeat(96), "]".repeat(96));
        is_ok_verbatim(&deep);
    }

    #[test]
    fn objects_accepted() {
        for s in [
            "{}",
            r#"{"abc":1}"#,
            r#"{"abc":1,"def":2,"ghi":[3,4],"hij":{"klm":5,"nop":[6]}}"#,
            r#"{ "a" : 1 , "b" : [ 2 , 3 ] }"#,
        ] {
            is_ok_verbatim(s);
        }
    }

    #[test]
    fn whitespace_preserved_and_allowed() {

        is_ok_verbatim(" true ");
        is_ok_verbatim("\n\t[1,\n\t 2]\n");

        is_ok_verbatim("{\n\t\t\"one\": 1,\n\t\t\"two\":\"two\",\n\t\t\"three\":\n\t\ttrue}");
    }

    #[test]
    fn duplicate_keys_preserved() {

        is_ok_verbatim(r#"{"a":1,"a":2,"a":3}"#);
    }

    #[test]
    fn invalid_strings_rejected() {
        is_err(r#"''"#);
        is_err(r#""abc"#);
        is_err("\"abc\ndef\"");
        is_err(r#""\v""#);
        is_err("\"tab\there\"");
        is_err(r#""\uZZZZ""#);
        is_err(r#""\u12""#);
    }

    #[test]
    fn invalid_numbers_rejected() {
        is_err("01");
        is_err("1f2");
        is_err("0.x1");
        is_err("1.3ex100");
        is_err("-");
        is_err("1.");
        is_err(".5");
        is_err("1e");
        is_err("+1");
        is_err("00");
    }

    #[test]
    fn invalid_arrays_rejected() {
        is_err("[1,2,]");
        is_err("[1,2");
        is_err("[1,[2]");
        is_err("[,]");
        is_err("[1 2]");
    }

    #[test]
    fn invalid_objects_rejected() {
        is_err(r#"{"abc"}"#);
        is_err(r#"{1:"abc"}"#);
        is_err(r#"{"abc",1}"#);
        is_err(r#"{"abc"=1}"#);
        is_err(r#"{"abc"::1}"#);
        is_err(r#"{"abc":1:2}"#);
        is_err(r#"{"abc":1,3}"#);
        is_err(r#"{"a":}"#);
        is_err(r#"{"a":1,}"#);
    }

    #[test]
    fn keywords_and_multi_value_rejected() {
        is_err("true false");
        is_err("true, false");
        is_err("truf");
        is_err("trues");
        is_err("");
        is_err("    ");
        is_err("nul");
        is_err("TRUE");
    }

    #[test]
    fn bare_words_rejected() {
        is_err("abc");
        is_err("hello world");
    }

    #[test]
    fn output_non_text_is_empty() {
        assert_eq!(output(JSON, &SqlValue::Null), "");
        assert_eq!(output(JSON, &SqlValue::Int(1)), "");
    }
}
