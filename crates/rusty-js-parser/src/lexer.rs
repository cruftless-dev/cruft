
use crate::token::{NumberKind, Punct, Span, TemplatePart, Token, TokenKind};

const RAW_SURROGATE_MARKER_BASE: u32 = 0xF0000;

fn raw_surrogate_marker_to_unit(cp: u32) -> Option<u16> {
    let offset = cp.checked_sub(RAW_SURROGATE_MARKER_BASE)?;
    if offset <= 0x7ff {
        Some(0xD800 + offset as u16)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexerGoal {

    Div,

    RegExp,

    TemplateTail,
}

#[derive(Debug, Clone)]
pub struct LexError {
    pub kind: LexErrorKind,
    pub span: Span,
    pub message: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexErrorKind {
    UnterminatedString,
    UnterminatedTemplate,
    UnterminatedRegex,
    UnterminatedComment,
    InvalidEscape,
    InvalidNumeric,
    InvalidIdentifier,
    LegacyOctalInModule,
    UnexpectedChar,
}

pub struct Lexer<'src> {
    src: &'src [u8],

    pos: usize,

    saw_line_terminator: bool,

    at_start: bool,

    pub(crate) strict_mode: bool,

    pub(crate) last_string_had_legacy_escape: bool,

    pub(crate) script_goal: bool,
}

impl<'src> Lexer<'src> {
    pub fn new(src: &'src str) -> Self {
        Self {
            src: src.as_bytes(),
            pos: 0,
            saw_line_terminator: false,
            at_start: true,
            strict_mode: false,
            last_string_had_legacy_escape: false,
            script_goal: true,
        }
    }

    pub fn set_strict(&mut self, strict: bool) {
        self.strict_mode = strict;
    }

    pub fn set_script_goal(&mut self, script_goal: bool) {
        self.script_goal = script_goal;
    }

    pub fn set_pos(&mut self, pos: usize) {
        self.pos = pos;
        self.saw_line_terminator = false;
        self.at_start = false;
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    fn peek_lt_bytes(&self) -> Option<usize> {
        let c = self.peek_byte()?;
        if c == b'\n' {
            return Some(1);
        }
        if c == b'\r' {
            return Some(if self.peek_byte_at(1) == Some(b'\n') {
                2
            } else {
                1
            });
        }
        if c == 0xE2 && self.peek_byte_at(1) == Some(0x80) {
            return match self.peek_byte_at(2) {
                Some(0xA8) | Some(0xA9) => Some(3),
                _ => None,
            };
        }
        None
    }

    fn peek_is_ident_start_strict(&self) -> bool {
        let Some(c) = self.peek_byte() else {
            return false;
        };
        if c < 0x80 {
            return is_identifier_start_byte(c);
        }
        if self.peek_lt_bytes().is_some() {
            return false;
        }
        if let Some(cp) = self.peek_codepoint() {
            if is_unicode_whitespace(cp) {
                return false;
            }
        }
        true
    }

    pub fn next_token(&mut self, goal: LexerGoal) -> Result<Token, LexError> {

        if self.at_start && self.peek_str(2) == Some("#!") {
            let _profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::LexerHashbang,
            );
            let start = self.pos;
            self.pos += 2;
            while self.peek_byte().is_some() {
                if self.peek_lt_bytes().is_some() {
                    break;
                }
                self.advance_one_char();
            }
            self.at_start = false;
            let lexeme = std::str::from_utf8(&self.src[start..self.pos])
                .unwrap()
                .to_string();
            return Ok(Token {
                kind: TokenKind::Hashbang(lexeme),
                span: Span::new(start, self.pos),
                preceded_by_line_terminator: false,
            });
        }
        self.saw_line_terminator = false;
        {
            let _profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::LexerTrivia,
            );
            self.skip_trivia()?;
        }
        let preceded_by_lt = self.saw_line_terminator;
        self.at_start = false;

        let start = self.pos;
        let Some(c) = self.peek_byte() else {
            let _profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::LexerEof,
            );
            return Ok(Token {
                kind: TokenKind::Eof,
                span: Span::new(start, start),
                preceded_by_line_terminator: preceded_by_lt,
            });
        };

        if goal == LexerGoal::TemplateTail && c == b'}' {
            self.pos += 1;
            return self.continue_template(start, preceded_by_lt);
        }

        if c == b'#' {
            let _profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::LexerPrivateIdent,
            );
            self.pos += 1;
            let name = self.read_identifier_name().ok_or_else(|| {
                self.err(
                    LexErrorKind::InvalidIdentifier,
                    start,
                    "expected identifier after #",
                )
            })?;
            return Ok(Token {
                kind: TokenKind::PrivateIdent(name),
                span: Span::new(start, self.pos),
                preceded_by_line_terminator: preceded_by_lt,
            });
        }
        if is_identifier_start_byte(c) || c == b'\\' {
            let _profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::LexerIdent,
            );
            let name = self.read_identifier_name().ok_or_else(|| {
                self.err(LexErrorKind::InvalidIdentifier, start, "invalid identifier")
            })?;
            return Ok(Token {
                kind: TokenKind::Ident(name),
                span: Span::new(start, self.pos),
                preceded_by_line_terminator: preceded_by_lt,
            });
        }

        if c.is_ascii_digit()
            || (c == b'.' && self.peek_byte_at(1).map_or(false, |b| b.is_ascii_digit()))
        {
            let _profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::LexerNumeric,
            );
            return self.read_numeric_literal(start, preceded_by_lt);
        }

        if c == b'"' || c == b'\'' {
            let _profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::LexerString,
            );
            return self.read_string_literal(start, preceded_by_lt, c);
        }

        if c == b'`' {
            let _profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::LexerTemplate,
            );
            self.pos += 1;
            return self.read_template_segment(start, preceded_by_lt, true);
        }

        if c == b'/' && goal == LexerGoal::RegExp {
            let _profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::LexerRegex,
            );
            return self.read_regex_literal(start, preceded_by_lt);
        }

        let _profile = crate::parser::parse_profile::Guard::new(
            crate::parser::parse_profile::Kind::LexerPunct,
        );
        self.read_punctuator(start, preceded_by_lt)
    }

    fn skip_trivia(&mut self) -> Result<(), LexError> {
        loop {
            let Some(c) = self.peek_byte() else {
                return Ok(());
            };
            if is_whitespace_byte(c) {
                self.advance_one_char();
                continue;
            }
            if is_line_terminator_byte(c) {
                self.saw_line_terminator = true;

                if c == b'\r' && self.peek_byte_at(1) == Some(b'\n') {
                    self.pos += 2;
                } else {
                    self.advance_one_char();
                }
                continue;
            }

            if self.script_goal
                && c == b'<'
                && self.peek_byte_at(1) == Some(b'!')
                && self.peek_byte_at(2) == Some(b'-')
                && self.peek_byte_at(3) == Some(b'-')
            {
                self.pos += 4;
                while let Some(b) = self.peek_byte() {
                    if is_line_terminator_byte(b) || self.peek_lt_bytes().is_some() {
                        break;
                    }
                    self.advance_one_char();
                }
                continue;
            }
            if self.script_goal
                && c == b'-'
                && self.peek_byte_at(1) == Some(b'-')
                && self.peek_byte_at(2) == Some(b'>')
                && (self.at_start || self.saw_line_terminator)
            {
                self.pos += 3;
                while let Some(b) = self.peek_byte() {
                    if is_line_terminator_byte(b) || self.peek_lt_bytes().is_some() {
                        break;
                    }
                    self.advance_one_char();
                }
                continue;
            }
            if c == b'/' {
                match self.peek_byte_at(1) {
                    Some(b'/') => {
                        self.pos += 2;
                        while let Some(c) = self.peek_byte() {
                            if is_line_terminator_byte(c) {
                                break;
                            }

                            if self.peek_lt_bytes().is_some() {
                                break;
                            }
                            self.advance_one_char();
                            let _ = c;
                        }
                        continue;
                    }
                    Some(b'*') => {
                        let start = self.pos;
                        self.pos += 2;
                        let mut closed = false;
                        while self.pos < self.src.len() {
                            let b = self.src[self.pos];
                            if is_line_terminator_byte(b) || self.peek_lt_bytes().is_some() {
                                self.saw_line_terminator = true;
                            }
                            if b == b'*' && self.peek_byte_at(1) == Some(b'/') {
                                self.pos += 2;
                                closed = true;
                                break;
                            }
                            self.advance_one_char();
                        }
                        if !closed {
                            return Err(self.err(
                                LexErrorKind::UnterminatedComment,
                                start,
                                "unterminated /* */ comment",
                            ));
                        }
                        continue;
                    }
                    _ => return Ok(()),
                }
            }

            if c == 0xEF && self.peek_byte_at(1) == Some(0xBB) && self.peek_byte_at(2) == Some(0xBF)
            {
                self.pos += 3;
                continue;
            }

            if c >= 0x80 {
                if let Some(cp) = self.peek_codepoint() {
                    if is_unicode_whitespace(cp) {
                        let len = utf8_len(c);
                        self.pos += len;
                        continue;
                    }
                    if cp == 0x2028 || cp == 0x2029 {
                        self.saw_line_terminator = true;
                        self.pos += 3;
                        continue;
                    }
                }
            }
            return Ok(());
        }
    }

    fn read_identifier_name(&mut self) -> Option<String> {
        let mut out = String::new();

        let cp = self.consume_identifier_codepoint(true)?;
        push_char(&mut out, cp);

        while let Some(cp) = self.consume_identifier_codepoint(false) {
            push_char(&mut out, cp);
        }
        Some(out)
    }

    fn consume_identifier_codepoint(&mut self, is_start: bool) -> Option<u32> {
        if self.peek_byte() == Some(b'\\') {

            let save = self.pos;
            self.pos += 1;
            if self.peek_byte() != Some(b'u') {
                self.pos = save;
                return None;
            }
            self.pos += 1;
            let cp = self.read_unicode_escape_inner()?;
            if is_start {
                if !is_id_start(cp) {
                    self.pos = save;
                    return None;
                }
            } else if !is_id_continue(cp) {
                self.pos = save;
                return None;
            }
            return Some(cp);
        }
        let cp = self.peek_codepoint()?;
        if is_start {
            if !is_id_start(cp) {
                return None;
            }
        } else if !is_id_continue(cp) {
            return None;
        }
        let len = utf8_len(self.src[self.pos]);
        self.pos += len;
        Some(cp)
    }

    fn read_unicode_escape_inner(&mut self) -> Option<u32> {

        if self.peek_byte() == Some(b'{') {
            self.pos += 1;
            let mut val: u32 = 0;
            let mut count = 0;
            while let Some(c) = self.peek_byte() {
                if c == b'}' {
                    break;
                }
                let d = hex_digit_value(c)?;
                val = val.checked_mul(16)?.checked_add(d as u32)?;
                if val > 0x10FFFF {
                    return None;
                }
                self.pos += 1;
                count += 1;
            }
            if self.peek_byte() != Some(b'}') {
                return None;
            }

            if count == 0 {
                return None;
            }
            self.pos += 1;
            Some(val)
        } else {

            let mut val: u32 = 0;
            for _ in 0..4 {
                let c = self.peek_byte()?;
                let d = hex_digit_value(c)?;
                val = val * 16 + d as u32;
                self.pos += 1;
            }
            Some(val)
        }
    }

    fn read_numeric_literal(
        &mut self,
        start: usize,
        preceded_by_lt: bool,
    ) -> Result<Token, LexError> {
        let first = self.src[self.pos];

        if first == b'0' {
            if let Some(next) = self.peek_byte_at(1) {
                match next {
                    b'x' | b'X' => {
                        return self.read_radix_int(
                            start,
                            preceded_by_lt,
                            NumberKind::Hex,
                            16,
                            |b| b.is_ascii_hexdigit(),
                        )
                    }
                    b'b' | b'B' => {
                        return self.read_radix_int(
                            start,
                            preceded_by_lt,
                            NumberKind::Binary,
                            2,
                            |b| b == b'0' || b == b'1',
                        )
                    }
                    b'o' | b'O' => {
                        return self.read_radix_int(
                            start,
                            preceded_by_lt,
                            NumberKind::Octal,
                            8,
                            |b| (b'0'..=b'7').contains(&b),
                        )
                    }
                    _ => {}
                }
            }
        }

        let mut has_digits_before_dot = false;
        while let Some(c) = self.peek_byte() {
            if c.is_ascii_digit() || c == b'_' {
                if c == b'_'
                    && (!has_digits_before_dot
                        || !self.peek_byte_at(1).map_or(false, |b| b.is_ascii_digit()))
                {
                    return Err(self.err(
                        LexErrorKind::InvalidNumeric,
                        start,
                        "invalid numeric separator",
                    ));
                }

                if c == b'_' && first == b'0' {
                    return Err(self.err(
                        LexErrorKind::InvalidNumeric,
                        start,
                        "numeric separator not allowed in legacy-octal-like leading-zero form",
                    ));
                }
                if c.is_ascii_digit() {
                    has_digits_before_dot = true;
                }
                self.pos += 1;
            } else {
                break;
            }
        }

        if first == b'0' && self.pos > start + 1 && self.strict_mode {
            let second = self.src[start + 1];
            if second.is_ascii_digit() {
                return Err(self.err(
                    LexErrorKind::LegacyOctalInModule,
                    start,
                    "legacy octal/non-octal-decimal integer literal in strict mode",
                ));
            }
        }

        if first == b'0'
            && !self.strict_mode
            && self.pos > start + 1
            && self.src[start + 1..self.pos]
                .iter()
                .all(|b| (b'0'..=b'7').contains(b))
            && matches!(self.peek_byte(), Some(b'.') | Some(b'e') | Some(b'E'))
        {
            return Err(self.err(
                LexErrorKind::InvalidNumeric,
                start,
                "legacy octal integer literal cannot have a fraction or exponent",
            ));
        }

        let mut has_dot = false;
        if self.peek_byte() == Some(b'.') {
            has_dot = true;
            self.pos += 1;

            let mut frac_first = true;
            let mut frac_last_underscore = false;
            while let Some(c) = self.peek_byte() {
                if c == b'_' {
                    if frac_first || frac_last_underscore {
                        return Err(self.err(
                            LexErrorKind::InvalidNumeric,
                            start,
                            "invalid numeric separator in fractional part",
                        ));
                    }
                    frac_last_underscore = true;
                    self.pos += 1;
                } else if c.is_ascii_digit() {
                    frac_last_underscore = false;
                    self.pos += 1;
                } else {
                    break;
                }
                frac_first = false;
            }
            if frac_last_underscore {
                return Err(self.err(
                    LexErrorKind::InvalidNumeric,
                    start,
                    "trailing numeric separator in fractional part",
                ));
            }
        }

        let mut has_exp = false;
        if matches!(self.peek_byte(), Some(b'e') | Some(b'E')) {
            has_exp = true;
            self.pos += 1;
            if matches!(self.peek_byte(), Some(b'+') | Some(b'-')) {
                self.pos += 1;
            }
            let exp_start = self.pos;

            let mut exp_first = true;
            let mut exp_last_underscore = false;
            while let Some(c) = self.peek_byte() {
                if c == b'_' {
                    if exp_first || exp_last_underscore {
                        return Err(self.err(
                            LexErrorKind::InvalidNumeric,
                            start,
                            "invalid numeric separator in exponent",
                        ));
                    }
                    exp_last_underscore = true;
                    self.pos += 1;
                } else if c.is_ascii_digit() {
                    exp_last_underscore = false;
                    self.pos += 1;
                } else {
                    break;
                }
                exp_first = false;
            }
            if self.pos == exp_start {
                return Err(self.err(
                    LexErrorKind::InvalidNumeric,
                    start,
                    "exponent has no digits",
                ));
            }
            if exp_last_underscore {
                return Err(self.err(
                    LexErrorKind::InvalidNumeric,
                    start,
                    "trailing numeric separator in exponent",
                ));
            }
        }

        if self.peek_byte() == Some(b'n') {
            if has_dot || has_exp {
                return Err(self.err(
                    LexErrorKind::InvalidNumeric,
                    start,
                    "BigInt suffix on non-integer",
                ));
            }

            if first == b'0' && self.pos > start + 1 && self.src[start + 1].is_ascii_digit() {
                return Err(self.err(
                    LexErrorKind::InvalidNumeric,
                    start,
                    "leading-zero (legacy-octal / non-octal-decimal) form is not a valid BigInt literal",
                ));
            }
            let digits = std::str::from_utf8(&self.src[start..self.pos])
                .unwrap()
                .replace('_', "");
            self.pos += 1;
            return Ok(Token {
                kind: TokenKind::BigInt(digits, NumberKind::Decimal),
                span: Span::new(start, self.pos),
                preceded_by_line_terminator: preceded_by_lt,
            });
        }

        if self.peek_is_ident_start_strict() || self.peek_byte() == Some(b'\\') {
            return Err(self.err(
                LexErrorKind::InvalidNumeric,
                start,
                "identifier directly after numeric literal",
            ));
        }
        let lexeme = std::str::from_utf8(&self.src[start..self.pos])
            .unwrap()
            .replace('_', "");

        let is_legacy_octal = first == b'0'
            && !self.strict_mode
            && !has_dot
            && !has_exp
            && lexeme.len() >= 2
            && lexeme.as_bytes()[1..]
                .iter()
                .all(|b| (b'0'..=b'7').contains(b));
        let value: f64 = if is_legacy_octal {
            let mut v = 0f64;
            for &b in &lexeme.as_bytes()[1..] {
                v = v * 8.0 + (b - b'0') as f64;
            }
            v
        } else {
            lexeme.parse().map_err(|_| {
                self.err(
                    LexErrorKind::InvalidNumeric,
                    start,
                    "malformed numeric literal",
                )
            })?
        };
        Ok(Token {
            kind: TokenKind::Number(value, NumberKind::Decimal),
            span: Span::new(start, self.pos),
            preceded_by_line_terminator: preceded_by_lt,
        })
    }

    fn read_radix_int<F: Fn(u8) -> bool>(
        &mut self,
        start: usize,
        preceded_by_lt: bool,
        kind: NumberKind,
        radix: u32,
        is_digit: F,
    ) -> Result<Token, LexError> {
        self.pos += 2;
        let digits_start = self.pos;
        let mut last_was_underscore = false;
        let mut has_digits = false;
        while let Some(c) = self.peek_byte() {
            if c == b'_' {
                if last_was_underscore || !has_digits {
                    return Err(self.err(
                        LexErrorKind::InvalidNumeric,
                        start,
                        "invalid numeric separator",
                    ));
                }
                last_was_underscore = true;
                self.pos += 1;
            } else if is_digit(c) {
                has_digits = true;
                last_was_underscore = false;
                self.pos += 1;
            } else {
                break;
            }
        }
        if !has_digits || last_was_underscore {
            return Err(self.err(
                LexErrorKind::InvalidNumeric,
                start,
                "invalid radix-prefixed literal",
            ));
        }

        if let Some(b) = self.peek_byte() {
            let high_lt_or_ws = b >= 0x80
                && (self.peek_lt_bytes().is_some()
                    || self.peek_codepoint().map_or(false, is_unicode_whitespace));

            if b != b'n'
                && !high_lt_or_ws
                && (b.is_ascii_digit() || is_identifier_start_byte(b) || b == b'\\')
            {
                return Err(self.err(
                    LexErrorKind::InvalidNumeric,
                    start,
                    "invalid character after radix-prefixed literal",
                ));
            }
        }
        let digits = std::str::from_utf8(&self.src[digits_start..self.pos])
            .unwrap()
            .replace('_', "");

        if self.peek_byte() == Some(b'n') {
            self.pos += 1;
            return Ok(Token {
                kind: TokenKind::BigInt(digits, kind),
                span: Span::new(start, self.pos),
                preceded_by_line_terminator: preceded_by_lt,
            });
        }
        let value = u128::from_str_radix(&digits, radix).map_err(|_| {
            self.err(
                LexErrorKind::InvalidNumeric,
                start,
                "out-of-range radix-prefixed literal",
            )
        })?;
        Ok(Token {
            kind: TokenKind::Number(value as f64, kind),
            span: Span::new(start, self.pos),
            preceded_by_line_terminator: preceded_by_lt,
        })
    }

    fn read_string_literal(
        &mut self,
        start: usize,
        preceded_by_lt: bool,
        quote: u8,
    ) -> Result<Token, LexError> {
        self.pos += 1;
        self.last_string_had_legacy_escape = false;
        let mut out: Vec<u16> = Vec::new();
        {
            let _profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::LexerStringScan,
            );
            loop {
                let Some(c) = self.peek_byte() else {
                    return Err(self.err(
                        LexErrorKind::UnterminatedString,
                        start,
                        "unterminated string",
                    ));
                };
                if c == quote {
                    self.pos += 1;
                    break;
                }

                if is_line_terminator_byte(c) {
                    return Err(self.err(
                        LexErrorKind::UnterminatedString,
                        start,
                        "line terminator in string",
                    ));
                }
                if c == b'\\' {
                    let _profile = crate::parser::parse_profile::Guard::new(
                        crate::parser::parse_profile::Kind::LexerStringEscape,
                    );
                    self.pos += 1;
                    self.read_string_escape(start, &mut out)?;
                    continue;
                }
                {
                    let _profile = crate::parser::parse_profile::Guard::new(
                        crate::parser::parse_profile::Kind::LexerStringNoEscape,
                    );
                    if c < 0x80 {
                        let _profile = crate::parser::parse_profile::Guard::new(
                            crate::parser::parse_profile::Kind::LexerStringNoEscapeAscii,
                        );
                        out.push(c as u16);
                        self.pos += 1;
                        continue;
                    }
                    {
                        let _profile = crate::parser::parse_profile::Guard::new(
                            crate::parser::parse_profile::Kind::LexerStringNoEscapeNonAscii,
                        );
                    }

                    let cp = {
                        let _profile = crate::parser::parse_profile::Guard::new(
                            crate::parser::parse_profile::Kind::LexerStringNoEscapeDecode,
                        );
                        self.peek_codepoint().ok_or_else(|| {
                            self.err(LexErrorKind::UnterminatedString, start, "malformed UTF-8")
                        })?
                    };
                    let raw_surrogate_unit = {
                        let _profile = crate::parser::parse_profile::Guard::new(
                            crate::parser::parse_profile::Kind::LexerStringNoEscapeMarker,
                        );
                        raw_surrogate_marker_to_unit(cp)
                    };
                    {
                        let _profile = crate::parser::parse_profile::Guard::new(
                            crate::parser::parse_profile::Kind::LexerStringNoEscapePush,
                        );
                        if let Some(unit) = raw_surrogate_unit {
                            out.push(unit);
                        } else {
                            push_cp_u16(&mut out, cp);
                        }
                    }
                    {
                        let _profile = crate::parser::parse_profile::Guard::new(
                            crate::parser::parse_profile::Kind::LexerStringNoEscapeAdvance,
                        );
                        let len = utf8_len(c);
                        self.pos += len;
                    }
                }
            }
        }

        let kind = {
            let _profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::LexerStringConvert,
            );
            match String::from_utf16(&out) {
                Ok(text) => TokenKind::String(text),
                Err(_) => TokenKind::WtfString(out),
            }
        };
        let token = {
            let _profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::LexerStringToken,
            );
            Token {
                kind,
                span: Span::new(start, self.pos),
                preceded_by_line_terminator: preceded_by_lt,
            }
        };
        Ok(token)
    }

    fn read_string_escape(&mut self, start: usize, out: &mut Vec<u16>) -> Result<(), LexError> {
        let Some(c) = self.peek_byte() else {
            return Err(self.err(LexErrorKind::InvalidEscape, start, "lone backslash"));
        };
        if c == b'\n' {
            self.pos += 1;
            return Ok(());
        }
        if c == b'\r' {
            self.pos += if self.peek_byte_at(1) == Some(b'\n') {
                2
            } else {
                1
            };
            return Ok(());
        }

        if c == 0xE2 && self.peek_byte_at(1) == Some(0x80) {
            if let Some(b3) = self.peek_byte_at(2) {
                if b3 == 0xA8 || b3 == 0xA9 {
                    self.pos += 3;
                    return Ok(());
                }
            }
        }

        if c >= 0x80 {
            if let Some(cp) = self.peek_codepoint() {
                if let Some(ch) = char::from_u32(cp) {
                    self.pos += ch.len_utf8();
                    push_cp_u16(out, ch as u32);
                    return Ok(());
                }
            }
        }
        self.pos += 1;
        match c {
            b'n' => out.push('\n' as u16),
            b'r' => out.push('\r' as u16),
            b't' => out.push('\t' as u16),
            b'b' => out.push('\u{0008}' as u16),
            b'f' => out.push('\u{000C}' as u16),
            b'v' => out.push('\u{000B}' as u16),
            b'0' => {

                if self.peek_byte().map_or(false, |b| b.is_ascii_digit()) {
                    if self.strict_mode {
                        return Err(self.err(
                            LexErrorKind::InvalidEscape,
                            start,
                            "legacy octal escape sequence in strict mode",
                        ));
                    }
                    self.last_string_had_legacy_escape = true;
                    let mut v: u32 = 0;
                    let mut n = 0;
                    while n < 2 {
                        match self.peek_byte() {
                            Some(b) if (b'0'..=b'7').contains(&b) => {
                                v = v * 8 + (b - b'0') as u32;
                                self.pos += 1;
                                n += 1;
                            }
                            _ => break,
                        }
                    }
                    push_cp_u16(out, v);
                } else {
                    out.push('\0' as u16);
                }
            }
            b'\'' | b'"' | b'\\' => out.push(c as u16),
            b'x' => {
                let hi = self
                    .peek_byte()
                    .and_then(|b| hex_digit_value(b))
                    .ok_or_else(|| {
                        self.err(LexErrorKind::InvalidEscape, start, "bad \\x escape")
                    })?;
                self.pos += 1;
                let lo = self
                    .peek_byte()
                    .and_then(|b| hex_digit_value(b))
                    .ok_or_else(|| {
                        self.err(LexErrorKind::InvalidEscape, start, "bad \\x escape")
                    })?;
                self.pos += 1;
                let cp = (hi * 16 + lo) as u32;
                push_cp_u16(out, cp);
            }
            b'u' => {
                let cp = self.read_unicode_escape_inner().ok_or_else(|| {
                    self.err(LexErrorKind::InvalidEscape, start, "bad \\u escape")
                })?;
                if (0xD800..=0xDBFF).contains(&cp)
                    && self.peek_byte() == Some(b'\\')
                    && self.peek_byte_at(1) == Some(b'u')
                {
                    let save = self.pos;
                    self.pos += 2;
                    if let Some(low) = self.read_unicode_escape_inner() {
                        if (0xDC00..=0xDFFF).contains(&low) {
                            let scalar = 0x10000 + ((cp - 0xD800) << 10) + (low - 0xDC00);
                            push_cp_u16(out, scalar);
                            return Ok(());
                        }
                    }
                    self.pos = save;
                }
                push_cp_u16(out, cp);
            }
            b'\n' => {   }
            b'\r' => {
                if self.peek_byte() == Some(b'\n') {
                    self.pos += 1;
                }
            }

            b'1'..=b'7' => {
                if self.strict_mode {
                    return Err(self.err(
                        LexErrorKind::InvalidEscape,
                        start,
                        "legacy octal escape sequence in strict mode",
                    ));
                }
                self.last_string_had_legacy_escape = true;
                let mut v: u32 = (c - b'0') as u32;
                let leading_four_to_seven = matches!(c, b'4'..=b'7');
                let max_extra_digits = if leading_four_to_seven { 1 } else { 2 };
                let mut n = 0;
                while n < max_extra_digits {
                    match self.peek_byte() {
                        Some(b) if (b'0'..=b'7').contains(&b) => {
                            v = v * 8 + (b - b'0') as u32;
                            self.pos += 1;
                            n += 1;
                        }
                        _ => break,
                    }
                }
                push_cp_u16(out, v);
            }

            b'8' | b'9' => {
                if self.strict_mode {
                    return Err(self.err(
                        LexErrorKind::InvalidEscape,
                        start,
                        "legacy non-octal decimal escape sequence in strict mode",
                    ));
                }
                self.last_string_had_legacy_escape = true;
                out.push(c as u16);
            }
            _ => out.push(c as u16),
        }
        Ok(())
    }

    fn read_template_segment(
        &mut self,
        start: usize,
        preceded_by_lt: bool,
        is_open: bool,
    ) -> Result<Token, LexError> {
        let mut cooked = String::new();
        let mut raw = String::new();
        let mut cooked_ok = true;
        loop {
            let Some(c) = self.peek_byte() else {
                return Err(self.err(
                    LexErrorKind::UnterminatedTemplate,
                    start,
                    "unterminated template",
                ));
            };
            if c == b'`' {
                self.pos += 1;
                return Ok(Token {
                    kind: TokenKind::Template {
                        cooked: if cooked_ok { Some(cooked) } else { None },
                        raw,
                        part: if is_open {
                            TemplatePart::NoSubstitution
                        } else {
                            TemplatePart::Tail
                        },
                    },
                    span: Span::new(start, self.pos),
                    preceded_by_line_terminator: preceded_by_lt,
                });
            }
            if c == b'$' && self.peek_byte_at(1) == Some(b'{') {
                self.pos += 2;
                return Ok(Token {
                    kind: TokenKind::Template {
                        cooked: if cooked_ok { Some(cooked) } else { None },
                        raw,
                        part: if is_open {
                            TemplatePart::Head
                        } else {
                            TemplatePart::Middle
                        },
                    },
                    span: Span::new(start, self.pos),
                    preceded_by_line_terminator: preceded_by_lt,
                });
            }
            if c == b'\\' {

                let escape_start = self.pos;
                let template_decimal_escape_invalid_cooked = match self.peek_byte_at(1) {
                    Some(b'0') => self.peek_byte_at(2).map_or(false, |b| b.is_ascii_digit()),
                    Some(b'1'..=b'9') => true,
                    _ => false,
                };
                self.pos += 1;
                if self.consume_forbidden_template_numeric_escape() {
                    cooked_ok = false;
                    push_template_raw_escape(&mut raw, &self.src[escape_start..self.pos]);
                    continue;
                }
                let mut buf: Vec<u16> = Vec::new();
                match self.read_string_escape(start, &mut buf) {
                    Ok(()) => push_template_cooked_units(&mut cooked, &buf),
                    Err(_) => cooked_ok = false,
                }
                if template_decimal_escape_invalid_cooked {
                    cooked_ok = false;
                }
                push_template_raw_escape(&mut raw, &self.src[escape_start..self.pos]);
                continue;
            }
            if c == b'\r' {

                cooked.push('\n');
                raw.push('\n');
                self.pos += 1;
                if self.peek_byte() == Some(b'\n') {
                    self.pos += 1;
                }
                continue;
            }
            let cp = self.peek_codepoint().ok_or_else(|| {
                self.err(LexErrorKind::UnterminatedTemplate, start, "malformed UTF-8")
            })?;
            push_template_char(&mut cooked, cp);
            push_char(&mut raw, cp);
            let len = utf8_len(c);
            self.pos += len;
        }
    }

    fn continue_template(&mut self, start: usize, preceded_by_lt: bool) -> Result<Token, LexError> {
        self.read_template_segment(start, preceded_by_lt, false)
    }

    fn consume_forbidden_template_numeric_escape(&mut self) -> bool {
        let Some(c) = self.peek_byte() else {
            return false;
        };
        match c {
            b'0' if self.peek_byte_at(1).map_or(false, |b| b.is_ascii_digit()) => {
                self.pos += 1;
                let mut n = 0;
                while n < 2 {
                    match self.peek_byte() {
                        Some(b) if (b'0'..=b'7').contains(&b) => {
                            self.pos += 1;
                            n += 1;
                        }
                        _ => break,
                    }
                }
                true
            }
            b'1'..=b'7' => {
                self.pos += 1;
                let leading_four_to_seven = matches!(c, b'4'..=b'7');
                let max_extra_digits = if leading_four_to_seven { 1 } else { 2 };
                let mut n = 0;
                while n < max_extra_digits {
                    match self.peek_byte() {
                        Some(b) if (b'0'..=b'7').contains(&b) => {
                            self.pos += 1;
                            n += 1;
                        }
                        _ => break,
                    }
                }
                true
            }
            b'8' | b'9' => {
                self.pos += 1;
                true
            }
            _ => false,
        }
    }

    fn read_regex_literal(
        &mut self,
        start: usize,
        preceded_by_lt: bool,
    ) -> Result<Token, LexError> {
        self.pos += 1;
        let body_start = self.pos;
        let mut in_class = false;
        loop {
            let Some(c) = self.peek_byte() else {
                return Err(self.err(LexErrorKind::UnterminatedRegex, start, "unterminated regex"));
            };
            if is_line_terminator_byte(c) || self.peek_lt_bytes().is_some() {
                return Err(self.err(
                    LexErrorKind::UnterminatedRegex,
                    start,
                    "line terminator in regex",
                ));
            }
            if c == b'\\' {
                self.pos += 1;
                if self
                    .peek_byte()
                    .map_or(true, |b| is_line_terminator_byte(b))
                    || self.peek_lt_bytes().is_some()
                {
                    return Err(self.err(
                        LexErrorKind::UnterminatedRegex,
                        start,
                        "bad escape in regex",
                    ));
                }
                self.advance_one_char();
                continue;
            }
            if c == b'[' {
                in_class = true;
                self.pos += 1;
                continue;
            }
            if c == b']' {
                in_class = false;
                self.pos += 1;
                continue;
            }
            if c == b'/' && !in_class {
                let body = std::str::from_utf8(&self.src[body_start..self.pos])
                    .map_err(|_| {
                        self.err(LexErrorKind::UnterminatedRegex, start, "malformed UTF-8")
                    })?
                    .to_string();
                self.pos += 1;

                let flags_start = self.pos;
                while let Some(c) = self.peek_byte() {
                    if c >= 0x80 {
                        if self.peek_lt_bytes().is_some() {
                            break;
                        }
                        let Some(cp) = self.peek_codepoint() else {
                            return Err(self.err(
                                LexErrorKind::InvalidIdentifier,
                                self.pos,
                                "malformed UTF-8 in regex flags",
                            ));
                        };
                        if is_unicode_whitespace(cp) || !is_id_continue(cp) {
                            break;
                        }
                        self.advance_one_char();
                    } else if is_identifier_part_byte(c) {
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
                let flags = std::str::from_utf8(&self.src[flags_start..self.pos])
                    .map_err(|_| {
                        self.err(
                            LexErrorKind::InvalidIdentifier,
                            flags_start,
                            "malformed UTF-8 in regex flags",
                        )
                    })?
                    .to_string();
                return Ok(Token {
                    kind: TokenKind::Regex { body, flags },
                    span: Span::new(start, self.pos),
                    preceded_by_line_terminator: preceded_by_lt,
                });
            }
            self.advance_one_char();
        }
    }

    fn read_punctuator(&mut self, start: usize, preceded_by_lt: bool) -> Result<Token, LexError> {

        macro_rules! emit {
            ($p:expr, $len:expr) => {{
                self.pos += $len;
                Ok(Token {
                    kind: TokenKind::Punct($p),
                    span: Span::new(start, self.pos),
                    preceded_by_line_terminator: preceded_by_lt,
                })
            }};
        }

        let s0 = self.src[self.pos];
        let s1 = self.peek_byte_at(1);
        let s2 = self.peek_byte_at(2);
        let s3 = self.peek_byte_at(3);

        if s0 == b'>' && s1 == Some(b'>') && s2 == Some(b'>') && s3 == Some(b'=') {
            return emit!(Punct::UShrAssign, 4);
        }

        if s0 == b'.' && s1 == Some(b'.') && s2 == Some(b'.') {
            return emit!(Punct::Spread, 3);
        }
        if s0 == b'=' && s1 == Some(b'=') && s2 == Some(b'=') {
            return emit!(Punct::StrictEq, 3);
        }
        if s0 == b'!' && s1 == Some(b'=') && s2 == Some(b'=') {
            return emit!(Punct::StrictNe, 3);
        }
        if s0 == b'*' && s1 == Some(b'*') && s2 == Some(b'=') {
            return emit!(Punct::StarStarAssign, 3);
        }
        if s0 == b'<' && s1 == Some(b'<') && s2 == Some(b'=') {
            return emit!(Punct::ShlAssign, 3);
        }
        if s0 == b'>' && s1 == Some(b'>') && s2 == Some(b'=') {
            return emit!(Punct::ShrAssign, 3);
        }
        if s0 == b'>' && s1 == Some(b'>') && s2 == Some(b'>') {
            return emit!(Punct::UShr, 3);
        }
        if s0 == b'&' && s1 == Some(b'&') && s2 == Some(b'=') {
            return emit!(Punct::LogicalAndAssign, 3);
        }
        if s0 == b'|' && s1 == Some(b'|') && s2 == Some(b'=') {
            return emit!(Punct::LogicalOrAssign, 3);
        }
        if s0 == b'?' && s1 == Some(b'?') && s2 == Some(b'=') {
            return emit!(Punct::NullishAssign, 3);
        }

        let two = (s0, s1);
        match two {
            (b'=', Some(b'>')) => return emit!(Punct::Arrow, 2),
            (b'?', Some(b'.')) => {

                if s2.map_or(true, |b| !b.is_ascii_digit()) {
                    return emit!(Punct::OptionalChain, 2);
                }
            }
            (b'=', Some(b'=')) => return emit!(Punct::Eq, 2),
            (b'!', Some(b'=')) => return emit!(Punct::Ne, 2),
            (b'<', Some(b'=')) => return emit!(Punct::Le, 2),
            (b'>', Some(b'=')) => return emit!(Punct::Ge, 2),
            (b'+', Some(b'+')) => return emit!(Punct::Inc, 2),
            (b'-', Some(b'-')) => return emit!(Punct::Dec, 2),
            (b'*', Some(b'*')) => return emit!(Punct::StarStar, 2),
            (b'<', Some(b'<')) => return emit!(Punct::Shl, 2),
            (b'>', Some(b'>')) => return emit!(Punct::Shr, 2),
            (b'&', Some(b'&')) => return emit!(Punct::LogicalAnd, 2),
            (b'|', Some(b'|')) => return emit!(Punct::LogicalOr, 2),
            (b'?', Some(b'?')) => return emit!(Punct::NullishCoalesce, 2),
            (b'+', Some(b'=')) => return emit!(Punct::PlusAssign, 2),
            (b'-', Some(b'=')) => return emit!(Punct::MinusAssign, 2),
            (b'*', Some(b'=')) => return emit!(Punct::StarAssign, 2),
            (b'%', Some(b'=')) => return emit!(Punct::PercentAssign, 2),
            (b'/', Some(b'=')) => return emit!(Punct::SlashAssign, 2),
            (b'&', Some(b'=')) => return emit!(Punct::BitAndAssign, 2),
            (b'|', Some(b'=')) => return emit!(Punct::BitOrAssign, 2),
            (b'^', Some(b'=')) => return emit!(Punct::BitXorAssign, 2),
            _ => {}
        }

        let p = match s0 {
            b'{' => Punct::LBrace,
            b'}' => Punct::RBrace,
            b'(' => Punct::LParen,
            b')' => Punct::RParen,
            b'[' => Punct::LBracket,
            b']' => Punct::RBracket,
            b';' => Punct::Semicolon,
            b',' => Punct::Comma,
            b':' => Punct::Colon,
            b'@' => Punct::At,
            b'.' => Punct::Dot,
            b'<' => Punct::Lt,
            b'>' => Punct::Gt,
            b'+' => Punct::Plus,
            b'-' => Punct::Minus,
            b'*' => Punct::Star,
            b'%' => Punct::Percent,
            b'/' => Punct::Slash,
            b'&' => Punct::BitAnd,
            b'|' => Punct::BitOr,
            b'^' => Punct::BitXor,
            b'~' => Punct::BitNot,
            b'!' => Punct::LogicalNot,
            b'?' => Punct::Question,
            b'=' => Punct::Assign,
            _ => return Err(self.err(LexErrorKind::UnexpectedChar, start, "unexpected character")),
        };
        emit!(p, 1)
    }

    fn err(&self, kind: LexErrorKind, start: usize, message: &'static str) -> LexError {
        LexError {
            kind,
            span: Span::new(start, self.pos.max(start + 1)),
            message,
        }
    }

    fn peek_byte(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }
    fn peek_byte_at(&self, off: usize) -> Option<u8> {
        self.src.get(self.pos + off).copied()
    }
    fn peek_str(&self, len: usize) -> Option<&str> {
        let end = self.pos.checked_add(len)?;
        std::str::from_utf8(self.src.get(self.pos..end)?).ok()
    }
    fn peek_codepoint(&self) -> Option<u32> {
        let b0 = *self.src.get(self.pos)?;
        if b0 < 0x80 {
            return Some(b0 as u32);
        }
        if b0 < 0xC0 {
            return None;
        }
        if b0 < 0xE0 {
            let b1 = *self.src.get(self.pos + 1)?;
            if b1 & 0xC0 != 0x80 {
                return None;
            }
            return Some((((b0 & 0x1F) as u32) << 6) | ((b1 & 0x3F) as u32));
        }
        if b0 < 0xF0 {
            let b1 = *self.src.get(self.pos + 1)?;
            let b2 = *self.src.get(self.pos + 2)?;
            if (b1 & 0xC0 != 0x80) || (b2 & 0xC0 != 0x80) {
                return None;
            }
            return Some(
                (((b0 & 0x0F) as u32) << 12) | (((b1 & 0x3F) as u32) << 6) | ((b2 & 0x3F) as u32),
            );
        }
        if b0 < 0xF8 {
            let b1 = *self.src.get(self.pos + 1)?;
            let b2 = *self.src.get(self.pos + 2)?;
            let b3 = *self.src.get(self.pos + 3)?;
            if (b1 & 0xC0 != 0x80) || (b2 & 0xC0 != 0x80) || (b3 & 0xC0 != 0x80) {
                return None;
            }
            return Some(
                (((b0 & 0x07) as u32) << 18)
                    | (((b1 & 0x3F) as u32) << 12)
                    | (((b2 & 0x3F) as u32) << 6)
                    | ((b3 & 0x3F) as u32),
            );
        }
        None
    }
    fn advance_one_char(&mut self) {
        if let Some(b) = self.peek_byte() {
            self.pos += utf8_len(b);
        }
    }
}

fn utf8_len(b0: u8) -> usize {
    if b0 < 0x80 {
        1
    } else if b0 < 0xC0 {
        1
    }

    else if b0 < 0xE0 {
        2
    } else if b0 < 0xF0 {
        3
    } else {
        4
    }
}

fn push_cp_u16(out: &mut Vec<u16>, cp: u32) {
    if let Some(c) = char::from_u32(cp) {
        let mut buf = [0u16; 2];
        out.extend_from_slice(c.encode_utf16(&mut buf));
    } else {
        out.push(cp as u16);
    }
}

fn push_char(out: &mut String, cp: u32) {
    if let Some(c) = char::from_u32(cp) {
        out.push(c);
    }

    else {
        out.push('\u{FFFD}');
    }
}

fn push_template_char(out: &mut String, cp: u32) {
    if let Some(unit) = raw_surrogate_marker_to_unit(cp) {
        out.push_str(&String::from_utf16_lossy(&[unit]));
    } else {
        push_char(out, cp);
    }
}

fn push_template_cooked_units(out: &mut String, units: &[u16]) {
    let mut i = 0usize;
    while i < units.len() {
        let unit = units[i];
        if (0xD800..=0xDBFF).contains(&unit) {
            if let Some(next) = units.get(i + 1).copied() {
                if (0xDC00..=0xDFFF).contains(&next) {
                    let hi = (unit as u32) - 0xD800;
                    let lo = (next as u32) - 0xDC00;
                    if let Some(ch) = char::from_u32(0x10000 + ((hi << 10) | lo)) {
                        out.push(ch);
                        i += 2;
                        continue;
                    }
                }
            }
            push_raw_surrogate_marker(out, unit);
            i += 1;
            continue;
        }
        if (0xDC00..=0xDFFF).contains(&unit) {
            push_raw_surrogate_marker(out, unit);
            i += 1;
            continue;
        }
        if let Some(ch) = char::from_u32(unit as u32) {
            out.push(ch);
        }
        i += 1;
    }
}

fn push_raw_surrogate_marker(out: &mut String, unit: u16) {
    debug_assert!((0xD800..=0xDFFF).contains(&unit));
    let cp = RAW_SURROGATE_MARKER_BASE + (unit as u32 - 0xD800);
    if let Some(ch) = char::from_u32(cp) {
        out.push(ch);
    }
}

fn hex_digit_value(b: u8) -> Option<u32> {
    match b {
        b'0'..=b'9' => Some((b - b'0') as u32),
        b'a'..=b'f' => Some((b - b'a' + 10) as u32),
        b'A'..=b'F' => Some((b - b'A' + 10) as u32),
        _ => None,
    }
}

fn is_whitespace_byte(b: u8) -> bool {
    matches!(b, 0x09 | 0x0B | 0x0C | 0x20)
}

fn is_unicode_whitespace(cp: u32) -> bool {
    matches!(
        cp,
        0x00A0 | 0x1680 | 0x2000..=0x200A | 0x202F | 0x205F | 0x3000 | 0xFEFF
    )
}

fn is_line_terminator_byte(b: u8) -> bool {
    matches!(b, 0x0A | 0x0D)
}

fn is_identifier_start_byte(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b == b'$' || b >= 0x80
}

fn is_identifier_part_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b >= 0x80
}

fn push_template_raw_escape(out: &mut String, bytes: &[u8]) {
    match bytes {
        [b'\\', b'\r', b'\n'] | [b'\\', b'\r'] | [b'\\', b'\n'] => out.push_str("\\\n"),
        _ => out.push_str(std::str::from_utf8(bytes).unwrap()),
    }
}

pub(crate) fn is_id_start(cp: u32) -> bool {
    rusty_js_unicode_ident::is_id_start(cp)
}

pub(crate) fn is_id_continue(cp: u32) -> bool {
    rusty_js_unicode_ident::is_id_continue(cp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_literal_decodes_eval_source_surrogate_marker() {
        let mut lexer = Lexer::new("\"\u{F0438}\"");
        let token = lexer.next_token(LexerGoal::RegExp).expect("token");
        match token.kind {
            TokenKind::WtfString(units) => assert_eq!(units, vec![0xDC38]),
            other => panic!("expected WtfString, got {other:?}"),
        }
    }

    #[test]
    fn template_escape_cooks_lone_surrogate_as_internal_marker() {
        let mut lexer = Lexer::new("`\\uDC38`");
        let token = lexer.next_token(LexerGoal::RegExp).expect("token");
        match token.kind {
            TokenKind::Template { cooked, .. } => {
                assert_eq!(cooked.as_deref(), Some("\u{F0438}"));
            }
            other => panic!("expected Template, got {other:?}"),
        }
    }

    #[test]
    fn regex_literal_flags_advance_non_ascii_by_codepoint() {
        let mut lexer = Lexer::new("/x/é;");
        let token = lexer.next_token(LexerGoal::RegExp).expect("token");
        match token.kind {
            TokenKind::Regex { body, flags } => {
                assert_eq!(body, "x");
                assert_eq!(flags, "é");
                assert_eq!(token.span.end, "/x/é".len());
            }
            other => panic!("expected Regex, got {other:?}"),
        }

        let semi = lexer.next_token(LexerGoal::Div).expect("semicolon");
        assert!(matches!(semi.kind, TokenKind::Punct(Punct::Semicolon)));
    }
}
