
use super::PgError;
use sql_core::SqlValue;

pub type Entry = (String, Vec<u32>);

pub fn tokenize(text: &str) -> Vec<Entry> {
    let mut occurrences: Vec<(String, u32)> = Vec::new();
    let mut pos: u32 = 0;
    let mut cur = String::new();
    let flush = |cur: &mut String, occ: &mut Vec<(String, u32)>, pos: &mut u32| {
        if !cur.is_empty() {
            *pos += 1;
            occ.push((std::mem::take(cur), *pos));
        }
    };
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            cur.extend(ch.to_lowercase());
        } else {
            flush(&mut cur, &mut occurrences, &mut pos);
        }
    }
    flush(&mut cur, &mut occurrences, &mut pos);
    group(occurrences)
}

fn group(mut occ: Vec<(String, u32)>) -> Vec<Entry> {
    occ.sort();
    let mut out: Vec<Entry> = Vec::new();
    for (lex, p) in occ {
        match out.last_mut() {
            Some((l, ps)) if *l == lex => {
                if !ps.contains(&p) {
                    ps.push(p);
                }
            }
            _ => out.push((lex, vec![p])),
        }
    }
    for (_, ps) in out.iter_mut() {
        ps.sort_unstable();
        ps.dedup();
    }
    out
}

pub fn render(entries: &[Entry]) -> String {
    let mut out = String::new();
    for (i, (lex, positions)) in entries.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        push_quoted(lex, &mut out);
        if !positions.is_empty() {
            out.push(':');
            for (j, p) in positions.iter().enumerate() {
                if j > 0 {
                    out.push(',');
                }
                out.push_str(&p.to_string());
            }
        }
    }
    out
}

fn push_quoted(lex: &str, out: &mut String) {
    out.push('\'');
    for c in lex.chars() {
        match c {
            '\'' => out.push_str("''"),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out.push('\'');
}

pub fn strip(entries: &[Entry]) -> Vec<Entry> {
    entries
        .iter()
        .map(|(l, _)| (l.clone(), Vec::new()))
        .collect()
}

pub fn parse(input: &str) -> Result<Vec<Entry>, PgError> {
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut occ: Vec<(String, u32)> = Vec::new();
    let mut bare: Vec<(String, Vec<u32>)> = Vec::new();
    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }

        let lex = if chars[i] == '\'' {
            i += 1;
            let mut s = String::new();
            loop {
                match chars.get(i) {
                    None => return Err(syntax(input)),
                    Some('\'') => {
                        if chars.get(i + 1) == Some(&'\'') {
                            s.push('\'');
                            i += 2;
                        } else {
                            i += 1;
                            break;
                        }
                    }
                    Some('\\') => {
                        i += 1;
                        if let Some(&c) = chars.get(i) {
                            s.push(c);
                            i += 1;
                        } else {
                            return Err(syntax(input));
                        }
                    }
                    Some(&c) => {
                        s.push(c);
                        i += 1;
                    }
                }
            }
            s
        } else {
            let start = i;
            while i < chars.len() && !chars[i].is_whitespace() && chars[i] != ':' {
                i += 1;
            }
            if i == start {
                return Err(syntax(input));
            }
            chars[start..i].iter().collect()
        };
        if lex.is_empty() {
            return Err(syntax(input));
        }

        let mut positions: Vec<u32> = Vec::new();
        if chars.get(i) == Some(&':') {
            i += 1;
            loop {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                if i == start {
                    return Err(syntax(input));
                }
                let n: u32 = chars[start..i]
                    .iter()
                    .collect::<String>()
                    .parse()
                    .map_err(|_| syntax(input))?;
                positions.push(n);

                while matches!(chars.get(i), Some('A'..='D') | Some('a'..='d')) {
                    i += 1;
                }
                if chars.get(i) == Some(&',') {
                    i += 1;
                    continue;
                }
                break;
            }
        }
        if positions.is_empty() {
            bare.push((lex, Vec::new()));
        } else {
            for p in positions {
                occ.push((lex.clone(), p));
            }
        }
    }

    let mut entries = group(occ);
    for (lex, _) in bare {
        if !entries.iter().any(|(l, _)| *l == lex) {
            entries.push((lex, Vec::new()));
        }
    }
    entries.sort();
    Ok(entries)
}

fn syntax(input: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "tsvector",
        input: input.to_string(),
    }
}

pub fn input(_oid: u32, text: &str) -> Result<SqlValue, PgError> {
    Ok(SqlValue::Text(render(&parse(text)?)))
}

pub fn output(_oid: u32, v: &SqlValue) -> String {
    match v {
        SqlValue::Text(s) => s.clone(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_tokenize() {
        assert_eq!(
            render(&tokenize("a fat cat sat on a mat")),
            "'a':1,6 'cat':3 'fat':2 'mat':7 'on':5 'sat':4"
        );

        assert_eq!(
            render(&tokenize("The Fat Rats")),
            "'fat':2 'rats':3 'the':1"
        );
    }

    #[test]
    fn strip_drops_positions() {
        let v = tokenize("a fat cat");
        assert_eq!(render(&strip(&v)), "'a' 'cat' 'fat'");
    }

    #[test]
    fn parse_roundtrips_canonical() {
        let canon = "'a':1,6 'cat':3 'fat':2";
        assert_eq!(render(&parse(canon).unwrap()), canon);

        assert_eq!(render(&parse("cat fat a").unwrap()), "'a' 'cat' 'fat'");

        assert_eq!(render(&parse("'cat':3A,7B").unwrap()), "'cat':3,7");
    }
}
