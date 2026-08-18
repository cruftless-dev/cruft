
use super::PgError;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Int(i64),
    Float(f64),
    Str(String),
    Ident(String),
    Op(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    Lt,
    Gt,
    Eq,
    LtEq,
    GtEq,
    NotEq,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Dot,
    Cast,
    Eof,
}

fn is_op_char(c: char) -> bool {
    matches!(
        c,
        '~' | '!'
            | '@'
            | '#'
            | '^'
            | '&'
            | '|'
            | '`'
            | '?'
            | '+'
            | '-'
            | '*'
            | '/'
            | '%'
            | '<'
            | '>'
            | '='
    )
}

fn err(input: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: input.to_string(),
    }
}

pub fn lex(src: &str) -> Result<Vec<Tok>, PgError> {
    let cs: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();

    while i < cs.len() {
        let c = cs[i];

        if c.is_whitespace() {
            i += 1;
            continue;
        }

        if c == '-' && i + 1 < cs.len() && cs[i + 1] == '-' {
            while i < cs.len() && cs[i] != '\n' {
                i += 1;
            }
            continue;
        }

        if c == '/' && i + 1 < cs.len() && cs[i + 1] == '*' {
            i += 2;
            while i + 1 < cs.len() && !(cs[i] == '*' && cs[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            continue;
        }

        if c == '\'' {
            let mut s = String::new();
            i += 1;
            loop {
                if i >= cs.len() {
                    return Err(err(src));
                }
                if cs[i] == '\'' {
                    if i + 1 < cs.len() && cs[i + 1] == '\'' {
                        s.push('\'');
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                s.push(cs[i]);
                i += 1;
            }
            out.push(Tok::Str(s));
            continue;
        }

        if c.is_ascii_digit() || (c == '.' && i + 1 < cs.len() && cs[i + 1].is_ascii_digit()) {
            let start = i;
            let mut is_float = false;
            while i < cs.len() && cs[i].is_ascii_digit() {
                i += 1;
            }
            if i < cs.len() && cs[i] == '.' {
                is_float = true;
                i += 1;
                while i < cs.len() && cs[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if i < cs.len() && (cs[i] == 'e' || cs[i] == 'E') {
                is_float = true;
                i += 1;
                if i < cs.len() && (cs[i] == '+' || cs[i] == '-') {
                    i += 1;
                }
                while i < cs.len() && cs[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let text: String = cs[start..i].iter().collect();
            if is_float {
                out.push(Tok::Float(text.parse().map_err(|_| err(&text))?));
            } else {
                match text.parse::<i64>() {
                    Ok(n) => out.push(Tok::Int(n)),

                    Err(_) => out.push(Tok::Float(text.parse().map_err(|_| err(&text))?)),
                }
            }
            continue;
        }

        if c == '"' {
            let mut text = String::new();
            i += 1;
            while i < cs.len() {
                if cs[i] == '"' {
                    if i + 1 < cs.len() && cs[i + 1] == '"' {
                        text.push('"');
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                text.push(cs[i]);
                i += 1;
            }
            out.push(Tok::Ident(text));
            continue;
        }

        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_' || cs[i] == '$') {
                i += 1;
            }
            let word: String = cs[start..i].iter().collect();
            out.push(Tok::Ident(word.to_ascii_lowercase()));
            continue;
        }

        if c == '$' {

            if i + 1 < cs.len() && cs[i + 1].is_ascii_digit() {
                let start = i;
                i += 1;
                while i < cs.len() && cs[i].is_ascii_digit() {
                    i += 1;
                }
                let text: String = cs[start..i].iter().collect();
                out.push(Tok::Ident(text));
                continue;
            }

            let mut j = i + 1;

            while j < cs.len() && (cs[j].is_alphanumeric() || cs[j] == '_') {
                j += 1;
            }
            if j < cs.len() && cs[j] == '$' {
                let delim: Vec<char> = cs[i..=j].to_vec();
                let body_start = j + 1;

                let mut k = body_start;
                let close = loop {
                    if k + delim.len() > cs.len() {
                        return Err(err(src));
                    }
                    if cs[k..k + delim.len()] == delim[..] {
                        break k;
                    }
                    k += 1;
                };
                let body: String = cs[body_start..close].iter().collect();
                out.push(Tok::Str(body));
                i = close + delim.len();
                continue;
            }
            return Err(err(src));
        }

        if c == ':' && i + 1 < cs.len() && cs[i + 1] == ':' {
            out.push(Tok::Cast);
            i += 2;
            continue;
        }
        match c {
            '(' => {
                out.push(Tok::LParen);
                i += 1;
                continue;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
                continue;
            }
            '[' => {
                out.push(Tok::LBracket);
                i += 1;
                continue;
            }
            ']' => {
                out.push(Tok::RBracket);
                i += 1;
                continue;
            }
            ',' => {
                out.push(Tok::Comma);
                i += 1;
                continue;
            }
            '.' => {
                out.push(Tok::Dot);
                i += 1;
                continue;
            }
            _ => {}
        }

        if is_op_char(c) {
            let start = i;
            while i < cs.len() && is_op_char(cs[i]) {
                i += 1;
            }
            let run: String = cs[start..i].iter().collect();
            out.push(classify_op(&run));
            continue;
        }

        return Err(err(src));
    }

    out.push(Tok::Eof);
    Ok(out)
}

fn classify_op(run: &str) -> Tok {
    match run {
        "+" => Tok::Plus,
        "-" => Tok::Minus,
        "*" => Tok::Star,
        "/" => Tok::Slash,
        "%" => Tok::Percent,
        "^" => Tok::Caret,
        "<" => Tok::Lt,
        ">" => Tok::Gt,
        "=" => Tok::Eq,
        "<=" => Tok::LtEq,
        ">=" => Tok::GtEq,
        "<>" | "!=" => Tok::NotEq,
        other => Tok::Op(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lx(s: &str) -> Vec<Tok> {
        lex(s).expect("lex ok")
    }

    #[test]
    fn integers_and_floats() {
        assert_eq!(lx("42"), vec![Tok::Int(42), Tok::Eof]);
        assert_eq!(lx("3.5"), vec![Tok::Float(3.5), Tok::Eof]);
        assert_eq!(lx("1e3"), vec![Tok::Float(1000.0), Tok::Eof]);
        assert_eq!(lx(".5"), vec![Tok::Float(0.5), Tok::Eof]);
    }

    #[test]
    fn oversized_integer_promotes_to_float() {

        assert!(matches!(lx("99999999999999999999")[0], Tok::Float(_)));
    }

    #[test]
    fn arithmetic_self_chars() {
        assert_eq!(
            lx("1 + 2 * 3"),
            vec![
                Tok::Int(1),
                Tok::Plus,
                Tok::Int(2),
                Tok::Star,
                Tok::Int(3),
                Tok::Eof
            ]
        );
    }

    #[test]
    fn comparison_multichar_tokens() {
        assert_eq!(
            lx("a <= b"),
            vec![
                Tok::Ident("a".into()),
                Tok::LtEq,
                Tok::Ident("b".into()),
                Tok::Eof
            ]
        );
        assert_eq!(
            lx("a <> b"),
            vec![
                Tok::Ident("a".into()),
                Tok::NotEq,
                Tok::Ident("b".into()),
                Tok::Eof
            ]
        );
        assert_eq!(
            lx("a != b"),
            vec![
                Tok::Ident("a".into()),
                Tok::NotEq,
                Tok::Ident("b".into()),
                Tok::Eof
            ]
        );
    }

    #[test]
    fn general_op_run_is_one_token() {

        assert_eq!(
            lx("a || b"),
            vec![
                Tok::Ident("a".into()),
                Tok::Op("||".into()),
                Tok::Ident("b".into()),
                Tok::Eof
            ]
        );
        assert_eq!(
            lx("a @> b"),
            vec![
                Tok::Ident("a".into()),
                Tok::Op("@>".into()),
                Tok::Ident("b".into()),
                Tok::Eof
            ]
        );
    }

    #[test]
    fn cast_token() {
        assert_eq!(
            lx("1::int"),
            vec![Tok::Int(1), Tok::Cast, Tok::Ident("int".into()), Tok::Eof]
        );
    }

    #[test]
    fn identifiers_downcased() {
        assert_eq!(lx("Foo"), vec![Tok::Ident("foo".into()), Tok::Eof]);
    }

    #[test]
    fn string_literal_with_embedded_quote() {
        assert_eq!(lx("'it''s'"), vec![Tok::Str("it's".into()), Tok::Eof]);
    }

    #[test]
    fn unterminated_string_errors() {
        assert!(matches!(
            lex("'abc"),
            Err(PgError::InvalidInputSyntax { .. })
        ));
    }

    #[test]
    fn comments_skipped() {
        assert_eq!(
            lx("1 -- trailing\n+ 2"),
            vec![Tok::Int(1), Tok::Plus, Tok::Int(2), Tok::Eof]
        );
        assert_eq!(
            lx("1 /* mid */ + 2"),
            vec![Tok::Int(1), Tok::Plus, Tok::Int(2), Tok::Eof]
        );
    }
}
