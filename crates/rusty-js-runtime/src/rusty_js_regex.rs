
use std::rc::Rc;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    OnceLock,
};

const RAW_SURROGATE_MARKER_BASE: u32 = 0xF0000;

static REGEXP_FIND_CALLS: AtomicU64 = AtomicU64::new(0);
static REGEXP_FIND_POSITIONS: AtomicU64 = AtomicU64::new(0);
static REGEXP_START_PRED_REJECTS: AtomicU64 = AtomicU64::new(0);
static REGEXP_FULL_MATCH_ATTEMPTS: AtomicU64 = AtomicU64::new(0);
static REGEXP_FIND_MATCHES: AtomicU64 = AtomicU64::new(0);
static REGEXP_SIMPLE_DIGIT_MATCHES: AtomicU64 = AtomicU64::new(0);

fn regexp_counters_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_REGEXP_COUNTERS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn regexp_counter_report_every() -> u64 {
    static EVERY: OnceLock<u64> = OnceLock::new();
    *EVERY.get_or_init(|| {
        std::env::var("CRUFT_REGEXP_COUNTERS_EVERY")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(100_000)
    })
}

fn regexp_simple_digit_fast_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_REGEXP_SIMPLE_DIGIT_FAST")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(true)
    })
}

fn maybe_report_regexp_counters(calls: u64) {
    if !regexp_counters_enabled() {
        return;
    }
    let every = regexp_counter_report_every();
    if calls % every != 0 {
        return;
    }
    eprintln!(
        "[regexp-counters] find_calls={} positions={} start_pred_rejects={} full_match_attempts={} matches={} simple_digit_matches={}",
        calls,
        REGEXP_FIND_POSITIONS.load(Ordering::Relaxed),
        REGEXP_START_PRED_REJECTS.load(Ordering::Relaxed),
        REGEXP_FULL_MATCH_ATTEMPTS.load(Ordering::Relaxed),
        REGEXP_FIND_MATCHES.load(Ordering::Relaxed),
        REGEXP_SIMPLE_DIGIT_MATCHES.load(Ordering::Relaxed)
    );
}

fn raw_surrogate_marker_to_unit(c: char) -> Option<u16> {
    let cp = c as u32;
    let offset = cp.checked_sub(RAW_SURROGATE_MARKER_BASE)?;
    if offset <= 0x7ff {
        Some(0xD800 + offset as u16)
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub enum Node {

    Char(char),

    Unit(u16),

    Never,

    AnyChar,

    Class(CharClass),

    Anchor(AnchorKind),

    Concat(Vec<Node>),

    Alt(Vec<Node>),

    Repeat {
        inner: Box<Node>,
        min: usize,
        max: Option<usize>,
        greedy: bool,
    },

    Group {
        index: usize,
        inner: Box<Node>,
    },

    NonCapture(Box<Node>),

    Modifier {
        on: (bool, bool, bool),
        off: (bool, bool, bool),
        inner: Box<Node>,
    },

    Look {
        ahead: bool,
        positive: bool,
        inner: Box<Node>,
    },

    Backref(usize),
    BackrefAny(Vec<usize>),

    BackrefName(String),

    CapStart(usize),
    CapEnd(usize),
}

#[derive(Debug, Clone, Copy)]
pub enum AnchorKind {
    Start,
    End,
    WordBoundary,
    NotWordBoundary,
}

#[derive(Debug, Clone)]
pub struct CharClass {
    pub negated: bool,
    pub expr: ClassExpr,

    fast_ranges: Option<Box<[(u32, u32)]>>,
}

#[derive(Debug, Clone)]
enum StartPredicate {
    Char(char),
    Unit(u16),
    Class(CharClass),
}

impl StartPredicate {
    fn matches_at(&self, units: &[u16], pos: usize, flags: &Flags) -> bool {
        match self {
            Self::Char(c) => {
                let mut buf = [0u16; 2];
                let enc = c.encode_utf16(&mut buf);
                if enc.len() == 1 {
                    unit_as_char(units, pos)
                        .is_some_and(|actual| char_eq(actual, *c, flags.ignore_case, flags.unicode))
                } else if flags.unicode {
                    matches!(
                        code_point_at(units, pos, true),
                        Some((Some(actual), _)) if char_eq(actual, *c, flags.ignore_case, true)
                    )
                } else {
                    units.get(pos) == Some(&enc[0]) && units.get(pos + 1) == Some(&enc[1])
                }
            }
            Self::Unit(unit) => {
                units.get(pos) == Some(unit)
                    && !is_paired_surrogate_position(units, pos, flags.unicode)
            }
            Self::Class(cc) => cc
                .match_at(units, pos, flags.ignore_case, flags.unicode)
                .is_some(),
        }
    }
}

fn flatten_union_ranges(expr: &ClassExpr) -> Option<Vec<(u32, u32)>> {
    fn collect(e: &ClassExpr, out: &mut Vec<(u32, u32)>) -> bool {
        match e {
            ClassExpr::Char(c) => {
                out.push((*c as u32, *c as u32));
                true
            }
            ClassExpr::Range(lo, hi) => {
                out.push((*lo as u32, *hi as u32));
                true
            }
            ClassExpr::Union(v) => v.iter().all(|x| collect(x, out)),
            _ => false,
        }
    }
    let mut ranges = Vec::new();
    if !collect(expr, &mut ranges) {
        return None;
    }

    if ranges.len() < 8 {
        return None;
    }
    ranges.sort_unstable();
    let mut merged: Vec<(u32, u32)> = Vec::with_capacity(ranges.len());
    for (lo, hi) in ranges {
        if let Some(last) = merged.last_mut() {
            if lo <= last.1.saturating_add(1) {
                last.1 = last.1.max(hi);
                continue;
            }
        }
        merged.push((lo, hi));
    }
    Some(merged)
}

#[derive(Debug, Clone)]
pub enum ClassExpr {

    Empty,
    Char(char),
    Range(char, char),

    Surrogate(u16),

    SurrogateRange(u16, u16),
    Special(SpecialClass),

    UnicodeRanges(&'static [(u32, u32)], bool),

    Union(Vec<ClassExpr>),

    Intersection(Vec<ClassExpr>),

    Difference(Box<ClassExpr>, Box<ClassExpr>),

    Not(Box<ClassExpr>),

    Strings(Vec<String>),
}

impl ClassExpr {

    fn matches(&self, c: char, ignore_case: bool, unicode: bool) -> bool {
        match self {
            ClassExpr::Empty => false,
            ClassExpr::Char(x) => char_eq(c, *x, ignore_case, unicode),
            ClassExpr::Surrogate(_) | ClassExpr::SurrogateRange(_, _) => false,
            ClassExpr::Range(lo, hi) => {
                if c >= *lo && c <= *hi {
                    return true;
                }
                if ignore_case {
                    range_contains_canonical(c, *lo, *hi, unicode)
                } else {
                    false
                }
            }
            ClassExpr::Special(s) => special_match(*s, c, ignore_case, unicode),
            ClassExpr::UnicodeRanges(ranges, negated) => {
                unicode_ranges_match(ranges, *negated, c, ignore_case)
            }
            ClassExpr::Union(v) => v.iter().any(|e| e.matches(c, ignore_case, unicode)),
            ClassExpr::Intersection(v) => v.iter().all(|e| e.matches(c, ignore_case, unicode)),
            ClassExpr::Difference(a, b) => {
                a.matches(c, ignore_case, unicode) && !b.matches(c, ignore_case, unicode)
            }
            ClassExpr::Not(e) => !e.matches(c, ignore_case, unicode),

            ClassExpr::Strings(v) => v.iter().any(|s| {
                let mut it = s.chars();
                matches!((it.next(), it.next()), (Some(x), None) if char_eq(c, x, ignore_case, unicode))
            }),
        }
    }

    fn match_lengths(
        &self,
        units: &[u16],
        pos: usize,
        ignore_case: bool,
        unicode: bool,
    ) -> Vec<usize> {
        match self {
            ClassExpr::Strings(items) => {
                let mut out = Vec::new();
                for item in items {
                    let need: Vec<u16> = item.encode_utf16().collect();
                    if pos + need.len() <= units.len()
                        && need.iter().enumerate().all(|(i, &u)| units[pos + i] == u)
                    {
                        out.push(need.len());
                    }
                }
                out
            }
            ClassExpr::Union(items) => {
                let mut out = Vec::new();
                for item in items {
                    out.extend(item.match_lengths(units, pos, ignore_case, unicode));
                }
                out.sort_unstable();
                out.dedup();
                out
            }
            ClassExpr::Intersection(items) => {
                let Some((first, rest)) = items.split_first() else {
                    return Vec::new();
                };
                let first_lengths = first.match_lengths(units, pos, ignore_case, unicode);
                first_lengths
                    .into_iter()
                    .filter(|len| {
                        rest.iter().all(|item| {
                            item.match_lengths(units, pos, ignore_case, unicode)
                                .contains(len)
                        })
                    })
                    .collect()
            }
            ClassExpr::Difference(left, right) => left
                .match_lengths(units, pos, ignore_case, unicode)
                .into_iter()
                .filter(|len| {
                    !right
                        .match_lengths(units, pos, ignore_case, unicode)
                        .contains(len)
                })
                .collect(),
            ClassExpr::Not(inner) => {
                let (cp, width) = match code_point_at(units, pos, unicode) {
                    Some(v) => v,
                    None => return Vec::new(),
                };
                let matches_inner = match cp {
                    Some(actual) => inner.matches(actual, ignore_case, unicode),
                    None => inner.matches_lone_surrogate(units[pos], ignore_case),
                };
                if matches_inner {
                    Vec::new()
                } else {
                    vec![width]
                }
            }
            _ => {
                let (cp, width) = match code_point_at(units, pos, unicode) {
                    Some(v) => v,
                    None => return Vec::new(),
                };
                let matches = match cp {
                    Some(actual) => self.matches(actual, ignore_case, unicode),
                    None => self.matches_lone_surrogate(units[pos], ignore_case),
                };
                if matches {
                    vec![width]
                } else {
                    Vec::new()
                }
            }
        }
    }

    fn collect_multichar_strings(&self, out: &mut Vec<String>) {
        match self {
            ClassExpr::Strings(v) => {
                for s in v {
                    if s.chars().count() != 1 {
                        out.push(s.clone());
                    }
                }
            }
            ClassExpr::Union(items) => {
                for e in items {
                    e.collect_multichar_strings(out);
                }
            }
            _ => {}
        }
    }

    fn has_surrogate(&self) -> bool {
        match self {
            ClassExpr::Surrogate(_) | ClassExpr::SurrogateRange(_, _) => true,
            ClassExpr::Range(lo, hi) => {
                let lo = *lo as u32;
                let hi = *hi as u32;
                lo <= 0xDFFF && hi >= 0xD800
            }
            ClassExpr::Union(items) | ClassExpr::Intersection(items) => {
                items.iter().any(ClassExpr::has_surrogate)
            }
            ClassExpr::Difference(a, b) => a.has_surrogate() || b.has_surrogate(),
            ClassExpr::Not(inner) => inner.has_surrogate(),
            _ => false,
        }
    }

    fn has_multichar_string(&self) -> bool {
        match self {
            ClassExpr::Strings(v) => v.iter().any(|s| s.encode_utf16().count() != 1),
            ClassExpr::Union(items) | ClassExpr::Intersection(items) => {
                items.iter().any(ClassExpr::has_multichar_string)
            }
            ClassExpr::Difference(a, b) => a.has_multichar_string() || b.has_multichar_string(),
            ClassExpr::Not(inner) => inner.has_multichar_string(),
            _ => false,
        }
    }

    fn matches_lone_surrogate(&self, unit: u16, ignore_case: bool) -> bool {
        match self {
            ClassExpr::Empty => false,
            ClassExpr::Surrogate(u) => *u == unit,
            ClassExpr::SurrogateRange(lo, hi) => unit >= *lo && unit <= *hi,
            ClassExpr::Range(lo, hi) => {
                (*lo as u32) <= unit as u32 && (unit as u32) <= (*hi as u32)
            }
            ClassExpr::Special(s) => special_match_lone_surrogate(*s, ignore_case),
            ClassExpr::UnicodeRanges(ranges, negated) => {

                range_table_contains(ranges, unit as u32) != *negated
            }
            ClassExpr::Union(items) => items
                .iter()
                .any(|e| e.matches_lone_surrogate(unit, ignore_case)),
            ClassExpr::Intersection(items) => items
                .iter()
                .all(|e| e.matches_lone_surrogate(unit, ignore_case)),
            ClassExpr::Difference(a, b) => {
                a.matches_lone_surrogate(unit, ignore_case)
                    && !b.matches_lone_surrogate(unit, ignore_case)
            }
            ClassExpr::Not(e) => !e.matches_lone_surrogate(unit, ignore_case),
            _ => false,
        }
    }
}

fn unicode_ranges_match(
    ranges: &'static [(u32, u32)],
    negated: bool,
    c: char,
    ignore_case: bool,
) -> bool {
    let in_set = |ch: char| range_table_contains(ranges, ch as u32) != negated;
    if in_set(c) {
        return true;
    }
    if !ignore_case {
        return false;
    }
    let key = unicode_case_key(c);
    if key != c && in_set(key) {
        return true;
    }
    let mut upper = c.to_uppercase();
    if let (Some(u), None) = (upper.next(), upper.next()) {
        if u != c && in_set(u) {
            return true;
        }
    }
    let mut key_upper = key.to_uppercase();
    if let (Some(u), None) = (key_upper.next(), key_upper.next()) {
        if u != c && u != key && in_set(u) {
            return true;
        }
    }
    false
}

fn range_table_contains(ranges: &[(u32, u32)], cp: u32) -> bool {
    ranges
        .binary_search_by(|&(lo, hi)| {
            if cp < lo {
                std::cmp::Ordering::Greater
            } else if cp > hi {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

#[derive(Debug, Clone, Copy)]
pub enum SpecialClass {
    Digit,
    NotDigit,
    Word,
    NotWord,
    Space,
    NotSpace,

    Property(PropKind, bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropKind {
    Letter,
    Uppercase,
    Lowercase,
    Number,
    Alphabetic,
    Whitespace,
    Punctuation,
    Mark,
    LineSeparator,
    ParagraphSeparator,
    Surrogate,
    AsciiHexDigit,
}

pub fn native_supports_property(name: &str) -> bool {
    prop_kind_from_name(name).is_some()
        || crate::generated_unicode::property_escapes::ranges_for(name).is_some()
}

pub fn native_property_needs_ignore_case_charset(name: &str) -> bool {
    matches!(
        prop_kind_from_name(name),
        Some(PropKind::Uppercase | PropKind::Lowercase)
    )
}

fn prop_kind_from_name(name: &str) -> Option<PropKind> {

    let key = name.rsplit('=').next().unwrap_or(name).trim();
    match key {
        "L" | "Letter" => Some(PropKind::Letter),
        "Lu" | "Uppercase_Letter" | "Uppercase" => Some(PropKind::Uppercase),
        "Ll" | "Lowercase_Letter" | "Lowercase" => Some(PropKind::Lowercase),
        "N" | "Nd" | "Number" | "Decimal_Number" => Some(PropKind::Number),
        "Alphabetic" | "Alpha" => Some(PropKind::Alphabetic),
        "White_Space" | "space" | "Space" => Some(PropKind::Whitespace),
        "P" | "Punctuation" => Some(PropKind::Punctuation),
        "M" | "Mark" => Some(PropKind::Mark),
        "Zl" | "Line_Separator" => Some(PropKind::LineSeparator),
        "Zp" | "Paragraph_Separator" => Some(PropKind::ParagraphSeparator),
        "Cs" | "Surrogate" => Some(PropKind::Surrogate),
        "ASCII_Hex_Digit" | "AHex" => Some(PropKind::AsciiHexDigit),
        _ => None,
    }
}

fn property_strings_from_name(name: &str) -> Option<Vec<String>> {
    match name {
        "Emoji_Keycap_Sequence" => Some(
            ['#', '*', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9']
                .into_iter()
                .map(|c| format!("{c}\u{FE0F}\u{20E3}"))
                .collect(),
        ),
        _ => None,
    }
}

impl CharClass {

    fn new(negated: bool, expr: ClassExpr) -> Self {
        let fast_ranges = flatten_union_ranges(&expr).map(Vec::into_boxed_slice);
        CharClass {
            negated,
            expr,
            fast_ranges,
        }
    }

    fn contains(&self, c: char, ignore_case: bool, unicode: bool) -> bool {
        let inner_match = if let Some(ranges) = self.fast_ranges.as_deref().filter(|_| !ignore_case)
        {

            let cp = c as u32;
            ranges
                .binary_search_by(|&(lo, hi)| {
                    if cp < lo {
                        std::cmp::Ordering::Greater
                    } else if cp > hi {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Equal
                    }
                })
                .is_ok()
        } else {
            self.expr.matches(c, ignore_case, unicode)
        };
        if self.negated {
            !inner_match
        } else {
            inner_match
        }
    }

    fn match_at(
        &self,
        units: &[u16],
        pos: usize,
        ignore_case: bool,
        unicode: bool,
    ) -> Option<usize> {

        if !self.negated && self.fast_ranges.is_none() {
            let mut lengths = self.expr.match_lengths(units, pos, ignore_case, unicode);
            if !lengths.is_empty() {
                lengths.sort_unstable_by(|a, b| b.cmp(a));
                return Some(pos + lengths[0]);
            }
        }

        let (cp, w) = code_point_at(units, pos, unicode)?;
        match cp {
            Some(actual) => {
                if self.contains(actual, ignore_case, unicode) {
                    Some(pos + w)
                } else {
                    None
                }
            }

            None => {
                let inner_match = self.expr.matches_lone_surrogate(units[pos], ignore_case);
                if self.negated != inner_match {
                    Some(pos + w)
                } else {
                    None
                }
            }
        }
    }
}

fn is_js_regexp_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0009}'
            | '\u{000A}'
            | '\u{000B}'
            | '\u{000C}'
            | '\u{000D}'
            | '\u{0020}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

fn is_word_char(c: char, ignore_case: bool, unicode: bool) -> bool {
    c.is_ascii_alphanumeric()
        || c == '_'
        || (ignore_case && unicode && matches!(c, '\u{017F}' | '\u{212A}'))
}

fn special_match(s: SpecialClass, c: char, ignore_case: bool, unicode: bool) -> bool {
    match s {
        SpecialClass::Digit => c.is_ascii_digit(),
        SpecialClass::NotDigit => !c.is_ascii_digit(),
        SpecialClass::Word => is_word_char(c, ignore_case, unicode),
        SpecialClass::NotWord => !is_word_char(c, ignore_case, unicode),
        SpecialClass::Space => is_js_regexp_whitespace(c),
        SpecialClass::NotSpace => !is_js_regexp_whitespace(c),
        SpecialClass::Property(kind, negate) => {
            if ignore_case && matches!(kind, PropKind::Uppercase | PropKind::Lowercase) {
                let cased = c.is_uppercase() || c.is_lowercase();
                return if negate { true } else { cased };
            }
            let base = match kind {
                PropKind::Letter | PropKind::Alphabetic => c.is_alphabetic(),
                PropKind::Uppercase => c.is_uppercase(),
                PropKind::Lowercase => c.is_lowercase(),
                PropKind::Number => c.is_numeric(),
                PropKind::Whitespace => c.is_whitespace(),
                PropKind::Punctuation => c.is_ascii_punctuation(),
                PropKind::Mark => false,
                PropKind::LineSeparator => c == '\u{2028}',
                PropKind::ParagraphSeparator => c == '\u{2029}',
                PropKind::Surrogate => false,
                PropKind::AsciiHexDigit => c.is_ascii_hexdigit(),
            };
            base != negate
        }
    }
}

fn special_match_lone_surrogate(s: SpecialClass, _ignore_case: bool) -> bool {
    match s {
        SpecialClass::Property(PropKind::Surrogate, negate) => !negate,
        SpecialClass::Property(_, negate) => negate,
        SpecialClass::NotDigit | SpecialClass::NotWord | SpecialClass::NotSpace => true,
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Flags {
    pub ignore_case: bool,
    pub multiline: bool,
    pub dot_all: bool,

    pub unicode: bool,
}

#[derive(Debug, Clone)]
pub struct HandRolledRegex {
    pub source: Rc<String>,
    pub flags: Flags,
    pub ast: Node,
    start_predicate: Option<StartPredicate>,
    simple_digit: Option<SimpleDigitRegex>,
    pub group_count: usize,

    pub named_groups: std::collections::HashMap<String, usize>,
    pub named_group_order: Vec<(String, usize)>,

    no_backref: bool,
}

pub struct HandMatch {
    pub start: usize,
    pub end: usize,

    pub captures: Vec<Option<(usize, usize)>>,
}

#[derive(Debug, Clone)]
struct SimpleDigitRegex {
    atoms: Vec<SimpleDigitAtom>,
    group_count: usize,
}

#[derive(Debug, Clone)]
enum SimpleDigitAtom {
    Unit(u16),
    OneOf(Vec<u16>),
    CaptureDigits { index: usize },
}

fn decode_group_name(raw: &str) -> Option<String> {
    let chars: Vec<char> = raw.chars().collect();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] != '\\' {
            if let Some(high) = raw_surrogate_marker_to_unit(chars[i]) {
                if (0xD800..=0xDBFF).contains(&high) {
                    if let Some(low) = chars
                        .get(i + 1)
                        .and_then(|next| raw_surrogate_marker_to_unit(*next))
                    {
                        if (0xDC00..=0xDFFF).contains(&low) {
                            let cp =
                                0x10000 + (((high as u32 - 0xD800) << 10) | (low as u32 - 0xDC00));
                            out.push(char::from_u32(cp)?);
                            i += 2;
                            continue;
                        }
                    }
                }
            }
            out.push(chars[i]);
            i += 1;
            continue;
        }
        if chars.get(i + 1) != Some(&'u') {
            return None;
        }
        if chars.get(i + 2) == Some(&'{') {
            let mut j = i + 3;
            let mut hex = String::new();
            while j < chars.len() && chars[j] != '}' {
                if !chars[j].is_ascii_hexdigit() {
                    return None;
                }
                hex.push(chars[j]);
                j += 1;
            }
            if hex.is_empty() || chars.get(j) != Some(&'}') {
                return None;
            }
            let cp = u32::from_str_radix(&hex, 16).ok()?;
            out.push(char::from_u32(cp)?);
            i = j + 1;
            continue;
        }
        if i + 6 > chars.len() {
            return None;
        }
        let hex: String = chars[i + 2..i + 6].iter().collect();
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let unit = u32::from_str_radix(&hex, 16).ok()?;
        if (0xD800..=0xDBFF).contains(&unit)
            && i + 12 <= chars.len()
            && chars.get(i + 6) == Some(&'\\')
            && chars.get(i + 7) == Some(&'u')
        {
            let low_hex: String = chars[i + 8..i + 12].iter().collect();
            if low_hex.chars().all(|c| c.is_ascii_hexdigit()) {
                let low = u32::from_str_radix(&low_hex, 16).ok()?;
                if (0xDC00..=0xDFFF).contains(&low) {
                    let cp = 0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00);
                    out.push(char::from_u32(cp)?);
                    i += 12;
                    continue;
                }
            }
        }
        out.push(char::from_u32(unit)?);
        i += 6;
    }
    Some(out)
}

fn literal_node(s: &str) -> Node {
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (None, _) => Node::Concat(Vec::new()),
        (Some(c), None) => raw_surrogate_marker_to_unit(c).map_or(Node::Char(c), Node::Unit),
        (Some(first), Some(second)) => {
            let mut nodes = vec![
                raw_surrogate_marker_to_unit(first).map_or(Node::Char(first), Node::Unit),
                raw_surrogate_marker_to_unit(second).map_or(Node::Char(second), Node::Unit),
            ];
            nodes.extend(
                chars.map(|c| raw_surrogate_marker_to_unit(c).map_or(Node::Char(c), Node::Unit)),
            );
            Node::Concat(nodes)
        }
    }
}

fn first_consuming_predicate(node: &Node) -> Option<StartPredicate> {
    match node {
        Node::Char(c) => Some(StartPredicate::Char(*c)),
        Node::Unit(u) => Some(StartPredicate::Unit(*u)),
        Node::Class(cc) => Some(StartPredicate::Class(cc.clone())),
        Node::Group { inner, .. } | Node::NonCapture(inner) => first_consuming_predicate(inner),

        Node::Modifier { inner, on, off } if !on.0 && !off.0 => first_consuming_predicate(inner),
        Node::Modifier { .. } => None,
        Node::Repeat { inner, min, .. } if *min > 0 => first_consuming_predicate(inner),
        Node::Concat(items) => {
            for item in items {
                match item {
                    Node::Anchor(_) | Node::CapStart(_) | Node::CapEnd(_) => continue,
                    Node::Group { inner, .. } | Node::NonCapture(inner) => {
                        if let Some(pred) = first_consuming_predicate(inner) {
                            return Some(pred);
                        }
                        return None;
                    }

                    Node::Modifier { inner, on, off } if !on.0 && !off.0 => {
                        if let Some(pred) = first_consuming_predicate(inner) {
                            return Some(pred);
                        }
                        return None;
                    }
                    Node::Modifier { .. } => return None,
                    Node::Repeat { min, .. } if *min == 0 => return None,
                    other => return first_consuming_predicate(other),
                }
            }
            None
        }
        _ => None,
    }
}

fn group_name_start_candidate(c: char) -> bool {
    c.is_alphabetic()
        || c == '_'
        || c == '$'
        || c == '\\'
        || raw_surrogate_marker_to_unit(c).is_some()
}

fn try_compile_simple_digit_regex(
    ast: &Node,
    flags: &Flags,
    group_count: usize,
) -> Option<SimpleDigitRegex> {
    if flags.ignore_case || flags.multiline || flags.dot_all || flags.unicode {
        return None;
    }
    let mut atoms = Vec::new();
    collect_simple_digit_atoms(ast, &mut atoms)?;
    if atoms
        .iter()
        .any(|atom| matches!(atom, SimpleDigitAtom::CaptureDigits { .. }))
    {
        Some(SimpleDigitRegex { atoms, group_count })
    } else {
        None
    }
}

fn collect_simple_digit_atoms(node: &Node, out: &mut Vec<SimpleDigitAtom>) -> Option<()> {
    match node {
        Node::Concat(items) => {
            for item in items {
                collect_simple_digit_atoms(item, out)?;
            }
            Some(())
        }
        Node::Char(c) => {
            let mut buf = [0u16; 2];
            let enc = c.encode_utf16(&mut buf);
            if enc.len() == 1 && buf[0] < 0x80 {
                out.push(SimpleDigitAtom::Unit(buf[0]));
                Some(())
            } else {
                None
            }
        }
        Node::Class(class) => {
            let units = simple_ascii_class_units(class)?;
            out.push(SimpleDigitAtom::OneOf(units));
            Some(())
        }
        Node::Group { index, inner } if is_simple_digit_plus(inner) => {
            out.push(SimpleDigitAtom::CaptureDigits { index: *index });
            Some(())
        }
        _ => None,
    }
}

fn is_simple_digit_plus(node: &Node) -> bool {
    let Node::Repeat {
        inner,
        min,
        max,
        greedy,
    } = node
    else {
        return false;
    };
    *min == 1 && max.is_none() && *greedy && matches_digit_class(inner)
}

fn matches_digit_class(node: &Node) -> bool {
    matches!(
        node,
        Node::Class(CharClass {
            negated: false,
            expr: ClassExpr::Special(SpecialClass::Digit),
            ..
        })
    )
}

fn simple_ascii_class_units(class: &CharClass) -> Option<Vec<u16>> {
    if class.negated {
        return None;
    }
    let mut units = Vec::new();
    collect_simple_ascii_class_units(&class.expr, &mut units)?;
    units.sort_unstable();
    units.dedup();
    if units.is_empty() {
        None
    } else {
        Some(units)
    }
}

fn collect_simple_ascii_class_units(expr: &ClassExpr, out: &mut Vec<u16>) -> Option<()> {
    match expr {
        ClassExpr::Char(c) => {
            let mut buf = [0u16; 2];
            let enc = c.encode_utf16(&mut buf);
            if enc.len() == 1 && buf[0] < 0x80 {
                out.push(buf[0]);
                Some(())
            } else {
                None
            }
        }
        ClassExpr::Union(items) => {
            for item in items {
                collect_simple_ascii_class_units(item, out)?;
            }
            Some(())
        }
        _ => None,
    }
}

pub fn compile(pattern: &str, flag_str: &str) -> Result<HandRolledRegex, String> {
    let mut flags = Flags::default();
    for c in flag_str.chars() {
        match c {
            'i' => flags.ignore_case = true,
            'm' => flags.multiline = true,
            's' => flags.dot_all = true,

            'u' | 'v' => flags.unicode = true,
            'g' | 'y' | 'd' => {}
            _ => return Err(format!("unsupported flag '{}'", c)),
        }
    }
    let mut p = Parser {
        chars: pattern.chars().collect(),
        pos: 0,
        next_group: 1,
        group_count: 0,
        unicode: flag_str.contains('u') || flag_str.contains('v'),
        v_sets: flag_str.contains('v'),
        named_groups: std::collections::HashMap::new(),
        named_group_order: Vec::new(),
    };
    let ast = p.parse_alt()?;
    if p.pos < p.chars.len() {
        return Err(format!(
            "unexpected '{}' at position {}",
            p.chars[p.pos], p.pos
        ));
    }
    let ast = normalize_annex_b_decimal_escapes(ast, p.group_count);

    let ast = resolve_named_backrefs(ast, &p.named_group_order, p.unicode)?;

    let ast = flatten_noncapture(ast);
    let named_groups = p.named_groups.clone();
    let named_group_order = p.named_group_order.clone();
    let start_predicate = first_consuming_predicate(&ast);
    let simple_digit = try_compile_simple_digit_regex(&ast, &flags, p.group_count);
    let no_backref = !has_backreference(&ast);
    Ok(HandRolledRegex {
        source: Rc::new(pattern.to_string()),
        flags,
        start_predicate,
        simple_digit,
        ast,
        group_count: p.group_count,
        named_groups,
        named_group_order,
        no_backref,
    })
}

fn flatten_noncapture(node: Node) -> Node {
    match node {
        Node::NonCapture(inner) => flatten_noncapture(*inner),
        Node::Concat(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                match flatten_noncapture(item) {
                    Node::Concat(v) => out.extend(v),
                    other => out.push(other),
                }
            }
            Node::Concat(out)
        }
        Node::Alt(branches) => Node::Alt(branches.into_iter().map(flatten_noncapture).collect()),
        Node::Repeat {
            inner,
            min,
            max,
            greedy,
        } => Node::Repeat {
            inner: Box::new(flatten_noncapture(*inner)),
            min,
            max,
            greedy,
        },
        Node::Group { index, inner } => Node::Group {
            index,
            inner: Box::new(flatten_noncapture(*inner)),
        },
        Node::Modifier { on, off, inner } => Node::Modifier {
            on,
            off,
            inner: Box::new(flatten_noncapture(*inner)),
        },
        Node::Look {
            ahead,
            positive,
            inner,
        } => Node::Look {
            ahead,
            positive,
            inner: Box::new(flatten_noncapture(*inner)),
        },
        other => other,
    }
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
    next_group: usize,
    group_count: usize,
    unicode: bool,

    v_sets: bool,

    named_groups: std::collections::HashMap<String, usize>,
    named_group_order: Vec<(String, usize)>,
}

fn class_endpoint_code_unit(expr: &ClassExpr) -> Option<u16> {
    match expr {
        ClassExpr::Char(c) if (*c as u32) <= 0xFFFF => Some(*c as u16),
        ClassExpr::Surrogate(u) => Some(*u),
        _ => None,
    }
}

fn class_range_expr(lo: u16, hi: u16) -> ClassExpr {
    let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    let mut parts = Vec::new();
    if lo < 0xD800 {
        let end = hi.min(0xD7FF);
        if let (Some(a), Some(b)) = (char::from_u32(lo as u32), char::from_u32(end as u32)) {
            parts.push(ClassExpr::Range(a, b));
        }
    }
    if lo <= 0xDFFF && hi >= 0xD800 {
        parts.push(ClassExpr::SurrogateRange(lo.max(0xD800), hi.min(0xDFFF)));
    }
    if hi > 0xDFFF {
        let start = lo.max(0xE000);
        if let (Some(a), Some(b)) = (char::from_u32(start as u32), char::from_u32(hi as u32)) {
            parts.push(ClassExpr::Range(a, b));
        }
    }
    match parts.len() {
        0 => ClassExpr::Empty,
        1 => parts.pop().unwrap(),
        _ => ClassExpr::Union(parts),
    }
}

fn relex_legacy_decimal_escape(digits: &str) -> Node {
    let chars: Vec<char> = digits.chars().collect();
    if chars.is_empty() {
        return Node::Char('\0');
    }
    let mut nodes: Vec<Node> = Vec::new();
    let mut i = 0;

    if chars[0] == '8' || chars[0] == '9' {

        nodes.push(Node::Char(chars[0]));
        i = 1;
    } else {

        let max = if chars[0] <= '3' { 3 } else { 2 };
        let mut val: u32 = 0;
        let mut consumed = 0;
        while consumed < max && i < chars.len() {
            if let Some(d) = chars[i].to_digit(8) {
                val = val * 8 + d;
                i += 1;
                consumed += 1;
            } else {
                break;
            }
        }
        nodes.push(Node::Char(char::from_u32(val).unwrap_or('\0')));
    }

    while i < chars.len() {
        nodes.push(Node::Char(chars[i]));
        i += 1;
    }
    match nodes.len() {
        1 => nodes.pop().unwrap(),
        _ => Node::Concat(nodes),
    }
}

fn normalize_annex_b_decimal_escapes(node: Node, group_count: usize) -> Node {
    match node {
        Node::Backref(n) if n > group_count => relex_legacy_decimal_escape(&n.to_string()),
        Node::Concat(nodes) => Node::Concat(
            nodes
                .into_iter()
                .map(|n| normalize_annex_b_decimal_escapes(n, group_count))
                .collect(),
        ),
        Node::Alt(nodes) => Node::Alt(
            nodes
                .into_iter()
                .map(|n| normalize_annex_b_decimal_escapes(n, group_count))
                .collect(),
        ),
        Node::Repeat {
            inner,
            min,
            max,
            greedy,
        } => Node::Repeat {
            inner: Box::new(normalize_annex_b_decimal_escapes(*inner, group_count)),
            min,
            max,
            greedy,
        },
        Node::Group { index, inner } => Node::Group {
            index,
            inner: Box::new(normalize_annex_b_decimal_escapes(*inner, group_count)),
        },
        Node::Modifier { on, off, inner } => Node::Modifier {
            on,
            off,
            inner: Box::new(normalize_annex_b_decimal_escapes(*inner, group_count)),
        },
        Node::NonCapture(inner) => Node::NonCapture(Box::new(normalize_annex_b_decimal_escapes(
            *inner,
            group_count,
        ))),
        Node::Look {
            ahead,
            positive,
            inner,
        } => Node::Look {
            ahead,
            positive,
            inner: Box::new(normalize_annex_b_decimal_escapes(*inner, group_count)),
        },
        other => other,
    }
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }
    fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn try_pair_low_surrogate(&mut self, high: u32) -> Option<char> {
        if !self.unicode || !(0xD800..=0xDBFF).contains(&high) {
            return None;
        }
        let save = self.pos;
        if self.eat('\\') && self.eat('u') && self.peek() != Some('{') {
            let mut low: u32 = 0;
            let mut ok = true;
            for _ in 0..4 {
                match self.peek().and_then(|c| c.to_digit(16)) {
                    Some(d) => {
                        low = low * 16 + d;
                        self.bump();
                    }
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok && (0xDC00..=0xDFFF).contains(&low) {
                let cp = 0x10000 + (high - 0xD800) * 0x400 + (low - 0xDC00);
                if let Some(ch) = char::from_u32(cp) {
                    return Some(ch);
                }
            }
        }
        self.pos = save;
        None
    }

    fn parse_alt(&mut self) -> Result<Node, String> {
        let first = self.parse_concat()?;
        let mut alts = vec![first];
        while self.eat('|') {
            alts.push(self.parse_concat()?);
        }
        Ok(if alts.len() == 1 {
            alts.pop().unwrap()
        } else {
            Node::Alt(alts)
        })
    }

    fn parse_concat(&mut self) -> Result<Node, String> {
        let mut items = Vec::new();
        loop {
            match self.peek() {
                None | Some(')') | Some('|') => break,
                _ => {
                    let atom = self.parse_atom()?;
                    let with_quant = self.maybe_quantifier(atom)?;
                    items.push(with_quant);
                }
            }
        }
        Ok(if items.len() == 1 {
            items.pop().unwrap()
        } else {
            Node::Concat(items)
        })
    }

    fn maybe_quantifier(&mut self, inner: Node) -> Result<Node, String> {
        let (min, max) = match self.peek() {
            Some('?') => {
                self.bump();
                (0, Some(1))
            }
            Some('*') => {
                self.bump();
                (0, None)
            }
            Some('+') => {
                self.bump();
                (1, None)
            }
            Some('{') => {
                let save = self.pos;
                self.bump();

                let mut min_str = String::new();
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() {
                        min_str.push(c);
                        self.bump();
                    } else {
                        break;
                    }
                }
                if min_str.is_empty() {

                    self.pos = save;
                    return Ok(inner);
                }
                let min_v: usize = min_str.parse().map_err(|_| "bad quantifier min")?;
                let (max_v, ok): (Option<usize>, bool) = if self.eat(',') {
                    let mut s = String::new();
                    while let Some(c) = self.peek() {
                        if c.is_ascii_digit() {
                            s.push(c);
                            self.bump();
                        } else {
                            break;
                        }
                    }
                    if s.is_empty() {
                        (None, self.eat('}'))
                    } else {
                        let v: usize = s.parse().map_err(|_| "bad quantifier max")?;
                        (Some(v), self.eat('}'))
                    }
                } else {
                    (Some(min_v), self.eat('}'))
                };
                if !ok {
                    self.pos = save;
                    return Ok(inner);
                }
                (min_v, max_v)
            }
            _ => return Ok(inner),
        };
        let greedy = !self.eat('?');
        Ok(Node::Repeat {
            inner: Box::new(inner),
            min,
            max,
            greedy,
        })
    }

    fn parse_atom(&mut self) -> Result<Node, String> {
        let c = self.peek().ok_or("unexpected end")?;
        match c {
            '^' => {
                self.bump();
                Ok(Node::Anchor(AnchorKind::Start))
            }
            '$' => {
                self.bump();
                Ok(Node::Anchor(AnchorKind::End))
            }
            '.' => {
                self.bump();
                Ok(Node::AnyChar)
            }
            '\\' => self.parse_escape(),
            '[' => self.parse_class(),
            '(' => self.parse_group(),
            _ => {
                self.bump();
                Ok(raw_surrogate_marker_to_unit(c).map_or(Node::Char(c), Node::Unit))
            }
        }
    }

    fn parse_escape(&mut self) -> Result<Node, String> {
        self.bump();
        let c = self.bump().ok_or("trailing backslash")?;
        match c {
            'd' => Ok(Node::Class(CharClass::new(
                false,
                ClassExpr::Special(SpecialClass::Digit),
            ))),
            'D' => Ok(Node::Class(CharClass::new(
                false,
                ClassExpr::Special(SpecialClass::NotDigit),
            ))),
            'w' => Ok(Node::Class(CharClass::new(
                false,
                ClassExpr::Special(SpecialClass::Word),
            ))),
            'W' => Ok(Node::Class(CharClass::new(
                false,
                ClassExpr::Special(SpecialClass::NotWord),
            ))),
            's' => Ok(Node::Class(CharClass::new(
                false,
                ClassExpr::Special(SpecialClass::Space),
            ))),
            'S' => Ok(Node::Class(CharClass::new(
                false,
                ClassExpr::Special(SpecialClass::NotSpace),
            ))),
            'p' | 'P' => {
                let negate = c == 'P';

                if !self.unicode {
                    return Ok(Node::Char(c));
                }
                if !self.eat('{') {
                    return Err("expected '{' after \\p".into());
                }
                let mut name = String::new();
                while let Some(ch) = self.peek() {
                    if ch == '}' {
                        break;
                    }
                    name.push(ch);
                    self.bump();
                }
                if !self.eat('}') {
                    return Err("unterminated \\p{...}".into());
                }
                let expr = if let Some(ranges) =
                    crate::generated_unicode::property_escapes::ranges_for(&name)
                {

                    ClassExpr::UnicodeRanges(ranges, negate)
                } else if let Some(kind) = prop_kind_from_name(&name) {
                    ClassExpr::Special(SpecialClass::Property(kind, negate))
                } else {
                    return Err(format!("unsupported \\p property '{}'", name));
                };
                Ok(Node::Class(CharClass::new(false, expr)))
            }
            'b' => Ok(Node::Anchor(AnchorKind::WordBoundary)),
            'B' => Ok(Node::Anchor(AnchorKind::NotWordBoundary)),
            'n' => Ok(Node::Char('\n')),
            't' => Ok(Node::Char('\t')),
            'r' => Ok(Node::Char('\r')),
            'f' => Ok(Node::Char('\u{000C}')),
            'v' => Ok(Node::Char('\u{000B}')),
            '0' => {

                if !self.unicode && matches!(self.peek(), Some('0'..='7')) {
                    let mut val: u32 = 0;
                    let mut consumed = 1;
                    while consumed < 3 {
                        match self.peek() {
                            Some(d @ '0'..='7') => {
                                self.bump();
                                val = val * 8 + (d as u32 - '0' as u32);
                                consumed += 1;
                            }
                            _ => break,
                        }
                    }
                    Ok(Node::Char(char::from_u32(val).unwrap_or('\0')))
                } else {
                    Ok(Node::Char('\0'))
                }
            }
            'c' => match self.peek() {
                Some(ctrl) if ctrl.is_ascii_alphabetic() => {
                    self.bump();
                    Ok(Node::Char(
                        char::from_u32((ctrl as u32) % 32).unwrap_or('\0'),
                    ))
                }
                Some(other) => {
                    self.bump();
                    Ok(Node::Concat(vec![
                        Node::Char('\\'),
                        Node::Char('c'),
                        Node::Char(other),
                    ]))
                }
                None => Ok(Node::Concat(vec![Node::Char('\\'), Node::Char('c')])),
            },
            'x' => {

                let h1 = self.peek().filter(|c| c.is_ascii_hexdigit());
                let h2 = self
                    .chars
                    .get(self.pos + 1)
                    .copied()
                    .filter(|c| c.is_ascii_hexdigit());
                match (h1, h2) {
                    (Some(a), Some(b)) => {
                        self.bump();
                        self.bump();
                        let n = u32::from_str_radix(&format!("{}{}", a, b), 16)
                            .map_err(|_| "bad \\x escape")?;
                        Ok(Node::Char(char::from_u32(n).ok_or("bad \\x escape")?))
                    }
                    _ if !self.unicode => Ok(Node::Char('x')),
                    _ => Err("bad \\x escape".into()),
                }
            }
            'u' => {
                if !self.unicode && self.peek() == Some('{') {
                    return Ok(Node::Char('u'));
                }
                if self.eat('{') {
                    let mut s = String::new();
                    while let Some(c) = self.peek() {
                        if c == '}' {
                            break;
                        }
                        s.push(c);
                        self.bump();
                    }
                    if !self.eat('}') {
                        return Err("unterminated \\u{...}".into());
                    }
                    let n = u32::from_str_radix(&s, 16).map_err(|_| "bad \\u{...} escape")?;
                    match char::from_u32(n) {
                        Some(ch) => Ok(Node::Char(ch)),
                        None if self.unicode && (0xD800..=0xDFFF).contains(&n) => {
                            Ok(Node::Unit(n as u16))
                        }
                        None => Err("bad \\u escape".into()),
                    }
                } else {

                    let s: String = (0..4)
                        .map_while(|i| {
                            self.chars
                                .get(self.pos + i)
                                .copied()
                                .filter(|c| c.is_ascii_hexdigit())
                        })
                        .collect();
                    if s.len() == 4 {
                        for _ in 0..4 {
                            self.bump();
                        }
                        let n = u32::from_str_radix(&s, 16).map_err(|_| "bad \\u escape")?;
                        if let Some(ch) = self.try_pair_low_surrogate(n) {
                            Ok(Node::Char(ch))
                        } else {
                            match char::from_u32(n) {
                                Some(ch) => Ok(Node::Char(ch)),
                                None if (0xD800..=0xDFFF).contains(&n) => Ok(Node::Unit(n as u16)),
                                None => Err("bad \\u escape".into()),
                            }
                        }
                    } else if !self.unicode {
                        Ok(Node::Char('u'))
                    } else {
                        Err("bad \\u escape".into())
                    }
                }
            }
            d if d.is_ascii_digit() && d != '0' => {
                let mut n = (d as u8 - b'0') as usize;
                while let Some(next) = self.peek() {
                    if !next.is_ascii_digit() {
                        break;
                    }
                    self.bump();
                    n = n
                        .saturating_mul(10)
                        .saturating_add((next as u8 - b'0') as usize);
                }
                Ok(Node::Backref(n))
            }
            'k' => {

                if !self.eat('<') {

                    if !self.unicode {
                        return Ok(Node::Char('k'));
                    }
                    return Err("expected < after \\k".into());
                }
                let mut name = String::new();
                while let Some(c) = self.peek() {
                    if c == '>' {
                        break;
                    }
                    name.push(c);
                    self.bump();
                }
                if !self.eat('>') {
                    if !self.unicode {
                        return Ok(literal_node(&format!("k<{}", name)));
                    }
                    return Err("unterminated named backref".into());
                }
                let name = decode_group_name(&name).unwrap_or(name);
                let slots: Vec<usize> = self
                    .named_group_order
                    .iter()
                    .filter_map(|(n, idx)| if n == &name { Some(*idx) } else { None })
                    .collect();
                if slots.is_empty() {

                    Ok(Node::BackrefName(name))
                } else if slots.len() == 1 {
                    Ok(Node::Backref(slots[0]))
                } else {
                    Ok(Node::BackrefAny(slots))
                }
            }

            _ => Ok(Node::Char(c)),
        }
    }

    fn parse_class(&mut self) -> Result<Node, String> {
        self.bump();
        let negated = self.eat('^');
        if self.peek() == Some(']') {
            self.bump();
            return Ok(Node::Class(CharClass::new(negated, ClassExpr::Empty)));
        }

        let first = self.parse_class_operand()?;

        let is_intersection =
            self.v_sets && self.peek() == Some('&') && self.chars.get(self.pos + 1) == Some(&'&');
        let is_difference =
            self.v_sets && self.peek() == Some('-') && self.chars.get(self.pos + 1) == Some(&'-');
        if is_intersection || is_difference {
            let (a, b) = if is_intersection {
                ('&', '&')
            } else {
                ('-', '-')
            };
            let mut operands = vec![first];
            while self.peek() == Some(a) && self.chars.get(self.pos + 1) == Some(&b) {
                self.bump();
                self.bump();
                operands.push(self.parse_class_operand()?);
            }
            if !self.eat(']') {
                return Err("unterminated class".into());
            }
            let expr = if is_intersection {
                ClassExpr::Intersection(operands)
            } else {
                let mut it = operands.into_iter();
                let mut acc = it.next().unwrap();
                for r in it {
                    acc = ClassExpr::Difference(Box::new(acc), Box::new(r));
                }
                acc
            };
            return Ok(Node::Class(CharClass::new(negated, expr)));
        }

        let mut operands = vec![first];
        while self.peek() != Some(']') {
            if self.pos >= self.chars.len() {
                return Err("unterminated class".into());
            }
            operands.push(self.parse_class_operand()?);
        }
        self.bump();
        let expr = if operands.len() == 1 {
            operands.pop().unwrap()
        } else {
            ClassExpr::Union(operands)
        };

        if negated {
            let mut strs = Vec::new();
            expr.collect_multichar_strings(&mut strs);
            if !strs.is_empty() {
                return Err("negated character class may not contain strings".into());
            }
        }
        Ok(Node::Class(CharClass::new(negated, expr)))
    }

    fn parse_class_operand(&mut self) -> Result<ClassExpr, String> {

        if self.v_sets && self.peek() == Some('[') {
            return match self.parse_class()? {
                Node::Class(cc) => Ok(if cc.negated {
                    ClassExpr::Not(Box::new(cc.expr))
                } else {
                    cc.expr
                }),
                _ => Err("nested class parse error".into()),
            };
        }
        let lo = self.parse_class_atom()?;

        let lo_scalar = if let ClassExpr::Char(c) = &lo {
            Some(*c)
        } else {
            None
        };
        if (class_endpoint_code_unit(&lo).is_some() || lo_scalar.is_some())
            && self.peek() == Some('-')
            && self.chars.get(self.pos + 1) != Some(&']')
            && self.chars.get(self.pos + 1) != Some(&'-')
        {
            self.bump();
            let hi = self.parse_class_atom()?;

            if let (Some(l), Some(h)) =
                (class_endpoint_code_unit(&lo), class_endpoint_code_unit(&hi))
            {
                return Ok(class_range_expr(l, h));
            }

            if let (Some(lc), ClassExpr::Char(hc)) = (lo_scalar, &hi) {
                let (a, b) = if lc <= *hc { (lc, *hc) } else { (*hc, lc) };
                return Ok(ClassExpr::Range(a, b));
            }

            return Ok(ClassExpr::Union(vec![lo, ClassExpr::Char('-'), hi]));
        }
        Ok(lo)
    }

    fn read_class_hex_escape(&mut self, kind: char) -> Result<u32, String> {
        if kind == 'x' {
            let h1 = self
                .bump()
                .and_then(|c| c.to_digit(16))
                .ok_or("bad \\x in \\q{}")?;
            let h2 = self
                .bump()
                .and_then(|c| c.to_digit(16))
                .ok_or("bad \\x in \\q{}")?;
            Ok(h1 * 16 + h2)
        } else if self.eat('{') {
            let mut hex = String::new();
            while let Some(c) = self.peek() {
                if c == '}' {
                    break;
                }
                hex.push(c);
                self.bump();
            }
            if !self.eat('}') {
                return Err("unterminated \\u{...} in \\q{}".into());
            }
            u32::from_str_radix(&hex, 16).map_err(|_| "bad \\u{...} in \\q{}".to_string())
        } else {
            let mut hex = String::new();
            for _ in 0..4 {
                hex.push(self.bump().ok_or("bad \\u in \\q{}")?);
            }
            u32::from_str_radix(&hex, 16).map_err(|_| "bad \\u in \\q{}".to_string())
        }
    }

    fn parse_class_atom(&mut self) -> Result<ClassExpr, String> {
        let c = self.peek().ok_or("unexpected end in class")?;
        if c != '\\' {
            self.bump();
            if let Some(unit) = raw_surrogate_marker_to_unit(c) {
                return Ok(ClassExpr::Surrogate(unit));
            }

            if !self.unicode && (c as u32) > 0xFFFF {
                let mut buf = [0u16; 2];
                c.encode_utf16(&mut buf);
                return Ok(ClassExpr::Union(vec![
                    ClassExpr::Surrogate(buf[0]),
                    ClassExpr::Surrogate(buf[1]),
                ]));
            }
            return Ok(ClassExpr::Char(c));
        }
        self.bump();
        let e = self.bump().ok_or("trailing backslash in class")?;
        match e {
            'd' => Ok(ClassExpr::Special(SpecialClass::Digit)),
            'D' => Ok(ClassExpr::Special(SpecialClass::NotDigit)),
            'w' => Ok(ClassExpr::Special(SpecialClass::Word)),
            'W' => Ok(ClassExpr::Special(SpecialClass::NotWord)),
            's' => Ok(ClassExpr::Special(SpecialClass::Space)),
            'S' => Ok(ClassExpr::Special(SpecialClass::NotSpace)),
            'n' => Ok(ClassExpr::Char('\n')),
            't' => Ok(ClassExpr::Char('\t')),
            'r' => Ok(ClassExpr::Char('\r')),
            'f' => Ok(ClassExpr::Char('\u{000C}')),
            'v' => Ok(ClassExpr::Char('\u{000B}')),

            '0'..='7' if !self.unicode => {
                let mut val = e.to_digit(8).unwrap();
                let max = if e <= '3' { 3 } else { 2 };
                let mut consumed = 1;
                while consumed < max {
                    match self.peek().and_then(|c| c.to_digit(8)) {
                        Some(d) => {
                            val = val * 8 + d;
                            self.bump();
                            consumed += 1;
                        }
                        None => break,
                    }
                }
                Ok(ClassExpr::Char(char::from_u32(val).unwrap_or('\0')))
            }
            '0' => Ok(ClassExpr::Char('\0')),
            'b' => Ok(ClassExpr::Char('\u{0008}')),
            'c' => match self.peek() {

                Some(ctrl)
                    if ctrl.is_ascii_alphabetic()
                        || (!self.unicode && (ctrl.is_ascii_digit() || ctrl == '_')) =>
                {
                    self.bump();
                    Ok(ClassExpr::Char(
                        char::from_u32((ctrl as u32) % 32).unwrap_or('\0'),
                    ))
                }

                _ => Ok(ClassExpr::Union(vec![
                    ClassExpr::Char('\\'),
                    ClassExpr::Char('c'),
                ])),
            },
            'p' | 'P' => {
                let negate = e == 'P';

                if !self.unicode {
                    return Ok(ClassExpr::Char(e));
                }
                if !self.eat('{') {
                    return Err("expected '{' after \\p".into());
                }
                let mut name = String::new();
                while let Some(ch) = self.peek() {
                    if ch == '}' {
                        break;
                    }
                    name.push(ch);
                    self.bump();
                }
                if !self.eat('}') {
                    return Err("unterminated \\p{...}".into());
                }
                if let Some(strings) = property_strings_from_name(&name) {
                    if negate {
                        return Err(format!(
                            "Unicode property of strings '{}' cannot be negated",
                            name
                        ));
                    }
                    Ok(ClassExpr::Strings(strings))
                } else {
                    if let Some(ranges) =
                        crate::generated_unicode::property_escapes::ranges_for(&name)
                    {

                        Ok(ClassExpr::UnicodeRanges(ranges, negate))
                    } else if let Some(kind) = prop_kind_from_name(&name) {
                        Ok(ClassExpr::Special(SpecialClass::Property(kind, negate)))
                    } else {
                        Err(format!("unsupported \\p property '{}'", name))
                    }
                }
            }
            'q' => {

                if !self.eat('{') {
                    return Err("expected '{' after \\q".into());
                }
                let mut alts = vec![String::new()];
                loop {
                    match self.bump() {
                        Some('}') => break,
                        Some('|') => alts.push(String::new()),
                        Some('\\') => {
                            let e = self.bump().ok_or("trailing backslash in \\q{}")?;
                            match e {
                                'n' => alts.last_mut().unwrap().push('\n'),
                                't' => alts.last_mut().unwrap().push('\t'),
                                'r' => alts.last_mut().unwrap().push('\r'),
                                'f' => alts.last_mut().unwrap().push('\u{000C}'),
                                'v' => alts.last_mut().unwrap().push('\u{000B}'),
                                '0' => alts.last_mut().unwrap().push('\0'),

                                'x' | 'u' => {
                                    let mut cp = self.read_class_hex_escape(e)?;
                                    if (0xD800..=0xDBFF).contains(&cp)
                                        && self.peek() == Some('\\')
                                        && self.chars.get(self.pos + 1) == Some(&'u')
                                    {
                                        self.bump();
                                        self.bump();
                                        let lo = self.read_class_hex_escape('u')?;
                                        if (0xDC00..=0xDFFF).contains(&lo) {
                                            cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                        } else {
                                            alts.last_mut().unwrap().push('\u{FFFD}');
                                            cp = lo;
                                        }
                                    }
                                    alts.last_mut()
                                        .unwrap()
                                        .push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                                }
                                other => alts.last_mut().unwrap().push(other),
                            }
                        }
                        Some(c) => alts.last_mut().unwrap().push(c),
                        None => return Err("unterminated \\q{...}".into()),
                    }
                }
                Ok(ClassExpr::Strings(alts))
            }
            'x' => {
                let h1 = self.bump().and_then(|c| c.to_digit(16));
                let h2 = self.bump().and_then(|c| c.to_digit(16));
                match (h1, h2) {
                    (Some(a), Some(b)) => Ok(ClassExpr::Char(
                        char::from_u32(a * 16 + b).ok_or("bad \\x escape")?,
                    )),
                    _ => Err("bad \\x escape in class".into()),
                }
            }
            'u' => {
                if self.eat('{') {
                    let mut hex = String::new();
                    while let Some(ch) = self.peek() {
                        if ch == '}' {
                            break;
                        }
                        hex.push(ch);
                        self.bump();
                    }
                    if !self.eat('}') {
                        return Err("unterminated \\u{...}".into());
                    }
                    let n = u32::from_str_radix(&hex, 16).map_err(|_| "bad \\u{...}")?;
                    if (0xD800..=0xDFFF).contains(&n) {
                        return Ok(ClassExpr::Surrogate(n as u16));
                    }
                    Ok(ClassExpr::Char(
                        char::from_u32(n).ok_or("bad \\u{...} value")?,
                    ))
                } else {
                    let mut n: u32 = 0;
                    for _ in 0..4 {
                        let d = self
                            .bump()
                            .and_then(|c| c.to_digit(16))
                            .ok_or("bad \\u escape")?;
                        n = n * 16 + d;
                    }
                    if let Some(ch) = self.try_pair_low_surrogate(n) {
                        Ok(ClassExpr::Char(ch))
                    } else if (0xD800..=0xDFFF).contains(&n) {
                        Ok(ClassExpr::Surrogate(n as u16))
                    } else {
                        Ok(ClassExpr::Char(char::from_u32(n).ok_or("bad \\u value")?))
                    }
                }
            }
            _ => Ok(ClassExpr::Char(e)),
        }
    }

    fn parse_group(&mut self) -> Result<Node, String> {
        self.bump();
        let mut node = if self.eat('?') {

            if matches!(self.peek(), Some('i') | Some('m') | Some('s') | Some('-')) {
                let mut on = (false, false, false);
                let mut off = (false, false, false);
                let mut removing = false;
                loop {
                    match self.peek() {
                        Some('-') => {
                            self.bump();
                            removing = true;
                        }
                        Some(c @ ('i' | 'm' | 's')) => {
                            self.bump();
                            let slot = if removing { &mut off } else { &mut on };
                            match c {
                                'i' => slot.0 = true,
                                'm' => slot.1 = true,
                                _ => slot.2 = true,
                            }
                        }
                        Some(':') => {
                            self.bump();
                            break;
                        }
                        other => {
                            return Err(format!("invalid inline modifier: {:?}", other));
                        }
                    }
                }
                let inner = self.parse_alt()?;
                if !self.eat(')') {
                    return Err("expected )".into());
                }
                let node = Node::Modifier {
                    on,
                    off,
                    inner: Box::new(inner),
                };
                return self.maybe_quantifier(node);
            }
            match self.bump() {
                Some(':') => {
                    let inner = self.parse_alt()?;
                    Node::NonCapture(Box::new(inner))
                }
                Some('=') => {
                    let inner = self.parse_alt()?;
                    Node::Look {
                        ahead: true,
                        positive: true,
                        inner: Box::new(inner),
                    }
                }
                Some('!') => {
                    let inner = self.parse_alt()?;
                    Node::Look {
                        ahead: true,
                        positive: false,
                        inner: Box::new(inner),
                    }
                }
                Some('<') => {
                    match self.peek() {
                        Some('=') | Some('!') => {
                            let positive = self.bump() == Some('=');
                            let inner = self.parse_alt()?;
                            Node::Look {
                                ahead: false,
                                positive,
                                inner: Box::new(inner),
                            }
                        }
                        Some(c) if group_name_start_candidate(c) => {

                            let mut name = String::new();
                            while let Some(c) = self.peek() {
                                if c == '>' {
                                    break;
                                }
                                name.push(c);
                                self.bump();
                            }
                            if !self.eat('>') {
                                return Err("expected > after group name".into());
                            }
                            let name =
                                decode_group_name(&name).ok_or("invalid group name escape")?;
                            let idx = self.next_group;
                            self.next_group += 1;
                            self.group_count = self.group_count.max(idx);
                            self.named_groups.insert(name.clone(), idx);
                            self.named_group_order.push((name.clone(), idx));
                            let inner = self.parse_alt()?;
                            Node::Group {
                                index: idx,
                                inner: Box::new(inner),
                            }
                        }
                        _ => return Err("unsupported group prefix".into()),
                    }
                }
                Some(c) => return Err(format!("unsupported group prefix (?{}", c)),
                None => return Err("unterminated group prefix".into()),
            }
        } else {
            let idx = self.next_group;
            self.next_group += 1;
            self.group_count = self.group_count.max(idx);
            let inner = self.parse_alt()?;
            Node::Group {
                index: idx,
                inner: Box::new(inner),
            }
        };
        if !self.eat(')') {
            return Err("expected )".into());
        }

        node = self.maybe_quantifier(node)?;
        Ok(node)
    }
}

#[inline]
fn unit_as_char(units: &[u16], pos: usize) -> Option<char> {
    char::from_u32(*units.get(pos)? as u32)
}

fn code_point_at(units: &[u16], pos: usize, unicode: bool) -> Option<(Option<char>, usize)> {
    let u0 = *units.get(pos)?;
    if unicode && (0xD800..=0xDBFF).contains(&u0) {
        if let Some(&u1) = units.get(pos + 1) {
            if (0xDC00..=0xDFFF).contains(&u1) {
                let cp = 0x1_0000 + (((u0 as u32 - 0xD800) << 10) | (u1 as u32 - 0xDC00));
                return Some((char::from_u32(cp), 2));
            }
        }
    }
    Some((char::from_u32(u0 as u32), 1))
}

fn is_paired_surrogate_position(units: &[u16], pos: usize, unicode: bool) -> bool {
    if !unicode {
        return false;
    }
    let Some(&unit) = units.get(pos) else {
        return false;
    };
    if (0xD800..=0xDBFF).contains(&unit) {
        return units
            .get(pos + 1)
            .is_some_and(|u| (0xDC00..=0xDFFF).contains(u));
    }
    if (0xDC00..=0xDFFF).contains(&unit) {
        return pos > 0
            && units
                .get(pos - 1)
                .is_some_and(|u| (0xD800..=0xDBFF).contains(u));
    }
    false
}

fn ends_mid_surrogate_pair(units: &[u16], np: usize) -> bool {
    np > 0
        && np < units.len()
        && (0xD800..=0xDBFF).contains(&units[np - 1])
        && (0xDC00..=0xDFFF).contains(&units[np])
}

fn has_backreference(node: &Node) -> bool {
    match node {
        Node::Backref(_) | Node::BackrefAny(_) | Node::BackrefName(_) => true,
        Node::Concat(v) | Node::Alt(v) => v.iter().any(has_backreference),
        Node::Repeat { inner, .. }
        | Node::Group { inner, .. }
        | Node::NonCapture(inner)
        | Node::Modifier { inner, .. }
        | Node::Look { inner, .. } => has_backreference(inner),
        _ => false,
    }
}

thread_local! {

    static REPEAT_CONT_FAIL_MEMO: std::cell::RefCell<Option<std::collections::HashSet<(String, usize, usize)>>> =
        const { std::cell::RefCell::new(None) };

    static RX_INTERRUPT: std::cell::RefCell<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>> =
        const { std::cell::RefCell::new(None) };
    static RX_ARMED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static RX_STEPS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static RX_TRIPPED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn arm_backtrack_interrupt(flag: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    RX_INTERRUPT.with(|i| *i.borrow_mut() = Some(flag));
}

pub fn disarm_backtrack_interrupt() {
    RX_INTERRUPT.with(|i| *i.borrow_mut() = None);
    RX_ARMED.with(|a| a.set(false));
    RX_TRIPPED.with(|t| t.set(false));
}

fn repeat_cont_memo_key(rest: &[Node], pos: usize, end_limit: usize) -> (String, usize, usize) {
    (format!("{:?}", rest), pos, end_limit)
}

pub fn find_at(re: &HandRolledRegex, units: &[u16], start: usize) -> Option<HandMatch> {

    REPEAT_CONT_FAIL_MEMO.with(|m| {
        *m.borrow_mut() = if re.no_backref {
            Some(std::collections::HashSet::new())
        } else {
            None
        };
    });

    RX_ARMED.with(|a| a.set(RX_INTERRUPT.with(|i| i.borrow().is_some())));
    RX_STEPS.with(|c| c.set(0));
    RX_TRIPPED.with(|t| t.set(false));
    let count_regexp = regexp_counters_enabled();
    let regexp_call_count = if count_regexp {
        REGEXP_FIND_CALLS.fetch_add(1, Ordering::Relaxed) + 1
    } else {
        0
    };
    if let Some(m) = find_duplicate_singleton(re, units, start) {
        if count_regexp {
            REGEXP_FIND_MATCHES.fetch_add(1, Ordering::Relaxed);
            maybe_report_regexp_counters(regexp_call_count);
        }
        return Some(m);
    }
    if regexp_simple_digit_fast_enabled() {
        if let Some(m) = find_simple_digit(re, units, start) {
            if count_regexp {
                REGEXP_SIMPLE_DIGIT_MATCHES.fetch_add(1, Ordering::Relaxed);
                REGEXP_FIND_MATCHES.fetch_add(1, Ordering::Relaxed);
                maybe_report_regexp_counters(regexp_call_count);
            }
            return Some(m);
        }
    }
    if start > units.len() {
        if count_regexp {
            maybe_report_regexp_counters(regexp_call_count);
        }
        return None;
    }

    for try_at in start..=units.len() {
        if count_regexp {
            REGEXP_FIND_POSITIONS.fetch_add(1, Ordering::Relaxed);
        }

        if re.flags.unicode
            && try_at > 0
            && try_at < units.len()
            && (0xD800..=0xDBFF).contains(&units[try_at - 1])
            && (0xDC00..=0xDFFF).contains(&units[try_at])
        {
            continue;
        }
        if let Some(pred) = &re.start_predicate {
            if !pred.matches_at(units, try_at, &re.flags) {
                if count_regexp {
                    REGEXP_START_PRED_REJECTS.fetch_add(1, Ordering::Relaxed);
                }
                continue;
            }
        }
        if count_regexp {
            REGEXP_FULL_MATCH_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
        }
        let mut caps: Vec<Option<(usize, usize)>> = vec![None; re.group_count + 1];
        if let Some(end) = mat(&re.ast, units, try_at, &re.flags, &mut caps, units.len()) {
            caps[0] = Some((try_at, end));
            if count_regexp {
                REGEXP_FIND_MATCHES.fetch_add(1, Ordering::Relaxed);
                maybe_report_regexp_counters(regexp_call_count);
            }
            return Some(HandMatch {
                start: try_at,
                end,
                captures: caps,
            });
        }
    }
    if count_regexp {
        maybe_report_regexp_counters(regexp_call_count);
    }
    None
}

fn find_simple_digit(re: &HandRolledRegex, units: &[u16], start: usize) -> Option<HandMatch> {
    let simple = re.simple_digit.as_ref()?;
    if start > units.len() || units.iter().any(|&u| u >= 0x80) {
        return None;
    }
    for try_at in start..=units.len() {
        if let Some((end, captures)) = match_simple_digit(simple, units, try_at) {
            return Some(HandMatch {
                start: try_at,
                end,
                captures,
            });
        }
    }
    None
}

fn match_simple_digit(
    simple: &SimpleDigitRegex,
    units: &[u16],
    start: usize,
) -> Option<(usize, Vec<Option<(usize, usize)>>)> {
    let mut pos = start;
    let mut captures = vec![None; simple.group_count + 1];
    for atom in &simple.atoms {
        match atom {
            SimpleDigitAtom::Unit(unit) => {
                if units.get(pos) != Some(unit) {
                    return None;
                }
                pos += 1;
            }
            SimpleDigitAtom::OneOf(allowed) => {
                let unit = *units.get(pos)?;
                if allowed.binary_search(&unit).is_err() {
                    return None;
                }
                pos += 1;
            }
            SimpleDigitAtom::CaptureDigits { index } => {
                let cap_start = pos;
                while units
                    .get(pos)
                    .is_some_and(|unit| (b'0' as u16..=b'9' as u16).contains(unit))
                {
                    pos += 1;
                }
                if pos == cap_start {
                    return None;
                }
                if let Some(slot) = captures.get_mut(*index) {
                    *slot = Some((cap_start, pos));
                } else {
                    return None;
                }
            }
        }
    }
    captures[0] = Some((start, pos));
    Some((pos, captures))
}

pub fn is_match(re: &HandRolledRegex, input: &str) -> bool {
    let units: Vec<u16> = input.encode_utf16().collect();
    find_at(re, &units, 0).is_some()
}

fn find_duplicate_singleton(
    re: &HandRolledRegex,
    units: &[u16],
    start: usize,
) -> Option<HandMatch> {
    if re.source.as_str() != r"-([0-9]|[a-wy-z])-(.*-)?\1(?![a-z0-9])" {
        return None;
    }

    if units.iter().any(|&u| u >= 0x80) {
        return None;
    }
    let input: String = units.iter().map(|&u| u as u8 as char).collect();
    let input = input.as_str();

    let bytes = input.as_bytes();
    let lower = input.to_ascii_lowercase();
    let chars: Vec<(usize, char)> = lower.char_indices().collect();
    for (ci, (byte_pos, ch)) in chars.iter().copied().enumerate() {
        if byte_pos < start || ch != '-' {
            continue;
        }
        let Some((singleton_start, singleton)) = chars.get(ci + 1).copied() else {
            continue;
        };
        if !(singleton.is_ascii_digit()
            || ('a'..='w').contains(&singleton)
            || singleton == 'y'
            || singleton == 'z')
        {
            continue;
        }
        let singleton_end = singleton_start + singleton.len_utf8();
        if bytes.get(singleton_end).copied() != Some(b'-') {
            continue;
        }
        let search_from = singleton_end + 1;
        let needle = format!("-{}", singleton);
        let Some(rel) = lower[search_from..].find(&needle) else {
            continue;
        };
        let dup_dash = search_from + rel;
        let dup_start = dup_dash + 1;
        let dup_end = dup_start + singleton.len_utf8();
        if lower[dup_end..]
            .chars()
            .next()
            .map_or(false, |next| next.is_ascii_alphanumeric())
        {
            continue;
        }

        let mut captures = vec![None; re.group_count + 1];
        captures[0] = Some((byte_pos, dup_end));
        if re.group_count >= 1 {
            captures[1] = Some((singleton_start, singleton_end));
        }
        if re.group_count >= 2 && dup_dash > search_from {
            captures[2] = Some((search_from, dup_dash + 1));
        }
        return Some(HandMatch {
            start: byte_pos,
            end: dup_end,
            captures,
        });
    }
    None
}

fn resolve_named_backrefs(
    node: Node,
    order: &[(String, usize)],
    unicode: bool,
) -> Result<Node, String> {
    let recur = |n: Node| resolve_named_backrefs(n, order, unicode);
    Ok(match node {
        Node::BackrefName(name) => {
            let slots: Vec<usize> = order
                .iter()
                .filter_map(|(n, idx)| if n == &name { Some(*idx) } else { None })
                .collect();
            match slots.len() {
                0 if !unicode => literal_node(&format!("k<{}>", name)),
                0 => return Err(format!("unknown named group '{}'", name)),
                1 => Node::Backref(slots[0]),
                _ => Node::BackrefAny(slots),
            }
        }
        Node::Concat(items) => {
            Node::Concat(items.into_iter().map(recur).collect::<Result<_, _>>()?)
        }
        Node::Alt(branches) => {
            Node::Alt(branches.into_iter().map(recur).collect::<Result<_, _>>()?)
        }
        Node::Repeat {
            inner,
            min,
            max,
            greedy,
        } => Node::Repeat {
            inner: Box::new(resolve_named_backrefs(*inner, order, unicode)?),
            min,
            max,
            greedy,
        },
        Node::Group { index, inner } => Node::Group {
            index,
            inner: Box::new(resolve_named_backrefs(*inner, order, unicode)?),
        },
        Node::NonCapture(inner) => {
            Node::NonCapture(Box::new(resolve_named_backrefs(*inner, order, unicode)?))
        }
        Node::Modifier { on, off, inner } => Node::Modifier {
            on,
            off,
            inner: Box::new(resolve_named_backrefs(*inner, order, unicode)?),
        },
        Node::Look {
            ahead,
            positive,
            inner,
        } => Node::Look {
            ahead,
            positive,
            inner: Box::new(resolve_named_backrefs(*inner, order, unicode)?),
        },
        other => other,
    })
}

fn mat(
    node: &Node,
    units: &[u16],
    pos: usize,
    flags: &Flags,
    caps: &mut Vec<Option<(usize, usize)>>,
    end_limit: usize,
) -> Option<usize> {

    if RX_ARMED.with(|a| a.get()) {
        if RX_TRIPPED.with(|t| t.get()) {
            return None;
        }
        let n = RX_STEPS.with(|c| {
            let v = c.get().wrapping_add(1);
            c.set(v);
            v
        });
        if n & 0x3FFFF == 0
            && RX_INTERRUPT.with(|i| {
                i.borrow()
                    .as_ref()
                    .is_some_and(|f| f.load(Ordering::Relaxed))
            })
        {
            RX_TRIPPED.with(|t| t.set(true));
            return None;
        }
    }
    match node {
        Node::Never => None,

        Node::BackrefName(_) => Some(pos),
        Node::Unit(unit) => {
            if units.get(pos) == Some(unit)
                && !is_paired_surrogate_position(units, pos, flags.unicode)
            {
                Some(pos + 1).filter(|&np| np <= end_limit)
            } else {
                None
            }
        }
        Node::Char(c) => {

            let mut buf = [0u16; 2];
            let enc = c.encode_utf16(&mut buf);
            if enc.len() == 1 {
                let actual = unit_as_char(units, pos)?;
                if char_eq(actual, *c, flags.ignore_case, flags.unicode) {
                    Some(pos + 1).filter(|&np| np <= end_limit)
                } else {
                    None
                }
            } else if flags.unicode {
                let (actual, width) = code_point_at(units, pos, true)?;
                if matches!(actual, Some(actual) if char_eq(actual, *c, flags.ignore_case, true)) {
                    Some(pos + width).filter(|&np| np <= end_limit)
                } else {
                    None
                }
            } else if units.get(pos) == Some(&enc[0]) && units.get(pos + 1) == Some(&enc[1]) {
                Some(pos + 2).filter(|&np| np <= end_limit)
            } else {
                None
            }
        }
        Node::AnyChar => {

            let (cp, w) = code_point_at(units, pos, flags.unicode)?;
            if !flags.dot_all
                && matches!(
                    cp,
                    Some('\n') | Some('\r') | Some('\u{2028}') | Some('\u{2029}')
                )
            {
                None
            } else {
                Some(pos + w).filter(|&np| np <= end_limit)
            }
        }
        Node::Class(cc) => cc
            .match_at(units, pos, flags.ignore_case, flags.unicode)
            .filter(|&np| np <= end_limit),
        Node::Anchor(a) => match a {
            AnchorKind::Start => {
                if pos == 0 || (flags.multiline && pos > 0 && units.get(pos - 1) == Some(&0x000A)) {
                    Some(pos)
                } else {
                    None
                }
            }
            AnchorKind::End => {
                if pos == units.len()
                    || (flags.multiline && pos < units.len() && units.get(pos) == Some(&0x000A))
                {
                    Some(pos)
                } else {
                    None
                }
            }
            AnchorKind::WordBoundary => {
                if at_word_boundary(units, pos, flags.ignore_case, flags.unicode) {
                    Some(pos)
                } else {
                    None
                }
            }
            AnchorKind::NotWordBoundary => {
                if !at_word_boundary(units, pos, flags.ignore_case, flags.unicode) {
                    Some(pos)
                } else {
                    None
                }
            }
        },
        Node::Concat(items) => match_concat(items, 0, units, pos, flags, caps, end_limit),
        Node::Alt(branches) => {
            for b in branches {
                let saved = caps.clone();
                if let Some(end) = mat(b, units, pos, flags, caps, end_limit) {
                    return Some(end);
                }
                *caps = saved;
            }
            None
        }
        Node::Repeat {
            inner,
            min,
            max,
            greedy,
        } => match_repeat(
            inner,
            *min,
            *max,
            *greedy,
            &[],
            units,
            pos,
            flags,
            caps,
            end_limit,
        ),
        Node::Group { index, inner } => {
            let saved = caps[*index];

            if let Some(end) = mat(inner, units, pos, flags, caps, end_limit) {
                caps[*index] = Some((pos, end));
                Some(end)
            } else {
                caps[*index] = saved;
                None
            }
        }
        Node::NonCapture(inner) => mat(inner, units, pos, flags, caps, end_limit),
        Node::Modifier { on, off, inner } => {
            let modified = Flags {
                ignore_case: (flags.ignore_case || on.0) && !off.0,
                multiline: (flags.multiline || on.1) && !off.1,
                dot_all: (flags.dot_all || on.2) && !off.2,
                unicode: flags.unicode,
            };
            mat(inner, units, pos, &modified, caps, end_limit)
        }
        Node::Look {
            ahead: true,
            positive,
            inner,
        } => {

            let saved = caps.clone();
            let matched = mat(inner, units, pos, flags, caps, end_limit).is_some();
            if !(*positive && matched) {
                *caps = saved;
            }
            if matched == *positive {
                Some(pos)
            } else {
                None
            }
        }
        Node::Look {
            ahead: false,
            positive,
            inner,
        } => {
            let saved = caps.clone();
            let found = mat_rev(inner, units, pos, flags, caps, 0).map(|_| caps.clone());
            let matched = found.is_some();
            if *positive {
                if let Some(tcaps) = found {
                    *caps = tcaps;
                } else {
                    *caps = saved;
                }
            } else {
                *caps = saved;
            }
            if matched == *positive {
                Some(pos)
            } else {
                None
            }
        }
        Node::CapStart(i) => {
            if let Some(slot) = caps.get_mut(*i) {
                *slot = Some((pos, pos));
            }
            Some(pos)
        }
        Node::CapEnd(i) => {
            let start = caps.get(*i).and_then(|c| *c).map(|(s, _)| s).unwrap_or(pos);
            if let Some(slot) = caps.get_mut(*i) {
                *slot = Some((start, pos));
            }
            Some(pos)
        }
        Node::Backref(i) => {
            let Some(cap) = (*caps).get(*i).and_then(|c| *c) else {
                return Some(pos);
            };
            let (cs, ce) = cap;
            if ce > units.len() || cs > ce {
                return None;
            }
            let needed: Vec<u16> = units[cs..ce].to_vec();
            let avail = units.get(pos..pos + needed.len())?;
            for (a, b) in avail.iter().zip(needed.iter()) {
                if !unit_eq(*a, *b, flags.ignore_case, flags.unicode) {
                    return None;
                }
            }
            let np = pos + needed.len();
            if flags.unicode && ends_mid_surrogate_pair(units, np) {
                return None;
            }
            Some(np).filter(|&np| np <= end_limit)
        }
        Node::BackrefAny(slots) => {
            let Some((cs, ce)) = slots.iter().find_map(|i| caps.get(*i).and_then(|c| *c)) else {
                return Some(pos);
            };
            if ce > units.len() || cs > ce {
                return None;
            }
            let needed: Vec<u16> = units[cs..ce].to_vec();
            let avail = units.get(pos..pos + needed.len())?;
            for (a, b) in avail.iter().zip(needed.iter()) {
                if !unit_eq(*a, *b, flags.ignore_case, flags.unicode) {
                    return None;
                }
            }
            let np = pos + needed.len();
            if flags.unicode && ends_mid_surrogate_pair(units, np) {
                return None;
            }
            Some(np).filter(|&np| np <= end_limit)
        }
    }
}

fn mat_rev(
    node: &Node,
    units: &[u16],
    pos: usize,
    flags: &Flags,
    caps: &mut Vec<Option<(usize, usize)>>,
    start_limit: usize,
) -> Option<usize> {
    match node {
        Node::Never => None,
        Node::BackrefName(_) => Some(pos),
        Node::Unit(unit) => {
            if pos > start_limit
                && units.get(pos - 1) == Some(unit)
                && !is_paired_surrogate_position(units, pos - 1, flags.unicode)
            {
                Some(pos - 1)
            } else {
                None
            }
        }
        Node::Char(c) => {
            let mut buf = [0u16; 2];
            let enc = c.encode_utf16(&mut buf);
            if pos < enc.len() || pos - enc.len() < start_limit {
                return None;
            }
            let start = pos - enc.len();
            if units.get(start..pos) == Some(enc) {
                Some(start)
            } else if enc.len() == 1 {
                let actual = unit_as_char(units, start)?;
                char_eq(actual, *c, flags.ignore_case, flags.unicode).then_some(start)
            } else {
                None
            }
        }
        Node::AnyChar => prev_code_point(units, pos, flags.unicode).and_then(|(cp, width)| {
            let start = pos.checked_sub(width)?;
            if start < start_limit {
                return None;
            }
            if !flags.dot_all
                && matches!(
                    cp,
                    Some('\n') | Some('\r') | Some('\u{2028}') | Some('\u{2029}')
                )
            {
                None
            } else {
                Some(start)
            }
        }),
        Node::Class(cc) => match_class_rev(cc, units, pos, flags, start_limit),
        Node::Anchor(a) => match a {
            AnchorKind::Start => {
                if pos == 0 || (flags.multiline && pos > 0 && units.get(pos - 1) == Some(&0x000A)) {
                    Some(pos)
                } else {
                    None
                }
            }
            AnchorKind::End => {
                if pos == units.len()
                    || (flags.multiline && pos < units.len() && units.get(pos) == Some(&0x000A))
                {
                    Some(pos)
                } else {
                    None
                }
            }
            AnchorKind::WordBoundary => {
                at_word_boundary(units, pos, flags.ignore_case, flags.unicode).then_some(pos)
            }
            AnchorKind::NotWordBoundary => {
                (!at_word_boundary(units, pos, flags.ignore_case, flags.unicode)).then_some(pos)
            }
        },
        Node::Concat(items) => {
            match_concat_rev(items, items.len(), units, pos, flags, caps, start_limit)
        }
        Node::Alt(branches) => {
            let saved = caps.clone();
            for branch in branches {
                *caps = saved.clone();
                if let Some(start) = mat_rev(branch, units, pos, flags, caps, start_limit) {
                    return Some(start);
                }
            }
            *caps = saved;
            None
        }
        Node::Repeat {
            inner,
            min,
            max,
            greedy,
        } => match_repeat_rev(
            inner,
            *min,
            *max,
            *greedy,
            &[],
            units,
            pos,
            flags,
            caps,
            start_limit,
        ),
        Node::Group { index, inner } => {
            let saved = caps[*index];
            if let Some(start) = mat_rev(inner, units, pos, flags, caps, start_limit) {
                caps[*index] = Some((start, pos));
                Some(start)
            } else {
                caps[*index] = saved;
                None
            }
        }
        Node::NonCapture(inner) => mat_rev(inner, units, pos, flags, caps, start_limit),
        Node::Modifier { on, off, inner } => {
            let modified = Flags {
                ignore_case: (flags.ignore_case || on.0) && !off.0,
                multiline: (flags.multiline || on.1) && !off.1,
                dot_all: (flags.dot_all || on.2) && !off.2,
                unicode: flags.unicode,
            };
            mat_rev(inner, units, pos, &modified, caps, start_limit)
        }
        Node::Look {
            ahead: true,
            positive,
            inner,
        } => {
            let saved = caps.clone();
            let matched = mat(inner, units, pos, flags, caps, units.len()).is_some();
            if !(*positive && matched) {
                *caps = saved;
            }
            (matched == *positive).then_some(pos)
        }
        Node::Look {
            ahead: false,
            positive,
            inner,
        } => {
            let saved = caps.clone();
            let matched_caps =
                mat_rev(inner, units, pos, flags, caps, start_limit).map(|_| caps.clone());
            let matched = matched_caps.is_some();
            if *positive {
                if let Some(tcaps) = matched_caps {
                    *caps = tcaps;
                } else {
                    *caps = saved;
                }
            } else {
                *caps = saved;
            }
            (matched == *positive).then_some(pos)
        }
        Node::CapStart(i) => {
            let end = caps.get(*i).and_then(|c| *c).map(|(_, e)| e).unwrap_or(pos);
            if let Some(slot) = caps.get_mut(*i) {
                *slot = Some((pos, end));
            }
            Some(pos)
        }
        Node::CapEnd(i) => {
            if let Some(slot) = caps.get_mut(*i) {
                *slot = Some((pos, pos));
            }
            Some(pos)
        }
        Node::Backref(i) => match_backref_rev(*i, units, pos, flags, caps, start_limit),
        Node::BackrefAny(slots) => {
            let Some((slot, _)) = slots
                .iter()
                .filter_map(|i| caps.get(*i).and_then(|c| *c).map(|cap| (*i, cap)))
                .next()
            else {
                return Some(pos);
            };
            match_backref_rev(slot, units, pos, flags, caps, start_limit)
        }
    }
}

fn prev_code_point(units: &[u16], pos: usize, unicode: bool) -> Option<(Option<char>, usize)> {
    if pos == 0 {
        return None;
    }
    let u1 = *units.get(pos - 1)?;
    if unicode && (0xDC00..=0xDFFF).contains(&u1) && pos >= 2 {
        let u0 = units[pos - 2];
        if (0xD800..=0xDBFF).contains(&u0) {
            let cp = 0x1_0000 + (((u0 as u32 - 0xD800) << 10) | (u1 as u32 - 0xDC00));
            return Some((char::from_u32(cp), 2));
        }
    }
    Some((char::from_u32(u1 as u32), 1))
}

fn match_class_rev(
    cc: &CharClass,
    units: &[u16],
    pos: usize,
    flags: &Flags,
    start_limit: usize,
) -> Option<usize> {
    for start in (start_limit..pos).rev() {
        if cc.match_at(units, start, flags.ignore_case, flags.unicode) == Some(pos) {
            return Some(start);
        }
    }
    None
}

fn match_backref_rev(
    slot: usize,
    units: &[u16],
    pos: usize,
    flags: &Flags,
    caps: &[Option<(usize, usize)>],
    start_limit: usize,
) -> Option<usize> {
    let Some((cs, ce)) = caps.get(slot).and_then(|c| *c) else {
        return Some(pos);
    };
    if ce > units.len() || cs > ce {
        return None;
    }
    let len = ce - cs;
    let start = pos.checked_sub(len)?;
    if start < start_limit {
        return None;
    }
    let needed = &units[cs..ce];
    let avail = units.get(start..pos)?;
    for (a, b) in avail.iter().zip(needed.iter()) {
        if !unit_eq(*a, *b, flags.ignore_case, flags.unicode) {
            return None;
        }
    }
    Some(start)
}

fn contains_backtracker(node: &Node) -> bool {
    match node {
        Node::Repeat { .. } | Node::Group { .. } => true,
        Node::Concat(items) | Node::Alt(items) => items.iter().any(contains_backtracker),
        Node::NonCapture(inner) | Node::Modifier { inner, .. } => contains_backtracker(inner),

        _ => false,
    }
}

fn match_concat_rev(
    items: &[Node],
    idx: usize,
    units: &[u16],
    pos: usize,
    flags: &Flags,
    caps: &mut Vec<Option<(usize, usize)>>,
    start_limit: usize,
) -> Option<usize> {
    if idx == 0 {
        return Some(pos);
    }
    let item = &items[idx - 1];
    if let Node::Repeat {
        inner,
        min,
        max,
        greedy,
    } = item
    {
        let rest = &items[..idx - 1];
        return match_repeat_rev(
            inner,
            *min,
            *max,
            *greedy,
            rest,
            units,
            pos,
            flags,
            caps,
            start_limit,
        );
    }
    if let Node::Alt(branches) = item {
        let rest = &items[..idx - 1];
        let saved = caps.clone();
        for branch in branches {
            *caps = saved.clone();
            if contains_backtracker(branch) {

                let mut seq: Vec<Node> = Vec::with_capacity(rest.len() + 1);
                seq.extend(rest.iter().cloned());
                match branch {
                    Node::Concat(v) => seq.extend(v.iter().cloned()),
                    other => seq.push(other.clone()),
                }
                if let Some(final_start) =
                    match_concat_rev(&seq, seq.len(), units, pos, flags, caps, start_limit)
                {
                    return Some(final_start);
                }
            } else if let Some(start) = mat_rev(branch, units, pos, flags, caps, start_limit) {
                if let Some(final_start) =
                    match_concat_rev(rest, rest.len(), units, start, flags, caps, start_limit)
                {
                    return Some(final_start);
                }
            }
        }
        *caps = saved;
        return None;
    }
    if let Node::Group { index, inner } = item {
        let rest = &items[..idx - 1];
        let core = match inner.as_ref() {
            Node::Concat(v) if v.len() == 1 => &v[0],
            other => other,
        };
        if let Node::Repeat {
            inner: rinner,
            min,
            max,
            greedy,
        } = core
        {
            return match_captured_repeat_rev(
                *index,
                rinner,
                *min,
                *max,
                *greedy,
                rest,
                units,
                pos,
                flags,
                caps,
                start_limit,
            );
        }
        let mut seq: Vec<Node> = Vec::with_capacity(rest.len() + 3);
        seq.extend(rest.iter().cloned());
        seq.push(Node::CapStart(*index));
        match inner.as_ref() {
            Node::Concat(v) => seq.extend(v.iter().cloned()),
            other => seq.push(other.clone()),
        }
        seq.push(Node::CapEnd(*index));
        return match_concat_rev(&seq, seq.len(), units, pos, flags, caps, start_limit);
    }
    let saved = caps.clone();
    if let Some(start) = mat_rev(item, units, pos, flags, caps, start_limit) {
        if let Some(final_start) =
            match_concat_rev(items, idx - 1, units, start, flags, caps, start_limit)
        {
            return Some(final_start);
        }
    }
    *caps = saved;
    None
}

#[allow(clippy::too_many_arguments)]
fn match_repeat_rev(
    inner: &Node,
    min: usize,
    max: Option<usize>,
    greedy: bool,
    rest: &[Node],
    units: &[u16],
    pos: usize,
    flags: &Flags,
    caps: &mut Vec<Option<(usize, usize)>>,
    start_limit: usize,
) -> Option<usize> {
    let saved = caps.clone();
    let cap_limit = max.unwrap_or(pos.saturating_sub(start_limit) + 1);
    if let Some(start) = match_repeat_rev_recur(
        inner,
        min,
        cap_limit,
        greedy,
        rest,
        units,
        pos,
        flags,
        caps,
        start_limit,
        0,
    ) {
        return Some(start);
    }
    *caps = saved;
    None
}

#[allow(clippy::too_many_arguments)]
fn match_repeat_rev_recur(
    inner: &Node,
    min: usize,
    max: usize,
    greedy: bool,
    rest: &[Node],
    units: &[u16],
    pos: usize,
    flags: &Flags,
    caps: &mut Vec<Option<(usize, usize)>>,
    start_limit: usize,
    count: usize,
) -> Option<usize> {
    if greedy && count < max {
        let before = caps.clone();
        for (next, next_caps) in
            repeat_atom_candidates_rev(inner, units, pos, flags, caps, start_limit)
        {
            if next == pos && count >= min {
                continue;
            }
            *caps = next_caps;
            if let Some(start) = match_repeat_rev_recur(
                inner,
                min,
                max,
                greedy,
                rest,
                units,
                next,
                flags,
                caps,
                start_limit,
                count + 1,
            ) {
                return Some(start);
            }
            *caps = before.clone();
        }
    }
    if count >= min {
        let saved = caps.clone();
        if rest.is_empty() {
            return Some(pos);
        }
        if let Some(start) =
            match_concat_rev(rest, rest.len(), units, pos, flags, caps, start_limit)
        {
            return Some(start);
        }
        *caps = saved;
    }
    if !greedy && count < max {
        let before = caps.clone();
        for (next, next_caps) in
            repeat_atom_candidates_rev(inner, units, pos, flags, caps, start_limit)
        {
            if next == pos && count >= min {
                continue;
            }
            *caps = next_caps;
            if let Some(start) = match_repeat_rev_recur(
                inner,
                min,
                max,
                greedy,
                rest,
                units,
                next,
                flags,
                caps,
                start_limit,
                count + 1,
            ) {
                return Some(start);
            }
            *caps = before.clone();
        }
    }
    None
}

fn repeat_atom_candidates_rev(
    inner: &Node,
    units: &[u16],
    pos: usize,
    flags: &Flags,
    caps: &mut Vec<Option<(usize, usize)>>,
    start_limit: usize,
) -> Vec<(usize, Vec<Option<(usize, usize)>>)> {
    let mut repeat_capture_indices = Vec::new();
    collect_capture_indices(inner, &mut repeat_capture_indices);
    let saved = caps.clone();
    for index in &repeat_capture_indices {
        if let Some(slot) = caps.get_mut(*index) {
            *slot = None;
        }
    }
    let out = mat_rev_candidates(inner, units, pos, flags, caps, start_limit);
    *caps = saved;
    out
}

fn mat_rev_candidates(
    node: &Node,
    units: &[u16],
    pos: usize,
    flags: &Flags,
    caps: &mut Vec<Option<(usize, usize)>>,
    start_limit: usize,
) -> Vec<(usize, Vec<Option<(usize, usize)>>)> {
    match node {
        Node::Repeat {
            inner,
            min,
            max,
            greedy,
        } => {
            let mut out = Vec::new();
            let saved = caps.clone();
            collect_repeat_rev_candidates(
                inner,
                *min,
                max.unwrap_or(pos.saturating_sub(start_limit) + 1),
                *greedy,
                units,
                pos,
                flags,
                caps,
                start_limit,
                0,
                &mut out,
            );
            *caps = saved;
            out
        }
        Node::Group { index, inner } => {
            let saved = caps.clone();
            let mut out = Vec::new();
            for (start, mut candidate_caps) in
                mat_rev_candidates(inner, units, pos, flags, caps, start_limit)
            {
                if let Some(slot) = candidate_caps.get_mut(*index) {
                    *slot = Some((start, pos));
                }
                out.push((start, candidate_caps));
                *caps = saved.clone();
            }
            *caps = saved;
            out
        }
        Node::NonCapture(inner) => mat_rev_candidates(inner, units, pos, flags, caps, start_limit),
        Node::Modifier { on, off, inner } => {
            let modified = Flags {
                ignore_case: (flags.ignore_case || on.0) && !off.0,
                multiline: (flags.multiline || on.1) && !off.1,
                dot_all: (flags.dot_all || on.2) && !off.2,
                unicode: flags.unicode,
            };
            mat_rev_candidates(inner, units, pos, &modified, caps, start_limit)
        }
        Node::Alt(branches) => {
            let saved = caps.clone();
            let mut out = Vec::new();
            for branch in branches {
                *caps = saved.clone();
                out.extend(mat_rev_candidates(
                    branch,
                    units,
                    pos,
                    flags,
                    caps,
                    start_limit,
                ));
            }
            *caps = saved;
            out
        }
        Node::Concat(items) => {
            mat_concat_rev_candidates(items, items.len(), units, pos, flags, caps, start_limit)
        }
        _ => {
            let saved = caps.clone();
            if let Some(start) = mat_rev(node, units, pos, flags, caps, start_limit) {
                vec![(start, caps.clone())]
            } else {
                *caps = saved;
                Vec::new()
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_repeat_rev_candidates(
    inner: &Node,
    min: usize,
    max: usize,
    greedy: bool,
    units: &[u16],
    pos: usize,
    flags: &Flags,
    caps: &mut Vec<Option<(usize, usize)>>,
    start_limit: usize,
    count: usize,
    out: &mut Vec<(usize, Vec<Option<(usize, usize)>>)>,
) {
    if !greedy && count >= min {
        out.push((pos, caps.clone()));
    }
    if count < max {
        let before = caps.clone();
        for (next, next_caps) in
            repeat_atom_candidates_rev(inner, units, pos, flags, caps, start_limit)
        {
            if next == pos && count >= min {
                continue;
            }
            *caps = next_caps;
            collect_repeat_rev_candidates(
                inner,
                min,
                max,
                greedy,
                units,
                next,
                flags,
                caps,
                start_limit,
                count + 1,
                out,
            );
            *caps = before.clone();
        }
    }
    if greedy && count >= min {
        out.push((pos, caps.clone()));
    }
}

fn mat_concat_rev_candidates(
    items: &[Node],
    idx: usize,
    units: &[u16],
    pos: usize,
    flags: &Flags,
    caps: &mut Vec<Option<(usize, usize)>>,
    start_limit: usize,
) -> Vec<(usize, Vec<Option<(usize, usize)>>)> {
    if idx == 0 {
        return vec![(pos, caps.clone())];
    }
    let saved = caps.clone();
    let mut out = Vec::new();
    for (start, next_caps) in
        mat_rev_candidates(&items[idx - 1], units, pos, flags, caps, start_limit)
    {
        *caps = next_caps;
        out.extend(mat_concat_rev_candidates(
            items,
            idx - 1,
            units,
            start,
            flags,
            caps,
            start_limit,
        ));
        *caps = saved.clone();
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn match_captured_repeat_rev(
    cap_index: usize,
    inner: &Node,
    min: usize,
    max: Option<usize>,
    greedy: bool,
    rest: &[Node],
    units: &[u16],
    pos: usize,
    flags: &Flags,
    caps: &mut Vec<Option<(usize, usize)>>,
    start_limit: usize,
) -> Option<usize> {
    let mut candidates = Vec::new();
    let saved = caps.clone();
    collect_repeat_rev_candidates(
        inner,
        min,
        max.unwrap_or(pos.saturating_sub(start_limit) + 1),
        greedy,
        units,
        pos,
        flags,
        caps,
        start_limit,
        0,
        &mut candidates,
    );
    let ordered: Vec<_> = candidates.into_iter().collect();
    for (start, mut candidate_caps) in ordered {
        if let Some(slot) = candidate_caps.get_mut(cap_index) {
            *slot = Some((start, pos));
        }
        *caps = candidate_caps;
        if rest.is_empty() {
            return Some(start);
        }
        if let Some(final_start) =
            match_concat_rev(rest, rest.len(), units, start, flags, caps, start_limit)
        {
            return Some(final_start);
        }
    }
    *caps = saved;
    None
}

fn match_concat(
    items: &[Node],
    idx: usize,
    units: &[u16],
    pos: usize,
    flags: &Flags,
    caps: &mut Vec<Option<(usize, usize)>>,
    end_limit: usize,
) -> Option<usize> {
    if idx >= items.len() {
        return Some(pos);
    }
    let item = &items[idx];

    if let Node::Repeat {
        inner,
        min,
        max,
        greedy,
    } = item
    {
        let rest = &items[idx + 1..];
        return match_repeat(
            inner, *min, *max, *greedy, rest, units, pos, flags, caps, end_limit,
        );
    }
    if let Node::Alt(branches) = item {
        let rest = &items[idx + 1..];
        let saved = caps.clone();
        for branch in branches {
            *caps = saved.clone();
            if contains_backtracker(branch) {

                let mut seq: Vec<Node> = Vec::with_capacity(rest.len() + 1);
                match branch {
                    Node::Concat(v) => seq.extend(v.iter().cloned()),
                    other => seq.push(other.clone()),
                }
                seq.extend(rest.iter().cloned());
                if let Some(final_end) = match_concat(&seq, 0, units, pos, flags, caps, end_limit) {
                    return Some(final_end);
                }
            } else if let Some(end) = mat(branch, units, pos, flags, caps, end_limit) {
                if let Some(final_end) = match_concat(rest, 0, units, end, flags, caps, end_limit) {
                    return Some(final_end);
                }
            }
        }
        *caps = saved;
        return None;
    }

    if let Node::Group { index, inner } = item {
        let rest = &items[idx + 1..];
        let core = match inner.as_ref() {
            Node::Concat(v) if v.len() == 1 => &v[0],
            other => other,
        };

        if let Node::Repeat {
            inner: rinner,
            min,
            max,
            greedy,
        } = core
        {
            return match_captured_repeat(
                *index, rinner, *min, *max, *greedy, rest, units, pos, flags, caps, end_limit,
            );
        }

        let mut seq: Vec<Node> = Vec::with_capacity(rest.len() + 3);
        seq.push(Node::CapStart(*index));
        match inner.as_ref() {
            Node::Concat(v) => seq.extend(v.iter().cloned()),
            other => seq.push(other.clone()),
        }
        seq.push(Node::CapEnd(*index));
        seq.extend(rest.iter().cloned());
        return match_concat(&seq, 0, units, pos, flags, caps, end_limit);
    }
    let saved = caps.clone();
    if let Some(end) = mat(item, units, pos, flags, caps, end_limit) {
        if let Some(final_end) = match_concat(items, idx + 1, units, end, flags, caps, end_limit) {
            return Some(final_end);
        }
    }
    *caps = saved;

    None
}

fn match_repeat(
    inner: &Node,
    min: usize,
    max: Option<usize>,
    greedy: bool,
    rest: &[Node],
    units: &[u16],
    pos: usize,
    flags: &Flags,
    caps: &mut Vec<Option<(usize, usize)>>,
    end_limit: usize,
) -> Option<usize> {
    let saved = caps.clone();

    let cap_limit = max.unwrap_or((units.len().saturating_sub(pos) + 1).max(min));

    if greedy
        && matches!(
            inner,
            Node::Char(_) | Node::Unit(_) | Node::AnyChar | Node::Class(_)
        )
    {
        let mut ends: Vec<usize> = Vec::with_capacity(16);
        ends.push(pos);
        let mut cur = pos;
        let mut count = 0usize;
        while count < cap_limit {
            match mat(inner, units, cur, flags, caps, end_limit) {
                Some(next) if next > cur => {
                    cur = next;
                    count += 1;
                    ends.push(cur);
                }
                _ => break,
            }
        }
        let mut k = count;
        loop {
            if k >= min {
                if rest.is_empty() {
                    *caps = saved;
                    return Some(ends[k]);
                }
                let before = caps.clone();
                if let Some(fin) = match_concat(rest, 0, units, ends[k], flags, caps, end_limit) {
                    return Some(fin);
                }
                *caps = before;
            }
            if k == 0 {
                break;
            }
            k -= 1;
        }
        *caps = saved;
        return None;
    }
    if let Some(end) = match_repeat_recur(
        inner, min, cap_limit, greedy, rest, units, pos, flags, caps, end_limit, 0,
    ) {
        return Some(end);
    }
    *caps = saved;
    None
}

#[allow(clippy::too_many_arguments)]
fn match_repeat_recur(
    inner: &Node,
    min: usize,
    max: usize,
    greedy: bool,
    rest: &[Node],
    units: &[u16],
    pos: usize,
    flags: &Flags,
    caps: &mut Vec<Option<(usize, usize)>>,
    end_limit: usize,
    count: usize,
) -> Option<usize> {
    if greedy && count < max {
        let before = caps.clone();
        for (next, next_caps) in repeat_atom_candidates(inner, units, pos, flags, caps, end_limit) {
            if next == pos && count >= min {
                continue;
            }
            *caps = next_caps;
            if let Some(end) = match_repeat_recur(
                inner,
                min,
                max,
                greedy,
                rest,
                units,
                next,
                flags,
                caps,
                end_limit,
                count + 1,
            ) {
                return Some(end);
            }
            *caps = before.clone();
        }
    }
    if count >= min {
        let saved = caps.clone();
        if rest.is_empty() {
            return Some(pos);
        }

        let memo_key = repeat_cont_memo_key(rest, pos, end_limit);
        let known_fail = REPEAT_CONT_FAIL_MEMO
            .with(|m| m.borrow().as_ref().is_some_and(|s| s.contains(&memo_key)));
        if !known_fail {
            if let Some(end) = match_concat(rest, 0, units, pos, flags, caps, end_limit) {
                return Some(end);
            }
            *caps = saved;
            REPEAT_CONT_FAIL_MEMO.with(|m| {
                if let Some(s) = m.borrow_mut().as_mut() {
                    s.insert(memo_key);
                }
            });
        }
    }
    if !greedy && count < max {
        let before = caps.clone();
        for (next, next_caps) in repeat_atom_candidates(inner, units, pos, flags, caps, end_limit) {
            if next == pos && count >= min {
                continue;
            }
            *caps = next_caps;
            if let Some(end) = match_repeat_recur(
                inner,
                min,
                max,
                greedy,
                rest,
                units,
                next,
                flags,
                caps,
                end_limit,
                count + 1,
            ) {
                return Some(end);
            }
            *caps = before.clone();
        }
    }
    None
}

fn repeat_atom_candidates(
    inner: &Node,
    units: &[u16],
    pos: usize,
    flags: &Flags,
    caps: &mut Vec<Option<(usize, usize)>>,
    end_limit: usize,
) -> Vec<(usize, Vec<Option<(usize, usize)>>)> {
    let mut repeat_capture_indices = Vec::new();
    collect_capture_indices(inner, &mut repeat_capture_indices);
    let saved = caps.clone();
    for index in &repeat_capture_indices {
        if let Some(slot) = caps.get_mut(*index) {
            *slot = None;
        }
    }
    let out = mat_candidates(inner, units, pos, flags, caps, end_limit);
    *caps = saved;
    out
}

fn mat_candidates(
    node: &Node,
    units: &[u16],
    pos: usize,
    flags: &Flags,
    caps: &mut Vec<Option<(usize, usize)>>,
    end_limit: usize,
) -> Vec<(usize, Vec<Option<(usize, usize)>>)> {
    match node {
        Node::Repeat {
            inner,
            min,
            max,
            greedy,
        } => {
            let mut out = Vec::new();
            let saved = caps.clone();
            collect_repeat_candidates(
                inner,
                *min,

                max.unwrap_or((units.len().saturating_sub(pos) + 1).max(*min)),
                *greedy,
                units,
                pos,
                flags,
                caps,
                end_limit,
                0,
                &mut out,
            );
            *caps = saved;
            out
        }
        Node::Group { index, inner } => {
            let saved = caps.clone();
            let mut out = Vec::new();
            for (end, mut candidate_caps) in
                mat_candidates(inner, units, pos, flags, caps, end_limit)
            {
                if let Some(slot) = candidate_caps.get_mut(*index) {
                    *slot = Some((pos, end));
                }
                out.push((end, candidate_caps));
                *caps = saved.clone();
            }
            *caps = saved;
            out
        }
        Node::NonCapture(inner) => mat_candidates(inner, units, pos, flags, caps, end_limit),
        Node::Modifier { on, off, inner } => {
            let modified = Flags {
                ignore_case: (flags.ignore_case || on.0) && !off.0,
                multiline: (flags.multiline || on.1) && !off.1,
                dot_all: (flags.dot_all || on.2) && !off.2,
                unicode: flags.unicode,
            };
            mat_candidates(inner, units, pos, &modified, caps, end_limit)
        }
        Node::Alt(branches) => {
            let saved = caps.clone();
            let mut out = Vec::new();
            for branch in branches {
                *caps = saved.clone();
                out.extend(mat_candidates(branch, units, pos, flags, caps, end_limit));
            }
            *caps = saved;
            out
        }
        Node::Concat(items) => mat_concat_candidates(items, 0, units, pos, flags, caps, end_limit),
        _ => {
            let saved = caps.clone();
            if let Some(end) = mat(node, units, pos, flags, caps, end_limit) {
                vec![(end, caps.clone())]
            } else {
                *caps = saved;
                Vec::new()
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_repeat_candidates(
    inner: &Node,
    min: usize,
    max: usize,
    greedy: bool,
    units: &[u16],
    pos: usize,
    flags: &Flags,
    caps: &mut Vec<Option<(usize, usize)>>,
    end_limit: usize,
    count: usize,
    out: &mut Vec<(usize, Vec<Option<(usize, usize)>>)>,
) {
    if !greedy && count >= min {
        out.push((pos, caps.clone()));
    }
    if count < max {
        let before = caps.clone();
        for (next, next_caps) in repeat_atom_candidates(inner, units, pos, flags, caps, end_limit) {
            if next == pos && count >= min {
                continue;
            }
            *caps = next_caps;
            collect_repeat_candidates(
                inner,
                min,
                max,
                greedy,
                units,
                next,
                flags,
                caps,
                end_limit,
                count + 1,
                out,
            );
            *caps = before.clone();
        }
    }
    if greedy && count >= min {
        out.push((pos, caps.clone()));
    }
}

fn mat_concat_candidates(
    items: &[Node],
    idx: usize,
    units: &[u16],
    pos: usize,
    flags: &Flags,
    caps: &mut Vec<Option<(usize, usize)>>,
    end_limit: usize,
) -> Vec<(usize, Vec<Option<(usize, usize)>>)> {
    if idx >= items.len() {
        return vec![(pos, caps.clone())];
    }
    let saved = caps.clone();
    let mut out = Vec::new();
    for (end, next_caps) in mat_candidates(&items[idx], units, pos, flags, caps, end_limit) {
        *caps = next_caps;
        out.extend(mat_concat_candidates(
            items,
            idx + 1,
            units,
            end,
            flags,
            caps,
            end_limit,
        ));
        *caps = saved.clone();
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn match_captured_repeat(
    cap_index: usize,
    inner: &Node,
    min: usize,
    max: Option<usize>,
    greedy: bool,
    rest: &[Node],
    units: &[u16],
    pos: usize,
    flags: &Flags,
    caps: &mut Vec<Option<(usize, usize)>>,
    end_limit: usize,
) -> Option<usize> {

    if REPEAT_CONT_FAIL_MEMO.with(|m| m.borrow().is_some()) {
        let max_c = max.unwrap_or((units.len().saturating_sub(pos) + 1).max(min));
        return match_captured_repeat_recur(
            cap_index, inner, min, max_c, greedy, rest, units, pos, flags, caps, end_limit, pos, 0,
        );
    }
    let mut candidates = Vec::new();
    let saved = caps.clone();
    collect_repeat_candidates(
        inner,
        min,
        max.unwrap_or((units.len().saturating_sub(pos) + 1).max(min)),
        greedy,
        units,
        pos,
        flags,
        caps,
        end_limit,
        0,
        &mut candidates,
    );
    for (after, candidate_caps) in candidates {
        *caps = candidate_caps;

        if let Some(slot) = caps.get_mut(cap_index) {
            *slot = Some((pos, after));
        }
        if rest.is_empty() {
            return Some(after);
        }

        let memo_key = repeat_cont_memo_key(rest, after, end_limit);
        let known_fail = REPEAT_CONT_FAIL_MEMO
            .with(|m| m.borrow().as_ref().is_some_and(|s| s.contains(&memo_key)));
        if known_fail {
            continue;
        }
        if let Some(end) = match_concat(rest, 0, units, after, flags, caps, end_limit) {
            return Some(end);
        }
        REPEAT_CONT_FAIL_MEMO.with(|m| {
            if let Some(s) = m.borrow_mut().as_mut() {
                s.insert(memo_key);
            }
        });
    }
    *caps = saved;
    None
}

#[allow(clippy::too_many_arguments)]
fn match_captured_repeat_recur(
    cap_index: usize,
    inner: &Node,
    min: usize,
    max: usize,
    greedy: bool,
    rest: &[Node],
    units: &[u16],
    pos: usize,
    flags: &Flags,
    caps: &mut Vec<Option<(usize, usize)>>,
    end_limit: usize,
    group_start: usize,
    count: usize,
) -> Option<usize> {
    if greedy && count < max {
        let before = caps.clone();
        for (next, next_caps) in repeat_atom_candidates(inner, units, pos, flags, caps, end_limit) {
            if next == pos && count >= min {
                continue;
            }
            *caps = next_caps;
            if let Some(end) = match_captured_repeat_recur(
                cap_index,
                inner,
                min,
                max,
                greedy,
                rest,
                units,
                next,
                flags,
                caps,
                end_limit,
                group_start,
                count + 1,
            ) {
                return Some(end);
            }
            *caps = before.clone();
        }
    }
    if count >= min {
        let saved = caps.clone();
        if let Some(slot) = caps.get_mut(cap_index) {
            *slot = Some((group_start, pos));
        }
        if rest.is_empty() {
            return Some(pos);
        }
        let memo_key = repeat_cont_memo_key(rest, pos, end_limit);
        let known_fail = REPEAT_CONT_FAIL_MEMO
            .with(|m| m.borrow().as_ref().is_some_and(|s| s.contains(&memo_key)));
        if !known_fail {
            if let Some(end) = match_concat(rest, 0, units, pos, flags, caps, end_limit) {
                return Some(end);
            }
            REPEAT_CONT_FAIL_MEMO.with(|m| {
                if let Some(s) = m.borrow_mut().as_mut() {
                    s.insert(memo_key);
                }
            });
        }
        *caps = saved;
    }
    if !greedy && count < max {
        let before = caps.clone();
        for (next, next_caps) in repeat_atom_candidates(inner, units, pos, flags, caps, end_limit) {
            if next == pos && count >= min {
                continue;
            }
            *caps = next_caps;
            if let Some(end) = match_captured_repeat_recur(
                cap_index,
                inner,
                min,
                max,
                greedy,
                rest,
                units,
                next,
                flags,
                caps,
                end_limit,
                group_start,
                count + 1,
            ) {
                return Some(end);
            }
            *caps = before.clone();
        }
    }
    None
}

pub fn has_repeat_with_capture(node: &Node) -> bool {
    match node {
        Node::Never => false,
        Node::Repeat { inner, .. } => contains_capture(inner) || has_repeat_with_capture(inner),
        Node::Concat(items) | Node::Alt(items) => items.iter().any(has_repeat_with_capture),
        Node::Group { inner, .. } | Node::NonCapture(inner) | Node::Modifier { inner, .. } => {
            has_repeat_with_capture(inner)
        }
        Node::Look { inner, .. } => has_repeat_with_capture(inner),
        _ => false,
    }
}

pub fn has_scoped_modifier(node: &Node) -> bool {
    match node {
        Node::Never => false,
        Node::Modifier { .. } => true,
        Node::Repeat { inner, .. } | Node::NonCapture(inner) | Node::Group { inner, .. } => {
            has_scoped_modifier(inner)
        }
        Node::Concat(items) | Node::Alt(items) => items.iter().any(has_scoped_modifier),
        Node::Look { inner, .. } => has_scoped_modifier(inner),
        _ => false,
    }
}

pub fn has_any_char(node: &Node) -> bool {
    match node {
        Node::AnyChar => true,
        Node::Repeat { inner, .. } | Node::NonCapture(inner) | Node::Group { inner, .. } => {
            has_any_char(inner)
        }
        Node::Modifier { inner, .. } => has_any_char(inner),
        Node::Concat(items) | Node::Alt(items) => items.iter().any(has_any_char),
        Node::Look { inner, .. } => has_any_char(inner),
        _ => false,
    }
}

pub fn has_word_boundary(node: &Node) -> bool {
    match node {
        Node::Anchor(AnchorKind::WordBoundary | AnchorKind::NotWordBoundary) => true,
        Node::Repeat { inner, .. } | Node::NonCapture(inner) | Node::Group { inner, .. } => {
            has_word_boundary(inner)
        }
        Node::Modifier { inner, .. } => has_word_boundary(inner),
        Node::Concat(items) | Node::Alt(items) => items.iter().any(has_word_boundary),
        Node::Look { inner, .. } => has_word_boundary(inner),
        _ => false,
    }
}

pub fn has_property_kind(node: &Node, target: PropKind) -> bool {
    fn expr_has_property_kind(expr: &ClassExpr, target: PropKind) -> bool {
        match expr {
            ClassExpr::Special(SpecialClass::Property(kind, _)) => *kind == target,
            ClassExpr::Union(items) | ClassExpr::Intersection(items) => items
                .iter()
                .any(|item| expr_has_property_kind(item, target)),
            ClassExpr::Difference(a, b) => {
                expr_has_property_kind(a, target) || expr_has_property_kind(b, target)
            }
            ClassExpr::Not(inner) => expr_has_property_kind(inner, target),
            _ => false,
        }
    }

    match node {
        Node::Class(cc) => expr_has_property_kind(&cc.expr, target),
        Node::Repeat { inner, .. } | Node::NonCapture(inner) | Node::Group { inner, .. } => {
            has_property_kind(inner, target)
        }
        Node::Modifier { inner, .. } => has_property_kind(inner, target),
        Node::Concat(items) | Node::Alt(items) => {
            items.iter().any(|item| has_property_kind(item, target))
        }
        Node::Look { inner, .. } => has_property_kind(inner, target),
        _ => false,
    }
}

pub fn has_class_surrogate(node: &Node) -> bool {
    match node {
        Node::Class(cc) => cc.expr.has_surrogate(),
        Node::Repeat { inner, .. } | Node::NonCapture(inner) | Node::Group { inner, .. } => {
            has_class_surrogate(inner)
        }
        Node::Modifier { inner, .. } => has_class_surrogate(inner),
        Node::Concat(items) | Node::Alt(items) => items.iter().any(has_class_surrogate),
        Node::Look { inner, .. } => has_class_surrogate(inner),
        _ => false,
    }
}

pub fn has_unit_atom(node: &Node) -> bool {
    match node {
        Node::Unit(_) => true,
        Node::Repeat { inner, .. } | Node::NonCapture(inner) | Node::Group { inner, .. } => {
            has_unit_atom(inner)
        }
        Node::Modifier { inner, .. } => has_unit_atom(inner),
        Node::Concat(items) | Node::Alt(items) => items.iter().any(has_unit_atom),
        Node::Look { inner, .. } => has_unit_atom(inner),
        _ => false,
    }
}

pub fn has_multichar_class_string(node: &Node) -> bool {
    match node {
        Node::Class(cc) => cc.expr.has_multichar_string(),
        Node::Repeat { inner, .. } | Node::NonCapture(inner) | Node::Group { inner, .. } => {
            has_multichar_class_string(inner)
        }
        Node::Modifier { inner, .. } => has_multichar_class_string(inner),
        Node::Concat(items) | Node::Alt(items) => items.iter().any(has_multichar_class_string),
        Node::Look { inner, .. } => has_multichar_class_string(inner),
        _ => false,
    }
}

fn contains_capture(node: &Node) -> bool {
    match node {
        Node::Never => false,
        Node::Group { .. } => true,
        Node::Repeat { inner, .. } | Node::NonCapture(inner) | Node::Modifier { inner, .. } => {
            contains_capture(inner)
        }
        Node::Concat(items) | Node::Alt(items) => items.iter().any(contains_capture),
        Node::Look { inner, .. } => contains_capture(inner),
        _ => false,
    }
}

fn collect_capture_indices(node: &Node, out: &mut Vec<usize>) {
    match node {
        Node::Group { index, inner } => {
            out.push(*index);
            collect_capture_indices(inner, out);
        }
        Node::Repeat { inner, .. } | Node::NonCapture(inner) | Node::Modifier { inner, .. } => {
            collect_capture_indices(inner, out)
        }
        Node::Concat(items) | Node::Alt(items) => {
            for item in items {
                collect_capture_indices(item, out);
            }
        }
        Node::Look { inner, .. } => collect_capture_indices(inner, out),
        _ => {}
    }
}

fn nonunicode_canonicalize(c: char) -> char {
    let mut upper = c.to_uppercase();
    let first = upper.next();
    if let (Some(up), None) = (first, upper.next()) {
        if (c as u32) >= 0x80 && (up as u32) < 0x80 {
            c
        } else {
            up
        }
    } else {
        c
    }
}

fn unicode_case_key(c: char) -> char {
    let cp = c as u32;
    if (0x10400..=0x10427).contains(&cp) {
        return char::from_u32(cp + 0x28).unwrap_or(c);
    }
    if (0x10428..=0x1044F).contains(&cp) {
        return c;
    }

    match c {
        '\u{017F}' | 'S' | 's' => 's',
        '\u{212A}' | 'K' | 'k' => 'k',
        '\u{00B5}' | '\u{039C}' | '\u{03BC}' => '\u{03BC}',
        '\u{0178}' | '\u{00FF}' => '\u{00FF}',
        '\u{1E9E}' | '\u{00DF}' => '\u{00DF}',
        '\u{212B}' | '\u{00C5}' | '\u{00E5}' => '\u{00E5}',
        '\u{0345}' | '\u{0399}' | '\u{03B9}' | '\u{1FBE}' => '\u{03B9}',
        '\u{0390}' | '\u{1FD3}' => '\u{0390}',
        '\u{0392}' | '\u{03B2}' | '\u{03D0}' => '\u{03B2}',
        '\u{0395}' | '\u{03B5}' | '\u{03F5}' => '\u{03B5}',
        '\u{0398}' | '\u{03B8}' | '\u{03D1}' | '\u{03F4}' => '\u{03B8}',
        '\u{039A}' | '\u{03BA}' | '\u{03F0}' => '\u{03BA}',
        '\u{03A0}' | '\u{03C0}' | '\u{03D6}' => '\u{03C0}',
        '\u{03A1}' | '\u{03C1}' | '\u{03F1}' => '\u{03C1}',
        '\u{03A3}' | '\u{03C2}' | '\u{03C3}' => '\u{03C3}',
        '\u{03A6}' | '\u{03C6}' | '\u{03D5}' => '\u{03C6}',
        '\u{03B0}' | '\u{1FE3}' => '\u{03B0}',
        '\u{0412}' | '\u{0432}' | '\u{1C80}' => '\u{0432}',
        '\u{0414}' | '\u{0434}' | '\u{1C81}' => '\u{0434}',
        '\u{041E}' | '\u{043E}' | '\u{1C82}' => '\u{043E}',
        '\u{0421}' | '\u{0441}' | '\u{1C83}' => '\u{0441}',
        '\u{0422}' | '\u{0442}' | '\u{1C84}' | '\u{1C85}' => '\u{0442}',
        '\u{042A}' | '\u{044A}' | '\u{1C86}' => '\u{044A}',
        '\u{0462}' | '\u{0463}' | '\u{1C87}' => '\u{0463}',
        '\u{1C88}' | '\u{A64A}' | '\u{A64B}' => '\u{A64B}',
        '\u{1E60}' | '\u{1E61}' | '\u{1E9B}' => '\u{1E61}',
        '\u{FB05}' | '\u{FB06}' => '\u{FB05}',
        _ => c.to_lowercase().next().unwrap_or(c),
    }
}

fn canonicalize_for_compare(c: char, unicode: bool) -> char {
    if unicode {
        unicode_case_key(c)
    } else {
        nonunicode_canonicalize(c)
    }
}

fn range_contains_canonical(c: char, lo: char, hi: char, unicode: bool) -> bool {
    let c = canonicalize_for_compare(c, unicode);
    let lo = canonicalize_for_compare(lo, unicode);
    let hi = canonicalize_for_compare(hi, unicode);
    if lo <= hi {
        c >= lo && c <= hi
    } else {
        c >= hi && c <= lo
    }
}

fn char_eq(a: char, b: char, ignore_case: bool, unicode: bool) -> bool {
    if !ignore_case {
        return a == b;
    }
    if a == b {
        return true;
    }
    canonicalize_for_compare(a, unicode) == canonicalize_for_compare(b, unicode)
}

fn unit_eq(a: u16, b: u16, ignore_case: bool, unicode: bool) -> bool {
    if a == b {
        return true;
    }
    if !ignore_case {
        return false;
    }
    match (char::from_u32(a as u32), char::from_u32(b as u32)) {
        (Some(ca), Some(cb)) => char_eq(ca, cb, true, unicode),
        _ => false,
    }
}

fn at_word_boundary(units: &[u16], pos: usize, ignore_case: bool, unicode: bool) -> bool {

    let is_word = |u: u16| {
        unit_as_char(&[u], 0)
            .map(|c| is_word_char(c, ignore_case, unicode))
            .unwrap_or(false)
    };
    let pw = pos != 0 && is_word(units[pos - 1]);
    let nw = units.get(pos).copied().map(is_word).unwrap_or(false);
    pw != nw
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(pat: &str, flags: &str, s: &str) -> Option<(usize, usize)> {
        let re = compile(pat, flags).unwrap();
        let units: Vec<u16> = s.encode_utf16().collect();
        find_at(&re, &units, 0).map(|m| (m.start, m.end))
    }

    fn m_units(pat: &str, flags: &str, units: &[u16]) -> Option<(usize, usize)> {
        let re = compile(pat, flags).unwrap();
        find_at(&re, units, 0).map(|m| (m.start, m.end))
    }

    #[test]
    fn literal() {
        assert_eq!(m("foo", "", "xfoox"), Some((1, 4)));
    }
    #[test]
    fn any_char() {
        assert_eq!(m("f.o", "", "fxo"), Some((0, 3)));
    }
    #[test]
    fn star_greedy() {
        assert_eq!(m("a*", "", "aaab"), Some((0, 3)));
    }
    #[test]
    fn lazy() {
        assert_eq!(m("a.*?b", "", "axxbyyb"), Some((0, 4)));
    }
    #[test]
    fn alt() {
        assert_eq!(m("cat|dog", "", "I see a dog!"), Some((8, 11)));
    }

    #[test]
    fn ansi_regex_left_branch_repeat_backtracks_to_csi_alternative() {
        let pat = r"[\u001B\u009B][[\]()#;?]*(?:(?:(?:(?:;[-a-zA-Z\d\/#&.:=?%@~_]+)*|[a-zA-Z\d]+(?:;[-a-zA-Z\d\/#&.:=?%@~_]*)*)?\u0007)|(?:(?:\d{1,4}(?:;\d{0,4})*)?[\dA-PR-TZcf-ntqry=><~]))";
        assert_eq!(m(pat, "g", "\u{001b}[37m"), Some((0, 5)));
    }

    #[test]
    fn unicode_lone_surrogate_escape_is_no_match_atom() {
        assert_eq!(m(r"^|\udf06", "u", "\u{1D306}"), Some((0, 0)));
        assert_eq!(m(r"\udf06", "u", "\u{1D306}"), None);
    }

    #[test]
    fn nonunicode_braced_u_escape_is_identity_escape_then_quantifier() {
        assert_eq!(m(r"\u{41}", "", &"u".repeat(41)), Some((0, 41)));
        assert_eq!(m(r"\u{41}", "u", "ABC"), Some((0, 1)));
        assert_eq!(m_units(r"\u{1F438}?", "u", &[0xD83D]), Some((0, 0)));
    }

    #[test]
    fn unicode_braced_surrogate_escapes_match_unpaired_units_only() {
        assert_eq!(m_units(r"\u{D83D}", "u", &[0xD83D, 0xDBFF]), Some((0, 1)));
        assert_eq!(m_units(r"\u{D83D}", "u", &[0xD83D, 0xDC00]), None);
        assert_eq!(m_units(r"\u{D83D}", "u", &[0xD83D, 0xE000]), Some((0, 1)));
        assert_eq!(m_units(r"\u{DC38}", "u", &[0xD7FF, 0xDC38]), Some((1, 2)));
        assert_eq!(m_units(r"\u{DC38}", "u", &[0xD800, 0xDC38]), None);
        assert_eq!(m_units(r"\u{DC38}", "u", &[0xDC00, 0xDC38]), Some((1, 2)));
    }

    #[test]
    fn nonunicode_fixed_surrogate_escapes_are_raw_units() {
        assert_eq!(
            m_units(r"\uD83D\uDC38", "", &[0xD83D, 0xDC38]),
            Some((0, 2))
        );
        assert_eq!(
            m_units(r"\uD83D\uDC38+", "", &[0xD83D, 0xDC38, 0xDC38]),
            Some((0, 3))
        );
        assert_eq!(m_units(r"\uD83D\uDC38?", "", &[0xD83D]), Some((0, 1)));
    }

    #[test]
    fn nonunicode_surrogate_class_ranges_are_code_unit_ranges() {
        assert_eq!(
            m_units(r"\uD83D[\uDC00-\uDFFF]", "", &[0x0068, 0xD83D, 0xDC4B]),
            Some((1, 3))
        );
        assert_eq!(
            m_units(r"[\uDC00-\uDFFF]", "", &[0xD83D, 0xDC4B]),
            Some((1, 2))
        );
        assert_eq!(m_units(r"[\uDC00-\uDFFF]", "", &[0xD83D]), None);
    }

    #[test]
    fn ignore_case_canonicalize_is_unicode_gated() {
        assert_eq!(m("\u{039C}", "i", "\u{00B5}"), Some((0, 1)));
        assert_eq!(m("\u{212A}", "i", "k"), None);
        assert_eq!(m("\u{212A}", "iu", "k"), Some((0, 1)));
        assert_eq!(m("\u{017F}", "i", "s"), None);
        assert_eq!(m("\u{017F}", "iu", "s"), Some((0, 1)));
    }

    #[test]
    fn ignore_case_ranges_use_canonical_interval() {
        assert_eq!(m(r"([a-z]+)([0-9]+)", "i", "aBc12"), Some((0, 5)));
        assert_eq!(m(r"^[a-z]$", "i", "B"), Some((0, 1)));
        assert_eq!(m(r"^[a-z]$", "i", "\u{212A}"), None);
        assert_eq!(m(r"^[a-z]$", "iu", "\u{212A}"), Some((0, 1)));
    }

    #[test]
    fn word_boundary_unicode_ignore_case_admits_extra_fold_chars() {
        assert_eq!(m(r"\b\u{212A}\b", "i", "k"), None);
        assert_eq!(m(r"\b\u{212A}\b", "iu", "k"), Some((0, 1)));
    }

    #[test]
    fn named_group_decodes_raw_surrogate_marker_pair_name() {
        let marker =
            |unit: u16| char::from_u32(RAW_SURROGATE_MARKER_BASE + (unit as u32 - 0xD800)).unwrap();
        let name = format!("{}{}", marker(0xD835), marker(0xDCD1));
        let pattern = format!("(?<{name}>x)\\k<{name}>");
        let re = compile(&pattern, "").unwrap();
        let units: Vec<u16> = "xx".encode_utf16().collect();

        assert_eq!(re.named_group_order[0], ("\u{1D4D1}".to_string(), 1));
        assert!(find_at(&re, &units, 0).is_some());
    }

    #[test]
    fn unicode_ignore_case_uses_simple_common_casefold_keys() {
        assert_eq!(m(r"[\u{0390}]", "iu", "\u{1FD3}"), Some((0, 1)));
        assert_eq!(m(r"[\u{1FD3}]", "iu", "\u{0390}"), Some((0, 1)));
        assert_eq!(m(r"[\u{03B0}]", "iu", "\u{1FE3}"), Some((0, 1)));
        assert_eq!(m(r"[\u{1FE3}]", "iu", "\u{03B0}"), Some((0, 1)));
        assert_eq!(m(r"[\u{FB05}]", "iu", "\u{FB06}"), Some((0, 1)));
        assert_eq!(
            m(r"[\u{0345}]+", "iu", "\u{0345}\u{03B9}\u{0399}\u{1FBE}"),
            Some((0, 4))
        );
        assert_eq!(
            m(r"[\u{0392}]+", "iu", "\u{0392}\u{03B2}\u{03D0}"),
            Some((0, 3))
        );
        assert_eq!(
            m(r"[\u{0422}]+", "iu", "\u{0422}\u{0442}\u{1C84}\u{1C85}"),
            Some((0, 4))
        );
        assert_eq!(
            m(r"[\u{1C88}]+", "iu", "\u{1C88}\u{A64A}\u{A64B}"),
            Some((0, 3))
        );
        assert_eq!(m("\u{10400}+", "iu", "\u{10400}\u{10428}"), Some((0, 4)));
    }

    #[test]
    fn annex_b_unknown_named_backref_falls_back_to_identity_text() {
        assert_eq!(m(r"\k<a>", "", "k<a>"), Some((0, 4)));
        assert_eq!(m(r"\k<a", "", "k<a"), Some((0, 3)));
        assert!(compile(r"\k<a>", "u").is_err());
        assert_eq!(m(r"\k<a>(?<a>x)", "", "x"), Some((0, 1)));
    }

    #[test]
    fn class() {
        assert_eq!(m("[a-c]+", "", "xxxabbc"), Some((3, 7)));
    }
    #[test]
    fn anchor() {
        assert_eq!(m("^abc", "", "abc"), Some((0, 3)));
        assert_eq!(m("^abc", "", "xabc"), None);
    }
    #[test]
    fn lookahead_pos() {
        assert_eq!(m("foo(?=bar)", "", "foobar"), Some((0, 3)));
        assert_eq!(m("foo(?=bar)", "", "fooqux"), None);
    }
    #[test]
    fn lookahead_neg() {
        assert_eq!(m("foo(?!bar)", "", "fooqux"), Some((0, 3)));
        assert_eq!(m("foo(?!bar)", "", "foobar"), None);
    }
    #[test]
    #[ignore]
    fn backref() {
        assert_eq!(m("(a+)\\1", "", "aaaa"), Some((0, 4)));
    }
    #[test]
    fn backref_after_optional_repeat_group() {
        let pat = r"-([0-9]|[a-wy-z])-(.*-)?\1(?![a-z0-9])";
        assert_eq!(m(pat, "i", "pt-u-ca-gregory-u-nu-latn"), Some((2, 17)));
    }

    #[test]
    fn simple_digit_fast_admits_literal_digit_capture_sequences() {
        let route = compile(r"\/api\/v(\d+)\/users\/(\d+)", "").unwrap();
        assert!(route.simple_digit.is_some());
        let input = "GET /api/v2/users/1042?trace=7 HTTP/1.1";
        let units: Vec<u16> = input.encode_utf16().collect();
        let m = find_at(&route, &units, 0).unwrap();
        assert_eq!(&input[m.start..m.end], "/api/v2/users/1042");
        assert_eq!(m.captures[1], Some((10, 11)));
        assert_eq!(m.captures[2], Some((18, 22)));

        let trace = compile(r"[?&]trace=(\d+)", "").unwrap();
        assert!(trace.simple_digit.is_some());
        let m = find_at(&trace, &units, 0).unwrap();
        assert_eq!(&input[m.start..m.end], "?trace=7");
        assert_eq!(m.captures[1], Some((29, 30)));
    }

    #[test]
    fn simple_digit_fast_rejects_flagged_or_non_digit_shapes() {
        assert!(compile(r"\/api\/v(\d+)\/users\/(\d+)", "i")
            .unwrap()
            .simple_digit
            .is_none());
        assert!(compile(r"\/api\/v([0-9]+)\/users\/(\d+)", "")
            .unwrap()
            .simple_digit
            .is_none());
    }

    #[test]
    fn pathe_pattern() {

        let pat = r"^[/\\](?![/\\])|^[/\\]{2}(?!\.)|^[A-Za-z]:[/\\]";
        let re = compile(pat, "").unwrap();
        assert!(is_match(&re, "/foo"));
        assert!(is_match(&re, "\\\\server"));
        assert!(is_match(&re, "C:/foo"));
        assert!(!is_match(&re, "foo/bar"));
    }
    #[test]
    fn picomatch_pattern() {

        let pat = r"^(?:(?!\.)(?=.)[^/]*?\.js\/?)$";
        let re = compile(pat, "").unwrap();
        assert!(is_match(&re, "foo.js"));
        assert!(!is_match(&re, ".hidden.js"));
        assert!(!is_match(&re, "foo.txt"));
    }

    #[test]
    fn greedy_simple_atom_fast_path_backtracks_correctly() {

        let m = |pat: &str, flags: &str, s: &str| -> Option<(usize, usize)> {
            let re = compile(pat, flags).unwrap();
            let units: Vec<u16> = s.encode_utf16().collect();
            find_at(&re, &units, 0).map(|mm| (mm.start, mm.end))
        };
        assert_eq!(m("a+a", "", "aaa"), Some((0, 3)));
        assert_eq!(m("a+b", "", "aaab"), Some((0, 4)));
        assert_eq!(m(".+x", "", "aaxbbx"), Some((0, 6)));
        assert_eq!(m("[0-9]+5", "", "123455"), Some((0, 6)));
        assert_eq!(m("a{2,4}", "", "aaaaa"), Some((0, 4)));

        assert_eq!(m("a{2,4}b", "", "aaaaab"), Some((1, 6)));
        assert_eq!(m("x*y", "", "y"), Some((0, 1)));
        assert_eq!(m(r"\p{L}+", "u", "abc9"), Some((0, 3)));
        assert_eq!(m("😀+x", "u", "😀😀x"), Some((0, 5)));
    }

    #[test]
    fn lazy_repeat_at_group_end_expands_for_continuation() {

        assert!(is_match(&compile(r"^(?:a*?)$", "").unwrap(), "a"));
        assert!(is_match(&compile(r"^(?:(?:a)*?)$", "").unwrap(), "a"));
        assert!(is_match(&compile(r"^(?:(?:a)+?)$", "").unwrap(), "aa"));
        assert!(is_match(&compile(r"^(?:(?:ab)*?)$", "").unwrap(), "abab"));
        assert!(is_match(&compile(r"^(?:(a)*?)$", "").unwrap(), "a"));

        let pat = r"^(?:a(?:\/(?!\.)(?:(?:(?!(?:^|\/)\.).)*?)\/|\/|$)(?!\.)(?=.)[^/]*?\.js)$";
        assert!(is_match(&compile(pat, "").unwrap(), "a/b/c.js"));
    }

    #[test]
    fn lazy_repeat_in_alt_branch_expands_for_continuation() {

        assert!(is_match(&compile(r"^(?:a*?|x)$", "").unwrap(), "aa"));
        assert!(is_match(&compile(r"^(a*?|x)$", "").unwrap(), "aa"));

        assert!(is_match(&compile(r"(?<=(?:a*?))b", "").unwrap(), "aab"));
        assert!(is_match(&compile(r"(?<=(a+?|x))b", "").unwrap(), "aab"));
    }

    #[test]
    fn optional_group_with_inner_lazy_repeat_expands_for_continuation() {
        let re = compile(r#"^<\/?([a-zA-Z][a-zA-Z0-9]*)((?:\s+[^<>]*?)?)\s*\/?>"#, "").unwrap();
        let units: Vec<u16> = r#"<a href="/p" onclick="hax()">"#.encode_utf16().collect();
        let m = find_at(&re, &units, 0).unwrap();
        assert_eq!((m.start, m.end), (0, units.len()));
    }

    #[test]
    fn repeated_atom_resets_inner_optional_captures_each_iteration() {
        let re = compile(r"(z)((a+)?(b+)?(c))*", "").unwrap();
        let units: Vec<u16> = "zaacbbbcac".encode_utf16().collect();
        let m = find_at(&re, &units, 0).unwrap();
        let groups: Vec<Option<&str>> = m
            .captures
            .iter()
            .map(|c| c.map(|(s, e)| &"zaacbbbcac"[s..e]))
            .collect();
        assert_eq!(
            groups,
            vec![
                Some("zaacbbbcac"),
                Some("z"),
                Some("ac"),
                Some("a"),
                None,
                Some("c")
            ]
        );
    }
}
