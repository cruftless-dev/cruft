
#![allow(non_camel_case_types, non_snake_case, dead_code)]

use crate::value::{InternalKind, NativeFn, Object, PropertyDescriptor, Value};
use crate::Runtime;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::{c_char, c_void, CStr};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

pub type napi_status = i32;
pub const napi_ok: napi_status = 0;
pub const napi_invalid_arg: napi_status = 1;
pub const napi_object_expected: napi_status = 2;
pub const napi_string_expected: napi_status = 3;
pub const napi_name_expected: napi_status = 4;
pub const napi_function_expected: napi_status = 5;
pub const napi_number_expected: napi_status = 6;
pub const napi_boolean_expected: napi_status = 7;
pub const napi_array_expected: napi_status = 8;
pub const napi_generic_failure: napi_status = 9;
pub const napi_pending_exception: napi_status = 10;
pub const napi_cancelled: napi_status = 11;
pub const napi_escape_called_twice: napi_status = 12;
pub const napi_handle_scope_mismatch: napi_status = 13;

pub type napi_valuetype = i32;
pub const napi_undefined: napi_valuetype = 0;
pub const napi_null: napi_valuetype = 1;
pub const napi_boolean: napi_valuetype = 2;
pub const napi_number: napi_valuetype = 3;
pub const napi_string: napi_valuetype = 4;
pub const napi_symbol: napi_valuetype = 5;
pub const napi_object_t: napi_valuetype = 6;
pub const napi_function: napi_valuetype = 7;
pub const napi_external: napi_valuetype = 8;
pub const napi_bigint: napi_valuetype = 9;

pub type napi_env = *mut NapiEnv;
pub type napi_value = *mut c_void;
pub type napi_ref = *mut NapiRefHandle;
pub type napi_handle_scope = *mut c_void;
pub type napi_escapable_handle_scope = *mut c_void;
pub type napi_callback_info = *mut NapiCallbackInfo;
pub type napi_callback =
    unsafe extern "C" fn(env: napi_env, info: napi_callback_info) -> napi_value;
pub type napi_addon_register_func =
    unsafe extern "C" fn(env: napi_env, exports: napi_value) -> napi_value;
pub type napi_finalize = unsafe extern "C" fn(env: napi_env, data: *mut c_void, hint: *mut c_void);
pub type uv_async_cb = unsafe extern "C" fn(handle: *mut c_void);
pub type uv_close_cb = unsafe extern "C" fn(handle: *mut c_void);

pub type napi_typedarray_type = i32;
pub const napi_int8_array: napi_typedarray_type = 0;
pub const napi_uint8_array: napi_typedarray_type = 1;
pub const napi_uint8_clamped_array: napi_typedarray_type = 2;
pub const napi_int16_array: napi_typedarray_type = 3;
pub const napi_uint16_array: napi_typedarray_type = 4;
pub const napi_int32_array: napi_typedarray_type = 5;
pub const napi_uint32_array: napi_typedarray_type = 6;
pub const napi_float32_array: napi_typedarray_type = 7;
pub const napi_float64_array: napi_typedarray_type = 8;
pub const napi_bigint64_array: napi_typedarray_type = 9;
pub const napi_biguint64_array: napi_typedarray_type = 10;

#[repr(C)]
pub struct napi_extended_error_info {
    pub error_message: *const c_char,
    pub engine_reserved: *mut c_void,
    pub engine_error_code: u32,
    pub error_code: napi_status,
}

#[repr(C)]
pub struct napi_module {
    pub nm_version: i32,
    pub nm_flags: u32,
    pub nm_filename: *const c_char,
    pub nm_register_func: Option<napi_addon_register_func>,
    pub nm_modname: *const c_char,
    pub nm_priv: *mut c_void,
    pub reserved: [*mut c_void; 4],
}

static PENDING_NAPI_MODULE_REGISTER: Mutex<Option<usize>> = Mutex::new(None);
static OLD_V8_EXTRA_HANDLE_SLOTS: Mutex<Vec<usize>> = Mutex::new(Vec::new());
static OLD_V8_STATE_PTR: AtomicUsize = AtomicUsize::new(0);
static OLD_V8_ACTIVE_FAKE_OBJECT: AtomicUsize = AtomicUsize::new(0);
static mut UV_LOOP_SENTINEL: u8 = 0;

const OLD_V8_ISOLATE_HANDLE_SCOPE_DATA_OFFSET: usize = 560;
const OLD_V8_ISOLATE_ROOTS_OFFSET: usize = 688;
const OLD_V8_ROOT_SLOT_COUNT: usize = 6;
const OLD_V8_HANDLE_ARENA_LEN: usize = 4096;
const OLD_V8_ROOT_UNDEFINED: usize = 0x7ff8_0000_0000_0001;
const OLD_V8_ROOT_NULL: usize = 0x7ff8_0000_0000_0002;
const OLD_V8_ROOT_TRUE: usize = 0x7ff8_0000_0000_0003;
const OLD_V8_ROOT_FALSE: usize = 0x7ff8_0000_0000_0004;
const OLD_V8_ROOT_EMPTY_STRING: usize = 0x7ff8_0000_0000_0005;

#[repr(C)]
struct OldV8HandleScopeData {
    next: *mut usize,
    limit: *mut usize,
    level: i32,
    sealed_level: i32,
}

#[repr(C)]
struct OldV8IsolateFacade {
    prefix: [u8; OLD_V8_ISOLATE_HANDLE_SCOPE_DATA_OFFSET],
    handle_scope: OldV8HandleScopeData,
    roots_padding: [u8; OLD_V8_ISOLATE_ROOTS_OFFSET
        - OLD_V8_ISOLATE_HANDLE_SCOPE_DATA_OFFSET
        - std::mem::size_of::<OldV8HandleScopeData>()],
    roots: [usize; OLD_V8_ROOT_SLOT_COUNT],
    arena: [usize; OLD_V8_HANDLE_ARENA_LEN],
}

static mut OLD_V8_ISOLATE_FACADE: OldV8IsolateFacade = OldV8IsolateFacade {
    prefix: [0; OLD_V8_ISOLATE_HANDLE_SCOPE_DATA_OFFSET],
    handle_scope: OldV8HandleScopeData {
        next: std::ptr::null_mut(),
        limit: std::ptr::null_mut(),
        level: 0,
        sealed_level: 0,
    },
    roots_padding: [0; OLD_V8_ISOLATE_ROOTS_OFFSET
        - OLD_V8_ISOLATE_HANDLE_SCOPE_DATA_OFFSET
        - std::mem::size_of::<OldV8HandleScopeData>()],
    roots: [
        OLD_V8_ROOT_UNDEFINED,
        0,
        OLD_V8_ROOT_NULL,
        OLD_V8_ROOT_TRUE,
        OLD_V8_ROOT_FALSE,
        OLD_V8_ROOT_EMPTY_STRING,
    ],
    arena: [0; OLD_V8_HANDLE_ARENA_LEN],
};

type OldV8Callback = unsafe extern "C" fn(*const c_void);
type OldV8AccessorGetter = unsafe extern "C" fn(*mut c_void, *const c_void);

#[derive(Clone)]
struct OldV8AccessorDecl {
    name: String,
    getter: Option<OldV8AccessorGetter>,
    getter_is_nan: bool,
    data: Option<Value>,
}

#[derive(Clone)]
struct OldV8ObjectTemplate {
    internal_field_count: i32,
    data_properties: Vec<(String, *mut OldV8Cell)>,
    accessors: Vec<OldV8AccessorDecl>,
}

impl OldV8ObjectTemplate {
    fn new() -> Self {
        Self {
            internal_field_count: 0,
            data_properties: Vec::new(),
            accessors: Vec::new(),
        }
    }
}

#[derive(Clone)]
struct OldV8FunctionTemplate {
    class_name: Option<String>,
    callback: Option<OldV8Callback>,
    callback_is_nan: bool,
    data: Option<Value>,
    instance_template: *mut OldV8Cell,
    prototype_template: *mut OldV8Cell,
}

enum OldV8Cell {
    Value(Value),
    FunctionTemplate(OldV8FunctionTemplate),
    ObjectTemplate(OldV8ObjectTemplate),
    Context,
    External(*mut c_void),
}

struct OldV8HandleSlot {
    value: *mut OldV8Cell,
}

struct OldV8State {
    rt: *mut Runtime,
    cells: Vec<*mut OldV8Cell>,
    handle_slots: Vec<*mut OldV8HandleSlot>,
    fake_objects: Vec<(usize, *mut OldV8Cell)>,
    current_context: *mut OldV8Cell,
    pending_exception: Option<Value>,
}

impl OldV8State {
    fn new(rt: &mut Runtime) -> Self {
        unsafe {
            old_v8_reset_isolate_facade();
        }
        let mut state = Self {
            rt,
            cells: Vec::new(),
            handle_slots: Vec::new(),
            fake_objects: Vec::new(),
            current_context: std::ptr::null_mut(),
            pending_exception: None,
        };
        state.current_context = state.alloc(OldV8Cell::Context);
        state
    }

    fn alloc(&mut self, cell: OldV8Cell) -> *mut OldV8Cell {
        let ptr = Box::into_raw(Box::new(cell));
        self.cells.push(ptr);
        ptr
    }

    fn alloc_handle_slot(&mut self, value: *mut OldV8Cell) -> *mut OldV8HandleSlot {
        let ptr = Box::into_raw(Box::new(OldV8HandleSlot { value }));
        self.handle_slots.push(ptr);
        ptr
    }

    fn local_handle(&mut self, cell: OldV8Cell) -> *mut c_void {
        let cell = self.alloc(cell);
        old_v8_create_local_handle(cell)
    }

    fn local_handle_for(&mut self, cell: *mut OldV8Cell) -> *mut c_void {
        old_v8_create_local_handle(cell)
    }

    unsafe fn rt_mut(&mut self) -> &mut Runtime {
        &mut *self.rt
    }
}

unsafe fn old_v8_reset_isolate_facade() {
    let facade = &raw mut OLD_V8_ISOLATE_FACADE;
    let arena_start = (&raw mut (*facade).arena).cast::<usize>();
    let arena_end = arena_start.add(OLD_V8_HANDLE_ARENA_LEN);
    (*facade).handle_scope.next = arena_start;
    (*facade).handle_scope.limit = arena_end;
    (*facade).handle_scope.level = 0;
    (*facade).handle_scope.sealed_level = 0;
    for slot in (&mut (*facade).arena).iter_mut() {
        *slot = 0;
    }
    (*facade).roots = [
        OLD_V8_ROOT_UNDEFINED,
        0,
        OLD_V8_ROOT_NULL,
        OLD_V8_ROOT_TRUE,
        OLD_V8_ROOT_FALSE,
        OLD_V8_ROOT_EMPTY_STRING,
    ];
}

fn old_v8_isolate_facade_ptr() -> *mut c_void {
    (&raw mut OLD_V8_ISOLATE_FACADE).cast::<c_void>()
}

fn old_v8_arena_slot_value(slot: *mut c_void) -> Option<*mut OldV8Cell> {
    unsafe {
        let facade = &raw const OLD_V8_ISOLATE_FACADE;
        let start = (&raw const (*facade).arena).cast::<usize>() as usize;
        let end = start + OLD_V8_HANDLE_ARENA_LEN * std::mem::size_of::<usize>();
        let addr = slot as usize;
        if addr < start || addr >= end || (addr - start) % std::mem::size_of::<usize>() != 0 {
            return None;
        }
        let value = *(slot.cast::<usize>());
        if value == 0 {
            None
        } else {
            Some(value as *mut OldV8Cell)
        }
    }
}

fn old_v8_root_value(raw: usize) -> Option<Value> {
    match raw {
        OLD_V8_ROOT_UNDEFINED => Some(Value::Undefined),
        OLD_V8_ROOT_NULL => Some(Value::Null),
        OLD_V8_ROOT_TRUE => Some(Value::Boolean(true)),
        OLD_V8_ROOT_FALSE => Some(Value::Boolean(false)),
        OLD_V8_ROOT_EMPTY_STRING => Some(Value::String(Rc::new(crate::value::JsString::from("")))),
        _ => None,
    }
}

fn old_v8_root_slot_value(slot: *mut c_void) -> Option<Value> {
    unsafe {
        let facade = &raw const OLD_V8_ISOLATE_FACADE;
        let start = (&raw const (*facade).roots).cast::<usize>() as usize;
        let end = start + OLD_V8_ROOT_SLOT_COUNT * std::mem::size_of::<usize>();
        let addr = slot as usize;
        if addr < start || addr >= end || (addr - start) % std::mem::size_of::<usize>() != 0 {
            return None;
        }
        old_v8_root_value(*(slot.cast::<usize>()))
    }
}

fn old_v8_extra_slot_value(slot: *mut c_void) -> Option<*mut OldV8Cell> {
    let slot_addr = slot as usize;
    let slots = OLD_V8_EXTRA_HANDLE_SLOTS.lock().ok()?;
    if !slots.iter().any(|&known| known == slot_addr) {
        return None;
    }
    let value = unsafe { *(slot.cast::<usize>()) };
    if value == 0 {
        None
    } else {
        Some(value as *mut OldV8Cell)
    }
}

fn old_v8_create_extra_handle_slot() -> *mut usize {
    let ptr = Box::into_raw(Box::new(0usize));
    if let Ok(mut slots) = OLD_V8_EXTRA_HANDLE_SLOTS.lock() {
        slots.push(ptr as usize);
    }
    ptr
}

fn old_v8_create_local_handle(cell: *mut OldV8Cell) -> *mut c_void {
    unsafe {
        let facade = &raw mut OLD_V8_ISOLATE_FACADE;
        let arena_start = (&raw mut (*facade).arena).cast::<usize>();
        let arena_end = arena_start.add(OLD_V8_HANDLE_ARENA_LEN);
        let mut slot = (*facade).handle_scope.next;
        if slot.is_null() || slot < arena_start || slot >= arena_end {
            slot = arena_start;
        }
        *slot = cell as usize;
        let next = slot.add(1);
        (*facade).handle_scope.next = if next < arena_end { next } else { arena_start };
        (*facade).handle_scope.limit = arena_end;
        slot.cast::<c_void>()
    }
}

impl Drop for OldV8State {
    fn drop(&mut self) {
        for ptr in self.cells.drain(..) {
            if !ptr.is_null() {
                unsafe {
                    let _ = Box::from_raw(ptr);
                }
            }
        }
        for ptr in self.handle_slots.drain(..) {
            if !ptr.is_null() {
                unsafe {
                    let _ = Box::from_raw(ptr);
                }
            }
        }
    }
}

thread_local! {
    static OLD_V8_STATE: RefCell<Option<OldV8State>> = const { RefCell::new(None) };
}

#[cfg(unix)]
unsafe extern "C" {
    fn pthread_mutex_init(mutex: *mut c_void, attr: *const c_void) -> i32;
    fn pthread_mutex_destroy(mutex: *mut c_void) -> i32;
    fn pthread_mutex_lock(mutex: *mut c_void) -> i32;
    fn pthread_mutex_unlock(mutex: *mut c_void) -> i32;
}

#[cfg(windows)]
unsafe extern "system" {
    fn InitializeCriticalSection(lp: *mut c_void);
    fn DeleteCriticalSection(lp: *mut c_void);
    fn EnterCriticalSection(lp: *mut c_void);
    fn LeaveCriticalSection(lp: *mut c_void);
}

pub struct NapiEnv {
    rt: *mut Runtime,
    owner_thread_id: std::thread::ThreadId,
    handles: Vec<Option<Value>>,
    scopes: Vec<usize>,
    refs: Vec<Option<Value>>,
    wrapped: BTreeMap<u32, NapiWrappedNative>,
    pending_exception: Option<Value>,
    instance_data: *mut c_void,
    last_error_msg: std::ffi::CString,
    last_error_code: napi_status,
    last_error_info: napi_extended_error_info,
}

pub struct NapiRefHandle {
    pub slot: usize,
    pub env: *mut NapiEnv,
    pub count: u32,
}

struct NapiWrappedNative {
    native: usize,
    finalizer: Option<napi_finalize>,
    hint: usize,
}

pub struct NapiCallbackInfo {
    pub this: Value,
    pub args: Vec<Value>,
    pub data: *mut c_void,
    pub new_target: Option<Value>,
}

impl NapiEnv {
    pub fn new(rt: &mut Runtime) -> Box<Self> {
        let mut last_error_info = napi_extended_error_info {
            error_message: std::ptr::null(),
            engine_reserved: std::ptr::null_mut(),
            engine_error_code: 0,
            error_code: napi_ok,
        };
        let last_error_msg = std::ffi::CString::new("").unwrap();
        last_error_info.error_message = last_error_msg.as_ptr();
        Box::new(NapiEnv {
            rt: rt as *mut Runtime,
            owner_thread_id: rt.owner_thread_id(),
            handles: Vec::with_capacity(64),
            scopes: Vec::with_capacity(8),
            refs: Vec::new(),
            wrapped: BTreeMap::new(),
            pending_exception: None,
            instance_data: std::ptr::null_mut(),
            last_error_msg,
            last_error_code: napi_ok,
            last_error_info,
        })
    }

    pub fn push_handle(&mut self, v: Value) -> napi_value {
        self.handles.push(Some(v));

        self.handles.len() as napi_value
    }

    pub fn is_owner_thread(&self) -> bool {
        std::thread::current().id() == self.owner_thread_id
    }

    pub fn set_last_error(&mut self, code: napi_status, message: &str) -> napi_status {
        let sanitized = message.replace('\0', "\\0");
        self.last_error_msg =
            std::ffi::CString::new(sanitized).expect("sanitized N-API error message");
        self.last_error_code = code;
        self.last_error_info.error_message = self.last_error_msg.as_ptr();
        self.last_error_info.error_code = code;
        code
    }

    pub fn assert_owner_thread_status(&mut self, mouth: &str) -> napi_status {
        if self.is_owner_thread() {
            napi_ok
        } else {
            self.set_last_error(
                napi_generic_failure,
                &format!("{mouth}: runtime capability used from non-owner thread"),
            )
        }
    }

    pub fn get_handle(&self, h: napi_value) -> Option<&Value> {
        let idx = (h as usize).checked_sub(1)?;
        self.handles.get(idx)?.as_ref()
    }

    pub fn roots(&self) -> Vec<rusty_js_gc::ObjectId> {
        let mut out = Vec::new();
        for h in self.handles.iter().flatten() {
            if let Value::Object(id) = h {
                out.push(*id);
            }
        }
        for r in self.refs.iter().flatten() {
            if let Value::Object(id) = r {
                out.push(*id);
            }
        }
        if let Some(Value::Object(id)) = &self.pending_exception {
            out.push(*id);
        }
        out
    }

    pub(crate) fn dead_wrapped_object_ids(
        &self,
        mut is_dead: impl FnMut(rusty_js_gc::ObjectId) -> bool,
    ) -> Vec<u32> {
        self.wrapped
            .keys()
            .copied()
            .filter(|id| is_dead(rusty_js_gc::ObjectId(*id)))
            .collect()
    }

    pub(crate) fn finalize_dead_wrapped_object_ids(&mut self, ids: &[u32]) {
        let env = self as *mut NapiEnv;
        for id in ids {
            if let Some(entry) = self.wrapped.remove(id) {
                if let Some(finalizer) = entry.finalizer {
                    unsafe {
                        finalizer(env, entry.native as *mut c_void, entry.hint as *mut c_void);
                    }
                }
            }
        }
    }
}

impl Drop for NapiEnv {
    fn drop(&mut self) {
        let env = self as *mut NapiEnv;
        let wrapped = std::mem::take(&mut self.wrapped);
        for entry in wrapped.into_values() {
            if let Some(finalizer) = entry.finalizer {
                unsafe {
                    finalizer(env, entry.native as *mut c_void, entry.hint as *mut c_void);
                }
            }
        }
    }
}

macro_rules! env_mut {
    ($env:expr) => {{
        if $env.is_null() {
            return napi_invalid_arg;
        }
        &mut *$env
    }};
}

macro_rules! rt_mut {
    ($env:expr) => {{
        let env = env_mut!($env);
        &mut *env.rt
    }};
}

macro_rules! owner_env_mut {
    ($env:expr, $mouth:expr) => {{
        let env = env_mut!($env);
        let status = env.assert_owner_thread_status($mouth);
        if status != napi_ok {
            return status;
        }
        env
    }};
}

macro_rules! check_arg {
    ($p:expr) => {{
        if $p.is_null() {
            return napi_invalid_arg;
        }
    }};
}

fn old_v8_with_state<R>(default: R, f: impl FnOnce(&mut OldV8State) -> R) -> R {
    let ptr = OLD_V8_STATE_PTR.load(Ordering::SeqCst) as *mut OldV8State;
    if !ptr.is_null() {
        return unsafe { f(&mut *ptr) };
    }
    OLD_V8_STATE.with(|slot| {
        let mut borrowed = slot.borrow_mut();
        let Some(state) = borrowed.as_mut() else {
            return default;
        };
        f(state)
    })
}

unsafe fn old_v8_cell_string(cell: *mut OldV8Cell) -> Option<String> {
    let cell = old_v8_resolve_cell(cell.cast::<c_void>())?;
    match cell.as_ref()? {
        OldV8Cell::Value(Value::String(s)) => Some(s.as_str().to_string()),
        OldV8Cell::Value(v) => Some(crate::abstract_ops::to_string(v).to_string()),
        _ => None,
    }
}

unsafe fn old_v8_cell_value(cell: *mut OldV8Cell) -> Option<Value> {
    let cell = old_v8_resolve_cell(cell.cast::<c_void>())?;
    match cell.as_ref()? {
        OldV8Cell::Value(v) => Some(v.clone()),
        OldV8Cell::External(ptr) => Some(Value::Number(*ptr as usize as f64)),
        _ => None,
    }
}

unsafe fn old_v8_cell_object_id(cell: *mut OldV8Cell) -> Option<rusty_js_gc::ObjectId> {
    let cell = old_v8_resolve_cell(cell.cast::<c_void>())?;
    match cell.as_ref()? {
        OldV8Cell::Value(Value::Object(id)) => Some(*id),
        _ => None,
    }
}

unsafe fn old_v8_resolve_cell(handle: *mut c_void) -> Option<*mut OldV8Cell> {
    let ptr = OLD_V8_STATE_PTR.load(Ordering::SeqCst) as *mut OldV8State;
    if !ptr.is_null() {
        return old_v8_resolve_cell_in_state(&*ptr, handle);
    }
    OLD_V8_STATE.with(|slot| {
        let borrowed = slot.try_borrow().ok()?;
        let state = borrowed.as_ref()?;
        old_v8_resolve_cell_in_state(state, handle)
    })
}

fn old_v8_resolve_cell_in_state(state: &OldV8State, handle: *mut c_void) -> Option<*mut OldV8Cell> {
    if handle.is_null() {
        return None;
    }
    if let Some((_, cell)) = state
        .fake_objects
        .iter()
        .find(|(tagged, _)| *tagged == handle as usize)
    {
        return Some(*cell);
    }
    let slot_value = unsafe { std::ptr::read_unaligned(handle.cast::<usize>()) };
    if let Some((_, cell)) = state
        .fake_objects
        .iter()
        .find(|(tagged, _)| *tagged == slot_value)
    {
        return Some(*cell);
    }
    let ptr = handle.cast::<OldV8Cell>();
    if state.cells.iter().any(|&known| known == ptr) {
        return Some(ptr);
    }
    let slot_ptr = handle.cast::<OldV8HandleSlot>();
    if !state.handle_slots.iter().any(|&known| known == slot_ptr) {
        let cell = old_v8_arena_slot_value(handle).or_else(|| old_v8_extra_slot_value(handle));
        let cell = match cell {
            Some(cell) => cell,
            None => {
                if (handle as usize) % std::mem::align_of::<usize>() != 0 {
                    return None;
                }
                let indirect = unsafe { *(handle.cast::<usize>()) } as *mut c_void;
                if indirect.is_null() || indirect == handle {
                    return None;
                }
                old_v8_arena_slot_value(indirect).or_else(|| old_v8_extra_slot_value(indirect))?
            }
        };
        return if state.cells.iter().any(|&known| known == cell) {
            Some(cell)
        } else {
            None
        };
    }
    let slot = unsafe { slot_ptr.as_ref()? };
    if slot.value.is_null() {
        None
    } else {
        Some(slot.value)
    }
}

fn old_v8_truthy_maybe_bool() -> u16 {
    0x0101
}

fn old_v8_maybe_i32(value: i32) -> u64 {
    1u64 | ((value as u32 as u64) << 32)
}

fn old_v8_maybe_u32(value: u32) -> u64 {
    1u64 | ((value as u64) << 32)
}

fn old_v8_smi(value: i32) -> usize {
    #[cfg(target_pointer_width = "64")]
    {
        (value as usize) << 32
    }
    #[cfg(not(target_pointer_width = "64"))]
    {
        (value as usize) << 1
    }
}

fn old_v8_value_from_return_slot(state: &OldV8State, raw: usize) -> Value {
    if let Some(value) = old_v8_root_value(raw) {
        return value;
    }
    let cell_ptr = raw as *mut OldV8Cell;
    if state.cells.iter().any(|&known| known == cell_ptr) {
        return unsafe { old_v8_cell_value(cell_ptr) }.unwrap_or(Value::Undefined);
    }
    let slot_ptr = raw as *mut OldV8HandleSlot;
    if state.handle_slots.iter().any(|&known| known == slot_ptr) {
        if let Some(slot) = unsafe { slot_ptr.as_ref() } {
            return unsafe { old_v8_cell_value(slot.value) }.unwrap_or(Value::Undefined);
        }
    }
    if let Some(cell) = old_v8_arena_slot_value(raw as *mut c_void)
        .or_else(|| old_v8_extra_slot_value(raw as *mut c_void))
    {
        return unsafe { old_v8_cell_value(cell) }.unwrap_or(Value::Undefined);
    }
    if let Some(value) = old_v8_root_slot_value(raw as *mut c_void) {
        return value;
    }
    #[cfg(target_pointer_width = "64")]
    {
        if raw & 0xffff_ffff == 0 {
            return Value::Number(((raw >> 32) as i32) as f64);
        }
    }
    #[cfg(not(target_pointer_width = "64"))]
    {
        if raw & 1 == 0 {
            return Value::Number((raw as i32 >> 1) as f64);
        }
    }
    Value::Undefined
}

fn old_v8_smi_value(raw: usize) -> Option<i32> {
    #[cfg(target_pointer_width = "64")]
    {
        if raw & 0xffff_ffff == 0 {
            return Some((raw >> 32) as i32);
        }
    }
    #[cfg(not(target_pointer_width = "64"))]
    {
        if raw & 1 == 0 {
            return Some(raw as i32 >> 1);
        }
    }
    None
}

fn old_v8_value_from_any_handle(handle: *mut c_void) -> Option<Value> {
    let raw = handle as usize;
    if let Some(value) = old_v8_root_value(raw) {
        return Some(value);
    }
    if let Some(n) = old_v8_smi_value(raw) {
        return Some(Value::Number(n as f64));
    }
    old_v8_with_state(None, |state| {
        if let Some(cell) = old_v8_resolve_cell_in_state(state, handle) {
            return unsafe { old_v8_cell_value(cell) };
        }
        old_v8_root_slot_value(handle)
    })
}

fn old_v8_fake_js_api_object(wrapped_ptr: usize) -> (usize, usize, usize) {
    const K_HEAP_OBJECT_TAG: usize = 1;
    const K_MAP_INSTANCE_TYPE_OFFSET: usize = 12;
    const K_JS_OBJECT_TYPE: u16 = 0x421;
    const K_JS_API_OBJECT_WITH_EMBEDDER_SLOTS_HEADER_SIZE: usize = 32;

    let map = Box::leak(Box::new([0usize; 4]));
    let object = Box::leak(Box::new([0usize; 6]));
    let map_tagged = map.as_mut_ptr() as usize + K_HEAP_OBJECT_TAG;
    unsafe {
        std::ptr::write_unaligned(object.as_mut_ptr(), map_tagged);
        std::ptr::write_unaligned(
            map.as_mut_ptr()
                .cast::<u8>()
                .add(K_MAP_INSTANCE_TYPE_OFFSET)
                .cast::<u16>(),
            K_JS_OBJECT_TYPE,
        );
        std::ptr::write_unaligned(
            object
                .as_mut_ptr()
                .cast::<u8>()
                .add(K_JS_API_OBJECT_WITH_EMBEDDER_SLOTS_HEADER_SIZE)
                .cast::<usize>(),
            wrapped_ptr,
        );
    }
    let object_tagged = object.as_mut_ptr() as usize + K_HEAP_OBJECT_TAG;
    (
        map.as_mut_ptr() as usize,
        object.as_mut_ptr() as usize,
        object_tagged,
    )
}

fn old_v8_active_fake_object_from_handle(handle: *mut c_void) -> Option<usize> {
    let active_fake = OLD_V8_ACTIVE_FAKE_OBJECT.load(Ordering::SeqCst);
    if active_fake == 0 || handle.is_null() {
        return None;
    }
    let handle_addr = handle as usize;
    if handle_addr == active_fake || handle_addr + 1 == active_fake {
        return Some(active_fake);
    }
    let slot_value = unsafe { std::ptr::read_unaligned(handle.cast::<usize>()) };
    if slot_value == active_fake {
        Some(active_fake)
    } else {
        None
    }
}

fn old_v8_fake_object_aligned_field(fake_tagged: usize, index: i32) -> *mut c_void {
    const K_HEAP_OBJECT_TAG: usize = 1;
    const K_JS_API_OBJECT_WITH_EMBEDDER_SLOTS_HEADER_SIZE: usize = 32;
    if index < 0 {
        return std::ptr::null_mut();
    }
    let object_base = (fake_tagged - K_HEAP_OBJECT_TAG) as *const u8;
    let offset = K_JS_API_OBJECT_WITH_EMBEDDER_SLOTS_HEADER_SIZE
        + index as usize * std::mem::size_of::<usize>();
    unsafe { std::ptr::read_unaligned(object_base.add(offset).cast::<usize>()) as *mut c_void }
}

fn old_v8_internal_field_key(index: i32) -> String {
    format!("__old_v8_internal_field_{index}")
}

fn old_v8_aligned_internal_field_key(index: i32) -> String {
    format!("__old_v8_aligned_internal_field_{index}")
}

fn old_v8_make_native_method(
    rt: &mut Runtime,
    name: String,
    callback: Option<OldV8Callback>,
    callback_is_nan: bool,
    data: Option<Value>,
    internal_field_count: i32,
    accessors: Vec<OldV8AccessorDecl>,
) -> Value {
    let native_name = name.clone();
    let native: NativeFn = std::rc::Rc::new(move |rt, args| {
        let Some(callback) = callback else {
            return Ok(Value::Undefined);
        };
        let mut callback_state = OldV8State::new(rt);
        let state_ptr = (&mut callback_state as *mut OldV8State) as usize;
        OLD_V8_STATE_PTR.store(state_ptr, Ordering::SeqCst);
        let result = {
            let state = &mut callback_state;
            let receiver = rt.current_this();
            if internal_field_count > 0 {
                if let Value::Object(id) = receiver {
                    rt.obj_mut(id).set_own_internal(
                        "__old_v8_internal_field_count".into(),
                        Value::Number(internal_field_count as f64),
                    );
                    for accessor in &accessors {
                        let getter = old_v8_make_native_accessor(
                            rt,
                            accessor.name.clone(),
                            accessor.getter,
                            accessor.getter_is_nan,
                            accessor.data.clone(),
                        );
                        let getter_id = rt.alloc_object(getter);
                        rt.obj_mut(id).dict_mut().insert(
                            accessor.name.clone().into(),
                            PropertyDescriptor {
                                value: Value::Undefined,
                                writable: false,
                                enumerable: true,
                                configurable: true,
                                getter: Some(Value::Object(getter_id)),
                                setter: None,
                            },
                        );
                    }
                }
            }
            let receiver_handle = state.local_handle(OldV8Cell::Value(receiver.clone())) as usize;
            let receiver_cell = old_v8_resolve_cell_in_state(state, receiver_handle as *mut c_void);
            let fake_receiver = match receiver {
                Value::Object(id) => match rt.object_get(id, &old_v8_aligned_internal_field_key(0))
                {
                    Value::Number(ptr) if ptr != 0.0 => {
                        Some(old_v8_fake_js_api_object(ptr as usize))
                    }
                    _ => None,
                },
                _ => None,
            };
            let receiver_handle = fake_receiver
                .as_ref()
                .map(|(_, _, tagged)| *tagged)
                .unwrap_or(receiver_handle);
            if let (Some((_, _, tagged)), Some(cell)) = (&fake_receiver, receiver_cell) {
                state.fake_objects.push((*tagged, cell));
                OLD_V8_ACTIVE_FAKE_OBJECT.store(*tagged, Ordering::SeqCst);
            }
            let new_target = rt.current_new_target.clone().unwrap_or(Value::Undefined);
            let new_target_handle = state.local_handle(OldV8Cell::Value(new_target)) as usize;
            let undefined_handle = state.local_handle(OldV8Cell::Value(Value::Undefined)) as usize;
            let context_handle = state.local_handle_for(state.current_context) as usize;
            let target_handle = data
                .clone()
                .map(|value| state.local_handle(OldV8Cell::Value(value)) as usize)
                .unwrap_or(undefined_handle);
            let mut fake_arg_objects = Vec::new();
            let mut arg_handles = Vec::with_capacity(args.len());
            for arg in args {
                let arg_handle = state.local_handle(OldV8Cell::Value(arg.clone())) as usize;
                match arg {
                    Value::Number(n)
                        if n.is_finite()
                            && n.fract() == 0.0
                            && *n >= i32::MIN as f64
                            && *n <= i32::MAX as f64 =>
                    {
                        arg_handles.push(old_v8_smi(*n as i32));
                    }
                    Value::Boolean(true) => arg_handles.push(OLD_V8_ROOT_TRUE),
                    Value::Boolean(false) => arg_handles.push(OLD_V8_ROOT_FALSE),
                    Value::Null => arg_handles.push(OLD_V8_ROOT_NULL),
                    Value::Undefined => arg_handles.push(OLD_V8_ROOT_UNDEFINED),
                    Value::Object(_) => {
                        if let Some(cell) =
                            old_v8_resolve_cell_in_state(state, arg_handle as *mut c_void)
                        {
                            let fake = old_v8_fake_js_api_object(0);
                            state.fake_objects.push((fake.2, cell));
                            arg_handles.push(fake.2);
                            fake_arg_objects.push(fake);
                        } else {
                            arg_handles.push(arg_handle);
                        }
                    }
                    _ => arg_handles.push(arg_handle),
                }
            }

            const K_ARGC_INDEX: usize = 0;
            const K_FRAME_SP_INDEX: usize = 1;
            const K_FRAME_TYPE_INDEX: usize = 2;
            const K_FRAME_FP_INDEX: usize = 3;
            const K_FRAME_PC_INDEX: usize = 4;
            const K_ISOLATE_INDEX: usize = 5;
            const K_RETURN_VALUE_INDEX: usize = 6;
            const K_CONTEXT_INDEX: usize = 7;
            const K_TARGET_INDEX: usize = 8;
            const K_RECEIVER_INDEX: usize = 9;
            const K_FIRST_JS_ARGUMENT_INDEX: usize = 10;
            const K_API_CONSTRUCT_EXIT: i32 = 19;

            let mut frame = vec![0usize; 1 + K_FIRST_JS_ARGUMENT_INDEX + arg_handles.len()];
            let base = 1usize;
            frame[base - 1] = new_target_handle;
            frame[base + K_ARGC_INDEX] = arg_handles.len();
            frame[base + K_FRAME_SP_INDEX] = 0;
            frame[base + K_FRAME_TYPE_INDEX] = if rt.current_new_target.is_some() {
                old_v8_smi(K_API_CONSTRUCT_EXIT)
            } else {
                0
            };
            frame[base + K_FRAME_FP_INDEX] = 0;
            frame[base + K_FRAME_PC_INDEX] = 0;
            frame[base + K_ISOLATE_INDEX] = old_v8_isolate_facade_ptr() as usize;
            frame[base + K_RETURN_VALUE_INDEX] = undefined_handle;
            frame[base + K_CONTEXT_INDEX] = context_handle;
            frame[base + K_TARGET_INDEX] = target_handle;
            frame[base + K_RECEIVER_INDEX] = receiver_handle;
            for (idx, handle) in arg_handles.iter().enumerate() {
                frame[base + K_FIRST_JS_ARGUMENT_INDEX + idx] = *handle;
            }
            state.pending_exception = None;
            unsafe {
                let v8_info = frame.as_ptr().add(base);
                if callback_is_nan {
                    let nan_info = [v8_info as usize, target_handle];
                    callback(nan_info.as_ptr().cast::<c_void>());
                } else {
                    callback(v8_info.cast::<c_void>());
                }
            }
            drop(fake_arg_objects);
            OLD_V8_ACTIVE_FAKE_OBJECT.store(0, Ordering::SeqCst);
            if let Some(value) = state.pending_exception.take() {
                return Err(crate::RuntimeError::Thrown(value));
            }
            let returned = frame[base + K_RETURN_VALUE_INDEX] as *mut c_void;
            Ok(old_v8_value_from_return_slot(state, returned as usize))
        };
        OLD_V8_STATE_PTR.store(0, Ordering::SeqCst);
        result
    });
    let obj =
        crate::intrinsics::make_native_with_length(&name, 0, move |rt, args| native(rt, args));
    Value::Object(rt.alloc_object(obj))
}

fn old_v8_make_native_accessor(
    rt: &mut Runtime,
    name: String,
    getter: Option<OldV8AccessorGetter>,
    getter_is_nan: bool,
    data: Option<Value>,
) -> Object {
    let native_name = name.clone();
    let native: NativeFn = std::rc::Rc::new(move |rt, _args| {
        let Some(getter) = getter else {
            return Ok(Value::Undefined);
        };
        let mut callback_state = OldV8State::new(rt);
        let state_ptr = (&mut callback_state as *mut OldV8State) as usize;
        OLD_V8_STATE_PTR.store(state_ptr, Ordering::SeqCst);
        let result = {
            let state = &mut callback_state;
            let receiver = rt.current_this();
            let receiver_handle = state.local_handle(OldV8Cell::Value(receiver.clone())) as usize;
            let receiver_cell = old_v8_resolve_cell_in_state(state, receiver_handle as *mut c_void);
            let fake_holder = match receiver {
                Value::Object(id) => match rt.object_get(id, &old_v8_aligned_internal_field_key(0))
                {
                    Value::Number(ptr) if ptr != 0.0 => {
                        Some(old_v8_fake_js_api_object(ptr as usize))
                    }
                    _ => None,
                },
                _ => None,
            };
            let holder_handle = fake_holder
                .as_ref()
                .map(|(_, _, tagged)| *tagged)
                .unwrap_or(receiver_handle);
            if let (Some((_, _, tagged)), Some(cell)) = (&fake_holder, receiver_cell) {
                state.fake_objects.push((*tagged, cell));
                OLD_V8_ACTIVE_FAKE_OBJECT.store(*tagged, Ordering::SeqCst);
            }
            let undefined_handle = state.local_handle(OldV8Cell::Value(Value::Undefined)) as usize;
            let property_handle = state.local_handle(OldV8Cell::Value(Value::String(Rc::new(
                crate::value::JsString::from(native_name.clone()),
            )))) as usize;
            let data_handle = data
                .clone()
                .map(|value| state.local_handle(OldV8Cell::Value(value)) as usize)
                .unwrap_or(undefined_handle);

            const K_PROPERTY_KEY_INDEX: usize = 0;
            const K_FRAME_SP_INDEX: usize = 1;
            const K_FRAME_TYPE_INDEX: usize = 2;
            const K_FRAME_FP_INDEX: usize = 3;
            const K_FRAME_PC_INDEX: usize = 4;
            const K_ISOLATE_INDEX: usize = 5;
            const K_RETURN_VALUE_INDEX: usize = 6;
            const K_CALLBACK_INFO_INDEX: usize = 7;
            const K_HOLDER_INDEX: usize = 8;
            const K_API_NAMED_ACCESSOR_EXIT: i32 = 20;

            let mut frame = vec![0usize; 9];
            frame[K_PROPERTY_KEY_INDEX] = property_handle;
            frame[K_FRAME_SP_INDEX] = 0;
            frame[K_FRAME_TYPE_INDEX] = old_v8_smi(K_API_NAMED_ACCESSOR_EXIT);
            frame[K_FRAME_FP_INDEX] = 0;
            frame[K_FRAME_PC_INDEX] = 0;
            frame[K_HOLDER_INDEX] = holder_handle;
            frame[K_ISOLATE_INDEX] = old_v8_isolate_facade_ptr() as usize;
            frame[K_RETURN_VALUE_INDEX] = undefined_handle;
            frame[K_CALLBACK_INFO_INDEX] = 0;

            state.pending_exception = None;
            unsafe {
                let v8_info = frame.as_ptr();
                if getter_is_nan {
                    let nan_info = [v8_info as usize, data_handle];
                    getter(
                        property_handle as *mut c_void,
                        nan_info.as_ptr().cast::<c_void>(),
                    );
                } else {
                    getter(property_handle as *mut c_void, v8_info.cast::<c_void>());
                }
            }
            OLD_V8_ACTIVE_FAKE_OBJECT.store(0, Ordering::SeqCst);
            if let Some(value) = state.pending_exception.take() {
                return Err(crate::RuntimeError::Thrown(value));
            }
            let returned = frame[K_RETURN_VALUE_INDEX] as *mut c_void;
            Ok(old_v8_value_from_return_slot(state, returned as usize))
        };
        OLD_V8_STATE_PTR.store(0, Ordering::SeqCst);
        result
    });
    crate::intrinsics::make_native_with_length(&format!("get {name}"), 0, move |rt, args| {
        native(rt, args)
    })
}

unsafe fn old_v8_make_error_object(
    state: &mut OldV8State,
    name: &str,
    message: *mut c_void,
) -> *mut c_void {
    let msg = old_v8_cell_string(message.cast::<OldV8Cell>()).unwrap_or_default();
    let rt = state.rt_mut();
    let id = rt.alloc_object(Object::new_ordinary());
    rt.object_set(
        id,
        "name".into(),
        Value::String(Rc::new(crate::value::JsString::from(name))),
    );
    rt.object_set(
        id,
        "message".into(),
        Value::String(Rc::new(crate::value::JsString::from(msg))),
    );
    state.local_handle(OldV8Cell::Value(Value::Object(id)))
}

include!("napi_generated.rs");

unsafe fn napi_get_value_string_utf8__impl(
    env: napi_env,
    value: napi_value,
    buf: *mut c_char,
    bufsize: usize,
    result: *mut usize,
) -> napi_status {
    let env = env_mut!(env);
    let s = match env.get_handle(value) {
        Some(Value::String(s)) => s.clone(),
        _ => return napi_string_expected,
    };
    let bytes = s.as_bytes();
    if buf.is_null() {

        if !result.is_null() {
            *result = bytes.len();
        }
        return napi_ok;
    }
    if bufsize == 0 {
        if !result.is_null() {
            *result = 0;
        }
        return napi_ok;
    }
    let n = bytes.len().min(bufsize - 1);
    std::ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, buf, n);
    *buf.add(n) = 0;
    if !result.is_null() {
        *result = n;
    }
    napi_ok
}

struct NapiCallbackStorage {
    cb: napi_callback,
    data: *mut c_void,
    env: *mut NapiEnv,
}

unsafe fn napi_create_function__impl(
    env: napi_env,
    utf8name: *const c_char,
    _length: usize,
    cb: Option<napi_callback>,
    data: *mut c_void,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let cb = match cb {
        Some(f) => f,
        None => return napi_invalid_arg,
    };
    let env_ptr = env;
    let env = owner_env_mut!(env, "napi_create_function");
    let rt = &mut *env.rt;
    let name = if utf8name.is_null() {
        "".into()
    } else {
        CStr::from_ptr(utf8name).to_string_lossy().into_owned()
    };
    let storage = std::rc::Rc::new(NapiCallbackStorage {
        cb,
        data,
        env: env_ptr,
    });
    let fn_storage = storage.clone();
    let native: NativeFn = std::rc::Rc::new(move |rt, args| {

        let env = unsafe { &mut *fn_storage.env };
        let scope_start = env.handles.len();
        let mut handle_args: Vec<*mut c_void> = Vec::with_capacity(args.len());
        for a in args {
            handle_args.push(env.push_handle(a.clone()));
        }
        let this = rt.current_this();
        let info = NapiCallbackInfo {
            this,
            args: args.to_vec(),
            data: fn_storage.data,
            new_target: rt.current_new_target.clone(),
        };
        let info_box = Box::into_raw(Box::new(info));
        let ret_handle = unsafe { (fn_storage.cb)(fn_storage.env, info_box) };
        let _ = unsafe { Box::from_raw(info_box) };

        if let Some(exc) = env.pending_exception.take() {

            env.handles.truncate(scope_start);
            return Err(crate::RuntimeError::Thrown(exc));
        }
        let v = env
            .get_handle(ret_handle)
            .cloned()
            .unwrap_or(Value::Undefined);
        let v = canonicalize_napi_return_buffer_like(rt, v);
        env.handles.truncate(scope_start);
        Ok(v)
    });
    let obj = crate::intrinsics::make_native(&name, move |rt, args| native(rt, args));
    let id = rt.alloc_object(obj);
    let _ = storage;
    *result = env.push_handle(Value::Object(id));
    napi_ok
}

unsafe fn napi_get_cb_info__impl(
    env: napi_env,
    cbinfo: napi_callback_info,
    argc: *mut usize,
    argv: *mut napi_value,
    this_arg: *mut napi_value,
    data: *mut *mut c_void,
) -> napi_status {
    if cbinfo.is_null() {
        return napi_invalid_arg;
    }
    let env = env_mut!(env);
    let info = &*cbinfo;
    if !argc.is_null() {
        let wanted = *argc;
        let actual = info.args.len();
        if !argv.is_null() {
            let copy_n = wanted.min(actual);
            for i in 0..copy_n {
                *argv.add(i) = env.push_handle(info.args[i].clone());
            }

            for i in actual..wanted {
                *argv.add(i) = env.push_handle(Value::Undefined);
            }
        }
        *argc = actual;
    }
    if !this_arg.is_null() {
        *this_arg = env.push_handle(info.this.clone());
    }
    if !data.is_null() {
        *data = info.data;
    }
    napi_ok
}

unsafe fn napi_call_function__impl(
    env: napi_env,
    recv: napi_value,
    func: napi_value,
    argc: usize,
    argv: *const napi_value,
    result: *mut napi_value,
) -> napi_status {
    let env = owner_env_mut!(env, "napi_call_function");
    let recv_v = env.get_handle(recv).cloned().unwrap_or(Value::Undefined);
    let func_v = match env.get_handle(func) {
        Some(v) => v.clone(),
        None => return napi_function_expected,
    };
    let mut args: Vec<Value> = Vec::with_capacity(argc);
    for i in 0..argc {
        let h = *argv.add(i);
        args.push(env.get_handle(h).cloned().unwrap_or(Value::Undefined));
    }
    let rt = &mut *env.rt;
    let trace_call = std::env::var_os("CRUFT_NAPI_TRACE_CALL_FUNCTION").is_some();
    if trace_call {
        let arg_summaries: Vec<String> = args
            .iter()
            .take(8)
            .map(|arg| napi_trace_value_summary(rt, arg))
            .collect();
        eprintln!(
            "[cruft-napi-call-function:enter] func={} recv={} argc={} args={:?}",
            napi_trace_value_summary(rt, &func_v),
            napi_trace_value_summary(rt, &recv_v),
            argc,
            arg_summaries
        );
    }
    match rt.call_function(func_v, recv_v, args) {
        Ok(v) => {
            if trace_call {
                eprintln!("[cruft-napi-call-function:ok] result={v:?}");
            }
            if !result.is_null() {
                *result = env.push_handle(v);
            }
            napi_ok
        }
        Err(e) => {
            if trace_call {
                eprintln!("[cruft-napi-call-function:err] error={e:?}");
            }
            let pending = match e {
                crate::RuntimeError::Thrown(v) => v,
                _ => Value::String(Rc::new(crate::value::JsString::from(format!("{:?}", e)))),
            };
            if trace_call {
                eprintln!(
                    "[cruft-napi-call-function:pending-exception] {}",
                    napi_trace_value_summary(rt, &pending)
                );
            }
            env.pending_exception = Some(pending);
            napi_pending_exception
        }
    }
}

fn napi_trace_value_summary(rt: &Runtime, value: &Value) -> String {
    match value {
        Value::Object(id) => {
            let obj = rt.obj(*id);
            let keys: Vec<String> = obj.string_key_clones().take(32).collect();
            let field = |name: &str| match rt.object_get(*id, name) {
                Value::Undefined => None,
                Value::String(s) => Some(format!("{name}={:?}", s.as_str())),
                other => Some(format!("{name}={other:?}")),
            };
            let mut fields = Vec::new();
            for name in ["name", "message", "code", "stack"] {
                if let Some(v) = field(name) {
                    fields.push(v);
                }
            }
            format!("object={id:?} keys={keys:?} {}", fields.join(" "))
        }
        Value::String(s) => format!("string={:?}", s.as_str()),
        other => format!("{other:?}"),
    }
}

unsafe fn napi_create_reference__impl(
    env: napi_env,
    value: napi_value,
    initial_refcount: u32,
    result: *mut napi_ref,
) -> napi_status {
    check_arg!(result);
    if initial_refcount == 0 {

        return napi_generic_failure;
    }
    let env_ptr = env;

    let env = owner_env_mut!(env, "napi_create_reference");
    let v = match env.get_handle(value) {
        Some(v) => v.clone(),
        None => return napi_invalid_arg,
    };
    let slot = env.refs.len();
    env.refs.push(Some(v));
    let handle = Box::into_raw(Box::new(NapiRefHandle {
        slot,
        env: env_ptr,
        count: initial_refcount,
    }));
    *result = handle;
    napi_ok
}

unsafe fn napi_delete_reference__impl(env: napi_env, r: napi_ref) -> napi_status {
    if r.is_null() {
        return napi_invalid_arg;
    }
    let env = env_mut!(env);
    let handle = Box::from_raw(r);
    if handle.slot < env.refs.len() {
        env.refs[handle.slot] = None;
    }
    napi_ok
}

unsafe fn napi_reference_ref__impl(env: napi_env, r: napi_ref, result: *mut u32) -> napi_status {
    if r.is_null() {
        return napi_invalid_arg;
    }
    let _env = owner_env_mut!(env, "napi_reference_ref");
    let h = &mut *r;
    h.count += 1;
    if !result.is_null() {
        *result = h.count;
    }
    napi_ok
}

unsafe fn napi_reference_unref__impl(env: napi_env, r: napi_ref, result: *mut u32) -> napi_status {
    if r.is_null() {
        return napi_invalid_arg;
    }
    let _env = owner_env_mut!(env, "napi_reference_unref");
    let h = &mut *r;
    if h.count > 0 {
        h.count -= 1;
    }
    if !result.is_null() {
        *result = h.count;
    }
    napi_ok
}

unsafe fn napi_get_reference_value__impl(
    env: napi_env,
    r: napi_ref,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    if r.is_null() {
        return napi_invalid_arg;
    }
    let env = owner_env_mut!(env, "napi_get_reference_value");
    let h = &*r;
    let v = env
        .refs
        .get(h.slot)
        .and_then(|o| o.clone())
        .unwrap_or(Value::Undefined);
    *result = env.push_handle(v);
    napi_ok
}

unsafe fn napi_get_last_error_info__impl(
    env: napi_env,
    result: *mut *const napi_extended_error_info,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    *result = &env.last_error_info as *const _;
    napi_ok
}

unsafe fn napi_open_handle_scope__impl(
    env: napi_env,
    result: *mut napi_handle_scope,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    let saved = env.handles.len();
    env.scopes.push(saved);
    *result = saved as napi_handle_scope;
    napi_ok
}

unsafe fn napi_close_handle_scope__impl(env: napi_env, scope: napi_handle_scope) -> napi_status {
    let env = env_mut!(env);
    let saved = scope as usize;
    let mut boundary = saved;
    if let Some(pos) = env.scopes.iter().rposition(|&s| s == saved) {
        env.scopes.remove(pos);
    } else if let Some(pos) = env.scopes.iter().rposition(|&s| s == saved + 1) {
        boundary = saved + 1;
        env.scopes.remove(pos);
    }
    if env.handles.len() > boundary {
        env.handles.truncate(boundary);
    }
    napi_ok
}

unsafe fn napi_open_escapable_handle_scope__impl(
    env: napi_env,
    result: *mut napi_escapable_handle_scope,
) -> napi_status {
    napi_open_handle_scope(env, result as *mut napi_handle_scope)
}

unsafe fn napi_close_escapable_handle_scope__impl(
    env: napi_env,
    scope: napi_escapable_handle_scope,
) -> napi_status {
    napi_close_handle_scope(env, scope as napi_handle_scope)
}

unsafe fn napi_escape_handle__impl(
    env: napi_env,
    scope: napi_escapable_handle_scope,
    escapee: napi_value,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    let v = env.get_handle(escapee).cloned().unwrap_or(Value::Undefined);
    let saved = scope as usize;

    if saved > env.handles.len() {
        return napi_invalid_arg;
    }
    let escaped_index = saved;
    if escaped_index == env.handles.len() {
        env.handles.push(Some(v));
    } else {
        env.handles[escaped_index] = Some(v);
    }
    if let Some(scope_saved) = env.scopes.iter_mut().rfind(|s| **s == saved) {
        *scope_saved = saved + 1;
    }
    *result = (escaped_index + 1) as napi_value;
    napi_ok
}

#[repr(C)]
pub struct napi_property_descriptor {
    pub utf8name: *const c_char,
    pub name: napi_value,
    pub method: Option<napi_callback>,
    pub getter: Option<napi_callback>,
    pub setter: Option<napi_callback>,
    pub value: napi_value,
    pub attributes: i32,
    pub data: *mut c_void,
}

const NAPI_WRITABLE: i32 = 1 << 0;
const NAPI_ENUMERABLE: i32 = 1 << 1;
const NAPI_CONFIGURABLE: i32 = 1 << 2;
const NAPI_STATIC: i32 = 1 << 10;

unsafe fn napi_descriptor_flags(attributes: i32) -> (bool, bool, bool) {
    (
        attributes & NAPI_WRITABLE != 0,
        attributes & NAPI_ENUMERABLE != 0,
        attributes & NAPI_CONFIGURABLE != 0,
    )
}

unsafe fn napi_make_callback_value(
    env: napi_env,
    name: &str,
    callback: napi_callback,
    data: *mut c_void,
) -> Value {
    let mut handle: napi_value = std::ptr::null_mut();
    let cname = std::ffi::CString::new(name).ok();
    let utf8name = cname
        .as_ref()
        .map(|s| s.as_ptr())
        .unwrap_or(std::ptr::null());
    let _ = napi_create_function(env, utf8name, name.len(), Some(callback), data, &mut handle);
    let env_ref = &mut *env;
    env_ref
        .get_handle(handle)
        .cloned()
        .unwrap_or(Value::Undefined)
}

unsafe fn napi_property_descriptor_value(
    env: napi_env,
    desc: &napi_property_descriptor,
    name: &str,
) -> crate::value::PropertyDescriptor {
    let (writable, enumerable, configurable) = napi_descriptor_flags(desc.attributes);
    if desc.getter.is_some() || desc.setter.is_some() {
        let getter = desc
            .getter
            .map(|getter| napi_make_callback_value(env, &format!("get {name}"), getter, desc.data));
        let setter = desc
            .setter
            .map(|setter| napi_make_callback_value(env, &format!("set {name}"), setter, desc.data));
        crate::value::PropertyDescriptor {
            value: Value::Undefined,
            writable: false,
            enumerable,
            configurable,
            getter,
            setter,
        }
    } else if let Some(method) = desc.method {
        crate::value::PropertyDescriptor {
            value: napi_make_callback_value(env, name, method, desc.data),
            writable,
            enumerable,
            configurable,
            getter: None,
            setter: None,
        }
    } else if !desc.value.is_null() {
        let env_ref = &mut *env;
        crate::value::PropertyDescriptor {
            value: env_ref
                .get_handle(desc.value)
                .cloned()
                .unwrap_or(Value::Undefined),
            writable,
            enumerable,
            configurable,
            getter: None,
            setter: None,
        }
    } else {
        crate::value::PropertyDescriptor {
            value: Value::Undefined,
            writable,
            enumerable,
            configurable,
            getter: None,
            setter: None,
        }
    }
}

unsafe fn napi_define_properties__impl(
    env: napi_env,
    object: napi_value,
    property_count: usize,
    properties: *const napi_property_descriptor,
) -> napi_status {
    let env_ptr = env;
    let env = owner_env_mut!(env, "napi_define_properties");
    let target = match env.get_handle(object) {
        Some(Value::Object(id)) => *id,
        _ => return napi_object_expected,
    };
    let rt = &mut *env.rt;
    for i in 0..property_count {
        let d = &*properties.add(i);
        let name = if !d.utf8name.is_null() {
            CStr::from_ptr(d.utf8name).to_string_lossy().into_owned()
        } else if !d.name.is_null() {
            match env.get_handle(d.name) {
                Some(v) => crate::abstract_ops::to_string(v).as_str().to_string(),
                None => continue,
            }
        } else {
            continue;
        };
        let desc = napi_property_descriptor_value(env_ptr, d, &name);
        rt.obj_mut(target).insert_str(name, desc);
    }
    napi_ok
}

macro_rules! env_mut_or_null {
    ($env:expr) => {{
        if $env.is_null() {
            return std::ptr::null_mut();
        }
        &mut *$env
    }};
}

unsafe fn make_error_obj(
    env: napi_env,
    msg: napi_value,
    code: napi_value,
    name: &str,
) -> napi_value {
    let env_ref = env_mut_or_null!(env);
    let rt = &mut *env_ref.rt;
    let id = rt.alloc_object(Object::new_ordinary());
    if !msg.is_null() {
        if let Some(v) = env_ref.get_handle(msg).cloned() {
            rt.object_set(id, "message".into(), v);
        }
    }
    if !code.is_null() {
        if let Some(v) = env_ref.get_handle(code).cloned() {
            rt.object_set(id, "code".into(), v);
        }
    }
    rt.object_set(
        id,
        "name".into(),
        Value::String(Rc::new(crate::value::JsString::from(name))),
    );
    env_ref.push_handle(Value::Object(id))
}

unsafe fn napi_create_symbol__impl(
    env: napi_env,
    description: napi_value,
    result: *mut napi_value,
) -> napi_status {
    if result.is_null() {
        return napi_invalid_arg;
    }
    let env = env_mut!(env);
    let desc = if description.is_null() {
        String::new()
    } else {
        match env.get_handle(description) {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => String::new(),
        }
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(1_000_000);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    *result = env.push_handle(Value::Symbol(Rc::new(format!("@@sym:{}:{}", n, desc))));
    napi_ok
}

unsafe fn napi_get_value_string_latin1__impl(
    env: napi_env,
    value: napi_value,
    buf: *mut c_char,
    bufsize: usize,
    result: *mut usize,
) -> napi_status {

    napi_get_value_string_utf8(env, value, buf, bufsize, result)
}

unsafe fn napi_get_value_string_utf16__impl(
    env: napi_env,
    value: napi_value,
    buf: *mut u16,
    bufsize: usize,
    result: *mut usize,
) -> napi_status {
    let env = env_mut!(env);
    let s = match env.get_handle(value) {
        Some(Value::String(s)) => s.clone(),
        _ => return napi_string_expected,
    };
    let units = s.code_units();
    if buf.is_null() {
        if !result.is_null() {
            *result = units.len();
        }
        return napi_ok;
    }
    if bufsize == 0 {
        if !result.is_null() {
            *result = 0;
        }
        return napi_ok;
    }
    let n = units.len().min(bufsize - 1);
    std::ptr::copy_nonoverlapping(units.as_ptr(), buf, n);
    *buf.add(n) = 0;
    if !result.is_null() {
        *result = n;
    }
    napi_ok
}

unsafe fn napi_create_bigint_int64__impl(
    env: napi_env,
    value: i64,
    result: *mut napi_value,
) -> napi_status {
    if result.is_null() {
        return napi_invalid_arg;
    }
    let env = env_mut!(env);
    *result = env.push_handle(Value::BigInt(Rc::new(crate::bigint::JsBigInt::from_i64(
        value,
    ))));
    napi_ok
}

unsafe fn napi_create_bigint_uint64__impl(
    env: napi_env,
    value: u64,
    result: *mut napi_value,
) -> napi_status {
    if result.is_null() {
        return napi_invalid_arg;
    }
    let env = env_mut!(env);
    *result = env.push_handle(Value::BigInt(Rc::new(crate::bigint::JsBigInt::from_u64(
        value,
    ))));
    napi_ok
}

unsafe fn napi_create_bigint_words__impl(
    _env: napi_env,
    _sign_bit: i32,
    _word_count: usize,
    _words: *const u64,
    _result: *mut napi_value,
) -> napi_status {
    napi_generic_failure
}

unsafe fn napi_get_value_bigint_int64__impl(
    env: napi_env,
    value: napi_value,
    result: *mut i64,
    lossless: *mut bool,
) -> napi_status {
    if result.is_null() {
        return napi_invalid_arg;
    }
    let env = env_mut!(env);
    match env.get_handle(value) {
        Some(Value::BigInt(b)) => {
            *result = b.to_f64() as i64;
            if !lossless.is_null() {
                *lossless = true;
            }
            napi_ok
        }
        _ => napi_number_expected,
    }
}

unsafe fn napi_get_value_bigint_uint64__impl(
    env: napi_env,
    value: napi_value,
    result: *mut u64,
    lossless: *mut bool,
) -> napi_status {
    if result.is_null() {
        return napi_invalid_arg;
    }
    let env = env_mut!(env);
    match env.get_handle(value) {
        Some(Value::BigInt(b)) => {
            *result = b.to_f64() as u64;
            if !lossless.is_null() {
                *lossless = true;
            }
            napi_ok
        }
        _ => napi_number_expected,
    }
}

unsafe fn napi_get_value_bigint_words__impl(
    _env: napi_env,
    _value: napi_value,
    _sign_bit: *mut i32,
    _word_count: *mut usize,
    _words: *mut u64,
) -> napi_status {
    napi_generic_failure
}

unsafe fn napi_create_arraybuffer__impl(
    env: napi_env,
    byte_length: usize,
    data: *mut *mut c_void,
    result: *mut napi_value,
) -> napi_status {
    if result.is_null() {
        return napi_invalid_arg;
    }
    let env = owner_env_mut!(env, "napi_create_arraybuffer");
    let rt = &mut *env.rt;

    let ab_proto = match rt.global_get("ArrayBuffer") {
        Value::Object(c) => match rt.object_get(c, "prototype") {
            Value::Object(p) => Some(p),
            _ => None,
        },
        _ => None,
    };
    let mut o = Object::new_ordinary();
    o.proto = ab_proto;
    let ab = rt.alloc_object(o);
    rt.heap.note_external_alloc(byte_length);
    rt.array_buffers.insert(
        ab,
        crate::interp::ArrayBufferRecord {
            byte_length,
            max_byte_length: byte_length,
            backing_epoch: 0,
            data: vec![0u8; byte_length],
            detached: false,
            untransferable: false,
            shared: None,
        },
    );
    if !data.is_null() {
        *data = rt.array_buffers.get_mut(&ab).unwrap().data.as_mut_ptr() as *mut c_void;
    }
    *result = env.push_handle(Value::Object(ab));
    napi_ok
}

unsafe fn napi_create_dataview__impl(
    env: napi_env,
    length: usize,
    arraybuffer: napi_value,
    byte_offset: usize,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_create_dataview");
    let buf_id = match env.get_handle(arraybuffer) {
        Some(Value::Object(id)) => *id,
        _ => return napi_object_expected,
    };
    let rt = &mut *env.rt;
    if !rt.array_buffers.contains_key(&buf_id) {
        return napi_invalid_arg;
    }
    let dv_proto = match rt.global_get("DataView") {
        Value::Object(c) => match rt.object_get(c, "prototype") {
            Value::Object(p) => Some(p),
            _ => None,
        },
        _ => None,
    };
    let mut o = Object::new_ordinary();
    o.proto = dv_proto;
    let id = rt.alloc_object(o);
    rt.register_typed_array_view(
        id,
        crate::interp::TypedArrayViewRecord {
            buffer: buf_id,
            byte_offset,
            fixed_length: Some(length),
            bytes_per_element: 1,
            element_kind: "DataView".into(),
        },
    );
    *result = env.push_handle(Value::Object(id));
    napi_ok
}

fn napi_typedarray_ctor_name(type_: napi_typedarray_type) -> Option<(&'static str, usize)> {
    if type_ == napi_int8_array {
        Some(("Int8Array", 1))
    } else if type_ == napi_uint8_array {
        Some(("Uint8Array", 1))
    } else if type_ == napi_uint8_clamped_array {
        Some(("Uint8ClampedArray", 1))
    } else if type_ == napi_int16_array {
        Some(("Int16Array", 2))
    } else if type_ == napi_uint16_array {
        Some(("Uint16Array", 2))
    } else if type_ == napi_int32_array {
        Some(("Int32Array", 4))
    } else if type_ == napi_uint32_array {
        Some(("Uint32Array", 4))
    } else if type_ == napi_float32_array {
        Some(("Float32Array", 4))
    } else if type_ == napi_float64_array {
        Some(("Float64Array", 8))
    } else if type_ == napi_bigint64_array {
        Some(("BigInt64Array", 8))
    } else if type_ == napi_biguint64_array {
        Some(("BigUint64Array", 8))
    } else {
        None
    }
}

unsafe fn napi_create_typedarray__impl(
    env: napi_env,
    type_: napi_typedarray_type,
    length: usize,
    arraybuffer: napi_value,
    byte_offset: usize,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_create_typedarray");
    let Some((ctor_name, bytes_per_element)) = napi_typedarray_ctor_name(type_) else {
        return napi_invalid_arg;
    };
    let buf_id = match env.get_handle(arraybuffer) {
        Some(Value::Object(id)) => *id,
        _ => return napi_invalid_arg,
    };
    let rt = &mut *env.rt;
    let Some(buffer) = rt.array_buffers.get(&buf_id) else {
        return napi_invalid_arg;
    };
    let Some(byte_len) = length.checked_mul(bytes_per_element) else {
        return napi_invalid_arg;
    };
    let Some(end) = byte_offset.checked_add(byte_len) else {
        return napi_invalid_arg;
    };
    if byte_offset % bytes_per_element != 0 || end > buffer.byte_len() {
        return napi_invalid_arg;
    }
    let view_proto = match rt.global_get(ctor_name) {
        Value::Object(c) => match rt.object_get(c, "prototype") {
            Value::Object(p) => Some(p),
            _ => None,
        },
        _ => None,
    };
    let mut o = Object::new_ordinary();
    o.proto = view_proto;
    let id = rt.alloc_object(o);
    rt.register_typed_array_view(
        id,
        crate::interp::TypedArrayViewRecord {
            buffer: buf_id,
            byte_offset,
            fixed_length: Some(length),
            bytes_per_element,
            element_kind: ctor_name.into(),
        },
    );
    *result = env.push_handle(Value::Object(id));
    napi_ok
}

unsafe fn napi_run_script__impl(
    env: napi_env,
    script: napi_value,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_run_script");
    let script_v = env.get_handle(script).cloned().unwrap_or(Value::Undefined);
    let rt = &mut *env.rt;
    let eval = rt.global_get("eval");
    match rt.call_function(eval, Value::Undefined, vec![script_v]) {
        Ok(v) => {
            *result = env.push_handle(v);
            napi_ok
        }
        Err(e) => {
            let reason = crate::module::runtime_error_to_rejection_value(rt, &e);
            env.pending_exception = Some(reason);
            napi_pending_exception
        }
    }
}

unsafe fn napi_create_buffer_copy__impl(
    env: napi_env,
    length: usize,
    data: *const c_void,
    result_data: *mut *mut c_void,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_create_buffer_copy");
    let mut bytes = vec![0u8; length];
    if !data.is_null() && length > 0 {
        std::ptr::copy_nonoverlapping(data as *const u8, bytes.as_mut_ptr(), length);
    }
    create_buffer_from_bytes(env, bytes, result_data, result)
}

unsafe fn create_buffer_from_bytes(
    env: &mut NapiEnv,
    bytes: Vec<u8>,
    result_data: *mut *mut c_void,
    result: *mut napi_value,
) -> napi_status {
    let length = bytes.len();
    let rt = &mut *env.rt;
    let ab_proto = match rt.global_get("ArrayBuffer") {
        Value::Object(c) => match rt.object_get(c, "prototype") {
            Value::Object(p) => Some(p),
            _ => None,
        },
        _ => None,
    };
    let mut abo = Object::new_ordinary();
    abo.proto = ab_proto;
    let ab_id = rt.alloc_object(abo);
    rt.heap.note_external_alloc(length);
    rt.array_buffers.insert(
        ab_id,
        crate::interp::ArrayBufferRecord {
            byte_length: length,
            max_byte_length: length,
            backing_epoch: 0,
            data: bytes,
            detached: false,
            untransferable: false,
            shared: None,
        },
    );
    if !result_data.is_null() {
        *result_data = rt.array_buffers.get_mut(&ab_id).unwrap().data.as_mut_ptr() as *mut c_void;
    }

    let buf_proto = match rt.global_get("Buffer") {
        Value::Object(c) => match rt.object_get(c, "prototype") {
            Value::Object(p) => Some(p),
            _ => None,
        },
        _ => None,
    };
    let mut vo = Object::new_ordinary();
    vo.set_own_internal(
        "__kind".into(),
        Value::String(Rc::new(crate::value::JsString::from("Uint8Array"))),
    );
    vo.set_own_internal(
        "__ta_kind".into(),
        Value::String(Rc::new(crate::value::JsString::from("Uint8Array"))),
    );
    vo.is_buffer = true;
    vo.proto = buf_proto;
    let view_id = rt.alloc_object(vo);
    rt.register_typed_array_view(
        view_id,
        crate::interp::TypedArrayViewRecord {
            buffer: ab_id,
            byte_offset: 0,
            fixed_length: Some(length),
            bytes_per_element: 1,
            element_kind: "Uint8Array".into(),
        },
    );
    rt.object_set(view_id, "__is_buffer__".into(), Value::Boolean(true));
    *result = env.push_handle(Value::Object(view_id));
    napi_ok
}

fn canonicalize_napi_return_buffer_like(rt: &mut Runtime, value: Value) -> Value {
    let Value::Object(id) = value else {
        return value;
    };
    if rt.typed_array_views.contains_key(&id) {
        return Value::Object(id);
    }
    let buffer_proto = match rt.global_get("Buffer") {
        Value::Object(c) => match rt.object_get(c, "prototype") {
            Value::Object(p) => p,
            _ => return Value::Object(id),
        },
        _ => return Value::Object(id),
    };
    if rt.obj(id).proto != Some(buffer_proto)
        && !rt.obj(id).is_buffer
        && !matches!(rt.object_get(id, "__is_buffer__"), Value::Boolean(true))
    {
        return Value::Object(id);
    }
    let mut bytes = Vec::new();
    loop {
        let key = bytes.len().to_string();
        match rt.object_get(id, &key) {
            Value::Number(n) if n.is_finite() => bytes.push(n as u8),
            Value::Undefined => break,
            _ => return Value::Object(id),
        }
    }
    if bytes.is_empty() {
        return Value::Object(id);
    }

    let ab_proto = match rt.global_get("ArrayBuffer") {
        Value::Object(c) => match rt.object_get(c, "prototype") {
            Value::Object(p) => Some(p),
            _ => None,
        },
        _ => None,
    };
    let mut abo = Object::new_ordinary();
    abo.proto = ab_proto;
    let ab_id = rt.alloc_object(abo);
    let length = bytes.len();
    rt.heap.note_external_alloc(length);
    rt.array_buffers.insert(
        ab_id,
        crate::interp::ArrayBufferRecord {
            byte_length: length,
            max_byte_length: length,
            backing_epoch: 0,
            data: bytes,
            detached: false,
            untransferable: false,
            shared: None,
        },
    );
    {
        let o = rt.obj_mut(id);
        o.is_buffer = true;
        o.set_own_internal(
            "__kind".into(),
            Value::String(Rc::new(crate::value::JsString::from("Uint8Array"))),
        );
        o.set_own_internal(
            "__ta_kind".into(),
            Value::String(Rc::new(crate::value::JsString::from("Uint8Array"))),
        );
        o.set_own_internal("__is_buffer__".into(), Value::Boolean(true));
    }
    rt.register_typed_array_view(
        id,
        crate::interp::TypedArrayViewRecord {
            buffer: ab_id,
            byte_offset: 0,
            fixed_length: Some(length),
            bytes_per_element: 1,
            element_kind: "Uint8Array".into(),
        },
    );
    Value::Object(id)
}

unsafe fn napi_create_external_buffer__impl(
    env: napi_env,
    length: usize,
    data: *mut c_void,
    _finalize_cb: *mut c_void,
    _finalize_hint: *mut c_void,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_create_external_buffer");
    let bytes = if data.is_null() || length == 0 {
        vec![0u8; length]
    } else {
        std::slice::from_raw_parts(data as *const u8, length).to_vec()
    };
    create_buffer_from_bytes(env, bytes, std::ptr::null_mut(), result)
}

unsafe fn napi_get_arraybuffer_info__impl(
    env: napi_env,
    value: napi_value,
    data: *mut *mut c_void,
    byte_length: *mut usize,
) -> napi_status {
    let env = owner_env_mut!(env, "napi_get_arraybuffer_info");
    let id = match env.get_handle(value) {
        Some(Value::Object(id)) => *id,
        _ => return napi_invalid_arg,
    };
    let rt = &*env.rt;
    if !data.is_null() {
        *data = match rt.object_get(id, "__ab_data") {
            Value::Number(n) => n as usize as *mut c_void,
            _ => std::ptr::null_mut(),
        };
    }
    if !byte_length.is_null() {
        *byte_length = match rt.object_get(id, "byteLength") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
    }
    napi_ok
}

unsafe fn napi_detach_arraybuffer__impl(env: napi_env, arraybuffer: napi_value) -> napi_status {
    let env = owner_env_mut!(env, "napi_detach_arraybuffer");
    let id = match env.get_handle(arraybuffer) {
        Some(Value::Object(id)) => *id,
        _ => return napi_invalid_arg,
    };
    let rt = &mut *env.rt;
    if !rt.array_buffers.contains_key(&id) {
        return napi_invalid_arg;
    }
    match rt.detach_array_buffer(id) {
        Ok(()) => napi_ok,
        Err(_) => napi_generic_failure,
    }
}

unsafe fn napi_create_external__impl(
    env: napi_env,
    data: *mut c_void,
    _finalize_cb: *mut c_void,
    _finalize_hint: *mut c_void,
    result: *mut napi_value,
) -> napi_status {
    if result.is_null() {
        return napi_invalid_arg;
    }
    let env = owner_env_mut!(env, "napi_create_external");
    let rt = &mut *env.rt;
    let id = rt.alloc_object(Object::new_ordinary());
    rt.obj_mut(id)
        .set_own_internal("__external_ptr".into(), Value::Number(data as usize as f64));
    *result = env.push_handle(Value::Object(id));
    napi_ok
}

unsafe fn napi_get_value_external__impl(
    env: napi_env,
    value: napi_value,
    result: *mut *mut c_void,
) -> napi_status {
    if result.is_null() {
        return napi_invalid_arg;
    }
    let env = owner_env_mut!(env, "napi_get_value_external");
    let id = match env.get_handle(value) {
        Some(Value::Object(id)) => *id,
        _ => return napi_invalid_arg,
    };
    let rt = &*env.rt;
    *result = match rt.object_get(id, "__external_ptr") {
        Value::Number(n) => n as usize as *mut c_void,
        _ => std::ptr::null_mut(),
    };
    napi_ok
}

unsafe fn napi_add_finalizer__impl(
    env: napi_env,
    _object: napi_value,
    _native_object: *mut c_void,
    _finalize_cb: *mut c_void,
    _finalize_hint: *mut c_void,
    result: *mut napi_ref,
) -> napi_status {

    if !result.is_null() {
        *result = std::ptr::null_mut();
    }
    let _ = env;
    napi_ok
}

unsafe fn napi_adjust_external_memory__impl(
    _env: napi_env,
    change_in_bytes: i64,
    result: *mut i64,
) -> napi_status {
    if !result.is_null() {
        *result = change_in_bytes.max(0);
    }
    napi_ok
}

unsafe fn napi_object_freeze__impl(_env: napi_env, _object: napi_value) -> napi_status {
    napi_ok
}

unsafe fn napi_object_seal__impl(_env: napi_env, _object: napi_value) -> napi_status {
    napi_ok
}

unsafe fn napi_get_new_target__impl(
    env: napi_env,
    cbinfo: napi_callback_info,
    result: *mut napi_value,
) -> napi_status {
    if cbinfo.is_null() {
        return napi_invalid_arg;
    }
    if !result.is_null() {
        let env = env_mut!(env);
        let info = &*cbinfo;
        *result = match &info.new_target {
            Some(v) => env.push_handle(v.clone()),
            None => std::ptr::null_mut(),
        };
    }
    napi_ok
}

unsafe fn napi_get_property_names__impl(
    env: napi_env,
    object: napi_value,
    result: *mut napi_value,
) -> napi_status {
    if result.is_null() {
        return napi_invalid_arg;
    }
    let env = owner_env_mut!(env, "napi_get_property_names");
    let id = match env.get_handle(object) {
        Some(Value::Object(id)) => *id,
        _ => return napi_object_expected,
    };
    let rt = &mut *env.rt;

    let keys = rt.ordinary_own_enumerable_string_keys(id);
    let arr = rt.alloc_object(Object::new_array());
    for (i, k) in keys.iter().enumerate() {
        rt.object_set(
            arr,
            i.to_string(),
            Value::String(Rc::new(crate::value::JsString::from(k.clone()))),
        );
    }
    rt.object_set(arr, "length".into(), Value::Number(keys.len() as f64));
    *result = env.push_handle(Value::Object(arr));
    napi_ok
}

unsafe fn napi_add_env_cleanup_hook__impl(
    _env: napi_env,
    _fun: *mut c_void,
    _arg: *mut c_void,
) -> napi_status {
    napi_ok
}

unsafe fn napi_remove_env_cleanup_hook__impl(
    _env: napi_env,
    _fun: *mut c_void,
    _arg: *mut c_void,
) -> napi_status {
    napi_ok
}

pub type napi_deferred = *mut NapiDeferred;
pub struct NapiDeferred {

    promise_id: rusty_js_gc::ObjectId,
    env: SendPtr<NapiEnv>,

    promise_ref_slot: usize,
}

unsafe fn napi_create_promise__impl(
    env: napi_env,
    deferred: *mut napi_deferred,
    promise: *mut napi_value,
) -> napi_status {
    if deferred.is_null() || promise.is_null() {
        return napi_invalid_arg;
    }
    let env_ref = env_mut!(env);
    let rt = &mut *env_ref.rt;
    let p_id = crate::promise::new_promise(rt);
    let promise_ref_slot = env_ref.refs.len();
    env_ref.refs.push(Some(Value::Object(p_id)));
    let d = Box::into_raw(Box::new(NapiDeferred {
        promise_id: p_id,
        env: SendPtr(env),
        promise_ref_slot,
    }));
    *deferred = d;
    *promise = env_ref.push_handle(Value::Object(p_id));
    napi_ok
}

unsafe fn napi_resolve_deferred__impl(
    env: napi_env,
    deferred: napi_deferred,
    resolution: napi_value,
) -> napi_status {
    if deferred.is_null() {
        return napi_invalid_arg;
    }
    let env_ref = env_mut!(env);
    let v = env_ref
        .get_handle(resolution)
        .cloned()
        .unwrap_or(Value::Undefined);
    let d = Box::from_raw(deferred);
    let rt = &mut *env_ref.rt;
    crate::promise::resolve_promise(rt, d.promise_id, v);
    if let Some(slot) = env_ref.refs.get_mut(d.promise_ref_slot) {
        *slot = None;
    }
    napi_ok
}

unsafe fn napi_reject_deferred__impl(
    env: napi_env,
    deferred: napi_deferred,
    reason: napi_value,
) -> napi_status {
    if deferred.is_null() {
        return napi_invalid_arg;
    }
    let env_ref = env_mut!(env);
    let v = env_ref
        .get_handle(reason)
        .cloned()
        .unwrap_or(Value::Undefined);
    let d = Box::from_raw(deferred);
    let rt = &mut *env_ref.rt;
    crate::promise::reject_promise(rt, d.promise_id, v);
    if let Some(slot) = env_ref.refs.get_mut(d.promise_ref_slot) {
        *slot = None;
    }
    napi_ok
}

unsafe fn napi_coerce_to_object__impl(
    env: napi_env,
    value: napi_value,
    result: *mut napi_value,
) -> napi_status {
    if result.is_null() {
        return napi_invalid_arg;
    }
    let env_ref = env_mut!(env);
    let v = env_ref
        .get_handle(value)
        .cloned()
        .unwrap_or(Value::Undefined);

    let out = match v {
        Value::Object(_) => v,
        other => {
            let rt = &mut *env_ref.rt;
            let id = rt.alloc_object(Object::new_ordinary());
            rt.object_set(id, "__value".into(), other);
            Value::Object(id)
        }
    };
    *result = env_ref.push_handle(out);
    napi_ok
}

unsafe fn napi_get_buffer_info__impl(
    env: napi_env,
    value: napi_value,
    data: *mut *mut c_void,
    length: *mut usize,
) -> napi_status {
    let env_ref = env_mut!(env);
    let id = match env_ref.get_handle(value) {
        Some(Value::Object(id)) => *id,
        _ => return napi_invalid_arg,
    };
    let rt = &mut *env_ref.rt;
    if let Some(view) = rt.typed_array_views.get(&id).cloned() {
        let Some(buffer) = rt.array_buffers.get_mut(&view.buffer) else {
            return napi_invalid_arg;
        };
        if buffer.detached || buffer.shared.is_some() || view.byte_offset > buffer.data.len() {
            return napi_invalid_arg;
        }
        let len = view.fixed_length.unwrap_or_else(|| {
            buffer.byte_len().saturating_sub(view.byte_offset) / view.bytes_per_element.max(1)
        });
        if !length.is_null() {
            *length = len.saturating_mul(view.bytes_per_element.max(1));
        }
        if !data.is_null() {
            *data = buffer.data.as_mut_ptr().add(view.byte_offset) as *mut c_void;
        }
        return napi_ok;
    }

    let len = match rt.object_get(id, "length") {
        Value::Number(n) => n as usize,
        _ => 0,
    };
    if !length.is_null() {
        *length = len;
    }
    if !data.is_null() {

        let mut bytes: Vec<u8> = Vec::with_capacity(len);
        for i in 0..len {
            bytes.push(match rt.object_get(id, &i.to_string()) {
                Value::Number(n) => n as u8,
                _ => 0,
            });
        }
        let boxed = bytes.into_boxed_slice();
        *data = Box::into_raw(boxed) as *mut c_void;
    }
    napi_ok
}

unsafe fn napi_new_instance__impl(
    env: napi_env,
    constructor: napi_value,
    argc: usize,
    argv: *const napi_value,
    result: *mut napi_value,
) -> napi_status {
    if result.is_null() {
        return napi_invalid_arg;
    }
    let env_ref = env_mut!(env);
    let ctor_v = match env_ref.get_handle(constructor) {
        Some(v) => v.clone(),
        None => return napi_function_expected,
    };
    let mut args: Vec<Value> = Vec::with_capacity(argc);
    for i in 0..argc {
        let h = *argv.add(i);
        args.push(env_ref.get_handle(h).cloned().unwrap_or(Value::Undefined));
    }
    let rt = &mut *env_ref.rt;
    match rt.construct(ctor_v, args) {
        Ok(v) => {
            *result = env_ref.push_handle(v);
            napi_ok
        }
        Err(e) => {
            env_ref.pending_exception = Some(match e {
                crate::RuntimeError::Thrown(v) => v,
                _ => Value::String(Rc::new(crate::value::JsString::from(format!("{:?}", e)))),
            });
            napi_pending_exception
        }
    }
}

unsafe fn napi_fatal_exception__impl(env: napi_env, err: napi_value) -> napi_status {
    let env_ref = env_mut!(env);
    let v = env_ref.get_handle(err).cloned().unwrap_or(Value::Undefined);
    eprintln!("cruft: napi fatal exception: {:?}", v);
    napi_ok
}

unsafe fn napi_set_instance_data__impl(
    env: napi_env,
    data: *mut c_void,
    _finalize_cb: Option<napi_finalize>,
    _finalize_hint: *mut c_void,
) -> napi_status {
    let env_ref = env_mut!(env);
    env_ref.instance_data = data;
    napi_ok
}

unsafe fn napi_get_instance_data__impl(env: napi_env, data: *mut *mut c_void) -> napi_status {
    let env_ref = env_mut!(env);
    check_arg!(data);
    *data = env_ref.instance_data;
    napi_ok
}

unsafe fn napi_get_uv_event_loop__impl(env: napi_env, loop_: *mut *mut c_void) -> napi_status {
    let _env_ref = env_mut!(env);
    check_arg!(loop_);
    *loop_ = (&raw mut UV_LOOP_SENTINEL).cast::<c_void>();
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn uv_mutex_init(mutex: *mut c_void) -> i32 {
    if mutex.is_null() {
        return -1;
    }
    #[cfg(unix)]
    {
        pthread_mutex_init(mutex, std::ptr::null())
    }
    #[cfg(windows)]
    {
        InitializeCriticalSection(mutex);
        0
    }
    #[cfg(not(any(unix, windows)))]
    {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn uv_mutex_destroy(mutex: *mut c_void) {
    if !mutex.is_null() {
        #[cfg(unix)]
        let _ = pthread_mutex_destroy(mutex);
        #[cfg(windows)]
        DeleteCriticalSection(mutex);
    }
}

#[no_mangle]
pub unsafe extern "C" fn uv_mutex_lock(mutex: *mut c_void) {
    if !mutex.is_null() {
        #[cfg(unix)]
        let _ = pthread_mutex_lock(mutex);
        #[cfg(windows)]
        EnterCriticalSection(mutex);
    }
}

#[no_mangle]
pub unsafe extern "C" fn uv_mutex_unlock(mutex: *mut c_void) {
    if !mutex.is_null() {
        #[cfg(unix)]
        let _ = pthread_mutex_unlock(mutex);
        #[cfg(windows)]
        LeaveCriticalSection(mutex);
    }
}

#[no_mangle]
pub unsafe extern "C" fn uv_async_init(
    _loop_: *mut c_void,
    _handle: *mut c_void,
    _cb: Option<uv_async_cb>,
) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn uv_async_send(_handle: *mut c_void) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn uv_close(_handle: *mut c_void, _close_cb: Option<uv_close_cb>) {}

#[no_mangle]
pub unsafe extern "C" fn uv_run(_loop_: *mut c_void, _mode: i32) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn uv_default_loop() -> *mut c_void {
    (&raw mut UV_LOOP_SENTINEL).cast::<c_void>()
}

#[no_mangle]
pub unsafe extern "C" fn uv_ref(_handle: *mut c_void) {}

#[no_mangle]
pub unsafe extern "C" fn uv_unref(_handle: *mut c_void) {}

#[no_mangle]
pub unsafe extern "C" fn uv_poll_init(_loop_: *mut c_void, _handle: *mut c_void, _fd: i32) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn uv_poll_start(
    _handle: *mut c_void,
    _events: i32,
    _cb: *mut c_void,
) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn uv_poll_stop(_handle: *mut c_void) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn uv_strerror(_err: i32) -> *const c_char {
    static UNKNOWN: &[u8] = b"unknown system error\0";
    UNKNOWN.as_ptr().cast::<c_char>()
}

#[no_mangle]
pub unsafe extern "C" fn napi_async_init(
    env: napi_env,
    _async_resource: napi_value,
    _async_resource_name: napi_value,
    result: *mut *mut c_void,
) -> napi_status {
    let _ = env_mut!(env);
    check_arg!(result);
    *result = Box::into_raw(Box::new(0u8)).cast::<c_void>();
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_async_destroy(
    env: napi_env,
    async_context: *mut c_void,
) -> napi_status {
    let _ = env_mut!(env);
    if !async_context.is_null() {
        drop(Box::from_raw(async_context.cast::<u8>()));
    }
    napi_ok
}

static mut CALLBACK_SCOPE_SENTINEL: u8 = 0;

#[no_mangle]
pub unsafe extern "C" fn napi_open_callback_scope(
    env: napi_env,
    _resource_object: napi_value,
    _context: *mut c_void,
    result: *mut *mut c_void,
) -> napi_status {
    let _ = env_mut!(env);
    check_arg!(result);
    *result = (&raw mut CALLBACK_SCOPE_SENTINEL).cast::<c_void>();
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_close_callback_scope(
    env: napi_env,
    _scope: *mut c_void,
) -> napi_status {
    let _ = env_mut!(env);
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_make_callback(
    env: napi_env,
    _async_context: *mut c_void,
    recv: napi_value,
    func: napi_value,
    argc: usize,
    argv: *const napi_value,
    result: *mut napi_value,
) -> napi_status {
    napi_call_function__impl(env, recv, func, argc, argv, result)
}

#[export_name = "_ZN2v87Isolate10GetCurrentEv"]
pub unsafe extern "C" fn v8_isolate_get_current_abi() -> *mut c_void {
    old_v8_isolate_facade_ptr()
}

#[export_name = "_ZN4node19GetCurrentEventLoopEPN2v87IsolateE"]
pub unsafe extern "C" fn node_get_current_event_loop_abi(_isolate: *mut c_void) -> *mut c_void {
    (&raw mut UV_LOOP_SENTINEL).cast::<c_void>()
}

#[export_name = "_ZN2v88internal9Internals17GetCurrentIsolateEv"]
pub unsafe extern "C" fn v8_internal_get_current_isolate_abi() -> *mut c_void {
    v8_isolate_get_current_abi()
}

#[export_name = "_ZN2v87Isolate17GetCurrentContextEv"]
pub unsafe extern "C" fn v8_isolate_get_current_context_abi(_isolate: *mut c_void) -> *mut c_void {
    old_v8_with_state(std::ptr::null_mut(), |state| {
        state.local_handle_for(state.current_context)
    })
}

#[export_name = "_ZN2v811HandleScope6ExtendEPNS_7IsolateE"]
pub unsafe extern "C" fn v8_handle_scope_extend_abi(_isolate: *mut c_void) -> *mut c_void {
    unsafe {
        let facade = &raw mut OLD_V8_ISOLATE_FACADE;
        let arena_start = (&raw mut (*facade).arena).cast::<usize>();
        let arena_end = arena_start.add(OLD_V8_HANDLE_ARENA_LEN);
        let current = (*facade).handle_scope.next;
        if current.is_null() || current < arena_start || current >= arena_end {
            (*facade).handle_scope.next = arena_start;
            (*facade).handle_scope.limit = arena_end;
            return arena_start.cast::<c_void>();
        }
        current.cast::<c_void>()
    }
}

#[export_name = "_ZN2v811HandleScope16DeleteExtensionsEPNS_7IsolateE"]
pub unsafe extern "C" fn v8_handle_scope_delete_extensions_abi(_isolate: *mut c_void) {}

#[export_name = "_ZN2v824EscapableHandleScopeBaseC2EPNS_7IsolateE"]
pub unsafe extern "C" fn v8_escapable_handle_scope_ctor_abi(
    this: *mut c_void,
    isolate: *mut c_void,
) {
    if this.is_null() {
        return;
    }
    let isolate = if isolate.is_null() {
        old_v8_isolate_facade_ptr()
    } else {
        isolate
    };
    let facade = &raw mut OLD_V8_ISOLATE_FACADE;
    let arena_start = (&raw mut (*facade).arena).cast::<usize>();
    let arena_end = arena_start.add(OLD_V8_HANDLE_ARENA_LEN);
    if (*facade).handle_scope.next.is_null() {
        (*facade).handle_scope.next = arena_start;
        (*facade).handle_scope.limit = arena_end;
    }
    let prev_next = (*facade).handle_scope.next;
    let prev_limit = (*facade).handle_scope.limit;
    (*facade).handle_scope.level += 1;
    let escape_slot = old_v8_create_extra_handle_slot();
    let words = this.cast::<usize>();
    *words.add(0) = isolate as usize;
    *words.add(1) = prev_next as usize;
    *words.add(2) = prev_limit as usize;
    *words.add(3) = escape_slot as usize;
}

#[export_name = "_ZN2v824EscapableHandleScopeBase10EscapeSlotEPm"]
pub unsafe extern "C" fn v8_escapable_handle_scope_escape_slot_abi(
    this: *mut c_void,
    slot: *mut usize,
) -> *mut c_void {
    if slot.is_null() {
        return std::ptr::null_mut();
    }
    if this.is_null() {
        return slot.cast::<c_void>();
    }
    let words = this.cast::<usize>();
    let escape_slot = *words.add(3) as *mut usize;
    if escape_slot.is_null() {
        return slot.cast::<c_void>();
    }
    *escape_slot = *slot;
    escape_slot.cast::<c_void>()
}

#[export_name = "_ZN2v812api_internal12ToLocalEmptyEv"]
pub unsafe extern "C" fn v8_api_internal_to_local_empty_abi() {
    panic!("native-addon: V8 MaybeLocal::ToLocalChecked on empty value");
}

#[export_name = "_ZN2v812api_internal17FromJustIsNothingEv"]
pub unsafe extern "C" fn v8_api_internal_from_just_is_nothing_abi() {
    panic!("native-addon: V8 Maybe::FromJust on empty value");
}

#[export_name = "_ZN2v812api_internal13DisposeGlobalEPm"]
pub unsafe extern "C" fn v8_api_internal_dispose_global_abi(_location: *mut usize) {}

#[export_name = "_ZN2v812api_internal18GlobalizeReferenceEPNS_8internal7IsolateEm"]
pub unsafe extern "C" fn v8_api_internal_globalize_reference_abi(
    _isolate: *mut c_void,
    value: usize,
) -> *mut usize {
    Box::into_raw(Box::new(value))
}

#[export_name = "_ZN2v812api_internal9ClearWeakEPm"]
pub unsafe extern "C" fn v8_api_internal_clear_weak_abi(_location: *mut usize) -> *mut c_void {
    std::ptr::null_mut()
}

#[export_name = "_ZN2v812api_internal8MakeWeakEPmPvPFvRKNS_16WeakCallbackInfoIvEEENS_16WeakCallbackTypeE"]
pub unsafe extern "C" fn v8_api_internal_make_weak_abi(
    _location: *mut usize,
    _parameter: *mut c_void,
    _callback: *mut c_void,
    _type_: i32,
) {
}

#[export_name = "_ZN2v812api_internal23GetFunctionTemplateDataEPNS_7IsolateENS_5LocalINS_4DataEEE"]
pub unsafe extern "C" fn v8_api_internal_get_function_template_data_abi(
    _isolate: *mut c_void,
    data: *mut c_void,
) -> *mut c_void {
    data
}

#[export_name = "_ZN2v86String11NewFromUtf8EPNS_7IsolateEPKcNS_13NewStringTypeEi"]
pub unsafe extern "C" fn v8_string_new_from_utf8_abi(
    _isolate: *mut c_void,
    data: *const c_char,
    _new_type: i32,
    length: i32,
) -> *mut c_void {
    if data.is_null() {
        return std::ptr::null_mut();
    }
    let s = if length < 0 {
        CStr::from_ptr(data).to_string_lossy().into_owned()
    } else {
        let bytes = std::slice::from_raw_parts(data as *const u8, length as usize);
        String::from_utf8_lossy(bytes).into_owned()
    };
    old_v8_with_state(std::ptr::null_mut(), |state| {
        state.local_handle(OldV8Cell::Value(Value::String(Rc::new(
            crate::value::JsString::from(s),
        ))))
    })
}

#[export_name = "_ZN2v87Integer3NewEPNS_7IsolateEi"]
pub unsafe extern "C" fn v8_integer_new_abi(_isolate: *mut c_void, value: i32) -> *mut c_void {
    old_v8_with_state(std::ptr::null_mut(), |state| {
        state.local_handle(OldV8Cell::Value(Value::Number(value as f64)))
    })
}

#[export_name = "_ZN2v87Integer15NewFromUnsignedEPNS_7IsolateEj"]
pub unsafe extern "C" fn v8_integer_new_from_unsigned_abi(
    _isolate: *mut c_void,
    value: u32,
) -> *mut c_void {
    old_v8_with_state(std::ptr::null_mut(), |state| {
        state.local_handle(OldV8Cell::Value(Value::Number(value as f64)))
    })
}

#[export_name = "_ZN2v88External3NewEPNS_7IsolateEPvt"]
pub unsafe extern "C" fn v8_external_new_abi(
    _isolate: *mut c_void,
    value: *mut c_void,
    _tag: u16,
) -> *mut c_void {
    old_v8_with_state(std::ptr::null_mut(), |state| {
        state.local_handle(OldV8Cell::External(value))
    })
}

#[export_name = "_ZN2v820ToExternalPointerTagEt"]
pub unsafe extern "C" fn v8_to_external_pointer_tag_abi(tag: u16) -> u16 {
    tag
}

#[export_name = "_ZN2v89Exception5ErrorENS_5LocalINS_6StringEEENS1_INS_5ValueEEE"]
pub unsafe extern "C" fn v8_exception_error_abi(
    message: *mut c_void,
    _options: *mut c_void,
) -> *mut c_void {
    old_v8_with_state(std::ptr::null_mut(), |state| unsafe {
        old_v8_make_error_object(state, "Error", message)
    })
}

#[export_name = "_ZN2v89Exception9TypeErrorENS_5LocalINS_6StringEEENS1_INS_5ValueEEE"]
pub unsafe extern "C" fn v8_exception_type_error_abi(
    message: *mut c_void,
    _options: *mut c_void,
) -> *mut c_void {
    old_v8_with_state(std::ptr::null_mut(), |state| unsafe {
        old_v8_make_error_object(state, "TypeError", message)
    })
}

#[export_name = "_ZN2v87Isolate14ThrowExceptionENS_5LocalINS_5ValueEEE"]
pub unsafe extern "C" fn v8_isolate_throw_exception_abi(
    _isolate: *mut c_void,
    exception: *mut c_void,
) -> *mut c_void {
    old_v8_with_state(std::ptr::null_mut(), |state| {
        let value =
            unsafe { old_v8_cell_value(exception.cast::<OldV8Cell>()) }.unwrap_or(Value::Undefined);
        state.pending_exception = Some(value.clone());
        state.local_handle(OldV8Cell::Value(value))
    })
}

#[export_name = "_ZN4node6Buffer4CopyEPN2v87IsolateEPKcm"]
pub unsafe extern "C" fn node_buffer_copy_abi(
    _isolate: *mut c_void,
    data: *const c_char,
    length: usize,
) -> *mut c_void {
    if data.is_null() && length > 0 {
        return std::ptr::null_mut();
    }
    let bytes = if length == 0 {
        Vec::new()
    } else {
        std::slice::from_raw_parts(data as *const u8, length).to_vec()
    };
    old_v8_with_state(std::ptr::null_mut(), |state| {
        let rt = unsafe { &mut *state.rt };
        let id = rt.alloc_uint8_array_from_bytes(&bytes);
        rt.obj_mut(id)
            .set_own_internal("__is_buffer".into(), Value::Boolean(true));
        rt.obj_mut(id)
            .set_own_internal("__is_buffer__".into(), Value::Boolean(true));
        if let Value::Object(buffer_ctor) = rt.global_get("Buffer") {
            if let Value::Object(buffer_proto) = rt.object_get(buffer_ctor, "prototype") {
                rt.obj_mut(id).proto = Some(buffer_proto);
            }
        }
        state.local_handle(OldV8Cell::Value(Value::Object(id)))
    })
}

#[export_name = "_ZN4node6Buffer4DataEN2v85LocalINS1_6ObjectEEE"]
pub unsafe extern "C" fn node_buffer_data_abi(object: *mut c_void) -> *mut c_char {
    old_v8_with_state(std::ptr::null_mut(), |state| {
        let Some(OldV8Cell::Value(Value::Object(id))) =
            old_v8_resolve_cell_in_state(state, object).and_then(|p| unsafe { p.as_ref() })
        else {
            return std::ptr::null_mut();
        };
        let rt = unsafe { &mut *state.rt };
        let Some(view) = rt.typed_array_views.get(&id).cloned() else {
            return std::ptr::null_mut();
        };
        let Some(buf) = rt.array_buffers.get_mut(&view.buffer) else {
            return std::ptr::null_mut();
        };
        if buf.detached {
            return std::ptr::null_mut();
        }
        let start = view.byte_offset.min(buf.byte_len());
        unsafe { buf.data.as_mut_ptr().add(start) as *mut c_char }
    })
}

#[export_name = "_ZN4node6Buffer6LengthEN2v85LocalINS1_6ObjectEEE"]
pub unsafe extern "C" fn node_buffer_length_abi(object: *mut c_void) -> usize {
    old_v8_with_state(0, |state| {
        let Some(OldV8Cell::Value(Value::Object(id))) =
            old_v8_resolve_cell_in_state(state, object).and_then(|p| unsafe { p.as_ref() })
        else {
            return 0;
        };
        let rt = unsafe { state.rt_mut() };
        if let Some(view) = rt.typed_array_views.get(&id) {
            let Some(buf) = rt.array_buffers.get(&view.buffer) else {
                return 0;
            };
            if buf.detached {
                return 0;
            }
            return view
                .fixed_length
                .unwrap_or_else(|| buf.byte_len().saturating_sub(view.byte_offset));
        }
        match rt.object_get(*id, "byteLength") {
            Value::Number(n) => n.max(0.0) as usize,
            _ => 0,
        }
    })
}

#[export_name = "_ZNK2v85Value8IsNumberEv"]
pub unsafe extern "C" fn v8_value_is_number_abi(this: *mut c_void) -> bool {
    matches!(old_v8_value_from_any_handle(this), Some(Value::Number(_)))
}

#[export_name = "_ZNK2v85Value9IsBooleanEv"]
pub unsafe extern "C" fn v8_value_is_boolean_abi(this: *mut c_void) -> bool {
    matches!(old_v8_value_from_any_handle(this), Some(Value::Boolean(_)))
}

#[export_name = "_ZNK2v85Value8IsObjectEv"]
pub unsafe extern "C" fn v8_value_is_object_abi(this: *mut c_void) -> bool {
    matches!(old_v8_value_from_any_handle(this), Some(Value::Object(_)))
}

#[export_name = "_ZNK2v85Value12BooleanValueEPNS_7IsolateE"]
pub unsafe extern "C" fn v8_value_boolean_value_abi(
    this: *mut c_void,
    _isolate: *mut c_void,
) -> bool {
    match old_v8_value_from_any_handle(this) {
        Some(Value::Boolean(b)) => b,
        Some(Value::Null | Value::Undefined) | None => false,
        Some(Value::Number(n)) => n != 0.0 && !n.is_nan(),
        Some(Value::String(s)) => !s.as_str().is_empty(),
        Some(_) => true,
    }
}

#[export_name = "_ZNK2v85Value10Int32ValueENS_5LocalINS_7ContextEEE"]
pub unsafe extern "C" fn v8_value_int32_value_abi(this: *mut c_void, _context: *mut c_void) -> u64 {
    let n = match old_v8_value_from_any_handle(this) {
        Some(Value::Number(n)) => n as i32,
        Some(Value::Boolean(b)) => i32::from(b),
        Some(Value::String(s)) => s.as_str().parse::<f64>().unwrap_or(0.0) as i32,
        _ => 0,
    };
    old_v8_maybe_i32(n)
}

#[export_name = "_ZNK2v85Value11Uint32ValueENS_5LocalINS_7ContextEEE"]
pub unsafe extern "C" fn v8_value_uint32_value_abi(
    this: *mut c_void,
    _context: *mut c_void,
) -> u64 {
    let n = match old_v8_value_from_any_handle(this) {
        Some(Value::Number(n)) => n as u32,
        Some(Value::Boolean(b)) => u32::from(b),
        Some(Value::String(s)) => s.as_str().parse::<f64>().unwrap_or(0.0) as u32,
        _ => 0,
    };
    old_v8_maybe_u32(n)
}

#[export_name = "_ZNK2v85Value9ToIntegerENS_5LocalINS_7ContextEEE"]
pub unsafe extern "C" fn v8_value_to_integer_abi(
    this: *mut c_void,
    _context: *mut c_void,
) -> *mut c_void {
    let n = match old_v8_value_from_any_handle(this) {
        Some(Value::Number(n)) => n.trunc(),
        Some(Value::Boolean(b)) => f64::from(u8::from(b)),
        Some(Value::String(s)) => s.as_str().parse::<f64>().unwrap_or(0.0).trunc(),
        _ => 0.0,
    };
    old_v8_with_state(std::ptr::null_mut(), |state| {
        state.local_handle(OldV8Cell::Value(Value::Number(n)))
    })
}

#[export_name = "_ZNK2v85Value8ToStringENS_5LocalINS_7ContextEEE"]
pub unsafe extern "C" fn v8_value_to_string_abi(
    this: *mut c_void,
    _context: *mut c_void,
) -> *mut c_void {
    let s = match old_v8_cell_value(this.cast::<OldV8Cell>()) {
        Some(Value::String(s)) => s.as_str().to_string(),
        Some(v) => crate::abstract_ops::to_string(&v).to_string(),
        None => String::new(),
    };
    old_v8_with_state(std::ptr::null_mut(), |state| {
        state.local_handle(OldV8Cell::Value(Value::String(Rc::new(
            crate::value::JsString::from(s),
        ))))
    })
}

#[export_name = "_ZNK2v86String6LengthEv"]
pub unsafe extern "C" fn v8_string_length_abi(this: *mut c_void) -> i32 {
    match old_v8_resolve_cell(this).and_then(|p| p.as_ref()) {
        Some(OldV8Cell::Value(Value::String(s))) => s.as_str().encode_utf16().count() as i32,
        _ => 0,
    }
}

#[export_name = "_ZNK2v86String11WriteUtf8V2EPNS_7IsolateEPcmiPm"]
pub unsafe extern "C" fn v8_string_write_utf8_v2_abi(
    this: *mut c_void,
    _isolate: *mut c_void,
    buffer: *mut c_char,
    capacity: usize,
    _flags: i32,
    processed: *mut usize,
) -> i32 {
    let bytes = match old_v8_resolve_cell(this).and_then(|p| p.as_ref()) {
        Some(OldV8Cell::Value(Value::String(s))) => s.as_str().as_bytes(),
        _ => &[],
    };
    let n = bytes.len().min(capacity);
    if !buffer.is_null() && n > 0 {
        std::ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, buffer, n);
    }
    if !processed.is_null() {
        *processed = n;
    }
    n as i32
}

#[export_name = "_ZNK2v88External5ValueEt"]
pub unsafe extern "C" fn v8_external_value_abi(this: *mut c_void, _tag: u16) -> *mut c_void {
    match old_v8_resolve_cell(this).and_then(|p| p.as_ref()) {
        Some(OldV8Cell::External(ptr)) => *ptr,
        _ => std::ptr::null_mut(),
    }
}

#[export_name = "_ZN2v816FunctionTemplate3NewEPNS_7IsolateEPFvRKNS_20FunctionCallbackInfoINS_5ValueEEEENS_5LocalIS4_EENSA_INS_9SignatureEEEiNS_19ConstructorBehaviorENS_14SideEffectTypeEPKNS_9CFunctionEttt"]
pub unsafe extern "C" fn v8_function_template_new_abi(
    _isolate: *mut c_void,
    callback: Option<OldV8Callback>,
    data: *mut c_void,
    _signature: *mut c_void,
    _length: i32,
    _behavior: i32,
    _side_effect: i32,
    _c_function: *const c_void,
    _instance_type: u16,
    _instance_type2: u16,
    _instance_type3: u16,
) -> *mut c_void {
    old_v8_with_state(std::ptr::null_mut(), |state| {
        let instance_template = state.alloc(OldV8Cell::ObjectTemplate(OldV8ObjectTemplate::new()));
        let prototype_template = state.alloc(OldV8Cell::ObjectTemplate(OldV8ObjectTemplate::new()));
        let data = old_v8_cell_value(data.cast::<OldV8Cell>());
        let mut callback_is_nan = false;
        let callback = data
            .as_ref()
            .and_then(|value| {
                let Value::Object(id) = value else {
                    return None;
                };
                let rt = unsafe { state.rt_mut() };
                let field = rt.object_get(*id, &old_v8_internal_field_key(1));
                match field {
                    Value::Number(ptr) if ptr != 0.0 => {
                        callback_is_nan = true;
                        Some(unsafe { std::mem::transmute::<usize, OldV8Callback>(ptr as usize) })
                    }
                    _ => None,
                }
            })
            .or(callback);
        state.local_handle(OldV8Cell::FunctionTemplate(OldV8FunctionTemplate {
            class_name: None,
            callback,
            callback_is_nan,
            data,
            instance_template,
            prototype_template,
        }))
    })
}

#[export_name = "_ZN2v816FunctionTemplate12SetClassNameENS_5LocalINS_6StringEEE"]
pub unsafe extern "C" fn v8_function_template_set_class_name_abi(
    this: *mut c_void,
    name: *mut c_void,
) {
    let class_name = old_v8_cell_string(name.cast::<OldV8Cell>());
    let tpl_cell = old_v8_resolve_cell(this);
    if let (Some(class_name), Some(OldV8Cell::FunctionTemplate(tpl))) =
        (class_name, tpl_cell.and_then(|p| p.as_mut()))
    {
        tpl.class_name = Some(class_name);
    }
}

#[export_name = "_ZN2v816FunctionTemplate16InstanceTemplateEv"]
pub unsafe extern "C" fn v8_function_template_instance_template_abi(
    this: *mut c_void,
) -> *mut c_void {
    old_v8_with_state(
        std::ptr::null_mut(),
        |state| match old_v8_resolve_cell_in_state(state, this).and_then(|p| unsafe { p.as_ref() })
        {
            Some(OldV8Cell::FunctionTemplate(tpl)) => state.local_handle_for(tpl.instance_template),
            _ => std::ptr::null_mut(),
        },
    )
}

#[export_name = "_ZN2v816FunctionTemplate17PrototypeTemplateEv"]
pub unsafe extern "C" fn v8_function_template_prototype_template_abi(
    this: *mut c_void,
) -> *mut c_void {
    old_v8_with_state(
        std::ptr::null_mut(),
        |state| match old_v8_resolve_cell_in_state(state, this).and_then(|p| unsafe { p.as_ref() })
        {
            Some(OldV8Cell::FunctionTemplate(tpl)) => {
                state.local_handle_for(tpl.prototype_template)
            }
            _ => std::ptr::null_mut(),
        },
    )
}

#[export_name = "_ZN2v814ObjectTemplate3NewEPNS_7IsolateENS_5LocalINS_16FunctionTemplateEEE"]
pub unsafe extern "C" fn v8_object_template_new_abi(
    _isolate: *mut c_void,
    _constructor: *mut c_void,
) -> *mut c_void {
    old_v8_with_state(std::ptr::null_mut(), |state| {
        state.local_handle(OldV8Cell::ObjectTemplate(OldV8ObjectTemplate::new()))
    })
}

#[export_name = "_ZN2v814ObjectTemplate21SetInternalFieldCountEi"]
pub unsafe extern "C" fn v8_object_template_set_internal_field_count_abi(
    this: *mut c_void,
    count: i32,
) {
    if let Some(OldV8Cell::ObjectTemplate(tpl)) = old_v8_resolve_cell(this).and_then(|p| p.as_mut())
    {
        tpl.internal_field_count = count.max(0);
    }
}

#[export_name = "_ZN2v88Template3SetENS_5LocalINS_4NameEEENS1_INS_4DataEEENS_17PropertyAttributeE"]
pub unsafe extern "C" fn v8_template_set_abi(
    this: *mut c_void,
    name: *mut c_void,
    data: *mut c_void,
    _attrs: i32,
) {
    let Some(key) = old_v8_cell_string(name.cast::<OldV8Cell>()) else {
        return;
    };
    if let Some(OldV8Cell::ObjectTemplate(tpl)) = old_v8_resolve_cell(this).and_then(|p| p.as_mut())
    {
        tpl.data_properties.push((key, data.cast::<OldV8Cell>()));
    }
}

#[export_name = "_ZN2v88Template21SetNativeDataPropertyENS_5LocalINS_4NameEEEPFvS3_RKNS_20PropertyCallbackInfoINS_5ValueEEEEPFvS3_NS1_IS5_EERKNS4_IvEEESB_NS_17PropertyAttributeENS_14SideEffectTypeESI_"]
pub unsafe extern "C" fn v8_template_set_native_data_property_abi(
    this: *mut c_void,
    name: *mut c_void,
    getter: Option<OldV8AccessorGetter>,
    _setter: *mut c_void,
    data: *mut c_void,
    _attrs: i32,
    _getter_side_effect: i32,
    _setter_side_effect: i32,
) {
    let Some(key) = old_v8_cell_string(name.cast::<OldV8Cell>()) else {
        return;
    };
    if let Some(OldV8Cell::ObjectTemplate(tpl)) = old_v8_resolve_cell(this).and_then(|p| p.as_mut())
    {
        let data = old_v8_cell_value(data.cast::<OldV8Cell>());
        let mut getter_is_nan = false;
        let getter = data
            .as_ref()
            .and_then(|value| {
                let Value::Object(id) = value else {
                    return None;
                };
                let field = old_v8_with_state(Value::Undefined, |state| {
                    unsafe { state.rt_mut() }.object_get(*id, &old_v8_internal_field_key(1))
                });
                match field {
                    Value::Number(ptr) if ptr != 0.0 => {
                        getter_is_nan = true;
                        Some(unsafe {
                            std::mem::transmute::<usize, OldV8AccessorGetter>(ptr as usize)
                        })
                    }
                    _ => None,
                }
            })
            .or(getter);
        let data = data.and_then(|value| match value {
            Value::Object(id) => {
                let field = old_v8_with_state(Value::Undefined, |state| {
                    unsafe { state.rt_mut() }.object_get(id, &old_v8_internal_field_key(0))
                });
                if matches!(field, Value::Undefined) {
                    None
                } else {
                    Some(field)
                }
            }
            other => Some(other),
        });
        tpl.accessors.push(OldV8AccessorDecl {
            name: key,
            getter,
            getter_is_nan,
            data,
        });
    }
}

#[export_name = "_ZN2v816FunctionTemplate11GetFunctionENS_5LocalINS_7ContextEEE"]
pub unsafe extern "C" fn v8_function_template_get_function_abi(
    this: *mut c_void,
    _context: *mut c_void,
) -> *mut c_void {
    old_v8_with_state(std::ptr::null_mut(), |state| {
        let Some(OldV8Cell::FunctionTemplate(tpl)) =
            old_v8_resolve_cell_in_state(state, this).and_then(|p| unsafe { p.as_ref() })
        else {
            return std::ptr::null_mut();
        };
        let tpl = tpl.clone();
        let rt = unsafe { &mut *state.rt };
        let name = tpl.class_name.clone().unwrap_or_default();
        let callback = tpl.callback;
        let callback_is_nan = tpl.callback_is_nan;
        let data = tpl.data.clone();
        let internal_field_count = match tpl.instance_template.as_ref() {
            Some(OldV8Cell::ObjectTemplate(instance_tpl)) => instance_tpl.internal_field_count,
            _ => 0,
        };
        let accessors = match tpl.instance_template.as_ref() {
            Some(OldV8Cell::ObjectTemplate(instance_tpl)) => instance_tpl.accessors.clone(),
            _ => Vec::new(),
        };
        let ctor_value = old_v8_make_native_method(
            rt,
            name.clone(),
            callback,
            callback_is_nan,
            data,
            internal_field_count,
            accessors,
        );
        let Value::Object(ctor_id) = ctor_value else {
            return std::ptr::null_mut();
        };
        let proto_id = rt.alloc_object(Object::new_ordinary());
        rt.obj_mut(ctor_id)
            .set_own_frozen("prototype".into(), Value::Object(proto_id));
        rt.obj_mut(proto_id)
            .set_own_internal("constructor".into(), Value::Object(ctor_id));
        if let Some(OldV8Cell::ObjectTemplate(proto_tpl)) = tpl.prototype_template.as_ref() {
            for (key, value_cell) in &proto_tpl.data_properties {
                let value =
                    match old_v8_resolve_cell_in_state(state, (*value_cell).cast::<c_void>())
                        .and_then(|cell| unsafe { cell.as_ref() })
                    {
                        Some(OldV8Cell::Value(v)) => v.clone(),
                        Some(OldV8Cell::FunctionTemplate(method_tpl)) => old_v8_make_native_method(
                            rt,
                            key.clone(),
                            method_tpl.callback,
                            method_tpl.callback_is_nan,
                            method_tpl.data.clone(),
                            0,
                            Vec::new(),
                        ),
                        _ => Value::Undefined,
                    };
                rt.object_set(proto_id, key.clone(), value);
            }
        }
        if let Some(OldV8Cell::ObjectTemplate(instance_tpl)) = tpl.instance_template.as_ref() {
            rt.obj_mut(ctor_id).set_own_internal(
                "__old_v8_internal_field_count".into(),
                Value::Number(instance_tpl.internal_field_count as f64),
            );
            let accessor_names = instance_tpl
                .accessors
                .iter()
                .map(|a| a.name.clone())
                .collect::<Vec<_>>()
                .join(",");
            rt.obj_mut(ctor_id).set_own_internal(
                "__old_v8_accessor_names".into(),
                Value::String(Rc::new(crate::value::JsString::from(accessor_names))),
            );
        }
        state.local_handle(OldV8Cell::Value(Value::Object(ctor_id)))
    })
}

#[export_name = "_ZN2v89Signature3NewEPNS_7IsolateENS_5LocalINS_16FunctionTemplateEEE"]
pub unsafe extern "C" fn v8_signature_new_abi(
    _isolate: *mut c_void,
    receiver: *mut c_void,
) -> *mut c_void {
    receiver
}

#[export_name = "_ZN2v814ObjectTemplate11NewInstanceENS_5LocalINS_7ContextEEE"]
pub unsafe extern "C" fn v8_object_template_new_instance_abi(
    this: *mut c_void,
    _context: *mut c_void,
) -> *mut c_void {
    old_v8_with_state(std::ptr::null_mut(), |state| {
        let internal_field_count =
            match old_v8_resolve_cell_in_state(state, this).and_then(|p| unsafe { p.as_ref() }) {
                Some(OldV8Cell::ObjectTemplate(tpl)) => tpl.internal_field_count,
                _ => 0,
            };
        let rt = unsafe { state.rt_mut() };
        let id = rt.alloc_object(Object::new_ordinary());
        if internal_field_count > 0 {
            rt.obj_mut(id).set_own_internal(
                "__old_v8_internal_field_count".into(),
                Value::Number(internal_field_count as f64),
            );
        }
        state.local_handle(OldV8Cell::Value(Value::Object(id)))
    })
}

#[export_name = "_ZN2v86Object3SetENS_5LocalINS_7ContextEEENS1_INS_5ValueEEES5_"]
pub unsafe extern "C" fn v8_object_set_abi(
    this: *mut c_void,
    _context: *mut c_void,
    key: *mut c_void,
    value: *mut c_void,
) -> u16 {
    let Some(key) = old_v8_cell_string(key.cast::<OldV8Cell>()) else {
        return 0;
    };
    let value = old_v8_cell_value(value.cast::<OldV8Cell>()).unwrap_or(Value::Undefined);
    old_v8_with_state(0, |state| {
        let Some(OldV8Cell::Value(Value::Object(id))) =
            old_v8_resolve_cell_in_state(state, this).and_then(|p| unsafe { p.as_ref() })
        else {
            return 0;
        };
        unsafe { state.rt_mut() }.object_set(*id, key, value);
        old_v8_truthy_maybe_bool()
    })
}

#[export_name = "_ZN2v86Object17DefineOwnPropertyENS_5LocalINS_7ContextEEENS1_INS_4NameEEENS1_INS_5ValueEEENS_17PropertyAttributeE"]
pub unsafe extern "C" fn v8_object_define_own_property_abi(
    this: *mut c_void,
    _context: *mut c_void,
    key: *mut c_void,
    value: *mut c_void,
    _attrs: i32,
) -> u16 {
    v8_object_set_abi(this, _context, key, value)
}

#[export_name = "_ZN2v86Object3GetENS_5LocalINS_7ContextEEENS1_INS_5ValueEEE"]
pub unsafe extern "C" fn v8_object_get_abi(
    this: *mut c_void,
    _context: *mut c_void,
    key: *mut c_void,
) -> *mut c_void {
    let Some(key) = old_v8_cell_string(key.cast::<OldV8Cell>()) else {
        return std::ptr::null_mut();
    };
    old_v8_with_state(std::ptr::null_mut(), |state| {
        let Some(OldV8Cell::Value(Value::Object(id))) =
            old_v8_resolve_cell_in_state(state, this).and_then(|p| unsafe { p.as_ref() })
        else {
            return std::ptr::null_mut();
        };
        let value = unsafe { state.rt_mut() }.object_get(*id, &key);
        state.local_handle(OldV8Cell::Value(value))
    })
}

#[export_name = "_ZN2v86Object16SetInternalFieldEiNS_5LocalINS_4DataEEE"]
pub unsafe extern "C" fn v8_object_set_internal_field_abi(
    this: *mut c_void,
    index: i32,
    value: *mut c_void,
) {
    let value = old_v8_cell_value(value.cast::<OldV8Cell>()).unwrap_or(Value::Undefined);
    old_v8_with_state((), |state| {
        let Some(OldV8Cell::Value(Value::Object(id))) =
            old_v8_resolve_cell_in_state(state, this).and_then(|p| unsafe { p.as_ref() })
        else {
            return;
        };
        unsafe { state.rt_mut() }
            .obj_mut(*id)
            .set_own_internal(old_v8_internal_field_key(index), value);
    })
}

#[export_name = "_ZN2v86Object20SlowGetInternalFieldEi"]
pub unsafe extern "C" fn v8_object_slow_get_internal_field_abi(
    this: *mut c_void,
    index: i32,
) -> *mut c_void {
    old_v8_with_state(std::ptr::null_mut(), |state| {
        let Some(OldV8Cell::Value(Value::Object(id))) =
            old_v8_resolve_cell_in_state(state, this).and_then(|p| unsafe { p.as_ref() })
        else {
            return std::ptr::null_mut();
        };
        let value = unsafe { state.rt_mut() }.object_get(*id, &old_v8_internal_field_key(index));
        state.local_handle(OldV8Cell::Value(value))
    })
}

#[export_name = "_ZN2v86Object32SetAlignedPointerInInternalFieldEiPvt"]
pub unsafe extern "C" fn v8_object_set_aligned_pointer_in_internal_field_abi(
    this: *mut c_void,
    index: i32,
    value: *mut c_void,
    _tag: u16,
) {
    old_v8_with_state((), |state| {
        let Some(OldV8Cell::Value(Value::Object(id))) =
            old_v8_resolve_cell_in_state(state, this).and_then(|p| unsafe { p.as_ref() })
        else {
            return;
        };
        unsafe { state.rt_mut() }.obj_mut(*id).set_own_internal(
            old_v8_aligned_internal_field_key(index),
            Value::Number(value as usize as f64),
        );
    })
}

#[export_name = "_ZN2v86Object38SlowGetAlignedPointerFromInternalFieldEit"]
pub unsafe extern "C" fn v8_object_slow_get_aligned_pointer_from_internal_field_abi(
    this: *mut c_void,
    index: i32,
    _tag: u16,
) -> *mut c_void {
    if let Some(fake_tagged) = old_v8_active_fake_object_from_handle(this) {
        return old_v8_fake_object_aligned_field(fake_tagged, index);
    }
    old_v8_with_state(std::ptr::null_mut(), |state| {
        let Some(OldV8Cell::Value(Value::Object(id))) =
            old_v8_resolve_cell_in_state(state, this).and_then(|p| unsafe { p.as_ref() })
        else {
            return std::ptr::null_mut();
        };
        match unsafe { state.rt_mut() }.object_get(*id, &old_v8_aligned_internal_field_key(index)) {
            Value::Number(n) => n as usize as *mut c_void,
            _ => std::ptr::null_mut(),
        }
    })
}

#[export_name = "_ZNK2v86Object18InternalFieldCountEv"]
pub unsafe extern "C" fn v8_object_internal_field_count_abi(this: *mut c_void) -> i32 {
    if old_v8_active_fake_object_from_handle(this).is_some() {
        return 1;
    }
    old_v8_with_state(0, |state| {
        let Some(OldV8Cell::Value(Value::Object(id))) =
            old_v8_resolve_cell(this).and_then(|p| p.as_ref())
        else {
            return 0;
        };
        let value = unsafe { state.rt_mut() }.object_get(*id, "__old_v8_internal_field_count");
        match value {
            Value::Number(n) => n as i32,
            _ => 0,
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_typedarray_info(
    env: napi_env,
    value: napi_value,
    type_: *mut napi_typedarray_type,
    length: *mut usize,
    data: *mut *mut c_void,
    arraybuffer: *mut napi_value,
    byte_offset: *mut usize,
) -> napi_status {
    napi_get_typedarray_info__impl(env, value, type_, length, data, arraybuffer, byte_offset)
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_typedarray(
    env: napi_env,
    type_: napi_typedarray_type,
    length: usize,
    arraybuffer: napi_value,
    byte_offset: usize,
    result: *mut napi_value,
) -> napi_status {
    napi_create_typedarray__impl(env, type_, length, arraybuffer, byte_offset, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_external_buffer(
    env: napi_env,
    length: usize,
    data: *mut c_void,
    finalize_cb: *mut c_void,
    finalize_hint: *mut c_void,
    result: *mut napi_value,
) -> napi_status {
    napi_create_external_buffer__impl(env, length, data, finalize_cb, finalize_hint, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_set_instance_data(
    env: napi_env,
    data: *mut c_void,
    finalize_cb: Option<napi_finalize>,
    finalize_hint: *mut c_void,
) -> napi_status {
    napi_set_instance_data__impl(env, data, finalize_cb, finalize_hint)
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_instance_data(
    env: napi_env,
    data: *mut *mut c_void,
) -> napi_status {
    napi_get_instance_data__impl(env, data)
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_uv_event_loop(
    env: napi_env,
    loop_: *mut *mut c_void,
) -> napi_status {
    napi_get_uv_event_loop__impl(env, loop_)
}

#[no_mangle]
pub unsafe extern "C" fn napi_fatal_error(
    location: *const c_char,
    location_len: usize,
    message: *const c_char,
    message_len: usize,
) {
    let slice_to_string = |ptr: *const c_char, len: usize| -> String {
        if ptr.is_null() {
            return String::new();
        }
        let bytes = std::slice::from_raw_parts(ptr as *const u8, len);
        String::from_utf8_lossy(bytes).into_owned()
    };
    let loc = slice_to_string(location, location_len);
    let msg = slice_to_string(message, message_len);
    if loc.is_empty() {
        panic!("napi_fatal_error: {msg}");
    }
    if msg.is_empty() {
        panic!("napi_fatal_error: {loc}");
    }
    panic!("napi_fatal_error: {loc}: {msg}");
}

#[no_mangle]
pub unsafe extern "C" fn napi_module_register(module: *mut napi_module) {
    if module.is_null() {
        return;
    }
    let Some(register) = (*module).nm_register_func else {
        return;
    };
    if let Ok(mut pending) = PENDING_NAPI_MODULE_REGISTER.lock() {
        *pending = Some(register as usize);
    }
}

unsafe fn napi_define_class__impl(
    env: napi_env,
    utf8name: *const c_char,
    length: usize,
    ctor: Option<napi_callback>,
    data: *mut c_void,
    property_count: usize,
    properties: *const napi_property_descriptor,
    result: *mut napi_value,
) -> napi_status {
    let _ = length;
    if result.is_null() {
        return napi_invalid_arg;
    }
    let ctor = match ctor {
        Some(f) => f,
        None => return napi_invalid_arg,
    };
    let env_ref = env_mut!(env);
    let name = if utf8name.is_null() {
        "".into()
    } else {
        CStr::from_ptr(utf8name).to_string_lossy().into_owned()
    };

    let env_ptr = env;
    let storage = std::rc::Rc::new(NapiCallbackStorage {
        cb: ctor,
        data,
        env: env_ptr,
    });
    let storage2 = storage.clone();
    let native: NativeFn = std::rc::Rc::new(move |rt, args| {
        let env = unsafe { &mut *storage2.env };
        let scope_start = env.handles.len();
        let info = NapiCallbackInfo {
            this: rt.current_this(),
            args: args.to_vec(),
            data: storage2.data,
            new_target: rt.current_new_target.clone(),
        };
        let info_box = Box::into_raw(Box::new(info));
        let ret_handle = unsafe { (storage2.cb)(storage2.env, info_box) };
        let _ = unsafe { Box::from_raw(info_box) };
        if let Some(exc) = env.pending_exception.take() {
            env.handles.truncate(scope_start);
            return Err(crate::RuntimeError::Thrown(exc));
        }
        let v = env
            .get_handle(ret_handle)
            .cloned()
            .unwrap_or(rt.current_this());
        let v = canonicalize_napi_return_buffer_like(rt, v);
        env.handles.truncate(scope_start);
        Ok(v)
    });
    let rt = &mut *env_ref.rt;
    let ctor_obj = crate::intrinsics::make_native(&name, move |rt, args| native(rt, args));
    let ctor_id = rt.alloc_object(ctor_obj);

    let proto_id = rt.alloc_object(Object::new_ordinary());
    rt.obj_mut(ctor_id)
        .set_own_frozen("prototype".into(), Value::Object(proto_id));
    rt.obj_mut(proto_id)
        .set_own_internal("constructor".into(), Value::Object(ctor_id));

    for i in 0..property_count {
        let d = &*properties.add(i);
        let prop_name = if !d.utf8name.is_null() {
            CStr::from_ptr(d.utf8name).to_string_lossy().into_owned()
        } else {
            continue;
        };
        let desc = napi_property_descriptor_value(env, d, &prop_name);
        let rt2 = &mut *env_ref.rt;
        if d.attributes & NAPI_STATIC != 0 {
            rt2.obj_mut(ctor_id).insert_str(prop_name, desc);
        } else {
            rt2.obj_mut(proto_id).insert_str(prop_name, desc);
        }
    }
    let _ = storage;
    *result = env_ref.push_handle(Value::Object(ctor_id));
    napi_ok
}

unsafe fn napi_wrap__impl(
    env: napi_env,
    object: napi_value,
    native: *mut c_void,
    finalize_cb: *mut c_void,
    finalize_hint: *mut c_void,
    result: *mut napi_ref,
) -> napi_status {
    let env_ref = env_mut!(env);
    let id = match env_ref.get_handle(object) {
        Some(Value::Object(id)) => *id,
        _ => return napi_object_expected,
    };
    let finalizer = if finalize_cb.is_null() {
        None
    } else {
        Some(std::mem::transmute::<*mut c_void, napi_finalize>(
            finalize_cb,
        ))
    };
    if let Some(previous) = env_ref.wrapped.insert(
        id.0,
        NapiWrappedNative {
            native: native as usize,
            finalizer,
            hint: finalize_hint as usize,
        },
    ) {
        if let Some(finalizer) = previous.finalizer {
            finalizer(
                env,
                previous.native as *mut c_void,
                previous.hint as *mut c_void,
            );
        }
    }
    if !result.is_null() {

        let slot = env_ref.refs.len();
        env_ref.refs.push(Some(Value::Object(id)));
        let handle = Box::into_raw(Box::new(NapiRefHandle {
            slot,
            env,
            count: 1,
        }));
        *result = handle;
    }
    napi_ok
}

unsafe fn napi_unwrap__impl(
    env: napi_env,
    object: napi_value,
    result: *mut *mut c_void,
) -> napi_status {
    if result.is_null() {
        return napi_invalid_arg;
    }
    let env_ref = env_mut!(env);
    let id = match env_ref.get_handle(object) {
        Some(Value::Object(id)) => *id,
        _ => return napi_object_expected,
    };
    *result = env_ref
        .wrapped
        .get(&id.0)
        .map(|entry| entry.native as *mut c_void)
        .unwrap_or(std::ptr::null_mut());
    napi_ok
}

unsafe fn napi_remove_wrap__impl(
    env: napi_env,
    object: napi_value,
    result: *mut *mut c_void,
) -> napi_status {
    if result.is_null() {
        return napi_invalid_arg;
    }
    let env_ref = env_mut!(env);
    let id = match env_ref.get_handle(object) {
        Some(Value::Object(id)) => *id,
        _ => return napi_object_expected,
    };
    *result = match env_ref.wrapped.remove(&id.0) {
        Some(entry) => {
            if let Some(finalizer) = entry.finalizer {
                finalizer(env, entry.native as *mut c_void, entry.hint as *mut c_void);
            }
            entry.native as *mut c_void
        }
        None => std::ptr::null_mut(),
    };
    napi_ok
}

unsafe fn napi_get_version__impl(_env: napi_env, result: *mut u32) -> napi_status {
    if result.is_null() {
        return napi_invalid_arg;
    }
    *result = 8;
    napi_ok
}

unsafe fn napi_get_node_version__impl(_env: napi_env, result: *mut *const c_void) -> napi_status {

    static VERSION: [u32; 3] = [20, 10, 0];
    if !result.is_null() {
        *result = &VERSION as *const _ as *const c_void;
    }
    napi_ok
}

pub type NapiMainJob = crate::interp::HostCompletionJob;

struct SendPtr<T>(*mut T);
unsafe impl<T> Send for SendPtr<T> {}

impl<T> Copy for SendPtr<T> {}
impl<T> Clone for SendPtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}

struct SendFn<F>(F);
unsafe impl<F> Send for SendFn<F> {}
impl<F: Copy> Copy for SendFn<F> {}
impl<F: Copy> Clone for SendFn<F> {
    fn clone(&self) -> Self {
        *self
    }
}

#[repr(C)]
pub struct NapiAsyncWork {
    execute: unsafe extern "C" fn(env: napi_env, data: *mut c_void),
    complete: unsafe extern "C" fn(env: napi_env, status: napi_status, data: *mut c_void),
    data: SendPtr<c_void>,
    env: SendPtr<NapiEnv>,

    queued: bool,
}

unsafe fn napi_create_async_work__impl(
    env: napi_env,
    _async_resource: napi_value,
    _async_resource_name: napi_value,
    execute: Option<unsafe extern "C" fn(env: napi_env, data: *mut c_void)>,
    complete: Option<unsafe extern "C" fn(env: napi_env, status: napi_status, data: *mut c_void)>,
    data: *mut c_void,
    result: *mut *mut NapiAsyncWork,
) -> napi_status {
    if result.is_null() {
        return napi_invalid_arg;
    }
    let execute = match execute {
        Some(f) => f,
        None => return napi_invalid_arg,
    };
    let complete = match complete {
        Some(f) => f,
        None => return napi_invalid_arg,
    };
    let work = Box::new(NapiAsyncWork {
        execute,
        complete,
        data: SendPtr(data),
        env: SendPtr(env),
        queued: false,
    });
    *result = Box::into_raw(work);
    napi_ok
}

unsafe fn napi_queue_async_work__impl(env: napi_env, work: *mut NapiAsyncWork) -> napi_status {
    if work.is_null() {
        return napi_invalid_arg;
    }
    if env.is_null() {
        return napi_invalid_arg;
    }
    let env_ref = &mut *env;
    let rt = &mut *env_ref.rt;
    let inbox = rt.host_completion_inbox.clone();
    let keepalive = rt.napi_keepalive.clone();
    let wake = rt.agent_wake_handle();
    let w = &mut *work;
    if w.queued {
        return napi_generic_failure;
    }
    w.queued = true;

    let prev_keepalive = keepalive.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    if std::env::var_os("CRUFT_NAPI_TRACE_KEEPALIVE").is_some() {
        eprintln!(
            "[cruft-napi-keepalive:async-work:queue] work={:p} prev={} next={}",
            work,
            prev_keepalive,
            prev_keepalive + 1
        );
    }
    let execute = SendFn(w.execute);
    let complete = SendFn(w.complete);
    let data: SendPtr<c_void> = w.data;
    let env_send: SendPtr<NapiEnv> = w.env;
    let work_ptr = SendPtr(work);

    let keepalive_for_thread = keepalive.clone();
    let wake_for_thread = wake.clone();
    std::thread::spawn(move || {
        let execute_local = execute;
        let env_local = env_send;
        let data_local = data;
        let complete_local = complete;
        let work_local = work_ptr;
        let keepalive = keepalive_for_thread;
        let status: napi_status = {
            unsafe {
                (execute_local.0)(env_local.0, data_local.0);
            }
            napi_ok
        };
        let keepalive_for_job = keepalive.clone();
        let job: NapiMainJob = Box::new(move |_rt: &mut Runtime| {
            let complete2 = complete_local;
            let env2 = env_local;
            let data2 = data_local;
            let work2 = work_local;
            let ka = keepalive_for_job;
            unsafe {

                let w = &mut *work2.0;
                w.queued = false;
                (complete2.0)(env2.0, status, data2.0);
            }
            let prev_keepalive = ka.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            if std::env::var_os("CRUFT_NAPI_TRACE_KEEPALIVE").is_some() {
                eprintln!(
                    "[cruft-napi-keepalive:async-work:complete] work={:p} prev={} next={}",
                    work2.0,
                    prev_keepalive,
                    prev_keepalive.saturating_sub(1)
                );
            }
        });
        Runtime::enqueue_host_completion_to(&inbox, job);
        let (lock, cv) = &*wake_for_thread;
        if let Ok(mut generation) = lock.lock() {
            *generation = generation.wrapping_add(1);
            cv.notify_all();
        }
    });
    napi_ok
}

unsafe fn napi_delete_async_work__impl(_env: napi_env, work: *mut NapiAsyncWork) -> napi_status {
    if work.is_null() {
        return napi_invalid_arg;
    }
    let w = &*work;
    if w.queued {
        return napi_generic_failure;
    }
    let _ = Box::from_raw(work);
    napi_ok
}

unsafe fn napi_cancel_async_work__impl(_env: napi_env, _work: *mut NapiAsyncWork) -> napi_status {
    napi_generic_failure
}

#[repr(i32)]
pub enum napi_threadsafe_function_call_mode {
    napi_tsfn_nonblocking = 0,
    napi_tsfn_blocking = 1,
}

#[repr(i32)]
pub enum napi_threadsafe_function_release_mode {
    napi_tsfn_release = 0,
    napi_tsfn_abort = 1,
}

pub type napi_threadsafe_function = *mut NapiTsfn;
pub type napi_threadsafe_function_call_js = unsafe extern "C" fn(
    env: napi_env,
    js_callback: napi_value,
    context: *mut c_void,
    data: *mut c_void,
);

pub struct NapiTsfn {
    func_ref_slot: usize,
    debug_name: String,

    has_func: bool,
    call_js: Option<napi_threadsafe_function_call_js>,
    context: SendPtr<c_void>,
    env: SendPtr<NapiEnv>,
    ref_count: std::sync::atomic::AtomicUsize,
    active: std::sync::atomic::AtomicBool,

    keepalive_active: std::sync::atomic::AtomicBool,

    keepalive_counter: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

unsafe fn napi_create_threadsafe_function__impl(
    env: napi_env,
    func: napi_value,
    _async_resource: napi_value,
    async_resource_name: napi_value,
    _max_queue_size: usize,
    initial_thread_count: usize,
    _thread_finalize_data: *mut c_void,
    _thread_finalize_cb: *mut c_void,
    context: *mut c_void,
    call_js_cb: Option<napi_threadsafe_function_call_js>,
    result: *mut napi_threadsafe_function,
) -> napi_status {
    if result.is_null() {
        return napi_invalid_arg;
    }
    let env_ref = env_mut!(env);

    let func_opt = env_ref.get_handle(func).cloned();
    let has_func = func_opt.is_some();
    if !has_func && call_js_cb.is_none() {
        return napi_invalid_arg;
    }
    let slot = env_ref.refs.len();
    env_ref.refs.push(func_opt);
    let debug_name = match env_ref.get_handle(async_resource_name) {
        Some(Value::String(s)) => s.as_str().to_string(),
        Some(other) => format!("{other:?}"),
        None => "<unnamed>".to_string(),
    };
    let keepalive_counter = (&*env_ref.rt).napi_keepalive.clone();

    let prev_keepalive = keepalive_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    let initial_acquisitions = initial_thread_count.max(1);
    let tsfn = Box::new(NapiTsfn {
        func_ref_slot: slot,
        debug_name,
        has_func,
        call_js: call_js_cb,
        context: SendPtr(context),
        env: SendPtr(env),
        ref_count: std::sync::atomic::AtomicUsize::new(initial_acquisitions),
        active: std::sync::atomic::AtomicBool::new(true),
        keepalive_active: std::sync::atomic::AtomicBool::new(true),
        keepalive_counter,
    });
    if std::env::var_os("CRUFT_NAPI_TRACE_KEEPALIVE").is_some() {
        eprintln!(
            "[cruft-napi-keepalive:tsfn:create] name={} initial_refs={} prev={} next={}",
            tsfn.debug_name,
            initial_acquisitions,
            prev_keepalive,
            prev_keepalive + 1
        );
    }
    *result = Box::into_raw(tsfn);
    napi_ok
}

unsafe fn napi_call_threadsafe_function__impl(
    tsfn: napi_threadsafe_function,
    data: *mut c_void,
    _mode: napi_threadsafe_function_call_mode,
) -> napi_status {
    if tsfn.is_null() {
        return napi_invalid_arg;
    }
    let tsfn_ref = &*tsfn;
    if !tsfn_ref.active.load(std::sync::atomic::Ordering::SeqCst) {
        return napi_generic_failure;
    }
    let env_send = tsfn_ref.env;
    let env_ref = &mut *env_send.0;
    let rt = &*env_ref.rt;
    let inbox = rt.host_completion_inbox.clone();
    let wake = rt.agent_wake_handle();
    let func_slot = tsfn_ref.func_ref_slot;
    let has_func = tsfn_ref.has_func;
    let context = tsfn_ref.context;
    let data_send = SendPtr(data);
    let call_js = tsfn_ref.call_js.map(SendFn);
    if std::env::var_os("CRUFT_NAPI_TRACE_TSFN").is_some() {
        eprintln!(
            "[cruft-napi-tsfn:call] name={} slot={} has_func={} context={:p} data={:p}",
            tsfn_ref.debug_name, func_slot, has_func, context.0, data
        );
    }
    let env_for_job = env_send;
    let context_for_job = context;
    let data_for_job = data_send;
    let call_js_for_job = call_js;
    let job: NapiMainJob = Box::new(move |_rt: &mut Runtime| {
        let env_local = env_for_job;
        let ctx_local = context_for_job;
        let data_local = data_for_job;
        let cb_local = call_js_for_job;
        let env_ref = unsafe { &mut *env_local.0 };

        let func_handle = if has_func {
            match env_ref.refs.get(func_slot).and_then(|o| o.clone()) {
                Some(v) => env_ref.push_handle(v),
                None => return,
            }
        } else {
            std::ptr::null_mut()
        };
        if let Some(cb) = cb_local {
            unsafe {
                (cb.0)(env_local.0, func_handle, ctx_local.0, data_local.0);
            }
        }
    });
    Runtime::enqueue_host_completion_to(&inbox, job);
    let (lock, cv) = &*wake;
    if let Ok(mut generation) = lock.lock() {
        *generation = generation.wrapping_add(1);
        cv.notify_all();
    }
    napi_ok
}

unsafe fn napi_acquire_threadsafe_function__impl(tsfn: napi_threadsafe_function) -> napi_status {
    if tsfn.is_null() {
        return napi_invalid_arg;
    }
    let t = &*tsfn;
    let prev = t
        .ref_count
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    if std::env::var_os("CRUFT_NAPI_TRACE_TSFN").is_some() {
        eprintln!(
            "[cruft-napi-tsfn:acquire] name={} prev={} next={}",
            t.debug_name,
            prev,
            prev + 1
        );
    }
    napi_ok
}

unsafe fn napi_release_threadsafe_function__impl(
    tsfn: napi_threadsafe_function,
    _mode: napi_threadsafe_function_release_mode,
) -> napi_status {
    if tsfn.is_null() {
        return napi_invalid_arg;
    }
    let t = &*tsfn;
    let prev = t
        .ref_count
        .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    if std::env::var_os("CRUFT_NAPI_TRACE_TSFN").is_some() {
        eprintln!(
            "[cruft-napi-tsfn:release] name={} prev={} next={}",
            t.debug_name,
            prev,
            prev.saturating_sub(1)
        );
    }
    if prev == 1 {

        t.active.store(false, std::sync::atomic::Ordering::SeqCst);
        if t.keepalive_active
            .compare_exchange(
                true,
                false,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_ok()
        {
            let prev_keepalive = t
                .keepalive_counter
                .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            if std::env::var_os("CRUFT_NAPI_TRACE_KEEPALIVE").is_some() {
                eprintln!(
                    "[cruft-napi-keepalive:tsfn:release-last] name={} prev={} next={}",
                    t.debug_name,
                    prev_keepalive,
                    prev_keepalive.saturating_sub(1)
                );
            }
        }
    }
    napi_ok
}

unsafe fn napi_ref_threadsafe_function__impl(
    _env: napi_env,
    tsfn: napi_threadsafe_function,
) -> napi_status {
    if tsfn.is_null() {
        return napi_invalid_arg;
    }
    let t = &*tsfn;

    if t.keepalive_active
        .compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        )
        .is_ok()
    {
        let prev_keepalive = t
            .keepalive_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if std::env::var_os("CRUFT_NAPI_TRACE_KEEPALIVE").is_some() {
            eprintln!(
                "[cruft-napi-keepalive:tsfn:ref] name={} prev={} next={}",
                t.debug_name,
                prev_keepalive,
                prev_keepalive + 1
            );
        }
        if std::env::var_os("CRUFT_NAPI_TRACE_TSFN").is_some() {
            eprintln!("[cruft-napi-tsfn:ref] name={}", t.debug_name);
        }
    }
    napi_ok
}

unsafe fn napi_unref_threadsafe_function__impl(
    _env: napi_env,
    tsfn: napi_threadsafe_function,
) -> napi_status {
    if tsfn.is_null() {
        return napi_invalid_arg;
    }
    let t = &*tsfn;
    if t.keepalive_active
        .compare_exchange(
            true,
            false,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        )
        .is_ok()
    {
        let prev_keepalive = t
            .keepalive_counter
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        if std::env::var_os("CRUFT_NAPI_TRACE_KEEPALIVE").is_some() {
            eprintln!(
                "[cruft-napi-keepalive:tsfn:unref] name={} prev={} next={}",
                t.debug_name,
                prev_keepalive,
                prev_keepalive.saturating_sub(1)
            );
        }
        if std::env::var_os("CRUFT_NAPI_TRACE_TSFN").is_some() {
            eprintln!("[cruft-napi-tsfn:unref] name={}", t.debug_name);
        }
    }
    napi_ok
}

unsafe fn napi_get_threadsafe_function_context__impl(
    tsfn: napi_threadsafe_function,
    result: *mut *mut c_void,
) -> napi_status {
    if tsfn.is_null() || result.is_null() {
        return napi_invalid_arg;
    }
    let t = &*tsfn;
    *result = t.context.0;
    napi_ok
}

pub fn drain_main_inbox(rt: &mut Runtime) -> usize {
    rt.drain_host_completion_inbox()
}

pub fn has_pending(rt: &Runtime) -> bool {
    let keepalive = rt.napi_keepalive.load(std::sync::atomic::Ordering::SeqCst);
    if std::env::var_os("CRUFT_NAPI_TRACE_KEEPALIVE").is_some() && keepalive > 0 {
        let inbox_len = rt
            .host_completion_inbox
            .lock()
            .map(|q| q.len())
            .unwrap_or(0);
        eprintln!(
            "[cruft-napi-keepalive:pending] keepalive={} inbox={}",
            keepalive, inbox_len
        );
    }
    if keepalive > 0 {
        return true;
    }
    rt.has_host_completion_jobs()
}

unsafe fn napi_create_buffer__impl(
    env: napi_env,
    length: usize,
    data: *mut *mut c_void,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_create_buffer");
    create_buffer_from_bytes(env, vec![0u8; length], data, result)
}

#[repr(C)]
pub struct napi_type_tag {
    pub lower: u64,
    pub upper: u64,
}

unsafe fn napi_is_arraybuffer__impl(
    env: napi_env,
    value: napi_value,
    result: *mut bool,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_is_arraybuffer");
    let id = match env.get_handle(value) {
        Some(Value::Object(id)) => *id,
        _ => {
            *result = false;
            return napi_ok;
        }
    };
    *result = (*env.rt).array_buffers.contains_key(&id);
    napi_ok
}

unsafe fn napi_is_typedarray__impl(
    env: napi_env,
    value: napi_value,
    result: *mut bool,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_is_typedarray");
    let id = match env.get_handle(value) {
        Some(Value::Object(id)) => *id,
        _ => {
            *result = false;
            return napi_ok;
        }
    };
    *result = (*env.rt)
        .typed_array_views
        .get(&id)
        .is_some_and(|v| &*v.element_kind != "DataView");
    napi_ok
}

fn napi_typedarray_kind(kind: &str) -> Option<napi_typedarray_type> {
    match kind {
        "Int8Array" => Some(napi_int8_array),
        "Uint8Array" => Some(napi_uint8_array),
        "Uint8ClampedArray" => Some(napi_uint8_clamped_array),
        "Int16Array" => Some(napi_int16_array),
        "Uint16Array" => Some(napi_uint16_array),
        "Int32Array" => Some(napi_int32_array),
        "Uint32Array" => Some(napi_uint32_array),
        "Float32Array" => Some(napi_float32_array),
        "Float64Array" => Some(napi_float64_array),
        "BigInt64Array" => Some(napi_bigint64_array),
        "BigUint64Array" => Some(napi_biguint64_array),
        _ => None,
    }
}

unsafe fn napi_get_typedarray_info__impl(
    env: napi_env,
    value: napi_value,
    type_: *mut napi_typedarray_type,
    length: *mut usize,
    data: *mut *mut c_void,
    arraybuffer: *mut napi_value,
    byte_offset: *mut usize,
) -> napi_status {
    let env = owner_env_mut!(env, "napi_get_typedarray_info");
    let id = match env.get_handle(value) {
        Some(Value::Object(id)) => *id,
        _ => return napi_invalid_arg,
    };
    let (kind, len, buf_id, offset, data_ptr) = {
        let rt = &mut *env.rt;
        let Some(view) = rt.typed_array_views.get(&id) else {
            return napi_invalid_arg;
        };
        if &*view.element_kind == "DataView" {
            return napi_invalid_arg;
        }
        let Some(kind) = napi_typedarray_kind(&*view.element_kind) else {
            return napi_invalid_arg;
        };
        let Some(buf) = rt.array_buffers.get(&view.buffer) else {
            return napi_invalid_arg;
        };
        let ptr = if buf.detached || buf.shared.is_some() || view.byte_offset > buf.data.len() {
            std::ptr::null_mut()
        } else {
            buf.data.as_ptr().add(view.byte_offset) as *mut c_void
        };
        (
            kind,
            view.fixed_length.unwrap_or_else(|| {
                buf.byte_len().saturating_sub(view.byte_offset) / view.bytes_per_element.max(1)
            }),
            view.buffer,
            view.byte_offset,
            ptr,
        )
    };
    if !type_.is_null() {
        *type_ = kind;
    }
    if !length.is_null() {
        *length = len;
    }
    if !data.is_null() {
        *data = data_ptr;
    }
    if !arraybuffer.is_null() {
        *arraybuffer = env.push_handle(Value::Object(buf_id));
    }
    if !byte_offset.is_null() {
        *byte_offset = offset;
    }
    napi_ok
}

unsafe fn napi_is_dataview__impl(
    env: napi_env,
    value: napi_value,
    result: *mut bool,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_is_dataview");
    let id = match env.get_handle(value) {
        Some(Value::Object(id)) => *id,
        _ => {
            *result = false;
            return napi_ok;
        }
    };
    *result = (*env.rt)
        .typed_array_views
        .get(&id)
        .is_some_and(|v| &*v.element_kind == "DataView");
    napi_ok
}

unsafe fn napi_is_date__impl(env: napi_env, value: napi_value, result: *mut bool) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_is_date");
    let id = match env.get_handle(value) {
        Some(Value::Object(id)) => *id,
        _ => {
            *result = false;
            return napi_ok;
        }
    };
    *result = !matches!((*env.rt).object_get(id, "__date_ms"), Value::Undefined);
    napi_ok
}

unsafe fn napi_create_date__impl(env: napi_env, time: f64, result: *mut napi_value) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_create_date");
    let rt = &mut *env.rt;
    let date_proto = match rt.global_get("Date") {
        Value::Object(d) => match rt.object_get(d, "prototype") {
            Value::Object(p) => Some(p),
            _ => None,
        },
        _ => None,
    };
    let id = rt.alloc_object(Object::new_ordinary());
    if let Some(p) = date_proto {
        rt.obj_mut(id).proto = Some(p);
    }
    rt.mark_date_object(id);
    rt.object_set(id, "__date_ms".into(), Value::Number(time));
    *result = env.push_handle(Value::Object(id));
    napi_ok
}

unsafe fn napi_get_date_value__impl(
    env: napi_env,
    value: napi_value,
    result: *mut f64,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_get_date_value");
    let id = match env.get_handle(value) {
        Some(Value::Object(id)) => *id,
        _ => return napi_object_expected,
    };
    *result = match (*env.rt).object_get(id, "__date_ms") {
        Value::Number(n) => n,
        _ => f64::NAN,
    };
    napi_ok
}

unsafe fn napi_create_string_utf16__impl(
    env: napi_env,
    str: *const u16,
    length: usize,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    let s = if str.is_null() {
        String::new()
    } else if length == usize::MAX {
        let mut n = 0usize;
        while *str.add(n) != 0 {
            n += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(str, n))
    } else {
        String::from_utf16_lossy(std::slice::from_raw_parts(str, length))
    };
    *result = env.push_handle(Value::String(Rc::new(crate::value::JsString::from(s))));
    napi_ok
}

unsafe fn napi_type_tag_object__impl(
    env: napi_env,
    value: napi_value,
    type_tag: *const napi_type_tag,
) -> napi_status {
    if type_tag.is_null() {
        return napi_invalid_arg;
    }
    let env = owner_env_mut!(env, "napi_type_tag_object");
    let id = match env.get_handle(value) {
        Some(Value::Object(id)) => *id,
        _ => return napi_object_expected,
    };
    let tag = &*type_tag;
    let rt = &mut *env.rt;
    rt.object_set(
        id,
        "__type_tag".into(),
        Value::String(Rc::new(crate::value::JsString::from(format!(
            "{}:{}",
            tag.lower, tag.upper
        )))),
    );
    napi_ok
}

unsafe fn napi_check_object_type_tag__impl(
    env: napi_env,
    value: napi_value,
    type_tag: *const napi_type_tag,
    result: *mut bool,
) -> napi_status {
    check_arg!(result);
    if type_tag.is_null() {
        return napi_invalid_arg;
    }
    let env = owner_env_mut!(env, "napi_check_object_type_tag");
    let id = match env.get_handle(value) {
        Some(Value::Object(id)) => *id,
        _ => {
            *result = false;
            return napi_ok;
        }
    };
    let tag = &*type_tag;
    let expected = format!("{}:{}", tag.lower, tag.upper);
    *result = match (*env.rt).object_get(id, "__type_tag") {
        Value::String(actual) => actual.as_str() == expected,
        _ => false,
    };
    napi_ok
}

pub fn load_napi_module(rt: &mut Runtime, path: &str) -> Result<Value, crate::RuntimeError> {

    let lib = unsafe { rusty_host_dylib::Library::open(path) }.map_err(|e| {
        crate::RuntimeError::TypeError(format_native_addon_dlopen_error(path, &e.to_string()))
    })?;

    let init_addr: usize = match unsafe { lib.symbol(b"napi_register_module_v1") } {
        Ok(sym) => sym as usize,
        Err(dlsym_err) => match unsafe { lib.symbol(b"node_register_module_v147") } {
            Ok(sym) => {
                return load_old_v8_module_from_symbol(rt, path, lib, sym as usize);
            }
            Err(_) => {
                let pending = PENDING_NAPI_MODULE_REGISTER
                    .lock()
                    .ok()
                    .and_then(|mut p| p.take());
                pending.ok_or_else(|| {
                    crate::RuntimeError::TypeError(format!(
                        "napi: dlsym('napi_register_module_v1') in '{}': {}",
                        path, dlsym_err
                    ))
                })?
            }
        },
    };
    rt.napi_libs.push(lib);
    let init: unsafe extern "C" fn(napi_env, napi_value) -> napi_value =
        unsafe { std::mem::transmute(init_addr) };

    let exports_id = rt.alloc_object(Object::new_ordinary());
    let exports_v = Value::Object(exports_id);
    let mut env_box = NapiEnv::new(rt);
    let env_ptr = &mut *env_box as *mut NapiEnv;
    let exports_handle = env_box.push_handle(exports_v.clone());
    let ret_handle = unsafe { init(env_ptr, exports_handle) };
    if let Some(exc) = env_box.pending_exception.take() {
        return Err(crate::RuntimeError::Thrown(exc));
    }
    let final_exports = env_box.get_handle(ret_handle).cloned().unwrap_or(exports_v);
    let final_exports = canonicalize_napi_return_buffer_like(rt, final_exports);

    rt.napi_envs.push(env_box);
    Ok(final_exports)
}

fn load_old_v8_module_from_symbol(
    rt: &mut Runtime,
    _path: &str,
    lib: rusty_host_dylib::Library,
    init_addr: usize,
) -> Result<Value, crate::RuntimeError> {
    rt.napi_libs.push(lib);
    let exports_id = rt.alloc_object(Object::new_ordinary());
    let exports_v = Value::Object(exports_id);
    let module_id = rt.alloc_object(Object::new_ordinary());
    let mut old_v8_state = Box::new(OldV8State::new(rt));
    OLD_V8_STATE_PTR.store(
        (&mut *old_v8_state as *mut OldV8State) as usize,
        Ordering::SeqCst,
    );
    let exports = old_v8_with_state(std::ptr::null_mut(), |state| {
        state.local_handle(OldV8Cell::Value(exports_v.clone()))
    });
    let module = old_v8_with_state(std::ptr::null_mut(), |state| {
        state.local_handle(OldV8Cell::Value(Value::Object(module_id)))
    });
    let context = old_v8_with_state(std::ptr::null_mut(), |state| {
        state.local_handle_for(state.current_context)
    });
    let init: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) =
        unsafe { std::mem::transmute(init_addr) };
    unsafe {
        init(exports, module, context);
    }
    let result = if let Some(exc) = old_v8_state.pending_exception.take() {
        Err(crate::RuntimeError::Thrown(exc))
    } else {
        Ok(exports_v.clone())
    };
    OLD_V8_STATE_PTR.store(0, Ordering::SeqCst);
    result
}

fn format_native_addon_dlopen_error(path: &str, err: &str) -> String {
    if native_addon_error_mentions_old_v8_abi(err) {
        return format!(
            "native-addon: old V8/NAN ABI required by '{}': {}",
            path, err
        );
    }
    format!("napi: dlopen('{}'): {}", path, err)
}

fn native_addon_error_mentions_old_v8_abi(err: &str) -> bool {
    err.contains("__ZN2v8")
        || err.contains("__ZNK2v8")
        || err.contains("__ZN4node6Buffer")
        || err.contains("v8::")
        || err.contains("node::Buffer")
}

#[repr(transparent)]
pub struct NapiSymPtr(pub *const ());
unsafe impl Sync for NapiSymPtr {}

#[no_mangle]
pub static NAPI_KEEPALIVE: &[NapiSymPtr] = &[
    NapiSymPtr(napi_get_undefined as *const _),
    NapiSymPtr(napi_get_null as *const _),
    NapiSymPtr(napi_get_boolean as *const _),
    NapiSymPtr(napi_get_global as *const _),
    NapiSymPtr(napi_create_int32 as *const _),
    NapiSymPtr(napi_create_uint32 as *const _),
    NapiSymPtr(napi_create_int64 as *const _),
    NapiSymPtr(napi_create_double as *const _),
    NapiSymPtr(napi_get_value_int32 as *const _),
    NapiSymPtr(napi_get_value_uint32 as *const _),
    NapiSymPtr(napi_get_value_int64 as *const _),
    NapiSymPtr(napi_get_value_double as *const _),
    NapiSymPtr(napi_get_value_bool as *const _),
    NapiSymPtr(napi_create_string_utf8 as *const _),
    NapiSymPtr(napi_create_string_latin1 as *const _),
    NapiSymPtr(napi_get_value_string_utf8 as *const _),
    NapiSymPtr(napi_create_object as *const _),
    NapiSymPtr(napi_create_array as *const _),
    NapiSymPtr(napi_create_array_with_length as *const _),
    NapiSymPtr(napi_set_named_property as *const _),
    NapiSymPtr(napi_get_named_property as *const _),
    NapiSymPtr(napi_has_named_property as *const _),
    NapiSymPtr(napi_set_property as *const _),
    NapiSymPtr(napi_get_property as *const _),
    NapiSymPtr(napi_set_element as *const _),
    NapiSymPtr(napi_get_element as *const _),
    NapiSymPtr(napi_get_array_length as *const _),
    NapiSymPtr(napi_typeof as *const _),
    NapiSymPtr(napi_is_array as *const _),
    NapiSymPtr(napi_strict_equals as *const _),
    NapiSymPtr(napi_create_function as *const _),
    NapiSymPtr(napi_get_cb_info as *const _),
    NapiSymPtr(napi_call_function as *const _),
    NapiSymPtr(napi_create_reference as *const _),
    NapiSymPtr(napi_delete_reference as *const _),
    NapiSymPtr(napi_reference_ref as *const _),
    NapiSymPtr(napi_reference_unref as *const _),
    NapiSymPtr(napi_get_reference_value as *const _),
    NapiSymPtr(napi_throw as *const _),
    NapiSymPtr(napi_throw_error as *const _),
    NapiSymPtr(napi_throw_type_error as *const _),
    NapiSymPtr(napi_throw_range_error as *const _),
    NapiSymPtr(napi_is_exception_pending as *const _),
    NapiSymPtr(napi_get_and_clear_last_exception as *const _),
    NapiSymPtr(napi_get_last_error_info as *const _),
    NapiSymPtr(napi_open_handle_scope as *const _),
    NapiSymPtr(napi_close_handle_scope as *const _),
    NapiSymPtr(napi_open_escapable_handle_scope as *const _),
    NapiSymPtr(napi_close_escapable_handle_scope as *const _),
    NapiSymPtr(napi_escape_handle as *const _),
    NapiSymPtr(napi_define_properties as *const _),
    NapiSymPtr(napi_get_version as *const _),
    NapiSymPtr(napi_get_node_version as *const _),
    NapiSymPtr(napi_create_threadsafe_function as *const _),
    NapiSymPtr(napi_call_threadsafe_function as *const _),
    NapiSymPtr(napi_acquire_threadsafe_function as *const _),
    NapiSymPtr(napi_release_threadsafe_function as *const _),
    NapiSymPtr(napi_ref_threadsafe_function as *const _),
    NapiSymPtr(napi_unref_threadsafe_function as *const _),
    NapiSymPtr(napi_get_threadsafe_function_context as *const _),
    NapiSymPtr(napi_create_async_work as *const _),
    NapiSymPtr(napi_queue_async_work as *const _),
    NapiSymPtr(napi_delete_async_work as *const _),
    NapiSymPtr(napi_cancel_async_work as *const _),
    NapiSymPtr(napi_define_class as *const _),
    NapiSymPtr(napi_wrap as *const _),
    NapiSymPtr(napi_unwrap as *const _),
    NapiSymPtr(napi_create_buffer as *const _),
    NapiSymPtr(napi_create_external_buffer as *const _),
    NapiSymPtr(napi_fatal_error as *const _),
    NapiSymPtr(napi_create_error as *const _),
    NapiSymPtr(napi_create_type_error as *const _),
    NapiSymPtr(napi_create_range_error as *const _),
    NapiSymPtr(napi_create_syntax_error as *const _),
    NapiSymPtr(napi_throw_syntax_error as *const _),
    NapiSymPtr(napi_is_error as *const _),
    NapiSymPtr(napi_create_symbol as *const _),
    NapiSymPtr(napi_get_value_string_latin1 as *const _),
    NapiSymPtr(napi_get_value_string_utf16 as *const _),
    NapiSymPtr(napi_create_bigint_int64 as *const _),
    NapiSymPtr(napi_create_bigint_uint64 as *const _),
    NapiSymPtr(napi_create_bigint_words as *const _),
    NapiSymPtr(napi_get_value_bigint_int64 as *const _),
    NapiSymPtr(napi_get_value_bigint_uint64 as *const _),
    NapiSymPtr(napi_get_value_bigint_words as *const _),
    NapiSymPtr(napi_create_arraybuffer as *const _),
    NapiSymPtr(napi_get_arraybuffer_info as *const _),
    NapiSymPtr(napi_detach_arraybuffer as *const _),
    NapiSymPtr(napi_get_typedarray_info as *const _),
    NapiSymPtr(napi_create_typedarray as *const _),
    NapiSymPtr(napi_create_external_buffer as *const _),
    NapiSymPtr(napi_set_instance_data as *const _),
    NapiSymPtr(napi_get_instance_data as *const _),
    NapiSymPtr(napi_get_uv_event_loop as *const _),
    NapiSymPtr(uv_mutex_init as *const _),
    NapiSymPtr(uv_mutex_destroy as *const _),
    NapiSymPtr(uv_mutex_lock as *const _),
    NapiSymPtr(uv_mutex_unlock as *const _),
    NapiSymPtr(uv_async_init as *const _),
    NapiSymPtr(uv_async_send as *const _),
    NapiSymPtr(uv_close as *const _),
    NapiSymPtr(uv_run as *const _),
    NapiSymPtr(uv_default_loop as *const _),
    NapiSymPtr(uv_ref as *const _),
    NapiSymPtr(uv_unref as *const _),
    NapiSymPtr(uv_poll_init as *const _),
    NapiSymPtr(uv_poll_start as *const _),
    NapiSymPtr(uv_poll_stop as *const _),
    NapiSymPtr(uv_strerror as *const _),
    NapiSymPtr(napi_async_init as *const _),
    NapiSymPtr(napi_async_destroy as *const _),
    NapiSymPtr(napi_open_callback_scope as *const _),
    NapiSymPtr(napi_close_callback_scope as *const _),
    NapiSymPtr(napi_make_callback as *const _),
    NapiSymPtr(v8_isolate_get_current_abi as *const _),
    NapiSymPtr(node_get_current_event_loop_abi as *const _),
    NapiSymPtr(v8_internal_get_current_isolate_abi as *const _),
    NapiSymPtr(v8_isolate_get_current_context_abi as *const _),
    NapiSymPtr(v8_handle_scope_extend_abi as *const _),
    NapiSymPtr(v8_handle_scope_delete_extensions_abi as *const _),
    NapiSymPtr(v8_escapable_handle_scope_ctor_abi as *const _),
    NapiSymPtr(v8_escapable_handle_scope_escape_slot_abi as *const _),
    NapiSymPtr(v8_api_internal_to_local_empty_abi as *const _),
    NapiSymPtr(v8_api_internal_from_just_is_nothing_abi as *const _),
    NapiSymPtr(v8_api_internal_dispose_global_abi as *const _),
    NapiSymPtr(v8_api_internal_globalize_reference_abi as *const _),
    NapiSymPtr(v8_api_internal_clear_weak_abi as *const _),
    NapiSymPtr(v8_api_internal_make_weak_abi as *const _),
    NapiSymPtr(v8_api_internal_get_function_template_data_abi as *const _),
    NapiSymPtr(v8_string_new_from_utf8_abi as *const _),
    NapiSymPtr(v8_integer_new_abi as *const _),
    NapiSymPtr(v8_integer_new_from_unsigned_abi as *const _),
    NapiSymPtr(v8_external_new_abi as *const _),
    NapiSymPtr(v8_to_external_pointer_tag_abi as *const _),
    NapiSymPtr(v8_exception_error_abi as *const _),
    NapiSymPtr(v8_exception_type_error_abi as *const _),
    NapiSymPtr(v8_isolate_throw_exception_abi as *const _),
    NapiSymPtr(node_buffer_copy_abi as *const _),
    NapiSymPtr(node_buffer_data_abi as *const _),
    NapiSymPtr(node_buffer_length_abi as *const _),
    NapiSymPtr(v8_value_is_number_abi as *const _),
    NapiSymPtr(v8_value_is_boolean_abi as *const _),
    NapiSymPtr(v8_value_is_object_abi as *const _),
    NapiSymPtr(v8_value_boolean_value_abi as *const _),
    NapiSymPtr(v8_value_int32_value_abi as *const _),
    NapiSymPtr(v8_value_uint32_value_abi as *const _),
    NapiSymPtr(v8_value_to_integer_abi as *const _),
    NapiSymPtr(v8_value_to_string_abi as *const _),
    NapiSymPtr(v8_string_length_abi as *const _),
    NapiSymPtr(v8_string_write_utf8_v2_abi as *const _),
    NapiSymPtr(v8_external_value_abi as *const _),
    NapiSymPtr(v8_function_template_new_abi as *const _),
    NapiSymPtr(v8_function_template_set_class_name_abi as *const _),
    NapiSymPtr(v8_function_template_instance_template_abi as *const _),
    NapiSymPtr(v8_function_template_prototype_template_abi as *const _),
    NapiSymPtr(v8_object_template_new_abi as *const _),
    NapiSymPtr(v8_object_template_set_internal_field_count_abi as *const _),
    NapiSymPtr(v8_template_set_abi as *const _),
    NapiSymPtr(v8_template_set_native_data_property_abi as *const _),
    NapiSymPtr(v8_function_template_get_function_abi as *const _),
    NapiSymPtr(v8_signature_new_abi as *const _),
    NapiSymPtr(v8_object_template_new_instance_abi as *const _),
    NapiSymPtr(v8_object_set_abi as *const _),
    NapiSymPtr(v8_object_define_own_property_abi as *const _),
    NapiSymPtr(v8_object_get_abi as *const _),
    NapiSymPtr(v8_object_set_internal_field_abi as *const _),
    NapiSymPtr(v8_object_slow_get_internal_field_abi as *const _),
    NapiSymPtr(v8_object_set_aligned_pointer_in_internal_field_abi as *const _),
    NapiSymPtr(v8_object_slow_get_aligned_pointer_from_internal_field_abi as *const _),
    NapiSymPtr(v8_object_internal_field_count_abi as *const _),
    NapiSymPtr(napi_create_external as *const _),
    NapiSymPtr(napi_get_value_external as *const _),
    NapiSymPtr(napi_add_finalizer as *const _),
    NapiSymPtr(napi_adjust_external_memory as *const _),
    NapiSymPtr(napi_coerce_to_string as *const _),
    NapiSymPtr(napi_coerce_to_number as *const _),
    NapiSymPtr(napi_coerce_to_bool as *const _),
    NapiSymPtr(napi_object_freeze as *const _),
    NapiSymPtr(napi_object_seal as *const _),
    NapiSymPtr(napi_instanceof as *const _),
    NapiSymPtr(napi_get_new_target as *const _),
    NapiSymPtr(napi_get_property_names as *const _),
    NapiSymPtr(napi_add_env_cleanup_hook as *const _),
    NapiSymPtr(napi_remove_env_cleanup_hook as *const _),
    NapiSymPtr(napi_create_promise as *const _),
    NapiSymPtr(napi_resolve_deferred as *const _),
    NapiSymPtr(napi_reject_deferred as *const _),
    NapiSymPtr(napi_coerce_to_object as *const _),
    NapiSymPtr(napi_get_buffer_info as *const _),
    NapiSymPtr(napi_is_buffer as *const _),
    NapiSymPtr(napi_new_instance as *const _),
    NapiSymPtr(napi_fatal_exception as *const _),
    NapiSymPtr(napi_remove_wrap as *const _),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intrinsics;
    use crate::AgentId;

    fn run_test_on_large_stack(f: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(f)
            .expect("spawn large-stack N-API test runner")
            .join()
            .expect("large-stack N-API test runner must not panic");
    }

    fn js_string(s: &str) -> Value {
        Value::String(Rc::new(crate::value::JsString::from(s)))
    }

    unsafe fn install_test_env(rt: &mut Runtime) -> napi_env {
        let mut env_box = NapiEnv::new(rt);
        let env_ptr = &mut *env_box as *mut NapiEnv;
        rt.napi_envs.push(env_box);
        env_ptr
    }

    unsafe extern "C" fn wake_probe_call_js(
        _env: napi_env,
        _js_callback: napi_value,
        _context: *mut c_void,
        _data: *mut c_void,
    ) {
    }

    unsafe extern "C" fn owner_guard_created_callback(
        _env: napi_env,
        _info: napi_callback_info,
    ) -> napi_value {
        std::ptr::null_mut()
    }

    #[test]
    fn napi_create_function_rejects_non_owner_thread_env() {
        let mut rt = Runtime::new_with_agent_id(AgentId::from_raw(1304));
        let env = unsafe { install_test_env(&mut rt) };
        let mut owner_result = std::ptr::null_mut();
        let name = std::ffi::CString::new("owner_guard").unwrap();
        assert_eq!(
            unsafe {
                napi_create_function__impl(
                    env,
                    name.as_ptr(),
                    0,
                    Some(owner_guard_created_callback),
                    std::ptr::null_mut(),
                    &mut owner_result,
                )
            },
            napi_ok
        );
        assert!(!owner_result.is_null());

        let env_addr = env as usize;
        let status = std::thread::spawn(move || {
            let env = env_addr as napi_env;
            let mut wrong_thread_result = std::ptr::null_mut();
            unsafe {
                napi_create_function__impl(
                    env,
                    std::ptr::null(),
                    0,
                    Some(owner_guard_created_callback),
                    std::ptr::null_mut(),
                    &mut wrong_thread_result,
                )
            }
        })
        .join()
        .expect("wrong-thread N-API guard probe must not panic");

        assert_eq!(status, napi_generic_failure);
        let mut info = std::ptr::null();
        assert_eq!(
            unsafe { napi_get_last_error_info__impl(env, &mut info) },
            napi_ok
        );
        assert!(!info.is_null());
        let message = unsafe { std::ffi::CStr::from_ptr((*info).error_message) }
            .to_string_lossy()
            .into_owned();
        assert!(
            message.contains("napi_create_function: runtime capability used from non-owner thread"),
            "unexpected N-API owner-thread diagnostic: {message}"
        );
    }

    #[test]
    fn napi_create_reference_rejects_non_owner_thread_env() {

        let mut rt = Runtime::new_with_agent_id(AgentId::from_raw(1309));
        let env = unsafe { install_test_env(&mut rt) };

        let mut owner_value = std::ptr::null_mut();
        let name = std::ffi::CString::new("ref_target").unwrap();
        assert_eq!(
            unsafe {
                napi_create_function__impl(
                    env,
                    name.as_ptr(),
                    0,
                    Some(owner_guard_created_callback),
                    std::ptr::null_mut(),
                    &mut owner_value,
                )
            },
            napi_ok
        );
        assert!(!owner_value.is_null());

        let mut owner_ref = std::ptr::null_mut();
        assert_eq!(
            unsafe { napi_create_reference__impl(env, owner_value, 1, &mut owner_ref) },
            napi_ok
        );
        assert!(!owner_ref.is_null());

        let (env_addr, val_addr) = (env as usize, owner_value as usize);
        let status = std::thread::spawn(move || {
            let env = env_addr as napi_env;
            let value = val_addr as napi_value;
            let mut wrong_ref = std::ptr::null_mut();
            unsafe { napi_create_reference__impl(env, value, 1, &mut wrong_ref) }
        })
        .join()
        .expect("wrong-thread N-API guard probe must not panic");
        assert_eq!(status, napi_generic_failure);

        let mut info = std::ptr::null();
        assert_eq!(
            unsafe { napi_get_last_error_info__impl(env, &mut info) },
            napi_ok
        );
        assert!(!info.is_null());
        let message = unsafe { std::ffi::CStr::from_ptr((*info).error_message) }
            .to_string_lossy()
            .into_owned();
        assert!(
            message
                .contains("napi_create_reference: runtime capability used from non-owner thread"),
            "unexpected N-API owner-thread diagnostic: {message}"
        );

        let mut count = 0;
        assert_eq!(
            unsafe { napi_reference_ref__impl(env, owner_ref, &mut count) },
            napi_ok
        );
        assert_eq!(count, 2);

        for (mouth, call) in [
            ("napi_reference_ref", 0usize),
            ("napi_reference_unref", 1usize),
            ("napi_get_reference_value", 2usize),
        ] {
            let (env_addr, ref_addr) = (env as usize, owner_ref as usize);
            let status = std::thread::spawn(move || {
                let env = env_addr as napi_env;
                let owner_ref = ref_addr as napi_ref;
                match call {
                    0 => {
                        let mut out = 0;
                        unsafe { napi_reference_ref__impl(env, owner_ref, &mut out) }
                    }
                    1 => {
                        let mut out = 0;
                        unsafe { napi_reference_unref__impl(env, owner_ref, &mut out) }
                    }
                    _ => {
                        let mut out = std::ptr::null_mut();
                        unsafe { napi_get_reference_value__impl(env, owner_ref, &mut out) }
                    }
                }
            })
            .join()
            .expect("wrong-thread N-API reference guard probe must not panic");
            assert_eq!(status, napi_generic_failure, "{mouth} must fail closed");

            let mut info = std::ptr::null();
            assert_eq!(
                unsafe { napi_get_last_error_info__impl(env, &mut info) },
                napi_ok
            );
            assert!(!info.is_null());
            let message = unsafe { std::ffi::CStr::from_ptr((*info).error_message) }
                .to_string_lossy()
                .into_owned();
            assert!(
                message.contains(&format!(
                    "{mouth}: runtime capability used from non-owner thread"
                )),
                "unexpected N-API owner-thread diagnostic for {mouth}: {message}"
            );
        }

        let mut final_count = 0;
        assert_eq!(
            unsafe { napi_reference_unref__impl(env, owner_ref, &mut final_count) },
            napi_ok
        );
        assert_eq!(
            final_count, 1,
            "wrong-thread ref/unref probes must not mutate the ref count"
        );
    }

    #[test]
    fn napi_call_function_rejects_non_owner_thread_env() {
        let mut rt = Runtime::new_with_agent_id(AgentId::from_raw(1305));
        let env = unsafe { install_test_env(&mut rt) };
        let mut fn_handle = std::ptr::null_mut();
        let name = std::ffi::CString::new("owner_guard_call").unwrap();
        assert_eq!(
            unsafe {
                napi_create_function__impl(
                    env,
                    name.as_ptr(),
                    0,
                    Some(owner_guard_created_callback),
                    std::ptr::null_mut(),
                    &mut fn_handle,
                )
            },
            napi_ok
        );
        let mut owner_result = std::ptr::null_mut();
        assert_eq!(
            unsafe {
                napi_call_function__impl(
                    env,
                    std::ptr::null_mut(),
                    fn_handle,
                    0,
                    std::ptr::null(),
                    &mut owner_result,
                )
            },
            napi_ok
        );

        let env_addr = env as usize;
        let func_addr = fn_handle as usize;
        let status = std::thread::spawn(move || {
            let env = env_addr as napi_env;
            let func = func_addr as napi_value;
            let mut wrong_thread_result = std::ptr::null_mut();
            unsafe {
                napi_call_function__impl(
                    env,
                    std::ptr::null_mut(),
                    func,
                    0,
                    std::ptr::null(),
                    &mut wrong_thread_result,
                )
            }
        })
        .join()
        .expect("wrong-thread N-API call-function guard probe must not panic");

        assert_eq!(status, napi_generic_failure);
        let mut info = std::ptr::null();
        assert_eq!(
            unsafe { napi_get_last_error_info__impl(env, &mut info) },
            napi_ok
        );
        assert!(!info.is_null());
        let message = unsafe { std::ffi::CStr::from_ptr((*info).error_message) }
            .to_string_lossy()
            .into_owned();
        assert!(
            message.contains("napi_call_function: runtime capability used from non-owner thread"),
            "unexpected N-API owner-thread diagnostic: {message}"
        );
    }

    #[test]
    fn generated_napi_object_allocation_rejects_non_owner_thread_env() {
        let mut rt = Runtime::new_with_agent_id(AgentId::from_raw(1306));
        let env = unsafe { install_test_env(&mut rt) };

        let mut owner_object = std::ptr::null_mut();
        assert_eq!(
            unsafe { napi_create_object(env, &mut owner_object) },
            napi_ok
        );
        assert!(!owner_object.is_null());
        let mut owner_array = std::ptr::null_mut();
        assert_eq!(unsafe { napi_create_array(env, &mut owner_array) }, napi_ok);
        assert!(!owner_array.is_null());
        let mut owner_array_len = std::ptr::null_mut();
        assert_eq!(
            unsafe { napi_create_array_with_length(env, 3, &mut owner_array_len) },
            napi_ok
        );
        assert!(!owner_array_len.is_null());

        let env_addr = env as usize;
        let statuses = std::thread::spawn(move || {
            let env = env_addr as napi_env;
            let mut object_result = std::ptr::null_mut();
            let object_status = unsafe { napi_create_object(env, &mut object_result) };
            let mut array_result = std::ptr::null_mut();
            let array_status = unsafe { napi_create_array(env, &mut array_result) };
            let mut array_len_result = std::ptr::null_mut();
            let array_len_status =
                unsafe { napi_create_array_with_length(env, 2, &mut array_len_result) };
            (object_status, array_status, array_len_status)
        })
        .join()
        .expect("wrong-thread generated N-API alloc guard probe must not panic");

        assert_eq!(
            statuses,
            (
                napi_generic_failure,
                napi_generic_failure,
                napi_generic_failure
            )
        );
        let mut info = std::ptr::null();
        assert_eq!(
            unsafe { napi_get_last_error_info__impl(env, &mut info) },
            napi_ok
        );
        assert!(!info.is_null());
        let message = unsafe { std::ffi::CStr::from_ptr((*info).error_message) }
            .to_string_lossy()
            .into_owned();
        assert!(
            message.contains(
                "napi_create_array_with_length: runtime capability used from non-owner thread"
            ),
            "unexpected generated N-API owner-thread diagnostic: {message}"
        );
    }

    #[test]
    fn generated_napi_runtime_backed_ops_reject_non_owner_thread_env() {
        let mut rt = Runtime::new_with_agent_id(AgentId::from_raw(1307));
        let env = unsafe { install_test_env(&mut rt) };

        let mut object = std::ptr::null_mut();
        assert_eq!(unsafe { napi_create_object(env, &mut object) }, napi_ok);
        let value = unsafe { (&mut *env).push_handle(js_string("runtime-backed")) };
        let key = unsafe { (&mut *env).push_handle(js_string("k")) };
        let key_name = std::ffi::CString::new("k").unwrap();
        assert_eq!(
            unsafe { napi_set_named_property(env, object, key_name.as_ptr(), value) },
            napi_ok
        );

        let env_addr = env as usize;
        let object_addr = object as usize;
        let key_addr = key as usize;
        let statuses = std::thread::spawn(move || {
            let env = env_addr as napi_env;
            let object = object_addr as napi_value;
            let key = key_addr as napi_value;
            let key_name = std::ffi::CString::new("k").unwrap();

            let mut global = std::ptr::null_mut();
            let global_status = unsafe { napi_get_global(env, &mut global) };

            let mut named = std::ptr::null_mut();
            let get_named_status =
                unsafe { napi_get_named_property(env, object, key_name.as_ptr(), &mut named) };

            let mut has_named = false;
            let has_named_status =
                unsafe { napi_has_named_property(env, object, key_name.as_ptr(), &mut has_named) };

            let mut ty = 0;
            let typeof_status = unsafe { napi_typeof(env, object, &mut ty) };

            let mut deleted = false;
            let delete_status = unsafe { napi_delete_property(env, object, key, &mut deleted) };

            (
                global_status,
                get_named_status,
                has_named_status,
                typeof_status,
                delete_status,
            )
        })
        .join()
        .expect("wrong-thread generated N-API runtime-backed guard probe must not panic");

        assert_eq!(
            statuses,
            (
                napi_generic_failure,
                napi_generic_failure,
                napi_generic_failure,
                napi_generic_failure,
                napi_generic_failure
            )
        );
        let mut info = std::ptr::null();
        assert_eq!(
            unsafe { napi_get_last_error_info__impl(env, &mut info) },
            napi_ok
        );
        assert!(!info.is_null());
        let message = unsafe { std::ffi::CStr::from_ptr((*info).error_message) }
            .to_string_lossy()
            .into_owned();
        assert!(
            message.contains("napi_delete_property: runtime capability used from non-owner thread"),
            "unexpected generated N-API runtime-backed diagnostic: {message}"
        );
    }

    #[test]
    fn bespoke_napi_runtime_backed_helpers_reject_non_owner_thread_env() {
        let mut rt = Runtime::new_with_agent_id(AgentId::from_raw(1308));
        let env = unsafe { install_test_env(&mut rt) };

        let mut arraybuffer = std::ptr::null_mut();
        assert_eq!(
            unsafe { napi_create_arraybuffer(env, 8, std::ptr::null_mut(), &mut arraybuffer) },
            napi_ok
        );
        let mut object = std::ptr::null_mut();
        assert_eq!(unsafe { napi_create_object(env, &mut object) }, napi_ok);
        let tag = napi_type_tag {
            lower: 11,
            upper: 22,
        };

        let env_addr = env as usize;
        let arraybuffer_addr = arraybuffer as usize;
        let object_addr = object as usize;
        let tag_addr = (&tag as *const napi_type_tag) as usize;
        let statuses = std::thread::spawn(move || {
            let env = env_addr as napi_env;
            let arraybuffer = arraybuffer_addr as napi_value;
            let object = object_addr as napi_value;
            let tag = tag_addr as *const napi_type_tag;

            let mut created = std::ptr::null_mut();
            let create_arraybuffer_status =
                unsafe { napi_create_arraybuffer(env, 4, std::ptr::null_mut(), &mut created) };

            let mut is_arraybuffer = false;
            let is_arraybuffer_status =
                unsafe { napi_is_arraybuffer(env, arraybuffer, &mut is_arraybuffer) };

            let mut byte_length = 0usize;
            let info_status = unsafe {
                napi_get_arraybuffer_info(env, arraybuffer, std::ptr::null_mut(), &mut byte_length)
            };

            let mut copied = std::ptr::null_mut();
            let create_buffer_copy_status = unsafe {
                napi_create_buffer_copy(env, 0, std::ptr::null(), std::ptr::null_mut(), &mut copied)
            };

            let mut date = std::ptr::null_mut();
            let create_date_status = unsafe { napi_create_date(env, 1.0, &mut date) };

            let type_tag_status = unsafe { napi_type_tag_object(env, object, tag) };

            (
                create_arraybuffer_status,
                is_arraybuffer_status,
                info_status,
                create_buffer_copy_status,
                create_date_status,
                type_tag_status,
            )
        })
        .join()
        .expect("wrong-thread bespoke N-API runtime-backed guard probe must not panic");

        assert_eq!(
            statuses,
            (
                napi_generic_failure,
                napi_generic_failure,
                napi_generic_failure,
                napi_generic_failure,
                napi_generic_failure,
                napi_generic_failure
            )
        );
        let mut info = std::ptr::null();
        assert_eq!(
            unsafe { napi_get_last_error_info__impl(env, &mut info) },
            napi_ok
        );
        assert!(!info.is_null());
        let message = unsafe { std::ffi::CStr::from_ptr((*info).error_message) }
            .to_string_lossy()
            .into_owned();
        assert!(
            message.contains("napi_type_tag_object: runtime capability used from non-owner thread"),
            "unexpected bespoke N-API runtime-backed diagnostic: {message}"
        );
    }

    #[test]
    fn napi_main_inbox_and_keepalive_are_runtime_agent_scoped() {
        let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(1301));
        let mut rt_b = Runtime::new_with_agent_id(AgentId::from_raw(1302));

        rt_b.napi_keepalive
            .store(1, std::sync::atomic::Ordering::SeqCst);
        assert!(!has_pending(&rt_a));
        assert!(has_pending(&rt_b));
        rt_b.napi_keepalive
            .store(0, std::sync::atomic::Ordering::SeqCst);

        let ran = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let ran_for_job = ran.clone();
        rt_b.napi_main_inbox
            .lock()
            .expect("rt_b N-API inbox lock")
            .push_back(Box::new(move |_rt: &mut Runtime| {
                ran_for_job.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }));

        assert!(!has_pending(&rt_a));
        assert!(has_pending(&rt_b));
        assert_eq!(drain_main_inbox(&mut rt_a), 0);
        assert_eq!(ran.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(drain_main_inbox(&mut rt_b), 1);
        assert_eq!(ran.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn napi_threadsafe_function_call_wakes_owner_runtime() {
        let mut rt = Runtime::new_with_agent_id(AgentId::from_raw(1303));
        let env = unsafe { install_test_env(&mut rt) };
        let mut tsfn = std::ptr::null_mut();
        assert_eq!(
            unsafe {
                napi_create_threadsafe_function__impl(
                    env,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    0,
                    1,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    Some(wake_probe_call_js),
                    &mut tsfn,
                )
            },
            napi_ok
        );

        let before = rt.agent_wake_generation();
        assert_eq!(
            unsafe {
                napi_call_threadsafe_function__impl(
                    tsfn,
                    std::ptr::null_mut(),
                    napi_threadsafe_function_call_mode::napi_tsfn_nonblocking,
                )
            },
            napi_ok
        );
        assert_ne!(
            rt.agent_wake_generation(),
            before,
            "N-API producer must wake the owning runtime after queueing"
        );
        assert_eq!(drain_main_inbox(&mut rt), 1);
        assert_eq!(
            unsafe {
                napi_release_threadsafe_function__impl(
                    tsfn,
                    napi_threadsafe_function_release_mode::napi_tsfn_release,
                )
            },
            napi_ok
        );
    }

    #[test]
    fn old_v8_abi_dlopen_errors_leave_napi_lane() {
        let err = "symbol not found in flat namespace '__ZN2v811HandleScope16DeleteExtensionsEPNS_7IsolateE'";
        assert!(native_addon_error_mentions_old_v8_abi(err));
        assert!(format_native_addon_dlopen_error("/x/z.node", err)
            .starts_with("native-addon: old V8/NAN ABI required"));

        let napi_err = "image not found";
        assert!(!native_addon_error_mentions_old_v8_abi(napi_err));
        assert_eq!(
            format_native_addon_dlopen_error("/x/z.node", napi_err),
            "napi: dlopen('/x/z.node'): image not found"
        );
    }

    #[test]
    fn old_v8_node_buffer_copy_return_preserves_typed_array_view() {
        let mut rt = Runtime::new();
        let mut state = OldV8State::new(&mut rt);
        OLD_V8_STATE_PTR.store((&mut state as *mut OldV8State) as usize, Ordering::SeqCst);

        let bytes = [1u8, 2, 3, 4];
        let handle = unsafe {
            node_buffer_copy_abi(std::ptr::null_mut(), bytes.as_ptr().cast(), bytes.len())
        };
        let returned = old_v8_value_from_return_slot(&state, handle as usize);
        OLD_V8_STATE_PTR.store(0, Ordering::SeqCst);

        let buffer_id = match returned {
            Value::Object(id) => id,
            other => panic!("node::Buffer::Copy did not return an object: {other:?}"),
        };
        let view = rt
            .typed_array_views
            .get(&buffer_id)
            .expect("returned buffer should keep typed-array view record");
        assert_eq!(view.fixed_length, Some(bytes.len()));
        assert_eq!(view.byte_offset, 0);
        assert_eq!(view.bytes_per_element, 1);
        assert_eq!(&*view.element_kind, "Uint8Array");
        assert!(matches!(
            rt.object_get(buffer_id, "__is_buffer__"),
            Value::Boolean(true)
        ));
        assert_eq!(
            rt.typed_array_view_bytes(buffer_id).as_deref(),
            Some(bytes.as_slice())
        );
    }

    unsafe extern "C" fn f2_napi_return_buffer(
        env: napi_env,
        _info: napi_callback_info,
    ) -> napi_value {
        let bytes = [5u8, 6, 7, 8];
        let mut result = std::ptr::null_mut();
        let status = unsafe {
            napi_create_buffer_copy__impl(
                env,
                bytes.len(),
                bytes.as_ptr().cast(),
                std::ptr::null_mut(),
                &mut result,
            )
        };
        assert_eq!(status, napi_ok);
        result
    }

    #[test]
    fn napi_created_function_returned_buffer_preserves_typed_array_view() {
        let mut rt = Runtime::new();
        let env = unsafe { install_test_env(&mut rt) };
        let mut fn_handle = std::ptr::null_mut();
        let name = std::ffi::CString::new("f2_napi_return_buffer").unwrap();
        let status = unsafe {
            napi_create_function__impl(
                env,
                name.as_ptr(),
                0,
                Some(f2_napi_return_buffer),
                std::ptr::null_mut(),
                &mut fn_handle,
            )
        };
        assert_eq!(status, napi_ok);

        let fn_v = unsafe { (&*env).get_handle(fn_handle).cloned().unwrap() };
        let returned = rt
            .call_function(fn_v, Value::Undefined, Vec::new())
            .expect("N-API-created function should return a buffer");

        let buffer_id = match returned {
            Value::Object(id) => id,
            other => panic!("N-API callback did not return an object: {other:?}"),
        };
        let view = rt
            .typed_array_views
            .get(&buffer_id)
            .expect("returned buffer should keep typed-array view record");
        assert_eq!(view.fixed_length, Some(4));
        assert_eq!(view.byte_offset, 0);
        assert_eq!(view.bytes_per_element, 1);
        assert_eq!(&*view.element_kind, "Uint8Array");
        assert!(matches!(
            rt.object_get(buffer_id, "__is_buffer__"),
            Value::Boolean(true)
        ));
        assert_eq!(
            rt.typed_array_view_bytes(buffer_id).as_deref(),
            Some([5u8, 6, 7, 8].as_slice())
        );
    }

    unsafe extern "C" fn f2_napi_module_init_return_buffer(
        env: napi_env,
        _exports: napi_value,
    ) -> napi_value {
        let bytes = [9u8, 10, 11, 12];
        let mut result = std::ptr::null_mut();
        let status = unsafe {
            napi_create_buffer_copy__impl(
                env,
                bytes.len(),
                bytes.as_ptr().cast(),
                std::ptr::null_mut(),
                &mut result,
            )
        };
        assert_eq!(status, napi_ok);
        result
    }

    #[test]
    fn napi_module_init_returned_buffer_preserves_typed_array_view() {
        let mut rt = Runtime::new();
        let exports_id = rt.alloc_object(Object::new_ordinary());
        let exports_v = Value::Object(exports_id);
        let mut env_box = NapiEnv::new(&mut rt);
        let env_ptr = &mut *env_box as *mut NapiEnv;
        let exports_handle = env_box.push_handle(exports_v.clone());
        let ret_handle = unsafe { f2_napi_module_init_return_buffer(env_ptr, exports_handle) };
        assert!(env_box.pending_exception.is_none());

        let returned = env_box.get_handle(ret_handle).cloned().unwrap_or(exports_v);
        rt.napi_envs.push(env_box);

        let buffer_id = match returned {
            Value::Object(id) => id,
            other => panic!("N-API module init did not return an object: {other:?}"),
        };
        let view = rt
            .typed_array_views
            .get(&buffer_id)
            .expect("module init returned buffer should keep typed-array view record");
        assert_eq!(view.fixed_length, Some(4));
        assert_eq!(view.byte_offset, 0);
        assert_eq!(view.bytes_per_element, 1);
        assert_eq!(&*view.element_kind, "Uint8Array");
        assert!(matches!(
            rt.object_get(buffer_id, "__is_buffer__"),
            Value::Boolean(true)
        ));
        assert_eq!(
            rt.typed_array_view_bytes(buffer_id).as_deref(),
            Some([9u8, 10, 11, 12].as_slice())
        );
    }

    #[test]
    fn napi_return_buffer_proto_numeric_object_canonicalizes_typed_array_view() {
        let mut rt = Runtime::new();
        let global = rt
            .global_object
            .expect("Runtime::new should allocate global object");
        let buffer_proto = rt.alloc_object(Object::new_ordinary());
        let mut buffer_ctor = Object::new_ordinary();
        buffer_ctor.set_own_internal("prototype".into(), Value::Object(buffer_proto));
        let buffer_ctor = rt.alloc_object(buffer_ctor);
        rt.object_set(global, "Buffer".into(), Value::Object(buffer_ctor));
        let array_buffer_proto = rt.alloc_object(Object::new_ordinary());
        let mut array_buffer_ctor = Object::new_ordinary();
        array_buffer_ctor.set_own_internal("prototype".into(), Value::Object(array_buffer_proto));
        let array_buffer_ctor = rt.alloc_object(array_buffer_ctor);
        rt.object_set(
            global,
            "ArrayBuffer".into(),
            Value::Object(array_buffer_ctor),
        );
        let mut o = Object::new_ordinary();
        o.proto = Some(buffer_proto);
        let buffer_id = rt.alloc_object(o);
        for (i, b) in [72u8, 0, 0, 0].iter().enumerate() {
            rt.object_set(buffer_id, i.to_string(), Value::Number(*b as f64));
        }

        let returned = canonicalize_napi_return_buffer_like(&mut rt, Value::Object(buffer_id));
        let buffer_id = match returned {
            Value::Object(id) => id,
            other => panic!("canonicalized buffer is not object: {other:?}"),
        };
        let view = rt
            .typed_array_views
            .get(&buffer_id)
            .expect("Buffer-prototype numeric object should get a typed-array view");
        assert_eq!(view.fixed_length, Some(4));
        assert_eq!(view.byte_offset, 0);
        assert_eq!(view.bytes_per_element, 1);
        assert_eq!(&*view.element_kind, "Uint8Array");
        assert!(matches!(
            rt.object_get(buffer_id, "__ta_kind"),
            Value::String(ref s) if s.as_str() == "Uint8Array"
        ));
        assert_eq!(
            rt.typed_array_view_bytes(buffer_id).as_deref(),
            Some([72u8, 0, 0, 0].as_slice())
        );

        assert_eq!(rt.typed_array_view_len(buffer_id), Some(4));
    }

    #[test]
    fn napi_call_function_roots_recv_func_args_across_gc() {
        let mut rt = Runtime::new();
        let env = unsafe { install_test_env(&mut rt) };

        let receiver = rt.alloc_object(Object::new_ordinary());
        rt.object_set(receiver, "tag".into(), js_string("recv-live"));
        let arg = rt.alloc_object(Object::new_ordinary());
        rt.object_set(arg, "tag".into(), js_string("arg-live"));
        let func = rt.alloc_object(intrinsics::make_native("f2_napi_call", |rt, args| {
            rt.collect();
            let arg = match args.first() {
                Some(Value::Object(id)) => *id,
                other => panic!("unexpected arg after GC: {other:?}"),
            };
            match rt.object_get(arg, "tag") {
                Value::String(s) => Ok(Value::String(s)),
                other => panic!("unexpected arg tag after GC: {other:?}"),
            }
        }));

        let (recv_h, func_h, arg_h) = unsafe {
            let env_ref = &mut *env;
            (
                env_ref.push_handle(Value::Object(receiver)),
                env_ref.push_handle(Value::Object(func)),
                env_ref.push_handle(Value::Object(arg)),
            )
        };
        let argv = [arg_h];
        let mut result = std::ptr::null_mut();

        let status = unsafe {
            napi_call_function__impl(env, recv_h, func_h, argv.len(), argv.as_ptr(), &mut result)
        };
        assert_eq!(status, napi_ok);

        let out = unsafe { (&*env).get_handle(result).cloned() };
        match out {
            Some(Value::String(s)) => assert_eq!(s.as_str(), "arg-live"),
            other => panic!("unexpected result after N-API call GC: {other:?}"),
        }
        match rt.object_get(receiver, "tag") {
            Value::String(s) => assert_eq!(s.as_str(), "recv-live"),
            other => panic!("receiver was not live after N-API call GC: {other:?}"),
        }
    }

    #[test]
    fn napi_escaped_handle_survives_escapable_scope_close() {
        let mut rt = Runtime::new();
        let env = unsafe { install_test_env(&mut rt) };

        let mut scope: napi_escapable_handle_scope = std::ptr::null_mut();
        assert_eq!(
            unsafe { napi_open_escapable_handle_scope__impl(env, &mut scope) },
            napi_ok
        );
        let mut object: napi_value = std::ptr::null_mut();
        assert_eq!(unsafe { napi_create_object(env, &mut object) }, napi_ok);
        let object_id = match unsafe { (&*env).get_handle(object).cloned() } {
            Some(Value::Object(id)) => id,
            other => panic!("created object handle did not resolve: {other:?}"),
        };
        rt.object_set(object_id, "tag".into(), js_string("escaped-live"));

        let mut escaped: napi_value = std::ptr::null_mut();
        assert_eq!(
            unsafe { napi_escape_handle__impl(env, scope, object, &mut escaped) },
            napi_ok
        );
        assert_eq!(
            unsafe { napi_close_escapable_handle_scope__impl(env, scope) },
            napi_ok
        );

        let escaped_id = match unsafe { (&*env).get_handle(escaped).cloned() } {
            Some(Value::Object(id)) => id,
            other => panic!("escaped handle did not survive scope close: {other:?}"),
        };
        match rt.object_get(escaped_id, "tag") {
            Value::String(s) => assert_eq!(s.as_str(), "escaped-live"),
            other => panic!("escaped object lost property after scope close: {other:?}"),
        }
    }

    struct AsyncDeleteState {
        work: *mut NapiAsyncWork,
        delete_status: napi_status,
        completed: bool,
    }

    unsafe extern "C" fn async_delete_execute(_env: napi_env, _data: *mut c_void) {}

    unsafe extern "C" fn async_delete_complete(
        env: napi_env,
        _status: napi_status,
        data: *mut c_void,
    ) {
        let state = unsafe { &mut *(data as *mut AsyncDeleteState) };
        state.delete_status = unsafe { napi_delete_async_work__impl(env, state.work) };
        state.completed = true;
    }

    #[test]
    fn napi_async_work_can_be_deleted_from_complete_callback() {
        let mut rt = Runtime::new();
        let env = unsafe { install_test_env(&mut rt) };
        let mut state = Box::new(AsyncDeleteState {
            work: std::ptr::null_mut(),
            delete_status: napi_generic_failure,
            completed: false,
        });
        let state_ptr = &mut *state as *mut AsyncDeleteState as *mut c_void;
        let mut work = std::ptr::null_mut();

        let create_status = unsafe {
            napi_create_async_work__impl(
                env,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                Some(async_delete_execute),
                Some(async_delete_complete),
                state_ptr,
                &mut work,
            )
        };
        assert_eq!(create_status, napi_ok);
        state.work = work;

        let queue_status = unsafe { napi_queue_async_work__impl(env, work) };
        assert_eq!(queue_status, napi_ok);

        for _ in 0..100 {
            if drain_main_inbox(&mut rt) > 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        assert!(state.completed, "complete callback did not run");
        assert_eq!(state.delete_status, napi_ok);
        assert_eq!(
            rt.napi_keepalive.load(std::sync::atomic::Ordering::SeqCst),
            0
        );
    }

    #[test]
    fn napi_deferred_promise_is_rooted_until_resolution() {
        let mut rt = Runtime::new();
        let env = unsafe { install_test_env(&mut rt) };
        let mut deferred: napi_deferred = std::ptr::null_mut();
        let mut promise: napi_value = std::ptr::null_mut();

        let create_status = unsafe { napi_create_promise__impl(env, &mut deferred, &mut promise) };
        assert_eq!(create_status, napi_ok);
        assert!(!deferred.is_null());
        assert!(matches!(
            unsafe { (&*env).get_handle(promise).cloned() },
            Some(Value::Object(_))
        ));

        unsafe {
            (&mut *env).handles.clear();
        }
        rt.collect();

        let value_handle = unsafe { (&mut *env).push_handle(js_string("resolved-after-gc")) };
        let resolve_status = unsafe { napi_resolve_deferred__impl(env, deferred, value_handle) };
        assert_eq!(resolve_status, napi_ok);
        assert!(
            unsafe { (&*env).refs.iter().all(|slot| slot.is_none()) },
            "deferred promise root slot should be released after resolution"
        );
    }

    unsafe extern "C" fn f2_napi_created_callback(
        env: napi_env,
        info: napi_callback_info,
    ) -> napi_value {
        let mut argc = 1usize;
        let mut argv = [std::ptr::null_mut(); 1];
        let mut this_arg = std::ptr::null_mut();
        let status = unsafe {
            napi_get_cb_info__impl(
                env,
                info,
                &mut argc,
                argv.as_mut_ptr(),
                &mut this_arg,
                std::ptr::null_mut(),
            )
        };
        assert_eq!(status, napi_ok);
        assert_eq!(argc, 1);

        let env_ref = unsafe { &mut *env };
        let rt = unsafe { &mut *env_ref.rt };
        rt.collect();

        let arg = match env_ref.get_handle(argv[0]).cloned() {
            Some(Value::Object(id)) => id,
            other => panic!("callback arg missing after GC: {other:?}"),
        };
        let this = match env_ref.get_handle(this_arg).cloned() {
            Some(Value::Object(id)) => id,
            other => panic!("callback this missing after GC: {other:?}"),
        };
        match rt.object_get(this, "tag") {
            Value::String(s) => assert_eq!(s.as_str(), "this-live"),
            other => panic!("callback this tag missing after GC: {other:?}"),
        }
        match rt.object_get(arg, "tag") {
            Value::String(s) => env_ref.push_handle(Value::String(s)),
            other => panic!("callback arg tag missing after GC: {other:?}"),
        }
    }

    #[test]
    fn napi_created_function_roots_callback_info_across_gc() {
        let mut rt = Runtime::new();
        let env = unsafe { install_test_env(&mut rt) };
        let mut fn_handle = std::ptr::null_mut();
        let name = std::ffi::CString::new("f2_napi_created_callback").unwrap();
        let status = unsafe {
            napi_create_function__impl(
                env,
                name.as_ptr(),
                0,
                Some(f2_napi_created_callback),
                std::ptr::null_mut(),
                &mut fn_handle,
            )
        };
        assert_eq!(status, napi_ok);

        let this = rt.alloc_object(Object::new_ordinary());
        rt.object_set(this, "tag".into(), js_string("this-live"));
        let arg = rt.alloc_object(Object::new_ordinary());
        rt.object_set(arg, "tag".into(), js_string("callback-arg-live"));
        let fn_v = unsafe { (&*env).get_handle(fn_handle).cloned().unwrap() };
        let result = rt
            .call_function(fn_v, Value::Object(this), vec![Value::Object(arg)])
            .expect("N-API-created function should return");

        match result {
            Value::String(s) => assert_eq!(s.as_str(), "callback-arg-live"),
            other => panic!("unexpected callback result after GC: {other:?}"),
        }
        match rt.object_get(this, "tag") {
            Value::String(s) => assert_eq!(s.as_str(), "this-live"),
            other => panic!("callback receiver was not live after GC: {other:?}"),
        }
    }

    struct TsfnSubscriptionPayload {
        label: &'static str,
        include_chunk_names: bool,
        log: *mut Vec<String>,
    }

    unsafe extern "C" fn f2_tsfn_subscription_call_js(
        env: napi_env,
        js_callback: napi_value,
        _context: *mut c_void,
        data: *mut c_void,
    ) {
        assert!(
            !js_callback.is_null(),
            "subscription tsfn should have JS callback"
        );
        let payload = unsafe { &mut *(data as *mut TsfnSubscriptionPayload) };
        let env_ref = unsafe { &mut *env };
        let rt = unsafe { &mut *env_ref.rt };

        let object = rt.alloc_object(Object::new_ordinary());
        if payload.include_chunk_names {
            let chunk_names = rt.alloc_object(Object::new_array());
            rt.object_set(object, "chunkNames".into(), Value::Object(chunk_names));
        }
        let issues = rt.alloc_object(Object::new_array());
        let diagnostics = rt.alloc_object(Object::new_array());
        rt.object_set(object, "issues".into(), Value::Object(issues));
        rt.object_set(object, "diagnostics".into(), Value::Object(diagnostics));

        let callback = env_ref.get_handle(js_callback).cloned().unwrap();
        let arg = env_ref.push_handle(Value::Object(object));
        let argv = [arg];
        let mut result = std::ptr::null_mut();
        let status = unsafe {
            napi_call_function__impl(
                env,
                std::ptr::null_mut(),
                env_ref.push_handle(callback),
                argv.len(),
                argv.as_ptr(),
                &mut result,
            )
        };
        assert_eq!(status, napi_ok);

        let returned = env_ref
            .get_handle(result)
            .cloned()
            .unwrap_or(Value::Undefined);
        let line = match returned {
            Value::String(s) => s.as_str().to_string(),
            other => format!("{other:?}"),
        };
        unsafe {
            (*payload.log).push(format!("{}:{line}", payload.label));
        }
    }

    #[test]
    fn napi_threadsafe_subscription_channels_preserve_payload_shape() {
        let mut rt = Runtime::new();
        let env = unsafe { install_test_env(&mut rt) };
        let mut log: Vec<String> = Vec::new();

        let chunk_callback = rt.alloc_object(intrinsics::make_native("chunk-sub", |rt, args| {
            let data = match args.first() {
                Some(Value::Object(id)) => *id,
                other => panic!("chunk callback got non-object payload: {other:?}"),
            };
            let has_chunk_names = rt.obj(data).has_own_str("chunkNames");
            let chunk_names_is_array = match rt.object_get(data, "chunkNames") {
                Value::Object(id) => {
                    matches!(rt.obj(id).internal_kind, crate::value::InternalKind::Array)
                }
                _ => false,
            };
            let has_issues = rt.obj(data).has_own_str("issues");
            let has_diagnostics = rt.obj(data).has_own_str("diagnostics");
            let s = format!(
                "chunkNames={has_chunk_names}|issues={has_issues}|diagnostics={has_diagnostics}|array={chunk_names_is_array}"
            );
            Ok(js_string(&s))
        }));
        let diagnostics_callback =
            rt.alloc_object(intrinsics::make_native("diagnostics-sub", |rt, args| {
                let data = match args.first() {
                    Some(Value::Object(id)) => *id,
                    other => panic!("diagnostics callback got non-object payload: {other:?}"),
                };
                let has_chunk_names = rt.obj(data).has_own_str("chunkNames");
                let has_issues = rt.obj(data).has_own_str("issues");
                let has_diagnostics = rt.obj(data).has_own_str("diagnostics");
                let s =
                    format!("chunkNames={has_chunk_names}|issues={has_issues}|diagnostics={has_diagnostics}");
                Ok(js_string(&s))
            }));

        let chunk_callback_h = unsafe { (&mut *env).push_handle(Value::Object(chunk_callback)) };
        let diagnostics_callback_h =
            unsafe { (&mut *env).push_handle(Value::Object(diagnostics_callback)) };
        let mut chunk_tsfn = std::ptr::null_mut();
        let mut diagnostics_tsfn = std::ptr::null_mut();
        let mut chunk_payload = TsfnSubscriptionPayload {
            label: "chunk",
            include_chunk_names: true,
            log: &mut log,
        };
        let mut diagnostics_payload = TsfnSubscriptionPayload {
            label: "diagnostics",
            include_chunk_names: false,
            log: &mut log,
        };

        assert_eq!(
            unsafe {
                napi_create_threadsafe_function__impl(
                    env,
                    chunk_callback_h,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    0,
                    1,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    Some(f2_tsfn_subscription_call_js),
                    &mut chunk_tsfn,
                )
            },
            napi_ok
        );
        assert_eq!(
            unsafe {
                napi_create_threadsafe_function__impl(
                    env,
                    diagnostics_callback_h,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    0,
                    1,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    Some(f2_tsfn_subscription_call_js),
                    &mut diagnostics_tsfn,
                )
            },
            napi_ok
        );

        assert_eq!(
            unsafe {
                napi_call_threadsafe_function__impl(
                    diagnostics_tsfn,
                    &mut diagnostics_payload as *mut TsfnSubscriptionPayload as *mut c_void,
                    napi_threadsafe_function_call_mode::napi_tsfn_nonblocking,
                )
            },
            napi_ok
        );
        assert_eq!(
            unsafe {
                napi_call_threadsafe_function__impl(
                    chunk_tsfn,
                    &mut chunk_payload as *mut TsfnSubscriptionPayload as *mut c_void,
                    napi_threadsafe_function_call_mode::napi_tsfn_nonblocking,
                )
            },
            napi_ok
        );

        assert_eq!(drain_main_inbox(&mut rt), 2);
        assert_eq!(log.len(), 2);
        assert!(
            log.iter()
                .any(|line| line == "diagnostics:chunkNames=false|issues=true|diagnostics=true"),
            "diagnostics channel shape was not preserved: {log:?}"
        );
        assert!(
            log.iter().any(|line| line
                == "chunk:chunkNames=true|issues=true|diagnostics=true|array=true"),
            "chunk channel lost chunkNames empty-array payload: {log:?}"
        );
    }

    #[test]
    fn napi_object_array_payload_shape_survives_call_function_callback() {
        let mut rt = Runtime::new();
        let env = unsafe { install_test_env(&mut rt) };

        let callback = rt.alloc_object(intrinsics::make_native("payload-shape", |rt, args| {
            let data = match args.first() {
                Some(Value::Object(id)) => *id,
                other => panic!("callback got non-object payload: {other:?}"),
            };
            let has_chunk_names = rt.obj(data).has_own_str("chunkNames");
            let has_issues = rt.obj(data).has_own_str("issues");
            let has_diagnostics = rt.obj(data).has_own_str("diagnostics");
            let chunk_names_is_array = match rt.object_get(data, "chunkNames") {
                Value::Object(id) => {
                    matches!(rt.obj(id).internal_kind, crate::value::InternalKind::Array)
                }
                _ => false,
            };
            Ok(js_string(&format!(
                "chunkNames={has_chunk_names}|issues={has_issues}|diagnostics={has_diagnostics}|array={chunk_names_is_array}"
            )))
        }));
        let callback_h = unsafe { (&mut *env).push_handle(Value::Object(callback)) };
        let mut payload = std::ptr::null_mut();
        let mut chunk_names = std::ptr::null_mut();
        let mut issues = std::ptr::null_mut();
        let mut diagnostics = std::ptr::null_mut();

        assert_eq!(unsafe { napi_create_object(env, &mut payload) }, napi_ok);
        assert_eq!(unsafe { napi_create_array(env, &mut chunk_names) }, napi_ok);
        assert_eq!(unsafe { napi_create_array(env, &mut issues) }, napi_ok);
        assert_eq!(unsafe { napi_create_array(env, &mut diagnostics) }, napi_ok);

        let chunk_names_key = std::ffi::CString::new("chunkNames").unwrap();
        let issues_key = std::ffi::CString::new("issues").unwrap();
        let diagnostics_key = std::ffi::CString::new("diagnostics").unwrap();
        assert_eq!(
            unsafe { napi_set_named_property(env, payload, chunk_names_key.as_ptr(), chunk_names) },
            napi_ok
        );
        assert_eq!(
            unsafe { napi_set_named_property(env, payload, issues_key.as_ptr(), issues) },
            napi_ok
        );
        assert_eq!(
            unsafe { napi_set_named_property(env, payload, diagnostics_key.as_ptr(), diagnostics) },
            napi_ok
        );

        let argv = [payload];
        let mut result = std::ptr::null_mut();
        assert_eq!(
            unsafe {
                napi_call_function__impl(
                    env,
                    std::ptr::null_mut(),
                    callback_h,
                    argv.len(),
                    argv.as_ptr(),
                    &mut result,
                )
            },
            napi_ok
        );

        let out = unsafe { (&*env).get_handle(result).cloned() };
        match out {
            Some(Value::String(s)) => assert_eq!(
                s.as_str(),
                "chunkNames=true|issues=true|diagnostics=true|array=true"
            ),
            other => panic!("unexpected callback result for payload shape: {other:?}"),
        }
    }

    #[test]
    fn napi_property_names_enumerates_shaped_napi_writes_for_napi_rs_copy() {
        let mut rt = Runtime::new();
        let env = unsafe { install_test_env(&mut rt) };
        let mut inner = std::ptr::null_mut();
        let mut chunk_names = std::ptr::null_mut();
        let mut keys = std::ptr::null_mut();
        let mut key = std::ptr::null_mut();
        let mut copied = std::ptr::null_mut();
        let mut outer = std::ptr::null_mut();
        let key_name = std::ffi::CString::new("chunkNames").unwrap();

        assert_eq!(unsafe { napi_create_object(env, &mut inner) }, napi_ok);
        assert_eq!(unsafe { napi_create_array(env, &mut chunk_names) }, napi_ok);
        assert_eq!(
            unsafe { napi_set_named_property(env, inner, key_name.as_ptr(), chunk_names) },
            napi_ok
        );

        assert_eq!(
            unsafe { napi_get_property_names(env, inner, &mut keys) },
            napi_ok
        );
        let mut key_count = 0;
        assert_eq!(
            unsafe { napi_get_array_length(env, keys, &mut key_count) },
            napi_ok
        );
        assert_eq!(key_count, 1);
        assert_eq!(unsafe { napi_get_element(env, keys, 0, &mut key) }, napi_ok);
        assert!(matches!(
            unsafe { (&*env).get_handle(key).cloned() },
            Some(Value::String(ref s)) if s.as_str() == "chunkNames"
        ));

        assert_eq!(
            unsafe { napi_get_property(env, inner, key, &mut copied) },
            napi_ok
        );
        assert_eq!(unsafe { napi_create_object(env, &mut outer) }, napi_ok);
        assert_eq!(
            unsafe { napi_set_named_property(env, outer, key_name.as_ptr(), copied) },
            napi_ok
        );
        let outer_id = match unsafe { (&*env).get_handle(outer).cloned() } {
            Some(Value::Object(id)) => id,
            other => panic!("outer did not remain an object: {other:?}"),
        };
        assert!(matches!(
            rt.object_get(outer_id, "chunkNames"),
            Value::Object(_)
        ));
    }

    #[test]
    fn napi_get_value_string_utf16_extracts_code_units() {
        let mut rt = Runtime::new();
        let env = unsafe { install_test_env(&mut rt) };
        let handle = unsafe { (&mut *env).push_handle(js_string("server🚀")) };

        let mut len = 0usize;
        assert_eq!(
            unsafe {
                napi_get_value_string_utf16__impl(env, handle, std::ptr::null_mut(), 0, &mut len)
            },
            napi_ok
        );
        assert_eq!(len, 8);

        let mut buf = [0u16; 9];
        let mut copied = 0usize;
        assert_eq!(
            unsafe {
                napi_get_value_string_utf16__impl(
                    env,
                    handle,
                    buf.as_mut_ptr(),
                    buf.len(),
                    &mut copied,
                )
            },
            napi_ok
        );
        assert_eq!(copied, 8);
        assert_eq!(
            &buf[..copied],
            "server🚀".encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(buf[copied], 0);

        let mut short = [0u16; 4];
        let mut short_copied = 0usize;
        assert_eq!(
            unsafe {
                napi_get_value_string_utf16__impl(
                    env,
                    handle,
                    short.as_mut_ptr(),
                    short.len(),
                    &mut short_copied,
                )
            },
            napi_ok
        );
        assert_eq!(short_copied, 3);
        assert_eq!(&short[..], &['s' as u16, 'e' as u16, 'r' as u16, 0]);
    }

    #[test]
    fn napi_external_and_wrap_pointers_are_non_enumerable() {
        let mut rt = Runtime::new();
        let env = unsafe { install_test_env(&mut rt) };

        let raw_external = 0x1234usize as *mut c_void;
        let mut external = std::ptr::null_mut();
        assert_eq!(
            unsafe {
                napi_create_external__impl(
                    env,
                    raw_external,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut external,
                )
            },
            napi_ok
        );
        let mut extracted = std::ptr::null_mut();
        assert_eq!(
            unsafe { napi_get_value_external__impl(env, external, &mut extracted) },
            napi_ok
        );
        assert_eq!(extracted, raw_external);

        let external_id = match unsafe { (&*env).get_handle(external).cloned() } {
            Some(Value::Object(id)) => id,
            other => panic!("external did not create an object: {other:?}"),
        };
        assert_eq!(
            rt.obj(external_id)
                .properties
                .iter()
                .filter(|(_, desc)| desc.enumerable)
                .count(),
            0,
            "external pointer sentinel must not leak through Object.keys"
        );

        let object_id = rt.alloc_object(Object::new_ordinary());
        let object_h = unsafe { (&mut *env).push_handle(Value::Object(object_id)) };
        let raw_wrapped = 0x5678usize as *mut c_void;
        let mut reference = std::ptr::null_mut();
        assert_eq!(
            unsafe {
                napi_wrap__impl(
                    env,
                    object_h,
                    raw_wrapped,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut reference,
                )
            },
            napi_ok
        );
        assert!(!reference.is_null());
        let mut unwrapped = std::ptr::null_mut();
        assert_eq!(
            unsafe { napi_unwrap__impl(env, object_h, &mut unwrapped) },
            napi_ok
        );
        assert_eq!(unwrapped, raw_wrapped);
        assert_eq!(
            rt.obj(object_id)
                .properties
                .iter()
                .filter(|(_, desc)| desc.enumerable)
                .count(),
            0,
            "wrapped native pointer sentinel must not leak through Object.keys"
        );
    }

    static NAPI_WRAP_FINALIZER_CALLS: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(0);
    static NAPI_WRAP_FINALIZER_DATA: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(0);
    static NAPI_WRAP_FINALIZER_HINT: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(0);
    static NAPI_WRAP_FINALIZER_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    unsafe extern "C" fn f2_napi_wrap_finalizer(
        _env: napi_env,
        data: *mut c_void,
        hint: *mut c_void,
    ) {
        NAPI_WRAP_FINALIZER_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        NAPI_WRAP_FINALIZER_DATA.store(data as usize, std::sync::atomic::Ordering::SeqCst);
        NAPI_WRAP_FINALIZER_HINT.store(hint as usize, std::sync::atomic::Ordering::SeqCst);
    }

    #[test]
    fn napi_wrap_round_trips_high_bit_pointer_without_f64_truncation() {
        let mut rt = Runtime::new();
        let env = unsafe { install_test_env(&mut rt) };
        let object_id = rt.alloc_object(Object::new_ordinary());
        let object_h = unsafe { (&mut *env).push_handle(Value::Object(object_id)) };
        let high_pointer = ((1usize << 53) + 0x12_345usize) as *mut c_void;

        assert_eq!(
            unsafe {
                napi_wrap__impl(
                    env,
                    object_h,
                    high_pointer,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            napi_ok
        );
        let mut unwrapped = std::ptr::null_mut();
        assert_eq!(
            unsafe { napi_unwrap__impl(env, object_h, &mut unwrapped) },
            napi_ok
        );
        assert_eq!(unwrapped, high_pointer);
    }

    #[test]
    fn napi_wrap_finalizer_runs_once_on_remove_wrap() {
        let _guard = NAPI_WRAP_FINALIZER_TEST_LOCK.lock().unwrap();
        NAPI_WRAP_FINALIZER_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
        NAPI_WRAP_FINALIZER_DATA.store(0, std::sync::atomic::Ordering::SeqCst);
        NAPI_WRAP_FINALIZER_HINT.store(0, std::sync::atomic::Ordering::SeqCst);

        let mut rt = Runtime::new();
        let env = unsafe { install_test_env(&mut rt) };
        let object_id = rt.alloc_object(Object::new_ordinary());
        let object_h = unsafe { (&mut *env).push_handle(Value::Object(object_id)) };
        let native = 0xdead_beefusize as *mut c_void;
        let hint = 0xfeed_faceusize as *mut c_void;

        assert_eq!(
            unsafe {
                napi_wrap__impl(
                    env,
                    object_h,
                    native,
                    f2_napi_wrap_finalizer as *mut c_void,
                    hint,
                    std::ptr::null_mut(),
                )
            },
            napi_ok
        );
        let mut removed = std::ptr::null_mut();
        assert_eq!(
            unsafe { napi_remove_wrap__impl(env, object_h, &mut removed) },
            napi_ok
        );
        assert_eq!(removed, native);
        assert_eq!(
            NAPI_WRAP_FINALIZER_CALLS.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        assert_eq!(
            NAPI_WRAP_FINALIZER_DATA.load(std::sync::atomic::Ordering::SeqCst),
            native as usize
        );
        assert_eq!(
            NAPI_WRAP_FINALIZER_HINT.load(std::sync::atomic::Ordering::SeqCst),
            hint as usize
        );

        let mut unwrapped = std::ptr::null_mut();
        assert_eq!(
            unsafe { napi_unwrap__impl(env, object_h, &mut unwrapped) },
            napi_ok
        );
        assert!(unwrapped.is_null());

        drop(rt);
        assert_eq!(
            NAPI_WRAP_FINALIZER_CALLS.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }

    #[test]
    fn napi_wrap_finalizer_runs_once_on_env_teardown() {
        let _guard = NAPI_WRAP_FINALIZER_TEST_LOCK.lock().unwrap();
        NAPI_WRAP_FINALIZER_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
        NAPI_WRAP_FINALIZER_DATA.store(0, std::sync::atomic::Ordering::SeqCst);
        NAPI_WRAP_FINALIZER_HINT.store(0, std::sync::atomic::Ordering::SeqCst);

        let native = 0xabc0_1234usize as *mut c_void;
        let hint = 0x9876_5432usize as *mut c_void;
        {
            let mut rt = Runtime::new();
            let env = unsafe { install_test_env(&mut rt) };
            let object_id = rt.alloc_object(Object::new_ordinary());
            let object_h = unsafe { (&mut *env).push_handle(Value::Object(object_id)) };

            assert_eq!(
                unsafe {
                    napi_wrap__impl(
                        env,
                        object_h,
                        native,
                        f2_napi_wrap_finalizer as *mut c_void,
                        hint,
                        std::ptr::null_mut(),
                    )
                },
                napi_ok
            );
        }

        assert_eq!(
            NAPI_WRAP_FINALIZER_CALLS.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        assert_eq!(
            NAPI_WRAP_FINALIZER_DATA.load(std::sync::atomic::Ordering::SeqCst),
            native as usize
        );
        assert_eq!(
            NAPI_WRAP_FINALIZER_HINT.load(std::sync::atomic::Ordering::SeqCst),
            hint as usize
        );
    }

    #[test]
    fn napi_wrap_finalizer_runs_once_after_gc_collects_wrapped_object() {
        let _guard = NAPI_WRAP_FINALIZER_TEST_LOCK.lock().unwrap();
        NAPI_WRAP_FINALIZER_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
        NAPI_WRAP_FINALIZER_DATA.store(0, std::sync::atomic::Ordering::SeqCst);
        NAPI_WRAP_FINALIZER_HINT.store(0, std::sync::atomic::Ordering::SeqCst);

        let mut rt = Runtime::new();
        let env = unsafe { install_test_env(&mut rt) };
        let object_id = rt.alloc_object(Object::new_ordinary());
        let object_h = unsafe { (&mut *env).push_handle(Value::Object(object_id)) };
        let native = 0x1357_2468usize as *mut c_void;
        let hint = 0x2468_1357usize as *mut c_void;

        assert_eq!(
            unsafe {
                napi_wrap__impl(
                    env,
                    object_h,
                    native,
                    f2_napi_wrap_finalizer as *mut c_void,
                    hint,
                    std::ptr::null_mut(),
                )
            },
            napi_ok
        );
        unsafe {
            (&mut *env).handles.clear();
        }
        assert_eq!(rt.collect(), 1);
        assert_eq!(
            NAPI_WRAP_FINALIZER_CALLS.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        assert_eq!(
            NAPI_WRAP_FINALIZER_DATA.load(std::sync::atomic::Ordering::SeqCst),
            native as usize
        );
        assert_eq!(
            NAPI_WRAP_FINALIZER_HINT.load(std::sync::atomic::Ordering::SeqCst),
            hint as usize
        );

        drop(rt);
        assert_eq!(
            NAPI_WRAP_FINALIZER_CALLS.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }

    unsafe extern "C" fn f2_napi_class_ctor(
        env: napi_env,
        _info: napi_callback_info,
    ) -> napi_value {
        let env_ref = unsafe { &mut *env };
        env_ref.push_handle(Value::Undefined)
    }

    static F2_NAPI_NEW_TARGET_SEEN: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(0);

    unsafe extern "C" fn f2_napi_new_target_probe_ctor(
        env: napi_env,
        info: napi_callback_info,
    ) -> napi_value {
        let mut new_target = std::ptr::null_mut();
        let status = unsafe { napi_get_new_target__impl(env, info, &mut new_target) };
        assert_eq!(status, napi_ok);
        F2_NAPI_NEW_TARGET_SEEN.store(
            if new_target.is_null() { 0 } else { 1 },
            std::sync::atomic::Ordering::SeqCst,
        );
        let env_ref = unsafe { &mut *env };
        env_ref.push_handle(Value::Undefined)
    }

    unsafe extern "C" fn f2_napi_wrapped_ptr_getter(
        env: napi_env,
        info: napi_callback_info,
    ) -> napi_value {
        let mut this_arg = std::ptr::null_mut();
        let status = unsafe {
            napi_get_cb_info__impl(
                env,
                info,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut this_arg,
                std::ptr::null_mut(),
            )
        };
        assert_eq!(status, napi_ok);

        let mut native = std::ptr::null_mut();
        let status = unsafe { napi_unwrap__impl(env, this_arg, &mut native) };
        assert_eq!(status, napi_ok);

        let mut result = std::ptr::null_mut();
        let status = unsafe { napi_create_uint32(env, native as usize as u32, &mut result) };
        assert_eq!(status, napi_ok);
        result
    }

    #[test]
    fn napi_define_class_installs_prototype_accessors_for_wrapped_instances() {
        let mut rt = Runtime::new();
        let env = unsafe { install_test_env(&mut rt) };
        let class_name = std::ffi::CString::new("WrappedThing").unwrap();
        let prop_name = std::ffi::CString::new("wrapped").unwrap();
        let properties = [napi_property_descriptor {
            utf8name: prop_name.as_ptr(),
            name: std::ptr::null_mut(),
            method: None,
            getter: Some(f2_napi_wrapped_ptr_getter),
            setter: None,
            value: std::ptr::null_mut(),
            attributes: NAPI_CONFIGURABLE,
            data: std::ptr::null_mut(),
        }];
        let mut ctor_handle = std::ptr::null_mut();
        let status = unsafe {
            napi_define_class__impl(
                env,
                class_name.as_ptr(),
                0,
                Some(f2_napi_class_ctor),
                std::ptr::null_mut(),
                properties.len(),
                properties.as_ptr(),
                &mut ctor_handle,
            )
        };
        assert_eq!(status, napi_ok);

        let ctor = match unsafe { (&*env).get_handle(ctor_handle).cloned() } {
            Some(Value::Object(id)) => id,
            other => panic!("class constructor missing: {other:?}"),
        };
        let proto = match rt.object_get(ctor, "prototype") {
            Value::Object(id) => id,
            other => panic!("class prototype missing: {other:?}"),
        };
        let mut instance = Object::new_ordinary();
        instance.proto = Some(proto);
        let instance_id = rt.alloc_object(instance);
        let instance_handle = unsafe { (&mut *env).push_handle(Value::Object(instance_id)) };
        let native_ptr = 0xfeed_u32 as usize as *mut c_void;
        let status = unsafe {
            napi_wrap__impl(
                env,
                instance_handle,
                native_ptr,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert_eq!(status, napi_ok);

        let read = rt
            .read_property_via(&Value::Object(instance_id), "wrapped")
            .expect("accessor should return");
        assert!(matches!(read, Value::Number(n) if n == 0xfeed_u32 as f64));
    }

    #[test]
    fn napi_define_class_constructor_reports_new_target() {
        let mut rt = Runtime::new();
        let env = unsafe { install_test_env(&mut rt) };
        let class_name = std::ffi::CString::new("NewTargetProbe").unwrap();
        let mut ctor_handle = std::ptr::null_mut();
        let status = unsafe {
            napi_define_class__impl(
                env,
                class_name.as_ptr(),
                0,
                Some(f2_napi_new_target_probe_ctor),
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
                &mut ctor_handle,
            )
        };
        assert_eq!(status, napi_ok);

        let ctor = unsafe { (&*env).get_handle(ctor_handle).cloned().unwrap() };
        F2_NAPI_NEW_TARGET_SEEN.store(usize::MAX, std::sync::atomic::Ordering::SeqCst);
        rt.call_function(ctor.clone(), Value::Undefined, Vec::new())
            .expect("plain call should execute probe");
        assert_eq!(
            F2_NAPI_NEW_TARGET_SEEN.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "plain call must report null new_target"
        );

        F2_NAPI_NEW_TARGET_SEEN.store(usize::MAX, std::sync::atomic::Ordering::SeqCst);
        rt.construct(ctor.clone(), Vec::new())
            .expect("construct should execute probe");
        assert_eq!(
            F2_NAPI_NEW_TARGET_SEEN.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "construct must report non-null new_target"
        );

        F2_NAPI_NEW_TARGET_SEEN.store(usize::MAX, std::sync::atomic::Ordering::SeqCst);
        let mut instance = std::ptr::null_mut();
        let status = unsafe {
            napi_new_instance__impl(env, ctor_handle, 0, std::ptr::null(), &mut instance)
        };
        assert_eq!(status, napi_ok);
        assert!(!instance.is_null());
        assert_eq!(
            F2_NAPI_NEW_TARGET_SEEN.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "napi_new_instance must route through construct and report new_target"
        );
    }

    #[test]
    fn napi_external_buffer_uses_typed_array_side_tables() {
        run_test_on_large_stack(|| {
            let mut rt = Runtime::new();
            rt.install_intrinsics();
            let env = unsafe { install_test_env(&mut rt) };
            let mut bytes = [7u8, 8, 9];
            let mut result = std::ptr::null_mut();

            let status = unsafe {
                napi_create_external_buffer__impl(
                    env,
                    bytes.len(),
                    bytes.as_mut_ptr() as *mut c_void,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut result,
                )
            };
            assert_eq!(status, napi_ok);

            let buffer_id = match unsafe { (&*env).get_handle(result).cloned() } {
                Some(Value::Object(id)) => id,
                other => panic!("external buffer did not return object: {other:?}"),
            };
            assert!(rt.typed_array_views.contains_key(&buffer_id));
            assert!(matches!(
                rt.object_get(buffer_id, "__is_buffer__"),
                Value::Boolean(true)
            ));
            assert!(matches!(rt.object_get(buffer_id, "0"), Value::Number(n) if n == 7.0));
            assert!(matches!(rt.object_get(buffer_id, "2"), Value::Number(n) if n == 9.0));

            let mut ty = napi_int8_array;
            let mut len = 0usize;
            let mut data = std::ptr::null_mut();
            let mut arraybuffer = std::ptr::null_mut();
            let mut offset = usize::MAX;
            let status = unsafe {
                napi_get_typedarray_info__impl(
                    env,
                    result,
                    &mut ty,
                    &mut len,
                    &mut data,
                    &mut arraybuffer,
                    &mut offset,
                )
            };
            assert_eq!(status, napi_ok);
            assert_eq!(ty, napi_uint8_array);
            assert_eq!(len, 3);
            assert_eq!(offset, 0);
            assert!(!data.is_null());
            assert!(matches!(
                unsafe { (&*env).get_handle(arraybuffer).cloned() },
                Some(Value::Object(id)) if rt.array_buffers.contains_key(&id)
            ));
        });
    }

    #[test]
    fn napi_create_buffer_returns_writable_zeroed_buffer() {
        run_test_on_large_stack(|| {
            let mut rt = Runtime::new();
            rt.install_intrinsics();
            let env = unsafe { install_test_env(&mut rt) };
            let mut data = std::ptr::null_mut();
            let mut result = std::ptr::null_mut();

            let status = unsafe { napi_create_buffer__impl(env, 4, &mut data, &mut result) };
            assert_eq!(status, napi_ok);
            assert!(!data.is_null());

            unsafe {
                *(data as *mut u8).add(1) = 42;
            }
            let buffer_id = match unsafe { (&*env).get_handle(result).cloned() } {
                Some(Value::Object(id)) => id,
                other => panic!("buffer did not return object: {other:?}"),
            };
            assert!(matches!(rt.object_get(buffer_id, "0"), Value::Number(n) if n == 0.0));
            assert!(matches!(rt.object_get(buffer_id, "1"), Value::Number(n) if n == 42.0));
            assert!(matches!(rt.object_get(buffer_id, "length"), Value::Number(n) if n == 4.0));
        });
    }

    #[test]
    fn napi_get_buffer_info_returns_storeback_pointer_for_buffer_views() {
        run_test_on_large_stack(|| {
            let mut rt = Runtime::new();
            rt.install_intrinsics();
            let env = unsafe { install_test_env(&mut rt) };
            let mut result = std::ptr::null_mut();

            let status = unsafe {
                napi_create_buffer_copy__impl(
                    env,
                    4,
                    [1u8, 2, 3, 4].as_ptr() as *const c_void,
                    std::ptr::null_mut(),
                    &mut result,
                )
            };
            assert_eq!(status, napi_ok);

            let mut data = std::ptr::null_mut();
            let mut len = 0usize;
            let status = unsafe { napi_get_buffer_info__impl(env, result, &mut data, &mut len) };
            assert_eq!(status, napi_ok);
            assert_eq!(len, 4);
            assert!(!data.is_null());

            unsafe {
                *(data as *mut u8).add(0) = 9;
                *(data as *mut u8).add(3) = 42;
            }

            let buffer_id = match unsafe { (&*env).get_handle(result).cloned() } {
                Some(Value::Object(id)) => id,
                other => panic!("buffer did not return object: {other:?}"),
            };
            assert!(matches!(rt.object_get(buffer_id, "0"), Value::Number(n) if n == 9.0));
            assert!(matches!(rt.object_get(buffer_id, "1"), Value::Number(n) if n == 2.0));
            assert!(matches!(rt.object_get(buffer_id, "3"), Value::Number(n) if n == 42.0));
        });
    }

    #[test]
    fn napi_create_typedarray_registers_view_over_existing_arraybuffer() {
        let mut rt = Runtime::new();
        let mut abo = Object::new_ordinary();
        abo.set_own_internal(
            "__kind".into(),
            Value::String(Rc::new("ArrayBuffer".into())),
        );
        let backing_id = rt.alloc_object(abo);
        rt.array_buffers.insert(
            backing_id,
            crate::interp::ArrayBufferRecord {
                byte_length: 8,
                max_byte_length: 8,
                backing_epoch: 0,
                data: vec![0, 0, 11, 0, 0, 44, 0, 0],
                detached: false,
                untransferable: false,
                shared: None,
            },
        );
        let env = unsafe { install_test_env(&mut rt) };
        let arraybuffer = unsafe { (&mut *env).push_handle(Value::Object(backing_id)) };

        let mut view = std::ptr::null_mut();
        let status = unsafe {
            napi_create_typedarray__impl(env, napi_uint8_array, 4, arraybuffer, 2, &mut view)
        };
        assert_eq!(status, napi_ok);

        let view_id = match unsafe { (&*env).get_handle(view).cloned() } {
            Some(Value::Object(id)) => id,
            other => panic!("typedarray did not return object: {other:?}"),
        };
        let view_rec = rt
            .typed_array_views
            .get(&view_id)
            .expect("typedarray view record");
        assert_eq!(view_rec.buffer, backing_id);
        assert_eq!(view_rec.byte_offset, 2);
        assert_eq!(view_rec.fixed_length, Some(4));
        assert_eq!(view_rec.bytes_per_element, 1);
        assert_eq!(&*view_rec.element_kind, "Uint8Array");

        let mut ty = napi_int8_array;
        let mut len = 0usize;
        let mut ptr = std::ptr::null_mut();
        let mut backing = std::ptr::null_mut();
        let mut offset = usize::MAX;
        let status = unsafe {
            napi_get_typedarray_info__impl(
                env,
                view,
                &mut ty,
                &mut len,
                &mut ptr,
                &mut backing,
                &mut offset,
            )
        };
        assert_eq!(status, napi_ok);
        assert_eq!(ty, napi_uint8_array);
        assert_eq!(len, 4);
        assert_eq!(offset, 2);
        let base = rt
            .array_buffers
            .get_mut(&backing_id)
            .expect("backing buffer")
            .data
            .as_mut_ptr();
        assert_eq!(ptr, unsafe { base.add(2) as *mut c_void });
        assert!(matches!(
            unsafe { (&*env).get_handle(backing).cloned() },
            Some(Value::Object(id)) if rt.array_buffers.contains_key(&id)
        ));
    }

    #[test]
    fn napi_create_typedarray_rejects_out_of_bounds_or_misaligned_view() {
        let mut rt = Runtime::new();
        let backing_id = rt.alloc_object(Object::new_ordinary());
        rt.array_buffers.insert(
            backing_id,
            crate::interp::ArrayBufferRecord {
                byte_length: 8,
                max_byte_length: 8,
                backing_epoch: 0,
                data: vec![0; 8],
                detached: false,
                untransferable: false,
                shared: None,
            },
        );
        let env = unsafe { install_test_env(&mut rt) };
        let arraybuffer = unsafe { (&mut *env).push_handle(Value::Object(backing_id)) };

        let mut view = std::ptr::null_mut();
        let status = unsafe {
            napi_create_typedarray__impl(env, napi_uint32_array, 2, arraybuffer, 2, &mut view)
        };
        assert_eq!(status, napi_invalid_arg);
        let status = unsafe {
            napi_create_typedarray__impl(env, napi_uint32_array, 3, arraybuffer, 0, &mut view)
        };
        assert_eq!(status, napi_invalid_arg);
    }

    #[test]
    fn napi_detach_arraybuffer_marks_backing_detached() {
        let mut rt = Runtime::new();
        let env = unsafe { install_test_env(&mut rt) };
        let mut data = std::ptr::null_mut();
        let mut result = std::ptr::null_mut();

        let status = unsafe { napi_create_arraybuffer__impl(env, 4, &mut data, &mut result) };
        assert_eq!(status, napi_ok);
        assert!(!data.is_null());

        let buffer_id = match unsafe { (&*env).get_handle(result).cloned() } {
            Some(Value::Object(id)) => id,
            other => panic!("arraybuffer did not return object: {other:?}"),
        };
        assert!(!rt.array_buffers.get(&buffer_id).unwrap().detached);
        assert_eq!(rt.array_buffers.get(&buffer_id).unwrap().byte_len(), 4);

        let status = unsafe { napi_detach_arraybuffer__impl(env, result) };
        assert_eq!(status, napi_ok);
        let backing = rt.array_buffers.get(&buffer_id).unwrap();
        assert!(backing.detached);
        assert_eq!(backing.byte_len(), 0);
        assert!(backing.data.is_empty());
    }
}
