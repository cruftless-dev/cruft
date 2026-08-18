
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SegmentRecord {
    pub(crate) segment: String,
    pub(crate) utf16_index: usize,
    pub(crate) is_word_like: Option<bool>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SegmentClass {
    Word,
    Space,
    Other,
}

pub(crate) fn segment_text(input: &str, granularity: &str) -> Vec<SegmentRecord> {
    let chars: Vec<char> = input.chars().collect();

    match granularity {
        "word" => segment_words(&chars),
        "sentence" => segment_sentences(&chars),
        _ => segment_graphemes(&chars),
    }
}

fn absorb_word_extenders(chars: &[char], i: &mut usize, u16: &mut usize, text: &mut String) {
    while *i < chars.len() && is_combining_mark(chars[*i]) {
        let was_zwj = chars[*i] == '\u{200d}';
        text.push(chars[*i]);
        *u16 += chars[*i].len_utf16();
        *i += 1;
        if was_zwj && *i < chars.len() && !chars[*i].is_whitespace() {
            text.push(chars[*i]);
            *u16 += chars[*i].len_utf16();
            *i += 1;
        }
    }
}

fn segment_words(chars: &[char]) -> Vec<SegmentRecord> {
    let mut records = Vec::new();
    let mut i = 0usize;
    let mut u16 = 0usize;
    while i < chars.len() {
        let start = u16;
        let class = segment_class(chars[i]);
        let mut text = String::new();
        if class == SegmentClass::Other {
            text.push(chars[i]);
            u16 += chars[i].len_utf16();
            i += 1;
            absorb_word_extenders(chars, &mut i, &mut u16, &mut text);
        } else {
            while i < chars.len() && segment_class(chars[i]) == class {
                text.push(chars[i]);
                u16 += chars[i].len_utf16();
                i += 1;
                absorb_word_extenders(chars, &mut i, &mut u16, &mut text);
                if class == SegmentClass::Word
                    && i + 1 < chars.len()
                    && chars[i] == '.'
                    && chars[i - 1].is_ascii_digit()
                    && chars[i + 1].is_ascii_digit()
                {
                    text.push(chars[i]);
                    u16 += chars[i].len_utf16();
                    i += 1;
                }
                if class == SegmentClass::Word
                    && i + 1 < chars.len()
                    && is_mid_word_connector(chars[i])
                    && chars[i - 1].is_alphanumeric()
                    && chars[i + 1].is_alphanumeric()
                {
                    text.push(chars[i]);
                    u16 += chars[i].len_utf16();
                    i += 1;
                }
            }
        }
        records.push(SegmentRecord {
            segment: text,
            utf16_index: start,
            is_word_like: Some(class == SegmentClass::Word),
        });
    }
    records
}

fn segment_sentences(chars: &[char]) -> Vec<SegmentRecord> {
    let mut records = Vec::new();
    let mut u16 = 0usize;
    let mut start = 0usize;
    let mut text = String::new();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if text.is_empty() {
            start = u16;
        }
        text.push(c);
        u16 += c.len_utf16();
        i += 1;
        if is_sentence_terminal(c) && !is_decimal_point(chars, i - 1) {
            while i < chars.len() && is_sentence_terminal(chars[i]) {
                let term = chars[i];
                text.push(term);
                u16 += term.len_utf16();
                i += 1;
            }
            while i < chars.len() && is_sentence_close(chars[i]) {
                let close = chars[i];
                text.push(close);
                u16 += close.len_utf16();
                i += 1;
            }
            while i < chars.len() && chars[i].is_whitespace() {
                let ws = chars[i];
                text.push(ws);
                u16 += ws.len_utf16();
                i += 1;
            }
            records.push(SegmentRecord {
                segment: std::mem::take(&mut text),
                utf16_index: start,
                is_word_like: None,
            });
        }
    }
    if !text.is_empty() {
        records.push(SegmentRecord {
            segment: text,
            utf16_index: start,
            is_word_like: None,
        });
    }
    records
}

fn is_sentence_terminal(c: char) -> bool {
    matches!(c, '.' | '!' | '?')
}

fn is_sentence_close(c: char) -> bool {
    matches!(
        c,
        '"' | '\'' | ')' | ']' | '}' | '\u{2019}' | '\u{201d}' | '\u{300d}' | '\u{300f}'
    )
}

fn is_decimal_point(chars: &[char], dot_index: usize) -> bool {
    dot_index > 0
        && dot_index + 1 < chars.len()
        && chars[dot_index] == '.'
        && chars[dot_index - 1].is_ascii_digit()
        && chars[dot_index + 1].is_ascii_digit()
}

fn segment_graphemes(chars: &[char]) -> Vec<SegmentRecord> {
    let mut records = Vec::new();
    let mut u16 = 0usize;
    let mut i = 0usize;
    while i < chars.len() {
        let start = u16;
        let mut text = String::new();
        let c = chars[i];
        text.push(c);
        u16 += c.len_utf16();
        i += 1;
        if c == '\r' && i < chars.len() && chars[i] == '\n' {
            let lf = chars[i];
            text.push(lf);
            u16 += lf.len_utf16();
            i += 1;
        }
        if is_prepend(c) && i < chars.len() && !chars[i].is_whitespace() {
            let next = chars[i];
            text.push(next);
            u16 += next.len_utf16();
            i += 1;
        }
        if is_regional_indicator(c) && i < chars.len() && is_regional_indicator(chars[i]) {
            let next = chars[i];
            text.push(next);
            u16 += next.len_utf16();
            i += 1;
        }
        loop {
            let mut consumed = false;
            while i < chars.len() && chars[i] != '\u{200d}' && is_combining_mark(chars[i]) {
                let mark = chars[i];
                text.push(mark);
                u16 += mark.len_utf16();
                i += 1;
                consumed = true;
            }
            if i < chars.len() && chars[i] == '\u{200d}' {
                let zwj = chars[i];
                text.push(zwj);
                u16 += zwj.len_utf16();
                i += 1;
                if i < chars.len() {
                    let next = chars[i];
                    text.push(next);
                    u16 += next.len_utf16();
                    i += 1;
                }
                consumed = true;
            }
            if !consumed {
                break;
            }
        }
        records.push(SegmentRecord {
            segment: text,
            utf16_index: start,
            is_word_like: None,
        });
    }
    records
}

fn segment_class(c: char) -> SegmentClass {
    if c.is_alphanumeric() {
        SegmentClass::Word
    } else if c.is_whitespace() {
        SegmentClass::Space
    } else {
        SegmentClass::Other
    }
}

fn is_mid_word_connector(c: char) -> bool {
    matches!(c, '\'' | '\u{2019}' | '_')
}

fn is_prepend(c: char) -> bool {
    matches!(c as u32, 0x0600..=0x0605 | 0x06dd | 0x070f | 0x0890..=0x0891)
}

fn is_combining_mark(c: char) -> bool {
    matches!(
        c as u32,
        0x0300..=0x036f
            | 0x0591..=0x05bd
            | 0x05bf
            | 0x05c1..=0x05c2
            | 0x05c4..=0x05c5
            | 0x05c7
            | 0x0610..=0x061a
            | 0x064b..=0x065f
            | 0x0670
            | 0x06d6..=0x06dc
            | 0x06df..=0x06e4
            | 0x06e7..=0x06e8
            | 0x06ea..=0x06ed
            | 0x0e31
            | 0x0e34..=0x0e3a
            | 0x0e47..=0x0e4e
            | 0x1100..=0x11ff
            | 0x1ab0..=0x1aff
            | 0x1dc0..=0x1dff
            | 0x20d0..=0x20ff
            | 0xfe00..=0xfe0f
            | 0xfe20..=0xfe2f
            | 0x1f3fb..=0x1f3ff
    ) || c == '\u{200d}'
}

fn is_regional_indicator(c: char) -> bool {
    matches!(c as u32, 0x1f1e6..=0x1f1ff)
}
