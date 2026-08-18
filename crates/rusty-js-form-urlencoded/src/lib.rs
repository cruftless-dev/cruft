
use rusty_js_percent_encoding::EncodeSet;
use std::borrow::Cow;

const fn form_encode_table() -> [bool; 256] {
    let mut t = [true; 256];
    let mut b = 0usize;
    while b < 256 {
        let c = b as u8;
        let unreserved = matches!(c, b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z')
            || matches!(c, b'*' | b'-' | b'.' | b'_')
            || c == b' ';
        t[b] = !unreserved;
        b += 1;
    }
    t
}

const FORM_SET: EncodeSet = EncodeSet::from_table(form_encode_table());

fn serialize_string(s: &str, out: &mut String) {
    let encoded = rusty_js_percent_encoding::encode(s.as_bytes(), &FORM_SET);
    for ch in encoded.chars() {
        out.push(if ch == ' ' { '+' } else { ch });
    }
}

pub fn serialize(pairs: &[(Cow<str>, Cow<str>)]) -> String {
    serialize_borrowed(
        pairs
            .iter()
            .map(|(name, value)| (name.as_ref(), value.as_ref())),
    )
}

pub fn serialize_borrowed<'a, I>(pairs: I) -> String
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    let mut out = String::new();
    for (name, value) in pairs {
        if !out.is_empty() {
            out.push('&');
        }
        serialize_string(name, &mut out);
        out.push('=');
        serialize_string(value, &mut out);
    }
    out
}

pub fn parse(input: &str) -> Vec<(String, String)> {
    let mut output = Vec::new();
    for seq in input.as_bytes().split(|&b| b == b'&') {
        if seq.is_empty() {
            continue;
        }
        let (name_bytes, value_bytes) = match seq.iter().position(|&b| b == b'=') {
            Some(i) => (&seq[..i], &seq[i + 1..]),
            None => (seq, &seq[seq.len()..]),
        };
        output.push((decode_component(name_bytes), decode_component(value_bytes)));
    }
    output
}

fn decode_component(bytes: &[u8]) -> String {
    let s = String::from_utf8_lossy(bytes);
    let plus_replaced = s.replace('+', " ");
    let decoded = rusty_js_percent_encoding::decode_lenient(&plus_replaced);
    String::from_utf8_lossy(&decoded).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    fn p(s: &str) -> Vec<(Cow<str>, Cow<str>)> {

        parse(s)
            .into_iter()
            .map(|(k, v)| (Cow::Owned(k), Cow::Owned(v)))
            .collect()
    }

    #[test]
    fn serialize_basic() {
        let pairs = vec![
            (Cow::from("a"), Cow::from("b")),
            (Cow::from("c"), Cow::from("d")),
        ];
        assert_eq!(serialize(&pairs), "a=b&c=d");
    }

    #[test]
    fn serialize_borrowed_basic() {
        let pairs = [("a b", "c=d"), ("star", "*-._")];
        assert_eq!(serialize_borrowed(pairs), "a+b=c%3Dd&star=*-._");
    }

    #[test]
    fn space_becomes_plus() {
        let pairs = vec![(Cow::from("a b"), Cow::from("c d"))];
        assert_eq!(serialize(&pairs), "a+b=c+d");
    }

    #[test]
    fn special_chars_percent_encoded() {
        let pairs = vec![(Cow::from("a&b"), Cow::from("c=d"))];
        assert_eq!(serialize(&pairs), "a%26b=c%3Dd");
    }

    #[test]
    fn form_unreserved_passthrough() {
        let pairs = vec![(Cow::from("*-._"), Cow::from("AZaz09"))];
        assert_eq!(serialize(&pairs), "*-._=AZaz09");
    }

    #[test]
    fn unicode_utf8_encoded() {

        let pairs = vec![(Cow::from("k"), Cow::from("é"))];
        assert_eq!(serialize(&pairs), "k=%C3%A9");
    }

    #[test]
    fn parse_basic() {
        assert_eq!(
            parse("a=b&c=d"),
            vec![("a".into(), "b".into()), ("c".into(), "d".into())]
        );
    }

    #[test]
    fn parse_plus_is_space() {
        assert_eq!(parse("a+b=c+d"), vec![("a b".into(), "c d".into())]);
    }

    #[test]
    fn parse_percent_decoded() {
        assert_eq!(parse("a%26b=c%3Dd"), vec![("a&b".into(), "c=d".into())]);
    }

    #[test]
    fn parse_no_value() {
        assert_eq!(parse("key"), vec![("key".into(), "".into())]);
    }

    #[test]
    fn parse_empty_sequences_skipped() {
        assert_eq!(
            parse("&a=b&&c=d&"),
            vec![("a".into(), "b".into()), ("c".into(), "d".into())]
        );
    }

    #[test]
    fn parse_unicode() {
        assert_eq!(parse("k=%C3%A9"), vec![("k".into(), "é".into())]);
    }

    #[test]
    fn round_trip() {
        let s = "name=John+Doe&q=a%26b&emoji=%F0%9F%98%80";
        let parsed = p(s);
        assert_eq!(serialize(&parsed), s);
    }

    #[test]
    fn lone_percent_passthrough() {

        assert_eq!(parse("a=%zz"), vec![("a".into(), "%zz".into())]);
    }

    #[test]
    fn phase_c_encode_set_boundary() {

        assert_eq!(serialize(&[(Cow::from("a"), Cow::from("1+1"))]), "a=1%2B1");

        assert_eq!(serialize(&[(Cow::from("k"), Cow::from("\t"))]), "k=%09");

        assert_eq!(
            serialize(&[(Cow::from("k"), Cow::from("€"))]),
            "k=%E2%82%AC"
        );

        assert_eq!(serialize(&[(Cow::from("k"), Cow::from("a b"))]), "k=a+b");

        let s = "x=%2B&y=a+b&z=%E2%82%AC";
        let parsed: Vec<_> = parse(s)
            .into_iter()
            .map(|(k, v)| (Cow::Owned(k), Cow::Owned(v)))
            .collect();
        assert_eq!(serialize(&parsed), s);
    }
}
