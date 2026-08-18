
use crate::url::{default_port, is_special, Path, Url};
use rusty_js_percent_encoding as pe;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    SchemeStart,
    Scheme,
    NoScheme,
    SpecialRelativeOrAuthority,
    PathOrAuthority,
    Relative,
    RelativeSlash,
    SpecialAuthoritySlashes,
    SpecialAuthorityIgnoreSlashes,
    Authority,
    Host,
    Port,
    File,
    FileSlash,
    FileHost,
    PathStart,
    PathState,
    OpaquePath,
    Query,
    Fragment,
}

fn is_c0_or_space(c: char) -> bool {
    c <= ' '
}

fn strip_input(input: &str) -> String {

    let trimmed = input
        .trim_start_matches(is_c0_or_space)
        .trim_end_matches(is_c0_or_space);
    trimmed
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect()
}

fn is_normalized_windows_drive(seg: &str) -> bool {
    let b = seg.as_bytes();
    b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

fn is_windows_drive(seg: &str) -> bool {
    let b = seg.as_bytes();
    b.len() == 2 && b[0].is_ascii_alphabetic() && (b[1] == b':' || b[1] == b'|')
}

fn encode_cp(c: char, set: &pe::EncodeSet, out: &mut String) {
    let mut buf = [0u8; 4];
    let s = c.encode_utf8(&mut buf);
    out.push_str(&pe::encode(s.as_bytes(), set));
}

pub fn parse(input: &str, base: Option<&Url>) -> Result<Url, ()> {
    let input = strip_input(input);
    let chars: Vec<char> = input.chars().collect();

    let mut url = Url {
        scheme: String::new(),
        username: String::new(),
        password: String::new(),
        host: None,
        port: None,
        path: Path::List(Vec::new()),
        query: None,
        fragment: None,
    };

    let mut state = State::SchemeStart;
    let mut buffer = String::new();
    let mut at_sign_seen = false;
    let mut inside_brackets = false;
    let mut password_token_seen = false;
    let mut i: usize = 0;

    loop {
        let c = chars.get(i).copied();
        match state {
            State::SchemeStart => match c {
                Some(ch) if ch.is_ascii_alphabetic() => {
                    buffer.push(ch.to_ascii_lowercase());
                    state = State::Scheme;
                }
                _ => {
                    state = State::NoScheme;
                    continue;
                }
            },
            State::Scheme => match c {
                Some(ch) if ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.') => {
                    buffer.push(ch.to_ascii_lowercase());
                }
                Some(':') => {
                    url.scheme = std::mem::take(&mut buffer);
                    if url.scheme == "file" {
                        state = State::File;
                    } else if is_special(&url.scheme) {
                        if base.map(|b| b.scheme == url.scheme).unwrap_or(false) {
                            state = State::SpecialRelativeOrAuthority;
                        } else {
                            state = State::SpecialAuthoritySlashes;
                        }
                    } else if chars.get(i + 1) == Some(&'/') {
                        state = State::PathOrAuthority;
                        i += 1;
                    } else {
                        url.path = Path::Opaque(String::new());
                        state = State::OpaquePath;
                    }
                }
                _ => {
                    buffer.clear();
                    state = State::NoScheme;
                    i = 0;
                    continue;
                }
            },
            State::NoScheme => {
                let base = base.ok_or(())?;
                if base.has_opaque_path() {
                    if c == Some('#') {
                        url.scheme = base.scheme.clone();
                        url.path = base.path.clone();
                        url.query = base.query.clone();
                        url.fragment = Some(String::new());
                        state = State::Fragment;
                    } else {
                        return Err(());
                    }
                } else if base.scheme != "file" {
                    state = State::Relative;
                    continue;
                } else {
                    state = State::File;
                    continue;
                }
            }
            State::SpecialRelativeOrAuthority => {
                if c == Some('/') && chars.get(i + 1) == Some(&'/') {
                    state = State::SpecialAuthorityIgnoreSlashes;
                    i += 1;
                } else {
                    state = State::Relative;
                    continue;
                }
            }
            State::PathOrAuthority => {
                if c == Some('/') {
                    state = State::Authority;
                } else {
                    state = State::PathState;
                    continue;
                }
            }
            State::Relative => {
                let base = base.ok_or(())?;
                url.scheme = base.scheme.clone();
                match c {
                    Some('/') => state = State::RelativeSlash,
                    Some('\\') if is_special(&url.scheme) => state = State::RelativeSlash,
                    Some('?') => {
                        url.username = base.username.clone();
                        url.password = base.password.clone();
                        url.host = base.host.clone();
                        url.port = base.port;
                        url.path = base.path.clone();
                        url.query = Some(String::new());
                        state = State::Query;
                    }
                    Some('#') => {
                        url.username = base.username.clone();
                        url.password = base.password.clone();
                        url.host = base.host.clone();
                        url.port = base.port;
                        url.path = base.path.clone();
                        url.query = base.query.clone();
                        url.fragment = Some(String::new());
                        state = State::Fragment;
                    }
                    None => {
                        url.username = base.username.clone();
                        url.password = base.password.clone();
                        url.host = base.host.clone();
                        url.port = base.port;
                        url.path = base.path.clone();
                        url.query = base.query.clone();
                    }
                    Some(_) => {
                        url.username = base.username.clone();
                        url.password = base.password.clone();
                        url.host = base.host.clone();
                        url.port = base.port;

                        url.path = shorten_path(base);
                        state = State::PathState;
                        continue;
                    }
                }
            }
            State::RelativeSlash => {
                let base = base.ok_or(())?;
                if is_special(&url.scheme) && matches!(c, Some('/') | Some('\\')) {
                    state = State::SpecialAuthorityIgnoreSlashes;
                } else if c == Some('/') {
                    state = State::Authority;
                } else {
                    url.username = base.username.clone();
                    url.password = base.password.clone();
                    url.host = base.host.clone();
                    url.port = base.port;
                    state = State::PathState;
                    continue;
                }
            }
            State::SpecialAuthoritySlashes => {
                if c == Some('/') && chars.get(i + 1) == Some(&'/') {
                    state = State::SpecialAuthorityIgnoreSlashes;
                    i += 1;
                } else {
                    state = State::SpecialAuthorityIgnoreSlashes;
                    continue;
                }
            }
            State::SpecialAuthorityIgnoreSlashes => {
                if !matches!(c, Some('/') | Some('\\')) {
                    state = State::Authority;
                    continue;
                }
            }
            State::Authority => match c {
                Some('@') => {
                    if at_sign_seen {
                        let pre = buffer.clone();
                        buffer = format!("%40{}", pre);
                    }
                    at_sign_seen = true;
                    let buf = std::mem::take(&mut buffer);
                    for ch in buf.chars() {
                        if ch == ':' && !password_token_seen {
                            password_token_seen = true;
                            continue;
                        }
                        if password_token_seen {
                            encode_cp(ch, &pe::USERINFO, &mut url.password);
                        } else {
                            encode_cp(ch, &pe::USERINFO, &mut url.username);
                        }
                    }
                }
                None | Some('/') | Some('?') | Some('#') => {

                    i -= buffer.chars().count();
                    buffer.clear();
                    state = State::Host;
                    continue;
                }
                Some('\\') if is_special(&url.scheme) => {
                    i -= buffer.chars().count();
                    buffer.clear();
                    state = State::Host;
                    continue;
                }
                Some(ch) => buffer.push(ch),
            },
            State::Host => match c {
                Some(':') if !inside_brackets => {
                    url.host = Some(parse_host(&buffer, &url.scheme)?);
                    buffer.clear();
                    state = State::Port;
                }
                None | Some('/') | Some('?') | Some('#') => {
                    url.host = Some(parse_host(&buffer, &url.scheme)?);
                    buffer.clear();
                    state = State::PathStart;
                    continue;
                }
                Some('\\') if is_special(&url.scheme) => {
                    url.host = Some(parse_host(&buffer, &url.scheme)?);
                    buffer.clear();
                    state = State::PathStart;
                    continue;
                }
                Some('[') => {
                    inside_brackets = true;
                    buffer.push('[');
                }
                Some(']') => {
                    inside_brackets = false;
                    buffer.push(']');
                }
                Some(ch) => buffer.push(ch),
            },
            State::Port => match c {
                Some(ch) if ch.is_ascii_digit() => buffer.push(ch),
                None | Some('/') | Some('?') | Some('#') | Some('\\')
                    if c != Some('\\') || is_special(&url.scheme) =>
                {
                    if !buffer.is_empty() {
                        let p: u32 = buffer.parse().map_err(|_| ())?;
                        if p > 65535 {
                            return Err(());
                        }
                        let p = p as u16;
                        url.port = if default_port(&url.scheme) == Some(p) {
                            None
                        } else {
                            Some(p)
                        };
                        buffer.clear();
                    }
                    state = State::PathStart;
                    continue;
                }
                _ => return Err(()),
            },
            State::File => {
                url.scheme = "file".to_string();
                url.host = Some(String::new());
                match c {
                    Some('/') | Some('\\') => state = State::FileSlash,
                    _ => {
                        if let Some(b) = base {
                            if b.scheme == "file" {
                                url.host = b.host.clone();
                                url.path = b.path.clone();
                                url.query = b.query.clone();
                                match c {
                                    Some('?') => {
                                        url.query = Some(String::new());
                                        state = State::Query;
                                    }
                                    Some('#') => {
                                        url.fragment = Some(String::new());
                                        state = State::Fragment;
                                    }
                                    None => {}
                                    Some(_) => {
                                        url.query = None;
                                        if !starts_with_windows_drive(&chars[i..]) {
                                            url.path = shorten_path(&url);
                                        } else {
                                            url.path = Path::List(Vec::new());
                                        }
                                        state = State::PathState;
                                        continue;
                                    }
                                }
                            } else {
                                state = State::PathState;
                                continue;
                            }
                        } else {
                            state = State::PathState;
                            continue;
                        }
                    }
                }
            }
            State::FileSlash => match c {
                Some('/') | Some('\\') => state = State::FileHost,
                _ => {
                    if let Some(b) = base {
                        if b.scheme == "file" {
                            url.host = b.host.clone();
                            if !starts_with_windows_drive(&chars[i..]) && base_starts_with_drive(b)
                            {
                                if let Path::List(segs) = &b.path {
                                    if let Some(first) = segs.first() {
                                        url.path = Path::List(vec![first.clone()]);
                                    }
                                }
                            }
                        }
                    }
                    state = State::PathState;
                    continue;
                }
            },
            State::FileHost => match c {
                None | Some('/') | Some('\\') | Some('?') | Some('#') => {
                    if is_windows_drive(&buffer) {
                        state = State::PathState;
                        continue;
                    } else if buffer.is_empty() {
                        url.host = Some(String::new());
                        state = State::PathStart;
                        continue;
                    } else {
                        let h = parse_host(&buffer, &url.scheme)?;
                        url.host = Some(if h == "localhost" { String::new() } else { h });
                        buffer.clear();
                        state = State::PathStart;
                        continue;
                    }
                }
                Some(ch) => buffer.push(ch),
            },
            State::PathStart => {
                if is_special(&url.scheme) {
                    state = State::PathState;
                    if !matches!(c, Some('/') | Some('\\')) {
                        continue;
                    }
                } else {
                    match c {
                        Some('?') => {
                            url.query = Some(String::new());
                            state = State::Query;
                        }
                        Some('#') => {
                            url.fragment = Some(String::new());
                            state = State::Fragment;
                        }
                        None => {}
                        Some('/') => state = State::PathState,
                        Some(_) => {
                            state = State::PathState;
                            continue;
                        }
                    }
                }
            }
            State::PathState => {
                let ends_segment = c.is_none()
                    || c == Some('/')
                    || (is_special(&url.scheme) && c == Some('\\'))
                    || c == Some('?')
                    || c == Some('#');
                if ends_segment {
                    let seg = std::mem::take(&mut buffer);
                    if is_double_dot(&seg) {
                        pop_path(&mut url);
                        if !(c == Some('/') || (is_special(&url.scheme) && c == Some('\\'))) {
                            push_path(&mut url, String::new());
                        }
                    } else if is_single_dot(&seg)
                        && !(c == Some('/') || (is_special(&url.scheme) && c == Some('\\')))
                    {
                        push_path(&mut url, String::new());
                    } else if !is_single_dot(&seg) {
                        let mut seg = seg;
                        if url.scheme == "file" && path_is_empty(&url) && is_windows_drive(&seg) {
                            seg.replace_range(1..2, ":");
                        }
                        push_path(&mut url, seg);
                    }
                    match c {
                        Some('?') => {
                            url.query = Some(String::new());
                            state = State::Query;
                        }
                        Some('#') => {
                            url.fragment = Some(String::new());
                            state = State::Fragment;
                        }
                        _ => {}
                    }
                } else {
                    let ch = c.unwrap();
                    encode_cp(ch, &pe::PATH, &mut buffer);
                }
            }
            State::OpaquePath => match c {
                Some('?') => {
                    encode_trailing_opaque_space(&mut url);
                    url.query = Some(String::new());
                    state = State::Query;
                }
                Some('#') => {
                    encode_trailing_opaque_space(&mut url);
                    url.fragment = Some(String::new());
                    state = State::Fragment;
                }
                None => {}
                Some(ch) => {
                    if let Path::Opaque(s) = &mut url.path {
                        encode_cp(ch, &pe::CONTROLS, s);
                    }
                }
            },
            State::Query => match c {
                None | Some('#') => {
                    let set = if is_special(&url.scheme) {
                        &pe::SPECIAL_QUERY
                    } else {
                        &pe::FRAGMENT
                    };
                    let q = std::mem::take(&mut buffer);
                    let mut enc = String::new();
                    for ch in q.chars() {
                        encode_cp(ch, set, &mut enc);
                    }
                    url.query = Some(enc);
                    if c == Some('#') {
                        url.fragment = Some(String::new());
                        state = State::Fragment;
                    }
                }
                Some(ch) => buffer.push(ch),
            },
            State::Fragment => match c {
                None => {}
                Some(ch) => {
                    let mut enc = url.fragment.take().unwrap_or_default();
                    encode_cp(ch, &pe::FRAGMENT, &mut enc);
                    url.fragment = Some(enc);
                }
            },
        }

        if i >= chars.len() {
            break;
        }
        i += 1;
    }

    Ok(url)
}

pub fn parse_host(input: &str, scheme: &str) -> Result<String, ()> {
    if input.starts_with('[') {

        if !input.ends_with(']') {
            return Err(());
        }
        let inner = &input[1..input.len() - 1];
        let addr = crate::host::parse_ipv6(inner).ok_or(())?;
        return Ok(crate::host::serialize_ipv6(&addr));
    }
    if is_special(scheme) {
        if scheme != "file" && input.is_empty() {
            return Err(());
        }

        let ascii = crate::host::domain_to_ascii(input)?;

        if crate::host::ends_in_number(&ascii) {
            let v4 = crate::host::parse_ipv4(&ascii).ok_or(())?;
            return Ok(crate::host::serialize_ipv4(v4));
        }
        Ok(ascii)
    } else {
        if input.bytes().any(is_forbidden_host_code_point) {
            return Err(());
        }

        Ok(pe::encode(input.as_bytes(), &pe::CONTROLS))
    }
}

fn is_forbidden_host_code_point(byte: u8) -> bool {
    matches!(
        byte,
        0x00..=0x20
            | b'#'
            | b'%'
            | b'/'
            | b':'
            | b'<'
            | b'>'
            | b'?'
            | b'@'
            | b'['
            | b'\\'
            | b']'
            | b'^'
            | b'|'
    )
}

fn is_single_dot(seg: &str) -> bool {
    seg == "." || seg.eq_ignore_ascii_case("%2e")
}

fn is_double_dot(seg: &str) -> bool {
    let s = seg.to_ascii_lowercase();
    matches!(s.as_str(), ".." | ".%2e" | "%2e." | "%2e%2e")
}

fn path_is_empty(url: &Url) -> bool {
    matches!(&url.path, Path::List(s) if s.is_empty())
}

fn push_path(url: &mut Url, seg: String) {
    if let Path::List(segs) = &mut url.path {
        segs.push(seg);
    }
}

fn pop_path(url: &mut Url) {
    if let Path::List(segs) = &mut url.path {

        if !(url.scheme == "file" && segs.len() == 1 && is_normalized_windows_drive(&segs[0])) {
            segs.pop();
        }
    }
}

fn encode_trailing_opaque_space(url: &mut Url) {
    if let Path::Opaque(s) = &mut url.path {
        if s.ends_with(' ') {
            s.pop();
            s.push_str("%20");
        }
    }
}

fn shorten_path(url: &Url) -> Path {
    match &url.path {
        Path::Opaque(s) => Path::Opaque(s.clone()),
        Path::List(segs) => {
            if url.scheme == "file" && segs.len() == 1 && is_normalized_windows_drive(&segs[0]) {
                Path::List(segs.clone())
            } else {
                let mut s = segs.clone();
                s.pop();
                Path::List(s)
            }
        }
    }
}

fn starts_with_windows_drive(rest: &[char]) -> bool {
    rest.len() >= 2
        && rest[0].is_ascii_alphabetic()
        && (rest[1] == ':' || rest[1] == '|')
        && (rest.len() == 2 || matches!(rest[2], '/' | '\\' | '?' | '#'))
}

fn base_starts_with_drive(base: &Url) -> bool {
    if let Path::List(segs) = &base.path {
        segs.first()
            .map(|s| is_normalized_windows_drive(s))
            .unwrap_or(false)
    } else {
        false
    }
}
