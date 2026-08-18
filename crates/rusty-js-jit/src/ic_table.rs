
use cranelift_codegen::ir::types::{F64, I64};
use cranelift_codegen::ir::{
    AbiParam, FuncRef, InstBuilder, MemFlags, Signature, Value as ClValue,
};
use cranelift_frontend::FunctionBuilder;

pub struct IcEntry {
    pub key: &'static str,
    pub kind: IcEntryKind,
    pub receiver: ReceiverKind,
    pub extern_name: &'static str,
    pub extern_ptr: *const u8,
    pub extern_sig: fn(&mut Signature),
    pub lower: fn(&mut FunctionBuilder, &mut Vec<ClValue>, FuncRef) -> Result<(), String>,
}

unsafe impl Sync for IcEntry {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IcEntryKind {

    PropertyGet,

    MethodCall { arity: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiverKind {
    String,
    #[allow(dead_code)]
    Array,
    #[allow(dead_code)]
    Number,
    #[allow(dead_code)]
    Buffer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LejitValueDomain {
    StringReceiverPayload,
    Number,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LejitOverrideGuard {
    MethodIdentityUnchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LejitExpectationMetadata {
    pub key: &'static str,
    pub receiver: ReceiverKind,
    pub kind: IcEntryKind,
    pub arity: Option<u8>,
    pub extern_name: &'static str,
    pub arg_domains: &'static [LejitValueDomain],
    pub return_domain: LejitValueDomain,
    pub override_guard: LejitOverrideGuard,
    pub deopt_bailouts: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostNativeResultKind {
    Object,
    Number,
    Boolean,
    String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostNativeObjectKind {
    Buffer,
    PathParseResult,
    UrlSearchParams,
}

#[derive(Debug, Clone, Copy)]
pub struct HostNativeMethodCandidate {
    pub receiver: &'static str,
    pub key: &'static str,
    pub arity: u8,
    pub result: HostNativeResultKind,
    pub object_kind: Option<HostNativeObjectKind>,
}

#[derive(Debug, Clone, Copy)]
pub struct HostNativeConstructorCandidate {
    pub name: &'static str,
    pub arity: u8,
    pub result: HostNativeResultKind,
    pub object_kind: HostNativeObjectKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostGlobalObjectKind {
    Array,
    Buffer,
    Date,
    Math,
    Path,
    UrlSearchParamsCtor,
}

pub static HOST_NATIVE_METHOD_CANDIDATES: &[HostNativeMethodCandidate] = &[
    HostNativeMethodCandidate {
        receiver: "__nodeApiProbeBuffer",
        key: "alloc",
        arity: 1,
        result: HostNativeResultKind::Object,
        object_kind: Some(HostNativeObjectKind::Buffer),
    },
    HostNativeMethodCandidate {
        receiver: "Buffer",
        key: "alloc",
        arity: 1,
        result: HostNativeResultKind::Object,
        object_kind: Some(HostNativeObjectKind::Buffer),
    },
    HostNativeMethodCandidate {
        receiver: "B",
        key: "alloc",
        arity: 1,
        result: HostNativeResultKind::Object,
        object_kind: Some(HostNativeObjectKind::Buffer),
    },
    HostNativeMethodCandidate {
        receiver: "Buffer",
        key: "from",
        arity: 1,
        result: HostNativeResultKind::Object,
        object_kind: Some(HostNativeObjectKind::Buffer),
    },
    HostNativeMethodCandidate {
        receiver: "Buffer",
        key: "from",
        arity: 2,
        result: HostNativeResultKind::Object,
        object_kind: Some(HostNativeObjectKind::Buffer),
    },
    HostNativeMethodCandidate {
        receiver: "Buffer",
        key: "byteLength",
        arity: 1,
        result: HostNativeResultKind::Number,
        object_kind: None,
    },
    HostNativeMethodCandidate {
        receiver: "__nodeApiProbePath",
        key: "parse",
        arity: 1,
        result: HostNativeResultKind::Object,
        object_kind: Some(HostNativeObjectKind::PathParseResult),
    },
    HostNativeMethodCandidate {
        receiver: "Path",
        key: "parse",
        arity: 1,
        result: HostNativeResultKind::Object,
        object_kind: Some(HostNativeObjectKind::PathParseResult),
    },
    HostNativeMethodCandidate {
        receiver: "Path",
        key: "normalize",
        arity: 1,
        result: HostNativeResultKind::String,
        object_kind: None,
    },
    HostNativeMethodCandidate {
        receiver: "P",
        key: "basename",
        arity: 1,
        result: HostNativeResultKind::String,
        object_kind: None,
    },
    HostNativeMethodCandidate {
        receiver: "Path",
        key: "basename",
        arity: 1,
        result: HostNativeResultKind::String,
        object_kind: None,
    },
    HostNativeMethodCandidate {
        receiver: "__nodeApiProbePath",
        key: "basename",
        arity: 1,
        result: HostNativeResultKind::String,
        object_kind: None,
    },
    HostNativeMethodCandidate {
        receiver: "P",
        key: "dirname",
        arity: 1,
        result: HostNativeResultKind::String,
        object_kind: None,
    },
    HostNativeMethodCandidate {
        receiver: "Path",
        key: "dirname",
        arity: 1,
        result: HostNativeResultKind::String,
        object_kind: None,
    },
    HostNativeMethodCandidate {
        receiver: "__nodeApiProbePath",
        key: "dirname",
        arity: 1,
        result: HostNativeResultKind::String,
        object_kind: None,
    },
    HostNativeMethodCandidate {
        receiver: "P",
        key: "extname",
        arity: 1,
        result: HostNativeResultKind::String,
        object_kind: None,
    },
    HostNativeMethodCandidate {
        receiver: "Path",
        key: "extname",
        arity: 1,
        result: HostNativeResultKind::String,
        object_kind: None,
    },
    HostNativeMethodCandidate {
        receiver: "__nodeApiProbePath",
        key: "extname",
        arity: 1,
        result: HostNativeResultKind::String,
        object_kind: None,
    },
    HostNativeMethodCandidate {
        receiver: "P",
        key: "isAbsolute",
        arity: 1,
        result: HostNativeResultKind::Boolean,
        object_kind: None,
    },
    HostNativeMethodCandidate {
        receiver: "Path",
        key: "isAbsolute",
        arity: 1,
        result: HostNativeResultKind::Boolean,
        object_kind: None,
    },
    HostNativeMethodCandidate {
        receiver: "__nodeApiProbePath",
        key: "isAbsolute",
        arity: 1,
        result: HostNativeResultKind::Boolean,
        object_kind: None,
    },
    HostNativeMethodCandidate {
        receiver: "Path",
        key: "join",
        arity: 4,
        result: HostNativeResultKind::String,
        object_kind: None,
    },
    HostNativeMethodCandidate {
        receiver: "Date",
        key: "now",
        arity: 0,
        result: HostNativeResultKind::Number,
        object_kind: None,
    },
    HostNativeMethodCandidate {
        receiver: "Math",
        key: "imul",
        arity: 2,
        result: HostNativeResultKind::Number,
        object_kind: None,
    },
];

pub static HOST_NATIVE_CONSTRUCTOR_CANDIDATES: &[HostNativeConstructorCandidate] = &[
    HostNativeConstructorCandidate {
        name: "__nodeApiProbeURLSearchParams",
        arity: 1,
        result: HostNativeResultKind::Object,
        object_kind: HostNativeObjectKind::UrlSearchParams,
    },
    HostNativeConstructorCandidate {
        name: "URLSearchParams",
        arity: 1,
        result: HostNativeResultKind::Object,
        object_kind: HostNativeObjectKind::UrlSearchParams,
    },
];

pub fn lookup_host_global_object(name: &str) -> Option<HostGlobalObjectKind> {
    match name {
        "Array" => Some(HostGlobalObjectKind::Array),
        "Buffer" => Some(HostGlobalObjectKind::Buffer),
        "Date" => Some(HostGlobalObjectKind::Date),
        "Math" => Some(HostGlobalObjectKind::Math),
        "__nodeApiProbePath" => Some(HostGlobalObjectKind::Path),
        "URLSearchParams" => Some(HostGlobalObjectKind::UrlSearchParamsCtor),
        _ => None,
    }
}

pub fn host_global_object_candidate_name(kind: HostGlobalObjectKind) -> &'static str {
    match kind {
        HostGlobalObjectKind::Array => "Array",
        HostGlobalObjectKind::Buffer => "Buffer",
        HostGlobalObjectKind::Date => "Date",
        HostGlobalObjectKind::Math => "Math",
        HostGlobalObjectKind::Path => "Path",
        HostGlobalObjectKind::UrlSearchParamsCtor => "URLSearchParams",
    }
}

pub fn lookup_host_native_method_candidate(
    receiver: &str,
    key: &str,
    arity: u8,
) -> Option<&'static HostNativeMethodCandidate> {
    HOST_NATIVE_METHOD_CANDIDATES.iter().find(|candidate| {
        candidate.receiver == receiver && candidate.key == key && candidate.arity == arity
    })
}

pub fn lookup_host_native_constructor_candidate(
    name: &str,
    arity: u8,
) -> Option<&'static HostNativeConstructorCandidate> {
    HOST_NATIVE_CONSTRUCTOR_CANDIDATES
        .iter()
        .find(|candidate| candidate.name == name && candidate.arity == arity)
}

pub fn expectation_metadata_for_entry(entry: &IcEntry) -> Option<LejitExpectationMetadata> {
    if entry.key == "charCodeAt"
        && entry.receiver == ReceiverKind::String
        && entry.kind == (IcEntryKind::MethodCall { arity: 1 })
        && entry.extern_name == "ic_string_char_code_at"
    {
        return Some(LejitExpectationMetadata {
            key: "charCodeAt",
            receiver: ReceiverKind::String,
            kind: IcEntryKind::MethodCall { arity: 1 },
            arity: Some(1),
            extern_name: "ic_string_char_code_at",
            arg_domains: &[LejitValueDomain::Number],
            return_domain: LejitValueDomain::Number,
            override_guard: LejitOverrideGuard::MethodIdentityUnchanged,
            deopt_bailouts: &[
                "receiver-not-string",
                "arity-mismatch",
                "argument-not-number-or-undefined",
                "method-overridden",
            ],
        });
    }
    None
}

pub extern "C" fn ic_string_len(recv_bits: i64) -> f64 {

    const VD_HI16_MASK: u64 = 0xFFFF_0000_0000_0000;
    const VD_HI16_STRING: u64 = 0xFFF2_0000_0000_0000;
    const VD_PAYLOAD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;
    let bits = recv_bits as u64;
    if (bits & VD_HI16_MASK) == VD_HI16_STRING {

        return crate::deopt::call_string_len((bits & VD_PAYLOAD_MASK) as i64);
    }
    crate::deopt::call_object_length((bits & VD_PAYLOAD_MASK) as i64)
}

fn ic_string_len_sig(sig: &mut Signature) {
    sig.params.push(AbiParam::new(I64));
    sig.returns.push(AbiParam::new(F64));
}

fn lower_ic_string_len(
    builder: &mut FunctionBuilder,
    stack: &mut Vec<ClValue>,
    extern_ref: FuncRef,
) -> Result<(), String> {
    let recv_f64 = stack.pop().ok_or("ic_string_len: stack underflow")?;

    let recv_bits = builder.ins().bitcast(I64, MemFlags::new(), recv_f64);
    let call_inst = builder.ins().call(extern_ref, &[recv_bits]);
    let result = builder.inst_results(call_inst)[0];
    stack.push(result);
    Ok(())
}

pub extern "C" fn ic_string_char_code_at(payload: i64, i: i64) -> f64 {

    crate::deopt::call_string_char_code_at(payload, i)
}

fn ic_string_char_code_at_sig(sig: &mut Signature) {
    sig.params.push(AbiParam::new(I64));
    sig.params.push(AbiParam::new(I64));
    sig.returns.push(AbiParam::new(F64));
}

fn lower_ic_string_char_code_at(
    builder: &mut FunctionBuilder,
    stack: &mut Vec<ClValue>,
    extern_ref: FuncRef,
) -> Result<(), String> {
    let arg_f64 = stack
        .pop()
        .ok_or("ic_string_char_code_at: stack underflow (arg)")?;
    let _sentinel = stack
        .pop()
        .ok_or("ic_string_char_code_at: stack underflow (sentinel)")?;
    let recv_f64 = stack
        .pop()
        .ok_or("ic_string_char_code_at: stack underflow (receiver)")?;
    let recv_bits = builder.ins().bitcast(I64, MemFlags::new(), recv_f64);
    let payload_mask = builder.ins().iconst(I64, 0x0000_FFFF_FFFF_FFFF_u64 as i64);
    let payload = builder.ins().band(recv_bits, payload_mask);
    let arg_i64 = builder.ins().fcvt_to_sint_sat(I64, arg_f64);
    let call_inst = builder.ins().call(extern_ref, &[payload, arg_i64]);
    let result = builder.inst_results(call_inst)[0];
    stack.push(result);
    Ok(())
}

pub extern "C" fn ic_string_code_point_at(payload: i64, i: i64) -> f64 {

    crate::deopt::call_string_char_code_at(payload, i)
}

fn ic_string_code_point_at_sig(sig: &mut Signature) {
    sig.params.push(AbiParam::new(I64));
    sig.params.push(AbiParam::new(I64));
    sig.returns.push(AbiParam::new(F64));
}

fn lower_ic_string_code_point_at(
    builder: &mut FunctionBuilder,
    stack: &mut Vec<ClValue>,
    extern_ref: FuncRef,
) -> Result<(), String> {
    let arg_f64 = stack
        .pop()
        .ok_or("ic_string_code_point_at: stack underflow (arg)")?;
    let _sentinel = stack
        .pop()
        .ok_or("ic_string_code_point_at: stack underflow (sentinel)")?;
    let recv_f64 = stack
        .pop()
        .ok_or("ic_string_code_point_at: stack underflow (receiver)")?;
    let recv_bits = builder.ins().bitcast(I64, MemFlags::new(), recv_f64);
    let payload_mask = builder.ins().iconst(I64, 0x0000_FFFF_FFFF_FFFF_u64 as i64);
    let payload = builder.ins().band(recv_bits, payload_mask);
    let arg_i64 = builder.ins().fcvt_to_sint_sat(I64, arg_f64);
    let call_inst = builder.ins().call(extern_ref, &[payload, arg_i64]);
    let result = builder.inst_results(call_inst)[0];
    stack.push(result);
    Ok(())
}

pub extern "C" fn ic_string_index_of(haystack: i64, needle: i64, from: i64) -> f64 {

    crate::deopt::call_string_index_of(haystack, needle, from)
}

fn ic_string_index_of_sig(sig: &mut Signature) {
    sig.params.push(AbiParam::new(I64));
    sig.params.push(AbiParam::new(I64));
    sig.params.push(AbiParam::new(I64));
    sig.returns.push(AbiParam::new(F64));
}

fn lower_ic_string_index_of(
    builder: &mut FunctionBuilder,
    stack: &mut Vec<ClValue>,
    extern_ref: FuncRef,
) -> Result<(), String> {

    let from_f64 = stack
        .pop()
        .ok_or("ic_string_index_of: stack underflow (from)")?;
    let needle_f64 = stack
        .pop()
        .ok_or("ic_string_index_of: stack underflow (needle)")?;
    let _sentinel = stack
        .pop()
        .ok_or("ic_string_index_of: stack underflow (sentinel)")?;
    let recv_f64 = stack
        .pop()
        .ok_or("ic_string_index_of: stack underflow (receiver)")?;
    let recv_bits = builder.ins().bitcast(I64, MemFlags::new(), recv_f64);
    let needle_bits = builder.ins().bitcast(I64, MemFlags::new(), needle_f64);
    let payload_mask = builder.ins().iconst(I64, 0x0000_FFFF_FFFF_FFFF_u64 as i64);
    let recv_payload = builder.ins().band(recv_bits, payload_mask);
    let needle_payload = builder.ins().band(needle_bits, payload_mask);
    let from_i64 = builder.ins().fcvt_to_sint_sat(I64, from_f64);
    let call_inst = builder
        .ins()
        .call(extern_ref, &[recv_payload, needle_payload, from_i64]);
    let result = builder.inst_results(call_inst)[0];
    stack.push(result);
    Ok(())
}

pub extern "C" fn ic_array_push1(receiver_payload: i64, value: f64) -> f64 {
    crate::deopt::call_array_push1(receiver_payload, value)
}

fn ic_array_push1_sig(sig: &mut Signature) {
    sig.params.push(AbiParam::new(I64));
    sig.params.push(AbiParam::new(F64));
    sig.returns.push(AbiParam::new(F64));
}

fn lower_ic_array_push1(
    builder: &mut FunctionBuilder,
    stack: &mut Vec<ClValue>,
    extern_ref: FuncRef,
) -> Result<(), String> {
    let value = stack
        .pop()
        .ok_or("ic_array_push1: stack underflow (value)")?;
    let _sentinel = stack
        .pop()
        .ok_or("ic_array_push1: stack underflow (sentinel)")?;
    let recv_f64 = stack
        .pop()
        .ok_or("ic_array_push1: stack underflow (receiver)")?;
    let recv_bits = builder.ins().bitcast(I64, MemFlags::new(), recv_f64);
    let payload_mask = builder.ins().iconst(I64, 0x0000_FFFF_FFFF_FFFF_u64 as i64);
    let recv_payload = builder.ins().band(recv_bits, payload_mask);
    let call_inst = builder.ins().call(extern_ref, &[recv_payload, value]);
    let result = builder.inst_results(call_inst)[0];
    stack.push(result);
    Ok(())
}

pub extern "C" fn ic_array_pop(receiver_payload: i64) -> f64 {
    crate::deopt::call_array_pop(receiver_payload)
}

fn ic_array_pop_sig(sig: &mut Signature) {
    sig.params.push(AbiParam::new(I64));
    sig.returns.push(AbiParam::new(F64));
}

fn lower_ic_array_pop(
    builder: &mut FunctionBuilder,
    stack: &mut Vec<ClValue>,
    extern_ref: FuncRef,
) -> Result<(), String> {
    let _sentinel = stack
        .pop()
        .ok_or("ic_array_pop: stack underflow (sentinel)")?;
    let recv_f64 = stack
        .pop()
        .ok_or("ic_array_pop: stack underflow (receiver)")?;
    let recv_bits = builder.ins().bitcast(I64, MemFlags::new(), recv_f64);
    let payload_mask = builder.ins().iconst(I64, 0x0000_FFFF_FFFF_FFFF_u64 as i64);
    let recv_payload = builder.ins().band(recv_bits, payload_mask);
    let call_inst = builder.ins().call(extern_ref, &[recv_payload]);
    let result = builder.inst_results(call_inst)[0];
    stack.push(result);
    Ok(())
}

pub extern "C" fn ic_array_join(receiver_payload: i64, sep: f64) -> f64 {
    crate::deopt::call_array_join_owned_result(receiver_payload, sep)
}

fn ic_array_join_sig(sig: &mut Signature) {
    sig.params.push(AbiParam::new(I64));
    sig.params.push(AbiParam::new(F64));
    sig.returns.push(AbiParam::new(F64));
}

fn lower_ic_array_join(
    builder: &mut FunctionBuilder,
    stack: &mut Vec<ClValue>,
    extern_ref: FuncRef,
) -> Result<(), String> {
    let sep = stack
        .pop()
        .ok_or("ic_array_join: stack underflow (separator)")?;
    let _sentinel = stack
        .pop()
        .ok_or("ic_array_join: stack underflow (sentinel)")?;
    let recv_f64 = stack
        .pop()
        .ok_or("ic_array_join: stack underflow (receiver)")?;
    let recv_bits = builder.ins().bitcast(I64, MemFlags::new(), recv_f64);
    let payload_mask = builder.ins().iconst(I64, 0x0000_FFFF_FFFF_FFFF_u64 as i64);
    let recv_payload = builder.ins().band(recv_bits, payload_mask);
    let call_inst = builder.ins().call(extern_ref, &[recv_payload, sep]);
    let result = builder.inst_results(call_inst)[0];
    stack.push(result);
    Ok(())
}

pub static IC_TABLE: &[IcEntry] = &[
    IcEntry {
        key: "length",
        kind: IcEntryKind::PropertyGet,
        receiver: ReceiverKind::String,
        extern_name: "ic_string_len",
        extern_ptr: ic_string_len as *const u8,
        extern_sig: ic_string_len_sig,
        lower: lower_ic_string_len,
    },
    IcEntry {
        key: "charCodeAt",
        kind: IcEntryKind::MethodCall { arity: 1 },
        receiver: ReceiverKind::String,
        extern_name: "ic_string_char_code_at",
        extern_ptr: ic_string_char_code_at as *const u8,
        extern_sig: ic_string_char_code_at_sig,
        lower: lower_ic_string_char_code_at,
    },
    IcEntry {
        key: "codePointAt",
        kind: IcEntryKind::MethodCall { arity: 1 },
        receiver: ReceiverKind::String,
        extern_name: "ic_string_code_point_at",
        extern_ptr: ic_string_code_point_at as *const u8,
        extern_sig: ic_string_code_point_at_sig,
        lower: lower_ic_string_code_point_at,
    },
    IcEntry {
        key: "indexOf",
        kind: IcEntryKind::MethodCall { arity: 2 },
        receiver: ReceiverKind::String,
        extern_name: "ic_string_index_of",
        extern_ptr: ic_string_index_of as *const u8,
        extern_sig: ic_string_index_of_sig,
        lower: lower_ic_string_index_of,
    },
    IcEntry {
        key: "push",
        kind: IcEntryKind::MethodCall { arity: 1 },
        receiver: ReceiverKind::Array,
        extern_name: "ic_array_push1",
        extern_ptr: ic_array_push1 as *const u8,
        extern_sig: ic_array_push1_sig,
        lower: lower_ic_array_push1,
    },
    IcEntry {
        key: "pop",
        kind: IcEntryKind::MethodCall { arity: 0 },
        receiver: ReceiverKind::Array,
        extern_name: "ic_array_pop",
        extern_ptr: ic_array_pop as *const u8,
        extern_sig: ic_array_pop_sig,
        lower: lower_ic_array_pop,
    },
    IcEntry {
        key: "join",
        kind: IcEntryKind::MethodCall { arity: 1 },
        receiver: ReceiverKind::Array,
        extern_name: "ic_array_join",
        extern_ptr: ic_array_join as *const u8,
        extern_sig: ic_array_join_sig,
        lower: lower_ic_array_join,
    },

    IcEntry {
        key: "exec",
        kind: IcEntryKind::MethodCall { arity: 1 },
        receiver: ReceiverKind::Array,
        extern_name: "ic_regexp_exec",
        extern_ptr: ic_regexp_exec as *const u8,
        extern_sig: ic_regexp_exec_sig,
        lower: lower_ic_regexp_exec_reject,
    },

    IcEntry {
        key: "writeUInt32BE",
        kind: IcEntryKind::MethodCall { arity: 2 },
        receiver: ReceiverKind::Buffer,
        extern_name: "jit_buffer_write_u32be",
        extern_ptr: crate::deopt::jit_buffer_write_u32be as *const u8,
        extern_sig: ic_buffer_write_u32be_sig,
        lower: lower_ic_buffer_write_u32be_reject,
    },

    IcEntry {
        key: "readUInt32BE",
        kind: IcEntryKind::MethodCall { arity: 1 },
        receiver: ReceiverKind::Buffer,
        extern_name: "jit_buffer_read_u32be",
        extern_ptr: crate::deopt::jit_buffer_read_u32be as *const u8,
        extern_sig: ic_buffer_read_u32be_sig,
        lower: lower_ic_buffer_read_u32be_reject,
    },

    IcEntry {
        key: "writeUInt32LE",
        kind: IcEntryKind::MethodCall { arity: 2 },
        receiver: ReceiverKind::Buffer,
        extern_name: "ic_buffer_write_u32le",
        extern_ptr: ic_buffer_write_u32le as *const u8,
        extern_sig: ic_buffer_write_u32be_sig,
        lower: lower_ic_buffer_write_u32be_reject,
    },

    IcEntry {
        key: "writeUInt8",
        kind: IcEntryKind::MethodCall { arity: 2 },
        receiver: ReceiverKind::Buffer,
        extern_name: "ic_buffer_write_u8",
        extern_ptr: ic_buffer_write_u8 as *const u8,
        extern_sig: ic_buffer_write_u32be_sig,
        lower: lower_ic_buffer_write_u32be_reject,
    },
    IcEntry {
        key: "writeUInt16BE",
        kind: IcEntryKind::MethodCall { arity: 2 },
        receiver: ReceiverKind::Buffer,
        extern_name: "ic_buffer_write_u16be",
        extern_ptr: ic_buffer_write_u16be as *const u8,
        extern_sig: ic_buffer_write_u32be_sig,
        lower: lower_ic_buffer_write_u32be_reject,
    },
    IcEntry {
        key: "writeUInt16LE",
        kind: IcEntryKind::MethodCall { arity: 2 },
        receiver: ReceiverKind::Buffer,
        extern_name: "ic_buffer_write_u16le",
        extern_ptr: ic_buffer_write_u16le as *const u8,
        extern_sig: ic_buffer_write_u32be_sig,
        lower: lower_ic_buffer_write_u32be_reject,
    },

    IcEntry {
        key: "readUInt8",
        kind: IcEntryKind::MethodCall { arity: 1 },
        receiver: ReceiverKind::Buffer,
        extern_name: "ic_buffer_read_u8",
        extern_ptr: ic_buffer_read_u8 as *const u8,
        extern_sig: ic_buffer_read_u32be_sig,
        lower: lower_ic_buffer_read_u32be_reject,
    },
    IcEntry {
        key: "readUInt16BE",
        kind: IcEntryKind::MethodCall { arity: 1 },
        receiver: ReceiverKind::Buffer,
        extern_name: "ic_buffer_read_u16be",
        extern_ptr: ic_buffer_read_u16be as *const u8,
        extern_sig: ic_buffer_read_u32be_sig,
        lower: lower_ic_buffer_read_u32be_reject,
    },
    IcEntry {
        key: "readUInt16LE",
        kind: IcEntryKind::MethodCall { arity: 1 },
        receiver: ReceiverKind::Buffer,
        extern_name: "ic_buffer_read_u16le",
        extern_ptr: ic_buffer_read_u16le as *const u8,
        extern_sig: ic_buffer_read_u32be_sig,
        lower: lower_ic_buffer_read_u32be_reject,
    },
    IcEntry {
        key: "readUInt32LE",
        kind: IcEntryKind::MethodCall { arity: 1 },
        receiver: ReceiverKind::Buffer,
        extern_name: "ic_buffer_read_u32le",
        extern_ptr: ic_buffer_read_u32le as *const u8,
        extern_sig: ic_buffer_read_u32be_sig,
        lower: lower_ic_buffer_read_u32be_reject,
    },
    IcEntry {
        key: "readInt32BE",
        kind: IcEntryKind::MethodCall { arity: 1 },
        receiver: ReceiverKind::Buffer,
        extern_name: "ic_buffer_read_i32be",
        extern_ptr: ic_buffer_read_i32be as *const u8,
        extern_sig: ic_buffer_read_u32be_sig,
        lower: lower_ic_buffer_read_u32be_reject,
    },
];

pub extern "C" fn ic_regexp_exec(receiver_id: i64, arg: f64) -> i64 {
    crate::deopt::call_regexp_exec_ic(receiver_id, arg)
}

pub extern "C" fn ic_regexp_exec_global_object_or_null(receiver_id: i64, arg: f64) -> i64 {
    crate::deopt::call_regexp_exec_global_object_or_null_ic(receiver_id, arg)
}

fn ic_regexp_exec_sig(sig: &mut Signature) {
    sig.params.push(AbiParam::new(I64));
    sig.params.push(AbiParam::new(F64));
    sig.returns.push(AbiParam::new(I64));
}

fn lower_ic_regexp_exec_reject(
    _builder: &mut FunctionBuilder,
    _stack: &mut Vec<ClValue>,
    _extern_ref: FuncRef,
) -> Result<(), String> {
    Err("ic_regexp_exec: object-returning entry requires the mixed-domain exec arm".to_string())
}

pub extern "C" fn ic_buffer_write_u32be(_receiver_id: i64, _value: f64, _offset: f64) -> f64 {
    f64::NAN
}

pub extern "C" fn ic_buffer_write_u32le(_receiver_id: i64, _value: f64, _offset: f64) -> f64 {
    f64::NAN
}

pub extern "C" fn ic_buffer_write_u8(_receiver_id: i64, _value: f64, _offset: f64) -> f64 {
    f64::NAN
}

pub extern "C" fn ic_buffer_write_u16be(_receiver_id: i64, _value: f64, _offset: f64) -> f64 {
    f64::NAN
}

pub extern "C" fn ic_buffer_write_u16le(_receiver_id: i64, _value: f64, _offset: f64) -> f64 {
    f64::NAN
}

fn ic_buffer_write_u32be_sig(sig: &mut Signature) {
    sig.params.push(AbiParam::new(I64));
    sig.params.push(AbiParam::new(F64));
    sig.params.push(AbiParam::new(F64));
    sig.returns.push(AbiParam::new(F64));
}

fn lower_ic_buffer_write_u32be_reject(
    _builder: &mut FunctionBuilder,
    _stack: &mut Vec<ClValue>,
    _extern_ref: FuncRef,
) -> Result<(), String> {
    Err("ic_buffer_write_u32be: Buffer numeric-write lowering requires receiver/value/offset guards".to_string())
}

pub extern "C" fn ic_buffer_read_u32be(_receiver_id: i64, _offset: f64) -> f64 {
    f64::NAN
}

pub extern "C" fn ic_buffer_read_u8(_receiver_id: i64, _offset: f64) -> f64 {
    f64::NAN
}

pub extern "C" fn ic_buffer_read_u16be(_receiver_id: i64, _offset: f64) -> f64 {
    f64::NAN
}

pub extern "C" fn ic_buffer_read_u16le(_receiver_id: i64, _offset: f64) -> f64 {
    f64::NAN
}

pub extern "C" fn ic_buffer_read_u32le(_receiver_id: i64, _offset: f64) -> f64 {
    f64::NAN
}

pub extern "C" fn ic_buffer_read_i32be(_receiver_id: i64, _offset: f64) -> f64 {
    f64::NAN
}

fn ic_buffer_read_u32be_sig(sig: &mut Signature) {
    sig.params.push(AbiParam::new(I64));
    sig.params.push(AbiParam::new(F64));
    sig.returns.push(AbiParam::new(F64));
}

fn lower_ic_buffer_read_u32be_reject(
    _builder: &mut FunctionBuilder,
    _stack: &mut Vec<ClValue>,
    _extern_ref: FuncRef,
) -> Result<(), String> {
    Err(
        "ic_buffer_read_u32be: Buffer numeric-read lowering requires receiver/offset guards"
            .to_string(),
    )
}

pub fn lookup_by_key(key: &str) -> Option<u8> {
    IC_TABLE.iter().position(|e| e.key == key).map(|i| i as u8)
}

pub fn lower_ic_method_resolve(
    builder: &mut FunctionBuilder,
    stack: &mut Vec<ClValue>,
) -> Result<(), String> {
    let _ = stack.pop().ok_or("IcMethodResolve: stack underflow")?;
    let sentinel = builder.ins().f64const(0.0);
    stack.push(sentinel);
    Ok(())
}
