
use crate::abstract_ops;
use crate::interp::{Runtime, RuntimeError};
use crate::intrinsics::{make_native, make_native_non_ctor};
use crate::value::JsString;
use crate::value::{
    regexp_result_slot_counter_snapshot, CompiledRegex, InternalKind, Object, ObjectRef,
    PropertyDescriptor, RegExpInternals, RegExpResultSlots, Value,
};
use std::rc::Rc;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    OnceLock,
};

static REGEXP_RESULT_SUCCESSES: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_ARRAYS: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_DENSE_SLOTS: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_SUBSTRINGS: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_SUBSTRING_BYTES: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_MATCH_SUBSTRINGS: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_MATCH_SUBSTRING_BYTES: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_CAPTURE_SUBSTRINGS: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_CAPTURE_SUBSTRING_BYTES: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_SLICE_STRINGS: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_OWNED_STRINGS: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_UNDEFINED_SLOTS: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_NAMED_GROUP_STRINGS: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_NAMED_GROUP_BYTES: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_INDICES_ARRAYS: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_INPUT_REUSED: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_INPUT_FALLBACK_CLONES: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_LEGACY_CAPTURE_MOVES: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_LEGACY_CAPTURE_CLONES: AtomicU64 = AtomicU64::new(0);
static REGEXP_EXEC_CALLS: AtomicU64 = AtomicU64::new(0);
static REGEXP_EXEC_NULL_RESULTS: AtomicU64 = AtomicU64::new(0);
static REGEXP_LAST_INDEX_FAST_NUMBERS: AtomicU64 = AtomicU64::new(0);
static REGEXP_LAST_INDEX_FALLBACKS: AtomicU64 = AtomicU64::new(0);
static REGEXP_LAST_INDEX_SUCCESS_STOREBACKS: AtomicU64 = AtomicU64::new(0);
static REGEXP_LAST_INDEX_NULL_RESETS: AtomicU64 = AtomicU64::new(0);

static REGEXP_TIME_MATCHER_NS: AtomicU64 = AtomicU64::new(0);
static REGEXP_TIME_RESULT_NS: AtomicU64 = AtomicU64::new(0);
static REGEXP_IHI_EXEC_FAST_CALLS: AtomicU64 = AtomicU64::new(0);

fn regexp_exec_time_counters_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_REGEXP_EXEC_TIME_COUNTERS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn regexp_result_counters_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_REGEXP_RESULT_COUNTERS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn regexp_result_counter_report_every() -> u64 {
    static EVERY: OnceLock<u64> = OnceLock::new();
    *EVERY.get_or_init(|| {
        std::env::var("CRUFT_REGEXP_RESULT_COUNTERS_EVERY")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(100_000)
    })
}

fn regexp_result_slices_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_REGEXP_RESULT_SLICES")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(true)
    })
}

fn regexp_result_property_capacity_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_REGEXP_RESULT_PROP_CAP")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(true)
    })
}

fn regexp_result_lazy_slots_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_REGEXP_RESULT_LAZY_SLOTS")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(true)
    })
}

fn regexp_legacy_capture_move_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_REGEXP_LEGACY_CAPTURE_MOVE")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(true)
    })
}

fn maybe_report_regexp_result_counters(successes: u64) {
    if !regexp_result_counters_enabled() {
        return;
    }
    let every = regexp_result_counter_report_every();
    if successes % every != 0 {
        return;
    }
    let (
        lazy_index_reads,
        lazy_string_materializations,
        lazy_undefined_reads,
        lazy_full_materializations,
        lazy_full_materialized_slots,
        lazy_number_direct_reads,
    ) = regexp_result_slot_counter_snapshot();
    eprintln!(
        "[regexp-result-counters] successes={} exec_calls={} null_results={} last_index_fast_numbers={} last_index_fallbacks={} last_index_success_storebacks={} last_index_null_resets={} result_arrays={} dense_slots={} substrings={} substring_bytes={} match_substrings={} match_substring_bytes={} capture_substrings={} capture_substring_bytes={} slice_strings={} owned_strings={} undefined_slots={} named_group_strings={} named_group_bytes={} indices_arrays={} input_reused={} input_fallback_clones={} legacy_capture_moves={} legacy_capture_clones={} lazy_index_reads={} lazy_string_materializations={} lazy_undefined_reads={} lazy_full_materializations={} lazy_full_materialized_slots={} lazy_number_direct_reads={} ihi_exec_fast_calls={} matcher_ms={:.1} result_build_ms={:.1}",
        successes,
        REGEXP_EXEC_CALLS.load(Ordering::Relaxed),
        REGEXP_EXEC_NULL_RESULTS.load(Ordering::Relaxed),
        REGEXP_LAST_INDEX_FAST_NUMBERS.load(Ordering::Relaxed),
        REGEXP_LAST_INDEX_FALLBACKS.load(Ordering::Relaxed),
        REGEXP_LAST_INDEX_SUCCESS_STOREBACKS.load(Ordering::Relaxed),
        REGEXP_LAST_INDEX_NULL_RESETS.load(Ordering::Relaxed),
        REGEXP_RESULT_ARRAYS.load(Ordering::Relaxed),
        REGEXP_RESULT_DENSE_SLOTS.load(Ordering::Relaxed),
        REGEXP_RESULT_SUBSTRINGS.load(Ordering::Relaxed),
        REGEXP_RESULT_SUBSTRING_BYTES.load(Ordering::Relaxed),
        REGEXP_RESULT_MATCH_SUBSTRINGS.load(Ordering::Relaxed),
        REGEXP_RESULT_MATCH_SUBSTRING_BYTES.load(Ordering::Relaxed),
        REGEXP_RESULT_CAPTURE_SUBSTRINGS.load(Ordering::Relaxed),
        REGEXP_RESULT_CAPTURE_SUBSTRING_BYTES.load(Ordering::Relaxed),
        REGEXP_RESULT_SLICE_STRINGS.load(Ordering::Relaxed),
        REGEXP_RESULT_OWNED_STRINGS.load(Ordering::Relaxed),
        REGEXP_RESULT_UNDEFINED_SLOTS.load(Ordering::Relaxed),
        REGEXP_RESULT_NAMED_GROUP_STRINGS.load(Ordering::Relaxed),
        REGEXP_RESULT_NAMED_GROUP_BYTES.load(Ordering::Relaxed),
        REGEXP_RESULT_INDICES_ARRAYS.load(Ordering::Relaxed),
        REGEXP_RESULT_INPUT_REUSED.load(Ordering::Relaxed),
        REGEXP_RESULT_INPUT_FALLBACK_CLONES.load(Ordering::Relaxed),
        REGEXP_RESULT_LEGACY_CAPTURE_MOVES.load(Ordering::Relaxed),
        REGEXP_RESULT_LEGACY_CAPTURE_CLONES.load(Ordering::Relaxed),
        lazy_index_reads,
        lazy_string_materializations,
        lazy_undefined_reads,
        lazy_full_materializations,
        lazy_full_materialized_slots,
        lazy_number_direct_reads,
        REGEXP_IHI_EXEC_FAST_CALLS.load(Ordering::Relaxed),
        REGEXP_TIME_MATCHER_NS.load(Ordering::Relaxed) as f64 / 1.0e6,
        REGEXP_TIME_RESULT_NS.load(Ordering::Relaxed) as f64 / 1.0e6
    );
}

#[derive(Clone, Debug)]
pub enum LegacyRegExpState {
    Materialized {
        input: String,
        last_match: String,
        last_paren: String,
        left_context: String,
        right_context: String,
        captures: Vec<String>,
    },
    LazyByte {
        input: Rc<JsString>,
        match_start: usize,
        match_end: usize,
        captures: Vec<Option<(usize, usize)>>,
        capture_offset: usize,
        last_paren: Option<(usize, usize)>,
    },
}

impl Default for LegacyRegExpState {
    fn default() -> Self {
        Self::Materialized {
            input: String::new(),
            last_match: String::new(),
            last_paren: String::new(),
            left_context: String::new(),
            right_context: String::new(),
            captures: Vec::new(),
        }
    }
}

impl LegacyRegExpState {
    fn materialized(
        input: String,
        last_match: String,
        last_paren: String,
        left_context: String,
        right_context: String,
        captures: Vec<String>,
    ) -> Self {
        Self::Materialized {
            input,
            last_match,
            last_paren,
            left_context,
            right_context,
            captures,
        }
    }

    fn lazy_byte(
        input: Rc<JsString>,
        match_start: usize,
        match_end: usize,
        captures: Vec<Option<(usize, usize)>>,
        last_paren: Option<(usize, usize)>,
    ) -> Self {
        Self::LazyByte {
            input,
            match_start,
            match_end,
            captures,
            capture_offset: 0,
            last_paren,
        }
    }

    fn lazy_byte_with_capture_offset(
        input: Rc<JsString>,
        match_start: usize,
        match_end: usize,
        captures: Vec<Option<(usize, usize)>>,
        capture_offset: usize,
        last_paren: Option<(usize, usize)>,
    ) -> Self {
        Self::LazyByte {
            input,
            match_start,
            match_end,
            captures,
            capture_offset,
            last_paren,
        }
    }

    fn capture(&self, i: usize) -> String {
        match self {
            Self::Materialized { captures, .. } => captures.get(i - 1).cloned().unwrap_or_default(),
            Self::LazyByte {
                input,
                captures,
                capture_offset,
                ..
            } => captures
                .get(capture_offset + i - 1)
                .and_then(|c| c.map(|(s, e)| input.as_str()[s..e].to_string()))
                .unwrap_or_default(),
        }
    }

    fn input(&self) -> String {
        match self {
            Self::Materialized { input, .. } => input.clone(),
            Self::LazyByte { input, .. } => input.as_str().to_string(),
        }
    }

    fn last_match(&self) -> String {
        match self {
            Self::Materialized { last_match, .. } => last_match.clone(),
            Self::LazyByte {
                input,
                match_start,
                match_end,
                ..
            } => input.as_str()[*match_start..*match_end].to_string(),
        }
    }

    fn last_paren(&self) -> String {
        match self {
            Self::Materialized { last_paren, .. } => last_paren.clone(),
            Self::LazyByte {
                input, last_paren, ..
            } => last_paren
                .map(|(s, e)| input.as_str()[s..e].to_string())
                .unwrap_or_default(),
        }
    }

    fn left_context(&self) -> String {
        match self {
            Self::Materialized { left_context, .. } => left_context.clone(),
            Self::LazyByte {
                input, match_start, ..
            } => input.as_str()[..*match_start].to_string(),
        }
    }

    fn right_context(&self) -> String {
        match self {
            Self::Materialized { right_context, .. } => right_context.clone(),
            Self::LazyByte {
                input, match_end, ..
            } => input.as_str()[*match_end..].to_string(),
        }
    }

    fn captures(&self) -> Vec<String> {
        match self {
            Self::Materialized { captures, .. } => captures.clone(),
            Self::LazyByte {
                input,
                captures,
                capture_offset,
                ..
            } => captures
                .iter()
                .skip(*capture_offset)
                .map(|c| {
                    c.map(|(s, e)| input.as_str()[s..e].to_string())
                        .unwrap_or_default()
                })
                .collect(),
        }
    }

    fn set_input(&mut self, input: String) {
        let last_match = self.last_match();
        let last_paren = self.last_paren();
        let left_context = self.left_context();
        let right_context = self.right_context();
        let captures = self.captures();
        *self = Self::materialized(
            input,
            last_match,
            last_paren,
            left_context,
            right_context,
            captures,
        );
    }
}

impl Runtime {

    pub fn install_regexp(&mut self) {

        let object_proto = self.object_prototype;
        let proto = self.alloc_object(Object {
            proto: object_proto,
            extensible: true,
            properties: indexmap::IndexMap::new(),

            internal_kind: InternalKind::Ordinary,
            ..Default::default()
        });
        self.regexp_prototype = Some(proto);
        self.realms[0].regexp_prototype = Some(proto);
        install_regexp_proto(self, proto);

        let crx_obj = make_native("__createRegExp", |rt, args| {
            let pattern =
                abstract_ops::to_js_string(&args.first().cloned().unwrap_or(Value::Undefined));
            let flags = abstract_ops::to_string(&args.get(1).cloned().unwrap_or(Value::Undefined))
                .as_str()
                .to_string();
            Ok(Value::Object(new_regexp_from_js_string(
                rt, &pattern, &flags, true,
            )?))
        });
        let crx_id = self.alloc_object(crx_obj);
        self.engine_helpers
            .insert("__createRegExp".into(), Value::Object(crx_id));

        register_global_native(self, "RegExp", |rt, args| {

            let first = args.first().cloned().unwrap_or(Value::Undefined);
            let flags_arg = args.get(1).cloned().unwrap_or(Value::Undefined);
            let new_target_root = rt.current_new_target.clone().unwrap_or(Value::Undefined);
            let _arg_roots = rt.push_temporary_value_roots(&[
                first.clone(),
                flags_arg.clone(),
                new_target_root.clone(),
            ]);
            let flags_is_undefined = matches!(flags_arg, Value::Undefined);
            let new_regexp_proto;
            let (pattern, flags, pattern_is_internal_raw) = match &first {
                Value::Object(id) => {
                    if rt.is_regexp_like_via(&first)? {
                        if rt.current_new_target.is_none() && flags_is_undefined {
                            let _constructor_roots = rt.push_temporary_value_roots(&[
                                first.clone(),
                                flags_arg.clone(),
                                new_target_root.clone(),
                            ]);
                            let pattern_constructor = rt.spec_get(&first, "constructor")?;
                            let _pattern_constructor_roots = rt.push_temporary_value_roots(&[
                                first.clone(),
                                flags_arg.clone(),
                                new_target_root.clone(),
                                pattern_constructor.clone(),
                            ]);
                            if abstract_ops::same_value(
                                &rt.global_get("RegExp"),
                                &pattern_constructor,
                            ) && regexp_constructor_can_return_pattern(rt, *id)
                            {
                                return Ok(first);
                            }
                        }
                        let p = match &rt.obj(*id).internal_kind {
                            InternalKind::RegExp(re) => JsString::from((*re.source).clone()),
                            _ => {
                                let _source_roots = rt.push_temporary_value_roots(&[
                                    first.clone(),
                                    flags_arg.clone(),
                                    new_target_root.clone(),
                                ]);
                                let source_v = rt.spec_get(&first, "source")?;
                                let _source_value_roots = rt.push_temporary_value_roots(&[
                                    first.clone(),
                                    flags_arg.clone(),
                                    new_target_root.clone(),
                                    source_v.clone(),
                                ]);
                                rt.coerce_to_js_string(&source_v)?
                            }
                        };
                        new_regexp_proto = regexp_new_target_prototype(rt)?;
                        let f = if flags_is_undefined {
                            match &rt.obj(*id).internal_kind {
                                InternalKind::RegExp(re) => (*re.flags).clone(),
                                _ => {
                                    let _flags_roots = rt.push_temporary_value_roots(&[
                                        first.clone(),
                                        flags_arg.clone(),
                                        new_target_root.clone(),
                                    ]);
                                    let flags_v = rt.spec_get(&first, "flags")?;
                                    let _flags_value_roots = rt.push_temporary_value_roots(&[
                                        first.clone(),
                                        flags_arg.clone(),
                                        new_target_root.clone(),
                                        flags_v.clone(),
                                    ]);
                                    rt.coerce_to_string(&flags_v)?
                                }
                            }
                        } else {
                            let _flags_arg_roots = rt.push_temporary_value_roots(&[
                                first.clone(),
                                flags_arg.clone(),
                                new_target_root.clone(),
                            ]);
                            rt.coerce_to_string(&flags_arg)?
                        };
                        let raw = matches!(rt.obj(*id).internal_kind, InternalKind::RegExp(_));
                        (p, f, raw)
                    } else if let InternalKind::RegExp(re) = &rt.obj(*id).internal_kind {
                        let src = JsString::from((*re.source).clone());
                        let raw_flags = (*re.flags).clone();
                        new_regexp_proto = regexp_new_target_prototype(rt)?;
                        let f = match args.get(1) {
                            Some(Value::Undefined) | None => raw_flags,
                            Some(v) => rt.coerce_to_string(v)?,
                        };
                        (src, f, true)
                    } else {
                        let p = rt.coerce_to_js_string(&first)?;
                        new_regexp_proto = regexp_new_target_prototype(rt)?;
                        let f = match flags_arg {
                            Value::Undefined => String::new(),
                            v => rt.coerce_to_string(&v)?,
                        };
                        (p, f, false)
                    }
                }
                Value::Undefined => {
                    new_regexp_proto = regexp_new_target_prototype(rt)?;
                    match flags_arg {
                        Value::Undefined => (JsString::from(""), String::new(), false),
                        v => (JsString::from(""), rt.coerce_to_string(&v)?, false),
                    }
                }
                v => {
                    let p = rt.coerce_to_js_string(v)?;
                    new_regexp_proto = regexp_new_target_prototype(rt)?;
                    let f = match flags_arg {
                        Value::Undefined => String::new(),
                        v => rt.coerce_to_string(&v)?,
                    };
                    (p, f, false)
                }
            };
            let re = new_regexp_from_js_string(rt, &pattern, &flags, pattern_is_internal_raw)?;
            if let Some(proto) = new_regexp_proto {
                let _install_proto_roots =
                    rt.push_temporary_value_roots(&[Value::Object(re), Value::Object(proto)]);
                rt.obj_mut(re).proto = Some(proto);
            }
            Ok(Value::Object(re))
        });

        if let Value::Object(ctor_id) = self.global_get("RegExp") {
            self.obj_mut(ctor_id)
                .set_own_frozen("prototype".into(), Value::Object(proto));
            let escape_obj = make_native_non_ctor("escape", 1, |_rt, args| {
                let src = match args.first() {
                    Some(Value::String(s)) => s.clone(),
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "RegExp.escape: argument must be a string".into(),
                        ))
                    }
                };
                Ok(Value::String(Rc::new(crate::value::JsString::from(
                    regexp_escape(&src.code_units()),
                ))))
            });
            let escape_id = self.alloc_object(escape_obj);
            self.obj_mut(ctor_id)
                .set_own_internal("escape".into(), Value::Object(escape_id));
            install_legacy_regexp_accessors(self, ctor_id);

            self.obj_mut(proto)
                .set_own_internal("constructor".into(), Value::Object(ctor_id));

            let sp_get = crate::intrinsics::make_native_with_length(
                "get [Symbol.species]",
                0,
                |rt, _args| Ok(rt.current_this()),
            );
            let sp_id = self.alloc_object(sp_get);
            self.obj_mut(ctor_id).dict_mut().insert(
                crate::value::PropertyKey::String("@@species".into()),
                PropertyDescriptor {
                    value: Value::Undefined,
                    writable: false,
                    enumerable: false,
                    configurable: true,
                    getter: Some(Value::Object(sp_id)),
                    setter: None,
                },
            );
        }

        install_string_regex_methods(self);
    }
}

fn regexp_escape(units: &[u16]) -> String {
    let mut out = String::new();
    let mut i = 0;
    let mut first = true;
    while i < units.len() {
        let cu = units[i];
        if (0xD800..=0xDBFF).contains(&cu) {
            if let Some(&low) = units.get(i + 1) {
                if (0xDC00..=0xDFFF).contains(&low) {
                    let cp = 0x1_0000 + (((cu as u32 - 0xD800) << 10) | (low as u32 - 0xDC00));
                    regexp_escape_char(&mut out, char::from_u32(cp).unwrap(), first);
                    first = false;
                    i += 2;
                    continue;
                }
            }

            push_regexp_unicode_escape(&mut out, cu as u32);
            first = false;
            i += 1;
            continue;
        }
        if (0xDC00..=0xDFFF).contains(&cu) {

            push_regexp_unicode_escape(&mut out, cu as u32);
            first = false;
            i += 1;
            continue;
        }
        regexp_escape_char(&mut out, char::from_u32(cu as u32).unwrap(), first);
        first = false;
        i += 1;
    }
    out
}

fn regexp_escape_char(out: &mut String, ch: char, first: bool) {
    if first && ch.is_ascii_alphanumeric() {
        out.push_str(&format!("\\x{:02x}", ch as u32));
        return;
    }
    match ch {
        '^' | '$' | '\\' | '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
        | '/' => {
            out.push('\\');
            out.push(ch);
        }
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        '\u{000B}' => out.push_str("\\v"),
        '\u{000C}' => out.push_str("\\f"),
        ' ' => out.push_str("\\x20"),
        ',' | '-' | '=' | '<' | '>' | '#' | '&' | '!' | '%' | ':' | ';' | '@' | '~' | '\''
        | '"' | '`' => {
            push_regexp_unicode_escape(out, ch as u32);
        }
        _ if ch.is_control() => {
            out.push_str(&format!("\\x{:02x}", ch as u32));
        }
        '\u{2028}' | '\u{2029}' | '\u{feff}' => {
            push_regexp_unicode_escape(out, ch as u32);
        }
        _ if ch.is_whitespace() && (ch as u32) > 0x7f => {
            push_regexp_unicode_escape(out, ch as u32);
        }
        _ => out.push(ch),
    }
}

fn push_regexp_unicode_escape(out: &mut String, code: u32) {
    if code <= 0xff {
        out.push_str(&format!("\\x{code:02x}"));
    } else if code <= 0xffff {
        out.push_str(&format!("\\u{code:04x}"));
    } else {
        let scalar = code - 0x1_0000;
        let high = 0xd800 + (scalar >> 10);
        let low = 0xdc00 + (scalar & 0x3ff);
        out.push_str(&format!("\\u{high:04x}\\u{low:04x}"));
    }
}

fn normalize_regexp_constructor_pattern(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut iter = pattern.chars().peekable();
    while let Some(ch) = iter.next() {
        if ch == '\\' {
            match iter.peek().copied() {
                Some('\n') => {
                    iter.next();
                    out.push('\n');
                    continue;
                }
                Some('\r') => {
                    iter.next();
                    if iter.peek() == Some(&'\n') {
                        iter.next();
                    }
                    out.push('\r');
                    continue;
                }
                Some('\u{2028}') => {
                    iter.next();
                    out.push('\u{2028}');
                    continue;
                }
                Some('\u{2029}') => {
                    iter.next();
                    out.push('\u{2029}');
                    continue;
                }
                _ => {}
            }
        }
        out.push(ch);
    }
    out
}

const RAW_SURROGATE_MARKER_BASE: u32 = 0xF0000;

fn push_raw_surrogate_marker(out: &mut String, unit: u16) {
    debug_assert!((0xD800..=0xDFFF).contains(&unit));
    let cp = RAW_SURROGATE_MARKER_BASE + (unit as u32 - 0xD800);
    if let Some(ch) = char::from_u32(cp) {
        out.push(ch);
    }
}

fn raw_surrogate_marker_to_unit(c: char) -> Option<u16> {
    let offset = (c as u32).checked_sub(RAW_SURROGATE_MARKER_BASE)?;
    if offset <= 0x7ff {
        Some(0xD800 + offset as u16)
    } else {
        None
    }
}

fn regexp_compile_pattern_from_js_string(
    pattern: &JsString,
    raw_literal_or_clone: bool,
    flags: &str,
) -> String {
    let unicode_mode = flags.contains('u') || flags.contains('v');
    let preserve_raw_units = raw_literal_or_clone || unicode_mode;
    let units = pattern.code_units();
    let mut out = String::new();
    let mut i = 0usize;
    while i < units.len() {
        let unit = units[i];
        if !preserve_raw_units && unit == b'\\' as u16 {
            match units.get(i + 1).copied() {
                Some(0x000A) => {
                    out.push('\n');
                    i += 2;
                    continue;
                }
                Some(0x000D) => {
                    out.push('\r');
                    i += if units.get(i + 2) == Some(&(0x000A)) {
                        3
                    } else {
                        2
                    };
                    continue;
                }
                Some(0x2028) => {
                    out.push('\u{2028}');
                    i += 2;
                    continue;
                }
                Some(0x2029) => {
                    out.push('\u{2029}');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        if !unicode_mode && (0xD800..=0xDFFF).contains(&unit) {
            push_raw_surrogate_marker(&mut out, unit);
            i += 1;
            continue;
        }
        if let Some((ch, width)) = decode_utf16_scalar(&units, i) {
            out.push(ch);
            i += width;
        } else {
            push_raw_surrogate_marker(&mut out, unit);
            i += 1;
        }
    }
    out
}

fn decode_utf16_scalar(units: &[u16], i: usize) -> Option<(char, usize)> {
    let u0 = *units.get(i)?;
    if (0xD800..=0xDBFF).contains(&u0) {
        let u1 = *units.get(i + 1)?;
        if (0xDC00..=0xDFFF).contains(&u1) {
            let cp = 0x1_0000 + (((u0 as u32 - 0xD800) << 10) | (u1 as u32 - 0xDC00));
            return char::from_u32(cp).map(|ch| (ch, 2));
        }
        return None;
    }
    if (0xDC00..=0xDFFF).contains(&u0) {
        return None;
    }
    char::from_u32(u0 as u32).map(|ch| (ch, 1))
}

pub fn new_regexp(rt: &mut Runtime, pattern: &str, flags: &str) -> Result<ObjectRef, RuntimeError> {
    let pattern = JsString::from(pattern);
    new_regexp_from_js_string(rt, &pattern, flags, true)
}

fn new_regexp_from_js_string(
    rt: &mut Runtime,
    pattern: &JsString,
    flags: &str,
    raw_literal_or_clone: bool,
) -> Result<ObjectRef, RuntimeError> {
    validate_regexp_flags(flags)?;
    let compile_pattern =
        regexp_compile_pattern_from_js_string(pattern, raw_literal_or_clone, flags);
    let public_source = if raw_literal_or_clone || flags.contains('u') || flags.contains('v') {
        pattern.as_str().to_string()
    } else {
        compile_pattern.clone()
    };
    validate_compile_pattern_syntax(&compile_pattern, flags)?;
    validate_character_class_ranges(&compile_pattern, flags)?;
    validate_unicode_property_escapes(&compile_pattern, flags)
        .map_err(|e| wrap_regexp_syntax_error(e, &public_source, flags))?;
    validate_unicode_mode_syntax(&compile_pattern, flags)?;
    validate_inline_modifiers(&compile_pattern)?;
    validate_named_group_refs(&compile_pattern, flags)?;
    validate_string_properties(&compile_pattern, flags)?;
    validate_v_mode_class_syntax(&compile_pattern, flags)?;
    let compiled = compile_either(&compile_pattern, flags);
    let internals = RegExpInternals {
        source: Rc::new(public_source),
        flags: Rc::new(flags.to_string()),
        compiled,
        last_index: 0,
    };
    let base_proto = rt.realms[rt.current_realm]
        .regexp_prototype
        .or(rt.regexp_prototype);
    let proto = match base_proto {
        Some(p) => Some(rt.prototype_from_new_target_or(p)?),
        None => None,
    };
    let obj = Object {
        proto,
        extensible: true,
        properties: indexmap::IndexMap::new(),
        internal_kind: InternalKind::RegExp(Box::new(internals)),

        ..Default::default()
    };
    let id = rt.alloc_object(obj);

    rt.obj_mut(id).dict_mut().insert(
        crate::value::PropertyKey::String("lastIndex".to_string()),
        PropertyDescriptor {
            value: Value::Number(0.0),
            writable: true,
            enumerable: false,
            configurable: false,
            getter: None,
            setter: None,
        },
    );
    Ok(id)
}

fn elide_surrogate_pair_alternatives(pattern: &str) -> Option<String> {
    let bytes = pattern.as_bytes();

    let is_high_surrogate_at = |p: usize| -> bool {
        if p + 6 > bytes.len() || &bytes[p..p + 2] != b"\\u" {
            return false;
        }
        let hex = &bytes[p + 2..p + 6];
        if !hex.iter().all(|b| b.is_ascii_hexdigit()) {
            return false;
        }
        let val = u32::from_str_radix(std::str::from_utf8(hex).unwrap(), 16).unwrap();
        (0xD800..=0xDBFF).contains(&val)
    };

    fn clean_segment(
        bytes: &[u8],
        start: usize,
        end: usize,
        is_high_surrogate_at: &dyn Fn(usize) -> bool,
        changed: &mut bool,
    ) -> String {

        let mut alt_starts: Vec<usize> = vec![start];
        let mut alt_ends: Vec<usize> = Vec::new();
        let mut group_depth: i32 = 0;
        let mut class_depth: i32 = 0;
        let mut i = start;
        while i < end {
            match bytes[i] {
                b'\\' if i + 1 < end => {
                    i += 2;
                }
                b'(' if class_depth == 0 => {
                    group_depth += 1;
                    i += 1;
                }
                b')' if class_depth == 0 => {
                    group_depth -= 1;
                    i += 1;
                }
                b'[' if class_depth == 0 => {
                    class_depth = 1;
                    i += 1;
                }
                b']' if class_depth > 0 => {
                    class_depth = 0;
                    i += 1;
                }
                b'|' if group_depth == 0 && class_depth == 0 => {
                    alt_ends.push(i);
                    alt_starts.push(i + 1);
                    i += 1;
                }
                _ => {
                    i += 1;
                }
            }
        }
        alt_ends.push(end);

        let mut kept: Vec<String> = Vec::new();
        for (&s, &e) in alt_starts.iter().zip(alt_ends.iter()) {

            let mut has_surrogate = false;
            let mut k = s;
            let mut scan_group_depth: i32 = 0;
            while k < e {
                match bytes[k] {
                    b'\\' if k + 1 < e => {
                        if scan_group_depth == 0 && is_high_surrogate_at(k) {
                            has_surrogate = true;
                            break;
                        }
                        k += 2;
                    }
                    b'(' => {
                        scan_group_depth += 1;
                        k += 1;
                    }
                    b')' => {
                        scan_group_depth -= 1;
                        k += 1;
                    }
                    _ => {
                        k += 1;
                    }
                }
            }
            if has_surrogate {
                *changed = true;
                if let Some(translated) = translate_surrogate_alt(bytes, s, e) {
                    kept.push(translated);
                }
                continue;
            }

            let mut rebuilt = String::with_capacity(e - s);
            let mut p = s;
            let mut cd = 0i32;
            while p < e {
                match bytes[p] {
                    b'\\' if p + 1 < e => {
                        rebuilt.push(bytes[p] as char);
                        rebuilt.push(bytes[p + 1] as char);
                        p += 2;
                    }
                    b'[' if cd == 0 => {
                        cd = 1;
                        rebuilt.push('[');
                        p += 1;
                    }
                    b']' if cd > 0 => {
                        cd = 0;
                        rebuilt.push(']');
                        p += 1;
                    }
                    b'(' if cd == 0 => {

                        let group_start = p;
                        let mut d = 1i32;
                        let mut q = p + 1;

                        let mut inner_start = p + 1;
                        if q < e && bytes[q] == b'?' {

                            q += 1;
                            while q < e
                                && bytes[q] != b':'
                                && bytes[q] != b'='
                                && bytes[q] != b'!'
                                && bytes[q] != b'<'
                                && bytes[q] != b'>'
                            {
                                q += 1;
                            }
                            if q < e {
                                if bytes[q] == b'<' {
                                    while q < e && bytes[q] != b'>' {
                                        q += 1;
                                    }
                                }
                                q += 1;
                                inner_start = q;
                            }
                        }

                        let mut cd2 = 0i32;
                        let mut close = q;
                        while close < e && d > 0 {
                            match bytes[close] {
                                b'\\' if close + 1 < e => {
                                    close += 2;
                                }
                                b'[' if cd2 == 0 => {
                                    cd2 = 1;
                                    close += 1;
                                }
                                b']' if cd2 > 0 => {
                                    cd2 = 0;
                                    close += 1;
                                }
                                b'(' if cd2 == 0 => {
                                    d += 1;
                                    close += 1;
                                }
                                b')' if cd2 == 0 => {
                                    d -= 1;
                                    if d == 0 {
                                        break;
                                    }
                                    close += 1;
                                }
                                _ => {
                                    close += 1;
                                }
                            }
                        }
                        if d == 0 && close < e {

                            for b in &bytes[group_start..inner_start] {
                                rebuilt.push(*b as char);
                            }

                            let inner = clean_segment(
                                bytes,
                                inner_start,
                                close,
                                is_high_surrogate_at,
                                changed,
                            );
                            rebuilt.push_str(&inner);
                            rebuilt.push(')');
                            p = close + 1;
                        } else {

                            rebuilt.push_str(std::str::from_utf8(&bytes[p..e]).unwrap());
                            p = e;
                        }
                    }
                    _ => {
                        rebuilt.push(bytes[p] as char);
                        p += 1;
                    }
                }
            }
            kept.push(rebuilt);
        }
        if kept.is_empty() {
            "(?!)".to_string()
        } else {
            kept.join("|")
        }
    }

    let mut changed = false;
    let cleaned = clean_segment(bytes, 0, bytes.len(), &is_high_surrogate_at, &mut changed);
    if changed {
        Some(cleaned)
    } else {
        None
    }
}

fn parse_unicode_esc(bytes: &[u8], p: usize) -> Option<(u32, usize)> {
    if p + 6 > bytes.len() || &bytes[p..p + 2] != b"\\u" {
        return None;
    }
    let hex = std::str::from_utf8(&bytes[p + 2..p + 6]).ok()?;
    let v = u32::from_str_radix(hex, 16).ok()?;
    Some((v, p + 6))
}

fn rewrite_unicode_surrogate_escape_pairs(pattern: &str) -> Option<String> {
    let bytes = pattern.as_bytes();
    let mut out = String::with_capacity(pattern.len());
    let mut changed = false;
    let mut in_class = false;
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            if !in_class {
                if let Some((hi, mid)) = parse_unicode_esc(bytes, i) {
                    if (0xD800..=0xDBFF).contains(&hi) {
                        if let Some((lo, end)) = parse_unicode_esc(bytes, mid) {
                            if (0xDC00..=0xDFFF).contains(&lo) {
                                let scalar = 0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00);
                                out.push_str(&format!("\\u{{{:X}}}", scalar));
                                i = end;
                                changed = true;
                                continue;
                            }
                        }
                    }
                }
            }
            if i + 1 < bytes.len() {
                out.push(bytes[i] as char);
                out.push(bytes[i + 1] as char);
                i += 2;
            } else {
                out.push('\\');
                i += 1;
            }
            continue;
        }
        match bytes[i] {
            b'[' if !in_class => {
                in_class = true;
                out.push('[');
            }
            b']' if in_class => {
                in_class = false;
                out.push(']');
            }
            b => out.push(b as char),
        }
        i += 1;
    }
    changed.then_some(out)
}

fn parse_uesc_class(bytes: &[u8], start: usize) -> Option<(Vec<(u32, u32)>, usize)> {
    if start >= bytes.len() || bytes[start] != b'[' {
        return None;
    }
    let mut p = start + 1;
    if p < bytes.len() && bytes[p] == b'^' {
        return None;
    }
    let mut ranges = Vec::new();
    while p < bytes.len() && bytes[p] != b']' {
        let (lo, q) = parse_unicode_esc(bytes, p)?;
        p = q;
        let hi =
            if p < bytes.len() && bytes[p] == b'-' && p + 1 < bytes.len() && bytes[p + 1] != b']' {
                let (h, q2) = parse_unicode_esc(bytes, p + 1)?;
                p = q2;
                h
            } else {
                lo
            };
        ranges.push((lo, hi));
    }
    if p >= bytes.len() {
        return None;
    }
    Some((ranges, p + 1))
}

fn emit_scalar_class(mut ranges: Vec<(u32, u32)>) -> String {
    if ranges.is_empty() {
        return "(?!)".to_string();
    }
    ranges.sort();
    let mut merged: Vec<(u32, u32)> = Vec::new();
    for r in ranges {
        if let Some(last) = merged.last_mut() {
            if r.0 <= last.1.saturating_add(1) {
                last.1 = last.1.max(r.1);
                continue;
            }
        }
        merged.push(r);
    }
    let mut s = String::from("[");
    for (a, b) in merged {
        if a == b {
            s.push_str(&format!("\\u{{{:X}}}", a));
        } else {
            s.push_str(&format!("\\u{{{:X}}}-\\u{{{:X}}}", a, b));
        }
    }
    s.push(']');
    s
}

fn translate_surrogate_alt(bytes: &[u8], start: usize, end: usize) -> Option<String> {
    let mut p = start;

    let high_ranges: Vec<(u32, u32)>;
    if p + 6 <= end && &bytes[p..p + 2] == b"\\u" {
        let (v, q) = parse_unicode_esc(bytes, p)?;
        high_ranges = vec![(v, v)];
        p = q;
    } else if p < end && bytes[p] == b'[' {
        let (rs, q) = parse_uesc_class(bytes, p)?;
        if q > end {
            return None;
        }
        high_ranges = rs;
        p = q;
    } else {
        return None;
    }

    for &(a, b) in &high_ranges {
        if !(0xD800..=0xDBFF).contains(&a) || !(0xD800..=0xDBFF).contains(&b) {
            return None;
        }
    }

    if p >= end || bytes[p] != b'[' {
        return None;
    }
    let (low_ranges, q) = parse_uesc_class(bytes, p)?;
    if q != end {
        return None;
    }
    for &(a, b) in &low_ranges {
        if !(0xDC00..=0xDFFF).contains(&a) || !(0xDC00..=0xDFFF).contains(&b) {
            return None;
        }
    }

    let mut scalars: Vec<(u32, u32)> = Vec::new();
    for &(ha, hb) in &high_ranges {
        for h in ha..=hb {
            let base = 0x10000u32 + ((h - 0xD800) << 10);
            for &(la, lb) in &low_ranges {
                scalars.push((base + (la - 0xDC00), base + (lb - 0xDC00)));
            }
        }
    }
    Some(emit_scalar_class(scalars))
}

pub(crate) fn translate(pattern: &str, flags: &str) -> Result<String, String> {
    let mut flag_set = String::new();
    for c in flags.chars() {
        match c {
            'i' => flag_set.push('i'),
            'm' => flag_set.push('m'),
            's' => flag_set.push('s'),

            'g' | 'y' | 'u' | 'd' | 'v' => {}
            _ => return Err(format!("unsupported regex flag '{}'", c)),
        }
    }

    if !flags.contains('u') && !flags.contains('v') && class_range_has_class_endpoint(pattern) {
        return Err("annexB class range with class-escape endpoint: defer to Hand".into());
    }
    let identity_rewritten = rewrite_angle_identity_escapes(pattern);
    let pattern = identity_rewritten.as_str();

    let js_class_rewritten = rewrite_js_class_escapes(pattern);
    let pattern = js_class_rewritten.as_str();
    let dot_rewritten = if flags.contains('s') {
        None
    } else {
        rewrite_js_dot_atoms(pattern)
    };
    let pattern = dot_rewritten.as_deref().unwrap_or(pattern);
    let annex_b_decimal_class_expanded = if flags.contains('u') || flags.contains('v') {
        None
    } else {
        rewrite_annex_b_decimal_class_escapes(pattern)
    };
    let pattern = annex_b_decimal_class_expanded.as_deref().unwrap_or(pattern);
    let surrogate_pair_rewritten = if flags.contains('u') || flags.contains('v') {
        rewrite_unicode_surrogate_escape_pairs(pattern)
    } else {
        None
    };
    let pattern = surrogate_pair_rewritten.as_deref().unwrap_or(pattern);
    let cleaned = elide_surrogate_pair_alternatives(pattern);
    let property_expanded =
        expand_unicode_property_escapes(cleaned.as_deref().unwrap_or(pattern), flags)?;
    let body = property_expanded
        .as_deref()
        .unwrap_or(cleaned.as_deref().unwrap_or(pattern));
    let prefixed = if flag_set.is_empty() {
        body.to_string()
    } else {
        format!("(?{}){}", flag_set, body)
    };
    Ok(prefixed)
}

fn rewrite_angle_identity_escapes(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek().copied() {
                Some('<' | '>') => {
                    out.push(chars.next().unwrap());
                    continue;
                }
                Some(next) => {
                    out.push('\\');
                    out.push(next);
                    chars.next();
                    continue;
                }
                None => {
                    out.push('\\');
                    continue;
                }
            }
        }
        out.push(c);
    }
    out
}

fn rewrite_js_dot_atoms(pattern: &str) -> Option<String> {

    let chars: Vec<char> = pattern.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(pattern.len());
    let mut changed = false;
    let mut in_class = false;
    let mut dotall_stack: Vec<bool> = vec![false];
    let mut i = 0;
    while i < n {
        let c = chars[i];
        if c == '\\' {
            out.push(c);
            if i + 1 < n {
                out.push(chars[i + 1]);
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if c == '[' && !in_class {
            in_class = true;
            out.push(c);
            i += 1;
            continue;
        }
        if c == ']' && in_class {
            in_class = false;
            out.push(c);
            i += 1;
            continue;
        }
        if in_class {
            out.push(c);
            i += 1;
            continue;
        }
        if c == '(' {
            let cur = *dotall_stack.last().unwrap_or(&false);
            let mut new_scope = cur;

            if chars.get(i + 1) == Some(&'?') {
                let mut j = i + 2;
                let mut seen_dash = false;
                let mut add_s = false;
                let mut rem_s = false;
                while j < n && matches!(chars[j], 'i' | 'm' | 's' | '-') {
                    match chars[j] {
                        '-' => seen_dash = true,
                        's' if seen_dash => rem_s = true,
                        's' => add_s = true,
                        _ => {}
                    }
                    j += 1;
                }
                if j > i + 2 && chars.get(j) == Some(&':') {
                    if add_s {
                        new_scope = true;
                    } else if rem_s {
                        new_scope = false;
                    }
                }
            }
            dotall_stack.push(new_scope);
            out.push(c);
            i += 1;
            continue;
        }
        if c == ')' {
            if dotall_stack.len() > 1 {
                dotall_stack.pop();
            }
            out.push(c);
            i += 1;
            continue;
        }
        if c == '.' {
            if *dotall_stack.last().unwrap_or(&false) {
                out.push_str("[\\s\\S]");
            } else {
                out.push_str("[^\\n\\r\\x{2028}\\x{2029}]");
            }
            changed = true;
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    changed.then_some(out)
}

fn rewrite_js_class_escapes(pattern: &str) -> String {

    const WS: &str = "\\t\\n\\x{0B}\\f\\r \\x{A0}\\x{FEFF}\\x{1680}\\x{2000}-\\x{200A}\\x{2028}\\x{2029}\\x{202F}\\x{205F}\\x{3000}";
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::with_capacity(pattern.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() {
            let n = chars[i + 1];
            match n {
                'd' => out.push_str("[0-9]"),
                'D' => out.push_str("[^0-9]"),
                'w' => out.push_str("[0-9A-Za-z_]"),
                'W' => out.push_str("[^0-9A-Za-z_]"),
                's' => {
                    out.push('[');
                    out.push_str(WS);
                    out.push(']');
                }
                'S' => {
                    out.push_str("[^");
                    out.push_str(WS);
                    out.push(']');
                }

                _ => {
                    out.push(c);
                    out.push(n);
                }
            }
            i += 2;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

fn rewrite_annex_b_decimal_class_escapes(pattern: &str) -> Option<String> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::with_capacity(pattern.len());
    let mut changed = false;
    let mut in_class = false;
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            if in_class
                && i + 1 < chars.len()
                && matches!(chars[i + 1], '0'..='7')
                && !matches!(chars[i + 1], '0')
            {
                let mut value = 0u32;
                let mut consumed = 0usize;
                while consumed < 3 && i + 1 + consumed < chars.len() {
                    let d = chars[i + 1 + consumed];
                    let Some(n) = d.to_digit(8) else {
                        break;
                    };
                    value = value * 8 + n;
                    consumed += 1;
                }
                out.push_str(&format!("\\x{{{:X}}}", value));
                i += 1 + consumed;
                changed = true;
                continue;
            }

            if in_class && i + 1 < chars.len() && chars[i + 1] == 'c' {
                let after = chars.get(i + 2).copied();
                if let Some(d) = after.filter(|d| d.is_ascii_digit() || *d == '_') {
                    out.push_str(&format!("\\x{{{:X}}}", (d as u32) % 32));
                    i += 3;
                    changed = true;
                    continue;
                }
                if !matches!(after, Some(d) if d.is_ascii_alphabetic()) {
                    out.push_str("\\\\c");
                    i += 2;
                    changed = true;
                    continue;
                }
            }
            out.push(c);
            if i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if c == '[' {
            in_class = true;
        } else if c == ']' {
            in_class = false;
        }
        out.push(c);
        i += 1;
    }
    changed.then_some(out)
}

fn validate_unicode_mode_syntax(pattern: &str, flags: &str) -> Result<(), RuntimeError> {

    if !flags.contains('u') || flags.contains('v') {
        return Ok(());
    }
    let chars: Vec<char> = pattern.chars().collect();
    let n = chars.len();
    let mut in_class = false;
    let mut prev_was_bare_assertion = false;
    let mut group_depth = 0usize;
    let is_syntax_char = |c: char| {
        matches!(
            c,
            '^' | '$'
                | '\\'
                | '.'
                | '*'
                | '+'
                | '?'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '|'
                | '/'
        )
    };
    let mut i = 0;
    while i < n {
        let c = chars[i];
        if c == '\\' {
            let Some(&e) = chars.get(i + 1) else {
                return Err(RuntimeError::SyntaxError("\\ at end of pattern".into()));
            };
            let mut this_is_assertion = false;

            let mut consumed = 2usize;
            match e {
                'd' | 'D' | 's' | 'S' | 'w' | 'W' => {}
                'b' | 'B' => {
                    if in_class && e == 'B' {
                        return Err(RuntimeError::SyntaxError(
                            "invalid \\B class escape in Unicode-mode RegExp".into(),
                        ));
                    }
                    if !in_class {
                        this_is_assertion = true;
                    }
                }
                'f' | 'n' | 'r' | 't' | 'v' => {}
                '0' => {
                    if matches!(chars.get(i + 2), Some(d) if d.is_ascii_digit()) {
                        return Err(RuntimeError::SyntaxError(
                            "legacy octal escape sequence in Unicode-mode RegExp".into(),
                        ));
                    }
                }
                'c' => {
                    if !matches!(chars.get(i + 2), Some(l) if l.is_ascii_alphabetic()) {
                        return Err(RuntimeError::SyntaxError(
                            "invalid \\c control escape in Unicode-mode RegExp".into(),
                        ));
                    }
                    consumed = 3;
                }
                'x' => {
                    let ok = (i + 4) <= n
                        && chars[i + 2].is_ascii_hexdigit()
                        && chars[i + 3].is_ascii_hexdigit();
                    if !ok {
                        return Err(RuntimeError::SyntaxError(
                            "invalid \\x hex escape in Unicode-mode RegExp".into(),
                        ));
                    }
                    consumed = 4;
                }
                'u' => {
                    if chars.get(i + 2) == Some(&'{') {
                        let mut j = i + 3;
                        let mut any = false;
                        while j < n && chars[j].is_ascii_hexdigit() {
                            any = true;
                            j += 1;
                        }
                        if !any || chars.get(j) != Some(&'}') {
                            return Err(RuntimeError::SyntaxError(
                                "invalid \\u Unicode escape in Unicode-mode RegExp".into(),
                            ));
                        }
                        consumed = (j - i) + 1;
                    } else {
                        let ok = (i + 6) <= n
                            && chars[i + 2..i + 6].iter().all(|d| d.is_ascii_hexdigit());
                        if !ok {
                            return Err(RuntimeError::SyntaxError(
                                "invalid \\u Unicode escape in Unicode-mode RegExp".into(),
                            ));
                        }
                        consumed = 6;
                    }
                }
                'p' | 'P' => {
                    if chars.get(i + 2) == Some(&'{') {
                        let mut j = i + 3;
                        while j < n && chars[j] != '}' {
                            j += 1;
                        }
                        consumed = if j < n { (j - i) + 1 } else { j - i };
                    }
                }
                'k' => {
                    if chars.get(i + 2) == Some(&'<') {
                        let mut j = i + 3;
                        while j < n && chars[j] != '>' {
                            j += 1;
                        }
                        consumed = if j < n { (j - i) + 1 } else { j - i };
                    } else {
                        return Err(RuntimeError::SyntaxError(
                            "invalid \\k identity escape in Unicode-mode RegExp".into(),
                        ));
                    }
                }
                d if d.is_ascii_digit() => {}
                other => {

                    if !(other == '-' && in_class) && !is_syntax_char(other) {
                        return Err(RuntimeError::SyntaxError(format!(
                            "invalid identity escape `\\{other}` in Unicode-mode RegExp"
                        )));
                    }
                }
            }
            prev_was_bare_assertion = this_is_assertion;
            i += consumed;
            continue;
        }
        if in_class {
            if c == ']' {
                in_class = false;
            }
            prev_was_bare_assertion = false;
            i += 1;
            continue;
        }
        match c {
            '(' => {
                if matches!(chars.get(i + 1), Some('?'))
                    && matches!(chars.get(i + 2), Some('=' | '!'))
                {
                    let mut j = i + 3;
                    let mut depth = 1usize;
                    let mut class_depth = false;
                    while j < n {
                        match chars[j] {
                            '\\' => {
                                j += 2;
                                continue;
                            }
                            '[' if !class_depth => {
                                class_depth = true;
                            }
                            ']' if class_depth => {
                                class_depth = false;
                            }
                            '(' if !class_depth => {
                                depth += 1;
                            }
                            ')' if !class_depth => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        j += 1;
                    }
                    if depth != 0 {
                        return Err(RuntimeError::SyntaxError(
                            "unclosed assertion in Unicode-mode RegExp".into(),
                        ));
                    }
                    if matches!(chars.get(j + 1), Some('*' | '+' | '?'))
                        || matches!(chars.get(j + 1), Some('{'))
                    {
                        return Err(RuntimeError::SyntaxError(
                            "quantifier applied to an assertion in Unicode-mode RegExp".into(),
                        ));
                    }
                    prev_was_bare_assertion = false;
                    i = j + 1;
                    continue;
                } else {
                    group_depth += 1;
                    prev_was_bare_assertion = false;
                }
            }
            '[' => {
                in_class = true;
                prev_was_bare_assertion = false;
            }
            '*' | '+' | '?' => {
                if prev_was_bare_assertion {
                    return Err(RuntimeError::SyntaxError(
                        "quantifier applied to an assertion in Unicode-mode RegExp".into(),
                    ));
                }
                prev_was_bare_assertion = false;
            }
            '{' => {

                let mut j = i + 1;
                let mut saw_digit = false;
                while matches!(chars.get(j), Some(d) if d.is_ascii_digit()) {
                    saw_digit = true;
                    j += 1;
                }
                let mut shaped = saw_digit;
                if shaped && chars.get(j) == Some(&',') {
                    j += 1;
                    while matches!(chars.get(j), Some(d) if d.is_ascii_digit()) {
                        j += 1;
                    }
                }
                if !shaped || chars.get(j) != Some(&'}') {
                    shaped = false;
                }
                if !shaped {
                    return Err(RuntimeError::SyntaxError(
                        "incomplete or lone `{` quantifier in Unicode-mode RegExp".into(),
                    ));
                }

                prev_was_bare_assertion = false;
                i = j + 1;
                continue;
            }
            ')' => {
                if group_depth == 0 {
                    return Err(RuntimeError::SyntaxError(
                        "lone `)` in Unicode-mode RegExp".into(),
                    ));
                }
                group_depth -= 1;
                prev_was_bare_assertion = false;
            }
            '}' | ']' => {

                return Err(RuntimeError::SyntaxError(format!(
                    "lone `{c}` in Unicode-mode RegExp"
                )));
            }
            _ => {
                prev_was_bare_assertion = false;
            }
        }
        i += 1;
    }
    if in_class {
        return Err(RuntimeError::SyntaxError(
            "unclosed character class in Unicode-mode RegExp".into(),
        ));
    }
    if group_depth != 0 {
        return Err(RuntimeError::SyntaxError(
            "unclosed group in Unicode-mode RegExp".into(),
        ));
    }
    Ok(())
}

fn validate_unicode_property_escapes(pattern: &str, flags: &str) -> Result<(), RuntimeError> {

    if !flags.contains('u') && !flags.contains('v') {
        return Ok(());
    }

    let prop_err = |in_class: bool| {
        RuntimeError::SyntaxError(if in_class {
            "Invalid property name in character class".into()
        } else {
            "Invalid property name".to_string()
        })
    };
    let bytes = pattern.as_bytes();
    let mut i = 0;
    let mut in_class = false;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\\' {
            match bytes.get(i + 1) {
                Some(&c) if matches!(c, b'p' | b'P') => {
                    if bytes.get(i + 2) != Some(&b'{') {
                        return Err(prop_err(in_class));
                    }
                    let start = i + 3;
                    let Some(rel_end) = pattern[start..].find('}') else {
                        return Err(prop_err(in_class));
                    };
                    let end = start + rel_end;
                    let body = &pattern[start..end];
                    if !crate::generated_unicode::property_escapes::is_known_unicode_property_escape(
                        body,
                    ) {
                        return Err(prop_err(in_class));
                    }
                    i = end + 1;
                    continue;
                }

                Some(_) => {
                    i += 2;
                    continue;
                }
                None => break,
            }
        }
        match b {
            b'[' => in_class = true,
            b']' => in_class = false,
            _ => {}
        }
        i += 1;
    }
    Ok(())
}

fn wrap_regexp_syntax_error(err: RuntimeError, source: &str, flags: &str) -> RuntimeError {
    match err {
        RuntimeError::SyntaxError(inner) => RuntimeError::SyntaxError(format!(
            "Invalid regular expression: /{source}/{flags}: {inner}"
        )),
        other => other,
    }
}

fn validate_inline_modifiers(pattern: &str) -> Result<(), RuntimeError> {
    let chars: Vec<char> = pattern.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut in_class = false;
    while i < n {
        let c = chars[i];
        if c == '\\' {
            i += 2;
            continue;
        }
        if in_class {
            if c == ']' {
                in_class = false;
            }
            i += 1;
            continue;
        }
        if c == '[' {
            in_class = true;
            i += 1;
            continue;
        }
        if c == '(' && i + 1 < n && chars[i + 1] == '?' {
            match chars.get(i + 2).copied() {
                Some(':') | Some('=') | Some('!') | Some('<') => {
                    i += 2;
                    continue;
                }
                _ => {
                    let mut j = i + 2;
                    let mut add: Vec<char> = Vec::new();
                    while j < n && !matches!(chars[j], ':' | '-' | ')') {
                        add.push(chars[j]);
                        j += 1;
                    }
                    let mut has_dash = false;
                    let mut remove: Vec<char> = Vec::new();
                    if j < n && chars[j] == '-' {
                        has_dash = true;
                        j += 1;
                        while j < n && !matches!(chars[j], ':' | '-' | ')') {
                            remove.push(chars[j]);
                            j += 1;
                        }
                    }
                    if j >= n || chars[j] != ':' {
                        return Err(RuntimeError::SyntaxError(
                            "invalid regular expression inline modifier: \
                             expected `:` after modifier flags"
                                .into(),
                        ));
                    }
                    validate_modifier_run(&add)?;
                    if has_dash {
                        validate_modifier_run(&remove)?;
                        for c in &add {
                            if remove.contains(c) {
                                return Err(RuntimeError::SyntaxError(format!(
                                    "regular expression modifier flag `{c}` appears \
                                     in both the added and removed flag sets"
                                )));
                            }
                        }
                        if add.is_empty() && remove.is_empty() {
                            return Err(RuntimeError::SyntaxError(
                                "regular expression modifier group has empty added \
                                 and removed flag sets"
                                    .into(),
                            ));
                        }
                    }
                    i = j + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    Ok(())
}

fn validate_modifier_run(run: &[char]) -> Result<(), RuntimeError> {
    let mut seen: Vec<char> = Vec::new();
    for &c in run {
        if c != 'i' && c != 'm' && c != 's' {
            return Err(RuntimeError::SyntaxError(format!(
                "invalid regular expression modifier flag `{c}`"
            )));
        }
        if seen.contains(&c) {
            return Err(RuntimeError::SyntaxError(format!(
                "duplicate regular expression modifier flag `{c}`"
            )));
        }
        seen.push(c);
    }
    Ok(())
}

fn validate_regexp_flags(flags: &str) -> Result<(), RuntimeError> {
    let mut seen: Vec<char> = Vec::new();
    for c in flags.chars() {
        if !matches!(c, 'd' | 'g' | 'i' | 'm' | 's' | 'u' | 'v' | 'y') {
            return Err(RuntimeError::SyntaxError(format!(
                "invalid regular expression flag `{c}`"
            )));
        }
        if seen.contains(&c) {
            return Err(RuntimeError::SyntaxError(format!(
                "duplicate regular expression flag `{c}`"
            )));
        }
        seen.push(c);
    }
    if seen.contains(&'u') && seen.contains(&'v') {
        return Err(RuntimeError::SyntaxError(
            "regular expression flags `u` and `v` are mutually exclusive".into(),
        ));
    }
    Ok(())
}

fn count_regexp_capturing_groups(chars: &[char]) -> usize {
    let mut count = 0usize;
    let mut esc = false;
    let mut in_cls = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if esc {
            esc = false;
            i += 1;
            continue;
        }
        match c {
            '\\' => esc = true,
            '[' => in_cls = true,
            ']' => in_cls = false,
            '(' if !in_cls => {

                let is_named = chars.get(i + 1) == Some(&'?')
                    && chars.get(i + 2) == Some(&'<')
                    && matches!(chars.get(i + 3), Some(d) if d.is_alphabetic() || *d == '_' || *d == '$');
                let is_noncap = chars.get(i + 1) == Some(&'?') && !is_named;
                if !is_noncap {
                    count += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    count
}

fn validate_compile_pattern_syntax(pattern: &str, flags: &str) -> Result<(), RuntimeError> {
    let chars: Vec<char> = pattern.chars().collect();
    let total_groups = count_regexp_capturing_groups(&chars);
    let mut in_class = false;
    let mut escaped = false;
    let mut previous_atom = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if escaped {
            if matches!(c, 'u' | 'p' | 'P') && chars.get(i + 1) == Some(&'{') {
                let mut j = i + 2;
                while j < chars.len() && chars[j] != '}' {
                    j += 1;
                }
                escaped = false;
                previous_atom = true;
                i = if j < chars.len() { j + 1 } else { j };
                continue;
            }
            if (flags.contains('u') || flags.contains('v')) && c.is_ascii_digit() && c != '0' {

                let mut num = String::new();
                let mut j = i;
                while j < chars.len() && chars[j].is_ascii_digit() {
                    num.push(chars[j]);
                    j += 1;
                }
                let n: usize = num.parse().unwrap_or(usize::MAX);
                if in_class || n > total_groups {
                    return Err(RuntimeError::SyntaxError(format!(
                        "invalid decimal escape `\\{c}`"
                    )));
                }

                escaped = false;
                previous_atom = true;
                i = j;
                continue;
            }
            escaped = false;
            previous_atom = true;
            i += 1;
            continue;
        }
        if c == '\\' {
            escaped = true;
            i += 1;
            continue;
        }
        if in_class {
            if c == ']' {
                in_class = false;
                previous_atom = true;
            }
            i += 1;
            continue;
        }
        match c {
            '[' => {
                in_class = true;
                previous_atom = true;
                i += 1;
            }
            '?' | '*' | '+' => {
                if !previous_atom {
                    return Err(RuntimeError::SyntaxError(format!(
                        "invalid regular expression quantifier `{c}`"
                    )));
                }
                i += 1;
                if chars.get(i) == Some(&'?') {
                    i += 1;
                }
                previous_atom = false;
            }
            '{' => {
                let u_mode = flags.contains('u') || flags.contains('v');
                let Some((end, min, max)) = parse_quantifier_range(&chars, i) else {

                    if !previous_atom && u_mode {
                        return Err(RuntimeError::SyntaxError(
                            "invalid regular expression quantifier `{`".into(),
                        ));
                    }
                    previous_atom = true;
                    i += 1;
                    continue;
                };
                if max.is_some_and(|m| min > m) {
                    return Err(RuntimeError::SyntaxError(
                        "invalid regular expression quantifier range".into(),
                    ));
                }
                if !previous_atom {
                    return Err(RuntimeError::SyntaxError(
                        "invalid regular expression quantifier range".into(),
                    ));
                }
                i = end + 1;
                if chars.get(i) == Some(&'?') {
                    i += 1;
                }
                previous_atom = false;
            }
            '|' => {
                previous_atom = false;
                i += 1;
            }
            '(' => {
                previous_atom = false;
                if chars.get(i + 1) == Some(&'?') {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            ')' => {
                previous_atom = true;
                i += 1;
            }
            _ => {
                previous_atom = true;
                i += 1;
            }
        }
    }
    if escaped {
        return Err(RuntimeError::SyntaxError(
            "trailing regular expression escape".into(),
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum ClassAtomForRange {
    Single(u32),
    ClassEscape,
}

fn class_range_has_class_endpoint(pattern: &str) -> bool {
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i] != '[' {
            i += 1;
            continue;
        }
        i += 1;
        if chars.get(i) == Some(&'^') {
            i += 1;
        }
        while i < chars.len() && chars[i] != ']' {
            let (left, next) = parse_class_atom_for_range(&chars, i);
            i = next;
            if chars.get(i) != Some(&'-') || chars.get(i + 1) == Some(&']') {
                continue;
            }
            let (right, after_right) = parse_class_atom_for_range(&chars, i + 1);
            match (left, right) {
                (ClassAtomForRange::Single(_), ClassAtomForRange::Single(_)) => {
                    i = after_right;
                }
                _ => return true,
            }
        }
        if i < chars.len() && chars[i] == ']' {
            i += 1;
        }
    }
    false
}

fn validate_character_class_ranges(pattern: &str, flags: &str) -> Result<(), RuntimeError> {

    if flags.contains('v') {
        return Ok(());
    }
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i] != '[' {
            i += 1;
            continue;
        }
        i += 1;
        if chars.get(i) == Some(&'^') {
            i += 1;
        }
        while i < chars.len() && chars[i] != ']' {
            let (left, next) = parse_class_atom_for_range(&chars, i);
            i = next;
            if chars.get(i) != Some(&'-') || chars.get(i + 1) == Some(&']') {
                continue;
            }
            let (right, after_right) = parse_class_atom_for_range(&chars, i + 1);
            match (left, right) {
                (ClassAtomForRange::Single(a), ClassAtomForRange::Single(b)) => {
                    if a > b {
                        return Err(RuntimeError::SyntaxError(
                            "invalid regular expression character class range".into(),
                        ));
                    }
                    i = after_right;
                }
                _ => {

                    if flags.contains('u') {
                        return Err(RuntimeError::SyntaxError(
                            "invalid regular expression character class range".into(),
                        ));
                    }
                    i += 1;
                }
            }
        }
        if i < chars.len() && chars[i] == ']' {
            i += 1;
        }
    }
    Ok(())
}

fn parse_class_atom_for_range(chars: &[char], start: usize) -> (ClassAtomForRange, usize) {
    if start >= chars.len() {
        return (ClassAtomForRange::ClassEscape, start);
    }
    if chars[start] != '\\' {
        return (ClassAtomForRange::Single(chars[start] as u32), start + 1);
    }
    let Some(&esc) = chars.get(start + 1) else {
        return (ClassAtomForRange::Single('\\' as u32), start + 1);
    };
    match esc {
        'd' | 'D' | 's' | 'S' | 'w' | 'W' | 'B' => (ClassAtomForRange::ClassEscape, start + 2),
        'p' | 'P' if chars.get(start + 2) == Some(&'{') => {
            let mut i = start + 3;
            while i < chars.len() && chars[i] != '}' {
                i += 1;
            }
            (ClassAtomForRange::ClassEscape, (i + 1).min(chars.len()))
        }
        'b' => (ClassAtomForRange::Single(0x08), start + 2),
        't' => (ClassAtomForRange::Single(0x09), start + 2),
        'n' => (ClassAtomForRange::Single(0x0A), start + 2),
        'v' => (ClassAtomForRange::Single(0x0B), start + 2),
        'f' => (ClassAtomForRange::Single(0x0C), start + 2),
        'r' => (ClassAtomForRange::Single(0x0D), start + 2),
        '0'..='7' => {
            let mut value = 0u32;
            let mut i = start + 1;
            let mut consumed = 0;
            while consumed < 3 && i < chars.len() {
                let Some(d) = chars[i].to_digit(8) else {
                    break;
                };
                value = value * 8 + d;
                i += 1;
                consumed += 1;
            }
            (ClassAtomForRange::Single(value), i)
        }
        'x' if start + 3 < chars.len() => {
            let hex: String = chars[start + 2..=start + 3].iter().collect();
            let value = u32::from_str_radix(&hex, 16).unwrap_or(esc as u32);
            (ClassAtomForRange::Single(value), start + 4)
        }

        'u' if chars.get(start + 2) == Some(&'{') => {
            let mut i = start + 3;
            let mut value = 0u32;
            while i < chars.len() && chars[i] != '}' {
                if let Some(d) = chars[i].to_digit(16) {
                    value = value.saturating_mul(16).saturating_add(d);
                }
                i += 1;
            }
            (ClassAtomForRange::Single(value), (i + 1).min(chars.len()))
        }
        'u' if start + 5 < chars.len() => {
            let hex: String = chars[start + 2..=start + 5].iter().collect();
            let value = u32::from_str_radix(&hex, 16).unwrap_or(esc as u32);
            (ClassAtomForRange::Single(value), start + 6)
        }
        'c' if start + 2 < chars.len() => {
            let c = chars[start + 2] as u32;
            (ClassAtomForRange::Single(c & 0x1f), start + 3)
        }
        _ => (ClassAtomForRange::Single(esc as u32), start + 2),
    }
}

fn parse_quantifier_range(chars: &[char], start: usize) -> Option<(usize, u32, Option<u32>)> {
    let mut i = start + 1;
    let min_start = i;
    while i < chars.len() && chars[i].is_ascii_digit() {
        i += 1;
    }
    if i == min_start {
        return None;
    }
    let min: u32 = chars[min_start..i]
        .iter()
        .collect::<String>()
        .parse()
        .ok()?;
    let max = if i < chars.len() && chars[i] == ',' {
        i += 1;
        let max_start = i;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
        if i == max_start {
            None
        } else {
            Some(
                chars[max_start..i]
                    .iter()
                    .collect::<String>()
                    .parse()
                    .ok()?,
            )
        }
    } else {
        Some(min)
    };
    if i >= chars.len() || chars[i] != '}' {
        return None;
    }
    Some((i, min, max))
}

fn is_regexp_id_start(cp: u32) -> bool {
    rusty_js_unicode_ident::is_id_start(cp)
}

fn is_regexp_id_continue(cp: u32) -> bool {
    rusty_js_unicode_ident::is_id_continue(cp)
}

fn is_regexp_identifier_name(name: &str) -> bool {
    let mut chs = name.chars();
    match chs.next() {
        Some(first) if is_regexp_id_start(first as u32) => {
            chs.all(|ch| is_regexp_id_continue(ch as u32))
        }
        _ => false,
    }
}

fn decode_regexp_group_name(raw: &str) -> Option<String> {
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

fn validate_named_group_refs(pattern: &str, flags: &str) -> Result<(), RuntimeError> {
    let unicode = flags.contains('u') || flags.contains('v');
    let chars: Vec<char> = pattern.chars().collect();
    let n = chars.len();
    let mut frames: Vec<Vec<String>> = vec![Vec::new()];
    let mut all_defined: Vec<String> = Vec::new();
    let mut refs: Vec<String> = Vec::new();
    let mut duplicate: Option<String> = None;
    let mut i = 0;
    let mut in_class = false;
    while i < n {
        let c = chars[i];
        if c == '\\' {
            if i + 2 < n && chars[i + 1] == 'k' && chars[i + 2] == '<' {
                let mut j = i + 3;
                let mut name = String::new();
                while j < n && chars[j] != '>' {
                    name.push(chars[j]);
                    j += 1;
                }
                if j < n && chars[j] == '>' {
                    let decoded = decode_regexp_group_name(&name).unwrap_or(name);
                    refs.push(decoded);
                    i = j + 1;
                    continue;
                }
            }
            i += 2;
            continue;
        }
        if in_class {
            if c == ']' {
                in_class = false;
            }
            i += 1;
            continue;
        }
        if c == '[' {
            in_class = true;
            i += 1;
            continue;
        }
        if c == '|' {
            if let Some(top) = frames.last_mut() {
                top.clear();
            }
            i += 1;
            continue;
        }
        if c == ')' {
            if frames.len() > 1 {
                frames.pop();
            }
            i += 1;
            continue;
        }
        if c == '(' {
            if i + 2 < n
                && chars[i + 1] == '?'
                && chars[i + 2] == '<'
                && !matches!(chars.get(i + 3), Some('=') | Some('!'))
            {
                let mut j = i + 3;
                let mut name = String::new();
                while j < n && chars[j] != '>' {
                    name.push(chars[j]);
                    j += 1;
                }
                if j < n && chars[j] == '>' {
                    let decoded = decode_regexp_group_name(&name).unwrap_or(name);

                    if !decoded.is_empty() && !is_regexp_identifier_name(&decoded) {
                        return Err(RuntimeError::SyntaxError(format!(
                            "invalid capture group name `{decoded}` in RegExp"
                        )));
                    }
                    if duplicate.is_none() && frames.iter().any(|f| f.contains(&decoded)) {
                        duplicate = Some(decoded.clone());
                    }
                    if let Some(top) = frames.last_mut() {
                        top.push(decoded.clone());
                    }
                    if !all_defined.contains(&decoded) {
                        all_defined.push(decoded);
                    }
                    frames.push(Vec::new());
                    i = j + 1;
                    continue;
                }
            }
            frames.push(Vec::new());
            i += 1;
            continue;
        }
        i += 1;
    }
    if let Some(dup) = duplicate {
        return Err(RuntimeError::SyntaxError(format!(
            "duplicate capture group name `{dup}`"
        )));
    }
    let strict = unicode || !all_defined.is_empty();
    if strict {
        for r in &refs {
            if !all_defined.contains(r) {
                return Err(RuntimeError::SyntaxError(format!(
                    "reference to undefined capture group name `{r}`"
                )));
            }
        }
    }
    Ok(())
}

const STRING_PROPERTIES: &[&str] = &[
    "Basic_Emoji",
    "Emoji_Keycap_Sequence",
    "RGI_Emoji",
    "RGI_Emoji_Flag_Sequence",
    "RGI_Emoji_Modifier_Sequence",
    "RGI_Emoji_Tag_Sequence",
    "RGI_Emoji_ZWJ_Sequence",
];

fn validate_string_properties(pattern: &str, flags: &str) -> Result<(), RuntimeError> {
    let v_mode = flags.contains('v');
    let bytes = pattern.as_bytes();
    let mut i = 0;

    let mut class_neg: Vec<bool> = Vec::new();
    while i + 2 < bytes.len() {

        if bytes[i] == b'\\' && !matches!(bytes[i + 1], b'p' | b'P') {
            i += 2;
            continue;
        }
        if bytes[i] == b'[' {
            let negated_class = bytes.get(i + 1) == Some(&b'^');
            class_neg.push(negated_class);
            i += if negated_class { 2 } else { 1 };
            continue;
        }
        if bytes[i] == b']' {
            class_neg.pop();
            i += 1;
            continue;
        }
        if bytes[i] == b'\\' && matches!(bytes[i + 1], b'p' | b'P') && bytes[i + 2] == b'{' {
            let negated = bytes[i + 1] == b'P';
            let start = i + 3;
            if let Some(rel_end) = pattern[start..].find('}') {
                let end = start + rel_end;
                let body = &pattern[start..end];
                if STRING_PROPERTIES.contains(&body) {
                    if negated {
                        return Err(RuntimeError::SyntaxError(format!(
                            "Unicode property of strings `{body}` cannot be negated"
                        )));
                    }
                    if !v_mode {
                        return Err(RuntimeError::SyntaxError(format!(
                            "Unicode property of strings `{body}` requires the `v` flag"
                        )));
                    }
                    if class_neg.iter().any(|&n| n) {
                        return Err(RuntimeError::SyntaxError(format!(
                            "Unicode property of strings `{body}` cannot appear in a negated class"
                        )));
                    }
                }
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
    Ok(())
}

fn validate_v_mode_class_syntax(pattern: &str, flags: &str) -> Result<(), RuntimeError> {
    if !flags.contains('v') {
        return Ok(());
    }

    const RESERVED: &str = "!#$%*+,.:;<=>?@^`~";
    let chars: Vec<char> = pattern.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut class_depth: i32 = 0;
    while i < n {
        let c = chars[i];
        if c == '\\' {

            if i + 2 < n && chars[i + 1] == 'u' && chars[i + 2] == '{' {

                i += 3;
                while i < n && chars[i] != '}' {
                    i += 1;
                }
                i = (i + 1).min(n);
            } else if i + 2 < n && matches!(chars[i + 1], 'p' | 'P' | 'q') && chars[i + 2] == '{' {
                if chars[i + 1] == 'q' && class_depth == 0 {
                    return Err(RuntimeError::SyntaxError(
                        "Invalid regular expression: `\\q{}` is only valid inside a `v`-mode character class"
                            .into(),
                    ));
                }

                i += 3;
                let mut depth = 1;
                while i < n && depth > 0 {
                    if chars[i] == '\\' {
                        i += 2;
                        continue;
                    }
                    if chars[i] == '{' {
                        depth += 1;
                    } else if chars[i] == '}' {
                        depth -= 1;
                    }
                    i += 1;
                }
            } else {
                i += 2;
            }
            continue;
        }
        if c == '[' {
            class_depth += 1;
            i += 1;

            if i < n && chars[i] == '^' {
                i += 1;
            }
            continue;
        }
        if class_depth == 0 {
            i += 1;
            continue;
        }
        if c == ']' {
            class_depth -= 1;
            i += 1;
            continue;
        }

        if i + 1 < n && chars[i + 1] == c && RESERVED.contains(c) {
            return Err(RuntimeError::SyntaxError(format!(
                "Invalid regular expression: reserved double punctuator `{c}{c}` in a `v`-mode character class"
            )));
        }

        if matches!(c, '(' | ')' | '{' | '}' | '|' | '/') {
            return Err(RuntimeError::SyntaxError(format!(
                "Invalid regular expression: `{c}` must be escaped in a `v`-mode character class"
            )));
        }
        i += 1;
    }
    Ok(())
}

fn expand_unicode_property_escapes(pattern: &str, flags: &str) -> Result<Option<String>, String> {
    let bytes = pattern.as_bytes();
    let mut out = String::with_capacity(pattern.len());
    let mut changed = false;

    let mut class_depth = 0u32;
    let mut ignore_case_stack: Vec<bool> = vec![flags.contains('i')];
    let mut i = 0;
    while i < bytes.len() {
        if i + 3 < bytes.len()
            && bytes[i] == b'\\'
            && matches!(bytes[i + 1], b'p' | b'P')
            && bytes[i + 2] == b'{'
        {
            let negated = bytes[i + 1] == b'P';
            let start = i + 3;
            let rel_end = pattern[start..]
                .find('}')
                .ok_or_else(|| "invalid Unicode property escape".to_string())?;
            let end = start + rel_end;
            let body = &pattern[start..end];

            if is_surrogate_property_escape(body) {

            } else if crate::rusty_js_regex::native_property_needs_ignore_case_charset(body)
                && *ignore_case_stack.last().unwrap_or(&false)
            {

            } else if crate::rusty_js_regex::native_supports_property(body) {
                out.push_str(&pattern[i..end + 1]);
                i = end + 1;
                continue;
            } else if class_depth > 0 {

                if let Some(expanded) =
                    crate::generated_unicode::property_escapes::expand_unicode_property_escape_class_body(
                        body, negated,
                    )
                {
                    out.push_str(&expanded);
                    changed = true;
                    i = end + 1;
                    continue;
                }

                if !negated {
                    if let Some(alt) =
                        crate::generated_unicode::property_escapes::property_of_strings(body)
                    {

                        let members = alt
                            .strip_prefix("(?:")
                            .and_then(|s| s.strip_suffix(')'))
                            .unwrap_or(alt);
                        out.push_str("\\q{");
                        out.push_str(members);
                        out.push('}');
                        changed = true;
                        i = end + 1;
                        continue;
                    }
                }
            } else if let Some(expanded) = scalar_unicode_property_escape(body, negated) {
                out.push_str(expanded);
                changed = true;
                i = end + 1;
                continue;
            } else if let Some(expanded) =
                crate::generated_unicode::property_escapes::expand_unicode_property_escape(
                    body, negated,
                )
            {

                out.push_str(&expanded);
                changed = true;
                i = end + 1;
                continue;
            }
        }

        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            out.push('\\');
            out.push(bytes[i + 1] as char);
            i += 2;
            continue;
        }
        if bytes[i] == b'[' {
            class_depth += 1;
        } else if bytes[i] == b']' {
            class_depth = class_depth.saturating_sub(1);
        } else if class_depth == 0 && bytes[i] == b'(' {
            let cur = *ignore_case_stack.last().unwrap_or(&false);
            let mut new_scope = cur;
            if bytes.get(i + 1) == Some(&b'?') {
                let mut j = i + 2;
                let mut seen_dash = false;
                let mut add_i = false;
                let mut rem_i = false;
                while j < bytes.len() && matches!(bytes[j], b'i' | b'm' | b's' | b'-') {
                    match bytes[j] {
                        b'-' => seen_dash = true,
                        b'i' if seen_dash => rem_i = true,
                        b'i' => add_i = true,
                        _ => {}
                    }
                    j += 1;
                }
                if j > i + 2 && bytes.get(j) == Some(&b':') {
                    if add_i {
                        new_scope = true;
                    } else if rem_i {
                        new_scope = false;
                    }
                }
            }
            ignore_case_stack.push(new_scope);
        } else if class_depth == 0 && bytes[i] == b')' && ignore_case_stack.len() > 1 {
            ignore_case_stack.pop();
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    Ok(changed.then_some(out))
}

fn is_surrogate_property_escape(body: &str) -> bool {
    matches!(
        body,
        "Surrogate"
            | "Cs"
            | "gc=Surrogate"
            | "gc=Cs"
            | "General_Category=Surrogate"
            | "General_Category=Cs"
    )
}

fn scalar_unicode_property_escape(body: &str, negated: bool) -> Option<&'static str> {
    match (body, negated) {
        ("Surrogate", false)
        | ("Cs", false)
        | ("gc=Surrogate", false)
        | ("gc=Cs", false)
        | ("General_Category=Surrogate", false)
        | ("General_Category=Cs", false) => Some("(?!)"),
        ("Surrogate", true)
        | ("Cs", true)
        | ("gc=Surrogate", true)
        | ("gc=Cs", true)
        | ("General_Category=Surrogate", true)
        | ("General_Category=Cs", true) => Some("(?s:.)"),
        _ => None,
    }
}

pub fn compile_either(pattern: &str, flags: &str) -> Option<CompiledRegex> {

    let expanded = if flags.contains('u') || flags.contains('v') {
        expand_unicode_property_escapes(pattern, flags)
            .ok()
            .flatten()
    } else {
        None
    };
    let effective = expanded.as_deref().unwrap_or(pattern);
    crate::rusty_js_regex::compile(effective, flags)
        .ok()
        .map(CompiledRegex::Hand)
}

fn escape_regexp_literal(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' | '^' | '$' | '.' | '|' | '?' | '*' | '+' | '(' | ')' | '[' | ']' | '{' | '}' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

fn has_annex_b_identity_escape(pattern: &str, flags: &str) -> bool {
    if flags.contains('u') || flags.contains('v') {
        return false;
    }

    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            continue;
        }
        let Some(next) = chars.next() else {
            break;
        };
        if next == 'u' && chars.peek() == Some(&'{') {
            return true;
        }
        if matches!(
            next,
            'a' | 'e' | 'g' | 'h' | 'i' | 'j' | 'l' | 'm' | 'o' | 'q' | 'z'
        ) {
            return true;
        }
    }
    false
}

fn has_unicode_braced_utf16_sensitive_escape(pattern: &str, flags: &str) -> bool {
    if !flags.contains('u') && !flags.contains('v') {
        return false;
    }

    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    let mut in_class = false;
    while i + 3 < chars.len() {
        if chars[i] == '\\' {
            if chars[i + 1] == 'u' && chars[i + 2] == '{' && !in_class {
                let mut j = i + 3;
                let mut value: u32 = 0;
                let mut saw_digit = false;
                while j < chars.len() && chars[j] != '}' {
                    let Some(digit) = chars[j].to_digit(16) else {
                        saw_digit = false;
                        break;
                    };
                    saw_digit = true;
                    value = value.saturating_mul(16).saturating_add(digit);
                    j += 1;
                }
                if saw_digit
                    && j < chars.len()
                    && chars[j] == '}'
                    && (value > 0xFFFF || (0xD800..=0xDFFF).contains(&value))
                {
                    return true;
                }
            }
            i += 2;
            continue;
        }
        if chars[i] == '[' {
            in_class = true;
            i += 1;
            continue;
        }
        if chars[i] == ']' {
            in_class = false;
            i += 1;
            continue;
        }
        i += 1;
    }
    false
}

pub(crate) fn install_regexp_proto(rt: &mut Runtime, host: ObjectRef) {

    for name in &[
        "source",
        "flags",
        "global",
        "ignoreCase",
        "multiline",
        "sticky",
        "unicode",
        "unicodeSets",
        "dotAll",
        "hasIndices",
    ] {
        install_regexp_proto_accessor(rt, host, name);
    }

    register_regexp_proto_method(rt, host, "compile", 2, |rt, args| {
        let this_id = current_regexp_this(rt, "RegExp.prototype.compile")?;
        if rt.obj(this_id).proto != rt.regexp_prototype {
            return Err(RuntimeError::TypeError(
                "RegExp.prototype.compile: this is not a RegExp object".into(),
            ));
        }
        let (pattern, flags) = regexp_compile_args(
            rt,
            args.first().cloned().unwrap_or(Value::Undefined),
            args.get(1).cloned().unwrap_or(Value::Undefined),
        )?;
        validate_regexp_flags(&flags)?;
        validate_unicode_property_escapes(&pattern, &flags)
            .map_err(|e| wrap_regexp_syntax_error(e, &pattern, &flags))?;
        validate_inline_modifiers(&pattern)?;
        validate_named_group_refs(&pattern, &flags)?;
        validate_string_properties(&pattern, &flags)?;
        validate_compile_pattern_syntax(&pattern, &flags)?;
        let compiled = compile_either(&pattern, &flags);
        if compiled.is_none() {
            return Err(RuntimeError::SyntaxError(format!(
                "invalid regular expression pattern `{}`",
                pattern
            )));
        }
        if let InternalKind::RegExp(re) = &mut rt.obj_mut(this_id).internal_kind {
            re.source = Rc::new(pattern);
            re.flags = Rc::new(flags);
            re.compiled = compiled;
            re.last_index = 0;
        }
        set_last_index_strict(rt, this_id, 0.0)?;
        Ok(Value::Object(this_id))
    });

    crate::intrinsics::register_intrinsic_method(rt, host, "test", 1, |rt, args| {

        let this_id = current_object_this(rt, "RegExp.prototype.test")?;
        let arg = args.first().cloned().unwrap_or(Value::Undefined);
        let js = rt.to_js_string_strict(&arg)?;

        let exec_is_builtin = matches!(rt.obj(this_id).internal_kind, InternalKind::RegExp(_))
            && rt.regexp_builtin_exec.is_some()
            && matches!(rt.read_property(this_id, "exec")?, Value::Object(id) if Some(id) == rt.regexp_builtin_exec);
        if exec_is_builtin {
            if regexp_hand_faithful_needed(rt, this_id, &js) {
                let r = regexp_exec_via_jsstring(rt, this_id, &js)?;
                return Ok(Value::Boolean(!matches!(r, Value::Null)));
            }
            let input = js.as_str().to_string();
            let stateless_match = {
                let re = match &rt.obj(this_id).internal_kind {
                    InternalKind::RegExp(r) => r,
                    _ => unreachable!(),
                };
                if !re.flags.contains('g') && !re.flags.contains('y') {
                    re.compiled.as_ref().map(|rx| rx.is_match(&input))
                } else {
                    None
                }
            };
            if let Some(matched) = stateless_match {
                let last_index_v = rt.object_get(this_id, "lastIndex");
                let _ = regexp_to_length(rt, &last_index_v)?;
                return Ok(Value::Boolean(matched));
            }
            let r = regexp_exec(rt, this_id, &input)?;
            return Ok(Value::Boolean(!matches!(r, Value::Null)));
        }

        let r = regexp_exec_generic_object(rt, this_id, js.as_str())?;
        Ok(Value::Boolean(!matches!(r, Value::Null)))
    });

    crate::intrinsics::register_intrinsic_method(rt, host, "exec", 1, |rt, args| {
        let this_id = current_regexp_this(rt, "RegExp.prototype.exec")?;

        let arg = args.first().cloned().unwrap_or(Value::Undefined);
        let js = rt.to_js_string_strict(&arg)?;
        regexp_exec_via_jsstring(rt, this_id, &js)
    });

    rt.regexp_builtin_exec = match rt.object_get(host, "exec") {
        Value::Object(id) => Some(id),
        _ => None,
    };

    register_method(rt, host, "toString", |rt, _args| {

        let this_v = rt.current_this();
        if !matches!(this_v, Value::Object(_)) {
            return Err(RuntimeError::TypeError(
                "RegExp.prototype.toString: this is not an Object".into(),
            ));
        }
        let source_v = rt.spec_get(&this_v, "source")?;
        let source = rt.to_js_string_strict(&source_v)?;
        let flags_v = rt.spec_get(&this_v, "flags")?;
        let flags = rt.to_js_string_strict(&flags_v)?;
        Ok(Value::String(Rc::new(crate::value::JsString::from(
            format!("/{}/{}", source.as_str(), flags.as_str()),
        ))))
    });

    register_regexp_proto_method(rt, host, "@@match", 1, |rt, args| {
        let this_id = current_object_this(rt, "RegExp.prototype[@@match]")?;
        let s = rt.to_string_strict(&args.first().cloned().unwrap_or(Value::Undefined))?;
        regexp_match_generic_object(rt, this_id, &s)
    });
    register_regexp_proto_method(rt, host, "@@search", 1, |rt, args| {
        let this_id = current_object_this(rt, "RegExp.prototype[@@search]")?;
        let s = rt.to_string_strict(&args.first().cloned().unwrap_or(Value::Undefined))?;
        regexp_search_generic_object(rt, this_id, &s)
    });
    register_regexp_proto_method(rt, host, "@@replace", 2, |rt, args| {
        let this_id = current_object_this(rt, "RegExp.prototype[@@replace]")?;
        let s = rt.to_string_strict(&args.first().cloned().unwrap_or(Value::Undefined))?;
        let repl = args.get(1).cloned().unwrap_or(Value::Undefined);
        regexp_replace_generic_object(rt, this_id, &s, repl)
    });
    register_regexp_proto_method(rt, host, "@@split", 2, |rt, args| {
        let this_id = current_object_this(rt, "RegExp.prototype[@@split]")?;

        let s = rt.to_js_string_strict(&args.first().cloned().unwrap_or(Value::Undefined))?;
        let limit = args.get(1).cloned().unwrap_or(Value::Undefined);
        let units = s.code_units().into_owned();
        regexp_split_protocol_object(rt, this_id, s.as_str(), &units, limit)
    });
    register_regexp_proto_method(rt, host, "@@matchAll", 1, |rt, args| {
        let this_id = current_object_this(rt, "RegExp.prototype[@@matchAll]")?;
        let s = rt.to_string_strict(&args.first().cloned().unwrap_or(Value::Undefined))?;
        regexp_match_all_protocol_object(rt, this_id, &s)
    });

}

fn regexp_split_with_captures(rx: &CompiledRegex, input: &str, limit: usize) -> Vec<Value> {
    if limit == 0 {
        return Vec::new();
    }
    if input.is_empty() {
        if matches!(rx.captures_at(input, 0), Some((0, 0, _))) {
            return Vec::new();
        }
        return vec![Value::String(Rc::new(crate::value::JsString::from(
            String::new(),
        )))];
    }

    let mut out = Vec::new();
    let mut p = 0usize;
    let mut q = 0usize;
    while q < input.len() {
        let Some((ms, me, groups)) = rx.captures_at(input, q) else {
            break;
        };
        if ms >= input.len() {
            break;
        }
        if me == p || ms < p {
            q = advance_string_index(input, q);
            continue;
        }

        out.push(Value::String(Rc::new(crate::value::JsString::from(
            input[p..ms].to_string(),
        ))));
        if out.len() >= limit {
            return out;
        }
        for g in groups.iter().skip(1) {
            let v = g
                .as_ref()
                .map(|s| Value::String(Rc::new(crate::value::JsString::from(s.clone()))))
                .unwrap_or(Value::Undefined);
            out.push(v);
            if out.len() >= limit {
                return out;
            }
        }

        p = me;
        q = if me == ms {
            advance_string_index(input, q)
        } else {
            me
        };
    }
    out.push(Value::String(Rc::new(crate::value::JsString::from(
        input[p..].to_string(),
    ))));
    out.truncate(limit);
    out
}

fn advance_string_index(input: &str, byte_index: usize) -> usize {
    if byte_index >= input.len() {
        return input.len();
    }
    byte_index + input[byte_index..].chars().next().map_or(1, char::len_utf8)
}

fn regexp_compile_args(
    rt: &mut Runtime,
    pattern_v: Value,
    flags_v: Value,
) -> Result<(String, String), RuntimeError> {
    if let Value::Object(id) = pattern_v {
        if let InternalKind::RegExp(re) = &rt.obj(id).internal_kind {
            if !matches!(flags_v, Value::Undefined) {
                return Err(RuntimeError::TypeError(
                    "RegExp.prototype.compile: flags must be undefined when pattern is RegExp"
                        .into(),
                ));
            }
            return Ok(((*re.source).clone(), (*re.flags).clone()));
        }
        let pattern = rt.coerce_to_string(&Value::Object(id))?;
        let flags = match flags_v {
            Value::Undefined => String::new(),
            v => rt.coerce_to_string(&v)?,
        };
        return Ok((pattern, flags));
    }

    let pattern = match pattern_v {
        Value::Undefined => String::new(),
        v => rt.coerce_to_string(&v)?,
    };
    let flags = match flags_v {
        Value::Undefined => String::new(),
        v => rt.coerce_to_string(&v)?,
    };
    Ok((pattern, flags))
}

pub(crate) fn set_last_index_strict(
    rt: &mut Runtime,
    id: ObjectRef,
    n: f64,
) -> Result<(), RuntimeError> {
    set_last_index_value_strict(rt, id, Value::Number(n))
}

fn set_last_index_value_strict(
    rt: &mut Runtime,
    id: ObjectRef,
    value: Value,
) -> Result<(), RuntimeError> {
    rt.object_set_strict(id, "lastIndex".into(), value)
}

pub(crate) fn regexp_to_length(rt: &mut Runtime, v: &Value) -> Result<f64, RuntimeError> {
    let n = rt.coerce_to_number(v)?;
    Ok(regexp_to_length_number(n))
}

fn regexp_to_length_number(n: f64) -> f64 {
    if n.is_nan() || n <= 0.0 {
        0.0
    } else if !n.is_finite() || n > 9007199254740991.0 {
        9007199254740991.0
    } else {
        n.floor()
    }
}

fn regexp_last_index_to_length(rt: &mut Runtime, this_id: ObjectRef) -> Result<f64, RuntimeError> {
    if let Some(n) = {
        let o = rt.obj(this_id);
        o.get_own("lastIndex").and_then(|d| {
            if d.getter.is_none() && d.setter.is_none() {
                if let Value::Number(n) = d.value {
                    return Some(regexp_to_length_number(n));
                }
            }
            None
        })
    } {
        REGEXP_LAST_INDEX_FAST_NUMBERS.fetch_add(1, Ordering::Relaxed);
        return Ok(n);
    }
    REGEXP_LAST_INDEX_FALLBACKS.fetch_add(1, Ordering::Relaxed);
    let last_index_v = rt.object_get(this_id, "lastIndex");
    regexp_to_length(rt, &last_index_v)
}

fn byte_to_utf16(s: &str, byte_off: usize) -> usize {
    let off = byte_off.min(s.len());
    if s.is_ascii() {
        return off;
    }
    s[..off]
        .chars()
        .map(|c| if (c as u32) >= 0x10000 { 2 } else { 1 })
        .sum()
}

fn utf16_len(s: &str) -> usize {
    if s.is_ascii() {
        return s.len();
    }
    crate::interp::utf16_code_unit_len(s)
}

fn utf16_to_byte(s: &str, utf16_off: usize) -> Option<usize> {
    if s.is_ascii() {
        return (utf16_off <= s.len()).then_some(utf16_off);
    }
    let mut units = 0usize;
    for (byte_idx, ch) in s.char_indices() {
        if units == utf16_off {
            return Some(byte_idx);
        }
        let width = if (ch as u32) >= 0x10000 { 2 } else { 1 };
        if units + width > utf16_off {
            return None;
        }
        units += width;
    }
    if units == utf16_off {
        Some(s.len())
    } else {
        None
    }
}

pub(crate) fn advance_string_index_utf16(s: &str, utf16_off: usize, unicode: bool) -> usize {
    if !unicode {
        return utf16_off + 1;
    }
    match utf16_to_byte(s, utf16_off) {
        Some(byte_idx) if byte_idx < s.len() => {
            let ch = s[byte_idx..].chars().next().unwrap();
            utf16_off + if (ch as u32) >= 0x10000 { 2 } else { 1 }
        }
        _ => utf16_off + 1,
    }
}

fn substring_utf16_preserve_lone(s: &str, start: usize, end: usize) -> String {
    let mut units = Vec::new();
    for i in start..end {
        if let Some(unit) = crate::interp::utf16_code_unit_at(s, i) {
            units.push(unit);
        }
    }
    String::from_utf16_lossy(&units)
}

fn substring_utf16_lossy(s: &str, start: usize, end: usize) -> String {
    if let (Some(start_b), Some(end_b)) = (utf16_to_byte(s, start), utf16_to_byte(s, end)) {
        let start_b = start_b.min(s.len());
        let end_b = end_b.min(s.len());
        if start_b <= end_b {
            return s[start_b..end_b].to_string();
        }
    }
    let mut units = Vec::new();
    for i in start..end {
        if let Some(unit) = crate::interp::utf16_code_unit_at(s, i) {
            units.push(unit);
        }
    }
    String::from_utf16_lossy(&units)
}

fn low_surrogate_escape_unit(source: &str) -> Option<u16> {
    let (unit, end) = parse_unicode_esc(source.as_bytes(), 0)?;
    if end == source.len() && (0xDC00..=0xDFFF).contains(&unit) {
        Some(unit as u16)
    } else {
        None
    }
}

fn start_or_low_surrogate_escape_unit(source: &str) -> Option<u16> {
    source
        .strip_prefix("^|")
        .and_then(low_surrogate_escape_unit)
}

fn find_utf16_code_unit(s: &str, start: usize, needle: u16) -> Option<usize> {
    let len = utf16_len(s);
    for i in start..len {
        if crate::interp::utf16_code_unit_at(s, i) == Some(needle) {
            return Some(i);
        }
    }
    None
}

const STANDIN_BASE: u32 = 0x10_0000;

fn standin_char(u: u16) -> char {
    if (0xD800..=0xDFFF).contains(&u) {
        char::from_u32(STANDIN_BASE + (u as u32 - 0xD800)).unwrap()
    } else {
        char::from_u32(u as u32).unwrap_or('\u{FFFD}')
    }
}

fn input_needs_codeunit_standin(input: &str) -> bool {
    input.chars().any(|c| (c as u32) > 0xFFFF)
}

fn build_codeunit_standin(input: &str) -> String {
    let n = utf16_len(input);
    let mut t = String::with_capacity(n);
    for i in 0..n {
        if let Some(u) = crate::interp::utf16_code_unit_at(input, i) {
            t.push(standin_char(u));
        }
    }
    t
}

fn t_byte_to_u16(t: &str, byte_off: usize) -> usize {
    t[..byte_off.min(t.len())].chars().count()
}
fn t_u16_to_byte(t: &str, u16_off: usize) -> usize {
    t.char_indices()
        .nth(u16_off)
        .map(|(b, _)| b)
        .unwrap_or(t.len())
}

#[allow(clippy::too_many_arguments)]
fn regexp_exec_codeunit_standin(
    rt: &mut Runtime,
    this_id: ObjectRef,
    input: &str,
    start_u16: usize,
    is_global: bool,
    is_sticky: bool,
    has_indices: bool,
) -> Result<Value, RuntimeError> {
    let t = build_codeunit_standin(input);
    let t_len_u16 = utf16_len(input);
    let input_units: Vec<u16> = input.encode_utf16().collect();
    let reset_and_null = |rt: &mut Runtime| -> Result<Value, RuntimeError> {
        if is_global {
            if let InternalKind::RegExp(r) = &mut rt.obj_mut(this_id).internal_kind {
                r.last_index = 0;
            }
            set_last_index_strict(rt, this_id, 0.0)?;
        }
        Ok(Value::Null)
    };
    if start_u16 > t_len_u16 {
        return reset_and_null(rt);
    }
    let caps = {
        let re = match &rt.obj(this_id).internal_kind {
            InternalKind::RegExp(r) => r,
            _ => unreachable!(),
        };
        match re.compiled.as_ref().unwrap() {
            CompiledRegex::Hand(h) => crate::rusty_js_regex::find_at(h, &input_units, start_u16)
                .map(|m| {
                    let groups: Vec<Option<Vec<u16>>> = m
                        .captures
                        .iter()
                        .map(|c| c.map(|(s, e)| input_units[s..e].to_vec()))
                        .collect();
                    (t_u16_to_byte(&t, m.start), t_u16_to_byte(&t, m.end), groups)
                }),
        }
    };

    let caps = match caps {
        Some((ms, _, _)) if is_sticky && t_byte_to_u16(&t, ms) != start_u16 => None,
        other => other,
    };
    let (ms, me, groups) = match caps {
        Some(c) => c,
        None => return reset_and_null(rt),
    };
    let mstart_u16 = t_byte_to_u16(&t, ms);
    let mend_u16 = t_byte_to_u16(&t, me);
    if is_global {
        if let InternalKind::RegExp(r) = &mut rt.obj_mut(this_id).internal_kind {
            r.last_index = mend_u16;
        }
        set_last_index_strict(rt, this_id, mend_u16 as f64)?;
    }
    let group_strs: Vec<Option<String>> = groups
        .iter()
        .map(|g| g.as_ref().map(|s| String::from_utf16_lossy(s)))
        .collect();
    let matched = substring_utf16_preserve_lone(input, mstart_u16, mend_u16);
    let input_len = utf16_len(input);
    let captures_for_legacy: Vec<String> = group_strs
        .iter()
        .skip(1)
        .map(|g| g.clone().unwrap_or_default())
        .collect();
    let last_paren = captures_for_legacy
        .iter()
        .rev()
        .find(|s| !s.is_empty())
        .cloned()
        .unwrap_or_default();
    rt.legacy_regexp_state = LegacyRegExpState::materialized(
        input.to_string(),
        matched.clone(),
        last_paren,
        substring_utf16_preserve_lone(input, 0, mstart_u16),
        substring_utf16_preserve_lone(input, mend_u16, input_len),
        captures_for_legacy,
    );
    let named = match &rt.obj(this_id).internal_kind {
        InternalKind::RegExp(r) => r.compiled.as_ref().map(|c| c.named_group_slots()),
        _ => None,
    };
    let arr = rt.alloc_object(Object::new_array());
    rt.object_set(
        arr,
        "0".into(),
        Value::String(Rc::new(crate::value::JsString::from_code_units(
            input_units[mstart_u16..mend_u16].to_vec(),
        ))),
    );
    for (i, g) in groups.iter().enumerate().skip(1) {
        let v = match g {
            Some(s) => Value::String(Rc::new(crate::value::JsString::from_code_units(s.clone()))),
            None => Value::Undefined,
        };
        rt.object_set(arr, i.to_string(), v);
    }
    rt.object_set(arr, "length".into(), Value::Number(group_strs.len() as f64));
    rt.object_set(arr, "index".into(), Value::Number(mstart_u16 as f64));
    rt.object_set(
        arr,
        "input".into(),
        Value::String(Rc::new(crate::value::JsString::from(input.to_string()))),
    );
    let named_list = named.unwrap_or_default();
    if !named_list.is_empty() {
        let g_obj = rt.alloc_object_with_explicit_null_proto(Object::new_ordinary());
        for (name, slots) in &named_list {
            let v = slots
                .iter()
                .find_map(|idx| groups.get(*idx).and_then(|g| g.clone()))
                .map(|s| Value::String(Rc::new(crate::value::JsString::from_code_units(s))))
                .unwrap_or(Value::Undefined);
            rt.object_set(g_obj, name.clone(), v);
        }
        rt.object_set(arr, "groups".into(), Value::Object(g_obj));
    } else {
        rt.object_set(arr, "groups".into(), Value::Undefined);
    }
    if has_indices {
        let positions = {
            let re = match &rt.obj(this_id).internal_kind {
                InternalKind::RegExp(r) => r,
                _ => unreachable!(),
            };
            match re.compiled.as_ref().unwrap() {
                CompiledRegex::Hand(h) => {
                    crate::rusty_js_regex::find_at(h, &input_units, start_u16).map(|m| {
                        let caps: Vec<Option<(usize, usize)>> = m
                            .captures
                            .iter()
                            .map(|c| c.map(|(s, e)| (t_u16_to_byte(&t, s), t_u16_to_byte(&t, e))))
                            .collect();
                        (t_u16_to_byte(&t, m.start), t_u16_to_byte(&t, m.end), caps)
                    })
                }
            }
        };
        let indices_arr = rt.alloc_object(Object::new_array());
        if let Some((_, _, pos_caps)) = &positions {
            for (i, p) in pos_caps.iter().enumerate() {
                let v = match p {
                    Some((s, e)) => {
                        let pair = rt.alloc_object(Object::new_array());
                        rt.object_set(
                            pair,
                            "0".into(),
                            Value::Number(t_byte_to_u16(&t, *s) as f64),
                        );
                        rt.object_set(
                            pair,
                            "1".into(),
                            Value::Number(t_byte_to_u16(&t, *e) as f64),
                        );
                        rt.object_set(pair, "length".into(), Value::Number(2.0));
                        Value::Object(pair)
                    }
                    None => Value::Undefined,
                };
                rt.object_set(indices_arr, i.to_string(), v);
            }
            rt.object_set(
                indices_arr,
                "length".into(),
                Value::Number(pos_caps.len() as f64),
            );
            if !named_list.is_empty() {
                let ig_obj = rt.alloc_object_with_explicit_null_proto(Object::new_ordinary());
                for (name, slots) in &named_list {
                    let v = match slots
                        .iter()
                        .find_map(|idx| pos_caps.get(*idx).and_then(|c| c.as_ref()))
                    {
                        Some((s, e)) => {
                            let pair = rt.alloc_object(Object::new_array());
                            rt.object_set(
                                pair,
                                "0".into(),
                                Value::Number(t_byte_to_u16(&t, *s) as f64),
                            );
                            rt.object_set(
                                pair,
                                "1".into(),
                                Value::Number(t_byte_to_u16(&t, *e) as f64),
                            );
                            rt.object_set(pair, "length".into(), Value::Number(2.0));
                            Value::Object(pair)
                        }
                        None => Value::Undefined,
                    };
                    rt.object_set(ig_obj, name.clone(), v);
                }
                rt.object_set(indices_arr, "groups".into(), Value::Object(ig_obj));
            } else {
                rt.object_set(indices_arr, "groups".into(), Value::Undefined);
            }
        }
        rt.object_set(arr, "indices".into(), Value::Object(indices_arr));
    }
    Ok(Value::Object(arr))
}

fn make_utf16_regexp_match_result(
    rt: &mut Runtime,
    this_id: ObjectRef,
    input: &str,
    start_u16: usize,
    end_u16: usize,
    is_global: bool,
) -> Result<Value, RuntimeError> {
    let matched = substring_utf16_preserve_lone(input, start_u16, end_u16);
    if is_global {
        if let InternalKind::RegExp(r) = &mut rt.obj_mut(this_id).internal_kind {
            r.last_index = end_u16;
        }
        set_last_index_strict(rt, this_id, end_u16 as f64)?;
    }
    let input_len = utf16_len(input);
    rt.legacy_regexp_state = LegacyRegExpState::materialized(
        input.to_string(),
        matched.clone(),
        String::new(),
        substring_utf16_preserve_lone(input, 0, start_u16),
        substring_utf16_preserve_lone(input, end_u16, input_len),
        Vec::new(),
    );
    let arr = rt.alloc_object(Object::new_array());
    let arr_roots = [Value::Object(this_id), Value::Object(arr)];
    let _arr_roots = rt.push_temporary_value_roots(&arr_roots);
    rt.object_set(
        arr,
        "0".into(),
        Value::String(Rc::new(crate::value::JsString::from(matched))),
    );
    rt.object_set(arr, "length".into(), Value::Number(1.0));
    rt.object_set(arr, "index".into(), Value::Number(start_u16 as f64));
    rt.object_set(
        arr,
        "input".into(),
        Value::String(Rc::new(crate::value::JsString::from(input.to_string()))),
    );
    rt.object_set(arr, "groups".into(), Value::Undefined);
    Ok(Value::Object(arr))
}

fn regexp_exec_nonunicode_low_surrogate(
    rt: &mut Runtime,
    this_id: ObjectRef,
    input: &str,
    source: &str,
    flags: &str,
    start_u16: usize,
) -> Result<Option<Value>, RuntimeError> {
    if flags.contains('u') || flags.contains('v') {
        return Ok(None);
    }
    let is_sticky = flags.contains('y');
    let is_global = flags.contains('g') || is_sticky;
    let input_len = utf16_len(input);
    if let Some(unit) = low_surrogate_escape_unit(source) {
        let found = if is_sticky {
            (start_u16 < input_len
                && crate::interp::utf16_code_unit_at(input, start_u16) == Some(unit))
            .then_some(start_u16)
        } else {
            find_utf16_code_unit(input, start_u16, unit)
        };
        return Ok(Some(match found {
            Some(pos) => {
                make_utf16_regexp_match_result(rt, this_id, input, pos, pos + 1, is_global)?
            }
            None => {
                if is_global {
                    if let InternalKind::RegExp(r) = &mut rt.obj_mut(this_id).internal_kind {
                        r.last_index = 0;
                    }
                    set_last_index_strict(rt, this_id, 0.0)?;
                }
                Value::Null
            }
        }));
    }
    if let Some(unit) = start_or_low_surrogate_escape_unit(source) {
        if start_u16 == 0 && !is_sticky {
            return Ok(Some(make_utf16_regexp_match_result(
                rt, this_id, input, 0, 0, is_global,
            )?));
        }
        let found = if is_sticky {
            (start_u16 < input_len
                && crate::interp::utf16_code_unit_at(input, start_u16) == Some(unit))
            .then_some(start_u16)
        } else {
            find_utf16_code_unit(input, start_u16, unit)
        };
        return Ok(Some(match found {
            Some(pos) => {
                make_utf16_regexp_match_result(rt, this_id, input, pos, pos + 1, is_global)?
            }
            None => {
                if is_global {
                    if let InternalKind::RegExp(r) = &mut rt.obj_mut(this_id).internal_kind {
                        r.last_index = 0;
                    }
                    set_last_index_strict(rt, this_id, 0.0)?;
                }
                Value::Null
            }
        }));
    }
    Ok(None)
}

fn regexp_hand_faithful_needed(
    rt: &Runtime,
    this_id: ObjectRef,
    js: &crate::value::JsString,
) -> bool {
    match &rt.obj(this_id).internal_kind {
        InternalKind::RegExp(r) => match r.compiled.as_ref() {
            Some(CompiledRegex::Hand(h)) => {
                !js.is_well_formed()
                    || crate::rusty_js_regex::has_class_surrogate(&h.ast)
                    || crate::rusty_js_regex::has_unit_atom(&h.ast)
            }
            _ => false,
        },
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn regexp_exec_faithful(
    rt: &mut Runtime,
    this_id: ObjectRef,
    units: &[u16],
    start_u16: usize,
    is_global: bool,
    is_sticky: bool,
    has_indices: bool,
) -> Result<Value, RuntimeError> {
    let reset_and_null = |rt: &mut Runtime| -> Result<Value, RuntimeError> {
        if is_global {
            if let InternalKind::RegExp(r) = &mut rt.obj_mut(this_id).internal_kind {
                r.last_index = 0;
            }
            set_last_index_strict(rt, this_id, 0.0)?;
        }
        Ok(Value::Null)
    };
    if start_u16 > units.len() {
        return reset_and_null(rt);
    }

    let (matched, named_list) = {
        let re = match &rt.obj(this_id).internal_kind {
            InternalKind::RegExp(r) => r,
            _ => unreachable!(),
        };
        let h = match re.compiled.as_ref() {
            Some(CompiledRegex::Hand(h)) => h,
            _ => unreachable!("regexp_exec_faithful requires the hand engine"),
        };
        let named = re.compiled.as_ref().map(|c| c.named_group_slots());
        (
            crate::rusty_js_regex::find_at(h, units, start_u16),
            named.unwrap_or_default(),
        )
    };
    let m = match matched {
        Some(m) if !(is_sticky && m.start != start_u16) => m,
        _ => return reset_and_null(rt),
    };
    let mstart_u16 = m.start;
    let mend_u16 = m.end;
    if is_global {
        if let InternalKind::RegExp(r) = &mut rt.obj_mut(this_id).internal_kind {
            r.last_index = mend_u16;
        }
        set_last_index_strict(rt, this_id, mend_u16 as f64)?;
    }
    let slice = |s: usize, e: usize| -> String {

        String::from_utf16_lossy(&units[s..e])
    };
    let group_units: Vec<Option<(usize, usize)>> = m.captures.clone();
    let captures_for_legacy: Vec<String> = group_units
        .iter()
        .skip(1)
        .map(|g| g.map(|(s, e)| slice(s, e)).unwrap_or_default())
        .collect();
    let last_paren = captures_for_legacy
        .iter()
        .rev()
        .find(|s| !s.is_empty())
        .cloned()
        .unwrap_or_default();
    rt.legacy_regexp_state = LegacyRegExpState::materialized(
        String::from_utf16_lossy(units),
        slice(mstart_u16, mend_u16),
        last_paren,
        slice(0, mstart_u16),
        slice(mend_u16, units.len()),
        captures_for_legacy,
    );
    let mk_str = |rt: &mut Runtime, s: usize, e: usize| {
        Value::String(Rc::new(crate::value::JsString::from_code_units(
            units[s..e].to_vec(),
        )))
    };
    let arr = rt.alloc_object(Object::new_array());
    let arr_roots = [Value::Object(this_id), Value::Object(arr)];
    let _arr_roots = rt.push_temporary_value_roots(&arr_roots);
    for (i, g) in group_units.iter().enumerate() {
        let v = match g {
            Some((s, e)) => mk_str(rt, *s, *e),
            None => Value::Undefined,
        };
        rt.object_set(arr, i.to_string(), v);
    }
    rt.object_set(
        arr,
        "length".into(),
        Value::Number(group_units.len() as f64),
    );
    rt.object_set(arr, "index".into(), Value::Number(mstart_u16 as f64));
    rt.object_set(
        arr,
        "input".into(),
        Value::String(Rc::new(crate::value::JsString::from_code_units(
            units.to_vec(),
        ))),
    );
    if !named_list.is_empty() {
        let g_obj = rt.alloc_object_with_explicit_null_proto(Object::new_ordinary());
        let group_roots = [
            Value::Object(this_id),
            Value::Object(arr),
            Value::Object(g_obj),
        ];
        let _group_roots = rt.push_temporary_value_roots(&group_roots);
        for (name, slots) in &named_list {
            let v = match slots
                .iter()
                .find_map(|idx| group_units.get(*idx).and_then(|c| *c))
            {
                Some((s, e)) => mk_str(rt, s, e),
                None => Value::Undefined,
            };
            rt.object_set(g_obj, name.clone(), v);
        }
        rt.object_set(arr, "groups".into(), Value::Object(g_obj));
    } else {
        rt.object_set(arr, "groups".into(), Value::Undefined);
    }
    if has_indices {
        let indices_arr = rt.alloc_object(Object::new_array());
        let indices_roots = [
            Value::Object(this_id),
            Value::Object(arr),
            Value::Object(indices_arr),
        ];
        let _indices_roots = rt.push_temporary_value_roots(&indices_roots);
        for (i, p) in group_units.iter().enumerate() {
            let v = match p {
                Some((s, e)) => {
                    let pair = rt.alloc_object(Object::new_array());
                    rt.object_set(pair, "0".into(), Value::Number(*s as f64));
                    rt.object_set(pair, "1".into(), Value::Number(*e as f64));
                    rt.object_set(pair, "length".into(), Value::Number(2.0));
                    Value::Object(pair)
                }
                None => Value::Undefined,
            };
            rt.object_set(indices_arr, i.to_string(), v);
        }
        rt.object_set(
            indices_arr,
            "length".into(),
            Value::Number(group_units.len() as f64),
        );
        if !named_list.is_empty() {
            let ig_obj = rt.alloc_object_with_explicit_null_proto(Object::new_ordinary());
            for (name, slots) in &named_list {
                let v = match slots
                    .iter()
                    .find_map(|idx| group_units.get(*idx).and_then(|c| *c))
                {
                    Some((s, e)) => {
                        let pair = rt.alloc_object(Object::new_array());
                        rt.object_set(pair, "0".into(), Value::Number(s as f64));
                        rt.object_set(pair, "1".into(), Value::Number(e as f64));
                        rt.object_set(pair, "length".into(), Value::Number(2.0));
                        Value::Object(pair)
                    }
                    None => Value::Undefined,
                };
                rt.object_set(ig_obj, name.clone(), v);
            }
            rt.object_set(indices_arr, "groups".into(), Value::Object(ig_obj));
        } else {
            rt.object_set(indices_arr, "groups".into(), Value::Undefined);
        }
        rt.object_set(arr, "indices".into(), Value::Object(indices_arr));
    }
    Ok(Value::Object(arr))
}

pub(crate) fn ihi_fast_regexp_exec(
    rt: &mut Runtime,
    recv: &Value,
    args: &[Value],
) -> Option<Value> {
    let Value::Object(this_id) = recv else {
        return None;
    };
    match &rt.obj(*this_id).internal_kind {
        InternalKind::RegExp(r)
            if r.compiled.is_some() && !r.flags.contains('g') && !r.flags.contains('y') => {}
        _ => return None,
    }
    let Some(Value::String(js)) = args.first() else {
        return None;
    };
    let js = js.clone();
    if regexp_result_counters_enabled() {
        REGEXP_IHI_EXEC_FAST_CALLS.fetch_add(1, Ordering::Relaxed);
    }
    regexp_exec_via_jsstring(rt, *this_id, &js).ok()
}

pub(crate) fn jit_ic_regexp_exec_via_jsstring(
    rt: &mut Runtime,
    this_id: ObjectRef,
    js: &Rc<crate::value::JsString>,
) -> Result<Value, RuntimeError> {
    regexp_exec_via_jsstring(rt, this_id, js)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum JitGlobalRegExpExecOutcome {
    Object(ObjectRef),
    NormalNull,
    Deopt,
}

impl JitGlobalRegExpExecOutcome {
    pub(crate) fn to_jit_i64(self) -> i64 {
        match self {
            Self::Object(id) => id.0 as i64,
            Self::NormalNull => rusty_js_jit::deopt::REGEXP_EXEC_NORMAL_NULL_SENTINEL,
            Self::Deopt => rusty_js_jit::deopt::REGEXP_EXEC_DEOPT_SENTINEL,
        }
    }
}

pub(crate) fn jit_ic_regexp_exec_global_object_or_null_via_jsstring(
    rt: &mut Runtime,
    this_id: ObjectRef,
    js: &Rc<crate::value::JsString>,
) -> Result<JitGlobalRegExpExecOutcome, RuntimeError> {
    let admit = {
        let object = rt.obj(this_id);
        let InternalKind::RegExp(re) = &object.internal_kind else {
            return Ok(JitGlobalRegExpExecOutcome::Deopt);
        };
        let ordinary_global = re.flags.contains('g') && !re.flags.contains('y');
        let compiled = re.compiled.is_some();
        let own_last_index_data = object
            .get_own_str_borrowed("lastIndex")
            .map(|d| {
                d.getter.is_none()
                    && d.setter.is_none()
                    && d.writable
                    && !d.enumerable
                    && !d.configurable
                    && matches!(d.value, Value::Number(_))
            })
            .unwrap_or(false);
        ordinary_global && compiled && own_last_index_data
    };
    if !admit {
        return Ok(JitGlobalRegExpExecOutcome::Deopt);
    }
    match regexp_exec_via_jsstring(rt, this_id, js)? {
        Value::Object(id) => Ok(JitGlobalRegExpExecOutcome::Object(id)),
        Value::Null => Ok(JitGlobalRegExpExecOutcome::NormalNull),
        _ => Ok(JitGlobalRegExpExecOutcome::Deopt),
    }
}

fn regexp_exec_via_jsstring(
    rt: &mut Runtime,
    this_id: ObjectRef,
    js: &Rc<crate::value::JsString>,
) -> Result<Value, RuntimeError> {
    REGEXP_EXEC_CALLS.fetch_add(1, Ordering::Relaxed);
    match &rt.obj(this_id).internal_kind {
        InternalKind::RegExp(_) => {}
        _ => {
            return Err(RuntimeError::TypeError(
                "RegExp.prototype.exec: this is not a RegExp".into(),
            ))
        }
    }

    let last_index_n = regexp_last_index_to_length(rt, this_id)?;
    if regexp_hand_faithful_needed(rt, this_id, js) {
        let (is_global, is_sticky, has_indices) = {
            let re = match &rt.obj(this_id).internal_kind {
                InternalKind::RegExp(r) => r,
                _ => unreachable!(),
            };
            let is_sticky = re.flags.contains('y');
            (
                re.flags.contains('g') || is_sticky,
                is_sticky,
                re.flags.contains('d'),
            )
        };
        let start_u16 = if is_global { last_index_n as usize } else { 0 };
        let units = js.code_units().into_owned();
        return regexp_exec_faithful(
            rt,
            this_id,
            &units,
            start_u16,
            is_global,
            is_sticky,
            has_indices,
        );
    }
    regexp_exec_after_last_index(rt, this_id, js.as_str(), Some(js.clone()), last_index_n)
}

pub fn regexp_exec(
    rt: &mut Runtime,
    this_id: ObjectRef,
    input: &str,
) -> Result<Value, RuntimeError> {
    REGEXP_EXEC_CALLS.fetch_add(1, Ordering::Relaxed);
    match &rt.obj(this_id).internal_kind {
        InternalKind::RegExp(_) => {}
        _ => {
            return Err(RuntimeError::TypeError(
                "RegExp.prototype.exec: this is not a RegExp".into(),
            ))
        }
    }

    let last_index_n = regexp_last_index_to_length(rt, this_id)?;
    regexp_exec_after_last_index(rt, this_id, input, None, last_index_n)
}

fn regexp_exec_after_last_index(
    rt: &mut Runtime,
    this_id: ObjectRef,
    input: &str,
    input_value: Option<Rc<JsString>>,
    last_index_n: f64,
) -> Result<Value, RuntimeError> {
    let (is_global, is_sticky, has_indices, has_compiled, source, flags) = {
        let o = rt.obj(this_id);
        let re = match &o.internal_kind {
            InternalKind::RegExp(r) => r,
            _ => unreachable!(),
        };
        let is_sticky = re.flags.contains('y');

        let is_global = re.flags.contains('g') || is_sticky;

        let has_indices = re.flags.contains('d');
        (
            is_global,
            is_sticky,
            has_indices,
            re.compiled.is_some(),
            (*re.source).clone(),
            (*re.flags).clone(),
        )
    };
    let start_u16: usize = if is_global { last_index_n as usize } else { 0 };

    if has_compiled
        && !flags.contains('u')
        && !flags.contains('v')
        && input_needs_codeunit_standin(input)
        && !source.chars().any(|c| (c as u32) > 0xFFFF)
    {
        return regexp_exec_codeunit_standin(
            rt,
            this_id,
            input,
            start_u16,
            is_global,
            is_sticky,
            has_indices,
        );
    }
    if !has_compiled {
        if let Some(result) =
            regexp_exec_nonunicode_low_surrogate(rt, this_id, input, &source, &flags, start_u16)?
        {
            return Ok(result);
        }
        return Err(RuntimeError::TypeError(format!(
            "RegExp pattern uses features unsupported by the v1 regex engine: /{}/{}",
            source, flags
        )));
    }
    if start_u16 > utf16_len(input) {
        if is_global {
            if let InternalKind::RegExp(r) = &mut rt.obj_mut(this_id).internal_kind {
                r.last_index = 0;
            }
            set_last_index_strict(rt, this_id, 0.0)?;
            if regexp_result_counters_enabled() {
                REGEXP_LAST_INDEX_NULL_RESETS.fetch_add(1, Ordering::Relaxed);
            }
        }
        if regexp_result_counters_enabled() {
            REGEXP_EXEC_NULL_RESULTS.fetch_add(1, Ordering::Relaxed);
        }
        return Ok(Value::Null);
    }
    let start = match utf16_to_byte(input, start_u16) {
        Some(start) => start,
        None => {
            if is_global {
                if let InternalKind::RegExp(r) = &mut rt.obj_mut(this_id).internal_kind {
                    r.last_index = 0;
                }
                set_last_index_strict(rt, this_id, 0.0)?;
                if regexp_result_counters_enabled() {
                    REGEXP_LAST_INDEX_NULL_RESETS.fetch_add(1, Ordering::Relaxed);
                }
            }
            if regexp_result_counters_enabled() {
                REGEXP_EXEC_NULL_RESULTS.fetch_add(1, Ordering::Relaxed);
            }
            return Ok(Value::Null);
        }
    };

    let captures_opt: Option<(usize, usize, Vec<Option<(usize, usize)>>)> = {
        let re = match &rt.obj(this_id).internal_kind {
            InternalKind::RegExp(r) => r,
            _ => unreachable!(),
        };
        let rx = re.compiled.as_ref().unwrap();
        if regexp_exec_time_counters_enabled() {
            let t0 = std::time::Instant::now();
            let out = rx.captures_positions_at(input, start);
            REGEXP_TIME_MATCHER_NS.fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
            out
        } else {
            rx.captures_positions_at(input, start)
        }
    };

    let captures_opt = match captures_opt {
        Some((ms, _, _)) if is_sticky && ms != start => None,
        other => other,
    };
    let captures_opt = match captures_opt {
        Some((ms, me, _))
            if simple_nonunicode_dot_match_spans_multiple_utf16_units(
                rt, this_id, input, ms, me,
            ) =>
        {
            None
        }
        other => other,
    };
    let result_build_t0 = if regexp_exec_time_counters_enabled() {
        Some(std::time::Instant::now())
    } else {
        None
    };
    let exec_result = match captures_opt {
        None => {
            if is_global {
                if let InternalKind::RegExp(r) = &mut rt.obj_mut(this_id).internal_kind {
                    r.last_index = 0;
                }
                set_last_index_strict(rt, this_id, 0.0)?;
                if regexp_result_counters_enabled() {
                    REGEXP_LAST_INDEX_NULL_RESETS.fetch_add(1, Ordering::Relaxed);
                }
            }
            if regexp_result_counters_enabled() {
                REGEXP_EXEC_NULL_RESULTS.fetch_add(1, Ordering::Relaxed);
            }
            Ok(Value::Null)
        }
        Some((mstart, mend, group_positions)) => {
            let result_counters = regexp_result_counters_enabled();

            let mend_u16 = byte_to_utf16(input, mend);
            let mstart_u16 = byte_to_utf16(input, mstart);
            if is_global {
                if let InternalKind::RegExp(r) = &mut rt.obj_mut(this_id).internal_kind {
                    r.last_index = mend_u16;
                }
                set_last_index_strict(rt, this_id, mend_u16 as f64)?;
                if result_counters {
                    REGEXP_LAST_INDEX_SUCCESS_STOREBACKS.fetch_add(1, Ordering::Relaxed);
                }
            }
            let legacy_capture_move = regexp_legacy_capture_move_enabled();
            let captures_for_legacy = if legacy_capture_move {
                None
            } else {
                Some(group_positions.iter().skip(1).copied().collect::<Vec<_>>())
            };
            let last_paren = group_positions
                .iter()
                .skip(1)
                .rev()
                .find_map(|c| c.filter(|(s, e)| e > s));
            let reused_input_value = input_value.is_some();
            let input_value =
                input_value.unwrap_or_else(|| Rc::new(crate::value::JsString::from(input)));
            if result_counters {
                if reused_input_value {
                    REGEXP_RESULT_INPUT_REUSED.fetch_add(1, Ordering::Relaxed);
                } else {
                    REGEXP_RESULT_INPUT_FALLBACK_CLONES.fetch_add(1, Ordering::Relaxed);
                }
            }
            if let Some(captures_for_legacy) = captures_for_legacy {
                if result_counters {
                    REGEXP_RESULT_LEGACY_CAPTURE_CLONES.fetch_add(1, Ordering::Relaxed);
                }
                rt.legacy_regexp_state = LegacyRegExpState::lazy_byte(
                    input_value.clone(),
                    mstart,
                    mend,
                    captures_for_legacy,
                    last_paren,
                );
            }

            let named = match &rt.obj(this_id).internal_kind {
                InternalKind::RegExp(r) => r.compiled.as_ref().map(|c| c.named_group_slots()),
                _ => None,
            };
            let result_array = if regexp_result_property_capacity_enabled() {
                Object::new_array_with_property_capacity(3)
            } else {
                Object::new_array()
            };
            let arr = rt.alloc_object(result_array);
            if result_counters {
                REGEXP_RESULT_ARRAYS.fetch_add(1, Ordering::Relaxed);
                REGEXP_RESULT_DENSE_SLOTS
                    .fetch_add(group_positions.len() as u64, Ordering::Relaxed);
            }
            let arr_roots = [Value::Object(this_id), Value::Object(arr)];
            let _arr_roots = rt.push_temporary_value_roots(&arr_roots);
            {
                let o = rt.obj_mut(arr);
                o.array_dense = true;
                if regexp_result_lazy_slots_enabled() {
                    o.regexp_result_slots = Some(Box::new(RegExpResultSlots {
                        input: input_value.clone(),
                        positions: group_positions.clone(),
                    }));
                } else {
                    let mut dense = Vec::with_capacity(group_positions.len());
                    for (slot, g) in group_positions.iter().enumerate() {
                        dense.push(match g {
                            Some((s, e)) => {
                                if result_counters {
                                    let bytes = (e - s) as u64;
                                    REGEXP_RESULT_SUBSTRINGS.fetch_add(1, Ordering::Relaxed);
                                    REGEXP_RESULT_SUBSTRING_BYTES
                                        .fetch_add(bytes, Ordering::Relaxed);
                                    if slot == 0 {
                                        REGEXP_RESULT_MATCH_SUBSTRINGS
                                            .fetch_add(1, Ordering::Relaxed);
                                        REGEXP_RESULT_MATCH_SUBSTRING_BYTES
                                            .fetch_add(bytes, Ordering::Relaxed);
                                    } else {
                                        REGEXP_RESULT_CAPTURE_SUBSTRINGS
                                            .fetch_add(1, Ordering::Relaxed);
                                        REGEXP_RESULT_CAPTURE_SUBSTRING_BYTES
                                            .fetch_add(bytes, Ordering::Relaxed);
                                    }
                                }
                                let js = if regexp_result_slices_enabled() {
                                    crate::value::JsString::slice_wellformed(
                                        input_value.clone(),
                                        *s,
                                        *e,
                                    )
                                } else {
                                    None
                                };
                                match js {
                                    Some(slice) => {
                                        if result_counters {
                                            REGEXP_RESULT_SLICE_STRINGS
                                                .fetch_add(1, Ordering::Relaxed);
                                        }
                                        Value::String(Rc::new(slice))
                                    }
                                    None => {
                                        if result_counters {
                                            REGEXP_RESULT_OWNED_STRINGS
                                                .fetch_add(1, Ordering::Relaxed);
                                        }
                                        Value::String(Rc::new(crate::value::JsString::from(
                                            input[*s..*e].to_string(),
                                        )))
                                    }
                                }
                            }
                            None => {
                                if result_counters {
                                    REGEXP_RESULT_UNDEFINED_SLOTS.fetch_add(1, Ordering::Relaxed);
                                }
                                Value::Undefined
                            }
                        });
                    }
                    o.dense_elements = dense;
                }
            }

            regexp_result_set_own(rt, arr, "index", Value::Number(mstart_u16 as f64));
            regexp_result_set_own(rt, arr, "input", Value::String(input_value.clone()));

            let named_list = named.unwrap_or_default();
            if !named_list.is_empty() {
                let g_obj = rt.alloc_object_with_explicit_null_proto(Object::new_ordinary());
                let group_roots = [
                    Value::Object(this_id),
                    Value::Object(arr),
                    Value::Object(g_obj),
                ];
                let _group_roots = rt.push_temporary_value_roots(&group_roots);
                for (name, slots) in &named_list {
                    let v = slots
                        .iter()
                        .find_map(|idx| group_positions.get(*idx).and_then(|g| *g))
                        .map(|(s, e)| {
                            if result_counters {
                                REGEXP_RESULT_NAMED_GROUP_STRINGS.fetch_add(1, Ordering::Relaxed);
                                REGEXP_RESULT_NAMED_GROUP_BYTES
                                    .fetch_add((e - s) as u64, Ordering::Relaxed);
                            }
                            Value::String(Rc::new(crate::value::JsString::from(
                                input[s..e].to_string(),
                            )))
                        })
                        .unwrap_or(Value::Undefined);
                    rt.object_set(g_obj, name.clone(), v);
                }
                regexp_result_set_own(rt, arr, "groups", Value::Object(g_obj));
            } else {
                regexp_result_set_own(rt, arr, "groups", Value::Undefined);
            }

            if has_indices {
                let indices_arr = rt.alloc_object(Object::new_array());
                if result_counters {
                    REGEXP_RESULT_INDICES_ARRAYS.fetch_add(1, Ordering::Relaxed);
                }
                let indices_roots = [
                    Value::Object(this_id),
                    Value::Object(arr),
                    Value::Object(indices_arr),
                ];
                let _indices_roots = rt.push_temporary_value_roots(&indices_roots);
                {
                    for (i, p) in group_positions.iter().enumerate() {
                        let v = match p {
                            Some((s, e)) => {
                                let pair = rt.alloc_object(Object::new_array());
                                let pair_roots = [
                                    Value::Object(this_id),
                                    Value::Object(arr),
                                    Value::Object(indices_arr),
                                    Value::Object(pair),
                                ];
                                let _pair_roots = rt.push_temporary_value_roots(&pair_roots);
                                rt.object_set(
                                    pair,
                                    "0".into(),
                                    Value::Number(byte_to_utf16(input, *s) as f64),
                                );
                                rt.object_set(
                                    pair,
                                    "1".into(),
                                    Value::Number(byte_to_utf16(input, *e) as f64),
                                );
                                rt.object_set(pair, "length".into(), Value::Number(2.0));
                                Value::Object(pair)
                            }
                            None => Value::Undefined,
                        };
                        rt.object_set(indices_arr, i.to_string(), v);
                    }
                    rt.object_set(
                        indices_arr,
                        "length".into(),
                        Value::Number(group_positions.len() as f64),
                    );

                    if !named_list.is_empty() {
                        let ig_obj =
                            rt.alloc_object_with_explicit_null_proto(Object::new_ordinary());
                        let index_group_roots = [
                            Value::Object(this_id),
                            Value::Object(arr),
                            Value::Object(indices_arr),
                            Value::Object(ig_obj),
                        ];
                        let _index_group_roots = rt.push_temporary_value_roots(&index_group_roots);
                        for (name, slots) in &named_list {
                            let v = match slots
                                .iter()
                                .find_map(|idx| group_positions.get(*idx).and_then(|c| c.as_ref()))
                            {
                                Some((s, e)) => {
                                    let pair = rt.alloc_object(Object::new_array());
                                    let pair_roots = [
                                        Value::Object(this_id),
                                        Value::Object(arr),
                                        Value::Object(indices_arr),
                                        Value::Object(ig_obj),
                                        Value::Object(pair),
                                    ];
                                    let _pair_roots = rt.push_temporary_value_roots(&pair_roots);
                                    rt.object_set(
                                        pair,
                                        "0".into(),
                                        Value::Number(byte_to_utf16(input, *s) as f64),
                                    );
                                    rt.object_set(
                                        pair,
                                        "1".into(),
                                        Value::Number(byte_to_utf16(input, *e) as f64),
                                    );
                                    rt.object_set(pair, "length".into(), Value::Number(2.0));
                                    Value::Object(pair)
                                }
                                None => Value::Undefined,
                            };
                            rt.object_set(ig_obj, name.clone(), v);
                        }
                        rt.object_set(indices_arr, "groups".into(), Value::Object(ig_obj));
                    } else {
                        rt.object_set(indices_arr, "groups".into(), Value::Undefined);
                    }
                }
                rt.object_set(arr, "indices".into(), Value::Object(indices_arr));
            }
            if legacy_capture_move {
                if result_counters {
                    REGEXP_RESULT_LEGACY_CAPTURE_MOVES.fetch_add(1, Ordering::Relaxed);
                }
                rt.legacy_regexp_state = LegacyRegExpState::lazy_byte_with_capture_offset(
                    input_value,
                    mstart,
                    mend,
                    group_positions,
                    1,
                    last_paren,
                );
            }
            if result_counters {
                let successes = REGEXP_RESULT_SUCCESSES.fetch_add(1, Ordering::Relaxed) + 1;
                maybe_report_regexp_result_counters(successes);
            }
            Ok(Value::Object(arr))
        }
    };
    if let Some(t0) = result_build_t0 {
        REGEXP_TIME_RESULT_NS.fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
    }
    exec_result
}

fn regexp_result_set_own(rt: &mut Runtime, arr: ObjectRef, key: &str, value: Value) {
    rt.obj_mut(arr).set_own_literal_key(key, value);
}

fn simple_nonunicode_dot_match_spans_multiple_utf16_units(
    rt: &Runtime,
    this_id: ObjectRef,
    input: &str,
    start: usize,
    end: usize,
) -> bool {
    let (source, flags) = match &rt.obj(this_id).internal_kind {
        InternalKind::RegExp(re) => (re.source.as_str(), re.flags.as_str()),
        _ => return false,
    };
    if source != "^.$" || flags.contains('u') || flags.contains('v') {
        return false;
    }
    utf16_len(&input[start..end]) != 1
}

fn byte_to_char_index(s: &str, byte_off: usize) -> usize {
    s[..byte_off.min(s.len())].chars().count()
}

fn current_regexp_this(rt: &Runtime, label: &str) -> Result<ObjectRef, RuntimeError> {
    match rt.current_this() {
        Value::Object(id) if matches!(rt.obj(id).internal_kind, InternalKind::RegExp(_)) => Ok(id),
        _ => Err(RuntimeError::TypeError(format!(
            "{}: this is not a RegExp",
            label
        ))),
    }
}

fn current_object_this(rt: &Runtime, label: &str) -> Result<ObjectRef, RuntimeError> {
    match rt.current_this() {
        Value::Object(id) => Ok(id),
        _ => Err(RuntimeError::TypeError(format!(
            "{}: this is not an Object",
            label
        ))),
    }
}

fn regexp_flags_from_properties(rt: &mut Runtime, this: ObjectRef) -> Result<String, RuntimeError> {
    let mut flags = String::new();
    for (prop, ch) in [
        ("hasIndices", 'd'),
        ("global", 'g'),
        ("ignoreCase", 'i'),
        ("multiline", 'm'),
        ("dotAll", 's'),
        ("unicode", 'u'),
        ("unicodeSets", 'v'),
        ("sticky", 'y'),
    ] {
        if abstract_ops::to_boolean(&rt.spec_get(&Value::Object(this), prop)?) {
            flags.push(ch);
        }
    }
    Ok(flags)
}

fn regexp_constructor_can_return_pattern(rt: &Runtime, id: ObjectRef) -> bool {
    let mut cur = id;
    let mut hops = 0;
    loop {
        let obj = rt.obj(cur);
        match &obj.internal_kind {
            InternalKind::RegExp(_) => return obj.proto == rt.regexp_prototype,
            InternalKind::Proxy(p) if !p.revoked && hops < 16 => {
                cur = p.target;
                hops += 1;
            }
            _ => return true,
        }
    }
}

fn regexp_new_target_prototype(rt: &mut Runtime) -> Result<Option<ObjectRef>, RuntimeError> {
    let Some(Value::Object(nt)) = rt.current_new_target.clone() else {
        return Ok(None);
    };
    let Some(fallback) = rt.regexp_prototype else {
        return Ok(None);
    };
    let _proto_roots = rt.push_temporary_value_roots(&[Value::Object(nt)]);
    rt.get_prototype_from_constructor(nt, |rr| rr.regexp_prototype, fallback)
        .map(Some)
}

pub(crate) fn regexp_exec_generic_object(
    rt: &mut Runtime,
    this_id: ObjectRef,
    input: &str,
) -> Result<Value, RuntimeError> {
    let this_v = Value::Object(this_id);
    let _this_roots = rt.push_temporary_value_roots(std::slice::from_ref(&this_v));
    let exec = rt.read_property(this_id, "exec")?;
    if rt.is_callable(&exec) {
        let exec_roots = [
            this_v.clone(),
            exec.clone(),
            Value::String(Rc::new(crate::value::JsString::from(input.to_string()))),
        ];
        let _exec_roots = rt.push_temporary_value_roots(&exec_roots);
        let result = rt.call_function(
            exec,
            Value::Object(this_id),
            vec![Value::String(Rc::new(crate::value::JsString::from(
                input.to_string(),
            )))],
        )?;
        if matches!(result, Value::Null | Value::Object(_)) {
            return Ok(result);
        }
        return Err(RuntimeError::TypeError(
            "RegExp exec result is not an Object or null".into(),
        ));
    }
    regexp_exec(rt, this_id, input)
}

fn regexp_match_generic_object(
    rt: &mut Runtime,
    this_id: ObjectRef,
    input: &str,
) -> Result<Value, RuntimeError> {
    let this_v = Value::Object(this_id);
    let _receiver_roots = rt.push_temporary_value_roots(std::slice::from_ref(&this_v));
    let flags_v = rt.read_property(this_id, "flags")?;
    let _flags_roots = rt.push_temporary_value_roots(&[this_v.clone(), flags_v.clone()]);
    let flags = rt.coerce_to_string(&flags_v)?;
    let global = flags.contains('g');
    let unicode = flags.contains('u') || flags.contains('v');
    if !global {
        let _exec_roots = rt.push_temporary_value_roots(std::slice::from_ref(&this_v));
        return regexp_exec_generic_object(rt, this_id, input);
    }
    let _set_roots = rt.push_temporary_value_roots(std::slice::from_ref(&this_v));
    set_last_index_strict(rt, this_id, 0.0)?;
    let arr = rt.alloc_object(Object::new_array());
    let arr_v = Value::Object(arr);
    let mut len = 0usize;
    loop {
        let _loop_roots = rt.push_temporary_value_roots(&[this_v.clone(), arr_v.clone()]);
        let result = regexp_exec_generic_object(rt, this_id, input)?;
        if matches!(result, Value::Null) {
            break;
        }
        let _result_roots =
            rt.push_temporary_value_roots(&[this_v.clone(), arr_v.clone(), result.clone()]);
        let result_id = match result {
            Value::Object(id) => id,
            _ => {
                return Err(RuntimeError::TypeError(
                    "RegExp generic exec result is not an Object or null".into(),
                ))
            }
        };
        let match_v = rt.spec_get(&Value::Object(result_id), "0")?;
        let _match_roots = rt.push_temporary_value_roots(&[
            this_v.clone(),
            arr_v.clone(),
            Value::Object(result_id),
            match_v.clone(),
        ]);
        let match_s = rt.coerce_to_string(&match_v)?;
        let stored = Value::String(Rc::new(crate::value::JsString::from(match_s.clone())));
        let _write_roots = rt.push_temporary_value_roots(&[
            this_v.clone(),
            arr_v.clone(),
            Value::Object(result_id),
            stored.clone(),
        ]);
        rt.object_set(arr, len.to_string(), stored);
        len += 1;
        if match_s.is_empty() {
            let _last_index_roots = rt.push_temporary_value_roots(&[this_v.clone(), arr_v.clone()]);
            let last_index_v = rt.read_property(this_id, "lastIndex")?;
            let _last_index_value_roots = rt.push_temporary_value_roots(&[
                this_v.clone(),
                arr_v.clone(),
                last_index_v.clone(),
            ]);
            let current = rt.coerce_to_number(&last_index_v)?.max(0.0) as usize;
            let next = advance_string_index_utf16(input, current, unicode) as f64;
            let _set_last_index_roots =
                rt.push_temporary_value_roots(&[this_v.clone(), arr_v.clone()]);
            set_last_index_strict(rt, this_id, next)?;
        }
    }
    if len == 0 {
        return Ok(Value::Null);
    }
    let _length_roots = rt.push_temporary_value_roots(std::slice::from_ref(&arr_v));
    rt.object_set(arr, "length".into(), Value::Number(len as f64));
    Ok(Value::Object(arr))
}

fn regexp_search_generic_object(
    rt: &mut Runtime,
    this_id: ObjectRef,
    input: &str,
) -> Result<Value, RuntimeError> {
    let previous_last_index = rt.read_property(this_id, "lastIndex")?;
    let zero = Value::Number(0.0);
    if !abstract_ops::same_value(&previous_last_index, &zero) {
        set_last_index_value_strict(rt, this_id, zero)?;
    }
    let result = regexp_exec_generic_object(rt, this_id, input)?;
    let _result_root = rt.push_temporary_value_roots(std::slice::from_ref(&result));
    let current_last_index = rt.read_property(this_id, "lastIndex")?;
    if !abstract_ops::same_value(&current_last_index, &previous_last_index) {
        set_last_index_value_strict(rt, this_id, previous_last_index)?;
    }
    if matches!(result, Value::Null) {
        return Ok(Value::Number(-1.0));
    }
    if !matches!(result, Value::Object(_)) {
        return Err(RuntimeError::TypeError(
            "RegExp generic exec result is not an Object or null".into(),
        ));
    }

    let index = rt.spec_get(&result, "index")?;
    Ok(Value::Number(rt.coerce_to_number(&index)?))
}

fn regexp_match_all_protocol_object(
    rt: &mut Runtime,
    this_id: ObjectRef,
    input: &str,
) -> Result<Value, RuntimeError> {
    let this_v = Value::Object(this_id);
    let default_ctor = rt.global_get("RegExp");
    let _species_roots = rt.push_temporary_value_roots(&[this_v.clone(), default_ctor.clone()]);
    let c = rt.species_constructor(&Value::Object(this_id), default_ctor)?;
    let _flags_roots = rt.push_temporary_value_roots(&[this_v.clone(), c.clone()]);
    let flags_v = rt.read_property(this_id, "flags")?;
    let _flags_value_roots =
        rt.push_temporary_value_roots(&[this_v.clone(), c.clone(), flags_v.clone()]);
    let flags = rt.coerce_to_string(&flags_v)?;
    let global = flags.contains('g');
    let unicode = flags.contains('u') || flags.contains('v');
    let flags_arg = Value::String(Rc::new(crate::value::JsString::from(flags.clone())));
    let _construct_roots =
        rt.push_temporary_value_roots(&[this_v.clone(), c.clone(), flags_arg.clone()]);
    let matcher = rt.construct(c, vec![Value::Object(this_id), flags_arg])?;
    let _matcher_roots = rt.push_temporary_value_roots(&[this_v.clone(), matcher.clone()]);
    let matcher_id = match matcher {
        Value::Object(id) => id,
        _ => {
            return Err(RuntimeError::TypeError(
                "RegExp.prototype[@@matchAll]: species constructor did not return object".into(),
            ))
        }
    };
    let _last_index_roots =
        rt.push_temporary_value_roots(&[this_v.clone(), Value::Object(matcher_id)]);
    let last_index_v = rt.read_property(this_id, "lastIndex")?;
    let _last_index_value_roots = rt.push_temporary_value_roots(&[
        this_v.clone(),
        Value::Object(matcher_id),
        last_index_v.clone(),
    ]);
    let last_index = regexp_to_length(rt, &last_index_v)?;
    let _set_last_index_roots =
        rt.push_temporary_value_roots(&[this_v.clone(), Value::Object(matcher_id)]);
    set_last_index_strict(rt, matcher_id, last_index)?;
    let _iterator_roots = rt.push_temporary_value_roots(&[this_v, Value::Object(matcher_id)]);

    Ok(Value::Object(crate::iterator::make_regexp_string_iterator(
        rt, matcher_id, input, global, unicode,
    )))
}

fn regexp_replace_generic_object(
    rt: &mut Runtime,
    this_id: ObjectRef,
    input: &str,
    replacement: Value,
) -> Result<Value, RuntimeError> {
    let functional_replace = rt.is_callable(&replacement);
    let replace_string = if functional_replace {
        None
    } else {
        Some(rt.coerce_to_string(&replacement)?)
    };
    let flags_v = rt.read_property(this_id, "flags")?;
    let flags = rt.coerce_to_string(&flags_v)?;
    let global = flags.contains('g');
    let unicode = flags.contains('u') || flags.contains('v');

    if global {
        set_last_index_strict(rt, this_id, 0.0)?;
    }

    let mut results: Vec<ObjectRef> = Vec::new();
    loop {
        let loop_roots = regexp_replace_roots(this_id, &replacement, &results, &[]);
        let _loop_roots = rt.push_temporary_value_roots(&loop_roots);
        let result = regexp_exec_generic_object(rt, this_id, input)?;
        let _result_roots = rt.push_temporary_value_roots(std::slice::from_ref(&result));
        if matches!(result, Value::Null) {
            break;
        }
        let result_id = match result {
            Value::Object(id) => id,
            _ => {
                return Err(RuntimeError::TypeError(
                    "RegExp generic exec result is not an Object or null".into(),
                ))
            }
        };
        results.push(result_id);
        let result_roots = regexp_replace_roots(this_id, &replacement, &results, &[]);
        let _result_roots = rt.push_temporary_value_roots(&result_roots);
        if !global {
            break;
        }
        let matched_v = rt.spec_get(&Value::Object(result_id), "0")?;
        let matched_roots =
            regexp_replace_roots(this_id, &replacement, &results, &[matched_v.clone()]);
        let _matched_roots = rt.push_temporary_value_roots(&matched_roots);
        let matched = rt.coerce_to_string(&matched_v)?;
        if matched.is_empty() {
            let last_index_v = rt.read_property(this_id, "lastIndex")?;
            let last_index_roots =
                regexp_replace_roots(this_id, &replacement, &results, &[last_index_v.clone()]);
            let _last_index_roots = rt.push_temporary_value_roots(&last_index_roots);
            let current = regexp_to_length(rt, &last_index_v)? as usize;
            let next = advance_string_index_utf16(input, current, unicode) as f64;
            set_last_index_strict(rt, this_id, next)?;
        }
    }

    if results.is_empty() {
        return Ok(Value::String(Rc::new(crate::value::JsString::from(
            input.to_string(),
        ))));
    }

    let mut accumulated = String::new();
    let mut next_source_position = 0usize;
    let input_len_u16 = utf16_len(input);
    for &result_id in &results {
        let result_roots = regexp_replace_roots(this_id, &replacement, &results, &[]);
        let _result_roots = rt.push_temporary_value_roots(&result_roots);

        let length_v = rt.spec_get(&Value::Object(result_id), "length")?;
        let length_roots =
            regexp_replace_roots(this_id, &replacement, &results, &[length_v.clone()]);
        let _length_roots = rt.push_temporary_value_roots(&length_roots);
        let captures_len = rt.coerce_to_number(&length_v)?.max(0.0) as usize;
        let matched_v = rt.spec_get(&Value::Object(result_id), "0")?;
        let matched_roots =
            regexp_replace_roots(this_id, &replacement, &results, &[matched_v.clone()]);
        let _matched_roots = rt.push_temporary_value_roots(&matched_roots);
        let matched = rt.coerce_to_string(&matched_v)?;
        let index_v = rt.spec_get(&Value::Object(result_id), "index")?;
        let index_roots = regexp_replace_roots(this_id, &replacement, &results, &[index_v.clone()]);
        let _index_roots = rt.push_temporary_value_roots(&index_roots);
        let position_u16 = rt
            .coerce_to_number(&index_v)?
            .max(0.0)
            .min(input_len_u16 as f64) as usize;
        let matched_len_u16 = utf16_len(&matched);
        let end_u16 = (position_u16 + matched_len_u16).min(input_len_u16);
        let mut captures: Vec<Option<String>> = Vec::new();
        for n in 1..captures_len {
            let capture_v = rt.spec_get(&Value::Object(result_id), &n.to_string())?;
            let capture_roots =
                regexp_replace_roots(this_id, &replacement, &results, &[capture_v.clone()]);
            let _capture_roots = rt.push_temporary_value_roots(&capture_roots);
            if matches!(capture_v, Value::Undefined) {
                captures.push(None);
            } else {
                captures.push(Some(rt.coerce_to_string(&capture_v)?));
            }
        }
        let groups_v = rt.spec_get(&Value::Object(result_id), "groups")?;
        let groups_roots =
            regexp_replace_roots(this_id, &replacement, &results, &[groups_v.clone()]);
        let _groups_roots = rt.push_temporary_value_roots(&groups_roots);
        let named_captures = if matches!(groups_v, Value::Undefined) {
            None
        } else if functional_replace {
            None
        } else {
            match rt.to_object(&groups_v)? {
                Value::Object(id) => Some(id),
                _ => None,
            }
        };
        let replacement_text = if functional_replace {
            let mut call_args = Vec::new();
            call_args.push(Value::String(Rc::new(crate::value::JsString::from(
                matched.clone(),
            ))));
            for capture in &captures {
                call_args.push(match capture {
                    Some(s) => Value::String(Rc::new(crate::value::JsString::from(s.clone()))),
                    None => Value::Undefined,
                });
            }
            call_args.push(Value::Number(position_u16 as f64));
            call_args.push(Value::String(Rc::new(crate::value::JsString::from(
                input.to_string(),
            ))));
            if !matches!(groups_v, Value::Undefined) {
                call_args.push(groups_v.clone());
            }
            let mut call_roots = regexp_replace_roots(this_id, &replacement, &results, &call_args);
            call_roots.push(groups_v.clone());
            let _call_roots = rt.push_temporary_value_roots(&call_roots);
            let r = rt.call_function(replacement.clone(), Value::Undefined, call_args)?;
            let r_roots = regexp_replace_roots(this_id, &replacement, &results, &[r.clone()]);
            let _r_roots = rt.push_temporary_value_roots(&r_roots);
            rt.coerce_to_string(&r)?
        } else {
            let before = substring_utf16_lossy(input, 0, position_u16);
            let after = substring_utf16_lossy(input, end_u16, input_len_u16);
            let capture_refs: Vec<Option<&str>> = captures.iter().map(|s| s.as_deref()).collect();
            let mut substitution_extra = vec![groups_v.clone()];
            if let Some(id) = named_captures {
                substitution_extra.push(Value::Object(id));
            }
            let substitution_roots =
                regexp_replace_roots(this_id, &replacement, &results, &substitution_extra);
            let _substitution_roots = rt.push_temporary_value_roots(&substitution_roots);
            process_regex_substitution_via(
                rt,
                replace_string.as_deref().unwrap_or(""),
                &matched,
                &before,
                &after,
                &capture_refs,
                named_captures,
            )?
        };
        if position_u16 >= next_source_position {
            regex_replacement_push_checked(
                &mut accumulated,
                &substring_utf16_lossy(input, next_source_position, position_u16),
            )?;
            regex_replacement_push_checked(&mut accumulated, &replacement_text)?;
            next_source_position = end_u16;
        }
    }
    regex_replacement_push_checked(
        &mut accumulated,
        &substring_utf16_lossy(input, next_source_position, input_len_u16),
    )?;
    Ok(Value::String(Rc::new(crate::value::JsString::from(
        accumulated,
    ))))
}

fn regexp_replace_roots(
    this_id: ObjectRef,
    replacement: &Value,
    results: &[ObjectRef],
    extra: &[Value],
) -> Vec<Value> {
    let mut roots = Vec::with_capacity(2 + results.len() + extra.len());
    roots.push(Value::Object(this_id));
    roots.push(replacement.clone());
    roots.extend(results.iter().copied().map(Value::Object));
    roots.extend(extra.iter().cloned());
    roots
}

fn regexp_to_uint32(rt: &mut Runtime, v: &Value) -> Result<u32, RuntimeError> {
    let n = rt.coerce_to_number(v)?;
    if n.is_nan() || n == 0.0 || !n.is_finite() {
        return Ok(0);
    }
    let int = n.trunc();
    let two32 = 4294967296.0_f64;
    let mut wrapped = int % two32;
    if wrapped < 0.0 {
        wrapped += two32;
    }
    Ok(wrapped as u32)
}

fn substring_utf16(s: &str, start: usize, end: usize) -> String {
    let start_b = utf16_to_byte(s, start).unwrap_or(s.len());
    let end_b = utf16_to_byte(s, end).unwrap_or(s.len());
    s[start_b.min(s.len())..end_b.min(s.len())].to_string()
}

fn push_split_part(
    rt: &mut Runtime,
    arr: ObjectRef,
    length: &mut usize,
    v: Value,
    limit: usize,
) -> bool {
    if *length >= limit {
        return false;
    }
    let _roots = rt.push_temporary_value_roots(&[Value::Object(arr), v.clone()]);
    rt.object_set(arr, length.to_string(), v);
    *length += 1;
    true
}

fn regexp_split_roots(
    this_id: ObjectRef,
    splitter: Option<ObjectRef>,
    arr: Option<ObjectRef>,
    extra: &[Value],
) -> Vec<Value> {
    let mut roots =
        Vec::with_capacity(1 + splitter.is_some() as usize + arr.is_some() as usize + extra.len());
    roots.push(Value::Object(this_id));
    if let Some(id) = splitter {
        roots.push(Value::Object(id));
    }
    if let Some(id) = arr {
        roots.push(Value::Object(id));
    }
    roots.extend(extra.iter().cloned());
    roots
}

fn regexp_split_protocol_object(
    rt: &mut Runtime,
    this_id: ObjectRef,
    input: &str,

    units: &[u16],
    limit: Value,
) -> Result<Value, RuntimeError> {
    let default_ctor = rt.global_get("RegExp");
    let _ctor_roots = rt.push_temporary_value_roots(&[
        Value::Object(this_id),
        default_ctor.clone(),
        limit.clone(),
    ]);
    let c = rt.species_constructor(&Value::Object(this_id), default_ctor)?;
    let _species_roots =
        rt.push_temporary_value_roots(&[Value::Object(this_id), c.clone(), limit.clone()]);
    let flags_v = rt.read_property(this_id, "flags")?;
    let _flags_roots = rt.push_temporary_value_roots(&[
        Value::Object(this_id),
        c.clone(),
        flags_v.clone(),
        limit.clone(),
    ]);
    let flags = rt.coerce_to_string(&flags_v)?;
    let new_flags = if flags.contains('y') {
        flags.clone()
    } else {
        format!("{}y", flags)
    };
    let unicode = flags.contains('u') || flags.contains('v');
    let new_flags_v = Value::String(Rc::new(crate::value::JsString::from(new_flags.clone())));
    let _construct_roots = rt.push_temporary_value_roots(&[
        Value::Object(this_id),
        c.clone(),
        new_flags_v.clone(),
        limit.clone(),
    ]);
    let splitter = rt.construct(c, vec![Value::Object(this_id), new_flags_v])?;
    let splitter_id = match splitter {
        Value::Object(id) => id,
        _ => {
            return Err(RuntimeError::TypeError(
                "RegExp.prototype[@@split]: species constructor did not return object".into(),
            ))
        }
    };
    let _splitter_roots = rt.push_temporary_value_roots(&[
        Value::Object(this_id),
        Value::Object(splitter_id),
        limit.clone(),
    ]);
    let lim = if matches!(limit, Value::Undefined) {
        u32::MAX as usize
    } else {
        regexp_to_uint32(rt, &limit)? as usize
    };
    let arr = rt.alloc_object(Object::new_array());
    let _split_roots = rt.push_temporary_value_roots(&[
        Value::Object(this_id),
        Value::Object(splitter_id),
        Value::Object(arr),
    ]);
    let mut length = 0usize;
    if lim == 0 {
        let len_v = Value::Number(0.0);
        let _roots = rt.push_temporary_value_roots(&[Value::Object(arr), len_v.clone()]);
        rt.object_set(arr, "length".into(), Value::Number(0.0));
        return Ok(Value::Object(arr));
    }
    let size = input.encode_utf16().count();
    if size == 0 {
        let roots = regexp_split_roots(this_id, Some(splitter_id), Some(arr), &[]);
        let _roots = rt.push_temporary_value_roots(&roots);
        let result = regexp_exec_generic_object(rt, splitter_id, input)?;
        let roots = regexp_split_roots(this_id, Some(splitter_id), Some(arr), &[result.clone()]);
        let _roots = rt.push_temporary_value_roots(&roots);
        if matches!(result, Value::Null) {
            push_split_part(
                rt,
                arr,
                &mut length,
                Value::String(Rc::new(crate::value::JsString::from(String::new()))),
                lim,
            );
        }
        let len_v = Value::Number(length as f64);
        let _roots = rt.push_temporary_value_roots(&[Value::Object(arr), len_v.clone()]);
        rt.object_set(arr, "length".into(), len_v);
        return Ok(Value::Object(arr));
    }

    let mut p = 0usize;
    let mut q = 0usize;
    while q < size {
        let q_v = Value::Number(q as f64);
        let _roots = rt.push_temporary_value_roots(&[
            Value::Object(this_id),
            Value::Object(splitter_id),
            Value::Object(arr),
            q_v.clone(),
        ]);
        set_last_index_strict(rt, splitter_id, q as f64)?;
        let roots = regexp_split_roots(this_id, Some(splitter_id), Some(arr), &[]);
        let _roots = rt.push_temporary_value_roots(&roots);
        let z = regexp_exec_generic_object(rt, splitter_id, input)?;
        let roots = regexp_split_roots(this_id, Some(splitter_id), Some(arr), &[z.clone()]);
        let _roots = rt.push_temporary_value_roots(&roots);
        if matches!(z, Value::Null) {
            q = advance_string_index_utf16(input, q, unicode);
            continue;
        }
        let z_id = match z {
            Value::Object(id) => id,
            _ => unreachable!(),
        };
        let roots = regexp_split_roots(
            this_id,
            Some(splitter_id),
            Some(arr),
            &[Value::Object(z_id)],
        );
        let _roots = rt.push_temporary_value_roots(&roots);
        let e_v = rt.read_property(splitter_id, "lastIndex")?;
        let roots = regexp_split_roots(
            this_id,
            Some(splitter_id),
            Some(arr),
            &[Value::Object(z_id), e_v.clone()],
        );
        let _roots = rt.push_temporary_value_roots(&roots);
        let mut e = regexp_to_length(rt, &e_v)? as usize;
        if e > size {
            e = size;
        }
        if e == p {
            q = advance_string_index_utf16(input, q, unicode);
            continue;
        }
        let part = Value::String(Rc::new(crate::value::JsString::from_code_units(
            units[p.min(units.len())..q.min(units.len())].to_vec(),
        )));
        let roots = regexp_split_roots(
            this_id,
            Some(splitter_id),
            Some(arr),
            &[Value::Object(z_id), part.clone()],
        );
        let _roots = rt.push_temporary_value_roots(&roots);
        if !push_split_part(rt, arr, &mut length, part, lim) {
            let len_v = Value::Number(length as f64);
            let _roots = rt.push_temporary_value_roots(&[Value::Object(arr), len_v.clone()]);
            rt.object_set(arr, "length".into(), len_v);
            return Ok(Value::Object(arr));
        }
        p = e;
        let number_of_captures = {
            let roots = regexp_split_roots(
                this_id,
                Some(splitter_id),
                Some(arr),
                &[Value::Object(z_id)],
            );
            let _roots = rt.push_temporary_value_roots(&roots);
            let len_v = rt.spec_get(&Value::Object(z_id), "length")?;
            let roots = regexp_split_roots(
                this_id,
                Some(splitter_id),
                Some(arr),
                &[Value::Object(z_id), len_v.clone()],
            );
            let _roots = rt.push_temporary_value_roots(&roots);
            rt.coerce_to_number(&len_v)?.max(0.0) as usize
        };
        for i in 1..number_of_captures {
            let roots = regexp_split_roots(
                this_id,
                Some(splitter_id),
                Some(arr),
                &[Value::Object(z_id)],
            );
            let _roots = rt.push_temporary_value_roots(&roots);
            let cap = rt.spec_get(&Value::Object(z_id), &i.to_string())?;
            let roots = regexp_split_roots(
                this_id,
                Some(splitter_id),
                Some(arr),
                &[Value::Object(z_id), cap.clone()],
            );
            let _roots = rt.push_temporary_value_roots(&roots);
            if !push_split_part(rt, arr, &mut length, cap, lim) {
                let len_v = Value::Number(length as f64);
                let _roots = rt.push_temporary_value_roots(&[Value::Object(arr), len_v.clone()]);
                rt.object_set(arr, "length".into(), len_v);
                return Ok(Value::Object(arr));
            }
        }
        q = p;
    }
    let tail = Value::String(Rc::new(crate::value::JsString::from_code_units(
        units[p.min(units.len())..size.min(units.len())].to_vec(),
    )));
    let roots = regexp_split_roots(this_id, Some(splitter_id), Some(arr), &[tail.clone()]);
    let _roots = rt.push_temporary_value_roots(&roots);
    push_split_part(rt, arr, &mut length, tail, lim);
    let len_v = Value::Number(length as f64);
    let _roots = rt.push_temporary_value_roots(&[Value::Object(arr), len_v.clone()]);
    rt.object_set(arr, "length".into(), len_v);
    Ok(Value::Object(arr))
}

fn regexp_split_generic_object(
    rt: &mut Runtime,
    this_id: ObjectRef,
    input: &str,
    limit: usize,
) -> Result<Value, RuntimeError> {
    let arr = rt.alloc_object(Object::new_array());
    let _roots = rt.push_temporary_value_roots(&[Value::Object(this_id), Value::Object(arr)]);
    if limit == 0 {
        let len_v = Value::Number(0.0);
        let _roots = rt.push_temporary_value_roots(&[Value::Object(arr), len_v.clone()]);
        rt.object_set(arr, "length".into(), len_v);
        return Ok(Value::Object(arr));
    }
    let _roots = rt.push_temporary_value_roots(&[Value::Object(this_id), Value::Object(arr)]);
    let result = regexp_exec_generic_object(rt, this_id, input)?;
    let _roots = rt.push_temporary_value_roots(&[
        Value::Object(this_id),
        Value::Object(arr),
        result.clone(),
    ]);
    if matches!(result, Value::Null) {
        let part = Value::String(Rc::new(crate::value::JsString::from(input.to_string())));
        let _roots = rt.push_temporary_value_roots(&[Value::Object(arr), part.clone()]);
        rt.object_set(arr, "0".into(), part);
        let len_v = Value::Number(1.0);
        let _roots = rt.push_temporary_value_roots(&[Value::Object(arr), len_v.clone()]);
        rt.object_set(arr, "length".into(), len_v);
        return Ok(Value::Object(arr));
    }
    let result_id = match result {
        Value::Object(id) => id,
        _ => {
            return Err(RuntimeError::TypeError(
                "RegExp generic exec result is not an Object or null".into(),
            ))
        }
    };
    let _roots = rt.push_temporary_value_roots(&[
        Value::Object(this_id),
        Value::Object(arr),
        Value::Object(result_id),
    ]);
    let matched_v = rt.spec_get(&Value::Object(result_id), "0")?;
    let _roots = rt.push_temporary_value_roots(&[
        Value::Object(this_id),
        Value::Object(arr),
        Value::Object(result_id),
        matched_v.clone(),
    ]);
    let matched = rt.coerce_to_string(&matched_v)?;
    let _roots = rt.push_temporary_value_roots(&[
        Value::Object(this_id),
        Value::Object(arr),
        Value::Object(result_id),
    ]);
    let index_v = rt.spec_get(&Value::Object(result_id), "index")?;
    let _roots = rt.push_temporary_value_roots(&[
        Value::Object(this_id),
        Value::Object(arr),
        Value::Object(result_id),
        index_v.clone(),
    ]);
    let index = rt.coerce_to_number(&index_v)?.max(0.0) as usize;
    let start = index.min(input.len());
    let end = (start + matched.len()).min(input.len());
    let first = Value::String(Rc::new(crate::value::JsString::from(
        input[..start].to_string(),
    )));
    let _roots = rt.push_temporary_value_roots(&[Value::Object(arr), first.clone()]);
    rt.object_set(arr, "0".into(), first);
    if limit > 1 {
        let second = Value::String(Rc::new(crate::value::JsString::from(
            input[end..].to_string(),
        )));
        let _roots = rt.push_temporary_value_roots(&[Value::Object(arr), second.clone()]);
        rt.object_set(arr, "1".into(), second);
        let len_v = Value::Number(2.0);
        let _roots = rt.push_temporary_value_roots(&[Value::Object(arr), len_v.clone()]);
        rt.object_set(arr, "length".into(), len_v);
    } else {
        let len_v = Value::Number(1.0);
        let _roots = rt.push_temporary_value_roots(&[Value::Object(arr), len_v.clone()]);
        rt.object_set(arr, "length".into(), len_v);
    }
    Ok(Value::Object(arr))
}

fn install_string_regex_methods(rt: &mut Runtime) {
    let host = match rt.string_prototype {
        Some(id) => id,
        None => return,
    };

    register_regexp_proto_method(rt, host, "match", 1, |rt, args| {

        rt.require_object_coercible(&rt.current_this())?;

        let arg = args.first().cloned().unwrap_or(Value::Undefined);
        if !matches!(arg, Value::Undefined | Value::Null) {
            if let Value::Object(arg_id) = &arg {
                let m = rt.read_property(*arg_id, "@@match")?;
                if rt.is_callable(&m) {
                    let receiver = rt.current_this();
                    return rt.call_function(m, arg.clone(), vec![receiver]);
                }

                if !matches!(m, Value::Undefined | Value::Null) {
                    return Err(RuntimeError::TypeError(
                        "String.prototype.match: Symbol.match is not a function".into(),
                    ));
                }
            }
        }

        let s = rt.to_string_strict(&rt.current_this())?.to_string();
        let re_id = coerce_regexp(rt, args.first().cloned().unwrap_or(Value::Undefined))?;
        let re_v = Value::Object(re_id);
        let matcher = crate::prototype::get_well_known_property_exact(rt, re_id, "@@match")?;
        if !rt.is_callable(&matcher) {
            return Err(RuntimeError::TypeError(
                "String.prototype.match: created RegExp @@match is not callable".into(),
            ));
        }
        rt.call_function(
            matcher,
            re_v,
            vec![Value::String(Rc::new(crate::value::JsString::from(s)))],
        )
    });

    register_regexp_proto_method(rt, host, "search", 1, |rt, args| {

        rt.require_object_coercible(&rt.current_this())?;

        let arg = args.first().cloned().unwrap_or(Value::Undefined);
        if !matches!(arg, Value::Undefined | Value::Null) {
            if let Value::Object(arg_id) = &arg {
                let m = rt.read_property(*arg_id, "@@search")?;
                if rt.is_callable(&m) {
                    let receiver = rt.current_this();
                    return rt.call_function(m, arg.clone(), vec![receiver]);
                }

                if !matches!(m, Value::Undefined | Value::Null) {
                    return Err(RuntimeError::TypeError(
                        "String.prototype.search: Symbol.search is not a function".into(),
                    ));
                }
            }
        }

        let s = rt.to_string_strict(&rt.current_this())?.to_string();
        let re_id = coerce_regexp(rt, args.first().cloned().unwrap_or(Value::Undefined))?;
        let re_v = Value::Object(re_id);
        let searcher = crate::prototype::get_well_known_property_exact(rt, re_id, "@@search")?;
        if !rt.is_callable(&searcher) {
            return Err(RuntimeError::TypeError(
                "String.prototype.search: created RegExp @@search is not callable".into(),
            ));
        }
        rt.call_function(
            searcher,
            re_v,
            vec![Value::String(Rc::new(crate::value::JsString::from(s)))],
        )
    });

    crate::intrinsics::register_intrinsic_method(rt, host, "replace", 2, |rt, args| {

        rt.require_object_coercible(&rt.current_this())?;

        let arg = args.first().cloned().unwrap_or(Value::Undefined);
        if !matches!(arg, Value::Undefined | Value::Null) {
            if let Value::Object(arg_id) = &arg {
                let m = rt.read_property(*arg_id, "@@replace")?;
                if rt.is_callable(&m) {
                    let receiver = rt.current_this();
                    let repl = args.get(1).cloned().unwrap_or(Value::Undefined);
                    return rt.call_function(m, arg.clone(), vec![receiver, repl]);
                }

                if !matches!(m, Value::Undefined | Value::Null) {
                    return Err(RuntimeError::TypeError(
                        "String.prototype.replace: Symbol.replace is not a function".into(),
                    ));
                }
            }
        }
        let s = rt.to_string_strict(&rt.current_this())?;
        let pat_arg = args.first().cloned().unwrap_or(Value::Undefined);
        let repl = args.get(1).cloned().unwrap_or(Value::Undefined);
        string_replace_impl(rt, &s, pat_arg, repl, false)
    });

    crate::intrinsics::register_intrinsic_method(rt, host, "replaceAll", 2, |rt, args| {

        rt.require_object_coercible(&rt.current_this())?;
        let arg = args.first().cloned().unwrap_or(Value::Undefined);

        if !matches!(arg, Value::Undefined | Value::Null) && rt.is_regexp_like_via(&arg)? {
            let flags = rt.get_via(
                &arg,
                &Value::String(std::rc::Rc::new(crate::value::JsString::from("flags"))),
            )?;
            rt.require_object_coercible(&flags)?;
            let flags_str = rt.to_string_strict(&flags)?;
            if !flags_str.contains('g') {
                return Err(RuntimeError::TypeError(
                    "String.prototype.replaceAll called with a non-global RegExp argument".into(),
                ));
            }
        }

        if !matches!(arg, Value::Undefined | Value::Null) {
            if let Value::Object(arg_id) = &arg {
                let m = rt.read_property(*arg_id, "@@replace")?;
                if rt.is_callable(&m) {
                    let receiver = rt.current_this();
                    let repl = args.get(1).cloned().unwrap_or(Value::Undefined);
                    return rt.call_function(m, arg.clone(), vec![receiver, repl]);
                }

                if !matches!(m, Value::Undefined | Value::Null) {
                    return Err(RuntimeError::TypeError(
                        "String.prototype.replace: Symbol.replace is not a function".into(),
                    ));
                }
            }
        }
        let s = rt.to_string_strict(&rt.current_this())?;
        let pat_arg = args.first().cloned().unwrap_or(Value::Undefined);
        let repl = args.get(1).cloned().unwrap_or(Value::Undefined);

        let pat_for_string = match &pat_arg {
            Value::Object(id) if matches!(rt.obj(*id).internal_kind, InternalKind::RegExp(_)) => {
                Value::String(std::rc::Rc::new(crate::value::JsString::from(
                    rt.to_string_strict(&pat_arg)?,
                )))
            }
            _ => pat_arg,
        };
        string_replace_impl(rt, &s, pat_for_string, repl, true)
    });

    crate::intrinsics::register_intrinsic_method(rt, host, "split", 2, |rt, args| {

        rt.require_object_coercible(&rt.current_this())?;

        let arg = args.first().cloned().unwrap_or(Value::Undefined);
        if !matches!(arg, Value::Undefined | Value::Null) {
            if let Value::Object(arg_id) = &arg {
                let m = rt.read_property(*arg_id, "@@split")?;
                if rt.is_callable(&m) {
                    let receiver = rt.current_this();
                    let lim = args.get(1).cloned().unwrap_or(Value::Undefined);
                    return rt.call_function(m, arg.clone(), vec![receiver, lim]);
                }

                if !matches!(m, Value::Undefined | Value::Null) {
                    return Err(RuntimeError::TypeError(
                        "String.prototype.split: Symbol.split is not a function".into(),
                    ));
                }
            }
        }
        let s = rt.to_string_strict(&rt.current_this())?;

        let limit: usize = match args.get(1) {
            None | Some(Value::Undefined) => u32::MAX as usize,
            Some(v) => {

                let n = rt.coerce_to_number(v)?;
                if !n.is_finite() {
                    0
                } else {
                    let f = n.trunc();
                    f.rem_euclid(4294967296.0) as u32 as usize
                }
            }
        };

        let sep_string: Option<String> = match args.first() {
            None | Some(Value::Undefined) => None,
            Some(Value::Object(id))
                if matches!(rt.obj(*id).internal_kind, InternalKind::RegExp(_)) =>
            {
                None
            }
            Some(v) => Some(rt.to_string_strict(v)?),
        };
        if limit == 0 {
            let out = rt.alloc_object(Object::new_array());
            rt.object_set(out, "length".into(), Value::Number(0.0));
            return Ok(Value::Object(out));
        }

        let parts: Vec<Value> = match args.first() {
            None | Some(Value::Undefined) => vec![Value::String(Rc::new(
                crate::value::JsString::from(s.clone()),
            ))],
            Some(Value::Object(id))
                if matches!(rt.obj(*id).internal_kind, InternalKind::RegExp(_)) =>
            {
                let rx = match &rt.obj(*id).internal_kind {
                    InternalKind::RegExp(r) => r.compiled.clone(),
                    _ => None,
                };
                let rx = match rx {
                    Some(r) => r,
                    None => {
                        return Err(RuntimeError::TypeError(
                            "String.prototype.split: regex pattern unsupported".into(),
                        ))
                    }
                };
                let mut out: Vec<Value> = Vec::new();
                let mut cursor: usize = 0;
                loop {
                    if cursor > s.len() {
                        break;
                    }
                    let caps = rx.captures_at(&s, cursor);
                    match caps {
                        None => {
                            out.push(Value::String(Rc::new(crate::value::JsString::from(
                                s[cursor..].to_string(),
                            ))));
                            break;
                        }
                        Some((mstart, mend, groups)) => {
                            if mend == cursor {

                                if cursor >= s.len() {
                                    out.push(Value::String(Rc::new(crate::value::JsString::from(
                                        s[cursor..].to_string(),
                                    ))));
                                    break;
                                }
                                let ch_len = s[cursor..]
                                    .chars()
                                    .next()
                                    .map(|c| c.len_utf8())
                                    .unwrap_or(1);
                                cursor += ch_len;
                                continue;
                            }
                            out.push(Value::String(Rc::new(crate::value::JsString::from(
                                s[cursor..mstart].to_string(),
                            ))));

                            for g in groups.iter().skip(1) {
                                out.push(match g {
                                    Some(s2) => Value::String(Rc::new(
                                        crate::value::JsString::from(s2.clone()),
                                    )),
                                    None => Value::Undefined,
                                });
                            }
                            cursor = mend;
                        }
                    }
                }
                out
            }
            Some(_sep_v) => {

                let sep = sep_string.clone().unwrap_or_default();
                if sep.is_empty() {

                    s.encode_utf16()
                        .map(|u| {
                            Value::String(Rc::new(crate::value::JsString::from_code_units(vec![u])))
                        })
                        .collect()
                } else {
                    s.split(&sep)
                        .map(|p| {
                            Value::String(Rc::new(crate::value::JsString::from(p.to_string())))
                        })
                        .collect()
                }
            }
        };
        let out = rt.alloc_object(Object::new_array());
        let len;
        {
            let obj = rt.obj_mut(out);
            obj.array_dense = true;
            obj.dense_elements = parts.into_iter().take(limit).collect();
            len = obj.dense_elements.len();
        }
        rt.object_set_checked(out, "length".into(), Value::Number(len as f64))?;
        Ok(Value::Object(out))
    });
}

const REGEX_REPLACEMENT_PRACTICAL_CAP: usize = 512usize << 20;

fn regex_replacement_push_checked(out: &mut String, part: &str) -> Result<(), RuntimeError> {
    match out.len().checked_add(part.len()) {
        Some(next) if next <= REGEX_REPLACEMENT_PRACTICAL_CAP => {
            out.push_str(part);
            Ok(())
        }
        _ => Err(RuntimeError::RangeError("Invalid string length".into())),
    }
}

fn regex_replacement_push_char_checked(out: &mut String, ch: char) -> Result<(), RuntimeError> {
    match out.len().checked_add(ch.len_utf8()) {
        Some(next) if next <= REGEX_REPLACEMENT_PRACTICAL_CAP => {
            out.push(ch);
            Ok(())
        }
        _ => Err(RuntimeError::RangeError("Invalid string length".into())),
    }
}

fn process_regex_substitution(
    repl: &str,
    matched: &str,
    before: &str,
    after: &str,
    groups: &[Option<&str>],
    named: &[(String, Vec<usize>)],
) -> Result<String, RuntimeError> {
    let mut out = String::with_capacity(repl.len());
    let bytes = repl.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'$' => {
                    regex_replacement_push_char_checked(&mut out, '$')?;
                    i += 2;
                    continue;
                }
                b'&' => {
                    regex_replacement_push_checked(&mut out, matched)?;
                    i += 2;
                    continue;
                }
                b'`' => {
                    regex_replacement_push_checked(&mut out, before)?;
                    i += 2;
                    continue;
                }
                b'\'' => {
                    regex_replacement_push_checked(&mut out, after)?;
                    i += 2;
                    continue;
                }
                b'0'..=b'9' => {

                    let n2 = if i + 2 < bytes.len() && (bytes[i + 2] as char).is_ascii_digit() {
                        let n =
                            (bytes[i + 1] - b'0') as usize * 10 + (bytes[i + 2] - b'0') as usize;
                        if n >= 1 && n <= groups.len() {
                            Some((n, 3usize))
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    let n1 = {
                        let n = (bytes[i + 1] - b'0') as usize;
                        if n >= 1 && n <= groups.len() {
                            Some((n, 2usize))
                        } else {
                            None
                        }
                    };
                    let pick = n2.or(n1);
                    if let Some((n, adv)) = pick {
                        if let Some(g) = groups.get(n - 1).and_then(|g| g.as_deref()) {
                            regex_replacement_push_checked(&mut out, g)?;
                        }
                        i += adv;
                        continue;
                    }
                }
                b'<' => {

                    if !named.is_empty() {
                        if let Some(end) = repl[i + 2..].find('>') {
                            let name = &repl[i + 2..i + 2 + end];
                            if let Some((_, slots)) = named.iter().find(|(n, _)| n == name) {
                                if let Some(g) = slots.iter().find_map(|idx| {
                                    groups.get(idx.saturating_sub(1)).and_then(|g| *g)
                                }) {
                                    regex_replacement_push_checked(&mut out, g)?;
                                }
                            }
                            i += 2 + end + 1;
                            continue;
                        }
                    }
                }
                _ => {}
            }
        }
        let ch_start = i;
        let mut ch_end = i + 1;
        while ch_end < bytes.len() && (bytes[ch_end] & 0xC0) == 0x80 {
            ch_end += 1;
        }
        regex_replacement_push_checked(&mut out, &repl[ch_start..ch_end])?;
        i = ch_end;
    }
    Ok(out)
}

fn process_regex_substitution_via(
    rt: &mut Runtime,
    repl: &str,
    matched: &str,
    before: &str,
    after: &str,
    groups: &[Option<&str>],
    named: Option<ObjectRef>,
) -> Result<String, RuntimeError> {
    let mut out = String::with_capacity(repl.len());
    let bytes = repl.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'$' => {
                    regex_replacement_push_char_checked(&mut out, '$')?;
                    i += 2;
                    continue;
                }
                b'&' => {
                    regex_replacement_push_checked(&mut out, matched)?;
                    i += 2;
                    continue;
                }
                b'`' => {
                    regex_replacement_push_checked(&mut out, before)?;
                    i += 2;
                    continue;
                }
                b'\'' => {
                    regex_replacement_push_checked(&mut out, after)?;
                    i += 2;
                    continue;
                }
                b'0'..=b'9' => {
                    let n2 = if i + 2 < bytes.len() && (bytes[i + 2] as char).is_ascii_digit() {
                        let n =
                            (bytes[i + 1] - b'0') as usize * 10 + (bytes[i + 2] - b'0') as usize;
                        if n >= 1 && n <= groups.len() {
                            Some((n, 3usize))
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    let n1 = {
                        let n = (bytes[i + 1] - b'0') as usize;
                        if n >= 1 && n <= groups.len() {
                            Some((n, 2usize))
                        } else {
                            None
                        }
                    };
                    if let Some((n, adv)) = n2.or(n1) {
                        if let Some(g) = groups.get(n - 1).and_then(|g| g.as_deref()) {
                            regex_replacement_push_checked(&mut out, g)?;
                        }
                        i += adv;
                        continue;
                    }
                }
                b'<' => {
                    if let Some(end) = repl[i + 2..].find('>') {
                        if let Some(named_id) = named {
                            let name = &repl[i + 2..i + 2 + end];
                            let capture = rt.spec_get(&Value::Object(named_id), name)?;
                            if !matches!(capture, Value::Undefined) {
                                regex_replacement_push_checked(
                                    &mut out,
                                    &rt.coerce_to_string(&capture)?,
                                )?;
                            }
                            i += 2 + end + 1;
                            continue;
                        }
                    }
                }
                _ => {}
            }
        }
        let ch_start = i;
        let mut ch_end = i + 1;
        while ch_end < bytes.len() && (bytes[ch_end] & 0xC0) == 0x80 {
            ch_end += 1;
        }
        regex_replacement_push_checked(&mut out, &repl[ch_start..ch_end])?;
        i = ch_end;
    }
    Ok(out)
}

fn string_replace_impl(
    rt: &mut Runtime,
    s: &str,
    pat: Value,
    repl: Value,
    force_global: bool,
) -> Result<Value, RuntimeError> {

    let (rx, is_global, pat_id) = match &pat {
        Value::Object(id) if matches!(rt.obj(*id).internal_kind, InternalKind::RegExp(_)) => {
            let (rx, flags) = match &rt.obj(*id).internal_kind {
                InternalKind::RegExp(r) => (r.compiled.clone(), (*r.flags).clone()),
                _ => unreachable!(),
            };
            let rx = match rx {
                Some(r) => r,
                None => {
                    return Err(RuntimeError::TypeError(
                        "String.prototype.replace: regex pattern unsupported".into(),
                    ))
                }
            };
            (rx, force_global || flags.contains('g'), Some(*id))
        }
        _ => {

            let needle = rt.coerce_to_string(&pat)?;
            let escaped = escape_regexp_literal(&needle);
            let rx = compile_either(&escaped, "").ok_or_else(|| {
                RuntimeError::TypeError(
                    "String.prototype.replace: string pattern unsupported".into(),
                )
            })?;
            (rx, force_global, None)
        }
    };
    let named = rx.named_group_slots();

    let is_callable = matches!(&repl, Value::Object(id) if {
        matches!(rt.obj(*id).internal_kind,
            InternalKind::Function(_) | InternalKind::Closure(_) | InternalKind::BoundFunction(_))
    });

    if !is_callable {
        let repl_s = rt.coerce_to_string(&repl)?;

        let mut out = String::new();
        let mut cursor = 0usize;
        let mut search_start = 0usize;
        let mut count = 0usize;
        let max_n = if is_global { usize::MAX } else { 1 };
        while count < max_n {
            let caps = match rx.captures_at(s, search_start) {
                Some(c) => c,
                None => break,
            };
            let (mstart, mend, groups) = caps;
            regex_replacement_push_checked(&mut out, &s[cursor..mstart])?;
            let matched = &s[mstart..mend];
            let before = &s[..mstart];
            let after = &s[mend..];
            let group_slices: Vec<Option<&str>> =
                groups.iter().skip(1).map(|g| g.as_deref()).collect();
            let substituted =
                process_regex_substitution(&repl_s, matched, before, after, &group_slices, &named)?;
            regex_replacement_push_checked(&mut out, &substituted)?;
            cursor = mend;
            search_start = if mend == mstart {
                advance_one_string_search_unit(s, mend)
            } else {
                mend
            };
            count += 1;
            if search_start > s.len() {
                break;
            }
        }
        if is_global {
            if let Some(id) = pat_id {
                rt.object_set(id, "lastIndex".into(), Value::Number(0.0));
            }
        }
        regex_replacement_push_checked(&mut out, &s[cursor..])?;
        return Ok(Value::String(Rc::new(crate::value::JsString::from(out))));
    }

    let mut out = String::new();
    let mut cursor = 0usize;
    let mut search_start = 0usize;
    let mut count = 0usize;
    let max_n = if is_global { usize::MAX } else { 1 };
    while count < max_n {
        let caps = match rx.captures_at(s, search_start) {
            Some(c) => c,
            None => break,
        };
        let (mstart, mend, groups) = caps;
        regex_replacement_push_checked(&mut out, &s[cursor..mstart])?;
        let mut call_args: Vec<Value> = Vec::new();

        for g in groups.iter() {
            call_args.push(match g {
                Some(s) => Value::String(Rc::new(crate::value::JsString::from(s.clone()))),
                None => Value::Undefined,
            });
        }

        call_args.push(Value::Number(s[..mstart].encode_utf16().count() as f64));

        call_args.push(Value::String(Rc::new(crate::value::JsString::from(
            s.to_string(),
        ))));

        if !named.is_empty() {
            let g_obj = rt.alloc_object_with_explicit_null_proto(Object::new_ordinary());
            for (name, slots) in &named {
                let v = slots
                    .iter()
                    .find_map(|idx| groups.get(*idx).and_then(|g| g.clone()))
                    .map(|s| Value::String(Rc::new(crate::value::JsString::from(s))))
                    .unwrap_or(Value::Undefined);
                rt.object_set(g_obj, name.clone(), v);
            }
            call_args.push(Value::Object(g_obj));
        }
        let r = rt.call_function(repl.clone(), Value::Undefined, call_args)?;

        let r_s = rt.coerce_to_string(&r)?;
        regex_replacement_push_checked(&mut out, &r_s)?;
        cursor = mend;

        search_start = if mend == mstart {
            advance_one_string_search_unit(s, mend)
        } else {
            mend
        };
        count += 1;
        if search_start > s.len() {
            break;
        }
    }
    if is_global {
        if let Some(id) = pat_id {
            rt.object_set(id, "lastIndex".into(), Value::Number(0.0));
        }
    }
    regex_replacement_push_checked(&mut out, &s[cursor..])?;
    Ok(Value::String(Rc::new(crate::value::JsString::from(out))))
}

fn advance_one_string_search_unit(s: &str, byte: usize) -> usize {
    if byte >= s.len() {
        return byte + 1;
    }
    let mut next = byte + 1;
    while next < s.len() && !s.is_char_boundary(next) {
        next += 1;
    }
    next
}

fn coerce_regexp(rt: &mut Runtime, v: Value) -> Result<ObjectRef, RuntimeError> {
    if let Value::Object(id) = &v {
        if matches!(rt.obj(*id).internal_kind, InternalKind::RegExp(_)) {
            return Ok(*id);
        }
    }

    let pattern = if matches!(v, Value::Undefined) {
        String::new()
    } else {
        rt.coerce_to_string(&v)?
    };
    new_regexp(rt, &pattern, "")
}

fn register_method<F>(rt: &mut Runtime, host: ObjectRef, name: &str, f: F)
where
    F: Fn(&mut Runtime, &[Value]) -> Result<Value, RuntimeError> + 'static,
{
    register_regexp_proto_method(rt, host, name, 0, f);
}

fn register_regexp_proto_method<F>(rt: &mut Runtime, host: ObjectRef, name: &str, length: u32, f: F)
where
    F: Fn(&mut Runtime, &[Value]) -> Result<Value, RuntimeError> + 'static,
{

    let display_name = regexp_method_display_name(name);
    let fn_obj = crate::intrinsics::make_native_non_ctor(&display_name, length, f);
    let fn_id = rt.alloc_object(fn_obj);
    let method_realm = rt
        .realms
        .iter()
        .enumerate()
        .find(|(_, realm)| realm.regexp_prototype == Some(host))
        .map(|(realm_idx, _)| realm_idx)
        .unwrap_or(rt.current_realm);
    if let InternalKind::Function(fi) = &mut rt.obj_mut(fn_id).internal_kind {
        fi.creation_realm = method_realm;
    }
    rt.obj_mut(host)
        .set_own_internal(name.into(), Value::Object(fn_id));
}

pub(crate) fn install_legacy_regexp_accessors(rt: &mut Runtime, ctor_id: ObjectRef) {
    for i in 1..=9 {
        let key = format!("${i}");
        install_legacy_regexp_accessor(rt, ctor_id, &key, LegacyRegExpAccessor::Capture(i), false);
    }
    for (key, kind, has_setter) in [
        ("input", LegacyRegExpAccessor::Input, true),
        ("$_", LegacyRegExpAccessor::Input, true),
        ("lastMatch", LegacyRegExpAccessor::LastMatch, false),
        ("$&", LegacyRegExpAccessor::LastMatch, false),
        ("lastParen", LegacyRegExpAccessor::LastParen, false),
        ("$+", LegacyRegExpAccessor::LastParen, false),
        ("leftContext", LegacyRegExpAccessor::LeftContext, false),
        ("$`", LegacyRegExpAccessor::LeftContext, false),
        ("rightContext", LegacyRegExpAccessor::RightContext, false),
        ("$'", LegacyRegExpAccessor::RightContext, false),
    ] {
        install_legacy_regexp_accessor(rt, ctor_id, key, kind, has_setter);
    }
}

#[derive(Clone, Copy)]
enum LegacyRegExpAccessor {
    Capture(usize),
    Input,
    LastMatch,
    LastParen,
    LeftContext,
    RightContext,
}

fn install_legacy_regexp_accessor(
    rt: &mut Runtime,
    ctor_id: ObjectRef,
    key: &str,
    kind: LegacyRegExpAccessor,
    has_setter: bool,
) {
    let getter_name = format!("get RegExp.{key}");
    let getter = crate::intrinsics::make_native_non_ctor(&getter_name, 0, move |rt, _args| {
        require_regexp_constructor_this(rt, "RegExp legacy accessor")?;
        let state = &rt.legacy_regexp_state;
        let out = match kind {
            LegacyRegExpAccessor::Capture(i) => state.capture(i),
            LegacyRegExpAccessor::Input => state.input(),
            LegacyRegExpAccessor::LastMatch => state.last_match(),
            LegacyRegExpAccessor::LastParen => state.last_paren(),
            LegacyRegExpAccessor::LeftContext => state.left_context(),
            LegacyRegExpAccessor::RightContext => state.right_context(),
        };
        Ok(Value::String(Rc::new(crate::value::JsString::from(out))))
    });
    let getter_id = rt.alloc_object(getter);
    let setter = if has_setter {
        let setter_name = format!("set RegExp.{key}");
        let setter = crate::intrinsics::make_native_non_ctor(&setter_name, 1, move |rt, args| {
            require_regexp_constructor_this(rt, "RegExp legacy accessor")?;
            let v = args.first().cloned().unwrap_or(Value::Undefined);
            let input = rt.coerce_to_string(&v)?;
            rt.legacy_regexp_state.set_input(input);
            Ok(Value::Undefined)
        });
        Some(Value::Object(rt.alloc_object(setter)))
    } else {
        None
    };
    rt.obj_mut(ctor_id).dict_mut().insert(
        key.into(),
        PropertyDescriptor {
            value: Value::Undefined,
            writable: false,
            enumerable: false,
            configurable: true,
            getter: Some(Value::Object(getter_id)),
            setter,
        },
    );
}

fn require_regexp_constructor_this(rt: &Runtime, label: &str) -> Result<(), RuntimeError> {
    match (rt.current_this(), rt.global_get("RegExp")) {
        (Value::Object(this_id), Value::Object(ctor_id)) if this_id == ctor_id => Ok(()),
        _ => Err(RuntimeError::TypeError(format!(
            "{label}: this is not the RegExp constructor"
        ))),
    }
}

fn escape_regexp_pattern(src: &str) -> String {
    if src.is_empty() {
        return "(?:)".to_string();
    }
    let mut out = String::with_capacity(src.len() + 2);
    let mut after_backslash = false;
    let mut in_class = false;
    for c in src.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            '/' if !after_backslash && !in_class => out.push_str("\\/"),
            other => out.push(other),
        }
        if !after_backslash {
            if c == '[' {
                in_class = true;
            } else if c == ']' {
                in_class = false;
            }
        }

        after_backslash = c == '\\' && !after_backslash;
    }
    out
}

fn install_regexp_proto_accessor(rt: &mut Runtime, host: ObjectRef, name: &'static str) {
    let getter_obj =
        crate::intrinsics::make_native_non_ctor(&format!("get {}", name), 0, move |rt, _args| {
            let this = match rt.current_this() {
                Value::Object(id) => id,
                _ => {
                    return Err(RuntimeError::TypeError(format!(
                        "get {}: this is not an Object",
                        name
                    )))
                }
            };

            if rt.regexp_prototype == Some(this) && !matches!(name, "source" | "flags") {
                return Ok(Value::Undefined);
            }

            if rt.regexp_prototype == Some(this) && name == "source" {
                return Ok(Value::String(Rc::new(crate::value::JsString::from("(?:)"))));
            }

            if !matches!(name, "source" | "flags")
                && rt.realms.iter().any(|r| r.regexp_prototype == Some(this))
            {
                return Err(RuntimeError::TypeError(format!(
                    "RegExp.prototype.{name}: this is not a RegExp (cross-realm prototype)"
                )));
            }
            if name == "flags" {
                return Ok(Value::String(Rc::new(crate::value::JsString::from(
                    regexp_flags_from_properties(rt, this)?,
                ))));
            }

            let re = match &rt.obj(this).internal_kind {
                InternalKind::RegExp(r) => r,
                _ => {
                    return Err(RuntimeError::TypeError(format!(
                        "RegExp.prototype.{}: this is not a RegExp",
                        name
                    )))
                }
            };

            Ok(match name {
                "source" => Value::String(Rc::new(crate::value::JsString::from(
                    escape_regexp_pattern(&re.source),
                ))),
                "flags" => Value::String(std::rc::Rc::new(crate::value::JsString::from(
                    re.flags.clone(),
                ))),
                "global" => Value::Boolean(re.flags.contains('g')),
                "ignoreCase" => Value::Boolean(re.flags.contains('i')),
                "multiline" => Value::Boolean(re.flags.contains('m')),
                "sticky" => Value::Boolean(re.flags.contains('y')),
                "unicode" => Value::Boolean(re.flags.contains('u')),
                "unicodeSets" => Value::Boolean(re.flags.contains('v')),
                "dotAll" => Value::Boolean(re.flags.contains('s')),
                "hasIndices" => Value::Boolean(re.flags.contains('d')),
                _ => Value::Undefined,
            })
        });
    let getter_id = rt.alloc_object(getter_obj);
    rt.obj_mut(host).dict_mut().insert(
        name.into(),
        PropertyDescriptor {
            value: Value::Undefined,
            writable: false,
            enumerable: false,
            configurable: true,
            getter: Some(Value::Object(getter_id)),
            setter: None,
        },
    );
}

fn regexp_method_display_name(name: &str) -> String {
    name.strip_prefix("@@")
        .map(|short| format!("[Symbol.{short}]"))
        .unwrap_or_else(|| name.to_string())
}

fn register_global_native<F>(rt: &mut Runtime, name: &str, f: F)
where
    F: Fn(&mut Runtime, &[Value]) -> Result<Value, RuntimeError> + 'static,
{
    let fn_obj = make_native(name, f);
    let fn_id = rt.alloc_object(fn_obj);

    rt.define_global_property(name, Value::Object(fn_id));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{CompiledRegex, InternalKind};

    fn compiled_for_units(pattern_units: Vec<u16>, flags: &str) -> CompiledRegex {
        let mut rt = Runtime::new();
        let pattern = JsString::from_code_units(pattern_units);
        let id = new_regexp_from_js_string(&mut rt, &pattern, flags, false).unwrap();
        match &rt.obj(id).internal_kind {
            InternalKind::RegExp(re) => re.compiled.clone().expect("regexp should compile"),
            _ => panic!("expected RegExp object"),
        }
    }

    fn hand_matches_frog(compiled: &CompiledRegex) -> bool {
        let CompiledRegex::Hand(hand) = compiled;
        crate::rusty_js_regex::find_at(hand, &[0xD83D, 0xDC38], 0).is_some()
    }

    fn regexp_last_index_number(rt: &Runtime, id: ObjectRef) -> f64 {
        match rt.object_get(id, "lastIndex") {
            Value::Number(n) => n,
            other => panic!("expected numeric lastIndex, got {other:?}"),
        }
    }

    #[test]
    fn regexp_group_name_decodes_raw_surrogate_marker_pair_for_identifier_gate() {
        let mut raw = String::new();
        push_raw_surrogate_marker(&mut raw, 0xD835);
        push_raw_surrogate_marker(&mut raw, 0xDCD1);

        let decoded = decode_regexp_group_name(&raw).expect("raw marker pair decodes");

        assert_eq!(decoded, "\u{1D4D1}");
        assert!(is_regexp_identifier_name(&decoded));
    }

    #[test]
    fn regexp_group_name_uses_derived_unicode_id_tables() {
        assert!(is_regexp_identifier_name("\u{2118}"));
        assert!(is_regexp_identifier_name("a\u{00B7}"));
        assert!(!is_regexp_identifier_name("\u{2E2F}"));
        assert!(!is_regexp_identifier_name("a\u{200E}"));
    }

    #[test]
    fn regexp_constructor_preserves_raw_surrogate_units_for_compile() {
        let escaped_lead_raw_trail = vec![
            b'\\' as u16,
            b'u' as u16,
            b'D' as u16,
            b'8' as u16,
            b'3' as u16,
            b'D' as u16,
            0xDC38,
        ];
        let raw_lead_escaped_trail = vec![
            0xD83D,
            b'\\' as u16,
            b'u' as u16,
            b'D' as u16,
            b'C' as u16,
            b'3' as u16,
            b'8' as u16,
        ];
        let raw_pair = vec![0xD83D, 0xDC38];

        assert!(!hand_matches_frog(&compiled_for_units(
            escaped_lead_raw_trail.clone(),
            "u"
        )));
        assert!(hand_matches_frog(&compiled_for_units(
            escaped_lead_raw_trail,
            ""
        )));
        assert!(!hand_matches_frog(&compiled_for_units(
            raw_lead_escaped_trail.clone(),
            "u"
        )));
        assert!(hand_matches_frog(&compiled_for_units(
            raw_lead_escaped_trail,
            ""
        )));
        assert!(hand_matches_frog(&compiled_for_units(raw_pair, "")));
    }

    #[test]
    fn jit_global_regexp_exec_contract_distinguishes_object_and_normal_null() {
        let mut rt = Runtime::new();
        let re = new_regexp(&mut rt, "a", "g").expect("regexp should compile");
        let input = Rc::new(JsString::from("a"));

        let first =
            jit_ic_regexp_exec_global_object_or_null_via_jsstring(&mut rt, re, &input).unwrap();
        assert!(matches!(first, JitGlobalRegExpExecOutcome::Object(_)));
        assert_eq!(regexp_last_index_number(&rt, re), 1.0);
        assert_eq!(
            rusty_js_jit::deopt::regexp_exec_ic_result_kind(first.to_jit_i64()),
            rusty_js_jit::deopt::RegexpExecIcResultKind::ObjectId
        );

        let second =
            jit_ic_regexp_exec_global_object_or_null_via_jsstring(&mut rt, re, &input).unwrap();
        assert_eq!(second, JitGlobalRegExpExecOutcome::NormalNull);
        assert_eq!(regexp_last_index_number(&rt, re), 0.0);
        assert_eq!(
            rusty_js_jit::deopt::regexp_exec_ic_result_kind(second.to_jit_i64()),
            rusty_js_jit::deopt::RegexpExecIcResultKind::NormalNull
        );
    }

    #[test]
    fn jit_global_regexp_exec_contract_rejects_non_global_and_sticky() {
        let mut rt = Runtime::new();
        let input = Rc::new(JsString::from("a"));
        let plain = new_regexp(&mut rt, "a", "").expect("plain regexp should compile");
        let sticky = new_regexp(&mut rt, "a", "y").expect("sticky regexp should compile");

        assert_eq!(
            jit_ic_regexp_exec_global_object_or_null_via_jsstring(&mut rt, plain, &input).unwrap(),
            JitGlobalRegExpExecOutcome::Deopt
        );
        assert_eq!(
            regexp_last_index_number(&rt, plain),
            0.0,
            "rejection must be side-effect-free"
        );
        assert_eq!(
            jit_ic_regexp_exec_global_object_or_null_via_jsstring(&mut rt, sticky, &input).unwrap(),
            JitGlobalRegExpExecOutcome::Deopt
        );
        assert_eq!(
            rusty_js_jit::deopt::regexp_exec_ic_result_kind(
                JitGlobalRegExpExecOutcome::Deopt.to_jit_i64()
            ),
            rusty_js_jit::deopt::RegexpExecIcResultKind::Deopt
        );
    }

    #[test]
    fn jit_global_regexp_exec_contract_rejects_nonordinary_last_index() {
        let mut rt = Runtime::new();
        let re = new_regexp(&mut rt, "a", "g").expect("regexp should compile");
        rt.obj_mut(re).dict_mut().insert(
            crate::value::PropertyKey::String("lastIndex".to_string()),
            PropertyDescriptor {
                value: Value::Number(0.0),
                writable: false,
                enumerable: false,
                configurable: false,
                getter: None,
                setter: None,
            },
        );
        let input = Rc::new(JsString::from("a"));

        assert_eq!(
            jit_ic_regexp_exec_global_object_or_null_via_jsstring(&mut rt, re, &input).unwrap(),
            JitGlobalRegExpExecOutcome::Deopt
        );
        assert_eq!(regexp_last_index_number(&rt, re), 0.0);
    }
}
