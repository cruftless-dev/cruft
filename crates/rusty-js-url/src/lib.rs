
pub mod host;
pub mod state_machine;
pub mod url;

pub use url::{Path, Url};

pub fn parse(input: &str, base: Option<&str>) -> Result<Url, ()> {
    let base_url = match base {
        Some(b) => Some(state_machine::parse(b, None)?),
        None => None,
    };
    state_machine::parse(input, base_url.as_ref())
}

pub fn parse_to_href(input: &str, base: Option<&str>) -> Result<String, ()> {
    parse(input, base).map(|u| u.serialize())
}

pub fn can_parse(input: &str, base: Option<&str>) -> bool {
    parse(input, base).is_ok()
}

pub fn parse_or_none(input: &str, base: Option<&str>) -> Option<Url> {
    parse(input, base).ok()
}

pub fn join_to_href(base: &str, input: &str) -> Result<String, ()> {
    parse_to_href(input, Some(base))
}

pub fn set_component_href(href: &str, which: &str, val: &str) -> Option<String> {
    if which == "href" {
        return parse_to_href(val, None).ok();
    }
    let mut u = parse(href, None).ok()?;
    if u.set_component(which, val) {
        Some(u.serialize())
    } else {
        None
    }
}

pub fn path_to_file_href(path: &str) -> Result<String, ()> {
    let absolute_posix = path.starts_with('/');
    let bytes = path.as_bytes();
    let absolute_windows = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\');
    if !absolute_posix && !absolute_windows {
        return Err(());
    }
    let normalized = path.replace('\\', "/");
    let encoded =
        rusty_js_percent_encoding::encode(normalized.as_bytes(), &rusty_js_percent_encoding::PATH);
    if absolute_windows {
        Ok(format!("file:///{}", encoded))
    } else {
        Ok(format!("file://{}", encoded))
    }
}
