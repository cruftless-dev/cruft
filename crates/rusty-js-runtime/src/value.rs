
use indexmap::IndexMap;
use std::cell::OnceCell;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

pub type UpvalueCell = Rc<RefCell<Value>>;

pub fn new_upvalue_cell(v: Value) -> UpvalueCell {
    Rc::new(RefCell::new(v))
}

fn trace_value_object(value: &Value, ids: &mut Vec<rusty_js_gc::ObjectId>) {
    if let Value::Object(id) = value {
        ids.push(*id);
    }
}

#[derive(Debug, Clone)]
pub enum CapturedBinding {
    Cell(UpvalueCell),
    GlobalObject { name: String },
    ScriptGlobalVar { name: String },
    EvalVarShadow { name: String, cell: UpvalueCell },
    ImmutableSelfName { name: String, cell: UpvalueCell },
}

impl CapturedBinding {
    pub fn cell(cell: UpvalueCell) -> Self {
        Self::Cell(cell)
    }
}

fn trace_captured_binding(binding: &CapturedBinding, ids: &mut Vec<rusty_js_gc::ObjectId>) {
    match binding {
        CapturedBinding::Cell(cell)
        | CapturedBinding::EvalVarShadow { cell, .. }
        | CapturedBinding::ImmutableSelfName { cell, .. } => {
            trace_value_object(&cell.borrow(), ids);
        }
        CapturedBinding::GlobalObject { .. } | CapturedBinding::ScriptGlobalVar { .. } => {}
    }
}

fn trace_boundary_wrapper_refs(
    bw: &BoundaryWrapperInternals,
    ids: &mut Vec<rusty_js_gc::ObjectId>,
) {
    trace_value_object(&bw.target, ids);
    trace_value_object(&bw.validator, ids);
    for default in &bw.sanitize_arg_defaults {
        if let Some(value) = default {
            trace_value_object(value, ids);
        }
    }
    if let Some(value) = &bw.sanitize_return_default {
        trace_value_object(value, ids);
    }
    if let Some(id) = bw.installer_compartment {
        ids.push(id);
    }
}

fn trace_promise_reaction_handler(
    handler: &Option<PromiseReactionHandler>,
    ids: &mut Vec<rusty_js_gc::ObjectId>,
) {
    match handler {
        Some(PromiseReactionHandler::Callable(value)) => trace_value_object(value, ids),
        Some(PromiseReactionHandler::LazyArrow(lazy)) => {
            if let Some(id) = lazy.creation_global {
                ids.push(id);
            }
            if let Some(id) = lazy.import_meta {
                ids.push(id);
            }
            for cell in &lazy.upvalues {
                trace_value_object(&cell.borrow(), ids);
            }
            for binding in &lazy.captured_bindings {
                trace_captured_binding(binding, ids);
            }
            if let Some(value) = &lazy.bound_this {
                trace_value_object(value, ids);
            }
            if let Some(cell) = &lazy.bound_this_cell {
                trace_value_object(&cell.borrow(), ids);
            }
            if let Some(value) = &lazy.bound_derived_initial_this {
                trace_value_object(value, ids);
            }
            if let Some(id) = lazy.bound_executing_function {
                ids.push(id);
            }
            if let Some(value) = &lazy.bound_new_target {
                trace_value_object(value, ids);
            }
            ids.extend(lazy.captured_with_env_stack.iter().copied());
        }
        Some(PromiseReactionHandler::LazyArrowOneCell(lazy)) => {
            if let Some(id) = lazy.creation_global {
                ids.push(id);
            }
            trace_value_object(&lazy.upvalue.borrow(), ids);
            if let Some(value) = &lazy.bound_this {
                trace_value_object(value, ids);
            }
            if let Some(id) = lazy.bound_executing_function {
                ids.push(id);
            }
        }
        Some(PromiseReactionHandler::AsyncAwaitContinuation { promise, snapshot }) => {
            ids.push(*promise);
            snapshot.trace_object_refs(ids);
        }
        None => {}
    }
}

fn trace_internal_kind_refs(kind: &InternalKind, ids: &mut Vec<rusty_js_gc::ObjectId>) {
    match kind {
        InternalKind::Ordinary
        | InternalKind::IsHtmlDda
        | InternalKind::Array
        | InternalKind::Error
        | InternalKind::ModuleNamespace
        | InternalKind::RegExp(_) => {}
        InternalKind::NumberWrapper(value)
        | InternalKind::StringWrapper(value)
        | InternalKind::BooleanWrapper(value)
        | InternalKind::BigIntWrapper(value) => trace_value_object(value, ids),
        InternalKind::MappedArguments { parameter_map } => {
            for cell in parameter_map.values() {
                trace_value_object(&cell.borrow(), ids);
            }
        }
        InternalKind::Closure(c) => {
            if let Some(id) = c.creation_global {
                ids.push(id);
            }
            if let Some(id) = c.import_meta {
                ids.push(id);
            }
            for cell in &c.upvalues {
                trace_value_object(&cell.borrow(), ids);
            }
            if let Some(value) = &c.bound_this {
                trace_value_object(value, ids);
            }
            if let Some(cell) = &c.bound_this_cell {
                trace_value_object(&cell.borrow(), ids);
            }
            if let Some(value) = &c.bound_derived_initial_this {
                trace_value_object(value, ids);
            }
            if let Some(id) = c.bound_executing_function {
                ids.push(id);
            }
            if let Some(value) = &c.bound_new_target {
                trace_value_object(value, ids);
            }
            ids.extend(c.captured_with_env_stack.iter().copied());
        }
        InternalKind::Function(f) => {
            ids.extend(f.roots.iter().copied());
        }
        InternalKind::BoundFunction(b) => {
            ids.push(b.target);
            trace_value_object(&b.this, ids);
            for value in &b.args {
                trace_value_object(value, ids);
            }
        }
        InternalKind::Promise(ps) => {
            trace_value_object(&ps.value, ids);
            for reaction in ps
                .fulfill_reactions
                .iter()
                .chain(ps.reject_reactions.iter())
            {
                ids.push(reaction.chain);
                trace_promise_reaction_handler(&reaction.handler, ids);
                if let Some(value) = &reaction.cap_resolve {
                    trace_value_object(value, ids);
                }
                if let Some(value) = &reaction.cap_reject {
                    trace_value_object(value, ids);
                }
            }
        }
        InternalKind::Generator(g) => {
            if let Some(snapshot) = &g.continuation {
                snapshot.trace_object_refs(ids);
            }
            if let Some(value) = &g.pending_return {
                trace_value_object(value, ids);
            }
            if let Some(delegate) = &g.delegate {
                ids.push(delegate.iterator);
                trace_value_object(&delegate.next_method, ids);
            }
            for req in &g.request_queue {
                ids.push(req.promise);
                trace_value_object(&req.value, ids);
            }
        }
        InternalKind::Proxy(p) => {
            ids.push(p.target);
            ids.push(p.handler);
        }
        InternalKind::BoundaryWrapper(bw) => trace_boundary_wrapper_refs(bw, ids),
    }
}

pub type ObjectRef = rusty_js_gc::ObjectId;

impl rusty_js_gc::Trace for Object {
    fn trace(&self, ids: &mut Vec<rusty_js_gc::ObjectId>) {
        if let Some(p) = self.proto {
            ids.push(p);
        }

        if let Some(buf) = self.viewed_buffer {
            ids.push(buf);
        }
        if let Some(home) = self.private_home {
            ids.push(home);
        }
        ids.extend(self.private_outer_homes.iter().copied());
        ids.extend(self.private_pending_homes.iter().copied());

        for v in &self.shape_values {
            if let Value::Object(id) = v {
                ids.push(*id);
            }
        }

        for v in &self.dense_elements {
            if let Value::Object(id) = v {
                ids.push(*id);
            }
        }
        if !self.has_own_str("__weak_collection_storage") {
            for (k, d) in &self.properties {
                if k.as_str() == "__weakref_target" {
                    continue;
                }
                if let Value::Object(id) = &d.value {
                    ids.push(*id);
                }
                if let Some(Value::Object(id)) = &d.getter {
                    ids.push(*id);
                }
                if let Some(Value::Object(id)) = &d.setter {
                    ids.push(*id);
                }
            }
        }
        for v in self
            .private_members
            .as_deref()
            .into_iter()
            .flat_map(|pm| pm.fields.values())
        {
            if let Value::Object(id) = v {
                ids.push(*id);
            }
        }
        trace_internal_kind_refs(&self.internal_kind, ids);
    }

    fn trace_slice(
        &self,
        start: usize,
        budget: usize,
        ids: &mut Vec<rusty_js_gc::ObjectId>,
    ) -> rusty_js_gc::TraceSlice {
        if budget == 0 {
            return rusty_js_gc::TraceSlice {
                next_index: start,
                complete: false,
            };
        }
        let mut emitted = 0usize;
        let mut index = 0usize;

        macro_rules! offer {
            ($edge:expr) => {{
                if index >= start {
                    if let Some(id) = $edge {
                        ids.push(id);
                        emitted += 1;
                        if emitted >= budget {
                            return rusty_js_gc::TraceSlice {
                                next_index: index + 1,
                                complete: false,
                            };
                        }
                    }
                }
                index += 1;
            }};
        }

        offer!(self.proto);
        offer!(self.viewed_buffer);
        offer!(self.private_home);

        if start < index + self.private_outer_homes.len() {
            let begin = start.saturating_sub(index);
            index += begin;
            for offset in begin..self.private_outer_homes.len() {
                offer!(Some(self.private_outer_homes[offset]));
            }
        } else {
            index += self.private_outer_homes.len();
        }

        if start < index + self.private_pending_homes.len() {
            let begin = start.saturating_sub(index);
            index += begin;
            for offset in begin..self.private_pending_homes.len() {
                offer!(Some(self.private_pending_homes[offset]));
            }
        } else {
            index += self.private_pending_homes.len();
        }

        if start < index + self.shape_values.len() {
            let begin = start.saturating_sub(index);
            index += begin;
            for offset in begin..self.shape_values.len() {
                let edge = match self.shape_values.get(offset) {
                    Some(Value::Object(id)) => Some(*id),
                    _ => None,
                };
                offer!(edge);
            }
        } else {
            index += self.shape_values.len();
        }

        if start < index + self.dense_elements.len() {
            let begin = start.saturating_sub(index);
            index += begin;
            for offset in begin..self.dense_elements.len() {
                let edge = match self.dense_elements.get(offset) {
                    Some(Value::Object(id)) => Some(*id),
                    _ => None,
                };
                offer!(edge);
            }
        } else {
            index += self.dense_elements.len();
        }

        if !self.has_own_str("__weak_collection_storage") {
            let property_slots = self.properties.len() * 3;
            if start < index + property_slots {
                let mut slot = start.saturating_sub(index);
                index += slot;
                while slot < property_slots {
                    let prop_index = slot / 3;
                    let prop_field = slot % 3;
                    let edge =
                        self.properties
                            .get_index(prop_index)
                            .and_then(|(key, descriptor)| {
                                if key.as_str() == "__weakref_target" {
                                    return None;
                                }
                                match prop_field {
                                    0 => match &descriptor.value {
                                        Value::Object(id) => Some(*id),
                                        _ => None,
                                    },
                                    1 => match &descriptor.getter {
                                        Some(Value::Object(id)) => Some(*id),
                                        _ => None,
                                    },
                                    2 => match &descriptor.setter {
                                        Some(Value::Object(id)) => Some(*id),
                                        _ => None,
                                    },
                                    _ => None,
                                }
                            });
                    offer!(edge);
                    slot += 1;
                }
            } else {
                index += property_slots;
            }
        }

        let pf_len = self
            .private_members
            .as_deref()
            .map_or(0, |pm| pm.fields.len());
        if start < index + pf_len {
            let begin = start.saturating_sub(index);
            index += begin;
            if let Some(pm) = self.private_members.as_deref() {
                for offset in begin..pf_len {
                    let edge = pm.fields.get_index(offset).and_then(|(_, value)| {
                        if let Value::Object(id) = value {
                            Some(*id)
                        } else {
                            None
                        }
                    });
                    offer!(edge);
                }
            }
        } else {
            index += pf_len;
        }

        if start <= index {
            let before = ids.len();
            trace_internal_kind_refs(&self.internal_kind, ids);
            emitted += ids.len() - before;
            let _ = emitted;
        }

        rusty_js_gc::TraceSlice {
            next_index: index + 1,
            complete: true,
        }
    }
}

fn record_string_flatten(byte_len: usize) {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::OnceLock;
    static ENABLED: OnceLock<bool> = OnceLock::new();
    if !*ENABLED.get_or_init(|| {
        std::env::var("CRUFT_STRING_FLATTEN_COUNTERS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }) {
        return;
    }
    static FLATTENS: AtomicU64 = AtomicU64::new(0);
    static FLATTEN_BYTES: AtomicU64 = AtomicU64::new(0);
    static EVERY: OnceLock<u64> = OnceLock::new();
    let every = *EVERY.get_or_init(|| {
        std::env::var("CRUFT_STRING_FLATTEN_COUNTERS_EVERY")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(100_000)
    });
    let n = FLATTENS.fetch_add(1, Ordering::Relaxed) + 1;
    let bytes = FLATTEN_BYTES.fetch_add(byte_len as u64, Ordering::Relaxed) + byte_len as u64;
    if n % every == 0 {
        eprintln!(
            "[string-flatten-counters] flattens={} flatten_bytes={}",
            n, bytes
        );
    }
}

thread_local! {

    static PENDING_FLATTEN_BYTES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(crate) fn note_rc_payload_bytes_for_gc(byte_len: usize) {
    if byte_len >= 1024 {
        PENDING_FLATTEN_BYTES.with(|c| c.set(c.get().saturating_add(byte_len)));
    }
}

pub(crate) fn take_pending_flatten_bytes() -> usize {
    PENDING_FLATTEN_BYTES.with(|c| c.replace(0))
}

const LARGE_WELLFORMED_MMAP_THRESHOLD: usize = 256 * 1024;

#[derive(Debug)]
enum WellformedStringStorage {
    Inline(String),
    Mmap(LargeMmapString),
}

#[derive(Debug)]
struct LargeMmapString {
    ptr: std::ptr::NonNull<u8>,
    len: usize,
}

impl LargeMmapString {
    fn from_string(s: String) -> Result<Self, String> {
        let len = s.len();
        if len == 0 {
            return Err(s);
        }
        let ptr = unsafe { platform_mmap_bytes(len) };
        let Some(ptr) = std::ptr::NonNull::new(ptr) else {
            return Err(s);
        };
        unsafe {
            std::ptr::copy_nonoverlapping(s.as_ptr(), ptr.as_ptr(), len);
        }
        Ok(Self { ptr, len })
    }

    #[inline]
    fn as_str(&self) -> &str {
        unsafe {
            let bytes = std::slice::from_raw_parts(self.ptr.as_ptr(), self.len);
            std::str::from_utf8_unchecked(bytes)
        }
    }
}

impl Clone for LargeMmapString {
    fn clone(&self) -> Self {
        Self::from_string(self.as_str().to_owned()).unwrap_or_else(|_| {
            panic!(
                "large wellformed string mmap clone failed for {} bytes",
                self.len
            )
        })
    }
}

impl Drop for LargeMmapString {
    fn drop(&mut self) {
        unsafe {
            platform_munmap_bytes(self.ptr.as_ptr(), self.len);
        }
    }
}

#[cfg(unix)]
unsafe fn platform_mmap_bytes(len: usize) -> *mut u8 {
    unsafe extern "C" {
        fn mmap(
            addr: *mut core::ffi::c_void,
            len: usize,
            prot: i32,
            flags: i32,
            fd: i32,
            offset: isize,
        ) -> *mut core::ffi::c_void;
    }

    const PROT_READ: i32 = 0x1;
    const PROT_WRITE: i32 = 0x2;
    const MAP_PRIVATE: i32 = 0x0002;
    #[cfg(target_os = "macos")]
    const MAP_ANON_FLAG: i32 = 0x1000;
    #[cfg(not(target_os = "macos"))]
    const MAP_ANON_FLAG: i32 = 0x20;
    let ptr = mmap(
        std::ptr::null_mut(),
        len,
        PROT_READ | PROT_WRITE,
        MAP_PRIVATE | MAP_ANON_FLAG,
        -1,
        0,
    );
    if ptr as isize == -1 {
        std::ptr::null_mut()
    } else {
        ptr.cast::<u8>()
    }
}

#[cfg(unix)]
unsafe fn platform_munmap_bytes(ptr: *mut u8, len: usize) {
    unsafe extern "C" {
        fn munmap(addr: *mut core::ffi::c_void, len: usize) -> i32;
    }
    let _ = munmap(ptr.cast::<core::ffi::c_void>(), len);
}

#[cfg(not(unix))]
unsafe fn platform_mmap_bytes(_len: usize) -> *mut u8 {
    std::ptr::null_mut()
}

#[cfg(not(unix))]
unsafe fn platform_munmap_bytes(_ptr: *mut u8, _len: usize) {}

#[derive(Debug)]
pub struct WellformedString {
    storage: WellformedStringStorage,
    is_ascii: bool,
}

impl WellformedString {
    pub fn new(string: String) -> Self {
        let is_ascii = string.is_ascii();
        let storage = if string.len() >= LARGE_WELLFORMED_MMAP_THRESHOLD {
            match LargeMmapString::from_string(string) {
                Ok(mmap) => WellformedStringStorage::Mmap(mmap),
                Err(string) => WellformedStringStorage::Inline(string),
            }
        } else {
            WellformedStringStorage::Inline(string)
        };
        Self { storage, is_ascii }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        match &self.storage {
            WellformedStringStorage::Inline(s) => s.as_str(),
            WellformedStringStorage::Mmap(s) => s.as_str(),
        }
    }

    #[inline]
    pub fn is_ascii(&self) -> bool {
        self.is_ascii
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.as_str().as_bytes()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.as_str().len()
    }

    #[inline]
    pub fn encode_utf16(&self) -> std::str::EncodeUtf16<'_> {
        self.as_str().encode_utf16()
    }

    #[inline]
    pub fn clone_string(&self) -> String {
        self.as_str().to_owned()
    }
}

impl Clone for WellformedString {
    fn clone(&self) -> Self {
        Self {
            storage: match &self.storage {
                WellformedStringStorage::Inline(s) => WellformedStringStorage::Inline(s.clone()),
                WellformedStringStorage::Mmap(s) => WellformedStringStorage::Mmap(s.clone()),
            },
            is_ascii: self.is_ascii,
        }
    }
}

impl std::ops::Deref for WellformedString {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Debug)]
pub enum JsString {

    Wellformed(WellformedString),

    Slice {
        base: Rc<JsString>,
        start: usize,
        end: usize,
    },

    Wtf16 { units: Vec<u16>, lossy: String },

    Latin1 { bytes: Vec<u8>, string: String },

    Concat {
        left: Rc<JsString>,
        right: Rc<JsString>,
        byte_len: usize,
        flat: OnceCell<String>,
    },
}

impl Clone for JsString {
    fn clone(&self) -> Self {
        match self {
            JsString::Wellformed(s) => JsString::Wellformed(s.clone()),
            JsString::Slice { base, start, end } => JsString::Slice {
                base: base.clone(),
                start: *start,
                end: *end,
            },
            JsString::Wtf16 { units, lossy } => JsString::Wtf16 {
                units: units.clone(),
                lossy: lossy.clone(),
            },
            JsString::Latin1 { bytes, string } => JsString::Latin1 {
                bytes: bytes.clone(),
                string: string.clone(),
            },
            JsString::Concat {
                left,
                right,
                byte_len,
                ..
            } => JsString::Concat {
                left: left.clone(),
                right: right.clone(),
                byte_len: *byte_len,
                flat: OnceCell::new(),
            },
        }
    }
}

impl JsString {
    pub fn wellformed(s: String) -> JsString {

        note_rc_payload_bytes_for_gc(s.len());
        JsString::Wellformed(WellformedString::new(s))
    }

    pub fn as_str(&self) -> &str {
        match self {
            JsString::Wellformed(s) => s.as_str(),
            JsString::Slice { base, start, end } => &base.as_str()[*start..*end],
            JsString::Wtf16 { lossy, .. } => lossy.as_str(),
            JsString::Latin1 { string, .. } => string.as_str(),
            JsString::Concat { flat, byte_len, .. } => flat
                .get_or_init(|| {
                    record_string_flatten(*byte_len);
                    note_rc_payload_bytes_for_gc(*byte_len);
                    let mut out = String::with_capacity(*byte_len);
                    self.push_wellformed_flattened_to(&mut out);
                    out
                })
                .as_str(),
        }
    }

    fn push_wellformed_flattened_to(&self, out: &mut String) {
        match self {
            JsString::Wellformed(s) => out.push_str(s),
            JsString::Slice { .. } => out.push_str(self.as_str()),
            JsString::Concat { left, right, .. } => {
                left.push_wellformed_flattened_to(out);
                right.push_wellformed_flattened_to(out);
            }
            JsString::Wtf16 { .. } => out.push_str(self.as_str()),
            JsString::Latin1 { string, .. } => out.push_str(string),
        }
    }

    pub fn to_str_lossy(&self) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Borrowed(self.as_str())
    }

    pub fn as_wellformed(&self) -> Option<&str> {
        match self {
            JsString::Wellformed(s) => Some(s.as_str()),
            JsString::Slice { .. } => Some(self.as_str()),
            JsString::Concat { .. } => Some(self.as_str()),
            JsString::Latin1 { string, .. } => Some(string.as_str()),
            JsString::Wtf16 { .. } => None,
        }
    }

    pub fn code_units(&self) -> std::borrow::Cow<'_, [u16]> {
        match self {
            JsString::Wellformed(s) => std::borrow::Cow::Owned(s.encode_utf16().collect()),
            JsString::Slice { .. } => {
                std::borrow::Cow::Owned(self.as_str().encode_utf16().collect())
            }
            JsString::Concat { left, right, .. } => {
                let left_units = left.code_units();
                let right_units = right.code_units();
                let mut out = Vec::with_capacity(left_units.len() + right_units.len());
                out.extend_from_slice(&left_units);
                out.extend_from_slice(&right_units);
                std::borrow::Cow::Owned(out)
            }
            JsString::Latin1 { bytes, .. } => {
                std::borrow::Cow::Owned(bytes.iter().map(|b| u16::from(*b)).collect())
            }
            JsString::Wtf16 { units, .. } => std::borrow::Cow::Borrowed(units.as_slice()),
        }
    }

    pub fn code_unit_at(&self, index: usize) -> Option<u16> {
        match self {
            JsString::Wellformed(s) if s.is_ascii() => {
                s.as_bytes().get(index).copied().map(u16::from)
            }
            JsString::Wellformed(s) => s.encode_utf16().nth(index),
            JsString::Slice { .. } | JsString::Concat { .. } => {
                let s = self.as_str();
                if s.is_ascii() {
                    s.as_bytes().get(index).copied().map(u16::from)
                } else {
                    s.encode_utf16().nth(index)
                }
            }
            JsString::Latin1 { bytes, .. } => bytes.get(index).copied().map(u16::from),
            JsString::Wtf16 { units, .. } => units.get(index).copied(),
        }
    }

    pub fn latin1_code_unit_at(&self, index: usize) -> Option<Option<u16>> {
        match self {
            JsString::Latin1 { bytes, .. } => Some(bytes.get(index).copied().map(u16::from)),
            _ => None,
        }
    }

    pub fn code_unit_len(&self) -> usize {
        match self {
            JsString::Wellformed(s) if s.is_ascii() => s.len(),
            JsString::Wellformed(s) => s.encode_utf16().count(),
            JsString::Slice { .. } | JsString::Concat { .. } => {
                let s = self.as_str();
                if s.is_ascii() {
                    s.len()
                } else {
                    s.encode_utf16().count()
                }
            }
            JsString::Latin1 { bytes, .. } => bytes.len(),
            JsString::Wtf16 { units, .. } => units.len(),
        }
    }

    pub fn to_lossless_source_text(&self) -> String {
        const RAW_SURROGATE_MARKER_BASE: u32 = 0xF0000;

        fn push_raw_surrogate_marker(out: &mut String, unit: u16) {
            debug_assert!((0xD800..=0xDFFF).contains(&unit));
            let cp = RAW_SURROGATE_MARKER_BASE + (unit as u32 - 0xD800);
            if let Some(ch) = char::from_u32(cp) {
                out.push(ch);
            }
        }

        fn decode_utf16_scalar(units: &[u16], i: usize) -> Option<(char, usize)> {
            let u0 = *units.get(i)?;
            if (0xD800..=0xDBFF).contains(&u0) {
                let u1 = *units.get(i + 1)?;
                if (0xDC00..=0xDFFF).contains(&u1) {
                    let hi = (u0 as u32) - 0xD800;
                    let lo = (u1 as u32) - 0xDC00;
                    return char::from_u32(0x10000 + ((hi << 10) | lo)).map(|ch| (ch, 2));
                }
                return None;
            }
            if (0xDC00..=0xDFFF).contains(&u0) {
                return None;
            }
            char::from_u32(u0 as u32).map(|ch| (ch, 1))
        }

        match self {
            JsString::Wellformed(s) => s.clone_string(),
            JsString::Slice { .. } => self.as_str().to_string(),
            JsString::Concat { .. } => self.as_str().to_string(),
            JsString::Latin1 { string, .. } => string.clone(),
            JsString::Wtf16 { units, .. } => {
                let mut out = String::new();
                let mut i = 0usize;
                while i < units.len() {
                    let unit = units[i];
                    if let Some((ch, width)) = decode_utf16_scalar(units, i) {
                        out.push(ch);
                        i += width;
                    } else {
                        push_raw_surrogate_marker(&mut out, unit);
                        i += 1;
                    }
                }
                out
            }
        }
    }

    pub fn is_well_formed(&self) -> bool {
        matches!(
            self,
            JsString::Wellformed(_)
                | JsString::Slice { .. }
                | JsString::Concat { .. }
                | JsString::Latin1 { .. }
        )
    }

    pub fn slice_wellformed(base: Rc<JsString>, start: usize, end: usize) -> Option<JsString> {
        let s = base.as_wellformed()?;
        if start > end || end > s.len() || !s.is_char_boundary(start) || !s.is_char_boundary(end) {
            return None;
        }
        if start == 0 && end == s.len() {
            return Some((*base).clone());
        }
        Some(JsString::Slice { base, start, end })
    }

    pub fn from_code_units(units: Vec<u16>) -> JsString {
        match String::from_utf16(&units) {
            Ok(s) => JsString::wellformed(s),
            Err(_) => {
                let lossy = String::from_utf16_lossy(&units);
                JsString::Wtf16 { units, lossy }
            }
        }
    }

    pub fn from_latin1_bytes(bytes: Vec<u8>) -> JsString {
        let mut string = String::with_capacity(bytes.len());
        for &byte in &bytes {
            string.push(char::from(byte));
        }
        JsString::Latin1 { bytes, string }
    }

    pub fn code_unit_as_string(&self, i: usize) -> Option<JsString> {
        self.code_unit_at(i)
            .map(|u| JsString::from_code_units(vec![u]))
    }

    pub fn len_utf16(&self) -> usize {
        self.code_unit_len()
    }

    #[inline]
    pub fn len(&self) -> usize {
        match self {
            JsString::Wellformed(s) => s.len(),
            JsString::Slice { start, end, .. } => end - start,
            JsString::Wtf16 { lossy, .. } => lossy.len(),
            JsString::Latin1 { string, .. } => string.len(),
            JsString::Concat { byte_len, .. } => *byte_len,
        }
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.as_str().is_empty()
    }

    #[inline]
    pub fn is_ascii(&self) -> bool {
        match self {
            JsString::Wellformed(s) => s.is_ascii(),
            JsString::Slice { .. } | JsString::Concat { .. } => self.as_str().is_ascii(),
            JsString::Wtf16 { lossy, .. } => lossy.is_ascii(),
            JsString::Latin1 { bytes, .. } => bytes.is_ascii(),
        }
    }
}

impl PartialEq for JsString {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_wellformed(), other.as_wellformed()) {
            (Some(a), Some(b)) => a == b,
            _ => self.code_units().as_ref() == other.code_units().as_ref(),
        }
    }
}

impl Eq for JsString {}

impl Hash for JsString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self.as_wellformed() {
            Some(s) => {
                for unit in s.encode_utf16() {
                    unit.hash(state);
                }
            }
            None => self.code_units().as_ref().hash(state),
        }
    }
}

impl std::ops::Deref for JsString {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl From<String> for JsString {
    fn from(s: String) -> Self {
        JsString::wellformed(s)
    }
}
impl From<&str> for JsString {
    fn from(s: &str) -> Self {
        JsString::wellformed(s.to_string())
    }
}
impl From<Rc<String>> for JsString {
    fn from(s: Rc<String>) -> Self {
        JsString::wellformed((*s).clone())
    }
}
impl From<&String> for JsString {
    fn from(s: &String) -> Self {
        JsString::wellformed(s.clone())
    }
}
impl std::fmt::Display for JsString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_str_lossy())
    }
}

impl PartialEq<str> for JsString {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}
impl PartialEq<&str> for JsString {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[derive(Clone)]
#[repr(C, u8)]
pub enum Value {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    String(Rc<JsString>),

    BigInt(Rc<crate::bigint::JsBigInt>),

    Symbol(Rc<String>),
    Object(ObjectRef),
}

pub const VALUE_TAG_UNDEFINED: u8 = 0;
pub const VALUE_TAG_NULL: u8 = 1;
pub const VALUE_TAG_BOOLEAN: u8 = 2;
pub const VALUE_TAG_NUMBER: u8 = 3;
pub const VALUE_TAG_STRING: u8 = 4;
pub const VALUE_TAG_BIGINT: u8 = 5;
pub const VALUE_TAG_SYMBOL: u8 = 6;
pub const VALUE_TAG_OBJECT: u8 = 7;

pub const VALUE_NUMBER_PAYLOAD_OFFSET: usize = 8;

const _: () = {

    assert!(
        std::mem::size_of::<Value>() >= 16,
        "Value must be at least 16 bytes (1B tag + 7B pad + 8B payload)"
    );

    assert!(
        std::mem::align_of::<Value>() >= 8,
        "Value alignment must be at least 8 for f64 payload"
    );
};

pub fn assert_value_layout() {
    let v = Value::Number(0.0);

    let tag = unsafe { *((&v as *const Value) as *const u8) };
    assert_eq!(
        tag, VALUE_TAG_NUMBER,
        "Value::Number discriminant byte ({}) does not match \
         VALUE_TAG_NUMBER ({}); rustc layout drift detected. \
         VTI-EXT 3a invariant violated.",
        tag, VALUE_TAG_NUMBER
    );

    let v2 = Value::Number(1.5_f64);
    let payload = unsafe {
        let base = &v2 as *const Value as *const u8;
        let pf = base.add(VALUE_NUMBER_PAYLOAD_OFFSET) as *const f64;
        *pf
    };
    assert_eq!(
        payload, 1.5,
        "Value::Number payload not at offset {}; rustc layout drift.",
        VALUE_NUMBER_PAYLOAD_OFFSET
    );
}

impl Value {
    pub fn type_of(&self) -> &'static str {
        match self {
            Value::Undefined => "undefined",
            Value::Null => "object",
            Value::Boolean(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::BigInt(_) => "bigint",
            Value::Symbol(_) => "symbol",

            Value::Object(_) => "object",
        }
    }

    pub fn same_value(a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Undefined, Value::Undefined) => true,
            (Value::Null, Value::Null) => true,
            (Value::Boolean(x), Value::Boolean(y)) => x == y,
            (Value::Number(x), Value::Number(y)) => {
                if x.is_nan() && y.is_nan() {
                    return true;
                }
                x.to_bits() == y.to_bits()
            }
            (Value::String(x), Value::String(y)) => x == y,
            (Value::BigInt(x), Value::BigInt(y)) => x == y,
            (Value::Symbol(x), Value::Symbol(y)) => x == y,
            (Value::Object(x), Value::Object(y)) => x == y,
            _ => false,
        }
    }
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Undefined => write!(f, "undefined"),
            Value::Null => write!(f, "null"),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Number(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "{:?}", s.as_str()),
            Value::BigInt(b) => write!(f, "{}n", b.to_decimal()),
            Value::Symbol(s) => write!(f, "Symbol({:?})", s.as_str()),
            Value::Object(id) => write!(f, "[Object #{}]", id.0),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        Self::same_value(self, other)
    }
}

#[derive(Clone, Debug)]
pub enum PropertyKey {
    String(String),
    Symbol(Rc<String>),
}

impl PartialEq for PropertyKey {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::String(a), Self::String(b)) => a == b,

            (Self::Symbol(a), Self::Symbol(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl Eq for PropertyKey {}

impl std::hash::Hash for PropertyKey {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        match self {
            Self::String(s) => {
                0u8.hash(h);
                s.hash(h);
            }
            Self::Symbol(rc) => {
                1u8.hash(h);
                (Rc::as_ptr(rc) as usize).hash(h);
            }
        }
    }
}

impl PropertyKey {

    pub fn as_str(&self) -> &str {
        match self {
            Self::String(s) => s.as_str(),
            Self::Symbol(rc) => rc.as_str(),
        }
    }
    pub fn is_symbol(&self) -> bool {
        matches!(self, Self::Symbol(_))
    }
    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }

    pub fn to_string_content(&self) -> String {
        match self {
            Self::String(s) => s.clone(),
            Self::Symbol(rc) => (**rc).clone(),
        }
    }
}

impl From<&str> for PropertyKey {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}
impl From<String> for PropertyKey {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}
impl From<&String> for PropertyKey {
    fn from(s: &String) -> Self {
        Self::String(s.clone())
    }
}

#[derive(Debug, Clone)]
pub struct RegExpResultSlots {
    pub input: Rc<JsString>,
    pub positions: Vec<Option<(usize, usize)>>,
}

static REGEXP_RESULT_LAZY_INDEX_READS: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_LAZY_STRING_MATERIALIZATIONS: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_LAZY_UNDEFINED_READS: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_LAZY_FULL_MATERIALIZATIONS: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_LAZY_FULL_MATERIALIZED_SLOTS: AtomicU64 = AtomicU64::new(0);
static REGEXP_RESULT_LAZY_NUMBER_DIRECT_READS: AtomicU64 = AtomicU64::new(0);

pub fn regexp_result_slot_counter_snapshot() -> (u64, u64, u64, u64, u64, u64) {
    (
        REGEXP_RESULT_LAZY_INDEX_READS.load(Ordering::Relaxed),
        REGEXP_RESULT_LAZY_STRING_MATERIALIZATIONS.load(Ordering::Relaxed),
        REGEXP_RESULT_LAZY_UNDEFINED_READS.load(Ordering::Relaxed),
        REGEXP_RESULT_LAZY_FULL_MATERIALIZATIONS.load(Ordering::Relaxed),
        REGEXP_RESULT_LAZY_FULL_MATERIALIZED_SLOTS.load(Ordering::Relaxed),
        REGEXP_RESULT_LAZY_NUMBER_DIRECT_READS.load(Ordering::Relaxed),
    )
}

impl RegExpResultSlots {
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    pub fn get(&self, i: usize) -> Value {
        REGEXP_RESULT_LAZY_INDEX_READS.fetch_add(1, Ordering::Relaxed);
        match self.positions.get(i).copied().flatten() {
            Some((start, end)) => {
                REGEXP_RESULT_LAZY_STRING_MATERIALIZATIONS.fetch_add(1, Ordering::Relaxed);
                match JsString::slice_wellformed(self.input.clone(), start, end) {
                    Some(slice) => Value::String(Rc::new(slice)),
                    None => Value::String(Rc::new(JsString::from(
                        self.input.as_str()[start..end].to_string(),
                    ))),
                }
            }
            None => {
                REGEXP_RESULT_LAZY_UNDEFINED_READS.fetch_add(1, Ordering::Relaxed);
                Value::Undefined
            }
        }
    }

    pub fn get_ascii_digit_number(&self, i: usize) -> Option<f64> {
        let (start, end) = self.positions.get(i).copied().flatten()?;
        let s = self.input.as_wellformed()?;
        let bytes = s.get(start..end)?.as_bytes();
        if bytes.is_empty() || bytes.len() > 15 {
            return None;
        }
        let mut n = 0u64;
        for &b in bytes {
            if !b.is_ascii_digit() {
                return None;
            }
            n = n * 10 + u64::from(b - b'0');
        }
        REGEXP_RESULT_LAZY_NUMBER_DIRECT_READS.fetch_add(1, Ordering::Relaxed);
        Some(n as f64)
    }

    pub fn materialize(&self) -> Vec<Value> {
        REGEXP_RESULT_LAZY_FULL_MATERIALIZATIONS.fetch_add(1, Ordering::Relaxed);
        REGEXP_RESULT_LAZY_FULL_MATERIALIZED_SLOTS.fetch_add(self.len() as u64, Ordering::Relaxed);
        (0..self.len()).map(|i| self.get(i)).collect()
    }
}

fn shape_enroll_enabled() -> bool {
    static FLAG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *FLAG.get_or_init(|| {
        match std::env::var("CRUFT_SHAPE_ENROLL") {
            Ok(v) => !(v == "0" || v.eq_ignore_ascii_case("false")),
            Err(_) => true,
        }
    })
}

fn packed_i64_sidecar_probe_enabled() -> bool {
    static FLAG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *FLAG.get_or_init(|| match std::env::var("CRUFT_LEJIT_PACKED_I64_SIDECAR") {
        Ok(v) => !(v == "0" || v.eq_ignore_ascii_case("false")),
        Err(_) => false,
    })
}

impl Default for Object {
    fn default() -> Self {
        Self {
            proto: None,
            extensible: true,
            viewed_buffer: None,
            is_buffer: false,
            cruftscript_class_brand: None,
            constructed_display_name: None,
            properties: IndexMap::new(),
            internal_kind: InternalKind::Ordinary,
            shape: None,
            shape_values: Vec::new(),
            private_members: None,
            private_home: None,
            private_outer_homes: Vec::new(),
            private_pending_homes: Vec::new(),
            dense_elements: Vec::new(),
            regexp_result_slots: None,
            array_dense: false,
            dense_doubles: Vec::new(),
            dense_i64_sidecar: None,
            dense_i64_sidecar_valid: false,
            array_packed_all_safe_i64: false,
            array_packed: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PrivateMembers {

    pub fields: IndexMap<String, Value>,

    pub names: IndexMap<String, ()>,

    pub methods: IndexMap<String, ()>,
}

pub struct Object {
    pub proto: Option<ObjectRef>,
    pub extensible: bool,

    pub viewed_buffer: Option<ObjectRef>,

    pub is_buffer: bool,

    pub cruftscript_class_brand: Option<String>,

    pub constructed_display_name: Option<String>,

    pub properties: IndexMap<PropertyKey, PropertyDescriptor>,
    pub internal_kind: InternalKind,

    pub shape: Option<std::rc::Rc<rusty_js_shapes::Shape>>,

    pub shape_values: Vec<Value>,

    pub private_members: Option<Box<PrivateMembers>>,

    pub private_home: Option<ObjectRef>,

    pub private_outer_homes: Vec<ObjectRef>,

    pub private_pending_homes: Vec<ObjectRef>,

    pub dense_elements: Vec<Value>,

    pub regexp_result_slots: Option<Box<RegExpResultSlots>>,

    pub array_dense: bool,

    pub dense_doubles: Vec<f64>,

    pub dense_i64_sidecar: Option<Box<Vec<i64>>>,
    pub dense_i64_sidecar_valid: bool,

    pub array_packed_all_safe_i64: bool,
    pub array_packed: bool,
}

impl Object {
    pub fn new_ordinary() -> Self {

        Self {
            proto: None,
            extensible: true,
            viewed_buffer: None,
            is_buffer: false,
            cruftscript_class_brand: None,
            constructed_display_name: None,
            properties: IndexMap::new(),
            internal_kind: InternalKind::Ordinary,
            shape: if shape_enroll_enabled() {
                Some(rusty_js_shapes::Shape::root())
            } else {
                None
            },
            shape_values: Vec::new(),
            private_members: None,
            private_home: None,
            private_outer_homes: Vec::new(),
            private_pending_homes: Vec::new(),
            dense_elements: Vec::new(),
            regexp_result_slots: None,
            array_dense: false,
            dense_doubles: Vec::new(),
            dense_i64_sidecar: None,
            dense_i64_sidecar_valid: false,
            array_packed_all_safe_i64: false,
            array_packed: false,
        }
    }

    pub fn new_ordinary_with_shape_capacity(shape_capacity: usize) -> Self {
        let mut object = Self::new_ordinary();
        if object.shape.is_some() && shape_capacity > 0 {
            object.shape_values.reserve(shape_capacity);
        }
        object
    }

    pub fn new_ordinary_with_shape_template(
        final_shape: std::rc::Rc<rusty_js_shapes::Shape>,
        slot_count: usize,
    ) -> Self {
        let mut object = Self::new_ordinary();
        object.shape = Some(final_shape);
        object.shape_values = vec![Value::Undefined; slot_count];
        object
    }

    pub fn new_dictionary() -> Self {
        Self {
            proto: None,
            extensible: true,
            viewed_buffer: None,
            is_buffer: false,
            cruftscript_class_brand: None,
            constructed_display_name: None,
            properties: IndexMap::new(),
            internal_kind: InternalKind::Ordinary,
            shape: None,
            shape_values: Vec::new(),
            private_members: None,
            private_home: None,
            private_outer_homes: Vec::new(),
            private_pending_homes: Vec::new(),
            dense_elements: Vec::new(),
            regexp_result_slots: None,
            array_dense: false,
            dense_doubles: Vec::new(),
            dense_i64_sidecar: None,
            dense_i64_sidecar_valid: false,
            array_packed_all_safe_i64: false,
            array_packed: false,
        }
    }

    pub fn new_dictionary_with_property_capacity(property_capacity: usize) -> Self {
        let mut object = Self::new_dictionary();
        if property_capacity > 0 {
            object.properties = IndexMap::with_capacity(property_capacity);
        }
        object
    }

    pub fn new_array() -> Self {

        Self {
            proto: None,
            extensible: true,
            viewed_buffer: None,
            is_buffer: false,
            cruftscript_class_brand: None,
            constructed_display_name: None,
            properties: IndexMap::new(),
            internal_kind: InternalKind::Array,
            shape: None,
            shape_values: Vec::new(),
            private_members: None,
            private_home: None,
            private_outer_homes: Vec::new(),
            private_pending_homes: Vec::new(),
            dense_elements: Vec::new(),
            regexp_result_slots: None,

            array_dense: false,
            dense_doubles: Vec::new(),
            dense_i64_sidecar: None,
            dense_i64_sidecar_valid: false,
            array_packed_all_safe_i64: false,
            array_packed: false,
        }
    }

    pub fn new_array_with_property_capacity(property_capacity: usize) -> Self {
        let mut object = Self::new_array();
        if property_capacity > 0 {
            object.properties = IndexMap::with_capacity(property_capacity);
        }
        object
    }

    pub fn private_members(&self) -> Option<&PrivateMembers> {
        self.private_members.as_deref()
    }

    pub fn private_members_mut(&mut self) -> &mut PrivateMembers {
        self.private_members
            .get_or_insert_with(|| Box::new(PrivateMembers::default()))
    }

    pub fn get_private(&self, key: &str) -> Option<&Value> {
        key.strip_prefix('#')
            .and_then(|name| self.private_members()?.fields.get(name))
    }

    pub fn set_private(&mut self, key: &str, value: Value) -> bool {
        let Some(name) = key.strip_prefix('#') else {
            return false;
        };
        let pm = self.private_members_mut();
        pm.names.insert(name.to_string(), ());
        pm.fields.insert(name.to_string(), value);
        true
    }

    pub fn set_private_method(&mut self, key: &str, value: Value) -> bool {
        let Some(name) = key.strip_prefix('#') else {
            return false;
        };
        let pm = self.private_members_mut();
        pm.names.insert(name.to_string(), ());
        pm.fields.insert(name.to_string(), value);
        pm.methods.insert(name.to_string(), ());
        true
    }

    pub fn mark_private_name(&mut self, key: &str) -> bool {
        let Some(name) = key.strip_prefix('#') else {
            return false;
        };
        self.private_members_mut()
            .names
            .insert(name.to_string(), ());
        true
    }

    pub fn is_private_method(&self, key: &str) -> bool {
        key.strip_prefix('#').is_some_and(|name| {
            self.private_members()
                .is_some_and(|pm| pm.methods.contains_key(name))
        })
    }

    pub fn has_declared_private_name(&self, key: &str) -> bool {
        key.strip_prefix('#').is_some_and(|name| {
            self.private_members()
                .is_some_and(|pm| pm.names.contains_key(name))
        })
    }

    pub fn has_private_names(&self) -> bool {
        self.private_members().is_some_and(|pm| {
            !pm.names.is_empty() || !pm.fields.is_empty() || !pm.methods.is_empty()
        })
    }

    pub fn set_private_home(&mut self, home: ObjectRef) {
        if let Some(prev) = self.private_home {
            if prev != home && !self.private_outer_homes.contains(&prev) {
                self.private_outer_homes.insert(0, prev);
            }
        }
        self.private_outer_homes.retain(|outer| *outer != home);
        self.private_home = Some(home);
    }

    pub fn inherit_private_environment(&mut self, home: Option<ObjectRef>, outers: &[ObjectRef]) {
        for outer in outers.iter().rev().copied() {
            if Some(outer) != self.private_home && !self.private_outer_homes.contains(&outer) {
                self.private_outer_homes.insert(0, outer);
            }
        }
        if let Some(home) = home {
            if Some(home) != self.private_home && !self.private_outer_homes.contains(&home) {
                self.private_outer_homes.insert(0, home);
            }
        }
    }

    pub fn mark_private_home_pending(&mut self, home: ObjectRef) {
        if !self.private_pending_homes.contains(&home) {
            self.private_pending_homes.push(home);
        }
    }

    pub fn clear_private_home_pending(&mut self, home: ObjectRef) {
        self.private_pending_homes
            .retain(|pending| *pending != home);
    }

    pub fn is_private_home_pending(&self, home: ObjectRef) -> bool {
        self.private_pending_homes.contains(&home)
    }

    pub fn is_shaped(&self) -> bool {
        self.shape.is_some()
    }

    pub fn shape_get(&self, name: &str) -> Option<&Value> {
        let shape = self.shape.as_ref()?;
        let slot = shape.slot_of(name)? as usize;
        self.shape_values.get(slot)
    }

    pub fn shape_ptr_and_slot_for(
        &self,
        name: &str,
    ) -> Option<(*const rusty_js_shapes::Shape, u32)> {
        let shape = self.shape.as_ref()?;
        let slot = shape.slot_of(name)?;
        Some((std::rc::Rc::as_ptr(shape), slot))
    }

    pub fn dict_mut(&mut self) -> &mut IndexMap<PropertyKey, PropertyDescriptor> {
        self.migrate_to_dictionary();
        &mut self.properties
    }

    pub fn migrate_to_dictionary(&mut self) {
        let Some(shape) = self.shape.take() else {
            return;
        };
        let values = std::mem::take(&mut self.shape_values);
        for (name, slot) in shape.iter_slots() {
            let idx = slot as usize;
            if idx >= values.len() {
                continue;
            }
            self.properties.insert(
                PropertyKey::String(name.to_string()),
                PropertyDescriptor {
                    value: values[idx].clone(),
                    writable: true,
                    enumerable: true,
                    configurable: true,
                    getter: None,
                    setter: None,
                },
            );
        }
    }

    pub fn array_densify_migrate(&mut self) {
        if !self.array_dense {
            return;
        }

        self.array_depack();
        self.array_dense = false;
        let elems = if let Some(slots) = self.regexp_result_slots.take() {
            slots.materialize()
        } else {
            std::mem::take(&mut self.dense_elements)
        };
        for (i, v) in elems.into_iter().enumerate() {
            self.properties.insert(
                PropertyKey::String(i.to_string()),
                PropertyDescriptor {
                    value: v,
                    writable: true,
                    enumerable: true,
                    configurable: true,
                    getter: None,
                    setter: None,
                },
            );
        }

    }

    pub fn get_own(&self, key: &str) -> Option<&PropertyDescriptor> {

        self.properties.get(&PropertyKey::String(key.to_string()))
    }

    pub fn get_own_str_borrowed(&self, key: &str) -> Option<&PropertyDescriptor> {
        if self.properties.len() > 16 {
            return self.get_own(key);
        }
        self.properties.iter().find_map(|(pk, desc)| match pk {
            PropertyKey::String(s) if s == key => Some(desc),
            _ => None,
        })
    }

    #[inline]
    pub fn array_store_len(&self) -> usize {
        if let Some(slots) = &self.regexp_result_slots {
            return slots.len();
        }
        if self.array_packed {
            self.dense_doubles.len()
        } else {
            self.dense_elements.len()
        }
    }

    #[inline]
    pub fn array_store_get(&self, i: usize) -> Value {
        if let Some(slots) = &self.regexp_result_slots {
            return slots.get(i);
        }
        if self.array_packed {
            Value::Number(self.dense_doubles[i])
        } else {
            self.dense_elements[i].clone()
        }
    }

    pub fn array_depack(&mut self) {
        if !self.array_packed {
            return;
        }
        self.array_packed = false;
        self.array_packed_all_safe_i64 = false;
        self.clear_packed_i64_sidecar();
        let doubles = std::mem::take(&mut self.dense_doubles);
        self.dense_elements = doubles.into_iter().map(Value::Number).collect();
        self.regexp_result_slots = None;
    }

    #[inline]
    pub fn is_safe_i64_number(n: f64) -> bool {
        const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_992.0;
        n.is_finite() && n.fract() == 0.0 && n >= -MAX_SAFE_INTEGER && n <= MAX_SAFE_INTEGER
    }

    #[inline]
    pub fn note_packed_number_write(&mut self, n: f64) {
        self.array_packed_all_safe_i64 &= Self::is_safe_i64_number(n);
        if packed_i64_sidecar_probe_enabled() {
            self.rebuild_packed_i64_sidecar();
        }
    }

    #[inline]
    pub fn clear_packed_i64_sidecar(&mut self) {
        if let Some(sidecar) = self.dense_i64_sidecar.as_mut() {
            sidecar.clear();
        }
        self.dense_i64_sidecar_valid = false;
    }

    #[inline]
    pub fn truncate_packed_doubles(&mut self, len: usize) {
        self.dense_doubles.truncate(len);
        if self.dense_i64_sidecar_valid {
            if let Some(sidecar) = self.dense_i64_sidecar.as_mut() {
                sidecar.truncate(len);
            }
        } else if packed_i64_sidecar_probe_enabled() {
            self.rebuild_packed_i64_sidecar();
        }
    }

    #[inline]
    pub fn pop_packed_double(&mut self) -> Option<f64> {
        let value = self.dense_doubles.pop();
        if value.is_some() {
            if self.dense_i64_sidecar_valid {
                if let Some(sidecar) = self.dense_i64_sidecar.as_mut() {
                    sidecar.pop();
                }
            } else if packed_i64_sidecar_probe_enabled() {
                self.rebuild_packed_i64_sidecar();
            }
        }
        value
    }

    pub fn rebuild_packed_i64_sidecar(&mut self) {
        self.clear_packed_i64_sidecar();
        if !self.array_packed || !self.array_packed_all_safe_i64 {
            return;
        }
        let mut sidecar = Vec::with_capacity(self.dense_doubles.len());
        for value in self.dense_doubles.iter().copied() {
            if !Self::is_safe_i64_number(value) {
                self.array_packed_all_safe_i64 = false;
                self.clear_packed_i64_sidecar();
                return;
            }
            sidecar.push(value as i64);
        }
        self.dense_i64_sidecar = Some(Box::new(sidecar));
        self.dense_i64_sidecar_valid = true;
    }

    pub fn try_repack(&mut self) {
        if self.array_packed || !self.array_dense {
            return;
        }
        if self
            .dense_elements
            .iter()
            .all(|v| matches!(v, Value::Number(_)))
        {
            let boxed = std::mem::take(&mut self.dense_elements);
            self.dense_doubles = boxed
                .into_iter()
                .map(|v| match v {
                    Value::Number(n) => n,
                    _ => unreachable!("checked all-Number above"),
                })
                .collect();
            self.array_packed = true;
            self.array_packed_all_safe_i64 = self
                .dense_doubles
                .iter()
                .copied()
                .all(Self::is_safe_i64_number);
            if packed_i64_sidecar_probe_enabled() {
                self.rebuild_packed_i64_sidecar();
            }
        }
    }

    pub fn dense_index_present(&self, key: &str) -> bool {
        if !self.array_dense {
            return false;
        }
        if key.len() > 1 && key.starts_with('0') {
            return false;
        }
        matches!(key.parse::<usize>(), Ok(i) if i < self.array_store_len())
    }

    pub fn get_own_symbol(&self, sym: &std::rc::Rc<String>) -> Option<&PropertyDescriptor> {
        self.properties.get(&PropertyKey::Symbol(sym.clone()))
    }

    pub fn get_own_mut(&mut self, key: &str) -> Option<&mut PropertyDescriptor> {
        self.properties
            .get_mut(&PropertyKey::String(key.to_string()))
    }

    pub fn has_own_str(&self, key: &str) -> bool {

        if self.array_dense {
            if let Ok(i) = key.parse::<usize>() {
                if i < self.array_store_len() && !(key.len() > 1 && key.starts_with('0')) {
                    return true;
                }
            }
        }
        if let Some(shape) = self.shape.as_ref() {
            if shape.slot_of(key).is_some() {
                return true;
            }
        }
        if self
            .properties
            .contains_key(&PropertyKey::String(key.to_string()))
        {
            return true;
        }
        if key == "length" && matches!(self.internal_kind, InternalKind::Array) {
            return true;
        }
        false
    }

    pub fn remove_str(&mut self, key: &str) -> Option<PropertyDescriptor> {
        self.migrate_to_dictionary();
        self.properties
            .shift_remove(&PropertyKey::String(key.to_string()))
    }

    pub fn insert_str(
        &mut self,
        key: impl Into<String>,
        desc: PropertyDescriptor,
    ) -> Option<PropertyDescriptor> {
        self.migrate_to_dictionary();
        self.properties
            .insert(PropertyKey::String(key.into()), desc)
    }

    pub fn string_keys(&self) -> impl Iterator<Item = &str> {
        let shape_names: Vec<&str> = match self.shape.as_ref() {
            Some(shape) => shape.iter_slots().map(|(n, _)| n).collect(),
            None => Vec::new(),
        };
        let prop_names: Vec<&str> = self
            .properties
            .keys()
            .filter_map(|k| match k {
                PropertyKey::String(s) => Some(s.as_str()),
                PropertyKey::Symbol(_) => None,
            })
            .collect();
        shape_names.into_iter().chain(prop_names)
    }

    pub fn string_key_clones(&self) -> impl Iterator<Item = String> + '_ {
        let shape_names: Vec<String> = match self.shape.as_ref() {
            Some(shape) => shape.iter_slots().map(|(n, _)| n.to_string()).collect(),
            None => Vec::new(),
        };
        let prop_names: Vec<String> = self
            .properties
            .keys()
            .filter_map(|k| match k {
                PropertyKey::String(s) => Some(s.clone()),
                PropertyKey::Symbol(_) => None,
            })
            .collect();
        shape_names.into_iter().chain(prop_names)
    }

    pub fn set_own(&mut self, key: String, value: Value) {

        if key.starts_with("__") {
            self.migrate_to_dictionary();
        }
        if let Some(shape) = self.shape.as_ref() {
            if let Some(slot) = shape.slot_of(&key) {
                self.shape_values[slot as usize] = value;
                return;
            }
            let next = shape.transition_to(&key);
            self.shape = Some(next);
            self.shape_values.push(value);
            return;
        }
        let pk = PropertyKey::String(key);
        if let Some(d) = self.properties.get_mut(&pk) {
            if d.getter.is_some() || d.setter.is_some() {

                *d = PropertyDescriptor {
                    value,
                    writable: true,
                    enumerable: true,
                    configurable: true,
                    getter: None,
                    setter: None,
                };
            } else {
                d.value = value;
            }
            return;
        }
        self.properties.insert(
            pk,
            PropertyDescriptor {
                value,
                writable: true,
                enumerable: true,
                configurable: true,
                getter: None,
                setter: None,
            },
        );
    }

    pub fn set_own_literal_key(&mut self, key: &str, value: Value) {
        if key.starts_with("__") {
            self.migrate_to_dictionary();
        }
        if let Some(shape) = self.shape.as_ref() {
            if let Some(slot) = shape.slot_of(key) {
                self.shape_values[slot as usize] = value;
                return;
            }
            let next = shape.transition_to(key);
            self.shape = Some(next);
            self.shape_values.push(value);
            return;
        }
        let pk = PropertyKey::String(key.to_string());
        if let Some(d) = self.properties.get_mut(&pk) {
            if d.getter.is_some() || d.setter.is_some() {
                *d = PropertyDescriptor {
                    value,
                    writable: true,
                    enumerable: true,
                    configurable: true,
                    getter: None,
                    setter: None,
                };
            } else {
                d.value = value;
            }
            return;
        }
        self.properties.insert(
            pk,
            PropertyDescriptor {
                value,
                writable: true,
                enumerable: true,
                configurable: true,
                getter: None,
                setter: None,
            },
        );
    }

    pub fn set_own_internal(&mut self, key: String, value: Value) {

        self.migrate_to_dictionary();
        self.properties.insert(
            PropertyKey::String(key),
            PropertyDescriptor {
                value,
                writable: true,
                enumerable: false,
                configurable: true,
                getter: None,
                setter: None,
            },
        );
    }

    pub fn set_own_internal_symbol(&mut self, key: std::rc::Rc<String>, value: Value) {
        self.migrate_to_dictionary();
        self.properties.insert(
            PropertyKey::Symbol(key),
            PropertyDescriptor {
                value,
                writable: true,
                enumerable: false,
                configurable: true,
                getter: None,
                setter: None,
            },
        );
    }

    pub fn set_own_frozen(&mut self, key: String, value: Value) {

        self.migrate_to_dictionary();
        self.properties.insert(
            PropertyKey::String(key),
            PropertyDescriptor {
                value,
                writable: false,
                enumerable: false,
                configurable: false,
                getter: None,
                setter: None,
            },
        );
    }

    pub fn set_own_string_index(&mut self, key: String, value: Value) {
        self.migrate_to_dictionary();
        self.properties.insert(
            PropertyKey::String(key),
            PropertyDescriptor {
                value,
                writable: false,
                enumerable: true,
                configurable: false,
                getter: None,
                setter: None,
            },
        );
    }

    pub fn set_own_module_export(&mut self, key: String, value: Value) {
        self.migrate_to_dictionary();
        self.properties.insert(
            PropertyKey::String(key),
            PropertyDescriptor {
                value,
                writable: true,
                enumerable: true,
                configurable: false,
                getter: None,
                setter: None,
            },
        );
    }
}

#[derive(Debug, Clone)]
pub struct PropertyDescriptor {
    pub value: Value,
    pub writable: bool,
    pub enumerable: bool,
    pub configurable: bool,

    pub getter: Option<Value>,
    pub setter: Option<Value>,
}

#[derive(Debug)]
pub enum InternalKind {
    Ordinary,

    IsHtmlDda,
    Array,
    Function(Box<FunctionInternals>),
    Closure(Box<ClosureInternals>),
    BoundFunction(Box<BoundFunctionInternals>),
    Error,
    ModuleNamespace,

    Promise(Box<PromiseState>),

    RegExp(Box<RegExpInternals>),

    Proxy(ProxyInternals),

    BoundaryWrapper(Box<BoundaryWrapperInternals>),

    NumberWrapper(Value),
    StringWrapper(Value),
    BooleanWrapper(Value),
    BigIntWrapper(Value),

    Generator(Box<GeneratorObject>),

    MappedArguments {

        parameter_map: Box<IndexMap<String, UpvalueCell>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratorState {
    SuspendedStart,
    SuspendedYield,
    AwaitingYield,
    Executing,
    Completed,
}

#[derive(Debug)]
pub struct GeneratorObject {
    pub state: GeneratorState,
    pub continuation: Option<Box<crate::interp::FrameSnapshot>>,
    pub yielded_value: Option<Value>,
    pub yielded_delegate_result: bool,
    pub pending_return: Option<Value>,
    pub delegate: Option<GeneratorDelegate>,

    pub request_queue: std::collections::VecDeque<AsyncGenRequest>,

    pub is_async: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsyncGenRequestKind {
    Next,
    Throw,
    Return,
}

#[derive(Debug, Clone)]
pub struct AsyncGenRequest {
    pub kind: AsyncGenRequestKind,
    pub value: Value,

    pub promise: ObjectRef,
}

#[derive(Debug, Clone)]
pub struct GeneratorDelegate {
    pub iterator: ObjectRef,
    pub next_method: Value,

    pub is_async: bool,

    pub async_from_sync: bool,
}

#[derive(Debug)]
pub struct BoundaryWrapperInternals {
    pub target: Value,
    pub policy_id: u32,
    pub continue_mode: BoundaryContinueMode,
    pub validator: Value,
    pub sanitize_arg_defaults: Vec<Option<Value>>,
    pub sanitize_return_default: Option<Value>,

    pub installer_compartment: Option<ObjectRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryContinueMode {
    Halt,
    PropagateAsUnknown,
    Sanitize,
    TrustWithOptOutRecorded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompartmentPolicy {
    Secure,
    Debug,
    Override(u32),
    Weaken(u32),
}

impl CompartmentPolicy {

    pub fn named_level(self) -> Option<u8> {
        match self {
            CompartmentPolicy::Secure => Some(1),
            CompartmentPolicy::Debug => Some(0),
            _ => None,
        }
    }

    pub fn level_of_id(id: u32) -> u8 {
        if id == 0 {
            0
        } else {
            1
        }
    }

    pub fn resolve_over(self, inherited: u8) -> u8 {
        match self {
            CompartmentPolicy::Secure => 1,
            CompartmentPolicy::Debug => 0,
            CompartmentPolicy::Override(id) => CompartmentPolicy::level_of_id(id),
            CompartmentPolicy::Weaken(id) => inherited.min(CompartmentPolicy::level_of_id(id)),
        }
    }
}

#[derive(Debug)]
pub struct ProxyInternals {
    pub target: ObjectRef,
    pub handler: ObjectRef,

    pub revoked: bool,
}

#[derive(Debug)]
pub struct RegExpInternals {
    pub source: Rc<String>,
    pub flags: Rc<String>,

    pub compiled: Option<CompiledRegex>,
    pub last_index: usize,
}

#[derive(Debug)]
pub enum CompiledRegex {
    Hand(crate::rusty_js_regex::HandRolledRegex),
}

impl Clone for CompiledRegex {
    fn clone(&self) -> Self {
        match self {
            CompiledRegex::Hand(h) => CompiledRegex::Hand(h.clone()),
        }
    }
}

impl CompiledRegex {

    pub fn named_groups(&self) -> Vec<(String, usize)> {
        match self {
            CompiledRegex::Hand(h) => h
                .named_groups
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
        }
    }
    pub fn named_group_slots(&self) -> Vec<(String, Vec<usize>)> {
        match self {
            CompiledRegex::Hand(h) => {
                let mut out: Vec<(String, Vec<usize>)> = Vec::new();
                for (name, idx) in &h.named_group_order {
                    if let Some((_, slots)) = out.iter_mut().find(|(n, _)| n == name) {
                        slots.push(*idx);
                    } else {
                        out.push((name.clone(), vec![*idx]));
                    }
                }
                out
            }
        }
    }
    pub fn is_match(&self, input: &str) -> bool {
        match self {
            CompiledRegex::Hand(h) => {
                if input.is_ascii() {
                    let units: Vec<u16> = input.as_bytes().iter().map(|b| *b as u16).collect();
                    crate::rusty_js_regex::find_at(h, &units, 0).is_some()
                } else {
                    crate::rusty_js_regex::is_match(h, input)
                }
            }
        }
    }

    pub fn captures_at(
        &self,
        input: &str,
        start: usize,
    ) -> Option<(usize, usize, Vec<Option<String>>)> {
        match self {
            CompiledRegex::Hand(h) => {

                if input.is_ascii() {
                    let units: Vec<u16> = input.as_bytes().iter().map(|b| *b as u16).collect();
                    return crate::rusty_js_regex::find_at(h, &units, start).map(|m| {
                        let groups: Vec<Option<String>> = m
                            .captures
                            .iter()
                            .map(|c| {
                                c.map(|(s, e)| {
                                    input
                                        .get(s..e)
                                        .expect("ASCII regex capture range is byte-aligned")
                                        .to_string()
                                })
                            })
                            .collect();
                        (m.start, m.end, groups)
                    });
                }
                let units: Vec<u16> = input.encode_utf16().collect();
                let to_cu = |byte: usize| input[..byte.min(input.len())].encode_utf16().count();
                let to_byte =
                    |cu: usize| String::from_utf16_lossy(&units[..cu.min(units.len())]).len();
                crate::rusty_js_regex::find_at(h, &units, to_cu(start)).map(|m| {
                    let groups: Vec<Option<String>> = m
                        .captures
                        .iter()
                        .map(|c| c.map(|(s, e)| String::from_utf16_lossy(&units[s..e])))
                        .collect();
                    (to_byte(m.start), to_byte(m.end), groups)
                })
            }
        }
    }

    pub fn captures_positions_at(
        &self,
        input: &str,
        start: usize,
    ) -> Option<(usize, usize, Vec<Option<(usize, usize)>>)> {
        match self {
            CompiledRegex::Hand(h) => {
                if input.is_ascii() {
                    let units: Vec<u16> = input.as_bytes().iter().map(|b| *b as u16).collect();
                    return crate::rusty_js_regex::find_at(h, &units, start).map(|m| {
                        let caps: Vec<Option<(usize, usize)>> = m.captures.to_vec();
                        (m.start, m.end, caps)
                    });
                }
                let units: Vec<u16> = input.encode_utf16().collect();
                let to_cu = |byte: usize| input[..byte.min(input.len())].encode_utf16().count();
                let to_byte =
                    |cu: usize| String::from_utf16_lossy(&units[..cu.min(units.len())]).len();
                crate::rusty_js_regex::find_at(h, &units, to_cu(start)).map(|m| {
                    let caps: Vec<Option<(usize, usize)>> = m
                        .captures
                        .iter()
                        .map(|c| c.map(|(s, e)| (to_byte(s), to_byte(e))))
                        .collect();
                    (to_byte(m.start), to_byte(m.end), caps)
                })
            }
        }
    }

    pub fn find_iter_owned(&self, input: &str) -> Vec<(usize, usize, String)> {
        match self {
            CompiledRegex::Hand(h) => {
                let units: Vec<u16> = input.encode_utf16().collect();
                let to_byte =
                    |cu: usize| String::from_utf16_lossy(&units[..cu.min(units.len())]).len();
                let mut out = Vec::new();
                let mut start = 0usize;
                while start <= units.len() {
                    match crate::rusty_js_regex::find_at(h, &units, start) {
                        Some(m) => {
                            let s = String::from_utf16_lossy(&units[m.start..m.end]);
                            out.push((to_byte(m.start), to_byte(m.end), s));

                            start = if m.end == m.start { m.end + 1 } else { m.end };
                        }
                        None => break,
                    }
                }
                out
            }
        }
    }

    pub fn find_first(&self, input: &str) -> Option<(usize, usize)> {
        self.captures_at(input, 0).map(|(s, e, _)| (s, e))
    }

    pub fn split_str(&self, input: &str) -> Vec<String> {

        let matches = self.find_iter_owned(input);
        if input.is_empty() {
            if matches.iter().any(|(s, e, _)| *s == 0 && *e == 0) {
                return Vec::new();
            }
            return vec![String::new()];
        }
        let mut out = Vec::new();
        let mut p: usize = 0;
        for (ms, me, _) in matches {

            if ms >= input.len() {
                break;
            }
            if me == p {
                continue;
            }
            if ms < p {
                continue;
            }
            out.push(input[p..ms].to_string());
            p = me;
        }
        out.push(input[p..].to_string());
        out
    }

    pub fn replacen_lit(&self, input: &str, n: usize, repl: &str) -> String {
        match self {
            CompiledRegex::Hand(_) => {
                let matches = self.find_iter_owned(input);
                let mut out = String::new();
                let mut cursor = 0;
                for (i, (ms, me, _)) in matches.into_iter().enumerate() {
                    if i >= n {
                        break;
                    }
                    out.push_str(&input[cursor..ms]);
                    out.push_str(repl);
                    cursor = me;
                }
                out.push_str(&input[cursor..]);
                out
            }
        }
    }
    pub fn replace_all_lit(&self, input: &str, repl: &str) -> String {
        match self {
            CompiledRegex::Hand(_) => self.replacen_lit(input, usize::MAX, repl),
        }
    }
}

#[derive(Debug)]
pub struct PromiseState {
    pub status: PromiseStatus,
    pub value: Value,
    pub fulfill_reactions: Vec<PromiseReaction>,
    pub reject_reactions: Vec<PromiseReaction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromiseStatus {
    Pending,
    Fulfilled,
    Rejected,
}

#[derive(Debug)]
pub struct PromiseReaction {
    pub handler: Option<PromiseReactionHandler>,

    pub chain: ObjectRef,

    pub cap_resolve: Option<Value>,

    pub cap_reject: Option<Value>,
}

#[derive(Debug, Clone)]
pub enum PromiseReactionHandler {
    Callable(Value),
    LazyArrow(PromiseLazyArrowHandler),
    LazyArrowOneCell(PromiseLazyArrowOneCellHandler),
    AsyncAwaitContinuation {
        promise: ObjectRef,
        snapshot: Box<crate::interp::FrameSnapshot>,
    },
}

#[derive(Debug, Clone)]
pub struct PromiseLazyArrowHandler {
    pub proto: Rc<rusty_js_bytecode::compiler::FunctionProto>,
    pub upvalues: Vec<UpvalueCell>,
    pub captured_bindings: Vec<CapturedBinding>,
    pub creation_realm: usize,
    pub creation_global: Option<ObjectRef>,
    pub import_meta: Option<ObjectRef>,
    pub bound_this: Option<Value>,
    pub bound_this_cell: Option<UpvalueCell>,
    pub bound_derived_initial_this: Option<Value>,
    pub bound_executing_function: Option<ObjectRef>,
    pub bound_new_target: Option<Value>,
    pub bound_new_target_allowed: bool,
    pub captured_with_env_stack: Vec<ObjectRef>,
}

impl PromiseLazyArrowHandler {
    pub fn into_reaction_handler(self) -> PromiseReactionHandler {
        if promise_lazy_one_cell_payload_enabled()
            && self.upvalues.len() == 1
            && self.captured_bindings.len() == 1
            && self.import_meta.is_none()
            && self.bound_this_cell.is_none()
            && self.bound_derived_initial_this.is_none()
            && self.bound_new_target.is_none()
            && self.captured_with_env_stack.is_empty()
        {
            let mut upvalues = self.upvalues;
            let mut captured_bindings = self.captured_bindings;
            let upvalue = upvalues.pop().expect("one upvalue checked above");
            match captured_bindings.pop().expect("one binding checked above") {
                CapturedBinding::Cell(binding_cell) if Rc::ptr_eq(&binding_cell, &upvalue) => {
                    return PromiseReactionHandler::LazyArrowOneCell(
                        PromiseLazyArrowOneCellHandler {
                            proto: self.proto,
                            upvalue,
                            creation_realm: self.creation_realm,
                            creation_global: self.creation_global,
                            bound_this: self.bound_this,
                            bound_executing_function: self.bound_executing_function,
                            bound_new_target_allowed: self.bound_new_target_allowed,
                        },
                    );
                }
                binding => {
                    captured_bindings.push(binding);
                    upvalues.push(upvalue);
                    return PromiseReactionHandler::LazyArrow(PromiseLazyArrowHandler {
                        proto: self.proto,
                        upvalues,
                        captured_bindings,
                        creation_realm: self.creation_realm,
                        creation_global: self.creation_global,
                        import_meta: self.import_meta,
                        bound_this: self.bound_this,
                        bound_this_cell: self.bound_this_cell,
                        bound_derived_initial_this: self.bound_derived_initial_this,
                        bound_executing_function: self.bound_executing_function,
                        bound_new_target: self.bound_new_target,
                        bound_new_target_allowed: self.bound_new_target_allowed,
                        captured_with_env_stack: self.captured_with_env_stack,
                    });
                }
            }
        }
        PromiseReactionHandler::LazyArrow(self)
    }
}

fn promise_lazy_one_cell_payload_enabled() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("CRUFT_PROMISE_LAZY_ONE_CELL_PAYLOAD")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(true)
    })
}

#[derive(Debug, Clone)]
pub struct PromiseLazyArrowOneCellHandler {
    pub proto: Rc<rusty_js_bytecode::compiler::FunctionProto>,
    pub upvalue: UpvalueCell,
    pub creation_realm: usize,
    pub creation_global: Option<ObjectRef>,
    pub bound_this: Option<Value>,
    pub bound_executing_function: Option<ObjectRef>,
    pub bound_new_target_allowed: bool,
}

impl InternalKind {
    pub fn kind_name(&self) -> &'static str {
        match self {
            InternalKind::Ordinary => "ordinary",
            InternalKind::IsHtmlDda => "is-html-dda",
            InternalKind::Array => "array",
            InternalKind::Function(_) => "function",
            InternalKind::Promise(_) => "promise",
            InternalKind::Closure(_) => "closure",
            InternalKind::BoundFunction(_) => "bound-function",
            InternalKind::Error => "error",
            InternalKind::ModuleNamespace => "module-namespace",
            InternalKind::RegExp(_) => "regexp",
            InternalKind::Proxy(_) => "proxy",
            InternalKind::BoundaryWrapper(_) => "boundary-wrapper",
            InternalKind::NumberWrapper(_) => "number-wrapper",
            InternalKind::StringWrapper(_) => "string-wrapper",
            InternalKind::BooleanWrapper(_) => "boolean-wrapper",
            InternalKind::BigIntWrapper(_) => "bigint-wrapper",
            InternalKind::Generator(_) => "generator",
            InternalKind::MappedArguments { .. } => "mapped-arguments",
        }
    }
}

pub enum FunctionProtoCarrier {
    Eager(Rc<rusty_js_bytecode::compiler::FunctionProto>),
    Lazy {
        metadata: rusty_js_bytecode::constants::FunctionConstantMetadata,
        cache: Rc<
            RefCell<
                Option<
                    Result<
                        Rc<rusty_js_bytecode::compiler::FunctionProto>,
                        rusty_js_bytecode::compiler::CompileError,
                    >,
                >,
            >,
        >,
        materialize: Rc<
            dyn Fn() -> Result<
                Rc<rusty_js_bytecode::compiler::FunctionProto>,
                rusty_js_bytecode::compiler::CompileError,
            >,
        >,
        materialization_count: Rc<Cell<u32>>,
    },
}

impl fmt::Debug for FunctionProtoCarrier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FunctionProtoCarrier::Eager(proto) => f.debug_tuple("Eager").field(proto).finish(),
            FunctionProtoCarrier::Lazy {
                metadata,
                cache,
                materialization_count,
                ..
            } => f
                .debug_struct("Lazy")
                .field("metadata", metadata)
                .field("cached", &cache.borrow().is_some())
                .field("materialization_count", &materialization_count.get())
                .finish(),
        }
    }
}

impl Clone for FunctionProtoCarrier {
    fn clone(&self) -> Self {
        match self {
            FunctionProtoCarrier::Eager(proto) => FunctionProtoCarrier::Eager(proto.clone()),
            FunctionProtoCarrier::Lazy {
                metadata,
                cache,
                materialize,
                materialization_count,
            } => FunctionProtoCarrier::Lazy {
                metadata: metadata.clone(),
                cache: cache.clone(),
                materialize: materialize.clone(),
                materialization_count: materialization_count.clone(),
            },
        }
    }
}

impl FunctionProtoCarrier {
    pub fn from_lazy_materializer(
        materialize: impl Fn() -> Rc<rusty_js_bytecode::compiler::FunctionProto> + 'static,
    ) -> Self {
        Self::from_lazy_materializer_with_metadata(
            rusty_js_bytecode::constants::FunctionConstantMetadata::default(),
            materialize,
        )
    }

    pub fn from_lazy_materializer_with_metadata(
        metadata: rusty_js_bytecode::constants::FunctionConstantMetadata,
        materialize: impl Fn() -> Rc<rusty_js_bytecode::compiler::FunctionProto> + 'static,
    ) -> Self {
        Self::from_fallible_lazy_materializer_with_metadata(metadata, move || Ok(materialize()))
    }

    pub fn from_fallible_lazy_materializer_with_metadata(
        metadata: rusty_js_bytecode::constants::FunctionConstantMetadata,
        materialize: impl Fn() -> Result<
                Rc<rusty_js_bytecode::compiler::FunctionProto>,
                rusty_js_bytecode::compiler::CompileError,
            > + 'static,
    ) -> Self {
        FunctionProtoCarrier::Lazy {
            metadata,
            cache: Rc::new(RefCell::new(None)),
            materialize: Rc::new(materialize),
            materialization_count: Rc::new(Cell::new(0)),
        }
    }

    pub fn from_lazy_bytecode_constant(
        lazy: rusty_js_bytecode::constants::LazyFunctionConstant,
    ) -> Self {
        let metadata = lazy.metadata().clone();
        Self::from_fallible_lazy_materializer_with_metadata(metadata, move || lazy.proto_result())
    }

    pub fn metadata(&self) -> rusty_js_bytecode::constants::FunctionConstantMetadata {
        match self {
            FunctionProtoCarrier::Eager(proto) => {
                rusty_js_bytecode::constants::FunctionConstantMetadata::from_proto(proto.as_ref())
            }
            FunctionProtoCarrier::Lazy { metadata, .. } => metadata.clone(),
        }
    }

    pub fn proto_result(
        &self,
    ) -> Result<
        Rc<rusty_js_bytecode::compiler::FunctionProto>,
        rusty_js_bytecode::compiler::CompileError,
    > {
        match self {
            FunctionProtoCarrier::Eager(proto) => Ok(proto.clone()),
            FunctionProtoCarrier::Lazy {
                metadata: _,
                cache,
                materialize,
                materialization_count,
            } => {
                if let Some(result) = cache.borrow().as_ref().cloned() {
                    return result;
                }
                let result = materialize();
                materialization_count.set(materialization_count.get() + 1);
                *cache.borrow_mut() = Some(result.clone());
                result
            }
        }
    }

    pub fn proto_rc(&self) -> Rc<rusty_js_bytecode::compiler::FunctionProto> {
        self.proto_result()
            .unwrap_or_else(|err| panic!("lazy function materialization failed: {}", err.message))
    }

    pub fn as_ref_if_materialized(&self) -> Option<&rusty_js_bytecode::compiler::FunctionProto> {
        match self {
            FunctionProtoCarrier::Eager(proto) => Some(proto.as_ref()),
            FunctionProtoCarrier::Lazy { cache, .. } => {
                let _ = cache;
                None
            }
        }
    }

    pub fn materialization_count_for_tests(&self) -> u32 {
        match self {
            FunctionProtoCarrier::Eager(_) => 0,
            FunctionProtoCarrier::Lazy {
                metadata: _,
                materialization_count,
                ..
            } => materialization_count.get(),
        }
    }
}

impl From<Rc<rusty_js_bytecode::compiler::FunctionProto>> for FunctionProtoCarrier {
    fn from(proto: Rc<rusty_js_bytecode::compiler::FunctionProto>) -> Self {
        FunctionProtoCarrier::Eager(proto)
    }
}

impl AsRef<rusty_js_bytecode::compiler::FunctionProto> for FunctionProtoCarrier {
    fn as_ref(&self) -> &rusty_js_bytecode::compiler::FunctionProto {
        match self {
            FunctionProtoCarrier::Eager(proto) => proto.as_ref(),
            FunctionProtoCarrier::Lazy { .. } => {

                let rc = self.proto_rc();

                unsafe { &*std::rc::Rc::as_ptr(&rc) }
            }
        }
    }
}

impl std::ops::Deref for FunctionProtoCarrier {
    type Target = Rc<rusty_js_bytecode::compiler::FunctionProto>;

    fn deref(&self) -> &Self::Target {
        match self {
            FunctionProtoCarrier::Eager(proto) => proto,
            FunctionProtoCarrier::Lazy { cache, .. } => {

                let _ = self.proto_rc();
                let borrow = cache.borrow();
                match borrow.as_ref() {
                    Some(Ok(rc)) => {

                        unsafe {
                            &*(rc as *const std::rc::Rc<rusty_js_bytecode::compiler::FunctionProto>)
                        }
                    }
                    _ => unreachable!("proto_rc materialized the cache to Some(Ok)"),
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct ClosureInternals {
    pub proto: FunctionProtoCarrier,

    pub creation_realm: usize,
    pub creation_global: Option<ObjectRef>,

    pub import_meta: Option<ObjectRef>,

    pub upvalues: Vec<UpvalueCell>,
    pub captured_bindings: Vec<CapturedBinding>,
    pub is_arrow: bool,

    pub bound_this: Option<Value>,

    pub bound_this_cell: Option<UpvalueCell>,

    pub bound_derived_initial_this: Option<Value>,

    pub bound_executing_function: Option<ObjectRef>,

    pub bound_new_target: Option<Value>,

    pub bound_new_target_allowed: bool,

    pub bound_arguments_forbidden: bool,

    pub captured_with_env_stack: Vec<ObjectRef>,

    pub call_count: std::cell::Cell<u32>,

    pub jit_disabled: std::cell::Cell<bool>,

    pub tb_metadata_ptr: std::cell::Cell<Option<std::ptr::NonNull<()>>>,
}

pub struct FunctionInternals {
    pub name: String,

    pub length: u32,
    pub native: NativeFn,

    pub is_constructor: bool,

    pub creation_realm: usize,

    pub roots: Vec<ObjectRef>,
}

impl std::fmt::Debug for FunctionInternals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FunctionInternals {{ name: {:?}, length: {} }}",
            self.name, self.length
        )
    }
}

pub fn install_function_meta_props(
    properties: &mut indexmap::IndexMap<PropertyKey, PropertyDescriptor>,
    name: &str,
    length: f64,
) {
    properties.insert(
        PropertyKey::String("length".to_string()),
        PropertyDescriptor {
            value: Value::Number(length),
            writable: false,
            enumerable: false,
            configurable: true,
            getter: None,
            setter: None,
        },
    );
    properties.insert(
        PropertyKey::String("name".to_string()),
        PropertyDescriptor {
            value: Value::String(std::rc::Rc::new(crate::value::JsString::from(
                name.to_string(),
            ))),
            writable: false,
            enumerable: false,
            configurable: true,
            getter: None,
            setter: None,
        },
    );
}

pub fn install_sloppy_arguments_callee_shadow_props(
    properties: &mut indexmap::IndexMap<PropertyKey, PropertyDescriptor>,
) {
    for name in ["caller", "arguments"] {
        let key = PropertyKey::String(name.to_string());
        properties.entry(key).or_insert(PropertyDescriptor {
            value: Value::Undefined,
            writable: true,
            enumerable: false,
            configurable: true,
            getter: None,
            setter: None,
        });
    }
}

pub type NativeFn = std::rc::Rc<
    dyn Fn(&mut crate::interp::Runtime, &[Value]) -> Result<Value, crate::interp::RuntimeError>,
>;

#[derive(Debug)]
pub struct BoundFunctionInternals {
    pub target: ObjectRef,
    pub this: Value,
    pub args: Vec<Value>,
}

#[cfg(test)]
mod vti_layout_tests {
    use super::*;
    use rusty_js_gc::{ObjectId, Trace};

    fn trace_edges(object: &Object) -> Vec<ObjectId> {
        let mut ids = Vec::new();
        object.trace(&mut ids);
        ids
    }

    fn trace_slice_edges(object: &Object) -> Vec<ObjectId> {
        let mut ids = Vec::new();
        let slice = object.trace_slice(0, usize::MAX, &mut ids);
        assert!(slice.complete);
        ids
    }

    fn assert_full_and_sliced_trace_match(object: &Object) {
        assert_eq!(trace_edges(object), trace_slice_edges(object));
    }

    #[test]
    fn gc_trace_slice_keeps_viewed_buffer_edge() {
        let mut obj = Object::new_ordinary();
        obj.viewed_buffer = Some(ObjectId(41));

        assert_eq!(trace_edges(&obj), vec![ObjectId(41)]);
        assert_full_and_sliced_trace_match(&obj);
    }

    #[test]
    fn gc_trace_keeps_sloppy_mapped_arguments_cells() {
        let mut parameter_map = IndexMap::new();
        parameter_map.insert(
            "0".to_string(),
            Rc::new(RefCell::new(Value::Object(ObjectId(71)))),
        );
        parameter_map.insert("1".to_string(), Rc::new(RefCell::new(Value::Number(13.0))));
        parameter_map.insert(
            "2".to_string(),
            Rc::new(RefCell::new(Value::Object(ObjectId(72)))),
        );

        let mut obj = Object::new_ordinary();
        obj.internal_kind = InternalKind::MappedArguments {
            parameter_map: Box::new(parameter_map),
        };

        assert_eq!(trace_edges(&obj), vec![ObjectId(71), ObjectId(72)]);
        assert_full_and_sliced_trace_match(&obj);
    }

    #[test]
    fn gc_trace_defensively_follows_wrapper_value_objects() {
        for internal_kind in [
            InternalKind::NumberWrapper(Value::Object(ObjectId(81))),
            InternalKind::StringWrapper(Value::Object(ObjectId(82))),
            InternalKind::BooleanWrapper(Value::Object(ObjectId(83))),
            InternalKind::BigIntWrapper(Value::Object(ObjectId(84))),
        ] {
            let mut obj = Object::new_ordinary();
            obj.internal_kind = internal_kind;
            assert_full_and_sliced_trace_match(&obj);
            assert_eq!(trace_edges(&obj).len(), 1);
        }
    }

    #[test]
    fn eval_source_text_preserves_lone_surrogate_with_internal_marker() {
        let source = JsString::from_code_units(vec![
            b'/' as u16,
            b'\\' as u16,
            b'u' as u16,
            b'D' as u16,
            b'8' as u16,
            b'3' as u16,
            b'D' as u16,
            0xDC38,
            b'/' as u16,
        ]);
        let text = source.to_lossless_source_text();
        assert!(text.contains('\u{F0438}'));
        assert!(!text.contains('\u{FFFD}'));
    }

    #[test]
    fn code_unit_at_preserves_utf16_indexing_without_full_view() {
        let ascii = JsString::from("ascii");
        assert_eq!(ascii.code_unit_at(0), Some(b'a' as u16));
        assert_eq!(ascii.code_unit_at(4), Some(b'i' as u16));
        assert_eq!(ascii.code_unit_at(5), None);
        assert_eq!(ascii.code_unit_len(), 5);

        let astral = JsString::from("a😀é");
        assert_eq!(astral.code_unit_at(0), Some(b'a' as u16));
        assert_eq!(astral.code_unit_at(1), Some(0xD83D));
        assert_eq!(astral.code_unit_at(2), Some(0xDE00));
        assert_eq!(astral.code_unit_at(3), Some(0x00E9));
        assert_eq!(astral.code_unit_at(4), None);
        assert_eq!(astral.code_unit_len(), 4);

        let lone = JsString::from_code_units(vec![0xD800, b'x' as u16]);
        assert_eq!(lone.code_unit_at(0), Some(0xD800));
        assert_eq!(lone.code_unit_at(1), Some(b'x' as u16));
        assert_eq!(lone.code_unit_at(2), None);
        assert_eq!(lone.code_unit_len(), 2);
    }

    #[test]
    fn code_unit_as_string_uses_indexed_utf16_primitive() {
        let ascii = JsString::from("ascii");
        assert_eq!(ascii.code_unit_as_string(1), Some(JsString::from("s")));
        assert_eq!(ascii.code_unit_as_string(5), None);

        let astral = JsString::from("a😀");
        assert_eq!(
            astral.code_unit_as_string(1),
            Some(JsString::from_code_units(vec![0xD83D]))
        );
        assert_eq!(
            astral.code_unit_as_string(2),
            Some(JsString::from_code_units(vec![0xDE00]))
        );

        let lone = JsString::from_code_units(vec![0xD800]);
        assert_eq!(
            lone.code_unit_as_string(0),
            Some(JsString::from_code_units(vec![0xD800]))
        );
    }

    #[test]
    fn large_wellformed_strings_use_owned_mmap_storage_without_semantic_drift() {
        let source = format!("{}é", "x".repeat(LARGE_WELLFORMED_MMAP_THRESHOLD));
        let s = WellformedString::new(source.clone());
        assert!(matches!(s.storage, WellformedStringStorage::Mmap(_)));
        assert_eq!(s.as_str(), source);
        assert_eq!(s.as_bytes(), source.as_bytes());
        assert_eq!(s.len(), source.len());
        assert_eq!(s.clone_string(), source);
        assert_eq!(s.encode_utf16().count(), source.encode_utf16().count());

        let cloned = s.clone();
        assert!(matches!(cloned.storage, WellformedStringStorage::Mmap(_)));
        assert_eq!(cloned.as_str(), source);
    }

    #[test]
    fn latin1_strings_preserve_binary_code_units() {
        let binary = JsString::from_latin1_bytes(vec![0, 0x7f, 0x80, 0xff]);
        assert_eq!(binary.as_str(), "\u{0}\u{7f}\u{80}ÿ");
        assert_eq!(binary.code_unit_at(0), Some(0));
        assert_eq!(binary.code_unit_at(1), Some(0x7f));
        assert_eq!(binary.code_unit_at(2), Some(0x80));
        assert_eq!(binary.code_unit_at(3), Some(0xff));
        assert_eq!(binary.code_unit_at(4), None);
        assert_eq!(binary.latin1_code_unit_at(3), Some(Some(0xff)));
        assert_eq!(binary.latin1_code_unit_at(4), Some(None));
        assert_eq!(binary.code_unit_len(), 4);
        assert_eq!(binary.code_units().as_ref(), &[0, 0x7f, 0x80, 0xff]);
    }

    #[test]
    fn js_string_hashmap_keys_are_representation_invariant_for_wellformed_text() {
        use std::collections::HashMap;
        use std::hash::DefaultHasher;

        fn hash_of(s: &JsString) -> u64 {
            let mut hasher = DefaultHasher::new();
            s.hash(&mut hasher);
            hasher.finish()
        }

        let base = Rc::new(JsString::from("xxabczz"));
        let slice = JsString::slice_wellformed(base, 2, 5).expect("ascii slice");
        let concat = JsString::Concat {
            left: Rc::new(JsString::from("a")),
            right: Rc::new(JsString::from("bc")),
            byte_len: 3,
            flat: OnceCell::new(),
        };
        let forms = vec![
            JsString::from("abc"),
            JsString::from_latin1_bytes(b"abc".to_vec()),
            slice,
            concat,
            JsString::from_code_units(vec![b'a' as u16, b'b' as u16, b'c' as u16]),
        ];

        for candidate in &forms {
            assert_eq!(candidate, &forms[0]);
            assert_eq!(hash_of(candidate), hash_of(&forms[0]));
        }

        let mut map = HashMap::new();
        map.insert(forms[0].clone(), 17usize);
        for candidate in &forms[1..] {
            assert_eq!(map.get(candidate), Some(&17));
        }
    }

    #[test]
    fn js_string_hashmap_keys_preserve_latin1_and_wtf16_boundaries() {
        use std::collections::HashMap;

        let latin1_e = JsString::from_latin1_bytes(vec![0xE9]);
        let wellformed_e = JsString::from("é");
        assert_eq!(latin1_e, wellformed_e);
        assert_eq!(latin1_e.code_unit_at(0), Some(0x00E9));
        assert_eq!(latin1_e.code_unit_len(), 1);

        let lone = JsString::from_code_units(vec![0xD800]);
        let replacement = JsString::from("\u{FFFD}");
        assert_ne!(lone, replacement);
        assert_eq!(lone.code_unit_at(0), Some(0xD800));

        let mut map = HashMap::new();
        map.insert(latin1_e, "latin1");
        assert_eq!(map.get(&wellformed_e), Some(&"latin1"));
        assert_eq!(map.get(&replacement), None);
    }

    #[test]
    fn number_tag_at_offset_zero() {
        let v = Value::Number(42.0);
        let tag = unsafe { *((&v as *const Value) as *const u8) };
        assert_eq!(tag, VALUE_TAG_NUMBER);
    }

    #[test]
    fn number_payload_at_declared_offset() {
        let v = Value::Number(1.5_f64);
        let payload = unsafe {
            let base = &v as *const Value as *const u8;
            let pf = base.add(VALUE_NUMBER_PAYLOAD_OFFSET) as *const f64;
            *pf
        };
        assert_eq!(payload, 1.5);
    }

    #[test]
    fn all_variants_have_distinct_tags() {
        let cases: &[(Value, u8)] = &[
            (Value::Undefined, VALUE_TAG_UNDEFINED),
            (Value::Null, VALUE_TAG_NULL),
            (Value::Boolean(true), VALUE_TAG_BOOLEAN),
            (Value::Number(0.0), VALUE_TAG_NUMBER),
        ];
        for (v, expected) in cases {
            let tag = unsafe { *((v as *const Value) as *const u8) };
            assert_eq!(tag, *expected, "variant tag mismatch (rustc layout drift)");
        }
    }

    #[test]
    fn assert_value_layout_runs() {
        assert_value_layout();
    }

    #[test]
    fn packed_safe_i64_fact_tracks_number_writes() {
        let mut obj = Object::new_array();
        obj.array_dense = true;
        obj.array_packed = true;
        obj.array_packed_all_safe_i64 = true;

        obj.dense_doubles.push(1.0);
        obj.note_packed_number_write(1.0);
        obj.dense_doubles.push(-42.0);
        obj.note_packed_number_write(-42.0);
        assert!(obj.array_packed_all_safe_i64);

        obj.dense_doubles[1] = 1.5;
        obj.note_packed_number_write(1.5);
        assert!(!obj.array_packed_all_safe_i64);
    }

    #[test]
    fn packed_safe_i64_fact_recomputes_on_repack_and_clears_on_depack() {
        let mut obj = Object::new_array();
        obj.array_dense = true;
        obj.dense_elements.push(Value::Number(7.0));
        obj.dense_elements.push(Value::Number(9.0));

        obj.try_repack();
        assert!(obj.array_packed);
        assert!(obj.array_packed_all_safe_i64);

        obj.array_depack();
        assert!(!obj.array_packed);
        assert!(!obj.array_packed_all_safe_i64);
    }

    #[test]
    fn packed_safe_i64_fact_rejects_fractional_and_unsafe_numbers() {
        assert!(Object::is_safe_i64_number(9_007_199_254_740_992.0));
        assert!(!Object::is_safe_i64_number(9_007_199_254_740_994.0));
        assert!(!Object::is_safe_i64_number(0.25));
        assert!(!Object::is_safe_i64_number(f64::NAN));
        assert!(!Object::is_safe_i64_number(f64::INFINITY));
    }

    #[test]
    fn packed_i64_sidecar_rebuilds_for_safe_packed_numbers() {
        let mut obj = Object::new_array();
        obj.array_dense = true;
        obj.array_packed = true;
        obj.array_packed_all_safe_i64 = true;
        obj.dense_doubles
            .extend([1.0, -42.0, 9_007_199_254_740_992.0]);

        obj.rebuild_packed_i64_sidecar();

        assert!(obj.dense_i64_sidecar_valid);
        assert_eq!(
            obj.dense_i64_sidecar.as_deref().map(Vec::as_slice),
            Some([1, -42, 9_007_199_254_740_992].as_slice())
        );
    }

    #[test]
    fn packed_i64_sidecar_rebuild_clears_on_fractional_payload() {
        let mut obj = Object::new_array();
        obj.array_dense = true;
        obj.array_packed = true;
        obj.array_packed_all_safe_i64 = true;
        obj.dense_doubles.extend([1.0, 1.5]);

        obj.rebuild_packed_i64_sidecar();

        assert!(!obj.array_packed_all_safe_i64);
        assert!(!obj.dense_i64_sidecar_valid);
        assert!(obj.dense_i64_sidecar.as_deref().is_none_or(Vec::is_empty));
    }

    #[test]
    fn packed_i64_sidecar_clears_on_depack() {
        let mut obj = Object::new_array();
        obj.array_dense = true;
        obj.array_packed = true;
        obj.array_packed_all_safe_i64 = true;
        obj.dense_doubles.extend([3.0, 4.0]);
        obj.rebuild_packed_i64_sidecar();
        assert!(obj.dense_i64_sidecar_valid);

        obj.array_depack();

        assert!(!obj.array_packed);
        assert!(!obj.array_packed_all_safe_i64);
        assert!(!obj.dense_i64_sidecar_valid);
        assert!(obj.dense_i64_sidecar.as_deref().is_none_or(Vec::is_empty));
    }

    #[test]
    fn packed_i64_sidecar_tracks_truncate_and_pop() {
        let mut obj = Object::new_array();
        obj.array_dense = true;
        obj.array_packed = true;
        obj.array_packed_all_safe_i64 = true;
        obj.dense_doubles.extend([3.0, 4.0, 5.0]);
        obj.rebuild_packed_i64_sidecar();

        obj.truncate_packed_doubles(2);
        assert!(obj.dense_i64_sidecar_valid);
        assert_eq!(
            obj.dense_i64_sidecar.as_deref().map(Vec::as_slice),
            Some([3, 4].as_slice())
        );

        assert_eq!(obj.pop_packed_double(), Some(4.0));
        assert!(obj.dense_i64_sidecar_valid);
        assert_eq!(
            obj.dense_i64_sidecar.as_deref().map(Vec::as_slice),
            Some([3].as_slice())
        );
    }
}
