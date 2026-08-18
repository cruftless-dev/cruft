
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeApiReceiver {
    Array,
    ArrayBuffer,
    Buffer,
    DataView,
    String,
    TypedArray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeApiKind {
    Method,
    StaticMethod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeApiAuthority {
    PureNonRetained,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeApiOverrideGuard {
    MethodIdentityUnchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeApiDomain {
    BigInt,
    Boolean,
    Buffer,
    Number,
    String,
    Undefined,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeApiIndexCoercion {
    None,
    ToIntegerOrInfinity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeApiExceptionalResult {
    None,
    ClampedIndex,
    EmptyString,
    NumberNaN,
    RangeError,
    Undefined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeApiEffect {
    Pure,
    MutatesReceiverBytes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeApiEndian {
    None,
    SingleByte,
    Little,
    Big,
    RuntimeFlag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneratedIhiSpec {
    pub key: &'static str,
    pub receiver: crate::interp_ic_table::IhiReceiverKind,
    pub arity: Option<u8>,
    pub cached_id_field: crate::interp_ic_table::IhiCachedField,
    pub fast_fn: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneratedJitIcSpec {
    pub key: &'static str,
    pub receiver: &'static str,
    pub kind: &'static str,
    pub arity: Option<u8>,
    pub extern_name: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneratedLejitExpectationSpec {
    pub key: &'static str,
    pub receiver: rusty_js_jit::ic_table::ReceiverKind,
    pub kind: rusty_js_jit::ic_table::IcEntryKind,
    pub arity: Option<u8>,
    pub extern_name: &'static str,
    pub arg_domains: &'static [rusty_js_jit::ic_table::LejitValueDomain],
    pub return_domain: rusty_js_jit::ic_table::LejitValueDomain,
    pub override_guard: rusty_js_jit::ic_table::LejitOverrideGuard,
    pub deopt_bailouts: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneratedRegistrationSpec {
    pub property: &'static str,
    pub display_name: &'static str,
    pub length: u32,
    pub writable: bool,
    pub enumerable: bool,
    pub configurable: bool,
    pub constructor: bool,
    pub function_prototype: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneratedValidationSpec {
    pub receiver: NativeApiReceiver,
    pub arity: u8,
    pub args: &'static [NativeApiDomain],
    pub returns: &'static [NativeApiDomain],
    pub missing_arg_defaults_to_undefined: bool,
    pub index_coercion: NativeApiIndexCoercion,
    pub negative_or_infinite_result: NativeApiExceptionalResult,
    pub out_of_range_result: NativeApiExceptionalResult,
    pub effect: NativeApiEffect,
    pub argument_shape: &'static str,
    pub encoding_policy: &'static str,
    pub range_window: &'static str,
    pub receiver_semantics: &'static str,
    pub callback_policy: &'static str,
    pub error_codes: &'static [&'static str],
    pub byte_width: u8,
    pub signed: bool,
    pub endian: NativeApiEndian,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneratedFixtureDocCase {
    pub id: &'static str,
    pub surface: &'static str,
    pub expectation: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneratedFixtureDocSpec {
    pub api: &'static str,
    pub cases: &'static [GeneratedFixtureDocCase],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeApiManifestRow {
    pub api: &'static str,
    pub receiver: NativeApiReceiver,
    pub property: &'static str,
    pub kind: NativeApiKind,
    pub arity: u8,
    pub args: &'static [NativeApiDomain],
    pub returns: &'static [NativeApiDomain],
    pub constructor: bool,
    pub authority: NativeApiAuthority,
    pub override_guard: NativeApiOverrideGuard,
    pub slow_path: &'static str,
    pub fast_path: &'static str,
    pub ihi: Option<GeneratedIhiSpec>,
    pub registration: GeneratedRegistrationSpec,
    pub validation: GeneratedValidationSpec,
    pub fixture_doc: GeneratedFixtureDocSpec,
    pub jit_ic: Option<GeneratedJitIcSpec>,
    pub lejit_expectation: Option<GeneratedLejitExpectationSpec>,
    pub bailouts: &'static [&'static str],
    pub fixtures: &'static [&'static str],
}

pub const STRING_CHAR_CODE_AT: NativeApiManifestRow = NativeApiManifestRow {
    api: "String.prototype.charCodeAt",
    receiver: NativeApiReceiver::String,
    property: "charCodeAt",
    kind: NativeApiKind::Method,
    arity: 1,
    args: &[NativeApiDomain::Number],
    returns: &[NativeApiDomain::Number],
    constructor: false,
    authority: NativeApiAuthority::PureNonRetained,
    override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
    slow_path: "generated::string_prototype_char_code_at",
    fast_path: "interp_ic_table::fast_string_char_code_at",
    ihi: crate::native_api_manifest_generated::string_char_code_at_generated_ihi_spec(),
    registration:
        crate::native_api_manifest_generated::string_char_code_at_generated_registration_spec(),
    validation: crate::native_api_manifest_generated::string_char_code_at_generated_validation_spec(),
    fixture_doc: GeneratedFixtureDocSpec {
        api: "String.prototype.charCodeAt",
        cases: &[
            GeneratedFixtureDocCase {
                id: "registration-descriptor",
                surface: "runtime-registration",
                expectation: "property=charCodeAt length=1 writable=true enumerable=false configurable=true constructor=false",
            },
            GeneratedFixtureDocCase {
                id: "validation-binding",
                surface: "runtime-validation",
                expectation: "receiver=string args=[number] return=number missing_arg=undefined nan_for_negative_infinite_or_oob",
            },
            GeneratedFixtureDocCase {
                id: "ihi-generated-consumer",
                surface: "interpreter-hot-intrinsic",
                expectation: "key=charCodeAt receiver=String arity=1 cached_id=StringCharCodeAt",
            },
            GeneratedFixtureDocCase {
                id: "lejit-expectation",
                surface: "lejit",
                expectation: "receiver=String arity=1 arg0=Number return=Number deopt_on_receiver_arg_or_override",
            },
            GeneratedFixtureDocCase {
                id: "cruftscript-stdlib-signature",
                surface: "cruftscript",
                expectation: "string.charCodeAt(number): number rejects_non_number_arg rejects_nullish_receiver",
            },
            GeneratedFixtureDocCase {
                id: "override-bailout",
                surface: "runtime-bailout",
                expectation: "method_identity_unchanged guard preserves user override slow path",
            },
        ],
    },
    jit_ic: crate::native_api_manifest_generated::string_char_code_at_generated_jit_ic_spec(),
    lejit_expectation:
        crate::native_api_manifest_generated::string_char_code_at_generated_lejit_expectation_spec(),
    bailouts: &[
        "receiver-not-string",
        "arity-mismatch",
        "argument-not-number-or-undefined",
        "method-overridden",
        "negative-or-nan-position",
        "function-call-receiver-coercion-stack-overflow",
    ],
    fixtures: &[
        "ordinary-call",
        "override-slow-path",
        "receiver-coercion",
        "argument-edge",
        "ihi-entry-match",
    ],
};

pub const STRING_TO_LOWER_CASE: NativeApiManifestRow = NativeApiManifestRow {
    api: "String.prototype.toLowerCase",
    receiver: NativeApiReceiver::String,
    property: "toLowerCase",
    kind: NativeApiKind::Method,
    arity: 0,
    args: &[],
    returns: &[NativeApiDomain::String],
    constructor: false,
    authority: NativeApiAuthority::PureNonRetained,
    override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
    slow_path: "generated::string_prototype_to_lower_case",
    fast_path: "interp_ic_table::fast_string_to_lower_case",
    ihi: crate::native_api_manifest_generated::string_to_lower_case_generated_ihi_spec(),
    registration:
        crate::native_api_manifest_generated::string_to_lower_case_generated_registration_spec(),
    validation:
        crate::native_api_manifest_generated::string_to_lower_case_generated_validation_spec(),
    fixture_doc: GeneratedFixtureDocSpec {
        api: "String.prototype.toLowerCase",
        cases: &[],
    },
    jit_ic: crate::native_api_manifest_generated::string_to_lower_case_generated_jit_ic_spec(),
    lejit_expectation:
        crate::native_api_manifest_generated::string_to_lower_case_generated_lejit_expectation_spec(
        ),
    bailouts: &["receiver-not-string", "arity-mismatch", "method-overridden"],
    fixtures: &["ordinary-call", "override-slow-path", "ihi-entry-match"],
};

pub const STRING_TO_UPPER_CASE: NativeApiManifestRow = NativeApiManifestRow {
    api: "String.prototype.toUpperCase",
    receiver: NativeApiReceiver::String,
    property: "toUpperCase",
    kind: NativeApiKind::Method,
    arity: 0,
    args: &[],
    returns: &[NativeApiDomain::String],
    constructor: false,
    authority: NativeApiAuthority::PureNonRetained,
    override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
    slow_path: "generated::string_prototype_to_upper_case",
    fast_path: "interp_ic_table::fast_string_to_upper_case",
    ihi: crate::native_api_manifest_generated::string_to_upper_case_generated_ihi_spec(),
    registration:
        crate::native_api_manifest_generated::string_to_upper_case_generated_registration_spec(),
    validation:
        crate::native_api_manifest_generated::string_to_upper_case_generated_validation_spec(),
    fixture_doc: GeneratedFixtureDocSpec {
        api: "String.prototype.toUpperCase",
        cases: &[],
    },
    jit_ic: crate::native_api_manifest_generated::string_to_upper_case_generated_jit_ic_spec(),
    lejit_expectation:
        crate::native_api_manifest_generated::string_to_upper_case_generated_lejit_expectation_spec(
        ),
    bailouts: &["receiver-not-string", "arity-mismatch", "method-overridden"],
    fixtures: &["ordinary-call", "override-slow-path", "ihi-entry-match"],
};

pub const STRING_TRIM: NativeApiManifestRow = NativeApiManifestRow {
    api: "String.prototype.trim",
    receiver: NativeApiReceiver::String,
    property: "trim",
    kind: NativeApiKind::Method,
    arity: 0,
    args: &[],
    returns: &[NativeApiDomain::String],
    constructor: false,
    authority: NativeApiAuthority::PureNonRetained,
    override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
    slow_path: "generated::string_prototype_trim",
    fast_path: "interp_ic_table::fast_string_trim",
    ihi: crate::native_api_manifest_generated::string_trim_generated_ihi_spec(),
    registration: crate::native_api_manifest_generated::string_trim_generated_registration_spec(),
    validation: crate::native_api_manifest_generated::string_trim_generated_validation_spec(),
    fixture_doc: GeneratedFixtureDocSpec {
        api: "String.prototype.trim",
        cases: &[],
    },
    jit_ic: crate::native_api_manifest_generated::string_trim_generated_jit_ic_spec(),
    lejit_expectation:
        crate::native_api_manifest_generated::string_trim_generated_lejit_expectation_spec(),
    bailouts: &["receiver-not-string", "arity-mismatch", "method-overridden"],
    fixtures: &["ordinary-call", "override-slow-path", "ihi-entry-match"],
};

pub const STRING_TRIM_START: NativeApiManifestRow = NativeApiManifestRow {
    api: "String.prototype.trimStart",
    receiver: NativeApiReceiver::String,
    property: "trimStart",
    kind: NativeApiKind::Method,
    arity: 0,
    args: &[],
    returns: &[NativeApiDomain::String],
    constructor: false,
    authority: NativeApiAuthority::PureNonRetained,
    override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
    slow_path: "generated::string_prototype_trim_start",
    fast_path: "None",
    ihi: crate::native_api_manifest_generated::string_trim_start_generated_ihi_spec(),
    registration:
        crate::native_api_manifest_generated::string_trim_start_generated_registration_spec(),
    validation: crate::native_api_manifest_generated::string_trim_start_generated_validation_spec(),
    fixture_doc: GeneratedFixtureDocSpec {
        api: "String.prototype.trimStart",
        cases: &[],
    },
    jit_ic: crate::native_api_manifest_generated::string_trim_start_generated_jit_ic_spec(),
    lejit_expectation:
        crate::native_api_manifest_generated::string_trim_start_generated_lejit_expectation_spec(),
    bailouts: &["receiver-not-string", "arity-mismatch", "method-overridden"],
    fixtures: &["ordinary-call", "override-slow-path", "no-fast-path"],
};

pub const STRING_TRIM_END: NativeApiManifestRow = NativeApiManifestRow {
    api: "String.prototype.trimEnd",
    receiver: NativeApiReceiver::String,
    property: "trimEnd",
    kind: NativeApiKind::Method,
    arity: 0,
    args: &[],
    returns: &[NativeApiDomain::String],
    constructor: false,
    authority: NativeApiAuthority::PureNonRetained,
    override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
    slow_path: "generated::string_prototype_trim_end",
    fast_path: "None",
    ihi: crate::native_api_manifest_generated::string_trim_end_generated_ihi_spec(),
    registration: crate::native_api_manifest_generated::string_trim_end_generated_registration_spec(
    ),
    validation: crate::native_api_manifest_generated::string_trim_end_generated_validation_spec(),
    fixture_doc: GeneratedFixtureDocSpec {
        api: "String.prototype.trimEnd",
        cases: &[],
    },
    jit_ic: crate::native_api_manifest_generated::string_trim_end_generated_jit_ic_spec(),
    lejit_expectation:
        crate::native_api_manifest_generated::string_trim_end_generated_lejit_expectation_spec(),
    bailouts: &["receiver-not-string", "arity-mismatch", "method-overridden"],
    fixtures: &["ordinary-call", "override-slow-path", "no-fast-path"],
};

pub const STRING_REPEAT: NativeApiManifestRow = NativeApiManifestRow {
    api: "String.prototype.repeat",
    receiver: NativeApiReceiver::String,
    property: "repeat",
    kind: NativeApiKind::Method,
    arity: 1,
    args: &[NativeApiDomain::Number],
    returns: &[NativeApiDomain::String],
    constructor: false,
    authority: NativeApiAuthority::PureNonRetained,
    override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
    slow_path: "generated::string_prototype_repeat",
    fast_path: "None",
    ihi: crate::native_api_manifest_generated::string_repeat_generated_ihi_spec(),
    registration: crate::native_api_manifest_generated::string_repeat_generated_registration_spec(),
    validation: crate::native_api_manifest_generated::string_repeat_generated_validation_spec(),
    fixture_doc: GeneratedFixtureDocSpec {
        api: "String.prototype.repeat",
        cases: &[],
    },
    jit_ic: crate::native_api_manifest_generated::string_repeat_generated_jit_ic_spec(),
    lejit_expectation:
        crate::native_api_manifest_generated::string_repeat_generated_lejit_expectation_spec(),
    bailouts: &["receiver-not-string", "arity-mismatch", "method-overridden"],
    fixtures: &["ordinary-call", "override-slow-path", "no-fast-path"],
};

pub const STRING_PAD_START: NativeApiManifestRow = NativeApiManifestRow {
    api: "String.prototype.padStart",
    receiver: NativeApiReceiver::String,
    property: "padStart",
    kind: NativeApiKind::Method,
    arity: 1,
    args: &[NativeApiDomain::Number],
    returns: &[NativeApiDomain::String],
    constructor: false,
    authority: NativeApiAuthority::PureNonRetained,
    override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
    slow_path: "generated::string_prototype_pad_start",
    fast_path: "None",
    ihi: crate::native_api_manifest_generated::string_pad_start_generated_ihi_spec(),
    registration:
        crate::native_api_manifest_generated::string_pad_start_generated_registration_spec(),
    validation: crate::native_api_manifest_generated::string_pad_start_generated_validation_spec(),
    fixture_doc: GeneratedFixtureDocSpec {
        api: "String.prototype.padStart",
        cases: &[],
    },
    jit_ic: crate::native_api_manifest_generated::string_pad_start_generated_jit_ic_spec(),
    lejit_expectation:
        crate::native_api_manifest_generated::string_pad_start_generated_lejit_expectation_spec(),
    bailouts: &["receiver-not-string", "arity-mismatch", "method-overridden"],
    fixtures: &["ordinary-call", "override-slow-path", "no-fast-path"],
};

pub const STRING_PAD_END: NativeApiManifestRow = NativeApiManifestRow {
    api: "String.prototype.padEnd",
    receiver: NativeApiReceiver::String,
    property: "padEnd",
    kind: NativeApiKind::Method,
    arity: 1,
    args: &[NativeApiDomain::Number],
    returns: &[NativeApiDomain::String],
    constructor: false,
    authority: NativeApiAuthority::PureNonRetained,
    override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
    slow_path: "generated::string_prototype_pad_end",
    fast_path: "None",
    ihi: crate::native_api_manifest_generated::string_pad_end_generated_ihi_spec(),
    registration: crate::native_api_manifest_generated::string_pad_end_generated_registration_spec(
    ),
    validation: crate::native_api_manifest_generated::string_pad_end_generated_validation_spec(),
    fixture_doc: GeneratedFixtureDocSpec {
        api: "String.prototype.padEnd",
        cases: &[],
    },
    jit_ic: crate::native_api_manifest_generated::string_pad_end_generated_jit_ic_spec(),
    lejit_expectation:
        crate::native_api_manifest_generated::string_pad_end_generated_lejit_expectation_spec(),
    bailouts: &["receiver-not-string", "arity-mismatch", "method-overridden"],
    fixtures: &["ordinary-call", "override-slow-path", "no-fast-path"],
};

pub const STRING_INCLUDES: NativeApiManifestRow = NativeApiManifestRow {
    api: "String.prototype.includes",
    receiver: NativeApiReceiver::String,
    property: "includes",
    kind: NativeApiKind::Method,
    arity: 1,
    args: &[NativeApiDomain::String],
    returns: &[NativeApiDomain::Boolean],
    constructor: false,
    authority: NativeApiAuthority::PureNonRetained,
    override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
    slow_path: "generated::string_prototype_includes",
    fast_path: "interp_ic_table::fast_string_includes",
    ihi: crate::native_api_manifest_generated::string_includes_generated_ihi_spec(),
    registration: crate::native_api_manifest_generated::string_includes_generated_registration_spec(
    ),
    validation: crate::native_api_manifest_generated::string_includes_generated_validation_spec(),
    fixture_doc: GeneratedFixtureDocSpec {
        api: "String.prototype.includes",
        cases: &[],
    },
    jit_ic: crate::native_api_manifest_generated::string_includes_generated_jit_ic_spec(),
    lejit_expectation:
        crate::native_api_manifest_generated::string_includes_generated_lejit_expectation_spec(),
    bailouts: &["receiver-not-string", "arity-mismatch", "method-overridden"],
    fixtures: &["ordinary-call", "override-slow-path", "ihi-entry-match"],
};

pub const STRING_STARTS_WITH: NativeApiManifestRow = NativeApiManifestRow {
    api: "String.prototype.startsWith",
    receiver: NativeApiReceiver::String,
    property: "startsWith",
    kind: NativeApiKind::Method,
    arity: 1,
    args: &[NativeApiDomain::String],
    returns: &[NativeApiDomain::Boolean],
    constructor: false,
    authority: NativeApiAuthority::PureNonRetained,
    override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
    slow_path: "generated::string_prototype_starts_with",
    fast_path: "interp_ic_table::fast_string_starts_with",
    ihi: crate::native_api_manifest_generated::string_starts_with_generated_ihi_spec(),
    registration:
        crate::native_api_manifest_generated::string_starts_with_generated_registration_spec(),
    validation: crate::native_api_manifest_generated::string_starts_with_generated_validation_spec(
    ),
    fixture_doc: GeneratedFixtureDocSpec {
        api: "String.prototype.startsWith",
        cases: &[],
    },
    jit_ic: crate::native_api_manifest_generated::string_starts_with_generated_jit_ic_spec(),
    lejit_expectation:
        crate::native_api_manifest_generated::string_starts_with_generated_lejit_expectation_spec(),
    bailouts: &["receiver-not-string", "arity-mismatch", "method-overridden"],
    fixtures: &["ordinary-call", "override-slow-path", "ihi-entry-match"],
};

pub const STRING_ENDS_WITH: NativeApiManifestRow = NativeApiManifestRow {
    api: "String.prototype.endsWith",
    receiver: NativeApiReceiver::String,
    property: "endsWith",
    kind: NativeApiKind::Method,
    arity: 1,
    args: &[NativeApiDomain::String],
    returns: &[NativeApiDomain::Boolean],
    constructor: false,
    authority: NativeApiAuthority::PureNonRetained,
    override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
    slow_path: "generated::string_prototype_ends_with",
    fast_path: "interp_ic_table::fast_string_ends_with",
    ihi: crate::native_api_manifest_generated::string_ends_with_generated_ihi_spec(),
    registration:
        crate::native_api_manifest_generated::string_ends_with_generated_registration_spec(),
    validation: crate::native_api_manifest_generated::string_ends_with_generated_validation_spec(),
    fixture_doc: GeneratedFixtureDocSpec {
        api: "String.prototype.endsWith",
        cases: &[],
    },
    jit_ic: crate::native_api_manifest_generated::string_ends_with_generated_jit_ic_spec(),
    lejit_expectation:
        crate::native_api_manifest_generated::string_ends_with_generated_lejit_expectation_spec(),
    bailouts: &["receiver-not-string", "arity-mismatch", "method-overridden"],
    fixtures: &["ordinary-call", "override-slow-path", "ihi-entry-match"],
};

pub const STRING_INDEX_OF: NativeApiManifestRow = NativeApiManifestRow {
    api: "String.prototype.indexOf",
    receiver: NativeApiReceiver::String,
    property: "indexOf",
    kind: NativeApiKind::Method,
    arity: 1,
    args: &[NativeApiDomain::String],
    returns: &[NativeApiDomain::Number],
    constructor: false,
    authority: NativeApiAuthority::PureNonRetained,
    override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
    slow_path: "generated::string_prototype_index_of",
    fast_path: "interp_ic_table::fast_string_index_of_1",
    ihi: crate::native_api_manifest_generated::string_index_of_generated_ihi_spec(),
    registration: crate::native_api_manifest_generated::string_index_of_generated_registration_spec(
    ),
    validation: crate::native_api_manifest_generated::string_index_of_generated_validation_spec(),
    fixture_doc: GeneratedFixtureDocSpec {
        api: "String.prototype.indexOf",
        cases: &[],
    },
    jit_ic: crate::native_api_manifest_generated::string_index_of_generated_jit_ic_spec(),
    lejit_expectation:
        crate::native_api_manifest_generated::string_index_of_generated_lejit_expectation_spec(),
    bailouts: &["receiver-not-string", "arity-mismatch", "method-overridden"],
    fixtures: &["ordinary-call", "override-slow-path", "ihi-entry-match"],
};

pub const STRING_LAST_INDEX_OF: NativeApiManifestRow = NativeApiManifestRow {
    api: "String.prototype.lastIndexOf",
    receiver: NativeApiReceiver::String,
    property: "lastIndexOf",
    kind: NativeApiKind::Method,
    arity: 1,
    args: &[NativeApiDomain::String],
    returns: &[NativeApiDomain::Number],
    constructor: false,
    authority: NativeApiAuthority::PureNonRetained,
    override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
    slow_path: "generated::string_prototype_last_index_of",
    fast_path: "None",
    ihi: crate::native_api_manifest_generated::string_last_index_of_generated_ihi_spec(),
    registration:
        crate::native_api_manifest_generated::string_last_index_of_generated_registration_spec(),
    validation:
        crate::native_api_manifest_generated::string_last_index_of_generated_validation_spec(),
    fixture_doc: GeneratedFixtureDocSpec {
        api: "String.prototype.lastIndexOf",
        cases: &[],
    },
    jit_ic: crate::native_api_manifest_generated::string_last_index_of_generated_jit_ic_spec(),
    lejit_expectation:
        crate::native_api_manifest_generated::string_last_index_of_generated_lejit_expectation_spec(
        ),
    bailouts: &["receiver-not-string", "arity-mismatch", "method-overridden"],
    fixtures: &["ordinary-call", "override-slow-path", "no-fast-path"],
};

pub const STRING_CHAR_AT: NativeApiManifestRow = NativeApiManifestRow {
    api: "String.prototype.charAt",
    receiver: NativeApiReceiver::String,
    property: "charAt",
    kind: NativeApiKind::Method,
    arity: 1,
    args: &[NativeApiDomain::Number],
    returns: &[NativeApiDomain::String],
    constructor: false,
    authority: NativeApiAuthority::PureNonRetained,
    override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
    slow_path: "generated::string_prototype_char_at",
    fast_path: "None",
    ihi: crate::native_api_manifest_generated::string_char_at_generated_ihi_spec(),
    registration: crate::native_api_manifest_generated::string_char_at_generated_registration_spec(
    ),
    validation: crate::native_api_manifest_generated::string_char_at_generated_validation_spec(),
    fixture_doc: GeneratedFixtureDocSpec {
        api: "String.prototype.charAt",
        cases: &[],
    },
    jit_ic: crate::native_api_manifest_generated::string_char_at_generated_jit_ic_spec(),
    lejit_expectation:
        crate::native_api_manifest_generated::string_char_at_generated_lejit_expectation_spec(),
    bailouts: &["receiver-not-string", "arity-mismatch", "method-overridden"],
    fixtures: &["ordinary-call", "override-slow-path", "no-fast-path"],
};

pub const STRING_AT: NativeApiManifestRow = NativeApiManifestRow {
    api: "String.prototype.at",
    receiver: NativeApiReceiver::String,
    property: "at",
    kind: NativeApiKind::Method,
    arity: 1,
    args: &[NativeApiDomain::Number],
    returns: &[NativeApiDomain::String, NativeApiDomain::Undefined],
    constructor: false,
    authority: NativeApiAuthority::PureNonRetained,
    override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
    slow_path: "generated::string_prototype_at",
    fast_path: "None",
    ihi: crate::native_api_manifest_generated::string_at_generated_ihi_spec(),
    registration: crate::native_api_manifest_generated::string_at_generated_registration_spec(),
    validation: crate::native_api_manifest_generated::string_at_generated_validation_spec(),
    fixture_doc: GeneratedFixtureDocSpec {
        api: "String.prototype.at",
        cases: &[],
    },
    jit_ic: crate::native_api_manifest_generated::string_at_generated_jit_ic_spec(),
    lejit_expectation:
        crate::native_api_manifest_generated::string_at_generated_lejit_expectation_spec(),
    bailouts: &["receiver-not-string", "arity-mismatch", "method-overridden"],
    fixtures: &["ordinary-call", "override-slow-path", "no-fast-path"],
};

pub const STRING_SLICE: NativeApiManifestRow = NativeApiManifestRow {
    api: "String.prototype.slice",
    receiver: NativeApiReceiver::String,
    property: "slice",
    kind: NativeApiKind::Method,
    arity: 2,
    args: &[NativeApiDomain::Number, NativeApiDomain::Number],
    returns: &[NativeApiDomain::String],
    constructor: false,
    authority: NativeApiAuthority::PureNonRetained,
    override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
    slow_path: "generated::string_prototype_slice",
    fast_path: "None",
    ihi: crate::native_api_manifest_generated::string_slice_generated_ihi_spec(),
    registration: crate::native_api_manifest_generated::string_slice_generated_registration_spec(),
    validation: crate::native_api_manifest_generated::string_slice_generated_validation_spec(),
    fixture_doc: GeneratedFixtureDocSpec {
        api: "String.prototype.slice",
        cases: &[],
    },
    jit_ic: crate::native_api_manifest_generated::string_slice_generated_jit_ic_spec(),
    lejit_expectation:
        crate::native_api_manifest_generated::string_slice_generated_lejit_expectation_spec(),
    bailouts: &["receiver-not-string", "arity-mismatch", "method-overridden"],
    fixtures: &["ordinary-call", "override-slow-path", "no-fast-path"],
};

pub const STRING_SUBSTRING: NativeApiManifestRow = NativeApiManifestRow {
    api: "String.prototype.substring",
    receiver: NativeApiReceiver::String,
    property: "substring",
    kind: NativeApiKind::Method,
    arity: 2,
    args: &[NativeApiDomain::Number, NativeApiDomain::Number],
    returns: &[NativeApiDomain::String],
    constructor: false,
    authority: NativeApiAuthority::PureNonRetained,
    override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
    slow_path: "generated::string_prototype_substring",
    fast_path: "None",
    ihi: crate::native_api_manifest_generated::string_substring_generated_ihi_spec(),
    registration:
        crate::native_api_manifest_generated::string_substring_generated_registration_spec(),
    validation: crate::native_api_manifest_generated::string_substring_generated_validation_spec(),
    fixture_doc: GeneratedFixtureDocSpec {
        api: "String.prototype.substring",
        cases: &[],
    },
    jit_ic: crate::native_api_manifest_generated::string_substring_generated_jit_ic_spec(),
    lejit_expectation:
        crate::native_api_manifest_generated::string_substring_generated_lejit_expectation_spec(),
    bailouts: &["receiver-not-string", "arity-mismatch", "method-overridden"],
    fixtures: &["ordinary-call", "override-slow-path", "no-fast-path"],
};

pub const STRING_SUBSTR: NativeApiManifestRow = NativeApiManifestRow {
    api: "String.prototype.substr",
    receiver: NativeApiReceiver::String,
    property: "substr",
    kind: NativeApiKind::Method,
    arity: 2,
    args: &[NativeApiDomain::Number, NativeApiDomain::Number],
    returns: &[NativeApiDomain::String],
    constructor: false,
    authority: NativeApiAuthority::PureNonRetained,
    override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
    slow_path: "generated::string_prototype_substr",
    fast_path: "None",
    ihi: crate::native_api_manifest_generated::string_substr_generated_ihi_spec(),
    registration: crate::native_api_manifest_generated::string_substr_generated_registration_spec(),
    validation: crate::native_api_manifest_generated::string_substr_generated_validation_spec(),
    fixture_doc: GeneratedFixtureDocSpec {
        api: "String.prototype.substr",
        cases: &[],
    },
    jit_ic: crate::native_api_manifest_generated::string_substr_generated_jit_ic_spec(),
    lejit_expectation:
        crate::native_api_manifest_generated::string_substr_generated_lejit_expectation_spec(),
    bailouts: &["receiver-not-string", "arity-mismatch", "method-overridden"],
    fixtures: &["ordinary-call", "override-slow-path", "no-fast-path"],
};

pub const STRING_CODE_POINT_AT: NativeApiManifestRow = NativeApiManifestRow {
    api: "String.prototype.codePointAt",
    receiver: NativeApiReceiver::String,
    property: "codePointAt",
    kind: NativeApiKind::Method,
    arity: 1,
    args: &[NativeApiDomain::Number],
    returns: &[NativeApiDomain::Number, NativeApiDomain::Undefined],
    constructor: false,
    authority: NativeApiAuthority::PureNonRetained,
    override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
    slow_path: "generated::string_prototype_code_point_at",
    fast_path: "interp_ic_table::fast_string_code_point_at",
    ihi: crate::native_api_manifest_generated::string_code_point_at_generated_ihi_spec(),
    registration:
        crate::native_api_manifest_generated::string_code_point_at_generated_registration_spec(),
    validation:
        crate::native_api_manifest_generated::string_code_point_at_generated_validation_spec(),
    fixture_doc: GeneratedFixtureDocSpec {
        api: "String.prototype.codePointAt",
        cases: &[],
    },
    jit_ic: crate::native_api_manifest_generated::string_code_point_at_generated_jit_ic_spec(),
    lejit_expectation:
        crate::native_api_manifest_generated::string_code_point_at_generated_lejit_expectation_spec(
        ),
    bailouts: &[
        "receiver-not-string",
        "arity-mismatch",
        "argument-not-number-or-undefined",
        "method-overridden",
        "negative-or-infinite-or-out-of-range-position",
    ],
    fixtures: &[
        "ordinary-call",
        "undefined-out-of-range",
        "override-slow-path",
        "ihi-entry-match",
        "jit-ic-entry-match",
    ],
};

macro_rules! buffer_byte_row {
    (
        $name:ident,
        $api:literal,
        $property:literal,
        $arity:literal,
        [$($arg:expr),* $(,)?],
        $signed:expr,
        $effect:expr,
        $slow:literal,
        $fast:literal,
        $ihi:expr,
        $registration:expr,
        $validation:expr,
        $jit:expr,
        $lejit:expr,
        [$($bailout:literal),* $(,)?],
        [$($fixture:literal),* $(,)?]
    ) => {
        pub const $name: NativeApiManifestRow = NativeApiManifestRow {
            api: $api,
            receiver: NativeApiReceiver::Buffer,
            property: $property,
            kind: NativeApiKind::Method,
            arity: $arity,
            args: &[$($arg),*],
            returns: &[NativeApiDomain::Number],
            constructor: false,
            authority: NativeApiAuthority::PureNonRetained,
            override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
            slow_path: $slow,
            fast_path: $fast,
            ihi: $ihi,
            registration: $registration,
            validation: $validation,
            fixture_doc: GeneratedFixtureDocSpec {
                api: $api,
                cases: &[],
            },
            jit_ic: $jit,
            lejit_expectation: $lejit,
            bailouts: &[$($bailout),*],
            fixtures: &[$($fixture),*],
        };
    };
}

macro_rules! buffer_bigint_row {
    (
        $name:ident,
        $api:literal,
        $property:literal,
        $arity:literal,
        [$($arg:expr),* $(,)?],
        [$($ret:expr),* $(,)?],
        $signed:expr,
        $effect:expr,
        $slow:literal,
        $fast:literal,
        $ihi:expr,
        $registration:expr,
        $validation:expr,
        $jit:expr,
        $lejit:expr,
        [$($bailout:literal),* $(,)?],
        [$($fixture:literal),* $(,)?]
    ) => {
        pub const $name: NativeApiManifestRow = NativeApiManifestRow {
            api: $api,
            receiver: NativeApiReceiver::Buffer,
            property: $property,
            kind: NativeApiKind::Method,
            arity: $arity,
            args: &[$($arg),*],
            returns: &[$($ret),*],
            constructor: false,
            authority: NativeApiAuthority::PureNonRetained,
            override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
            slow_path: $slow,
            fast_path: $fast,
            ihi: $ihi,
            registration: $registration,
            validation: $validation,
            fixture_doc: GeneratedFixtureDocSpec {
                api: $api,
                cases: &[],
            },
            jit_ic: $jit,
            lejit_expectation: $lejit,
            bailouts: &[$($bailout),*],
            fixtures: &[$($fixture),*],
        };
    };
}

buffer_byte_row!(
    BUFFER_READ_UINT8,
    "Buffer.prototype.readUInt8",
    "readUInt8",
    1,
    [NativeApiDomain::Number],
    false,
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.readUInt8",
    "interp_ic_table::fast_buffer_read_u8",
    crate::native_api_manifest_generated::buffer_read_uint8_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_read_uint8_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_uint8_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_read_uint8_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_read_uint8_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "arity-mismatch",
        "offset-not-number-or-undefined",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "offset-bounds-reject",
        "invalid-receiver-reject",
        "ihi-entry-match",
        "jit-ic-entry-match"
    ]
);

buffer_byte_row!(
    BUFFER_READ_INT8,
    "Buffer.prototype.readInt8",
    "readInt8",
    1,
    [NativeApiDomain::Number],
    true,
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.readInt8",
    "interp_ic_table::fast_buffer_read_i8",
    crate::native_api_manifest_generated::buffer_read_int8_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_read_int8_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_int8_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_read_int8_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_read_int8_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "arity-mismatch",
        "offset-not-number-or-undefined",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "signed-byte-read",
        "offset-bounds-reject",
        "invalid-receiver-reject",
        "ihi-entry-match"
    ]
);

buffer_byte_row!(
    BUFFER_WRITE_UINT8,
    "Buffer.prototype.writeUInt8",
    "writeUInt8",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    false,
    NativeApiEffect::MutatesReceiverBytes,
    "rusty_js_runtime::intrinsics::Buffer.prototype.writeUInt8",
    "interp_ic_table::fast_buffer_write_u8",
    crate::native_api_manifest_generated::buffer_write_uint8_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_write_uint8_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_uint8_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_write_uint8_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_write_uint8_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "arity-mismatch",
        "value-not-number-or-undefined",
        "offset-not-number-or-undefined",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "mutation-effect",
        "offset-bounds-reject",
        "invalid-receiver-reject",
        "ihi-entry-match",
        "jit-ic-entry-match"
    ]
);

buffer_byte_row!(
    BUFFER_WRITE_INT8,
    "Buffer.prototype.writeInt8",
    "writeInt8",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    true,
    NativeApiEffect::MutatesReceiverBytes,
    "rusty_js_runtime::intrinsics::Buffer.prototype.writeInt8",
    "interp_ic_table::fast_buffer_write_i8",
    crate::native_api_manifest_generated::buffer_write_int8_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_write_int8_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_int8_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_write_int8_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_write_int8_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "arity-mismatch",
        "value-not-number-or-undefined",
        "offset-not-number-or-undefined",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "signed-byte-write",
        "mutation-effect",
        "offset-bounds-reject",
        "invalid-receiver-reject",
        "ihi-entry-match"
    ]
);

buffer_byte_row!(
    BUFFER_READ_UINT16_LE,
    "Buffer.prototype.readUInt16LE",
    "readUInt16LE",
    1,
    [NativeApiDomain::Number],
    false,
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.readUInt16LE",
    "interp_ic_table::fast_buffer_read_u16le",
    crate::native_api_manifest_generated::buffer_read_uint16_le_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_read_uint16_le_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_uint16_le_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_read_uint16_le_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_read_uint16_le_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "endian-little-read",
        "ihi-entry-match",
        "jit-ic-entry-match"
    ]
);

buffer_byte_row!(
    BUFFER_READ_UINT16_BE,
    "Buffer.prototype.readUInt16BE",
    "readUInt16BE",
    1,
    [NativeApiDomain::Number],
    false,
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.readUInt16BE",
    "interp_ic_table::fast_buffer_read_u16be",
    crate::native_api_manifest_generated::buffer_read_uint16_be_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_read_uint16_be_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_uint16_be_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_read_uint16_be_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_read_uint16_be_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "endian-big-read",
        "ihi-entry-match",
        "jit-ic-entry-match"
    ]
);

buffer_byte_row!(
    BUFFER_READ_INT16_LE,
    "Buffer.prototype.readInt16LE",
    "readInt16LE",
    1,
    [NativeApiDomain::Number],
    true,
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.readInt16LE",
    "interp_ic_table::fast_buffer_read_i16le",
    crate::native_api_manifest_generated::buffer_read_int16_le_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_read_int16_le_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_int16_le_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_read_int16_le_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_read_int16_le_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "signed-endian-little-read",
        "ihi-entry-match"
    ]
);

buffer_byte_row!(
    BUFFER_READ_INT16_BE,
    "Buffer.prototype.readInt16BE",
    "readInt16BE",
    1,
    [NativeApiDomain::Number],
    true,
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.readInt16BE",
    "interp_ic_table::fast_buffer_read_i16be",
    crate::native_api_manifest_generated::buffer_read_int16_be_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_read_int16_be_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_int16_be_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_read_int16_be_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_read_int16_be_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-out-of-range"
    ],
    ["ordinary-call", "signed-endian-big-read", "ihi-entry-match"]
);

buffer_byte_row!(
    BUFFER_WRITE_UINT16_LE,
    "Buffer.prototype.writeUInt16LE",
    "writeUInt16LE",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    false,
    NativeApiEffect::MutatesReceiverBytes,
    "rusty_js_runtime::intrinsics::Buffer.prototype.writeUInt16LE",
    "interp_ic_table::fast_buffer_write_u16le",
    crate::native_api_manifest_generated::buffer_write_uint16_le_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_write_uint16_le_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_uint16_le_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_write_uint16_le_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_write_uint16_le_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-or-value-out-of-range"
    ],
    [
        "ordinary-call",
        "endian-little-mutation",
        "ihi-entry-match",
        "jit-ic-entry-match"
    ]
);

buffer_byte_row!(
    BUFFER_WRITE_UINT16_BE,
    "Buffer.prototype.writeUInt16BE",
    "writeUInt16BE",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    false,
    NativeApiEffect::MutatesReceiverBytes,
    "rusty_js_runtime::intrinsics::Buffer.prototype.writeUInt16BE",
    "interp_ic_table::fast_buffer_write_u16be",
    crate::native_api_manifest_generated::buffer_write_uint16_be_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_write_uint16_be_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_uint16_be_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_write_uint16_be_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_write_uint16_be_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-or-value-out-of-range"
    ],
    [
        "ordinary-call",
        "endian-big-mutation",
        "ihi-entry-match",
        "jit-ic-entry-match"
    ]
);

buffer_byte_row!(
    BUFFER_WRITE_INT16_LE,
    "Buffer.prototype.writeInt16LE",
    "writeInt16LE",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    true,
    NativeApiEffect::MutatesReceiverBytes,
    "rusty_js_runtime::intrinsics::Buffer.prototype.writeInt16LE",
    "interp_ic_table::fast_buffer_write_i16le",
    crate::native_api_manifest_generated::buffer_write_int16_le_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_write_int16_le_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_int16_le_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_write_int16_le_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_write_int16_le_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-or-value-out-of-range"
    ],
    [
        "ordinary-call",
        "signed-endian-little-mutation",
        "ihi-entry-match"
    ]
);

buffer_byte_row!(
    BUFFER_WRITE_INT16_BE,
    "Buffer.prototype.writeInt16BE",
    "writeInt16BE",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    true,
    NativeApiEffect::MutatesReceiverBytes,
    "rusty_js_runtime::intrinsics::Buffer.prototype.writeInt16BE",
    "interp_ic_table::fast_buffer_write_i16be",
    crate::native_api_manifest_generated::buffer_write_int16_be_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_write_int16_be_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_int16_be_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_write_int16_be_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_write_int16_be_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-or-value-out-of-range"
    ],
    [
        "ordinary-call",
        "signed-endian-big-mutation",
        "ihi-entry-match"
    ]
);

buffer_byte_row!(
    BUFFER_READ_UINT32_LE,
    "Buffer.prototype.readUInt32LE",
    "readUInt32LE",
    1,
    [NativeApiDomain::Number],
    false,
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.readUInt32LE",
    "interp_ic_table::fast_buffer_read_u32le",
    crate::native_api_manifest_generated::buffer_read_uint32_le_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_read_uint32_le_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_uint32_le_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_read_uint32_le_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_read_uint32_le_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "endian-little-read",
        "ihi-entry-match",
        "jit-ic-entry-match"
    ]
);

buffer_byte_row!(
    BUFFER_READ_UINT32_BE,
    "Buffer.prototype.readUInt32BE",
    "readUInt32BE",
    1,
    [NativeApiDomain::Number],
    false,
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.readUInt32BE",
    "interp_ic_table::fast_buffer_read_u32be",
    crate::native_api_manifest_generated::buffer_read_uint32_be_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_read_uint32_be_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_uint32_be_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_read_uint32_be_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_read_uint32_be_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "endian-big-read",
        "ihi-entry-match",
        "jit-ic-entry-match"
    ]
);

buffer_byte_row!(
    BUFFER_READ_INT32_LE,
    "Buffer.prototype.readInt32LE",
    "readInt32LE",
    1,
    [NativeApiDomain::Number],
    true,
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.readInt32LE",
    "interp_ic_table::fast_buffer_read_i32le",
    crate::native_api_manifest_generated::buffer_read_int32_le_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_read_int32_le_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_int32_le_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_read_int32_le_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_read_int32_le_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "signed-endian-little-read",
        "ihi-entry-match"
    ]
);

buffer_byte_row!(
    BUFFER_READ_INT32_BE,
    "Buffer.prototype.readInt32BE",
    "readInt32BE",
    1,
    [NativeApiDomain::Number],
    true,
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.readInt32BE",
    "interp_ic_table::fast_buffer_read_i32be",
    crate::native_api_manifest_generated::buffer_read_int32_be_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_read_int32_be_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_int32_be_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_read_int32_be_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_read_int32_be_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "signed-endian-big-read",
        "ihi-entry-match",
        "jit-ic-entry-match"
    ]
);

buffer_byte_row!(
    BUFFER_WRITE_UINT32_LE,
    "Buffer.prototype.writeUInt32LE",
    "writeUInt32LE",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    false,
    NativeApiEffect::MutatesReceiverBytes,
    "rusty_js_runtime::intrinsics::Buffer.prototype.writeUInt32LE",
    "interp_ic_table::fast_buffer_write_u32le",
    crate::native_api_manifest_generated::buffer_write_uint32_le_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_write_uint32_le_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_uint32_le_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_write_uint32_le_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_write_uint32_le_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-or-value-out-of-range"
    ],
    [
        "ordinary-call",
        "endian-little-mutation",
        "ihi-entry-match",
        "jit-ic-entry-match"
    ]
);

buffer_byte_row!(
    BUFFER_WRITE_UINT32_BE,
    "Buffer.prototype.writeUInt32BE",
    "writeUInt32BE",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    false,
    NativeApiEffect::MutatesReceiverBytes,
    "rusty_js_runtime::intrinsics::Buffer.prototype.writeUInt32BE",
    "interp_ic_table::fast_buffer_write_u32be",
    crate::native_api_manifest_generated::buffer_write_uint32_be_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_write_uint32_be_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_uint32_be_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_write_uint32_be_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_write_uint32_be_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-or-value-out-of-range"
    ],
    [
        "ordinary-call",
        "endian-big-mutation",
        "ihi-entry-match",
        "jit-ic-entry-match"
    ]
);

buffer_byte_row!(
    BUFFER_WRITE_INT32_LE,
    "Buffer.prototype.writeInt32LE",
    "writeInt32LE",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    true,
    NativeApiEffect::MutatesReceiverBytes,
    "rusty_js_runtime::intrinsics::Buffer.prototype.writeInt32LE",
    "interp_ic_table::fast_buffer_write_i32le",
    crate::native_api_manifest_generated::buffer_write_int32_le_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_write_int32_le_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_int32_le_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_write_int32_le_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_write_int32_le_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-or-value-out-of-range"
    ],
    [
        "ordinary-call",
        "signed-endian-little-mutation",
        "ihi-entry-match"
    ]
);

buffer_byte_row!(
    BUFFER_WRITE_INT32_BE,
    "Buffer.prototype.writeInt32BE",
    "writeInt32BE",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    true,
    NativeApiEffect::MutatesReceiverBytes,
    "rusty_js_runtime::intrinsics::Buffer.prototype.writeInt32BE",
    "interp_ic_table::fast_buffer_write_i32be",
    crate::native_api_manifest_generated::buffer_write_int32_be_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_write_int32_be_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_int32_be_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_write_int32_be_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_write_int32_be_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-or-value-out-of-range"
    ],
    [
        "ordinary-call",
        "signed-endian-big-mutation",
        "ihi-entry-match"
    ]
);

buffer_byte_row!(
    BUFFER_READ_FLOAT_LE,
    "Buffer.prototype.readFloatLE",
    "readFloatLE",
    1,
    [NativeApiDomain::Number],
    true,
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.readFloatLE",
    "interp_ic_table::fast_buffer_read_f32le",
    crate::native_api_manifest_generated::buffer_read_float_le_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_read_float_le_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_float_le_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_read_float_le_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_read_float_le_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "ieee-float32-little-read",
        "ihi-entry-match"
    ]
);
buffer_byte_row!(
    BUFFER_READ_FLOAT_BE,
    "Buffer.prototype.readFloatBE",
    "readFloatBE",
    1,
    [NativeApiDomain::Number],
    true,
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.readFloatBE",
    "interp_ic_table::fast_buffer_read_f32be",
    crate::native_api_manifest_generated::buffer_read_float_be_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_read_float_be_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_float_be_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_read_float_be_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_read_float_be_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-out-of-range"
    ],
    ["ordinary-call", "ieee-float32-big-read", "ihi-entry-match"]
);
buffer_byte_row!(
    BUFFER_READ_DOUBLE_LE,
    "Buffer.prototype.readDoubleLE",
    "readDoubleLE",
    1,
    [NativeApiDomain::Number],
    true,
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.readDoubleLE",
    "interp_ic_table::fast_buffer_read_f64le",
    crate::native_api_manifest_generated::buffer_read_double_le_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_read_double_le_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_double_le_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_read_double_le_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_read_double_le_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "ieee-float64-little-read",
        "ihi-entry-match"
    ]
);
buffer_byte_row!(
    BUFFER_READ_DOUBLE_BE,
    "Buffer.prototype.readDoubleBE",
    "readDoubleBE",
    1,
    [NativeApiDomain::Number],
    true,
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.readDoubleBE",
    "interp_ic_table::fast_buffer_read_f64be",
    crate::native_api_manifest_generated::buffer_read_double_be_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_read_double_be_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_double_be_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_read_double_be_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_read_double_be_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-out-of-range"
    ],
    ["ordinary-call", "ieee-float64-big-read", "ihi-entry-match"]
);
buffer_byte_row!(
    BUFFER_WRITE_FLOAT_LE,
    "Buffer.prototype.writeFloatLE",
    "writeFloatLE",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    true,
    NativeApiEffect::MutatesReceiverBytes,
    "rusty_js_runtime::intrinsics::Buffer.prototype.writeFloatLE",
    "interp_ic_table::fast_buffer_write_f32le",
    crate::native_api_manifest_generated::buffer_write_float_le_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_write_float_le_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_float_le_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_write_float_le_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_write_float_le_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "ieee-float32-little-mutation",
        "ihi-entry-match"
    ]
);
buffer_byte_row!(
    BUFFER_WRITE_FLOAT_BE,
    "Buffer.prototype.writeFloatBE",
    "writeFloatBE",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    true,
    NativeApiEffect::MutatesReceiverBytes,
    "rusty_js_runtime::intrinsics::Buffer.prototype.writeFloatBE",
    "interp_ic_table::fast_buffer_write_f32be",
    crate::native_api_manifest_generated::buffer_write_float_be_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_write_float_be_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_float_be_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_write_float_be_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_write_float_be_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "ieee-float32-big-mutation",
        "ihi-entry-match"
    ]
);
buffer_byte_row!(
    BUFFER_WRITE_DOUBLE_LE,
    "Buffer.prototype.writeDoubleLE",
    "writeDoubleLE",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    true,
    NativeApiEffect::MutatesReceiverBytes,
    "rusty_js_runtime::intrinsics::Buffer.prototype.writeDoubleLE",
    "interp_ic_table::fast_buffer_write_f64le",
    crate::native_api_manifest_generated::buffer_write_double_le_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_write_double_le_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_double_le_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_write_double_le_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_write_double_le_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "ieee-float64-little-mutation",
        "ihi-entry-match"
    ]
);
buffer_byte_row!(
    BUFFER_WRITE_DOUBLE_BE,
    "Buffer.prototype.writeDoubleBE",
    "writeDoubleBE",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    true,
    NativeApiEffect::MutatesReceiverBytes,
    "rusty_js_runtime::intrinsics::Buffer.prototype.writeDoubleBE",
    "interp_ic_table::fast_buffer_write_f64be",
    crate::native_api_manifest_generated::buffer_write_double_be_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_write_double_be_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_double_be_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_write_double_be_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_write_double_be_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "ieee-float64-big-mutation",
        "ihi-entry-match"
    ]
);

macro_rules! buffer_variable_numeric_row {
    (
        $name:ident,
        $api:literal,
        $property:literal,
        $arity:literal,
        [$($arg:expr),* $(,)?],
        $effect:expr,
        $registration:expr,
        $validation:expr,
        [$($bailout:literal),* $(,)?],
        [$($fixture:literal),* $(,)?]
    ) => {
        pub const $name: NativeApiManifestRow = NativeApiManifestRow {
            api: $api,
            receiver: NativeApiReceiver::Buffer,
            property: $property,
            kind: NativeApiKind::Method,
            arity: $arity,
            args: &[$($arg),*],
            returns: &[NativeApiDomain::Number],
            constructor: false,
            authority: NativeApiAuthority::PureNonRetained,
            override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
            slow_path: $api,
            fast_path: "None",
            ihi: None,
            registration: $registration,
            validation: $validation,
            fixture_doc: GeneratedFixtureDocSpec {
                api: $api,
                cases: &[],
            },
            jit_ic: None,
            lejit_expectation: None,
            bailouts: &[$($bailout),*],
            fixtures: &[$($fixture),*],
        };
    };
}

buffer_variable_numeric_row!(
    BUFFER_READ_UINT_LE,
    "Buffer.prototype.readUIntLE",
    "readUIntLE",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    NativeApiEffect::Pure,
    crate::native_api_manifest_generated::buffer_read_uint_le_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_uint_le_generated_validation_spec(),
    [
        "receiver-not-buffer",
        "offset-not-number-or-undefined",
        "byteLength-not-number",
        "byteLength-out-of-range",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "variable-little-read",
        "strict-byteLength",
        "override-slow-path"
    ]
);

buffer_variable_numeric_row!(
    BUFFER_READ_UINT_BE,
    "Buffer.prototype.readUIntBE",
    "readUIntBE",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    NativeApiEffect::Pure,
    crate::native_api_manifest_generated::buffer_read_uint_be_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_uint_be_generated_validation_spec(),
    [
        "receiver-not-buffer",
        "offset-not-number-or-undefined",
        "byteLength-not-number",
        "byteLength-out-of-range",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "variable-big-read",
        "strict-byteLength",
        "override-slow-path"
    ]
);

buffer_variable_numeric_row!(
    BUFFER_READ_INT_LE,
    "Buffer.prototype.readIntLE",
    "readIntLE",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    NativeApiEffect::Pure,
    crate::native_api_manifest_generated::buffer_read_int_le_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_int_le_generated_validation_spec(),
    [
        "receiver-not-buffer",
        "offset-not-number-or-undefined",
        "byteLength-not-number",
        "byteLength-out-of-range",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "signed-variable-little-read",
        "strict-byteLength",
        "override-slow-path"
    ]
);

buffer_variable_numeric_row!(
    BUFFER_READ_INT_BE,
    "Buffer.prototype.readIntBE",
    "readIntBE",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    NativeApiEffect::Pure,
    crate::native_api_manifest_generated::buffer_read_int_be_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_int_be_generated_validation_spec(),
    [
        "receiver-not-buffer",
        "offset-not-number-or-undefined",
        "byteLength-not-number",
        "byteLength-out-of-range",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "signed-variable-big-read",
        "strict-byteLength",
        "override-slow-path"
    ]
);

buffer_variable_numeric_row!(
    BUFFER_WRITE_UINT_LE,
    "Buffer.prototype.writeUIntLE",
    "writeUIntLE",
    3,
    [
        NativeApiDomain::Number,
        NativeApiDomain::Number,
        NativeApiDomain::Number
    ],
    NativeApiEffect::MutatesReceiverBytes,
    crate::native_api_manifest_generated::buffer_write_uint_le_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_uint_le_generated_validation_spec(),
    [
        "receiver-not-buffer",
        "value-out-of-range",
        "offset-not-number-or-undefined",
        "byteLength-not-number",
        "byteLength-out-of-range",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "variable-little-mutation",
        "strict-byteLength",
        "override-slow-path"
    ]
);

buffer_variable_numeric_row!(
    BUFFER_WRITE_UINT_BE,
    "Buffer.prototype.writeUIntBE",
    "writeUIntBE",
    3,
    [
        NativeApiDomain::Number,
        NativeApiDomain::Number,
        NativeApiDomain::Number
    ],
    NativeApiEffect::MutatesReceiverBytes,
    crate::native_api_manifest_generated::buffer_write_uint_be_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_uint_be_generated_validation_spec(),
    [
        "receiver-not-buffer",
        "value-out-of-range",
        "offset-not-number-or-undefined",
        "byteLength-not-number",
        "byteLength-out-of-range",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "variable-big-mutation",
        "strict-byteLength",
        "override-slow-path"
    ]
);

buffer_variable_numeric_row!(
    BUFFER_WRITE_INT_LE,
    "Buffer.prototype.writeIntLE",
    "writeIntLE",
    3,
    [
        NativeApiDomain::Number,
        NativeApiDomain::Number,
        NativeApiDomain::Number
    ],
    NativeApiEffect::MutatesReceiverBytes,
    crate::native_api_manifest_generated::buffer_write_int_le_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_int_le_generated_validation_spec(),
    [
        "receiver-not-buffer",
        "value-out-of-range",
        "offset-not-number-or-undefined",
        "byteLength-not-number",
        "byteLength-out-of-range",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "signed-variable-little-mutation",
        "strict-byteLength",
        "override-slow-path"
    ]
);

buffer_variable_numeric_row!(
    BUFFER_WRITE_INT_BE,
    "Buffer.prototype.writeIntBE",
    "writeIntBE",
    3,
    [
        NativeApiDomain::Number,
        NativeApiDomain::Number,
        NativeApiDomain::Number
    ],
    NativeApiEffect::MutatesReceiverBytes,
    crate::native_api_manifest_generated::buffer_write_int_be_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_int_be_generated_validation_spec(),
    [
        "receiver-not-buffer",
        "value-out-of-range",
        "offset-not-number-or-undefined",
        "byteLength-not-number",
        "byteLength-out-of-range",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "signed-variable-big-mutation",
        "strict-byteLength",
        "override-slow-path"
    ]
);

macro_rules! buffer_proto_helper_row {
    (
        $name:ident,
        $api:literal,
        $property:literal,
        $arity:literal,
        [$($arg:expr),* $(,)?],
        [$($ret:expr),* $(,)?],
        $effect:expr,
        [$($bailout:literal),* $(,)?],
        [$($fixture:literal),* $(,)?]
    ) => {
        pub const $name: NativeApiManifestRow = NativeApiManifestRow {
            api: $api,
            receiver: NativeApiReceiver::Buffer,
            property: $property,
            kind: NativeApiKind::Method,
            arity: $arity,
            args: &[$($arg),*],
            returns: &[$($ret),*],
            constructor: false,
            authority: NativeApiAuthority::PureNonRetained,
            override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
            slow_path: $api,
            fast_path: "None",
            ihi: None,
            registration: GeneratedRegistrationSpec {
                property: $property,
                display_name: $property,
                length: $arity,
                writable: true,
                enumerable: true,
                configurable: true,
                constructor: false,
                function_prototype: true,
            },
            validation: GeneratedValidationSpec {
                receiver: NativeApiReceiver::Buffer,
                arity: $arity as u8,
                args: &[$($arg),*],
                returns: &[$($ret),*],
                missing_arg_defaults_to_undefined: true,
                index_coercion: NativeApiIndexCoercion::ToIntegerOrInfinity,
                negative_or_infinite_result: NativeApiExceptionalResult::RangeError,
                out_of_range_result: NativeApiExceptionalResult::RangeError,
                effect: $effect,
                argument_shape: "legacy Buffer helper",
                encoding_policy: "Node Buffer helper semantics",
                range_window: "Buffer byte window",
                receiver_semantics: "ordinary",
                callback_policy: "none",
                error_codes: &[],
                byte_width: 0,
                signed: false,
                endian: NativeApiEndian::None,
            },
            fixture_doc: GeneratedFixtureDocSpec {
                api: $api,
                cases: &[],
            },
            jit_ic: None,
            lejit_expectation: None,
            bailouts: &[$($bailout),*],
            fixtures: &[$($fixture),*],
        };
    };
}

buffer_proto_helper_row!(
    BUFFER_SWAP16,
    "Buffer.prototype.swap16",
    "swap16",
    0,
    [],
    [NativeApiDomain::Buffer],
    NativeApiEffect::MutatesReceiverBytes,
    [
        "receiver-not-buffer",
        "length-not-multiple-of-2",
        "method-overridden"
    ],
    [
        "descriptor-shape",
        "swap-mutation",
        "range-reject",
        "consumer-ineligible"
    ]
);

buffer_proto_helper_row!(
    BUFFER_SWAP32,
    "Buffer.prototype.swap32",
    "swap32",
    0,
    [],
    [NativeApiDomain::Buffer],
    NativeApiEffect::MutatesReceiverBytes,
    [
        "receiver-not-buffer",
        "length-not-multiple-of-4",
        "method-overridden"
    ],
    [
        "descriptor-shape",
        "swap-mutation",
        "range-reject",
        "consumer-ineligible"
    ]
);

buffer_proto_helper_row!(
    BUFFER_SWAP64,
    "Buffer.prototype.swap64",
    "swap64",
    0,
    [],
    [NativeApiDomain::Buffer],
    NativeApiEffect::MutatesReceiverBytes,
    [
        "receiver-not-buffer",
        "length-not-multiple-of-8",
        "method-overridden"
    ],
    [
        "descriptor-shape",
        "swap-mutation",
        "range-reject",
        "consumer-ineligible"
    ]
);

buffer_proto_helper_row!(
    BUFFER_TO_JSON,
    "Buffer.prototype.toJSON",
    "toJSON",
    0,
    [],
    [NativeApiDomain::Buffer],
    NativeApiEffect::Pure,
    ["receiver-not-buffer", "method-overridden"],
    [
        "descriptor-shape",
        "json-shape",
        "receiver-compatibility",
        "consumer-ineligible"
    ]
);

buffer_proto_helper_row!(
    BUFFER_UTF8_SLICE,
    "Buffer.prototype.utf8Slice",
    "utf8Slice",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::String],
    NativeApiEffect::Pure,
    ["receiver-not-buffer", "method-overridden"],
    [
        "descriptor-shape",
        "decode-window",
        "receiver-compatibility",
        "consumer-ineligible"
    ]
);

buffer_proto_helper_row!(
    BUFFER_LATIN1_SLICE,
    "Buffer.prototype.latin1Slice",
    "latin1Slice",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::String],
    NativeApiEffect::Pure,
    ["receiver-not-buffer", "method-overridden"],
    [
        "descriptor-shape",
        "decode-window",
        "receiver-compatibility",
        "consumer-ineligible"
    ]
);

buffer_proto_helper_row!(
    BUFFER_ASCII_SLICE,
    "Buffer.prototype.asciiSlice",
    "asciiSlice",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::String],
    NativeApiEffect::Pure,
    ["receiver-not-buffer", "method-overridden"],
    [
        "descriptor-shape",
        "decode-window",
        "receiver-compatibility",
        "consumer-ineligible"
    ]
);

buffer_proto_helper_row!(
    BUFFER_UCS2_SLICE,
    "Buffer.prototype.ucs2Slice",
    "ucs2Slice",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::String],
    NativeApiEffect::Pure,
    ["receiver-not-buffer", "method-overridden"],
    [
        "descriptor-shape",
        "decode-window",
        "receiver-compatibility",
        "consumer-ineligible"
    ]
);

buffer_proto_helper_row!(
    BUFFER_BASE64_SLICE,
    "Buffer.prototype.base64Slice",
    "base64Slice",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::String],
    NativeApiEffect::Pure,
    ["receiver-not-buffer", "method-overridden"],
    [
        "descriptor-shape",
        "decode-window",
        "receiver-compatibility",
        "consumer-ineligible"
    ]
);

buffer_bigint_row!(
    BUFFER_READ_BIG_UINT64_LE,
    "Buffer.prototype.readBigUInt64LE",
    "readBigUInt64LE",
    1,
    [NativeApiDomain::Number],
    [NativeApiDomain::BigInt],
    false,
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.readBigUInt64LE",
    "interp_ic_table::fast_buffer_read_big_u64le",
    crate::native_api_manifest_generated::buffer_read_big_uint64_le_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_read_big_uint64_le_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_big_uint64_le_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_read_big_uint64_le_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_read_big_uint64_le_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "arity-mismatch",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "unsigned-little-read",
        "bigint-return",
        "ihi-entry-match"
    ]
);

buffer_bigint_row!(
    BUFFER_READ_BIG_UINT64_BE,
    "Buffer.prototype.readBigUInt64BE",
    "readBigUInt64BE",
    1,
    [NativeApiDomain::Number],
    [NativeApiDomain::BigInt],
    false,
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.readBigUInt64BE",
    "interp_ic_table::fast_buffer_read_big_u64be",
    crate::native_api_manifest_generated::buffer_read_big_uint64_be_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_read_big_uint64_be_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_big_uint64_be_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_read_big_uint64_be_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_read_big_uint64_be_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "arity-mismatch",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "unsigned-big-read",
        "bigint-return",
        "ihi-entry-match"
    ]
);

buffer_bigint_row!(
    BUFFER_READ_BIG_INT64_LE,
    "Buffer.prototype.readBigInt64LE",
    "readBigInt64LE",
    1,
    [NativeApiDomain::Number],
    [NativeApiDomain::BigInt],
    true,
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.readBigInt64LE",
    "interp_ic_table::fast_buffer_read_big_i64le",
    crate::native_api_manifest_generated::buffer_read_big_int64_le_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_read_big_int64_le_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_big_int64_le_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_read_big_int64_le_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_read_big_int64_le_generated_lejit_expectation_spec(
    ),
    [
        "receiver-not-buffer",
        "arity-mismatch",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "signed-little-read",
        "negative-bigint-return",
        "ihi-entry-match"
    ]
);

buffer_bigint_row!(
    BUFFER_READ_BIG_INT64_BE,
    "Buffer.prototype.readBigInt64BE",
    "readBigInt64BE",
    1,
    [NativeApiDomain::Number],
    [NativeApiDomain::BigInt],
    true,
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.readBigInt64BE",
    "interp_ic_table::fast_buffer_read_big_i64be",
    crate::native_api_manifest_generated::buffer_read_big_int64_be_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_read_big_int64_be_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_read_big_int64_be_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_read_big_int64_be_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_read_big_int64_be_generated_lejit_expectation_spec(
    ),
    [
        "receiver-not-buffer",
        "arity-mismatch",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "signed-big-read",
        "negative-bigint-return",
        "ihi-entry-match"
    ]
);

buffer_bigint_row!(
    BUFFER_WRITE_BIG_UINT64_LE,
    "Buffer.prototype.writeBigUInt64LE",
    "writeBigUInt64LE",
    2,
    [NativeApiDomain::BigInt, NativeApiDomain::Number],
    [NativeApiDomain::Number],
    false,
    NativeApiEffect::MutatesReceiverBytes,
    "rusty_js_runtime::intrinsics::Buffer.prototype.writeBigUInt64LE",
    "interp_ic_table::fast_buffer_write_big_u64le",
    crate::native_api_manifest_generated::buffer_write_big_uint64_le_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_write_big_uint64_le_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_big_uint64_le_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_write_big_uint64_le_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_write_big_uint64_le_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "arity-mismatch",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "unsigned-little-mutation",
        "bigint-value-required",
        "ihi-entry-match"
    ]
);

buffer_bigint_row!(
    BUFFER_WRITE_BIG_UINT64_BE,
    "Buffer.prototype.writeBigUInt64BE",
    "writeBigUInt64BE",
    2,
    [NativeApiDomain::BigInt, NativeApiDomain::Number],
    [NativeApiDomain::Number],
    false,
    NativeApiEffect::MutatesReceiverBytes,
    "rusty_js_runtime::intrinsics::Buffer.prototype.writeBigUInt64BE",
    "interp_ic_table::fast_buffer_write_big_u64be",
    crate::native_api_manifest_generated::buffer_write_big_uint64_be_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_write_big_uint64_be_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_big_uint64_be_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_write_big_uint64_be_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_write_big_uint64_be_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "arity-mismatch",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "unsigned-big-mutation",
        "bigint-value-required",
        "ihi-entry-match"
    ]
);

buffer_bigint_row!(
    BUFFER_WRITE_BIG_INT64_LE,
    "Buffer.prototype.writeBigInt64LE",
    "writeBigInt64LE",
    2,
    [NativeApiDomain::BigInt, NativeApiDomain::Number],
    [NativeApiDomain::Number],
    true,
    NativeApiEffect::MutatesReceiverBytes,
    "rusty_js_runtime::intrinsics::Buffer.prototype.writeBigInt64LE",
    "interp_ic_table::fast_buffer_write_big_i64le",
    crate::native_api_manifest_generated::buffer_write_big_int64_le_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_write_big_int64_le_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_big_int64_le_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_write_big_int64_le_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_write_big_int64_le_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "arity-mismatch",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "signed-little-mutation",
        "bigint-value-required",
        "ihi-entry-match"
    ]
);

buffer_bigint_row!(
    BUFFER_WRITE_BIG_INT64_BE,
    "Buffer.prototype.writeBigInt64BE",
    "writeBigInt64BE",
    2,
    [NativeApiDomain::BigInt, NativeApiDomain::Number],
    [NativeApiDomain::Number],
    true,
    NativeApiEffect::MutatesReceiverBytes,
    "rusty_js_runtime::intrinsics::Buffer.prototype.writeBigInt64BE",
    "interp_ic_table::fast_buffer_write_big_i64be",
    crate::native_api_manifest_generated::buffer_write_big_int64_be_generated_ihi_spec(),
    crate::native_api_manifest_generated::buffer_write_big_int64_be_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_big_int64_be_generated_validation_spec(),
    crate::native_api_manifest_generated::buffer_write_big_int64_be_generated_jit_ic_spec(),
    crate::native_api_manifest_generated::buffer_write_big_int64_be_generated_lejit_expectation_spec(),
    [
        "receiver-not-buffer",
        "arity-mismatch",
        "method-overridden",
        "offset-out-of-range"
    ],
    [
        "ordinary-call",
        "signed-big-mutation",
        "bigint-value-required",
        "ihi-entry-match"
    ]
);

macro_rules! buffer_string_row {
    (
        $name:ident,
        $api:literal,
        $property:literal,
        $kind:expr,
        $arity:literal,
        [$($arg:expr),* $(,)?],
        [$($ret:expr),* $(,)?],
        $effect:expr,
        $slow:literal,
        $registration:expr,
        $validation:expr,
        [$($bailout:literal),* $(,)?],
        [$($fixture:literal),* $(,)?]
    ) => {
        pub const $name: NativeApiManifestRow = NativeApiManifestRow {
            api: $api,
            receiver: NativeApiReceiver::Buffer,
            property: $property,
            kind: $kind,
            arity: $arity,
            args: &[$($arg),*],
            returns: &[$($ret),*],
            constructor: true,
            authority: NativeApiAuthority::PureNonRetained,
            override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
            slow_path: $slow,
            fast_path: "None",
            ihi: None,
            registration: $registration,
            validation: $validation,
            fixture_doc: GeneratedFixtureDocSpec {
                api: $api,
                cases: &[],
            },
            jit_ic: None,
            lejit_expectation: None,
            bailouts: &[$($bailout),*],
            fixtures: &[$($fixture),*],
        };
    };
}

buffer_string_row!(
    BUFFER_TO_STRING,
    "Buffer.prototype.toString",
    "toString",
    NativeApiKind::Method,
    3,
    [
        NativeApiDomain::String,
        NativeApiDomain::Number,
        NativeApiDomain::Number
    ],
    [NativeApiDomain::String],
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.toString",
    crate::native_api_manifest_generated::buffer_to_string_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_to_string_generated_validation_spec(),
    [
        "receiver-not-buffer",
        "encoding-unknown",
        "method-overridden",
        "range-window-clamped"
    ],
    [
        "descriptor-shape",
        "encoding-labels",
        "unknown-encoding-reject",
        "range-window",
        "consumer-ineligible"
    ]
);

buffer_string_row!(
    BUFFER_WRITE,
    "Buffer.prototype.write",
    "write",
    NativeApiKind::Method,
    4,
    [
        NativeApiDomain::String,
        NativeApiDomain::Number,
        NativeApiDomain::Number,
        NativeApiDomain::String
    ],
    [NativeApiDomain::Number],
    NativeApiEffect::MutatesReceiverBytes,
    "rusty_js_runtime::intrinsics::Buffer.prototype.write",
    crate::native_api_manifest_generated::buffer_write_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_write_generated_validation_spec(),
    [
        "receiver-not-buffer",
        "encoding-unknown",
        "offset-out-of-range",
        "method-overridden"
    ],
    [
        "descriptor-shape",
        "mutation-return",
        "unknown-encoding-reject",
        "offset-range",
        "consumer-ineligible"
    ]
);

buffer_string_row!(
    BUFFER_BYTE_LENGTH,
    "Buffer.byteLength",
    "byteLength",
    NativeApiKind::StaticMethod,
    2,
    [NativeApiDomain::String, NativeApiDomain::String],
    [NativeApiDomain::Number],
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.byteLength",
    crate::native_api_manifest_generated::buffer_static_byte_length_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_static_byte_length_generated_validation_spec(),
    ["invalid-arg-type"],
    [
        "descriptor-shape",
        "nominal-sizing",
        "object-byte-length",
        "invalid-argument",
        "consumer-ineligible"
    ]
);

buffer_string_row!(
    BUFFER_IS_ENCODING,
    "Buffer.isEncoding",
    "isEncoding",
    NativeApiKind::StaticMethod,
    1,
    [NativeApiDomain::String],
    [NativeApiDomain::Boolean],
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.isEncoding",
    crate::native_api_manifest_generated::buffer_static_is_encoding_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_static_is_encoding_generated_validation_spec(),
    [],
    [
        "descriptor-shape",
        "encoding-labels",
        "unknown-label",
        "non-string-false",
        "consumer-ineligible"
    ]
);

buffer_string_row!(
    BUFFER_FROM,
    "Buffer.from",
    "from",
    NativeApiKind::StaticMethod,
    3,
    [
        NativeApiDomain::String,
        NativeApiDomain::String,
        NativeApiDomain::Undefined
    ],
    [NativeApiDomain::Buffer],
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.from",
    crate::native_api_manifest_generated::buffer_static_from_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_static_from_generated_validation_spec(),
    ["encoding-unknown", "invalid-arg-type"],
    [
        "descriptor-shape",
        "string-encoding",
        "unknown-encoding-reject",
        "missing-first-argument",
        "consumer-ineligible"
    ]
);

buffer_string_row!(
    BUFFER_INCLUDES,
    "Buffer.prototype.includes",
    "includes",
    NativeApiKind::Method,
    3,
    [
        NativeApiDomain::Buffer,
        NativeApiDomain::Number,
        NativeApiDomain::String
    ],
    [NativeApiDomain::Boolean],
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.includes",
    crate::native_api_manifest_generated::buffer_includes_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_includes_generated_validation_spec(),
    ["encoding-unknown", "invalid-arg-type", "method-overridden"],
    [
        "descriptor-shape",
        "needle-coercion",
        "offset-window",
        "unknown-encoding-reject",
        "consumer-ineligible"
    ]
);

buffer_string_row!(
    BUFFER_INDEX_OF,
    "Buffer.prototype.indexOf",
    "indexOf",
    NativeApiKind::Method,
    3,
    [
        NativeApiDomain::Buffer,
        NativeApiDomain::Number,
        NativeApiDomain::String
    ],
    [NativeApiDomain::Number],
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.indexOf",
    crate::native_api_manifest_generated::buffer_index_of_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_index_of_generated_validation_spec(),
    ["encoding-unknown", "invalid-arg-type", "method-overridden"],
    [
        "descriptor-shape",
        "needle-coercion",
        "offset-window",
        "not-found-minus-one",
        "consumer-ineligible"
    ]
);

buffer_string_row!(
    BUFFER_LAST_INDEX_OF,
    "Buffer.prototype.lastIndexOf",
    "lastIndexOf",
    NativeApiKind::Method,
    3,
    [
        NativeApiDomain::Buffer,
        NativeApiDomain::Number,
        NativeApiDomain::String
    ],
    [NativeApiDomain::Number],
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.lastIndexOf",
    crate::native_api_manifest_generated::buffer_last_index_of_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_last_index_of_generated_validation_spec(),
    ["encoding-unknown", "invalid-arg-type", "method-overridden"],
    [
        "descriptor-shape",
        "needle-coercion",
        "reverse-offset-window",
        "not-found-minus-one",
        "consumer-ineligible"
    ]
);

buffer_string_row!(
    BUFFER_EQUALS,
    "Buffer.prototype.equals",
    "equals",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Buffer],
    [NativeApiDomain::Boolean],
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.equals",
    crate::native_api_manifest_generated::buffer_equals_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_equals_generated_validation_spec(),
    ["invalid-arg-type", "method-overridden"],
    [
        "descriptor-shape",
        "buffersource-target",
        "string-rejects",
        "equality-result",
        "consumer-ineligible"
    ]
);

buffer_string_row!(
    BUFFER_COMPARE,
    "Buffer.prototype.compare",
    "compare",
    NativeApiKind::Method,
    5,
    [
        NativeApiDomain::Buffer,
        NativeApiDomain::Number,
        NativeApiDomain::Number,
        NativeApiDomain::Number,
        NativeApiDomain::Number
    ],
    [NativeApiDomain::Number],
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.compare",
    crate::native_api_manifest_generated::buffer_compare_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_compare_generated_validation_spec(),
    ["invalid-arg-type", "method-overridden"],
    [
        "descriptor-shape",
        "buffersource-target",
        "range-window",
        "ordering-result",
        "consumer-ineligible"
    ]
);

buffer_string_row!(
    BUFFER_STATIC_COMPARE,
    "Buffer.compare",
    "compare",
    NativeApiKind::StaticMethod,
    2,
    [NativeApiDomain::Buffer, NativeApiDomain::Buffer],
    [NativeApiDomain::Number],
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.compare",
    crate::native_api_manifest_generated::buffer_static_compare_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_static_compare_generated_validation_spec(),
    ["invalid-arg-type"],
    [
        "descriptor-shape",
        "buffersource-operands",
        "string-rejects",
        "ordering-result",
        "consumer-ineligible"
    ]
);

buffer_string_row!(
    BUFFER_COPY,
    "Buffer.prototype.copy",
    "copy",
    NativeApiKind::Method,
    4,
    [
        NativeApiDomain::Buffer,
        NativeApiDomain::Number,
        NativeApiDomain::Number,
        NativeApiDomain::Number
    ],
    [NativeApiDomain::Number],
    NativeApiEffect::MutatesReceiverBytes,
    "rusty_js_runtime::intrinsics::Buffer.prototype.copy",
    crate::native_api_manifest_generated::buffer_copy_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_copy_generated_validation_spec(),
    ["invalid-arg-type", "method-overridden"],
    [
        "descriptor-shape",
        "target-validation",
        "copy-window",
        "mutation-return",
        "consumer-ineligible"
    ]
);

buffer_string_row!(
    BUFFER_FILL,
    "Buffer.prototype.fill",
    "fill",
    NativeApiKind::Method,
    4,
    [
        NativeApiDomain::Buffer,
        NativeApiDomain::Number,
        NativeApiDomain::Number,
        NativeApiDomain::String
    ],
    [NativeApiDomain::Buffer],
    NativeApiEffect::MutatesReceiverBytes,
    "rusty_js_runtime::intrinsics::Buffer.prototype.fill",
    crate::native_api_manifest_generated::buffer_fill_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_fill_generated_validation_spec(),
    ["encoding-unknown", "invalid-arg-type", "method-overridden"],
    [
        "descriptor-shape",
        "fill-cycling",
        "encoding-reject",
        "mutation-return-this",
        "consumer-ineligible"
    ]
);

buffer_string_row!(
    BUFFER_SLICE,
    "Buffer.prototype.slice",
    "slice",
    NativeApiKind::Method,
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::Buffer],
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.slice",
    crate::native_api_manifest_generated::buffer_slice_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_slice_generated_validation_spec(),
    ["method-overridden"],
    [
        "descriptor-shape",
        "view-alias",
        "negative-offset",
        "buffer-return",
        "consumer-ineligible"
    ]
);

buffer_string_row!(
    BUFFER_SUBARRAY,
    "Buffer.prototype.subarray",
    "subarray",
    NativeApiKind::Method,
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::Buffer],
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.prototype.subarray",
    crate::native_api_manifest_generated::buffer_subarray_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_subarray_generated_validation_spec(),
    ["method-overridden"],
    [
        "descriptor-shape",
        "view-alias",
        "negative-offset",
        "buffer-return",
        "consumer-ineligible"
    ]
);

buffer_string_row!(
    BUFFER_ALLOC,
    "Buffer.alloc",
    "alloc",
    NativeApiKind::StaticMethod,
    3,
    [
        NativeApiDomain::Number,
        NativeApiDomain::Buffer,
        NativeApiDomain::String
    ],
    [NativeApiDomain::Buffer],
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.alloc",
    crate::native_api_manifest_generated::buffer_static_alloc_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_static_alloc_generated_validation_spec(),
    ["out-of-range", "invalid-arg-type", "encoding-unknown"],
    [
        "descriptor-shape",
        "size-validation",
        "fill-cycling",
        "encoding-reject",
        "consumer-ineligible"
    ]
);

buffer_string_row!(
    BUFFER_ALLOC_UNSAFE,
    "Buffer.allocUnsafe",
    "allocUnsafe",
    NativeApiKind::StaticMethod,
    1,
    [NativeApiDomain::Number],
    [NativeApiDomain::Buffer],
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.allocUnsafe",
    crate::native_api_manifest_generated::buffer_static_alloc_unsafe_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_static_alloc_unsafe_generated_validation_spec(),
    ["out-of-range", "invalid-arg-type"],
    [
        "descriptor-shape",
        "size-validation",
        "buffer-length",
        "constructable",
        "consumer-ineligible"
    ]
);

buffer_string_row!(
    BUFFER_ALLOC_UNSAFE_SLOW,
    "Buffer.allocUnsafeSlow",
    "allocUnsafeSlow",
    NativeApiKind::StaticMethod,
    1,
    [NativeApiDomain::Number],
    [NativeApiDomain::Buffer],
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.allocUnsafeSlow",
    crate::native_api_manifest_generated::
        buffer_static_alloc_unsafe_slow_generated_registration_spec(),
    crate::native_api_manifest_generated::
        buffer_static_alloc_unsafe_slow_generated_validation_spec(),
    ["out-of-range", "invalid-arg-type"],
    [
        "descriptor-shape",
        "size-validation",
        "buffer-length",
        "constructable",
        "consumer-ineligible"
    ]
);

buffer_string_row!(
    BUFFER_CONCAT,
    "Buffer.concat",
    "concat",
    NativeApiKind::StaticMethod,
    2,
    [NativeApiDomain::Buffer, NativeApiDomain::Number],
    [NativeApiDomain::Buffer],
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.concat",
    crate::native_api_manifest_generated::buffer_static_concat_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_static_concat_generated_validation_spec(),
    ["invalid-arg-type"],
    [
        "descriptor-shape",
        "list-validation",
        "truncate-zero-fill",
        "buffersource-items",
        "consumer-ineligible"
    ]
);

buffer_string_row!(
    BUFFER_IS_BUFFER,
    "Buffer.isBuffer",
    "isBuffer",
    NativeApiKind::StaticMethod,
    1,
    [NativeApiDomain::Buffer],
    [NativeApiDomain::Boolean],
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.isBuffer",
    crate::native_api_manifest_generated::buffer_static_is_buffer_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_static_is_buffer_generated_validation_spec(),
    [],
    [
        "descriptor-shape",
        "brand-true",
        "uint8array-false",
        "forgeable-marker-false",
        "consumer-ineligible"
    ]
);

macro_rules! buffer_nonconstructable_static_row {
    (
        $name:ident,
        $api:literal,
        $property:literal,
        $arity:literal,
        [$($arg:expr),* $(,)?],
        [$($ret:expr),* $(,)?],
        $effect:expr,
        $slow:literal,
        $registration:expr,
        $validation:expr,
        [$($bailout:literal),* $(,)?],
        [$($fixture:literal),* $(,)?]
    ) => {
        pub const $name: NativeApiManifestRow = NativeApiManifestRow {
            api: $api,
            receiver: NativeApiReceiver::Buffer,
            property: $property,
            kind: NativeApiKind::StaticMethod,
            arity: $arity,
            args: &[$($arg),*],
            returns: &[$($ret),*],
            constructor: false,
            authority: NativeApiAuthority::PureNonRetained,
            override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
            slow_path: $slow,
            fast_path: "None",
            ihi: None,
            registration: $registration,
            validation: $validation,
            fixture_doc: GeneratedFixtureDocSpec {
                api: $api,
                cases: &[],
            },
            jit_ic: None,
            lejit_expectation: None,
            bailouts: &[$($bailout),*],
            fixtures: &[$($fixture),*],
        };
    };
}

buffer_nonconstructable_static_row!(
    BUFFER_OF,
    "Buffer.of",
    "of",
    0,
    [],
    [NativeApiDomain::Buffer],
    NativeApiEffect::Pure,
    "rusty_js_runtime::intrinsics::Buffer.of",
    crate::native_api_manifest_generated::buffer_static_of_generated_registration_spec(),
    crate::native_api_manifest_generated::buffer_static_of_generated_validation_spec(),
    [],
    [
        "descriptor-shape",
        "non-constructable",
        "variadic-bytes",
        "to-uint8",
        "consumer-ineligible"
    ]
);

macro_rules! array_scalar_search_row {
    (
        $name:ident,
        $api:literal,
        $property:literal,
        $kind:expr,
        $arity:literal,
        [$($arg:expr),* $(,)?],
        [$($ret:expr),* $(,)?],
        $slow:literal,
        $registration:expr,
        $validation:expr,
        [$($bailout:literal),* $(,)?],
        [$($fixture:literal),* $(,)?]
    ) => {
        pub const $name: NativeApiManifestRow = NativeApiManifestRow {
            api: $api,
            receiver: NativeApiReceiver::Array,
            property: $property,
            kind: $kind,
            arity: $arity,
            args: &[$($arg),*],
            returns: &[$($ret),*],
            constructor: false,
            authority: NativeApiAuthority::PureNonRetained,
            override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
            slow_path: $slow,
            fast_path: "None",
            ihi: None,
            registration: $registration,
            validation: $validation,
            fixture_doc: GeneratedFixtureDocSpec {
                api: $api,
                cases: &[],
            },
            jit_ic: None,
            lejit_expectation: None,
            bailouts: &[$($bailout),*],
            fixtures: &[$($fixture),*],
        };
    };
}

array_scalar_search_row!(
    ARRAY_IS_ARRAY,
    "Array.isArray",
    "isArray",
    NativeApiKind::StaticMethod,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Boolean],
    "rusty_js_runtime::intrinsics::Array.isArray",
    crate::native_api_manifest_generated::array_static_is_array_generated_registration_spec(),
    crate::native_api_manifest_generated::array_static_is_array_generated_validation_spec(),
    ["non-constructable"],
    [
        "descriptor-shape",
        "array-brand-true",
        "array-like-false",
        "non-constructable",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_AT,
    "Array.prototype.at",
    "at",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Number],
    [NativeApiDomain::Unknown, NativeApiDomain::Undefined],
    "rusty_js_runtime::generated::array_prototype_at",
    crate::native_api_manifest_generated::array_at_generated_registration_spec(),
    crate::native_api_manifest_generated::array_at_generated_validation_spec(),
    ["array-like-receiver", "index-coercion", "non-constructable"],
    [
        "descriptor-shape",
        "array-like-receiver",
        "to-integer-or-infinity",
        "undefined-out-of-range",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_INCLUDES,
    "Array.prototype.includes",
    "includes",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Boolean],
    "rusty_js_runtime::generated::array_prototype_includes",
    crate::native_api_manifest_generated::array_includes_generated_registration_spec(),
    crate::native_api_manifest_generated::array_includes_generated_validation_spec(),
    [
        "array-like-receiver",
        "same-value-zero-search",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "holes-prototype-read",
        "same-value-zero",
        "from-index-coercion",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_INDEX_OF,
    "Array.prototype.indexOf",
    "indexOf",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Number],
    "rusty_js_runtime::generated::array_prototype_index_of",
    crate::native_api_manifest_generated::array_index_of_generated_registration_spec(),
    crate::native_api_manifest_generated::array_index_of_generated_validation_spec(),
    [
        "array-like-receiver",
        "strict-equality-search",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "holes-prototype-read",
        "strict-equality",
        "from-index-coercion",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_LAST_INDEX_OF,
    "Array.prototype.lastIndexOf",
    "lastIndexOf",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Number],
    "rusty_js_runtime::generated::array_prototype_last_index_of",
    crate::native_api_manifest_generated::array_last_index_of_generated_registration_spec(),
    crate::native_api_manifest_generated::array_last_index_of_generated_validation_spec(),
    [
        "array-like-receiver",
        "reverse-strict-equality-search",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "holes-prototype-read",
        "reverse-strict-equality",
        "from-index-coercion",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_FIND,
    "Array.prototype.find",
    "find",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Unknown, NativeApiDomain::Undefined],
    "rusty_js_runtime::generated::array_prototype_find",
    crate::native_api_manifest_generated::array_find_generated_registration_spec(),
    crate::native_api_manifest_generated::array_find_generated_validation_spec(),
    [
        "callback-not-callable",
        "array-like-receiver",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "callback-order",
        "this-arg",
        "mutation-visibility",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_FIND_INDEX,
    "Array.prototype.findIndex",
    "findIndex",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Number],
    "rusty_js_runtime::generated::array_prototype_find_index",
    crate::native_api_manifest_generated::array_find_index_generated_registration_spec(),
    crate::native_api_manifest_generated::array_find_index_generated_validation_spec(),
    [
        "callback-not-callable",
        "array-like-receiver",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "callback-order",
        "this-arg",
        "mutation-visibility",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_FIND_LAST,
    "Array.prototype.findLast",
    "findLast",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Unknown, NativeApiDomain::Undefined],
    "rusty_js_runtime::generated::array_prototype_find_last",
    crate::native_api_manifest_generated::array_find_last_generated_registration_spec(),
    crate::native_api_manifest_generated::array_find_last_generated_validation_spec(),
    [
        "callback-not-callable",
        "array-like-receiver",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "reverse-callback-order",
        "this-arg",
        "mutation-visibility",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_FIND_LAST_INDEX,
    "Array.prototype.findLastIndex",
    "findLastIndex",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Number],
    "rusty_js_runtime::generated::array_prototype_find_last_index",
    crate::native_api_manifest_generated::array_find_last_index_generated_registration_spec(),
    crate::native_api_manifest_generated::array_find_last_index_generated_validation_spec(),
    [
        "callback-not-callable",
        "array-like-receiver",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "reverse-callback-order",
        "this-arg",
        "mutation-visibility",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_FOR_EACH,
    "Array.prototype.forEach",
    "forEach",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Undefined],
    "rusty_js_runtime::generated::array_prototype_for_each",
    crate::native_api_manifest_generated::array_for_each_generated_registration_spec(),
    crate::native_api_manifest_generated::array_for_each_generated_validation_spec(),
    [
        "callback-not-callable",
        "array-like-receiver",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "callback-order",
        "hole-skipping",
        "mutation-visibility",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_MAP,
    "Array.prototype.map",
    "map",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Unknown],
    "rusty_js_runtime::generated::array_prototype_map",
    crate::native_api_manifest_generated::array_map_generated_registration_spec(),
    crate::native_api_manifest_generated::array_map_generated_validation_spec(),
    [
        "callback-not-callable",
        "array-like-receiver",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "callback-order",
        "hole-preservation",
        "mutation-visibility",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_FILTER,
    "Array.prototype.filter",
    "filter",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Unknown],
    "rusty_js_runtime::generated::array_prototype_filter",
    crate::native_api_manifest_generated::array_filter_generated_registration_spec(),
    crate::native_api_manifest_generated::array_filter_generated_validation_spec(),
    [
        "callback-not-callable",
        "array-like-receiver",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "callback-order",
        "hole-skipping",
        "mutation-visibility",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_SOME,
    "Array.prototype.some",
    "some",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Boolean],
    "rusty_js_runtime::generated::array_prototype_some",
    crate::native_api_manifest_generated::array_some_generated_registration_spec(),
    crate::native_api_manifest_generated::array_some_generated_validation_spec(),
    [
        "callback-not-callable",
        "array-like-receiver",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "short-circuit-truthy",
        "hole-skipping",
        "mutation-visibility",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_EVERY,
    "Array.prototype.every",
    "every",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Boolean],
    "rusty_js_runtime::generated::array_prototype_every",
    crate::native_api_manifest_generated::array_every_generated_registration_spec(),
    crate::native_api_manifest_generated::array_every_generated_validation_spec(),
    [
        "callback-not-callable",
        "array-like-receiver",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "short-circuit-falsy",
        "hole-skipping",
        "mutation-visibility",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_REDUCE,
    "Array.prototype.reduce",
    "reduce",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Unknown],
    "rusty_js_runtime::generated::array_prototype_reduce",
    crate::native_api_manifest_generated::array_reduce_generated_registration_spec(),
    crate::native_api_manifest_generated::array_reduce_generated_validation_spec(),
    [
        "callback-not-callable",
        "empty-without-initial-value",
        "array-like-receiver",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "reducer-order",
        "seed-selection",
        "empty-type-error",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_REDUCE_RIGHT,
    "Array.prototype.reduceRight",
    "reduceRight",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Unknown],
    "rusty_js_runtime::generated::array_prototype_reduce_right",
    crate::native_api_manifest_generated::array_reduce_right_generated_registration_spec(),
    crate::native_api_manifest_generated::array_reduce_right_generated_validation_spec(),
    [
        "callback-not-callable",
        "empty-without-initial-value",
        "array-like-receiver",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "reverse-reducer-order",
        "seed-selection",
        "empty-type-error",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_FLAT_MAP,
    "Array.prototype.flatMap",
    "flatMap",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Unknown],
    "rusty_js_runtime::generated::array_prototype_flat_map",
    crate::native_api_manifest_generated::array_flat_map_generated_registration_spec(),
    crate::native_api_manifest_generated::array_flat_map_generated_validation_spec(),
    [
        "callback-not-callable",
        "array-like-receiver",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "callback-order",
        "one-level-flattening",
        "mutation-visibility",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_PUSH,
    "Array.prototype.push",
    "push",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Number],
    "rusty_js_runtime::generated::array_prototype_push",
    crate::native_api_manifest_generated::array_push_generated_registration_spec(),
    crate::native_api_manifest_generated::array_push_generated_validation_spec(),
    [
        "array-like-receiver",
        "length-mutation",
        "non-writable-length",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "array-like-receiver",
        "length-append",
        "non-writable-length",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_POP,
    "Array.prototype.pop",
    "pop",
    NativeApiKind::Method,
    0,
    [],
    [NativeApiDomain::Unknown, NativeApiDomain::Undefined],
    "rusty_js_runtime::generated::array_prototype_pop",
    crate::native_api_manifest_generated::array_pop_generated_registration_spec(),
    crate::native_api_manifest_generated::array_pop_generated_validation_spec(),
    [
        "array-like-receiver",
        "length-mutation",
        "prototype-index-read",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "array-like-receiver",
        "tail-delete",
        "prototype-read",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_SHIFT,
    "Array.prototype.shift",
    "shift",
    NativeApiKind::Method,
    0,
    [],
    [NativeApiDomain::Unknown, NativeApiDomain::Undefined],
    "rusty_js_runtime::generated::array_prototype_shift",
    crate::native_api_manifest_generated::array_shift_generated_registration_spec(),
    crate::native_api_manifest_generated::array_shift_generated_validation_spec(),
    [
        "array-like-receiver",
        "hole-preservation",
        "prototype-index-read",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "array-like-receiver",
        "head-shift",
        "hole-preservation",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_UNSHIFT,
    "Array.prototype.unshift",
    "unshift",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Number],
    "rusty_js_runtime::generated::array_prototype_unshift",
    crate::native_api_manifest_generated::array_unshift_generated_registration_spec(),
    crate::native_api_manifest_generated::array_unshift_generated_validation_spec(),
    [
        "array-like-receiver",
        "hole-preservation",
        "length-mutation",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "array-like-receiver",
        "head-insert",
        "hole-preservation",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_SPLICE,
    "Array.prototype.splice",
    "splice",
    NativeApiKind::Method,
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::Unknown],
    "rusty_js_runtime::generated::array_prototype_splice",
    crate::native_api_manifest_generated::array_splice_generated_registration_spec(),
    crate::native_api_manifest_generated::array_splice_generated_validation_spec(),
    [
        "array-like-receiver",
        "length-mutation",
        "species-allocation",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "start-delete-insert",
        "hole-prototype-behavior",
        "array-species-create",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_REVERSE,
    "Array.prototype.reverse",
    "reverse",
    NativeApiKind::Method,
    0,
    [],
    [NativeApiDomain::Unknown],
    "rusty_js_runtime::generated::array_prototype_reverse",
    crate::native_api_manifest_generated::array_reverse_generated_registration_spec(),
    crate::native_api_manifest_generated::array_reverse_generated_validation_spec(),
    [
        "array-like-receiver",
        "hole-preservation",
        "prototype-index-read",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "receiver-return",
        "whole-length",
        "hole-prototype-behavior",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_COPY_WITHIN,
    "Array.prototype.copyWithin",
    "copyWithin",
    NativeApiKind::Method,
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::Unknown],
    "rusty_js_runtime::generated::array_prototype_copy_within",
    crate::native_api_manifest_generated::array_copy_within_generated_registration_spec(),
    crate::native_api_manifest_generated::array_copy_within_generated_validation_spec(),
    [
        "array-like-receiver",
        "range-coercion",
        "hole-preservation",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "receiver-return",
        "target-start-end",
        "hole-prototype-behavior",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_FILL,
    "Array.prototype.fill",
    "fill",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Unknown],
    "rusty_js_runtime::generated::array_prototype_fill",
    crate::native_api_manifest_generated::array_fill_generated_registration_spec(),
    crate::native_api_manifest_generated::array_fill_generated_validation_spec(),
    [
        "array-like-receiver",
        "range-coercion",
        "hole-materialization",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "receiver-return",
        "start-end-fill",
        "hole-materialization",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_SLICE,
    "Array.prototype.slice",
    "slice",
    NativeApiKind::Method,
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::Unknown],
    "rusty_js_runtime::generated::array_prototype_slice",
    crate::native_api_manifest_generated::array_slice_generated_registration_spec(),
    crate::native_api_manifest_generated::array_slice_generated_validation_spec(),
    [
        "array-like-receiver",
        "range-coercion",
        "species-allocation",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "hole-prototype-behavior",
        "array-species-create",
        "fresh-result",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_CONCAT,
    "Array.prototype.concat",
    "concat",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Unknown],
    "rusty_js_runtime::generated::array_prototype_concat",
    crate::native_api_manifest_generated::array_concat_generated_registration_spec(),
    crate::native_api_manifest_generated::array_concat_generated_validation_spec(),
    [
        "array-like-receiver",
        "spreadability",
        "species-allocation",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "symbol-is-concat-spreadable",
        "array-species-create",
        "fresh-result",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_SORT,
    "Array.prototype.sort",
    "sort",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Unknown],
    "rusty_js_runtime::generated::array_prototype_sort",
    crate::native_api_manifest_generated::array_sort_generated_registration_spec(),
    crate::native_api_manifest_generated::array_sort_generated_validation_spec(),
    [
        "array-like-receiver",
        "callback-not-callable",
        "stable-sort",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "default-string-sort",
        "sort-compare",
        "undefined-holes-tail",
        "consumer-ineligible"
    ]
);

array_scalar_search_row!(
    ARRAY_FLAT,
    "Array.prototype.flat",
    "flat",
    NativeApiKind::Method,
    0,
    [],
    [NativeApiDomain::Unknown],
    "rusty_js_runtime::generated::array_prototype_flat",
    crate::native_api_manifest_generated::array_flat_generated_registration_spec(),
    crate::native_api_manifest_generated::array_flat_generated_validation_spec(),
    [
        "array-like-receiver",
        "depth-coercion",
        "species-allocation",
        "non-constructable"
    ],
    [
        "descriptor-shape",
        "depth-flatten",
        "hole-prototype-behavior",
        "array-species-create",
        "consumer-ineligible"
    ]
);

macro_rules! dataview_byte_getset_row {
    (
        $name:ident,
        $api:literal,
        $property:literal,
        $arity:literal,
        [$($arg:expr),* $(,)?],
        [$($ret:expr),* $(,)?],
        $effect:expr,
        $registration:expr,
        $validation:expr,
        [$($bailout:literal),* $(,)?],
        [$($fixture:literal),* $(,)?]
    ) => {
        pub const $name: NativeApiManifestRow = NativeApiManifestRow {
            api: $api,
            receiver: NativeApiReceiver::DataView,
            property: $property,
            kind: NativeApiKind::Method,
            arity: $arity,
            args: &[$($arg),*],
            returns: &[$($ret),*],
            constructor: false,
            authority: NativeApiAuthority::PureNonRetained,
            override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
            slow_path: "rusty_js_runtime::intrinsics::DataView.prototype numeric get/set",
            fast_path: "None",
            ihi: None,
            registration: $registration,
            validation: $validation,
            fixture_doc: GeneratedFixtureDocSpec {
                api: $api,
                cases: &[],
            },
            jit_ic: None,
            lejit_expectation: None,
            bailouts: &[$($bailout),*],
            fixtures: &[$($fixture),*],
        };
    };
}

dataview_byte_getset_row!(
    DATAVIEW_GET_INT8,
    "DataView.prototype.getInt8",
    "getInt8",
    1,
    [NativeApiDomain::Number],
    [NativeApiDomain::Number],
    NativeApiEffect::Pure,
    crate::native_api_manifest_generated::data_view_get_int8_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_get_int8_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer"
    ],
    [
        "descriptor-shape",
        "byte-width-1",
        "signed-int8",
        "bounds-rangeerror",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

dataview_byte_getset_row!(
    DATAVIEW_GET_UINT8,
    "DataView.prototype.getUint8",
    "getUint8",
    1,
    [NativeApiDomain::Number],
    [NativeApiDomain::Number],
    NativeApiEffect::Pure,
    crate::native_api_manifest_generated::data_view_get_uint8_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_get_uint8_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer"
    ],
    [
        "descriptor-shape",
        "byte-width-1",
        "unsigned-uint8",
        "bounds-rangeerror",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

dataview_byte_getset_row!(
    DATAVIEW_SET_INT8,
    "DataView.prototype.setInt8",
    "setInt8",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::Undefined],
    NativeApiEffect::MutatesReceiverBytes,
    crate::native_api_manifest_generated::data_view_set_int8_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_set_int8_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer"
    ],
    [
        "descriptor-shape",
        "byte-width-1",
        "signed-int8",
        "mutates-backing-buffer",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

dataview_byte_getset_row!(
    DATAVIEW_SET_UINT8,
    "DataView.prototype.setUint8",
    "setUint8",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::Undefined],
    NativeApiEffect::MutatesReceiverBytes,
    crate::native_api_manifest_generated::data_view_set_uint8_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_set_uint8_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer"
    ],
    [
        "descriptor-shape",
        "byte-width-1",
        "unsigned-uint8",
        "mutates-backing-buffer",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

dataview_byte_getset_row!(
    DATAVIEW_GET_INT16,
    "DataView.prototype.getInt16",
    "getInt16",
    1,
    [NativeApiDomain::Number],
    [NativeApiDomain::Number],
    NativeApiEffect::Pure,
    crate::native_api_manifest_generated::data_view_get_int16_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_get_int16_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer",
        "runtime-endian-flag"
    ],
    [
        "descriptor-shape",
        "byte-width-2",
        "signed-int16",
        "runtime-endian-flag",
        "bounds-rangeerror",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

dataview_byte_getset_row!(
    DATAVIEW_GET_UINT16,
    "DataView.prototype.getUint16",
    "getUint16",
    1,
    [NativeApiDomain::Number],
    [NativeApiDomain::Number],
    NativeApiEffect::Pure,
    crate::native_api_manifest_generated::data_view_get_uint16_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_get_uint16_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer",
        "runtime-endian-flag"
    ],
    [
        "descriptor-shape",
        "byte-width-2",
        "unsigned-uint16",
        "runtime-endian-flag",
        "bounds-rangeerror",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

dataview_byte_getset_row!(
    DATAVIEW_SET_INT16,
    "DataView.prototype.setInt16",
    "setInt16",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::Undefined],
    NativeApiEffect::MutatesReceiverBytes,
    crate::native_api_manifest_generated::data_view_set_int16_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_set_int16_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer",
        "runtime-endian-flag"
    ],
    [
        "descriptor-shape",
        "byte-width-2",
        "signed-int16",
        "runtime-endian-flag",
        "mutates-backing-buffer",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

dataview_byte_getset_row!(
    DATAVIEW_SET_UINT16,
    "DataView.prototype.setUint16",
    "setUint16",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::Undefined],
    NativeApiEffect::MutatesReceiverBytes,
    crate::native_api_manifest_generated::data_view_set_uint16_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_set_uint16_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer",
        "runtime-endian-flag"
    ],
    [
        "descriptor-shape",
        "byte-width-2",
        "unsigned-uint16",
        "runtime-endian-flag",
        "mutates-backing-buffer",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

dataview_byte_getset_row!(
    DATAVIEW_GET_INT32,
    "DataView.prototype.getInt32",
    "getInt32",
    1,
    [NativeApiDomain::Number],
    [NativeApiDomain::Number],
    NativeApiEffect::Pure,
    crate::native_api_manifest_generated::data_view_get_int32_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_get_int32_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer",
        "runtime-endian-flag"
    ],
    [
        "descriptor-shape",
        "byte-width-4",
        "signed-int32",
        "runtime-endian-flag",
        "bounds-rangeerror",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

dataview_byte_getset_row!(
    DATAVIEW_GET_UINT32,
    "DataView.prototype.getUint32",
    "getUint32",
    1,
    [NativeApiDomain::Number],
    [NativeApiDomain::Number],
    NativeApiEffect::Pure,
    crate::native_api_manifest_generated::data_view_get_uint32_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_get_uint32_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer",
        "runtime-endian-flag"
    ],
    [
        "descriptor-shape",
        "byte-width-4",
        "unsigned-uint32",
        "runtime-endian-flag",
        "bounds-rangeerror",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

dataview_byte_getset_row!(
    DATAVIEW_SET_INT32,
    "DataView.prototype.setInt32",
    "setInt32",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::Undefined],
    NativeApiEffect::MutatesReceiverBytes,
    crate::native_api_manifest_generated::data_view_set_int32_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_set_int32_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer",
        "runtime-endian-flag"
    ],
    [
        "descriptor-shape",
        "byte-width-4",
        "signed-int32",
        "runtime-endian-flag",
        "mutates-backing-buffer",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

dataview_byte_getset_row!(
    DATAVIEW_SET_UINT32,
    "DataView.prototype.setUint32",
    "setUint32",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::Undefined],
    NativeApiEffect::MutatesReceiverBytes,
    crate::native_api_manifest_generated::data_view_set_uint32_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_set_uint32_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer",
        "runtime-endian-flag"
    ],
    [
        "descriptor-shape",
        "byte-width-4",
        "unsigned-uint32",
        "runtime-endian-flag",
        "mutates-backing-buffer",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

dataview_byte_getset_row!(
    DATAVIEW_GET_FLOAT16,
    "DataView.prototype.getFloat16",
    "getFloat16",
    1,
    [NativeApiDomain::Number],
    [NativeApiDomain::Number],
    NativeApiEffect::Pure,
    crate::native_api_manifest_generated::data_view_get_float16_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_get_float16_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer",
        "runtime-endian-flag",
        "ieee-float16"
    ],
    [
        "descriptor-shape",
        "byte-width-2",
        "ieee-float16",
        "runtime-endian-flag",
        "bounds-rangeerror",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

dataview_byte_getset_row!(
    DATAVIEW_GET_FLOAT32,
    "DataView.prototype.getFloat32",
    "getFloat32",
    1,
    [NativeApiDomain::Number],
    [NativeApiDomain::Number],
    NativeApiEffect::Pure,
    crate::native_api_manifest_generated::data_view_get_float32_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_get_float32_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer",
        "runtime-endian-flag",
        "ieee-float32"
    ],
    [
        "descriptor-shape",
        "byte-width-4",
        "ieee-float32",
        "runtime-endian-flag",
        "bounds-rangeerror",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

dataview_byte_getset_row!(
    DATAVIEW_GET_FLOAT64,
    "DataView.prototype.getFloat64",
    "getFloat64",
    1,
    [NativeApiDomain::Number],
    [NativeApiDomain::Number],
    NativeApiEffect::Pure,
    crate::native_api_manifest_generated::data_view_get_float64_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_get_float64_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer",
        "runtime-endian-flag",
        "ieee-float64"
    ],
    [
        "descriptor-shape",
        "byte-width-8",
        "ieee-float64",
        "runtime-endian-flag",
        "bounds-rangeerror",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

dataview_byte_getset_row!(
    DATAVIEW_SET_FLOAT16,
    "DataView.prototype.setFloat16",
    "setFloat16",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::Undefined],
    NativeApiEffect::MutatesReceiverBytes,
    crate::native_api_manifest_generated::data_view_set_float16_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_set_float16_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer",
        "runtime-endian-flag",
        "ieee-float16"
    ],
    [
        "descriptor-shape",
        "byte-width-2",
        "ieee-float16",
        "runtime-endian-flag",
        "mutates-backing-buffer",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

dataview_byte_getset_row!(
    DATAVIEW_SET_FLOAT32,
    "DataView.prototype.setFloat32",
    "setFloat32",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::Undefined],
    NativeApiEffect::MutatesReceiverBytes,
    crate::native_api_manifest_generated::data_view_set_float32_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_set_float32_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer",
        "runtime-endian-flag",
        "ieee-float32"
    ],
    [
        "descriptor-shape",
        "byte-width-4",
        "ieee-float32",
        "runtime-endian-flag",
        "mutates-backing-buffer",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

dataview_byte_getset_row!(
    DATAVIEW_SET_FLOAT64,
    "DataView.prototype.setFloat64",
    "setFloat64",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::Undefined],
    NativeApiEffect::MutatesReceiverBytes,
    crate::native_api_manifest_generated::data_view_set_float64_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_set_float64_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer",
        "runtime-endian-flag",
        "ieee-float64"
    ],
    [
        "descriptor-shape",
        "byte-width-8",
        "ieee-float64",
        "runtime-endian-flag",
        "mutates-backing-buffer",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

dataview_byte_getset_row!(
    DATAVIEW_GET_BIG_INT64,
    "DataView.prototype.getBigInt64",
    "getBigInt64",
    1,
    [NativeApiDomain::Number],
    [NativeApiDomain::BigInt],
    NativeApiEffect::Pure,
    crate::native_api_manifest_generated::data_view_get_big_int64_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_get_big_int64_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer",
        "runtime-endian-flag",
        "signed-bigint64"
    ],
    [
        "descriptor-shape",
        "byte-width-8",
        "signed-bigint64",
        "runtime-endian-flag",
        "bounds-rangeerror",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

dataview_byte_getset_row!(
    DATAVIEW_GET_BIG_UINT64,
    "DataView.prototype.getBigUint64",
    "getBigUint64",
    1,
    [NativeApiDomain::Number],
    [NativeApiDomain::BigInt],
    NativeApiEffect::Pure,
    crate::native_api_manifest_generated::data_view_get_big_uint64_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_get_big_uint64_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer",
        "runtime-endian-flag",
        "unsigned-biguint64"
    ],
    [
        "descriptor-shape",
        "byte-width-8",
        "unsigned-biguint64",
        "runtime-endian-flag",
        "bounds-rangeerror",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

dataview_byte_getset_row!(
    DATAVIEW_SET_BIG_INT64,
    "DataView.prototype.setBigInt64",
    "setBigInt64",
    2,
    [NativeApiDomain::Number, NativeApiDomain::BigInt],
    [NativeApiDomain::Undefined],
    NativeApiEffect::MutatesReceiverBytes,
    crate::native_api_manifest_generated::data_view_set_big_int64_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_set_big_int64_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer",
        "runtime-endian-flag",
        "bigint-value-required",
        "signed-bigint64"
    ],
    [
        "descriptor-shape",
        "byte-width-8",
        "signed-bigint64",
        "runtime-endian-flag",
        "mutates-backing-buffer",
        "bigint-value-required",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

dataview_byte_getset_row!(
    DATAVIEW_SET_BIG_UINT64,
    "DataView.prototype.setBigUint64",
    "setBigUint64",
    2,
    [NativeApiDomain::Number, NativeApiDomain::BigInt],
    [NativeApiDomain::Undefined],
    NativeApiEffect::MutatesReceiverBytes,
    crate::native_api_manifest_generated::data_view_set_big_uint64_generated_registration_spec(),
    crate::native_api_manifest_generated::data_view_set_big_uint64_generated_validation_spec(),
    [
        "receiver-not-dataview",
        "offset-coercion",
        "bounds",
        "detached-buffer",
        "runtime-endian-flag",
        "bigint-value-required",
        "unsigned-biguint64"
    ],
    [
        "descriptor-shape",
        "byte-width-8",
        "unsigned-biguint64",
        "runtime-endian-flag",
        "mutates-backing-buffer",
        "bigint-value-required",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

macro_rules! typedarray_scalar_search_row {
    (
        $name:ident,
        $api:literal,
        $property:literal,
        $arity:literal,
        [$($arg:expr),* $(,)?],
        [$($ret:expr),* $(,)?],
        $registration:expr,
        $validation:expr,
        [$($bailout:literal),* $(,)?],
        [$($fixture:literal),* $(,)?]
    ) => {
        pub const $name: NativeApiManifestRow = NativeApiManifestRow {
            api: $api,
            receiver: NativeApiReceiver::TypedArray,
            property: $property,
            kind: NativeApiKind::Method,
            arity: $arity,
            args: &[$($arg),*],
            returns: &[$($ret),*],
            constructor: false,
            authority: NativeApiAuthority::PureNonRetained,
            override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
            slow_path: "rusty_js_runtime::intrinsics::TypedArray.prototype scalar/search",
            fast_path: "None",
            ihi: None,
            registration: $registration,
            validation: $validation,
            fixture_doc: GeneratedFixtureDocSpec {
                api: $api,
                cases: &[],
            },
            jit_ic: None,
            lejit_expectation: None,
            bailouts: &[$($bailout),*],
            fixtures: &[$($fixture),*],
        };
    };
}

typedarray_scalar_search_row!(
    TYPEDARRAY_AT,
    "TypedArray.prototype.at",
    "at",
    1,
    [NativeApiDomain::Number],
    [
        NativeApiDomain::Number,
        NativeApiDomain::BigInt,
        NativeApiDomain::Undefined
    ],
    crate::native_api_manifest_generated::typed_array_at_generated_registration_spec(),
    crate::native_api_manifest_generated::typed_array_at_generated_validation_spec(),
    [
        "receiver-not-typedarray",
        "detached-or-out-of-bounds",
        "index-coercion"
    ],
    [
        "descriptor-shape",
        "relative-index",
        "negative-index",
        "out-of-range-undefined",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

typedarray_scalar_search_row!(
    TYPEDARRAY_INCLUDES,
    "TypedArray.prototype.includes",
    "includes",
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Boolean],
    crate::native_api_manifest_generated::typed_array_includes_generated_registration_spec(),
    crate::native_api_manifest_generated::typed_array_includes_generated_validation_spec(),
    [
        "receiver-not-typedarray",
        "detached-or-out-of-bounds",
        "from-index-coercion"
    ],
    [
        "descriptor-shape",
        "same-value-zero",
        "negative-from-index",
        "nan-match",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

typedarray_scalar_search_row!(
    TYPEDARRAY_INDEX_OF,
    "TypedArray.prototype.indexOf",
    "indexOf",
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Number],
    crate::native_api_manifest_generated::typed_array_index_of_generated_registration_spec(),
    crate::native_api_manifest_generated::typed_array_index_of_generated_validation_spec(),
    [
        "receiver-not-typedarray",
        "detached-or-out-of-bounds",
        "from-index-coercion"
    ],
    [
        "descriptor-shape",
        "strict-equality",
        "negative-from-index",
        "miss-negative-one",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

typedarray_scalar_search_row!(
    TYPEDARRAY_LAST_INDEX_OF,
    "TypedArray.prototype.lastIndexOf",
    "lastIndexOf",
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Number],
    crate::native_api_manifest_generated::typed_array_last_index_of_generated_registration_spec(),
    crate::native_api_manifest_generated::typed_array_last_index_of_generated_validation_spec(),
    [
        "receiver-not-typedarray",
        "detached-or-out-of-bounds",
        "from-index-coercion"
    ],
    [
        "descriptor-shape",
        "strict-equality",
        "negative-from-index",
        "reverse-search",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

typedarray_scalar_search_row!(
    TYPEDARRAY_FILL,
    "TypedArray.prototype.fill",
    "fill",
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Unknown],
    crate::native_api_manifest_generated::typed_array_fill_generated_registration_spec(),
    crate::native_api_manifest_generated::typed_array_fill_generated_validation_spec(),
    [
        "receiver-not-typedarray",
        "detached-or-out-of-bounds",
        "index-coercion",
        "element-coercion"
    ],
    [
        "descriptor-shape",
        "value-coercion",
        "negative-index-window",
        "mutates-backing-buffer",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

typedarray_scalar_search_row!(
    TYPEDARRAY_REVERSE,
    "TypedArray.prototype.reverse",
    "reverse",
    0,
    [],
    [NativeApiDomain::Unknown],
    crate::native_api_manifest_generated::typed_array_reverse_generated_registration_spec(),
    crate::native_api_manifest_generated::typed_array_reverse_generated_validation_spec(),
    ["receiver-not-typedarray", "detached-or-out-of-bounds"],
    [
        "descriptor-shape",
        "in-place-reorder",
        "mutates-backing-buffer",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

typedarray_scalar_search_row!(
    TYPEDARRAY_COPY_WITHIN,
    "TypedArray.prototype.copyWithin",
    "copyWithin",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::Unknown],
    crate::native_api_manifest_generated::typed_array_copy_within_generated_registration_spec(),
    crate::native_api_manifest_generated::typed_array_copy_within_generated_validation_spec(),
    [
        "receiver-not-typedarray",
        "detached-or-out-of-bounds",
        "index-coercion",
        "overlap-copy"
    ],
    [
        "descriptor-shape",
        "negative-index-window",
        "overlap-safe-copy",
        "mutates-backing-buffer",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

typedarray_scalar_search_row!(
    TYPEDARRAY_SET,
    "TypedArray.prototype.set",
    "set",
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Undefined],
    crate::native_api_manifest_generated::typed_array_set_generated_registration_spec(),
    crate::native_api_manifest_generated::typed_array_set_generated_validation_spec(),
    [
        "receiver-not-typedarray",
        "detached-or-out-of-bounds",
        "offset-coercion",
        "bounds",
        "element-coercion"
    ],
    [
        "descriptor-shape",
        "source-copy",
        "overlap-safe-copy",
        "bounds-rangeerror",
        "mutates-backing-buffer",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

typedarray_scalar_search_row!(
    TYPEDARRAY_SLICE,
    "TypedArray.prototype.slice",
    "slice",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::Unknown],
    crate::native_api_manifest_generated::typed_array_slice_generated_registration_spec(),
    crate::native_api_manifest_generated::typed_array_slice_generated_validation_spec(),
    [
        "receiver-not-typedarray",
        "detached-or-out-of-bounds",
        "index-coercion",
        "species-constructor"
    ],
    [
        "descriptor-shape",
        "negative-index-window",
        "species-copy",
        "non-aliasing-result",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

typedarray_scalar_search_row!(
    TYPEDARRAY_SUBARRAY,
    "TypedArray.prototype.subarray",
    "subarray",
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::Unknown],
    crate::native_api_manifest_generated::typed_array_subarray_generated_registration_spec(),
    crate::native_api_manifest_generated::typed_array_subarray_generated_validation_spec(),
    [
        "receiver-not-typedarray",
        "detached-or-out-of-bounds",
        "index-coercion"
    ],
    [
        "descriptor-shape",
        "negative-index-window",
        "aliasing-view",
        "species-independent",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

typedarray_scalar_search_row!(
    TYPEDARRAY_TO_REVERSED,
    "TypedArray.prototype.toReversed",
    "toReversed",
    0,
    [],
    [NativeApiDomain::Unknown],
    crate::native_api_manifest_generated::typed_array_to_reversed_generated_registration_spec(),
    crate::native_api_manifest_generated::typed_array_to_reversed_generated_validation_spec(),
    ["receiver-not-typedarray", "detached-or-out-of-bounds"],
    [
        "descriptor-shape",
        "change-by-copy",
        "species-independent",
        "non-aliasing-result",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

typedarray_scalar_search_row!(
    TYPEDARRAY_TO_SORTED,
    "TypedArray.prototype.toSorted",
    "toSorted",
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Unknown],
    crate::native_api_manifest_generated::typed_array_to_sorted_generated_registration_spec(),
    crate::native_api_manifest_generated::typed_array_to_sorted_generated_validation_spec(),
    [
        "receiver-not-typedarray",
        "detached-or-out-of-bounds",
        "callback-not-callable"
    ],
    [
        "descriptor-shape",
        "change-by-copy",
        "comparator-call",
        "species-independent",
        "non-aliasing-result",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

macro_rules! arraybuffer_lifecycle_row {
    (
        $name:ident,
        $api:expr,
        $property:expr,
        $kind:expr,
        $arity:expr,
        [$($arg:expr),* $(,)?],
        [$($ret:expr),* $(,)?],
        $registration:expr,
        $validation:expr,
        [$($bailout:expr),* $(,)?],
        [$($fixture:expr),* $(,)?]
    ) => {
        pub const $name: NativeApiManifestRow = NativeApiManifestRow {
            api: $api,
            receiver: NativeApiReceiver::ArrayBuffer,
            property: $property,
            kind: $kind,
            arity: $arity,
            args: &[$($arg),*],
            returns: &[$($ret),*],
            constructor: false,
            authority: NativeApiAuthority::PureNonRetained,
            override_guard: NativeApiOverrideGuard::MethodIdentityUnchanged,
            slow_path: $api,
            fast_path: "None",
            ihi: None,
            registration: $registration,
            validation: $validation,
            fixture_doc: GeneratedFixtureDocSpec {
                api: $api,
                cases: &[],
            },
            jit_ic: None,
            lejit_expectation: None,
            bailouts: &[$($bailout),*],
            fixtures: &[$($fixture),*],
        };
    };
}

arraybuffer_lifecycle_row!(
    ARRAYBUFFER_IS_VIEW,
    "ArrayBuffer.isView",
    "isView",
    NativeApiKind::StaticMethod,
    1,
    [NativeApiDomain::Unknown],
    [NativeApiDomain::Boolean],
    crate::native_api_manifest_generated::array_buffer_static_is_view_generated_registration_spec(),
    crate::native_api_manifest_generated::array_buffer_static_is_view_generated_validation_spec(),
    ["static-receiver", "view-brand"],
    [
        "descriptor-shape",
        "typedarray-view",
        "dataview-view",
        "arraybuffer-false",
        "consumer-ineligible"
    ]
);

arraybuffer_lifecycle_row!(
    ARRAYBUFFER_SLICE,
    "ArrayBuffer.prototype.slice",
    "slice",
    NativeApiKind::Method,
    2,
    [NativeApiDomain::Number, NativeApiDomain::Number],
    [NativeApiDomain::Unknown],
    crate::native_api_manifest_generated::array_buffer_slice_generated_registration_spec(),
    crate::native_api_manifest_generated::array_buffer_slice_generated_validation_spec(),
    [
        "receiver-not-arraybuffer",
        "detached-buffer",
        "index-coercion"
    ],
    [
        "descriptor-shape",
        "negative-byte-window",
        "copy-result",
        "detached-typeerror",
        "consumer-ineligible"
    ]
);

arraybuffer_lifecycle_row!(
    ARRAYBUFFER_RESIZE,
    "ArrayBuffer.prototype.resize",
    "resize",
    NativeApiKind::Method,
    1,
    [NativeApiDomain::Number],
    [NativeApiDomain::Undefined],
    crate::native_api_manifest_generated::array_buffer_resize_generated_registration_spec(),
    crate::native_api_manifest_generated::array_buffer_resize_generated_validation_spec(),
    [
        "receiver-not-arraybuffer",
        "detached-buffer",
        "not-resizable",
        "max-byte-length"
    ],
    [
        "descriptor-shape",
        "resizable-brand",
        "byte-length-mutation",
        "max-byte-length-rangeerror",
        "consumer-ineligible"
    ]
);

arraybuffer_lifecycle_row!(
    ARRAYBUFFER_TRANSFER,
    "ArrayBuffer.prototype.transfer",
    "transfer",
    NativeApiKind::Method,
    0,
    [],
    [NativeApiDomain::Unknown],
    crate::native_api_manifest_generated::array_buffer_transfer_generated_registration_spec(),
    crate::native_api_manifest_generated::array_buffer_transfer_generated_validation_spec(),
    [
        "receiver-not-arraybuffer",
        "detached-buffer",
        "max-byte-length"
    ],
    [
        "descriptor-shape",
        "optional-byte-length",
        "transfer-detach",
        "copy-result",
        "consumer-ineligible"
    ]
);

arraybuffer_lifecycle_row!(
    ARRAYBUFFER_TRANSFER_TO_FIXED_LENGTH,
    "ArrayBuffer.prototype.transferToFixedLength",
    "transferToFixedLength",
    NativeApiKind::Method,
    0,
    [],
    [NativeApiDomain::Unknown],
    crate::native_api_manifest_generated::
        array_buffer_transfer_to_fixed_length_generated_registration_spec(),
    crate::native_api_manifest_generated::
        array_buffer_transfer_to_fixed_length_generated_validation_spec(),
    ["receiver-not-arraybuffer", "detached-buffer", "max-byte-length"],
    [
        "descriptor-shape",
        "optional-byte-length",
        "fixed-transfer-detach",
        "fixed-length-result",
        "consumer-ineligible"
    ]
);

pub const MANIFEST_ROWS: &[NativeApiManifestRow] = &[
    STRING_CHAR_CODE_AT,
    STRING_TO_LOWER_CASE,
    STRING_TO_UPPER_CASE,
    STRING_TRIM,
    STRING_TRIM_START,
    STRING_TRIM_END,
    STRING_REPEAT,
    STRING_PAD_START,
    STRING_PAD_END,
    STRING_INCLUDES,
    STRING_STARTS_WITH,
    STRING_ENDS_WITH,
    STRING_INDEX_OF,
    STRING_LAST_INDEX_OF,
    STRING_CHAR_AT,
    STRING_AT,
    STRING_SLICE,
    STRING_SUBSTRING,
    STRING_SUBSTR,
    STRING_CODE_POINT_AT,
    BUFFER_READ_UINT8,
    BUFFER_READ_INT8,
    BUFFER_WRITE_UINT8,
    BUFFER_WRITE_INT8,
    BUFFER_READ_UINT16_LE,
    BUFFER_READ_UINT16_BE,
    BUFFER_READ_INT16_LE,
    BUFFER_READ_INT16_BE,
    BUFFER_WRITE_UINT16_LE,
    BUFFER_WRITE_UINT16_BE,
    BUFFER_WRITE_INT16_LE,
    BUFFER_WRITE_INT16_BE,
    BUFFER_READ_UINT32_LE,
    BUFFER_READ_UINT32_BE,
    BUFFER_READ_INT32_LE,
    BUFFER_READ_INT32_BE,
    BUFFER_WRITE_UINT32_LE,
    BUFFER_WRITE_UINT32_BE,
    BUFFER_WRITE_INT32_LE,
    BUFFER_WRITE_INT32_BE,
    BUFFER_READ_FLOAT_LE,
    BUFFER_READ_FLOAT_BE,
    BUFFER_READ_DOUBLE_LE,
    BUFFER_READ_DOUBLE_BE,
    BUFFER_WRITE_FLOAT_LE,
    BUFFER_WRITE_FLOAT_BE,
    BUFFER_WRITE_DOUBLE_LE,
    BUFFER_WRITE_DOUBLE_BE,
    BUFFER_READ_BIG_UINT64_LE,
    BUFFER_READ_BIG_UINT64_BE,
    BUFFER_READ_BIG_INT64_LE,
    BUFFER_READ_BIG_INT64_BE,
    BUFFER_WRITE_BIG_UINT64_LE,
    BUFFER_WRITE_BIG_UINT64_BE,
    BUFFER_WRITE_BIG_INT64_LE,
    BUFFER_WRITE_BIG_INT64_BE,
    BUFFER_READ_UINT_LE,
    BUFFER_READ_UINT_BE,
    BUFFER_READ_INT_LE,
    BUFFER_READ_INT_BE,
    BUFFER_WRITE_UINT_LE,
    BUFFER_WRITE_UINT_BE,
    BUFFER_WRITE_INT_LE,
    BUFFER_WRITE_INT_BE,
    BUFFER_SWAP16,
    BUFFER_SWAP32,
    BUFFER_SWAP64,
    BUFFER_TO_JSON,
    BUFFER_UTF8_SLICE,
    BUFFER_LATIN1_SLICE,
    BUFFER_ASCII_SLICE,
    BUFFER_UCS2_SLICE,
    BUFFER_BASE64_SLICE,
    BUFFER_TO_STRING,
    BUFFER_WRITE,
    BUFFER_BYTE_LENGTH,
    BUFFER_IS_ENCODING,
    BUFFER_FROM,
    BUFFER_INCLUDES,
    BUFFER_INDEX_OF,
    BUFFER_LAST_INDEX_OF,
    BUFFER_EQUALS,
    BUFFER_COMPARE,
    BUFFER_STATIC_COMPARE,
    BUFFER_COPY,
    BUFFER_FILL,
    BUFFER_SLICE,
    BUFFER_SUBARRAY,
    BUFFER_ALLOC,
    BUFFER_ALLOC_UNSAFE,
    BUFFER_ALLOC_UNSAFE_SLOW,
    BUFFER_CONCAT,
    BUFFER_IS_BUFFER,
    BUFFER_OF,
    ARRAY_IS_ARRAY,
    ARRAY_AT,
    ARRAY_INCLUDES,
    ARRAY_INDEX_OF,
    ARRAY_LAST_INDEX_OF,
    ARRAY_FIND,
    ARRAY_FIND_INDEX,
    ARRAY_FIND_LAST,
    ARRAY_FIND_LAST_INDEX,
    ARRAY_FOR_EACH,
    ARRAY_MAP,
    ARRAY_FILTER,
    ARRAY_SOME,
    ARRAY_EVERY,
    ARRAY_REDUCE,
    ARRAY_REDUCE_RIGHT,
    ARRAY_FLAT_MAP,
    ARRAY_PUSH,
    ARRAY_POP,
    ARRAY_SHIFT,
    ARRAY_UNSHIFT,
    ARRAY_SPLICE,
    ARRAY_REVERSE,
    ARRAY_COPY_WITHIN,
    ARRAY_FILL,
    ARRAY_SLICE,
    ARRAY_CONCAT,
    ARRAY_SORT,
    ARRAY_FLAT,
    DATAVIEW_GET_INT8,
    DATAVIEW_GET_UINT8,
    DATAVIEW_SET_INT8,
    DATAVIEW_SET_UINT8,
    DATAVIEW_GET_INT16,
    DATAVIEW_GET_UINT16,
    DATAVIEW_SET_INT16,
    DATAVIEW_SET_UINT16,
    DATAVIEW_GET_INT32,
    DATAVIEW_GET_UINT32,
    DATAVIEW_SET_INT32,
    DATAVIEW_SET_UINT32,
    DATAVIEW_GET_FLOAT16,
    DATAVIEW_SET_FLOAT16,
    DATAVIEW_GET_FLOAT32,
    DATAVIEW_GET_FLOAT64,
    DATAVIEW_SET_FLOAT32,
    DATAVIEW_SET_FLOAT64,
    DATAVIEW_GET_BIG_INT64,
    DATAVIEW_GET_BIG_UINT64,
    DATAVIEW_SET_BIG_INT64,
    DATAVIEW_SET_BIG_UINT64,
    TYPEDARRAY_AT,
    TYPEDARRAY_INCLUDES,
    TYPEDARRAY_INDEX_OF,
    TYPEDARRAY_LAST_INDEX_OF,
    TYPEDARRAY_FILL,
    TYPEDARRAY_REVERSE,
    TYPEDARRAY_COPY_WITHIN,
    TYPEDARRAY_SET,
    TYPEDARRAY_SLICE,
    TYPEDARRAY_SUBARRAY,
    TYPEDARRAY_TO_REVERSED,
    TYPEDARRAY_TO_SORTED,
    ARRAYBUFFER_IS_VIEW,
    ARRAYBUFFER_SLICE,
    ARRAYBUFFER_RESIZE,
    ARRAYBUFFER_TRANSFER,
    ARRAYBUFFER_TRANSFER_TO_FIXED_LENGTH,
];

pub const fn string_char_code_at_generated_ihi_spec() -> GeneratedIhiSpec {
    match crate::native_api_manifest_generated::string_char_code_at_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("String.prototype.charCodeAt must have generated IHI facts"),
    }
}

pub const fn string_char_code_at_generated_registration_spec() -> GeneratedRegistrationSpec {
    crate::native_api_manifest_generated::string_char_code_at_generated_registration_spec()
}

pub const fn string_char_code_at_generated_validation_spec() -> GeneratedValidationSpec {
    crate::native_api_manifest_generated::string_char_code_at_generated_validation_spec()
}

pub const fn string_char_code_at_generated_fixture_doc_spec() -> GeneratedFixtureDocSpec {
    STRING_CHAR_CODE_AT.fixture_doc
}

pub fn string_char_code_at_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    crate::native_api_manifest_generated::string_char_code_at_generated_validation_args(args)
}

pub fn string_char_code_at_generated_jit_ic_spec() -> Option<GeneratedJitIcSpec> {
    crate::native_api_manifest_generated::string_char_code_at_generated_jit_ic_spec()
}

pub fn string_char_code_at_generated_lejit_expectation_spec(
) -> Option<GeneratedLejitExpectationSpec> {
    crate::native_api_manifest_generated::string_char_code_at_generated_lejit_expectation_spec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Runtime, Value};
    use std::rc::Rc;

    fn string_value(s: &str) -> Value {
        Value::String(Rc::new(crate::value::JsString::from(s)))
    }

    #[test]
    fn string_char_code_at_manifest_feeds_live_ihi_entry() {
        let row = STRING_CHAR_CODE_AT;
        let generated = string_char_code_at_generated_ihi_spec();
        let generated_entry = crate::interp_ic_table::generated_string_char_code_at_ihi_entry();
        assert_eq!(generated.key, row.property);
        assert_eq!(generated.arity, Some(row.arity));
        assert_eq!(
            generated.fast_fn,
            row.fast_path.rsplit("::").next().unwrap()
        );

        let live = crate::interp_ic_table::lookup(
            generated.key,
            generated.receiver,
            generated.arity.expect("method arity"),
        )
        .expect("live IHI entry for String.prototype.charCodeAt");

        assert_eq!(generated_entry.key, generated.key);
        assert_eq!(generated_entry.receiver, generated.receiver);
        assert_eq!(generated_entry.arity, generated.arity);
        assert_eq!(generated_entry.cached_id_field, generated.cached_id_field);

        assert_eq!(live.key, generated_entry.key);
        assert_eq!(live.receiver, generated_entry.receiver);
        assert_eq!(live.arity, generated_entry.arity);
        assert_eq!(live.cached_id_field, generated_entry.cached_id_field);
    }

    #[test]
    fn string_char_code_at_manifest_feeds_live_registration_descriptor() {
        let generated = string_char_code_at_generated_registration_spec();
        assert_eq!(generated.property, STRING_CHAR_CODE_AT.property);
        assert_eq!(generated.length, STRING_CHAR_CODE_AT.arity as u32);
        assert_eq!(generated.constructor, STRING_CHAR_CODE_AT.constructor);

        let mut rt = Runtime::new();
        rt.install_intrinsics();
        let string_proto = rt
            .string_prototype
            .expect("Runtime::new installs String.prototype");
        let desc = rt
            .obj(string_proto)
            .get_own(generated.property)
            .expect("live String.prototype.charCodeAt descriptor");

        assert_eq!(desc.writable, generated.writable);
        assert_eq!(desc.enumerable, generated.enumerable);
        assert_eq!(desc.configurable, generated.configurable);

        let fn_id = match desc.value {
            Value::Object(id) => id,
            ref other => panic!("expected function object, got {other:?}"),
        };
        let function = rt.obj(fn_id);
        assert_eq!(
            rt.object_get(fn_id, "name"),
            string_value(generated.display_name)
        );
        assert_eq!(
            rt.object_get(fn_id, "length"),
            Value::Number(generated.length as f64)
        );
        match &function.internal_kind {
            crate::value::InternalKind::Function(internals) => {
                assert_eq!(internals.name, generated.display_name);
                assert_eq!(internals.length, generated.length);
                assert_eq!(internals.is_constructor, generated.constructor);
            }
            other => panic!("expected Function internals, got {other:?}"),
        }
    }

    #[test]
    fn string_char_code_at_manifest_feeds_validation_binding() {
        let generated = string_char_code_at_generated_validation_spec();
        assert_eq!(generated.receiver, STRING_CHAR_CODE_AT.receiver);
        assert_eq!(generated.arity, STRING_CHAR_CODE_AT.arity);
        assert_eq!(generated.args, STRING_CHAR_CODE_AT.args);
        assert_eq!(generated.returns, STRING_CHAR_CODE_AT.returns);
        assert_eq!(
            generated.index_coercion,
            NativeApiIndexCoercion::ToIntegerOrInfinity
        );
        assert_eq!(
            generated.negative_or_infinite_result,
            NativeApiExceptionalResult::NumberNaN
        );
        assert_eq!(
            generated.out_of_range_result,
            NativeApiExceptionalResult::NumberNaN
        );

        assert_eq!(
            string_char_code_at_generated_validation_args(&[]),
            [Value::Undefined]
        );
        assert_eq!(
            string_char_code_at_generated_validation_args(&[Value::Number(1.8)]),
            [Value::Number(1.8)]
        );

        let mut rt = Runtime::new();
        assert_eq!(
            crate::generated::string_prototype_char_code_at(
                &mut rt,
                string_value("ABC"),
                &string_char_code_at_generated_validation_args(&[])
            )
            .unwrap(),
            Value::Number(65.0)
        );
        assert_eq!(
            crate::generated::string_prototype_char_code_at(
                &mut rt,
                string_value("ABC"),
                &string_char_code_at_generated_validation_args(&[Value::Number(1.8)])
            )
            .unwrap(),
            Value::Number(66.0)
        );
    }

    #[test]
    fn string_char_code_at_manifest_matches_cruftscript_stdlib_signature() {
        let row = STRING_CHAR_CODE_AT;
        let generated_witness = crate::native_api_manifest_generated::
            string_char_code_at_generated_cruftscript_stdlib_signature_spec()
                .expect("String.prototype.charCodeAt must have generated CruftScript facts");
        let generated =
            cruftscript_type_checker::string_char_code_at_generated_stdlib_signature_spec();
        assert_eq!(generated_witness.api, row.api);
        assert_eq!(generated_witness.receiver, "String");
        assert_eq!(generated_witness.property, row.property);
        assert_eq!(generated_witness.arity, row.arity);
        assert_eq!(generated_witness.args, &["Number"]);
        assert_eq!(generated_witness.returns, &["Number"]);
        assert!(generated_witness.nullish_receiver_rejects);
        assert!(generated_witness.boundary_safe);
        assert_eq!(generated.api, row.api);
        assert_eq!(generated.property, row.property);
        assert_eq!(generated.arity, row.arity);
        assert_eq!(
            generated.receiver,
            cruftscript_type_checker::GeneratedStdlibReceiver::String
        );
        assert_eq!(
            generated.args,
            &[cruftscript_type_checker::GeneratedStdlibDomain::Number]
        );
        assert_eq!(
            generated.returns,
            &[cruftscript_type_checker::GeneratedStdlibDomain::Number]
        );
        assert!(matches!(row.receiver, NativeApiReceiver::String));
        assert_eq!(row.args, &[NativeApiDomain::Number]);
        assert_eq!(row.returns, &[NativeApiDomain::Number]);
        assert!(generated.nullish_receiver_rejects);
        assert!(generated.boundary_safe);
    }

    #[test]
    fn string_char_code_at_manifest_generates_fixture_doc_surface() {
        let row = STRING_CHAR_CODE_AT;
        let generated = string_char_code_at_generated_fixture_doc_spec();
        assert_eq!(generated.api, row.api);
        assert_eq!(generated.cases.len(), 6);

        let has_case = |id: &str, surface: &str, needle: &str| {
            generated.cases.iter().any(|case| {
                case.id == id && case.surface == surface && case.expectation.contains(needle)
            })
        };

        assert!(has_case(
            "registration-descriptor",
            "runtime-registration",
            "length=1"
        ));
        assert!(has_case(
            "validation-binding",
            "runtime-validation",
            "args=[number]"
        ));
        assert!(has_case(
            "ihi-generated-consumer",
            "interpreter-hot-intrinsic",
            "cached_id=StringCharCodeAt"
        ));
        assert!(has_case(
            "lejit-expectation",
            "lejit",
            "deopt_on_receiver_arg_or_override"
        ));
        assert!(has_case(
            "cruftscript-stdlib-signature",
            "cruftscript",
            "string.charCodeAt(number): number"
        ));
        assert!(has_case(
            "override-bailout",
            "runtime-bailout",
            "method_identity_unchanged"
        ));
    }

    #[test]
    fn string_char_code_at_manifest_matches_hand_jit_ic_entry() {
        let generated = string_char_code_at_generated_jit_ic_spec()
            .expect("charCodeAt has a generated JIT IC mirror");
        let idx = rusty_js_jit::ic_table::lookup_by_key(generated.key)
            .expect("hand LeJIT IC entry for String.prototype.charCodeAt");
        let hand = &rusty_js_jit::ic_table::IC_TABLE[idx as usize];

        assert_eq!(hand.key, generated.key);
        assert_eq!(hand.extern_name, generated.extern_name);
        assert!(matches!(
            hand.receiver,
            rusty_js_jit::ic_table::ReceiverKind::String
        ));
        match hand.kind {
            rusty_js_jit::ic_table::IcEntryKind::MethodCall { arity } => {
                assert_eq!(Some(arity), generated.arity);
            }
            rusty_js_jit::ic_table::IcEntryKind::PropertyGet => {
                panic!("expected MethodCall JIT IC entry")
            }
        }
    }

    #[test]
    fn string_char_code_at_manifest_feeds_lejit_expectation_metadata() {
        let generated = string_char_code_at_generated_lejit_expectation_spec()
            .expect("charCodeAt has generated LeJIT expectation metadata");
        let idx = rusty_js_jit::ic_table::lookup_by_key(generated.key)
            .expect("hand LeJIT IC entry for String.prototype.charCodeAt");
        let hand = &rusty_js_jit::ic_table::IC_TABLE[idx as usize];
        let consumed = rusty_js_jit::ic_table::expectation_metadata_for_entry(hand)
            .expect("LeJIT expectation metadata for String.prototype.charCodeAt");

        assert_eq!(consumed.key, generated.key);
        assert_eq!(consumed.receiver, generated.receiver);
        assert_eq!(consumed.kind, generated.kind);
        assert_eq!(consumed.arity, generated.arity);
        assert_eq!(consumed.extern_name, generated.extern_name);
        assert_eq!(consumed.arg_domains, generated.arg_domains);
        assert_eq!(consumed.return_domain, generated.return_domain);
        assert_eq!(consumed.override_guard, generated.override_guard);
        assert_eq!(consumed.deopt_bailouts, generated.deopt_bailouts);
    }

    #[test]
    fn string_arity_zero_family_is_manifested_and_jit_ineligible() {
        for row in [STRING_TO_LOWER_CASE, STRING_TO_UPPER_CASE, STRING_TRIM] {
            assert_eq!(row.receiver, NativeApiReceiver::String);
            assert_eq!(row.kind, NativeApiKind::Method);
            assert_eq!(row.arity, 0);
            assert_eq!(row.args, &[]);
            assert_eq!(row.returns, &[NativeApiDomain::String]);
            assert_eq!(row.registration.property, row.property);
            assert_eq!(row.registration.length, 0);
            assert_eq!(row.validation.args, &[]);
            assert_eq!(row.validation.returns, &[NativeApiDomain::String]);
            assert!(row.jit_ic.is_none());
            assert!(row.lejit_expectation.is_none());

            let live = crate::interp_ic_table::lookup(
                row.ihi.expect("manifest row has IHI facts").key,
                row.ihi.expect("manifest row has IHI facts").receiver,
                0,
            )
            .expect("live generated IHI entry for arity-zero String method");
            assert_eq!(live.key, row.property);
            assert_eq!(
                live.receiver,
                row.ihi.expect("manifest row has IHI facts").receiver
            );
            assert_eq!(
                live.arity,
                row.ihi.expect("manifest row has IHI facts").arity
            );
            assert_eq!(
                live.cached_id_field,
                row.ihi.expect("manifest row has IHI facts").cached_id_field
            );
        }
    }

    #[test]
    fn string_case_scalar_runtime_matches_node_unicode_oracle() {
        let actual = run_manifest_json(
            r#"
            JSON.stringify({
              shape: ['toLowerCase','toUpperCase'].map((m) => {
                const f = String.prototype[m];
                const d = Object.getOwnPropertyDescriptor(String.prototype, m);
                let ctor;
                try { new f(); ctor = 'ok'; } catch (e) { ctor = e.name; }
                let nullish;
                try { f.call(null); nullish = 'ok'; } catch (e) { nullish = e.name; }
                return [m, f.length, d.writable, d.enumerable, d.configurable, ctor, nullish];
              }),
              calls: ['ABCxyz','Σ','ΟΣ','ΟΣ ','AΣ','ΑΣΑ','İ','I','ı','ß','ﬃ','𐐨','\uD83D','\uDE00','\uD83D\uDE00'].map((s) => {
                return [s, s.toLowerCase(), s.toUpperCase()];
              }),
              override: (() => {
                String.prototype.toLowerCase = function(){ return 77; };
                return 'abc'.toLowerCase();
              })()
            })
            "#,
        );
        assert_eq!(
            actual,
            "{\"shape\":[[\"toLowerCase\",0,true,false,true,\"TypeError\",\"TypeError\"],[\"toUpperCase\",0,true,false,true,\"TypeError\",\"TypeError\"]],\"calls\":[[\"ABCxyz\",\"abcxyz\",\"ABCXYZ\"],[\"Σ\",\"σ\",\"Σ\"],[\"ΟΣ\",\"ος\",\"ΟΣ\"],[\"ΟΣ \",\"ος \",\"ΟΣ \"],[\"AΣ\",\"aς\",\"AΣ\"],[\"ΑΣΑ\",\"ασα\",\"ΑΣΑ\"],[\"İ\",\"i̇\",\"İ\"],[\"I\",\"i\",\"I\"],[\"ı\",\"ı\",\"I\"],[\"ß\",\"ß\",\"SS\"],[\"ﬃ\",\"ﬃ\",\"FFI\"],[\"𐐨\",\"𐐨\",\"𐐀\"],[\"\\ud83d\",\"\\ud83d\",\"\\ud83d\"],[\"\\ude00\",\"\\ude00\",\"\\ude00\"],[\"😀\",\"😀\",\"😀\"]],\"override\":77}"
        );
    }

    #[test]
    fn string_predicate_family_is_manifested_and_jit_ineligible() {
        for row in [STRING_INCLUDES, STRING_STARTS_WITH, STRING_ENDS_WITH] {
            assert_eq!(row.receiver, NativeApiReceiver::String);
            assert_eq!(row.kind, NativeApiKind::Method);
            assert_eq!(row.arity, 1);
            assert_eq!(row.args, &[NativeApiDomain::String]);
            assert_eq!(row.returns, &[NativeApiDomain::Boolean]);
            assert_eq!(row.registration.property, row.property);
            assert_eq!(row.registration.length, 1);
            assert_eq!(row.validation.args, &[NativeApiDomain::String]);
            assert_eq!(row.validation.returns, &[NativeApiDomain::Boolean]);
            assert!(row.validation.missing_arg_defaults_to_undefined);
            assert!(row.jit_ic.is_none());
            assert!(row.lejit_expectation.is_none());

            let live = crate::interp_ic_table::lookup(
                row.ihi.expect("manifest row has IHI facts").key,
                row.ihi.expect("manifest row has IHI facts").receiver,
                1,
            )
            .expect("live generated IHI entry for one-argument String predicate");
            assert_eq!(live.key, row.property);
            assert_eq!(
                live.receiver,
                row.ihi.expect("manifest row has IHI facts").receiver
            );
            assert_eq!(
                live.arity,
                row.ihi.expect("manifest row has IHI facts").arity
            );
            assert_eq!(
                live.cached_id_field,
                row.ihi.expect("manifest row has IHI facts").cached_id_field
            );
        }
    }

    #[test]
    fn string_index_of_arity_one_manifest_feeds_live_consumers() {
        let row = STRING_INDEX_OF;
        assert_eq!(row.receiver, NativeApiReceiver::String);
        assert_eq!(row.kind, NativeApiKind::Method);
        assert_eq!(row.arity, 1);
        assert_eq!(row.args, &[NativeApiDomain::String]);
        assert_eq!(row.returns, &[NativeApiDomain::Number]);
        assert_eq!(row.registration.property, row.property);
        assert_eq!(row.registration.length, 1);
        assert_eq!(row.validation.args, &[NativeApiDomain::String]);
        assert_eq!(row.validation.returns, &[NativeApiDomain::Number]);
        assert!(row.jit_ic.is_none());
        assert!(row.lejit_expectation.is_none());

        let live = crate::interp_ic_table::lookup(
            row.ihi.expect("manifest row has IHI facts").key,
            row.ihi.expect("manifest row has IHI facts").receiver,
            1,
        )
        .expect("live generated IHI entry for String.prototype.indexOf arity one");
        assert_eq!(live.key, row.property);
        assert_eq!(
            live.receiver,
            row.ihi.expect("manifest row has IHI facts").receiver
        );
        assert_eq!(
            live.arity,
            row.ihi.expect("manifest row has IHI facts").arity
        );
        assert_eq!(
            live.cached_id_field,
            row.ihi.expect("manifest row has IHI facts").cached_id_field
        );
    }

    #[test]
    fn string_last_index_of_manifest_is_generated_checked_and_fast_path_ineligible() {
        let row = STRING_LAST_INDEX_OF;
        assert_eq!(row.receiver, NativeApiReceiver::String);
        assert_eq!(row.kind, NativeApiKind::Method);
        assert_eq!(row.arity, 1);
        assert_eq!(row.args, &[NativeApiDomain::String]);
        assert_eq!(row.returns, &[NativeApiDomain::Number]);
        assert_eq!(row.registration.property, row.property);
        assert_eq!(row.registration.length, 1);
        assert_eq!(row.validation.args, &[NativeApiDomain::String]);
        assert_eq!(row.validation.returns, &[NativeApiDomain::Number]);
        assert!(row.validation.missing_arg_defaults_to_undefined);
        assert!(row.ihi.is_none());
        assert!(row.jit_ic.is_none());
        assert!(row.lejit_expectation.is_none());
    }

    #[test]
    fn string_search_predicate_runtime_matches_node_utf16_oracle() {
        let actual = run_manifest_json(
            r#"
            JSON.stringify({
              shape: ['includes','startsWith','endsWith','indexOf','lastIndexOf'].map((m) => {
                const f = String.prototype[m];
                const d = Object.getOwnPropertyDescriptor(String.prototype, m);
                let ctor;
                try { new f('x'); ctor = 'ok'; } catch (e) { ctor = e.name; }
                return [m, f.length, d.writable, d.enumerable, d.configurable, ctor];
              }),
              calls: [
                'abc'.includes('b'),
                'abc'.includes('', 99),
                'abc'.startsWith('', 99),
                'abc'.endsWith('', 0),
                'abc'.indexOf('', 99),
                'abc'.lastIndexOf('', -1),
                '𝌆x'.indexOf('x'),
                '𝌆x'.includes('x', 2),
                '𝌆x'.startsWith('x', 2),
                '𝌆x'.endsWith('𝌆', 2),
                '𝌆x𝌆'.lastIndexOf('𝌆', 2),
                (() => { try { String.prototype.includes.call(null, 'x'); } catch (e) { return e.name; } })(),
                (() => { try { 'abc'.includes(/b/); } catch (e) { return e.name; } })(),
                (() => { try { 'abc'.startsWith(/b/); } catch (e) { return e.name; } })(),
                (() => { try { 'abc'.endsWith(/b/); } catch (e) { return e.name; } })(),
                'abc'.indexOf(/b/),
                'abc'.lastIndexOf(/b/),
                String.prototype.includes.call(123, '2'),
                String.prototype.indexOf.call({toString(){return 'zaz'}}, 'a', -Infinity),
                'abc'.lastIndexOf('a', NaN),
                'abc'.lastIndexOf('a', undefined),
                'abc'.lastIndexOf('a', Infinity)
              ],
              override: (() => {
                String.prototype.includes = function(){ return 77; };
                return 'abc'.includes('b');
              })()
            })
            "#,
        );
        assert_eq!(
            actual,
            "{\"shape\":[[\"includes\",1,true,false,true,\"TypeError\"],[\"startsWith\",1,true,false,true,\"TypeError\"],[\"endsWith\",1,true,false,true,\"TypeError\"],[\"indexOf\",1,true,false,true,\"TypeError\"],[\"lastIndexOf\",1,true,false,true,\"TypeError\"]],\"calls\":[true,true,true,true,3,0,2,true,true,true,0,\"TypeError\",\"TypeError\",\"TypeError\",\"TypeError\",-1,-1,true,1,0,0,0],\"override\":77}"
        );
    }

    #[test]
    fn string_trim_pad_repeat_manifest_is_generated_checked() {
        let trim = STRING_TRIM;
        assert!(trim.ihi.is_some());
        assert!(trim.jit_ic.is_none());
        assert!(trim.lejit_expectation.is_none());
        for row in [
            STRING_TRIM_START,
            STRING_TRIM_END,
            STRING_REPEAT,
            STRING_PAD_START,
            STRING_PAD_END,
        ] {
            assert_eq!(row.receiver, NativeApiReceiver::String);
            assert_eq!(row.kind, NativeApiKind::Method);
            assert_eq!(row.registration.property, row.property);
            assert_eq!(row.validation.receiver, NativeApiReceiver::String);
            assert!(row.ihi.is_none());
            assert!(row.jit_ic.is_none());
            assert!(row.lejit_expectation.is_none());
        }
        assert_eq!(STRING_TRIM_START.registration.length, 0);
        assert_eq!(STRING_TRIM_END.registration.length, 0);
        assert_eq!(STRING_REPEAT.registration.length, 1);
        assert_eq!(STRING_PAD_START.registration.length, 1);
        assert_eq!(STRING_PAD_END.registration.length, 1);
        assert_eq!(
            STRING_REPEAT.validation.index_coercion,
            NativeApiIndexCoercion::ToIntegerOrInfinity
        );
        assert_eq!(
            STRING_REPEAT.validation.negative_or_infinite_result,
            NativeApiExceptionalResult::RangeError
        );
    }

    #[test]
    fn string_trim_pad_repeat_runtime_matches_node_oracle() {
        let actual = run_manifest_json(
            r#"
            JSON.stringify({
              shape: ['trim','trimStart','trimEnd','repeat','padStart','padEnd'].map((m) => {
                const f = String.prototype[m];
                const d = Object.getOwnPropertyDescriptor(String.prototype, m);
                let ctor;
                try { new f('x'); ctor = 'ok'; } catch (e) { ctor = e.name; }
                let nullish;
                try { f.call(null); nullish = 'ok'; } catch (e) { nullish = e.name; }
                return [m, f.length, d.writable, d.enumerable, d.configurable, ctor, nullish];
              }),
              calls: (() => {
                const ws = '\u0009\u000b\u000c\u0020\u00a0\ufeff\u2028\u2029x\u0009\u000b\u000c\u0020\u00a0\ufeff\u2028\u2029';
                return [
                  [ws.trim(), ws.trimStart(), ws.trimEnd()],
                  ['ab'.repeat(), 'ab'.repeat(undefined), 'ab'.repeat(null), 'ab'.repeat(NaN), 'ab'.repeat(2.9),
                    (() => { try { return 'ab'.repeat(-1); } catch (e) { return e.name + ':' + e.message; } })(),
                    (() => { try { return 'ab'.repeat(Infinity); } catch (e) { return e.name + ':' + e.message; } })(),
                    (() => { try { return 'ab'.repeat(-Infinity); } catch (e) { return e.name + ':' + e.message; } })()],
                  ['x'.padStart(), 'x'.padStart(undefined), 'x'.padStart(null), 'x'.padStart(NaN), 'x'.padStart(3), 'x'.padStart(4,'0'), 'x'.padStart(4,''), 'abc'.padStart(6,'💩')],
                  ['x'.padEnd(), 'x'.padEnd(undefined), 'x'.padEnd(null), 'x'.padEnd(NaN), 'x'.padEnd(3), 'x'.padEnd(4,'0'), 'x'.padEnd(4,''), 'abc'.padEnd(6,'💩')]
                ];
              })(),
              override: (() => {
                String.prototype.repeat = function(){ return 77; };
                return 'x'.repeat(2);
              })()
            })
            "#,
        );
        assert_eq!(
            actual,
            "{\"shape\":[[\"trim\",0,true,false,true,\"TypeError\",\"TypeError\"],[\"trimStart\",0,true,false,true,\"TypeError\",\"TypeError\"],[\"trimEnd\",0,true,false,true,\"TypeError\",\"TypeError\"],[\"repeat\",1,true,false,true,\"TypeError\",\"TypeError\"],[\"padStart\",1,true,false,true,\"TypeError\",\"TypeError\"],[\"padEnd\",1,true,false,true,\"TypeError\",\"TypeError\"]],\"calls\":[[\"x\",\"x\\t\\u000b\\f  ﻿  \",\"\\t\\u000b\\f  ﻿  x\"],[\"\",\"\",\"\",\"\",\"abab\",\"RangeError:Invalid count value: -1\",\"RangeError:Invalid count value: Infinity\",\"RangeError:Invalid count value: -Infinity\"],[\"x\",\"x\",\"x\",\"x\",\"  x\",\"000x\",\"x\",\"💩\\ud83dabc\"],[\"x\",\"x\",\"x\",\"x\",\"x  \",\"x000\",\"x\",\"abc💩\\ud83d\"]],\"override\":77}"
        );
    }

    #[test]
    fn string_range_index_manifest_is_generated_checked_and_fast_path_ineligible() {
        for row in [
            STRING_CHAR_AT,
            STRING_AT,
            STRING_SLICE,
            STRING_SUBSTRING,
            STRING_SUBSTR,
        ] {
            assert_eq!(row.receiver, NativeApiReceiver::String);
            assert_eq!(row.kind, NativeApiKind::Method);
            assert_eq!(row.registration.property, row.property);
            assert_eq!(row.validation.receiver, NativeApiReceiver::String);
            assert!(row.validation.missing_arg_defaults_to_undefined);
            assert_eq!(
                row.validation.index_coercion,
                NativeApiIndexCoercion::ToIntegerOrInfinity
            );
            assert!(row.ihi.is_none());
            assert!(row.jit_ic.is_none());
            assert!(row.lejit_expectation.is_none());
        }
        assert_eq!(STRING_CHAR_AT.registration.length, 1);
        assert_eq!(
            STRING_AT.returns,
            &[NativeApiDomain::String, NativeApiDomain::Undefined]
        );
        assert_eq!(STRING_SLICE.registration.length, 2);
        assert_eq!(STRING_SUBSTRING.registration.length, 2);
        assert_eq!(STRING_SUBSTR.registration.length, 2);
        assert_eq!(
            STRING_CHAR_AT.validation.out_of_range_result,
            NativeApiExceptionalResult::EmptyString
        );
    }

    #[test]
    fn string_range_index_runtime_matches_node_utf16_oracle() {
        let actual = run_manifest_json(
            r#"
            JSON.stringify({
              shape: ['charAt','at','codePointAt','slice','substring','substr'].map((m) => {
                const f = String.prototype[m];
                const d = Object.getOwnPropertyDescriptor(String.prototype, m);
                let ctor;
                try { new f('x'); ctor = 'ok'; } catch (e) { ctor = e.name; }
                let nullish;
                try { f.call(null, 0); nullish = 'ok'; } catch (e) { nullish = e.name; }
                return [m, f.length, d.writable, d.enumerable, d.configurable, ctor, nullish];
              }),
              calls: (() => {
                const s = '𝌆x𝌆';
                return [
                  [s.charAt(), s.charAt(0), s.charAt(1), s.charAt(2), s.charAt(99), s.charAt(-1), s.charAt(NaN), s.charAt(Infinity), s.charAt({valueOf: function(){return 2.9;}})],
                  [s.at(), s.at(0), s.at(1), s.at(2), s.at(-1), s.at(-2), s.at(99), s.at(NaN), s.at(Infinity)],
                  [s.codePointAt(), s.codePointAt(0), s.codePointAt(1), s.codePointAt(2), s.codePointAt(-1), s.codePointAt(99), s.codePointAt(NaN), s.codePointAt(Infinity)],
                  [s.slice(), s.slice(0,2), s.slice(1,2), s.slice(-2), s.slice(1,-1), s.slice(Infinity), s.slice(NaN,2)],
                  [s.substring(), s.substring(0,2), s.substring(1,2), s.substring(2,0), s.substring(-2,2), s.substring(Infinity,1), s.substring(NaN,2)],
                  [s.substr(), s.substr(0,2), s.substr(1,2), s.substr(-2,2), s.substr(1,-1), s.substr(Infinity), s.substr(NaN,2)]
                ];
              })(),
              override: (() => {
                String.prototype.slice = function(){ return 77; };
                return 'abc'.slice(1);
              })()
            })
            "#,
        );
        assert_eq!(
            actual,
            "{\"shape\":[[\"charAt\",1,true,false,true,\"TypeError\",\"TypeError\"],[\"at\",1,true,false,true,\"TypeError\",\"TypeError\"],[\"codePointAt\",1,true,false,true,\"TypeError\",\"TypeError\"],[\"slice\",2,true,false,true,\"TypeError\",\"TypeError\"],[\"substring\",2,true,false,true,\"TypeError\",\"TypeError\"],[\"substr\",2,true,false,true,\"TypeError\",\"TypeError\"]],\"calls\":[[\"\\ud834\",\"\\ud834\",\"\\udf06\",\"x\",\"\",\"\",\"\\ud834\",\"\",\"x\"],[\"\\ud834\",\"\\ud834\",\"\\udf06\",\"x\",\"\\udf06\",\"\\ud834\",null,\"\\ud834\",null],[119558,119558,57094,120,null,null,119558,null],[\"𝌆x𝌆\",\"𝌆\",\"\\udf06\",\"𝌆\",\"\\udf06x\\ud834\",\"\",\"𝌆\"],[\"𝌆x𝌆\",\"𝌆\",\"\\udf06\",\"𝌆\",\"𝌆\",\"\\udf06x𝌆\",\"𝌆\"],[\"𝌆x𝌆\",\"𝌆\",\"\\udf06x\",\"𝌆\",\"\",\"\",\"𝌆\"]],\"override\":77}"
        );
    }

    #[test]
    fn string_code_point_at_manifest_feeds_live_consumers() {
        let row = STRING_CODE_POINT_AT;
        assert_eq!(row.receiver, NativeApiReceiver::String);
        assert_eq!(row.kind, NativeApiKind::Method);
        assert_eq!(row.arity, 1);
        assert_eq!(row.args, &[NativeApiDomain::Number]);
        assert_eq!(
            row.returns,
            &[NativeApiDomain::Number, NativeApiDomain::Undefined]
        );
        assert_eq!(row.registration.property, row.property);
        assert_eq!(row.registration.length, 1);
        assert_eq!(row.validation.args, &[NativeApiDomain::Number]);
        assert_eq!(
            row.validation.returns,
            &[NativeApiDomain::Number, NativeApiDomain::Undefined]
        );
        assert_eq!(
            row.validation.negative_or_infinite_result,
            NativeApiExceptionalResult::Undefined
        );
        assert_eq!(
            row.validation.out_of_range_result,
            NativeApiExceptionalResult::Undefined
        );

        let live = crate::interp_ic_table::lookup(
            row.ihi.expect("manifest row has IHI facts").key,
            row.ihi.expect("manifest row has IHI facts").receiver,
            1,
        )
        .expect("live generated IHI entry for String.prototype.codePointAt");
        assert_eq!(live.key, row.property);
        assert_eq!(
            live.receiver,
            row.ihi.expect("manifest row has IHI facts").receiver
        );
        assert_eq!(
            live.arity,
            row.ihi.expect("manifest row has IHI facts").arity
        );
        assert_eq!(
            live.cached_id_field,
            row.ihi.expect("manifest row has IHI facts").cached_id_field
        );

        let jit = row
            .jit_ic
            .expect("codePointAt has a generated JIT IC mirror");
        let idx = rusty_js_jit::ic_table::lookup_by_key(jit.key)
            .expect("hand JIT IC entry for String.prototype.codePointAt");
        let hand = &rusty_js_jit::ic_table::IC_TABLE[idx as usize];
        assert_eq!(hand.key, jit.key);
        assert_eq!(hand.extern_name, jit.extern_name);
        assert_eq!(row.lejit_expectation, None);
    }

    #[test]
    fn string_code_point_at_manifest_preserves_undefined_result() {
        let mut rt = Runtime::new();
        assert_eq!(
            crate::generated::string_prototype_code_point_at(
                &mut rt,
                string_value("ABC"),
                &crate::native_api_manifest_generated::
                    string_code_point_at_generated_validation_args(&[Value::Number(99.0)])
            )
            .unwrap(),
            Value::Undefined
        );
        assert_eq!(
            crate::native_api_manifest_generated::
                string_code_point_at_generated_cruftscript_stdlib_signature_spec()
                .expect("String.prototype.codePointAt must have generated CruftScript facts")
                .returns,
            &["Number", "Undefined"]
        );
    }

    #[test]
    fn buffer_byte_numeric_manifest_feeds_live_ihi_and_jit_facts() {
        for row in [
            BUFFER_READ_UINT8,
            BUFFER_READ_INT8,
            BUFFER_WRITE_UINT8,
            BUFFER_WRITE_INT8,
        ] {
            assert_eq!(row.receiver, NativeApiReceiver::Buffer);
            assert_eq!(row.kind, NativeApiKind::Method);
            assert_eq!(row.returns, &[NativeApiDomain::Number]);
            assert_eq!(row.validation.receiver, NativeApiReceiver::Buffer);
            assert_eq!(row.validation.byte_width, 1);
            assert_eq!(row.validation.endian, NativeApiEndian::SingleByte);
            assert_eq!(
                row.validation.negative_or_infinite_result,
                NativeApiExceptionalResult::RangeError
            );
            assert_eq!(
                row.validation.out_of_range_result,
                NativeApiExceptionalResult::RangeError
            );
            assert_eq!(row.registration.property, row.property);

            let live = crate::interp_ic_table::lookup(
                row.ihi.expect("manifest row has IHI facts").key,
                row.ihi.expect("manifest row has IHI facts").receiver,
                row.arity,
            )
            .expect("live generated IHI entry for Buffer byte numeric mouth");
            assert_eq!(live.key, row.property);
            assert_eq!(
                live.receiver,
                row.ihi.expect("manifest row has IHI facts").receiver
            );
            assert_eq!(
                live.arity,
                row.ihi.expect("manifest row has IHI facts").arity
            );
            assert_eq!(
                live.cached_id_field,
                row.ihi.expect("manifest row has IHI facts").cached_id_field
            );

            if let Some(jit) = row.jit_ic {
                let idx = rusty_js_jit::ic_table::lookup_by_key(jit.key)
                    .expect("hand JIT IC entry for generated Buffer mouth");
                let hand = &rusty_js_jit::ic_table::IC_TABLE[idx as usize];
                assert_eq!(hand.key, jit.key);
                assert_eq!(hand.receiver, rusty_js_jit::ic_table::ReceiverKind::Buffer);
                assert_eq!(hand.extern_name, jit.extern_name);
            }
            assert_eq!(row.lejit_expectation, None);
        }
    }

    #[test]
    fn buffer_byte_numeric_manifest_distinguishes_reads_from_writes() {
        for row in [BUFFER_READ_UINT8, BUFFER_READ_INT8] {
            assert_eq!(row.arity, 1);
            assert_eq!(row.args, &[NativeApiDomain::Number]);
            assert_eq!(row.validation.args, &[NativeApiDomain::Number]);
            assert_eq!(row.validation.effect, NativeApiEffect::Pure);
        }

        for row in [BUFFER_WRITE_UINT8, BUFFER_WRITE_INT8] {
            assert_eq!(row.arity, 2);
            assert_eq!(
                row.args,
                &[NativeApiDomain::Number, NativeApiDomain::Number]
            );
            assert_eq!(
                row.validation.args,
                &[NativeApiDomain::Number, NativeApiDomain::Number]
            );
            assert_eq!(row.validation.effect, NativeApiEffect::MutatesReceiverBytes);
        }

        assert!(!BUFFER_READ_UINT8.validation.signed);
        assert!(BUFFER_READ_INT8.validation.signed);
        assert!(!BUFFER_WRITE_UINT8.validation.signed);
        assert!(BUFFER_WRITE_INT8.validation.signed);
        assert_eq!(
            crate::native_api_manifest_generated::
                buffer_write_uint8_generated_cruftscript_stdlib_signature_spec()
                .expect("Buffer.prototype.writeUInt8 must have generated CruftScript facts")
                .boundary_safe,
            false
        );
    }

    #[test]
    fn buffer_byte_numeric_manifest_matches_live_registration_descriptors() {
        let mut rt = Runtime::new();
        rt.install_intrinsics();
        let buffer_ctor = match rt.global_get("Buffer") {
            Value::Object(id) => id,
            other => panic!("expected global Buffer constructor, got {other:?}"),
        };
        let buffer_proto = match rt.object_get(buffer_ctor, "prototype") {
            Value::Object(id) => id,
            other => panic!("expected Buffer.prototype object, got {other:?}"),
        };

        for row in [
            BUFFER_READ_UINT8,
            BUFFER_READ_INT8,
            BUFFER_WRITE_UINT8,
            BUFFER_WRITE_INT8,
        ] {
            let desc = rt
                .obj(buffer_proto)
                .get_own(row.property)
                .unwrap_or_else(|| panic!("missing Buffer.prototype.{} descriptor", row.property));
            assert_eq!(desc.writable, row.registration.writable);
            assert_eq!(desc.enumerable, row.registration.enumerable);
            assert_eq!(desc.configurable, row.registration.configurable);
            let fn_id = match rt.object_get(buffer_proto, row.property) {
                Value::Object(id) => id,
                other => panic!(
                    "expected Buffer.prototype.{} function, got {other:?}",
                    row.property
                ),
            };
            let function = rt.obj(fn_id);
            assert_eq!(rt.object_get(fn_id, "name"), string_value(row.property));
            assert_eq!(
                rt.object_get(fn_id, "length"),
                Value::Number(row.registration.length as f64),
                "{} generated registration length must match live function.length",
                row.property
            );
            match &function.internal_kind {
                crate::value::InternalKind::Function(internals) => {
                    assert_eq!(internals.name, row.registration.display_name);
                    assert_eq!(internals.length, row.registration.length);
                    assert_eq!(internals.is_constructor, row.registration.constructor);
                }
                other => panic!(
                    "expected Buffer.prototype.{} Function internals, got {other:?}",
                    row.property
                ),
            }
        }
    }

    fn run_manifest_json(source: &str) -> String {
        let mut rt = Runtime::new();
        rt.install_intrinsics();
        rt.run_script(
            &format!("globalThis.__manifest_result = ({source});"),
            "native-api-manifest-buffer-byte-test.js",
        )
        .unwrap_or_else(|err| panic!("script failed: {err:?}\n{source}"));
        match rt.global_get("__manifest_result") {
            Value::String(s) => s.as_str().to_string(),
            other => panic!("expected JSON string result, got {other:?}\n{source}"),
        }
    }

    #[test]
    fn buffer_byte_numeric_runtime_read_semantics_match_node_oracle() {
        let actual = run_manifest_json(
            r#"
            JSON.stringify([
              Buffer.from([0,127,128,255]).readUInt8(),
              Buffer.from([0,127,128,255]).readUInt8(3),
              Buffer.from([0,127,128,255]).readInt8(1),
              Buffer.from([0,127,128,255]).readInt8(2),
              Buffer.from([0,127,128,255]).readInt8(3),
              Buffer.prototype.readUInt8.call(new Uint8Array([5]), 0),
              (() => { try { Buffer.from([0]).readUInt8('1'); } catch (e) { return e.name; } })(),
              (() => { try { Buffer.from([0]).readUInt8(null); } catch (e) { return e.name; } })(),
              (() => { try { Buffer.from([0]).readUInt8(1.9); } catch (e) { return e.name; } })(),
              (() => { try { Buffer.from([0]).readUInt8(Infinity); } catch (e) { return e.name; } })(),
              (() => { try { Buffer.from([0]).readUInt8(-1); } catch (e) { return e.name; } })(),
              (() => { try { Buffer.from([0]).readUInt8(1); } catch (e) { return e.name; } })()
            ])
            "#,
        );
        assert_eq!(
            actual,
            r#"[0,255,127,-128,-1,5,"TypeError","TypeError","RangeError","RangeError","RangeError","RangeError"]"#
        );
    }

    #[test]
    fn buffer_byte_numeric_runtime_write_semantics_match_node_oracle() {
        let actual = run_manifest_json(
            r#"
            JSON.stringify([
              (() => { const b = Buffer.from([0,0]); const r = b.writeUInt8(255, 0); return [r, b[0], b[1]]; })(),
              (() => { const b = Buffer.from([0]); const r = b.writeUInt8('7', 0); return [r, b[0]]; })(),
              (() => { const b = Buffer.from([0]); const r = b.writeUInt8(true, 0); return [r, b[0]]; })(),
              (() => { const b = Buffer.from([0]); const r = b.writeUInt8(1.9, 0); return [r, b[0]]; })(),
              (() => { const b = Buffer.from([9]); const r = b.writeUInt8(undefined, 0); return [r, b[0]]; })(),
              (() => { const b = Buffer.from([0]); const r = b.writeInt8(127, 0); return [r, b[0]]; })(),
              (() => { const b = Buffer.from([0]); const r = b.writeInt8(-1, 0); return [r, b[0]]; })(),
              (() => { try { Buffer.from([0]).writeUInt8(256, 0); } catch (e) { return e.name; } })(),
              (() => { try { Buffer.from([0]).writeUInt8(-1, 0); } catch (e) { return e.name; } })(),
              (() => { try { Buffer.from([0]).writeUInt8(1, 1); } catch (e) { return e.name; } })(),
              (() => { try { Buffer.from([0]).writeInt8(128, 0); } catch (e) { return e.name; } })(),
              (() => { try { Buffer.from([0]).writeInt8(-129, 0); } catch (e) { return e.name; } })()
            ])
            "#,
        );
        assert_eq!(
            actual,
            r#"[[1,255,0],[1,7],[1,1],[1,1],[1,0],[1,127],[1,255],"RangeError","RangeError","RangeError","RangeError","RangeError"]"#
        );
    }

    #[test]
    fn buffer_byte_numeric_runtime_shape_override_and_construct_match_manifest() {
        let actual = run_manifest_json(
            r#"
            JSON.stringify([
              ['readUInt8','readInt8','writeUInt8','writeInt8'].map((m) => {
                const d = Object.getOwnPropertyDescriptor(Buffer.prototype, m);
                const pd = Object.getOwnPropertyDescriptor(Buffer.prototype[m], 'prototype');
                return [m, Buffer.prototype[m].length, d.writable, d.enumerable, d.configurable, !!pd, pd && pd.writable, pd && pd.enumerable, pd && pd.configurable];
              }),
              (() => { try { new Buffer.prototype.readUInt8(); } catch (e) { return e.name; } })(),
              (() => { try { new Buffer.prototype.writeUInt8(); } catch (e) { return e.name; } })(),
              (() => {
                const original = Buffer.prototype.readUInt8;
                Buffer.prototype.readUInt8 = function () { return 99; };
                const v = Buffer.from([1]).readUInt8(0);
                Buffer.prototype.readUInt8 = original;
                return v;
              })()
            ])
            "#,
        );
        assert_eq!(
            actual,
            r#"[[["readUInt8",1,true,true,true,true,true,false,false],["readInt8",1,true,true,true,true,true,false,false],["writeUInt8",1,true,true,true,true,true,false,false],["writeInt8",1,true,true,true,true,true,false,false]],"TypeError","TypeError",99]"#
        );
    }

    #[test]
    fn buffer_u16_numeric_manifest_feeds_live_ihi_and_jit_facts() {
        for row in [
            BUFFER_READ_UINT16_LE,
            BUFFER_READ_UINT16_BE,
            BUFFER_READ_INT16_LE,
            BUFFER_READ_INT16_BE,
            BUFFER_WRITE_UINT16_LE,
            BUFFER_WRITE_UINT16_BE,
            BUFFER_WRITE_INT16_LE,
            BUFFER_WRITE_INT16_BE,
        ] {
            assert_eq!(row.receiver, NativeApiReceiver::Buffer);
            assert_eq!(row.kind, NativeApiKind::Method);
            assert_eq!(row.returns, &[NativeApiDomain::Number]);
            assert_eq!(row.registration.length, 1);
            assert!(row.registration.enumerable);
            assert!(!row.registration.constructor);
            assert_eq!(row.validation.receiver, NativeApiReceiver::Buffer);
            assert_eq!(row.validation.byte_width, 2);
            assert!(matches!(
                row.validation.endian,
                NativeApiEndian::Little | NativeApiEndian::Big
            ));
            assert_eq!(
                row.validation.negative_or_infinite_result,
                NativeApiExceptionalResult::RangeError
            );
            assert_eq!(
                row.validation.out_of_range_result,
                NativeApiExceptionalResult::RangeError
            );

            let live = crate::interp_ic_table::lookup(
                row.ihi.expect("manifest row has IHI facts").key,
                row.ihi.expect("manifest row has IHI facts").receiver,
                row.arity,
            )
            .expect("live generated IHI entry for Buffer 16-bit numeric mouth");
            assert_eq!(live.key, row.property);
            assert_eq!(
                live.receiver,
                row.ihi.expect("manifest row has IHI facts").receiver
            );
            assert_eq!(
                live.arity,
                row.ihi.expect("manifest row has IHI facts").arity
            );
            assert_eq!(
                live.cached_id_field,
                row.ihi.expect("manifest row has IHI facts").cached_id_field
            );

            if let Some(jit) = row.jit_ic {
                let idx = rusty_js_jit::ic_table::lookup_by_key(jit.key)
                    .expect("hand JIT IC entry for generated Buffer mouth");
                let hand = &rusty_js_jit::ic_table::IC_TABLE[idx as usize];
                assert_eq!(hand.key, jit.key);
                assert_eq!(hand.receiver, rusty_js_jit::ic_table::ReceiverKind::Buffer);
                assert_eq!(hand.extern_name, jit.extern_name);
            }
            assert_eq!(row.lejit_expectation, None);
        }
    }

    #[test]
    fn buffer_u16_numeric_runtime_shape_override_and_semantics_match_node() {
        let actual = run_manifest_json(
            r#"
            JSON.stringify({
              shape: ['readUInt16LE','readUInt16BE','readInt16LE','readInt16BE','writeUInt16LE','writeUInt16BE','writeInt16LE','writeInt16BE'].map((m) => {
                const d = Object.getOwnPropertyDescriptor(Buffer.prototype, m);
                const pd = Object.getOwnPropertyDescriptor(Buffer.prototype[m], 'prototype');
                return [m, Buffer.prototype[m].length, d.writable, d.enumerable, d.configurable, !!pd, pd && pd.writable, pd && pd.enumerable, pd && pd.configurable];
              }),
              construct: [
                (() => { try { new Buffer.prototype.readUInt16LE(); } catch (e) { return e.name; } })(),
                (() => { try { new Buffer.prototype.writeUInt16BE(); } catch (e) { return e.name; } })()
              ],
              reads: [
                Buffer.from([0x12,0x34,0xff,0x80]).readUInt16LE(0),
                Buffer.from([0x12,0x34,0xff,0x80]).readUInt16BE(0),
                Buffer.from([0x12,0x34,0xff,0x80]).readInt16LE(2),
                Buffer.from([0x12,0x34,0xff,0x80]).readInt16BE(2),
                Buffer.prototype.readUInt16BE.call(new Uint8Array([1,2]), 0),
                (() => { try { Buffer.from([0]).readUInt16LE('1'); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.from([0]).readUInt16LE(null); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.from([0]).readUInt16LE(1.9); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.from([0]).readUInt16LE(Infinity); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.from([0]).readUInt16LE(-1); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.from([0]).readUInt16LE(0); } catch (e) { return e.name; } })()
              ],
              writes: [
                (() => { const b = Buffer.from([0,0,0]); const r = b.writeUInt16BE(0x1234,0); return [r,b[0],b[1],b[2]]; })(),
                (() => { const b = Buffer.from([0,0,0]); const r = b.writeUInt16LE(0x1234,0); return [r,b[0],b[1],b[2]]; })(),
                (() => { const b = Buffer.from([0,0]); const r = b.writeInt16BE(-1,0); return [r,b[0],b[1]]; })(),
                (() => { const b = Buffer.from([0,0]); const r = b.writeInt16LE(-32768,0); return [r,b[0],b[1]]; })(),
                (() => { const b = Buffer.from([0,0]); const r = b.writeUInt16BE('7',0); return [r,b[0],b[1]]; })(),
                (() => { const b = Buffer.from([0,0]); const r = b.writeUInt16BE(true,0); return [r,b[0],b[1]]; })(),
                (() => { const b = Buffer.from([0,0]); const r = b.writeUInt16BE(1.9,0); return [r,b[0],b[1]]; })(),
                (() => { const b = Buffer.from([9,9]); const r = b.writeUInt16BE(undefined,0); return [r,b[0],b[1]]; })(),
                (() => { try { Buffer.from([0,0]).writeUInt16BE(65536,0); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.from([0,0]).writeUInt16BE(-1,0); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.from([0,0]).writeInt16BE(32768,0); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.from([0,0]).writeInt16BE(-32769,0); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.from([0]).writeUInt16BE(1,0); } catch (e) { return e.name; } })()
              ],
              override: (() => {
                const original = Buffer.prototype.readUInt16BE;
                Buffer.prototype.readUInt16BE = function () { return 99; };
                const v = Buffer.from([1,2]).readUInt16BE(0);
                Buffer.prototype.readUInt16BE = original;
                return v;
              })()
            })
            "#,
        );
        assert_eq!(
            actual,
            r#"{"shape":[["readUInt16LE",1,true,true,true,true,true,false,false],["readUInt16BE",1,true,true,true,true,true,false,false],["readInt16LE",1,true,true,true,true,true,false,false],["readInt16BE",1,true,true,true,true,true,false,false],["writeUInt16LE",1,true,true,true,true,true,false,false],["writeUInt16BE",1,true,true,true,true,true,false,false],["writeInt16LE",1,true,true,true,true,true,false,false],["writeInt16BE",1,true,true,true,true,true,false,false]],"construct":["TypeError","TypeError"],"reads":[13330,4660,-32513,-128,258,"TypeError","TypeError","RangeError","RangeError","RangeError","RangeError"],"writes":[[2,18,52,0],[2,52,18,0],[2,255,255],[2,0,128],[2,0,7],[2,0,1],[2,0,1],[2,0,0],"RangeError","RangeError","RangeError","RangeError","RangeError"],"override":99}"#
        );
    }

    #[test]
    fn buffer_u32_numeric_manifest_feeds_live_ihi_and_jit_facts() {
        for row in [
            BUFFER_READ_UINT32_LE,
            BUFFER_READ_UINT32_BE,
            BUFFER_READ_INT32_LE,
            BUFFER_READ_INT32_BE,
            BUFFER_WRITE_UINT32_LE,
            BUFFER_WRITE_UINT32_BE,
            BUFFER_WRITE_INT32_LE,
            BUFFER_WRITE_INT32_BE,
        ] {
            assert_eq!(row.receiver, NativeApiReceiver::Buffer);
            assert_eq!(row.kind, NativeApiKind::Method);
            assert_eq!(row.returns, &[NativeApiDomain::Number]);
            assert_eq!(row.registration.length, 1);
            assert!(row.registration.enumerable);
            assert!(!row.registration.constructor);
            assert_eq!(row.validation.receiver, NativeApiReceiver::Buffer);
            assert_eq!(row.validation.byte_width, 4);
            assert!(matches!(
                row.validation.endian,
                NativeApiEndian::Little | NativeApiEndian::Big
            ));

            let live = crate::interp_ic_table::lookup(
                row.ihi.expect("manifest row has IHI facts").key,
                row.ihi.expect("manifest row has IHI facts").receiver,
                row.arity,
            )
            .expect("live generated IHI entry for Buffer 32-bit numeric mouth");
            assert_eq!(live.key, row.property);
            assert_eq!(
                live.receiver,
                row.ihi.expect("manifest row has IHI facts").receiver
            );
            assert_eq!(
                live.arity,
                row.ihi.expect("manifest row has IHI facts").arity
            );
            assert_eq!(
                live.cached_id_field,
                row.ihi.expect("manifest row has IHI facts").cached_id_field
            );

            if let Some(jit) = row.jit_ic {
                let idx = rusty_js_jit::ic_table::lookup_by_key(jit.key)
                    .expect("hand JIT IC entry for generated Buffer mouth");
                let hand = &rusty_js_jit::ic_table::IC_TABLE[idx as usize];
                assert_eq!(hand.key, jit.key);
                assert_eq!(hand.receiver, rusty_js_jit::ic_table::ReceiverKind::Buffer);
                assert_eq!(hand.extern_name, jit.extern_name);
            }
            assert_eq!(row.lejit_expectation, None);
        }
    }

    #[test]
    fn buffer_u32_numeric_runtime_shape_override_and_semantics_match_node() {
        let actual = run_manifest_json(
            r#"
            JSON.stringify({
              shape: ['readUInt32LE','readUInt32BE','readInt32LE','readInt32BE','writeUInt32LE','writeUInt32BE','writeInt32LE','writeInt32BE'].map((m) => {
                const d = Object.getOwnPropertyDescriptor(Buffer.prototype, m);
                const pd = Object.getOwnPropertyDescriptor(Buffer.prototype[m], 'prototype');
                return [m, Buffer.prototype[m].length, d.writable, d.enumerable, d.configurable, !!pd, pd && pd.writable, pd && pd.enumerable, pd && pd.configurable];
              }),
              construct: [
                (() => { try { new Buffer.prototype.readUInt32LE(); } catch (e) { return e.name; } })(),
                (() => { try { new Buffer.prototype.writeUInt32BE(); } catch (e) { return e.name; } })()
              ],
              reads: [
                Buffer.from([0x12,0x34,0x56,0x78,0xff,0x80,0,1]).readUInt32LE(0),
                Buffer.from([0x12,0x34,0x56,0x78,0xff,0x80,0,1]).readUInt32BE(0),
                Buffer.from([0x12,0x34,0x56,0x78,0xff,0x80,0,1]).readInt32LE(4),
                Buffer.from([0x12,0x34,0x56,0x78,0xff,0x80,0,1]).readInt32BE(4),
                Buffer.prototype.readUInt32BE.call(new Uint8Array([1,2,3,4]), 0),
                (() => { try { Buffer.from([0,0,0,0]).readUInt32BE('1'); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.from([0,0,0,0]).readUInt32BE(null); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.from([0,0,0,0]).readUInt32BE(0.5); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.from([0,0,0,0]).readUInt32BE(Infinity); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.from([0,0,0,0]).readUInt32BE(-1); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.from([0,0,0,0]).readUInt32BE(1); } catch (e) { return e.name; } })()
              ],
              writes: [
                (() => { const b = Buffer.alloc(5); const r = b.writeUInt32BE(0x12345678,1); return [r,b[0],b[1],b[2],b[3],b[4]]; })(),
                (() => { const b = Buffer.alloc(4); const r = b.writeUInt32LE(0x12345678,0); return [r,b[0],b[1],b[2],b[3]]; })(),
                (() => { const b = Buffer.alloc(4); const r = b.writeInt32BE(-1,0); return [r,b[0],b[1],b[2],b[3]]; })(),
                (() => { const b = Buffer.alloc(4); const r = b.writeInt32LE(-2147483648,0); return [r,b[0],b[1],b[2],b[3]]; })(),
                (() => { const b = Buffer.alloc(4); const r = b.writeUInt32BE('7',0); return [r,b[0],b[1],b[2],b[3]]; })(),
                (() => { const b = Buffer.alloc(4); const r = b.writeUInt32BE(true,0); return [r,b[0],b[1],b[2],b[3]]; })(),
                (() => { const b = Buffer.alloc(4); const r = b.writeUInt32BE(1.9,0); return [r,b[0],b[1],b[2],b[3]]; })(),
                (() => { const b = Buffer.from([9,9,9,9]); const r = b.writeUInt32BE(undefined,0); return [r,b[0],b[1],b[2],b[3]]; })(),
                (() => { try { Buffer.alloc(4).writeUInt32BE(4294967296,0); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.alloc(4).writeUInt32BE(-1,0); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.alloc(4).writeInt32BE(2147483648,0); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.alloc(4).writeInt32BE(-2147483649,0); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.alloc(3).writeUInt32BE(1,0); } catch (e) { return e.name; } })()
              ],
              override: (() => {
                const r0 = Buffer.prototype.readUInt32BE;
                Buffer.prototype.readUInt32BE = function () { return 99; };
                const read = Buffer.from([1,2,3,4]).readUInt32BE(0);
                Buffer.prototype.readUInt32BE = r0;
                const w0 = Buffer.prototype.writeUInt32LE;
                Buffer.prototype.writeUInt32LE = function () { return 77; };
                const write = Buffer.alloc(4).writeUInt32LE(1,0);
                Buffer.prototype.writeUInt32LE = w0;
                return [read, write];
              })()
            })
            "#,
        );
        assert_eq!(
            actual,
            r#"{"shape":[["readUInt32LE",1,true,true,true,true,true,false,false],["readUInt32BE",1,true,true,true,true,true,false,false],["readInt32LE",1,true,true,true,true,true,false,false],["readInt32BE",1,true,true,true,true,true,false,false],["writeUInt32LE",1,true,true,true,true,true,false,false],["writeUInt32BE",1,true,true,true,true,true,false,false],["writeInt32LE",1,true,true,true,true,true,false,false],["writeInt32BE",1,true,true,true,true,true,false,false]],"construct":["TypeError","TypeError"],"reads":[2018915346,305419896,16810239,-8388607,16909060,"TypeError","TypeError","RangeError","RangeError","RangeError","RangeError"],"writes":[[5,0,18,52,86,120],[4,120,86,52,18],[4,255,255,255,255],[4,0,0,0,128],[4,0,0,0,7],[4,0,0,0,1],[4,0,0,0,1],[4,0,0,0,0],"RangeError","RangeError","RangeError","RangeError","RangeError"],"override":[99,77]}"#
        );
    }

    #[test]
    fn buffer_float_double_numeric_runtime_shape_and_ieee_match_node() {
        for row in [
            BUFFER_READ_FLOAT_LE,
            BUFFER_READ_FLOAT_BE,
            BUFFER_READ_DOUBLE_LE,
            BUFFER_READ_DOUBLE_BE,
            BUFFER_WRITE_FLOAT_LE,
            BUFFER_WRITE_FLOAT_BE,
            BUFFER_WRITE_DOUBLE_LE,
            BUFFER_WRITE_DOUBLE_BE,
        ] {
            let live = crate::interp_ic_table::lookup(
                row.ihi.expect("manifest row has IHI facts").key,
                row.ihi.expect("manifest row has IHI facts").receiver,
                row.arity,
            )
            .expect("live generated IHI entry for Buffer float/double mouth");
            assert_eq!(
                live.cached_id_field,
                row.ihi.expect("manifest row has IHI facts").cached_id_field
            );
            assert_eq!(row.jit_ic, None);
            assert_eq!(row.lejit_expectation, None);
        }
        let actual = run_manifest_json(
            r#"
            JSON.stringify({
              shape: ['readFloatLE','readFloatBE','readDoubleLE','readDoubleBE','writeFloatLE','writeFloatBE','writeDoubleLE','writeDoubleBE'].map((m) => {
                const d = Object.getOwnPropertyDescriptor(Buffer.prototype, m);
                const pd = Object.getOwnPropertyDescriptor(Buffer.prototype[m], 'prototype');
                return [m, Buffer.prototype[m].length, d.writable, d.enumerable, d.configurable, !!pd, pd && pd.writable, pd && pd.enumerable, pd && pd.configurable];
              }),
              construct: [(() => { try { new Buffer.prototype.readFloatLE(); } catch (e) { return e.name; } })(), (() => { try { new Buffer.prototype.writeDoubleBE(); } catch (e) { return e.name; } })()],
              reads: [Buffer.from([0,0,192,63]).readFloatLE(0), Buffer.from([192,16,0,0]).readFloatBE(0), Object.is(Buffer.from([0,0,0,128]).readFloatLE(0), -0), String(Buffer.from([0,0,192,127]).readFloatLE(0)), String(Buffer.from([0,0,128,127]).readFloatLE(0)), String(Buffer.from([0,0,128,255]).readFloatLE(0)), Object.is(Buffer.from([0,0,0,0,0,0,0,128]).readDoubleLE(0), -0), Buffer.from([63,240,0,0,0,0,0,0]).readDoubleBE(0), Buffer.prototype.readFloatBE.call(new Uint8Array([63,128,0,0]), 0), (() => { try { Buffer.from([0,0,0,0]).readFloatBE('1'); } catch (e) { return e.name; } })(), (() => { try { Buffer.from([0,0,0,0]).readFloatBE(0.5); } catch (e) { return e.name; } })(), (() => { try { Buffer.from([0,0,0,0]).readFloatBE(1); } catch (e) { return e.name; } })()],
              writes: [(() => { const b = Buffer.alloc(5); const r = b.writeFloatLE(1.5,1); return [r,...b]; })(), (() => { const b = Buffer.alloc(4); const r = b.writeFloatBE(-2.25,0); return [r,...b]; })(), (() => { const b = Buffer.alloc(8); const r = b.writeDoubleLE(-0,0); return [r,...b,Object.is(b.readDoubleLE(0), -0)]; })(), (() => { const b = Buffer.alloc(8); const r = b.writeDoubleBE(Infinity,0); return [r,...b]; })(), (() => { const b = Buffer.alloc(4); const r = b.writeFloatBE('7.5',0); return [r,...b]; })(), (() => { const b = Buffer.alloc(4); const r = b.writeFloatBE(NaN,0); return [r,...b,String(b.readFloatBE(0))]; })(), (() => { try { Buffer.alloc(4).writeFloatBE(1,1); } catch (e) { return e.name; } })(), (() => { try { Buffer.alloc(7).writeDoubleBE(1,0); } catch (e) { return e.name; } })()],
              override: (() => { const r0 = Buffer.prototype.readFloatBE; Buffer.prototype.readFloatBE = function () { return 99; }; const read = Buffer.from([63,128,0,0]).readFloatBE(0); Buffer.prototype.readFloatBE = r0; const w0 = Buffer.prototype.writeDoubleLE; Buffer.prototype.writeDoubleLE = function () { return 77; }; const write = Buffer.alloc(8).writeDoubleLE(1,0); Buffer.prototype.writeDoubleLE = w0; return [read, write]; })()
            })
            "#,
        );
        assert_eq!(
            actual,
            r#"{"shape":[["readFloatLE",0,true,true,true,true,true,false,false],["readFloatBE",0,true,true,true,true,true,false,false],["readDoubleLE",0,true,true,true,true,true,false,false],["readDoubleBE",0,true,true,true,true,true,false,false],["writeFloatLE",1,true,true,true,true,true,false,false],["writeFloatBE",1,true,true,true,true,true,false,false],["writeDoubleLE",1,true,true,true,true,true,false,false],["writeDoubleBE",1,true,true,true,true,true,false,false]],"construct":["TypeError","TypeError"],"reads":[1.5,-2.25,true,"NaN","Infinity","-Infinity",true,1,1,"TypeError","RangeError","RangeError"],"writes":[[5,0,0,0,192,63],[4,192,16,0,0],[8,0,0,0,0,0,0,0,128,true],[8,127,240,0,0,0,0,0,0],[4,64,240,0,0],[4,127,192,0,0,"NaN"],"RangeError","RangeError"],"override":[99,77]}"#
        );
    }

    #[test]
    fn buffer_bigint_numeric_runtime_shape_and_ihi_match_node() {
        for row in [
            BUFFER_READ_BIG_UINT64_LE,
            BUFFER_READ_BIG_UINT64_BE,
            BUFFER_READ_BIG_INT64_LE,
            BUFFER_READ_BIG_INT64_BE,
            BUFFER_WRITE_BIG_UINT64_LE,
            BUFFER_WRITE_BIG_UINT64_BE,
            BUFFER_WRITE_BIG_INT64_LE,
            BUFFER_WRITE_BIG_INT64_BE,
        ] {
            let live = crate::interp_ic_table::lookup(
                row.ihi.expect("manifest row has IHI facts").key,
                row.ihi.expect("manifest row has IHI facts").receiver,
                row.arity,
            )
            .expect("live generated IHI entry for Buffer BigInt mouth");
            assert_eq!(
                live.cached_id_field,
                row.ihi.expect("manifest row has IHI facts").cached_id_field
            );
            assert_eq!(row.jit_ic, None);
            assert_eq!(row.lejit_expectation, None);
        }
        let actual = run_manifest_json(
            r#"
            JSON.stringify({
              shape: ['readBigUInt64LE','readBigUInt64BE','readBigInt64LE','readBigInt64BE','writeBigUInt64LE','writeBigUInt64BE','writeBigInt64LE','writeBigInt64BE'].map((m) => {
                const d = Object.getOwnPropertyDescriptor(Buffer.prototype, m);
                const pd = Object.getOwnPropertyDescriptor(Buffer.prototype[m], 'prototype');
                return [m, Buffer.prototype[m].length, d.writable, d.enumerable, d.configurable, !!pd, pd && pd.writable, pd && pd.enumerable, pd && pd.configurable];
              }),
              construct: [(() => { try { new Buffer.prototype.readBigUInt64LE(); } catch (e) { return e.name; } })(), (() => { try { new Buffer.prototype.writeBigInt64BE(); } catch (e) { return e.name; } })()],
              reads: (() => { const b = Buffer.from([1,2,3,4,5,6,7,8,255,255,255,255,255,255,255,255]); return ['readBigUInt64LE','readBigUInt64BE','readBigInt64LE','readBigInt64BE'].map((m) => [m, String(b[m](0)), String(b[m](8)), String(Buffer.prototype[m].call(new Uint8Array([1,2,3,4,5,6,7,8]), 0))]); })(),
              writes: [
                (() => { const b = Buffer.alloc(8); const r = b.writeBigUInt64LE((1n << 64n) - 1n, 0); return [r,...b]; })(),
                (() => { const b = Buffer.alloc(8); const r = b.writeBigUInt64BE(1n << 63n, 0); return [r,...b]; })(),
                (() => { const b = Buffer.alloc(8); const r = b.writeBigInt64LE(-1n, 0); return [r,...b]; })(),
                (() => { const b = Buffer.alloc(8); const r = b.writeBigInt64BE((1n << 63n) - 1n, 0); return [r,...b]; })(),
                (() => { try { Buffer.alloc(8).writeBigUInt64BE(-1n,0); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.alloc(8).writeBigInt64BE(1n << 63n,0); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.alloc(8).writeBigUInt64BE(1,0); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.alloc(8).writeBigUInt64BE(1n,'1'); } catch (e) { return e.name; } })()
              ],
              override: (() => { const r0 = Buffer.prototype.readBigInt64BE; Buffer.prototype.readBigInt64BE = function () { return 123n; }; const read = String(Buffer.from([255,255,255,255,255,255,255,255]).readBigInt64BE(0)); Buffer.prototype.readBigInt64BE = r0; const w0 = Buffer.prototype.writeBigUInt64LE; Buffer.prototype.writeBigUInt64LE = function () { return 77; }; const write = Buffer.alloc(8).writeBigUInt64LE(1n,0); Buffer.prototype.writeBigUInt64LE = w0; return [read, write]; })()
            })
            "#,
        );
        assert_eq!(
            actual,
            r#"{"shape":[["readBigUInt64LE",0,true,true,true,true,true,false,false],["readBigUInt64BE",0,true,true,true,true,true,false,false],["readBigInt64LE",0,true,true,true,true,true,false,false],["readBigInt64BE",0,true,true,true,true,true,false,false],["writeBigUInt64LE",1,true,true,true,true,true,false,false],["writeBigUInt64BE",1,true,true,true,true,true,false,false],["writeBigInt64LE",1,true,true,true,true,true,false,false],["writeBigInt64BE",1,true,true,true,true,true,false,false]],"construct":["TypeError","TypeError"],"reads":[["readBigUInt64LE","578437695752307201","18446744073709551615","578437695752307201"],["readBigUInt64BE","72623859790382856","18446744073709551615","72623859790382856"],["readBigInt64LE","578437695752307201","-1","578437695752307201"],["readBigInt64BE","72623859790382856","-1","72623859790382856"]],"writes":[[8,255,255,255,255,255,255,255,255],[8,128,0,0,0,0,0,0,0],[8,255,255,255,255,255,255,255,255],[8,127,255,255,255,255,255,255,255],"RangeError","RangeError","TypeError","TypeError"],"override":["123",77]}"#
        );
    }

    #[test]
    fn buffer_swap_json_and_legacy_slice_helpers_match_node_shape_and_behavior() {
        for row in [
            BUFFER_SWAP16,
            BUFFER_SWAP32,
            BUFFER_SWAP64,
            BUFFER_TO_JSON,
            BUFFER_UTF8_SLICE,
            BUFFER_LATIN1_SLICE,
            BUFFER_ASCII_SLICE,
            BUFFER_UCS2_SLICE,
            BUFFER_BASE64_SLICE,
        ] {
            assert_eq!(row.receiver, NativeApiReceiver::Buffer);
            assert_eq!(row.kind, NativeApiKind::Method);
            assert!(!row.constructor);
            assert!(row.registration.enumerable);
            assert!(!row.registration.constructor);
            assert!(row.registration.function_prototype);
            assert_eq!(row.ihi, None);
            assert_eq!(row.jit_ic, None);
            assert_eq!(row.lejit_expectation, None);
        }
        let actual = run_manifest_json(
            r#"
            JSON.stringify({
              shape: ['swap16','swap32','swap64','toJSON','utf8Slice','latin1Slice','asciiSlice','ucs2Slice','base64Slice'].map((m) => {
                const fn = Buffer.prototype[m];
                const d = Object.getOwnPropertyDescriptor(Buffer.prototype, m);
                const pd = Object.getOwnPropertyDescriptor(fn, 'prototype');
                let construct;
                try { new fn(); } catch (e) { construct = e.name; }
                return [m, fn.length, d.writable, d.enumerable, d.configurable, !!pd, pd && pd.writable, pd && pd.enumerable, pd && pd.configurable, construct];
              }),
              swap: [
                (() => { const b = Buffer.from([1,2,3,4]); const r = b.swap16(); return [r === b, ...b]; })(),
                (() => { const b = Buffer.from([1,2,3,4]); const r = b.swap32(); return [r === b, ...b]; })(),
                (() => { const b = Buffer.from([1,2,3,4,5,6,7,8]); const r = b.swap64(); return [r === b, ...b]; })(),
                (() => { try { Buffer.from([1]).swap16(); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.from([1,2]).swap32(); } catch (e) { return e.name; } })(),
                Buffer.prototype.swap16.call(new Uint8Array([1,2]))
              ],
              json: [
                Buffer.from([1,2,255]).toJSON(),
                JSON.stringify(Buffer.from([1,2])),
                Buffer.prototype.toJSON.call(new Uint8Array([7,8])),
                (() => { try { Buffer.prototype.toJSON.call({0:1,length:1}); } catch (e) { return e.name; } })()
              ],
              slices: [
                Buffer.from([0x68,0xc3,0xa9,0xff]).utf8Slice(0,3),
                Buffer.from([0x68,0xe9,0xff]).latin1Slice(0,3),
                Buffer.from([0x41,0xff]).asciiSlice(0,2),
                Buffer.from([0x61,0x00,0x62,0x00]).ucs2Slice(0,4),
                Buffer.from([0x66,0x6f,0x6f]).base64Slice(0,3),
                Buffer.prototype.utf8Slice.call(new Uint8Array([65]),0,1)
              ],
              override: (() => { const o = Buffer.prototype.swap16; Buffer.prototype.swap16 = function () { return 99; }; const v = Buffer.from([1,2]).swap16(); Buffer.prototype.swap16 = o; return v; })()
            })
            "#,
        );
        assert_eq!(
            actual,
            r#"{"shape":[["swap16",0,true,true,true,true,true,false,false,"TypeError"],["swap32",0,true,true,true,true,true,false,false,"TypeError"],["swap64",0,true,true,true,true,true,false,false,"TypeError"],["toJSON",0,true,true,true,true,true,false,false,"TypeError"],["utf8Slice",2,true,true,true,true,true,false,false,"TypeError"],["latin1Slice",2,true,true,true,true,true,false,false,"TypeError"],["asciiSlice",2,true,true,true,true,true,false,false,"TypeError"],["ucs2Slice",2,true,true,true,true,true,false,false,"TypeError"],["base64Slice",2,true,true,true,true,true,false,false,"TypeError"]],"swap":[[true,2,1,4,3],[true,4,3,2,1],[true,8,7,6,5,4,3,2,1],"RangeError","RangeError",{"0":2,"1":1}],"json":[{"type":"Buffer","data":[1,2,255]},"{\"type\":\"Buffer\",\"data\":[1,2]}",{"type":"Buffer","data":[7,8]},"TypeError"],"slices":["hé","héÿ","A","ab","Zm9v","A"],"override":99}"#
        );
    }

    #[test]
    fn buffer_encoding_string_mouths_match_node_runtime_shape() {
        let actual = run_manifest_json(
            r#"
            JSON.stringify({
              shape: ['toString','write'].map((m) => {
                const d = Object.getOwnPropertyDescriptor(Buffer.prototype, m);
                const pd = Object.getOwnPropertyDescriptor(Buffer.prototype[m], 'prototype');
                return [m, Buffer.prototype[m].length, d.writable, d.enumerable, d.configurable, !!pd, pd && pd.writable, pd && pd.enumerable, pd && pd.configurable];
              }).concat(['from','byteLength','isEncoding'].map((m) => {
                const d = Object.getOwnPropertyDescriptor(Buffer, m);
                const pd = Object.getOwnPropertyDescriptor(Buffer[m], 'prototype');
                return [m, Buffer[m].length, d.writable, d.enumerable, d.configurable, !!pd, pd && pd.writable, pd && pd.enumerable, pd && pd.configurable];
              })),
              labels: ['utf8','utf-8','hex','base64','base64url','latin1','binary','ascii','ucs2','ucs-2','utf16le','utf-16le','bad','UTF8'].map((e) => [e, Buffer.isEncoding(e)]),
              byteLength: [Buffer.byteLength('hé','utf8'), Buffer.byteLength('hé','latin1'), Buffer.byteLength('./x.js','hex'), Buffer.byteLength('aGVs=bG8=','base64'), (() => { try { Buffer.byteLength(1); } catch (e) { return [e.name, e.code]; } })()],
              from: [
                [...Buffer.from('hé','utf8')],
                [...Buffer.from('hé','latin1')],
                [...Buffer.from('6869','hex')],
                [...Buffer.from('686','hex')],
                [...Buffer.from('zz','hex')],
                [...Buffer.from('aGVsbG8=','base64')],
                [...Buffer.from('aGVsbG8','base64url')],
                (() => { try { Buffer.from('x','bad'); } catch (e) { return [e.name, e.code]; } })(),
                (() => { try { Buffer.from(); } catch (e) { return [e.name, e.code]; } })()
              ],
              toString: [
                Buffer.from([0x68,0xc3,0xa9]).toString('utf8'),
                Buffer.from([0xe9]).toString('latin1'),
                Buffer.from([0x68,0x69]).toString('hex'),
                Buffer.from([0x68,0x69]).toString('base64'),
                Buffer.from([0x68,0,0x69,0]).toString('utf16le'),
                Buffer.from([0x68,0x69]).toString('utf8',1,9),
                Buffer.from([0x68,0x69]).toString('utf8',-1,1),
                Buffer.prototype.toString.call(new Uint8Array([65]),'utf8'),
                (() => { try { Buffer.from([1]).toString('bad'); } catch (e) { return [e.name, e.code]; } })()
              ],
              write: [
                (() => { const b=Buffer.alloc(6); const r=b.write('hé',0,6,'utf8'); return [r,...b]; })(),
                (() => { const b=Buffer.alloc(4); const r=b.write('hé',0,4,'latin1'); return [r,...b]; })(),
                (() => { const b=Buffer.alloc(4); const r=b.write('6869',0,4,'hex'); return [r,...b]; })(),
                (() => { const b=Buffer.alloc(2); const r=b.write('hé',1,1,'utf8'); return [r,...b]; })(),
                (() => { try { Buffer.alloc(1).write('x',2); } catch (e) { return e.name; } })(),
                (() => { try { Buffer.alloc(1).write('x',0,1,'bad'); } catch (e) { return [e.name, e.code]; } })(),
                (() => { const u = new Uint8Array([0]); const r = Buffer.prototype.write.call(u,'A',0,1,'utf8'); return [r, u[0]]; })()
              ],
              construct: [
                (() => { try { new Buffer.prototype.toString(); } catch (e) { return e.name; } })(),
                (() => { try { new Buffer.prototype.write(); } catch (e) { return e.name; } })(),
                (() => [...new Buffer.from('x')])(),
                (() => { try { new Buffer.from(); } catch (e) { return [e.name, e.code]; } })()
              ]
            })
            "#,
        );
        assert_eq!(
            actual,
            r#"{"shape":[["toString",3,true,true,true,true,true,false,false],["write",4,true,true,true,true,true,false,false],["from",3,true,true,true,true,true,false,false],["byteLength",1,true,true,true,true,true,false,false],["isEncoding",1,true,true,true,true,true,false,false]],"labels":[["utf8",true],["utf-8",true],["hex",true],["base64",true],["base64url",true],["latin1",true],["binary",true],["ascii",true],["ucs2",true],["ucs-2",true],["utf16le",true],["utf-16le",true],["bad",false],["UTF8",true]],"byteLength":[3,2,3,6,["TypeError","ERR_INVALID_ARG_TYPE"]],"from":[[104,195,169],[104,233],[104,105],[104],[],[104,101,108,108,111],[104,101,108,108,111],["TypeError","ERR_UNKNOWN_ENCODING"],["TypeError","ERR_INVALID_ARG_TYPE"]],"toString":["hé","é","6869","aGk=","hi","i","h","A",["TypeError","ERR_UNKNOWN_ENCODING"]],"write":[[3,104,195,169,0,0,0],[2,104,233,0,0],[2,104,105,0,0],[1,0,104],"RangeError",["TypeError","ERR_UNKNOWN_ENCODING"],[1,65]],"construct":["TypeError","TypeError",[120],["TypeError","ERR_INVALID_ARG_TYPE"]]}"#
        );
    }

    #[test]
    fn buffer_encoding_string_rows_have_generated_schema_facts_without_fast_claims() {
        let rows = [
            BUFFER_TO_STRING,
            BUFFER_WRITE,
            BUFFER_BYTE_LENGTH,
            BUFFER_IS_ENCODING,
            BUFFER_FROM,
        ];
        assert!(MANIFEST_ROWS
            .iter()
            .any(|row| row.api == "Buffer.prototype.toString"));
        assert!(MANIFEST_ROWS.iter().any(|row| row.api == "Buffer.from"));
        for row in rows {
            assert!(matches!(row.receiver, NativeApiReceiver::Buffer));
            assert!(row.constructor);
            assert!(row.registration.writable);
            assert!(row.registration.enumerable);
            assert!(row.registration.configurable);
            assert!(row.registration.function_prototype);
            assert!(row.ihi.is_none(), "{} must not claim IHI", row.api);
            assert!(row.jit_ic.is_none(), "{} must not claim JIT", row.api);
            assert!(
                row.lejit_expectation.is_none(),
                "{} must not claim LeJIT",
                row.api
            );
            assert!(
                row.validation.error_codes.is_empty()
                    || row
                        .validation
                        .error_codes
                        .iter()
                        .all(|code| code.starts_with("ERR_")),
                "{} error-code facts must be Node error codes",
                row.api
            );
        }
        assert_eq!(BUFFER_TO_STRING.registration.length, 3);
        assert_eq!(BUFFER_WRITE.registration.length, 4);
        assert_eq!(BUFFER_BYTE_LENGTH.registration.length, 1);
        assert_eq!(BUFFER_BYTE_LENGTH.arity, 2);
        assert!(matches!(
            BUFFER_BYTE_LENGTH.kind,
            NativeApiKind::StaticMethod
        ));
        assert!(matches!(BUFFER_FROM.kind, NativeApiKind::StaticMethod));
        assert_eq!(BUFFER_FROM.returns, &[NativeApiDomain::Buffer]);
        assert_eq!(
            BUFFER_TO_STRING.validation.encoding_policy,
            "strict-label-or-ERR_UNKNOWN_ENCODING"
        );
        assert_eq!(
            BUFFER_WRITE.validation.effect,
            NativeApiEffect::MutatesReceiverBytes
        );
        assert_eq!(
            crate::native_api_manifest_generated::buffer_static_from_generated_ihi_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::
                buffer_static_from_generated_cruftscript_stdlib_signature_spec(),
            None
        );
    }

    #[test]
    fn buffer_search_compare_mouths_match_node_runtime_shape() {
        let actual = run_manifest_json(
            r#"
            JSON.stringify({
              shape: ['includes','indexOf','lastIndexOf','equals','compare'].map((m) => {
                const d = Object.getOwnPropertyDescriptor(Buffer.prototype, m);
                const pd = Object.getOwnPropertyDescriptor(Buffer.prototype[m], 'prototype');
                return [m, Buffer.prototype[m].length, d.writable, d.enumerable, d.configurable, !!pd, pd && pd.writable, pd && pd.enumerable, pd && pd.configurable];
              }).concat(['compare'].map((m) => {
                const d = Object.getOwnPropertyDescriptor(Buffer, m);
                const pd = Object.getOwnPropertyDescriptor(Buffer[m], 'prototype');
                return ['static.' + m, Buffer[m].length, d.writable, d.enumerable, d.configurable, !!pd, pd && pd.writable, pd && pd.enumerable, pd && pd.configurable];
              })),
              results: (() => {
                const b = Buffer.from('abcabc');
                const val = (fn) => { try { return fn(); } catch (e) { return [e.name, e.code || null]; } };
                return [
                  b.includes('6263', 0, 'hex'),
                  b.indexOf('6263', 0, 'hex'),
                  b.indexOf('ab', -3),
                  b.lastIndexOf('ab'),
                  b.lastIndexOf('ab', -3),
                  b.equals(new Uint8Array([97,98,99,97,98,99])),
                  val(() => b.equals('abcabc')),
                  b.compare(Buffer.from('zabc'), 1, 4, 0, 3),
                  val(() => b.compare('abcabc')),
                  Buffer.compare(new Uint8Array([1]), new Uint8Array([2])),
                  val(() => Buffer.compare('a', 'b')),
                  val(() => b.indexOf('x', 0, 'bogus')),
                  b.includes(98),
                  b.indexOf(''),
                  b.lastIndexOf('')
                ];
              })(),
              construct: [
                (() => { try { new Buffer.prototype.includes(); } catch (e) { return e.name; } })(),
                (() => { try { new Buffer.prototype.compare(); } catch (e) { return e.name; } })(),
                (() => { try { new Buffer.compare(); } catch (e) { return e.name; } })()
              ]
            })
            "#,
        );
        assert_eq!(
            actual,
            r#"{"shape":[["includes",4,true,true,true,true,true,false,false],["indexOf",4,true,true,true,true,true,false,false],["lastIndexOf",4,true,true,true,true,true,false,false],["equals",1,true,true,true,true,true,false,false],["compare",5,true,true,true,true,true,false,false],["static.compare",2,true,true,true,true,true,false,false]],"results":[true,1,3,3,3,true,["TypeError",null],0,["TypeError",null],-1,["TypeError",null],["TypeError","ERR_UNKNOWN_ENCODING"],true,0,6],"construct":["TypeError","TypeError","TypeError"]}"#
        );
    }

    #[test]
    fn buffer_search_compare_rows_have_generated_schema_facts_without_fast_claims() {
        let rows = [
            BUFFER_INCLUDES,
            BUFFER_INDEX_OF,
            BUFFER_LAST_INDEX_OF,
            BUFFER_EQUALS,
            BUFFER_COMPARE,
            BUFFER_STATIC_COMPARE,
        ];
        assert!(MANIFEST_ROWS
            .iter()
            .any(|row| row.api == "Buffer.prototype.includes"));
        assert!(MANIFEST_ROWS.iter().any(|row| row.api == "Buffer.compare"));
        for row in rows {
            assert!(matches!(row.receiver, NativeApiReceiver::Buffer));
            assert!(row.constructor);
            assert!(row.registration.writable);
            assert!(row.registration.enumerable);
            assert!(row.registration.configurable);
            assert!(row.registration.function_prototype);
            assert!(row.ihi.is_none(), "{} must not claim IHI", row.api);
            assert!(row.jit_ic.is_none(), "{} must not claim JIT", row.api);
            assert!(
                row.lejit_expectation.is_none(),
                "{} must not claim LeJIT",
                row.api
            );
        }
        assert_eq!(BUFFER_INCLUDES.registration.length, 4);
        assert_eq!(BUFFER_INDEX_OF.registration.length, 4);
        assert_eq!(BUFFER_LAST_INDEX_OF.registration.length, 4);
        assert_eq!(BUFFER_COMPARE.registration.length, 5);
        assert_eq!(BUFFER_STATIC_COMPARE.registration.length, 2);
        assert!(matches!(
            BUFFER_STATIC_COMPARE.kind,
            NativeApiKind::StaticMethod
        ));
        assert_eq!(
            BUFFER_INCLUDES.validation.encoding_policy,
            "string needles use strict label or ERR_UNKNOWN_ENCODING"
        );
        assert_eq!(
            crate::native_api_manifest_generated::buffer_static_compare_generated_ihi_spec(),
            None
        );
    }

    #[test]
    fn buffer_copy_fill_view_mouths_match_node_runtime_shape() {
        let actual = run_manifest_json(
            r#"
            JSON.stringify({
              shape: ['copy','fill','slice','subarray'].map((m) => {
                const d = Object.getOwnPropertyDescriptor(Buffer.prototype, m);
                const pd = Object.getOwnPropertyDescriptor(Buffer.prototype[m], 'prototype');
                return [m, Buffer.prototype[m].length, d.writable, d.enumerable, d.configurable, !!pd, pd && pd.writable, pd && pd.enumerable, pd && pd.configurable];
              }),
              results: (() => {
                const val = (fn) => { try { return fn(); } catch (e) { return [e.name, e.code || null]; } };
                return [
                  (() => { const s=Buffer.from('abcdef'); const t=Buffer.alloc(4); const r=s.copy(t,1,2,5); return [r,...t]; })(),
                  (() => { const s=Buffer.from([1,2,3]); const t=new Uint8Array(4); const r=Buffer.prototype.copy.call(s,t,1); return [r,...t]; })(),
                  val(() => Buffer.from('a').copy({})),
                  (() => { const b=Buffer.alloc(4); const r=b.fill(257); return [r===b,...b]; })(),
                  (() => { const b=Buffer.alloc(5); const r=b.fill('ab',1,5,'utf8'); return [r===b,...b]; })(),
                  (() => { const b=Buffer.alloc(4); b.fill('ff',0,4,'hex'); return [...b]; })(),
                  val(() => Buffer.alloc(2).fill('x',0,1,'bogus')),
                  (() => { const b=Buffer.from([1,2,3,4]); const s=b.slice(1,3); s[0]=9; return [Buffer.isBuffer(s), [...s], [...b]]; })(),
                  (() => { const b=Buffer.from([1,2,3,4]); const s=b.subarray(-3,-1); s[1]=8; return [Buffer.isBuffer(s), [...s], [...b]]; })(),
                  Buffer.prototype.slice.call(new Uint8Array([1,2,3]),1)[0]
                ];
              })(),
              construct: [
                (() => { try { new Buffer.prototype.copy(); } catch (e) { return e.name; } })(),
                (() => { try { new Buffer.prototype.fill(); } catch (e) { return e.name; } })(),
                (() => { try { new Buffer.prototype.slice(); } catch (e) { return e.name; } })(),
                (() => { try { new Buffer.prototype.subarray(); } catch (e) { return e.name; } })()
              ]
            })
            "#,
        );
        assert_eq!(
            actual,
            r#"{"shape":[["copy",4,true,true,true,true,true,false,false],["fill",4,true,true,true,true,true,false,false],["slice",2,true,true,true,true,true,false,false],["subarray",2,true,true,true,true,true,false,false]],"results":[[3,0,99,100,101],[3,0,1,2,3],["TypeError",null],[true,1,1,1,1],[true,0,97,98,97,98],[255,255,255,255],["TypeError","ERR_UNKNOWN_ENCODING"],[true,[9,3],[1,9,3,4]],[true,[2,8],[1,2,8,4]],2],"construct":["TypeError","TypeError","TypeError","TypeError"]}"#
        );
    }

    #[test]
    fn buffer_copy_fill_view_rows_have_generated_schema_facts_without_fast_claims() {
        let rows = [BUFFER_COPY, BUFFER_FILL, BUFFER_SLICE, BUFFER_SUBARRAY];
        assert!(MANIFEST_ROWS
            .iter()
            .any(|row| row.api == "Buffer.prototype.copy"));
        assert!(MANIFEST_ROWS
            .iter()
            .any(|row| row.api == "Buffer.prototype.subarray"));
        for row in rows {
            assert!(matches!(row.receiver, NativeApiReceiver::Buffer));
            assert!(row.constructor);
            assert!(row.registration.writable);
            assert!(row.registration.enumerable);
            assert!(row.registration.configurable);
            assert!(row.registration.function_prototype);
            assert!(row.ihi.is_none(), "{} must not claim IHI", row.api);
            assert!(row.jit_ic.is_none(), "{} must not claim JIT", row.api);
            assert!(
                row.lejit_expectation.is_none(),
                "{} must not claim LeJIT",
                row.api
            );
        }
        assert_eq!(BUFFER_COPY.registration.length, 4);
        assert_eq!(BUFFER_FILL.registration.length, 4);
        assert_eq!(BUFFER_SLICE.registration.length, 2);
        assert_eq!(BUFFER_SUBARRAY.registration.length, 2);
        assert_eq!(
            BUFFER_FILL.validation.encoding_policy,
            "string fill uses strict label or ERR_UNKNOWN_ENCODING"
        );
        assert_eq!(
            BUFFER_COPY.validation.effect,
            NativeApiEffect::MutatesReceiverBytes
        );
        assert_eq!(
            BUFFER_SLICE.validation.range_window,
            "returns Buffer view over byte window; negative offsets clamp"
        );
        assert_eq!(
            crate::native_api_manifest_generated::buffer_copy_generated_ihi_spec(),
            None
        );
    }

    #[test]
    fn buffer_static_allocation_mouths_match_node_runtime_shape() {
        let actual = run_manifest_json(
            r#"
            JSON.stringify({
              shape: ['alloc','allocUnsafe','allocUnsafeSlow','concat','isBuffer','of','from'].map((m) => {
                const d = Object.getOwnPropertyDescriptor(Buffer, m);
                const pd = Object.getOwnPropertyDescriptor(Buffer[m], 'prototype');
                return [m, Buffer[m].length, d.writable, d.enumerable, d.configurable, !!pd, pd && pd.writable, pd && pd.enumerable, pd && pd.configurable];
              }),
              results: (() => {
                const val = (fn) => { try { return fn(); } catch (e) { return [e.name, e.code || null]; } };
                return [
                  [...Buffer.alloc(4)],
                  [...Buffer.alloc(5,'ab')],
                  [...Buffer.alloc(4,'ff','hex')],
                  val(() => Buffer.alloc(2,'x','bogus')),
                  val(() => Buffer.alloc(-1)),
                  val(() => Buffer.alloc('2')),
                  Buffer.allocUnsafe(3).length,
                  val(() => Buffer.allocUnsafe(-1)),
                  Buffer.allocUnsafeSlow(3).length,
                  [...Buffer.concat([Buffer.from([1,2]), new Uint8Array([3])])],
                  [...Buffer.concat([Buffer.from([1,2]), Buffer.from([3,4])], 3)],
                  [...Buffer.concat([Buffer.from([1])], 3)],
                  val(() => Buffer.concat([{}])),
                  [Buffer.isBuffer(Buffer.alloc(1)), Buffer.isBuffer(new Uint8Array(1)), Buffer.isBuffer({__is_buffer__:true})],
                  [...Buffer.of(257,-1,'3')],
                  [...Buffer.from([257,-1,'3'])],
                  [...Buffer.from(new Uint16Array([0x0102,0x0304]))],
                  (() => { const ab=new ArrayBuffer(4); const u=new Uint8Array(ab); u.set([1,2,3,4]); const b=Buffer.from(ab,1,2); b[0]=9; return [[...b],[...u]]; })(),
                  [...Buffer.from({type:'Buffer', data:[5,6]})],
                  val(() => Buffer.from({})),
                  val(() => Buffer.from(2)),
                  [...new Buffer(3)],
                  [...new Buffer('hi')],
                  val(() => new Buffer({}))
                ];
              })()
            })
            "#,
        );
        assert_eq!(
            actual,
            r#"{"shape":[["alloc",3,true,true,true,true,true,false,false],["allocUnsafe",1,true,true,true,true,true,false,false],["allocUnsafeSlow",1,true,true,true,true,true,false,false],["concat",2,true,true,true,true,true,false,false],["isBuffer",1,true,true,true,true,true,false,false],["of",0,true,true,true,false,null,null,null],["from",3,true,true,true,true,true,false,false]],"results":[[0,0,0,0],[97,98,97,98,97],[255,255,255,255],["TypeError","ERR_UNKNOWN_ENCODING"],["RangeError",null],["TypeError",null],3,["RangeError",null],3,[1,2,3],[1,2,3],[1,0,0],["TypeError",null],[true,false,false],[1,255,3],[1,255,3],[2,4],[[9,3],[1,9,3,4]],[5,6],["TypeError","ERR_INVALID_ARG_TYPE"],["TypeError","ERR_INVALID_ARG_TYPE"],[0,0,0],[104,105],["TypeError","ERR_INVALID_ARG_TYPE"]]}"#
        );
    }

    #[test]
    fn buffer_static_allocation_rows_have_generated_schema_facts_without_fast_claims() {
        let rows = [
            BUFFER_ALLOC,
            BUFFER_ALLOC_UNSAFE,
            BUFFER_ALLOC_UNSAFE_SLOW,
            BUFFER_CONCAT,
            BUFFER_IS_BUFFER,
            BUFFER_OF,
        ];
        assert!(MANIFEST_ROWS.iter().any(|row| row.api == "Buffer.alloc"));
        assert!(MANIFEST_ROWS.iter().any(|row| row.api == "Buffer.of"));
        for row in rows {
            assert!(matches!(row.receiver, NativeApiReceiver::Buffer));
            assert!(matches!(row.kind, NativeApiKind::StaticMethod));
            assert!(row.registration.writable);
            assert!(row.registration.enumerable);
            assert!(row.registration.configurable);
            assert!(row.ihi.is_none(), "{} must not claim IHI", row.api);
            assert!(row.jit_ic.is_none(), "{} must not claim JIT", row.api);
            assert!(
                row.lejit_expectation.is_none(),
                "{} must not claim LeJIT",
                row.api
            );
        }
        assert_eq!(BUFFER_ALLOC.registration.length, 3);
        assert_eq!(BUFFER_CONCAT.registration.length, 2);
        assert_eq!(BUFFER_OF.registration.length, 0);
        assert!(BUFFER_ALLOC.registration.function_prototype);
        assert!(!BUFFER_OF.registration.function_prototype);
        assert!(!BUFFER_OF.constructor);
        assert_eq!(BUFFER_ALLOC.returns, &[NativeApiDomain::Buffer]);
        assert_eq!(
            BUFFER_ALLOC.validation.encoding_policy,
            "string fill uses strict label or ERR_UNKNOWN_ENCODING"
        );
        assert_eq!(
            crate::native_api_manifest_generated::buffer_static_alloc_generated_ihi_spec(),
            None
        );
    }

    #[test]
    fn string_char_code_at_manifest_observable_function_facts_hold() {
        assert_eq!(STRING_CHAR_CODE_AT.constructor, false);
        assert_eq!(STRING_CHAR_CODE_AT.arity, 1);
    }

    #[test]
    fn string_char_code_at_manifest_native_slow_path_fixtures_hold() {
        let mut rt = Runtime::new();
        assert_eq!(
            crate::generated::string_prototype_char_code_at(
                &mut rt,
                string_value("ABC"),
                &[Value::Number(1.0)]
            )
            .unwrap(),
            Value::Number(66.0)
        );
        assert_eq!(
            crate::generated::string_prototype_char_code_at(
                &mut rt,
                Value::Number(123.0),
                &[Value::Number(0.0)]
            )
            .unwrap(),
            Value::Number(49.0)
        );
        match crate::generated::string_prototype_char_code_at(
            &mut rt,
            string_value("ABC"),
            &[Value::Number(-1.0)],
        )
        .unwrap()
        {
            Value::Number(n) => assert!(n.is_nan()),
            other => panic!("expected NaN number, got {other:?}"),
        }
    }

    #[test]
    fn dataview_byte_getset_manifest_rows_are_generated_checked_and_fast_path_ineligible() {
        for row in [
            DATAVIEW_GET_INT8,
            DATAVIEW_GET_UINT8,
            DATAVIEW_SET_INT8,
            DATAVIEW_SET_UINT8,
        ] {
            assert_eq!(row.receiver, NativeApiReceiver::DataView);
            assert_eq!(row.kind, NativeApiKind::Method);
            assert_eq!(row.registration.property, row.property);
            assert_eq!(row.registration.writable, true);
            assert_eq!(row.registration.enumerable, false);
            assert_eq!(row.registration.configurable, true);
            assert_eq!(row.registration.constructor, false);
            assert_eq!(row.registration.function_prototype, false);
            assert_eq!(row.validation.receiver, NativeApiReceiver::DataView);
            assert_eq!(
                row.validation.index_coercion,
                NativeApiIndexCoercion::ToIntegerOrInfinity
            );
            assert_eq!(
                row.validation.negative_or_infinite_result,
                NativeApiExceptionalResult::RangeError
            );
            assert_eq!(
                row.validation.out_of_range_result,
                NativeApiExceptionalResult::RangeError
            );
            assert_eq!(row.validation.receiver_semantics, "dataview-brand-detach");
            assert_eq!(row.validation.range_window, "byte-offset");
            assert_eq!(row.validation.byte_width, 1);
            assert_eq!(row.validation.endian, NativeApiEndian::SingleByte);
            assert!(row
                .validation
                .error_codes
                .contains(&"TypeError:detached-buffer"));
            assert!(row.ihi.is_none(), "{} must not claim IHI", row.api);
            assert!(row.jit_ic.is_none(), "{} must not claim JIT", row.api);
            assert!(
                row.lejit_expectation.is_none(),
                "{} must not claim LeJIT",
                row.api
            );
            assert!(row.fixtures.contains(&"consumer-ineligible"));
            assert!(MANIFEST_ROWS
                .iter()
                .any(|candidate| candidate.api == row.api));
        }

        assert_eq!(DATAVIEW_GET_INT8.arity, 1);
        assert_eq!(DATAVIEW_SET_INT8.arity, 2);
        assert_eq!(DATAVIEW_GET_INT8.returns, &[NativeApiDomain::Number]);
        assert_eq!(DATAVIEW_SET_UINT8.returns, &[NativeApiDomain::Undefined]);
        assert_eq!(DATAVIEW_GET_INT8.validation.signed, true);
        assert_eq!(DATAVIEW_GET_UINT8.validation.signed, false);
        assert_eq!(
            DATAVIEW_SET_INT8.validation.effect,
            NativeApiEffect::MutatesReceiverBytes
        );
        assert_eq!(DATAVIEW_GET_UINT8.validation.effect, NativeApiEffect::Pure);
        assert_eq!(
            crate::native_api_manifest_generated::data_view_get_int8_generated_ihi_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::data_view_set_uint8_generated_jit_ic_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::
                data_view_set_int8_generated_lejit_expectation_spec(),
            None
        );
    }

    #[test]
    fn dataview_byte_getset_manifest_matches_live_registration_descriptors() {
        let mut rt = Runtime::new();
        rt.install_intrinsics();
        let dataview_ctor = match rt.global_get("DataView") {
            Value::Object(id) => id,
            other => panic!("expected global DataView constructor, got {other:?}"),
        };
        let dataview_proto = match rt.object_get(dataview_ctor, "prototype") {
            Value::Object(id) => id,
            other => panic!("expected DataView.prototype object, got {other:?}"),
        };

        for row in [
            DATAVIEW_GET_INT8,
            DATAVIEW_GET_UINT8,
            DATAVIEW_SET_INT8,
            DATAVIEW_SET_UINT8,
        ] {
            let desc = rt
                .obj(dataview_proto)
                .get_own(row.property)
                .unwrap_or_else(|| panic!("missing {} descriptor", row.api));
            assert_eq!(desc.writable, row.registration.writable, "{}", row.api);
            assert_eq!(desc.enumerable, row.registration.enumerable, "{}", row.api);
            assert_eq!(
                desc.configurable, row.registration.configurable,
                "{}",
                row.api
            );
            let fn_id = match rt.object_get(dataview_proto, row.property) {
                Value::Object(id) => id,
                other => panic!("expected {} function, got {other:?}", row.api),
            };
            assert_eq!(
                rt.object_get(fn_id, "name"),
                string_value(row.registration.display_name),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "length"),
                Value::Number(row.registration.length as f64),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "prototype"),
                Value::Undefined,
                "{}",
                row.api
            );
        }
    }

    #[test]
    fn dataview_int16_getset_manifest_rows_are_generated_checked_and_fast_path_ineligible() {
        for row in [
            DATAVIEW_GET_INT16,
            DATAVIEW_GET_UINT16,
            DATAVIEW_SET_INT16,
            DATAVIEW_SET_UINT16,
        ] {
            assert_eq!(row.receiver, NativeApiReceiver::DataView);
            assert_eq!(row.kind, NativeApiKind::Method);
            assert_eq!(row.registration.property, row.property);
            assert_eq!(row.registration.writable, true);
            assert_eq!(row.registration.enumerable, false);
            assert_eq!(row.registration.configurable, true);
            assert_eq!(row.registration.constructor, false);
            assert_eq!(row.registration.function_prototype, false);
            assert_eq!(row.validation.receiver, NativeApiReceiver::DataView);
            assert_eq!(
                row.validation.index_coercion,
                NativeApiIndexCoercion::ToIntegerOrInfinity
            );
            assert_eq!(
                row.validation.negative_or_infinite_result,
                NativeApiExceptionalResult::RangeError
            );
            assert_eq!(
                row.validation.out_of_range_result,
                NativeApiExceptionalResult::RangeError
            );
            assert_eq!(row.validation.receiver_semantics, "dataview-brand-detach");
            assert_eq!(row.validation.range_window, "byte-offset");
            assert_eq!(row.validation.byte_width, 2);
            assert_eq!(row.validation.endian, NativeApiEndian::RuntimeFlag);
            assert!(row
                .validation
                .error_codes
                .contains(&"TypeError:detached-buffer"));
            assert!(row.ihi.is_none(), "{} must not claim IHI", row.api);
            assert!(row.jit_ic.is_none(), "{} must not claim JIT", row.api);
            assert!(
                row.lejit_expectation.is_none(),
                "{} must not claim LeJIT",
                row.api
            );
            assert!(row.fixtures.contains(&"runtime-endian-flag"));
            assert!(row.fixtures.contains(&"consumer-ineligible"));
            assert!(MANIFEST_ROWS
                .iter()
                .any(|candidate| candidate.api == row.api));
        }

        assert_eq!(DATAVIEW_GET_INT16.arity, 1);
        assert_eq!(DATAVIEW_SET_INT16.arity, 2);
        assert_eq!(DATAVIEW_GET_INT16.returns, &[NativeApiDomain::Number]);
        assert_eq!(DATAVIEW_SET_UINT16.returns, &[NativeApiDomain::Undefined]);
        assert_eq!(DATAVIEW_GET_INT16.validation.signed, true);
        assert_eq!(DATAVIEW_GET_UINT16.validation.signed, false);
        assert_eq!(
            DATAVIEW_SET_INT16.validation.effect,
            NativeApiEffect::MutatesReceiverBytes
        );
        assert_eq!(DATAVIEW_GET_UINT16.validation.effect, NativeApiEffect::Pure);
        assert_eq!(
            crate::native_api_manifest_generated::data_view_get_int16_generated_ihi_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::data_view_set_uint16_generated_jit_ic_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::
                data_view_set_int16_generated_lejit_expectation_spec(),
            None
        );
    }

    #[test]
    fn dataview_int16_getset_manifest_matches_live_registration_descriptors() {
        let mut rt = Runtime::new();
        rt.install_intrinsics();
        let dataview_ctor = match rt.global_get("DataView") {
            Value::Object(id) => id,
            other => panic!("expected global DataView constructor, got {other:?}"),
        };
        let dataview_proto = match rt.object_get(dataview_ctor, "prototype") {
            Value::Object(id) => id,
            other => panic!("expected DataView.prototype object, got {other:?}"),
        };

        for row in [
            DATAVIEW_GET_INT16,
            DATAVIEW_GET_UINT16,
            DATAVIEW_SET_INT16,
            DATAVIEW_SET_UINT16,
        ] {
            let desc = rt
                .obj(dataview_proto)
                .get_own(row.property)
                .unwrap_or_else(|| panic!("missing {} descriptor", row.api));
            assert_eq!(desc.writable, row.registration.writable, "{}", row.api);
            assert_eq!(desc.enumerable, row.registration.enumerable, "{}", row.api);
            assert_eq!(
                desc.configurable, row.registration.configurable,
                "{}",
                row.api
            );
            let fn_id = match rt.object_get(dataview_proto, row.property) {
                Value::Object(id) => id,
                other => panic!("expected {} function, got {other:?}", row.api),
            };
            assert_eq!(
                rt.object_get(fn_id, "name"),
                string_value(row.registration.display_name),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "length"),
                Value::Number(row.registration.length as f64),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "prototype"),
                Value::Undefined,
                "{}",
                row.api
            );
        }
    }

    #[test]
    fn dataview_int32_getset_manifest_rows_are_generated_checked_and_fast_path_ineligible() {
        for row in [
            DATAVIEW_GET_INT32,
            DATAVIEW_GET_UINT32,
            DATAVIEW_SET_INT32,
            DATAVIEW_SET_UINT32,
        ] {
            assert_eq!(row.receiver, NativeApiReceiver::DataView);
            assert_eq!(row.kind, NativeApiKind::Method);
            assert_eq!(row.registration.property, row.property);
            assert_eq!(row.registration.writable, true);
            assert_eq!(row.registration.enumerable, false);
            assert_eq!(row.registration.configurable, true);
            assert_eq!(row.registration.constructor, false);
            assert_eq!(row.registration.function_prototype, false);
            assert_eq!(row.validation.receiver, NativeApiReceiver::DataView);
            assert_eq!(
                row.validation.index_coercion,
                NativeApiIndexCoercion::ToIntegerOrInfinity
            );
            assert_eq!(
                row.validation.negative_or_infinite_result,
                NativeApiExceptionalResult::RangeError
            );
            assert_eq!(
                row.validation.out_of_range_result,
                NativeApiExceptionalResult::RangeError
            );
            assert_eq!(row.validation.receiver_semantics, "dataview-brand-detach");
            assert_eq!(row.validation.range_window, "byte-offset");
            assert_eq!(row.validation.byte_width, 4);
            assert_eq!(row.validation.endian, NativeApiEndian::RuntimeFlag);
            assert!(row
                .validation
                .error_codes
                .contains(&"TypeError:detached-buffer"));
            assert!(row.ihi.is_none(), "{} must not claim IHI", row.api);
            assert!(row.jit_ic.is_none(), "{} must not claim JIT", row.api);
            assert!(
                row.lejit_expectation.is_none(),
                "{} must not claim LeJIT",
                row.api
            );
            assert!(row.fixtures.contains(&"runtime-endian-flag"));
            assert!(row.fixtures.contains(&"consumer-ineligible"));
            assert!(MANIFEST_ROWS
                .iter()
                .any(|candidate| candidate.api == row.api));
        }

        assert_eq!(DATAVIEW_GET_INT32.arity, 1);
        assert_eq!(DATAVIEW_SET_INT32.arity, 2);
        assert_eq!(DATAVIEW_GET_INT32.returns, &[NativeApiDomain::Number]);
        assert_eq!(DATAVIEW_SET_UINT32.returns, &[NativeApiDomain::Undefined]);
        assert_eq!(DATAVIEW_GET_INT32.validation.signed, true);
        assert_eq!(DATAVIEW_GET_UINT32.validation.signed, false);
        assert_eq!(
            DATAVIEW_SET_INT32.validation.effect,
            NativeApiEffect::MutatesReceiverBytes
        );
        assert_eq!(DATAVIEW_GET_UINT32.validation.effect, NativeApiEffect::Pure);
        assert_eq!(
            crate::native_api_manifest_generated::data_view_get_int32_generated_ihi_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::data_view_set_uint32_generated_jit_ic_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::
                data_view_set_int32_generated_lejit_expectation_spec(),
            None
        );
    }

    #[test]
    fn dataview_int32_getset_manifest_matches_live_registration_descriptors() {
        let mut rt = Runtime::new();
        rt.install_intrinsics();
        let dataview_ctor = match rt.global_get("DataView") {
            Value::Object(id) => id,
            other => panic!("expected global DataView constructor, got {other:?}"),
        };
        let dataview_proto = match rt.object_get(dataview_ctor, "prototype") {
            Value::Object(id) => id,
            other => panic!("expected DataView.prototype object, got {other:?}"),
        };

        for row in [
            DATAVIEW_GET_INT32,
            DATAVIEW_GET_UINT32,
            DATAVIEW_SET_INT32,
            DATAVIEW_SET_UINT32,
        ] {
            let desc = rt
                .obj(dataview_proto)
                .get_own(row.property)
                .unwrap_or_else(|| panic!("missing {} descriptor", row.api));
            assert_eq!(desc.writable, row.registration.writable, "{}", row.api);
            assert_eq!(desc.enumerable, row.registration.enumerable, "{}", row.api);
            assert_eq!(
                desc.configurable, row.registration.configurable,
                "{}",
                row.api
            );
            let fn_id = match rt.object_get(dataview_proto, row.property) {
                Value::Object(id) => id,
                other => panic!("expected {} function, got {other:?}", row.api),
            };
            assert_eq!(
                rt.object_get(fn_id, "name"),
                string_value(row.registration.display_name),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "length"),
                Value::Number(row.registration.length as f64),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "prototype"),
                Value::Undefined,
                "{}",
                row.api
            );
        }
    }

    #[test]
    fn dataview_float_getset_manifest_rows_are_generated_checked_and_fast_path_ineligible() {
        for row in [
            DATAVIEW_GET_FLOAT32,
            DATAVIEW_GET_FLOAT64,
            DATAVIEW_SET_FLOAT32,
            DATAVIEW_SET_FLOAT64,
        ] {
            assert_eq!(row.receiver, NativeApiReceiver::DataView);
            assert_eq!(row.kind, NativeApiKind::Method);
            assert_eq!(row.registration.property, row.property);
            assert_eq!(row.registration.writable, true);
            assert_eq!(row.registration.enumerable, false);
            assert_eq!(row.registration.configurable, true);
            assert_eq!(row.registration.constructor, false);
            assert_eq!(row.registration.function_prototype, false);
            assert_eq!(row.validation.receiver, NativeApiReceiver::DataView);
            assert_eq!(
                row.validation.index_coercion,
                NativeApiIndexCoercion::ToIntegerOrInfinity
            );
            assert_eq!(
                row.validation.negative_or_infinite_result,
                NativeApiExceptionalResult::RangeError
            );
            assert_eq!(
                row.validation.out_of_range_result,
                NativeApiExceptionalResult::RangeError
            );
            assert_eq!(row.validation.receiver_semantics, "dataview-brand-detach");
            assert_eq!(row.validation.range_window, "byte-offset");
            assert!(
                row.validation.byte_width == 4 || row.validation.byte_width == 8,
                "{}",
                row.api
            );
            assert_eq!(row.validation.endian, NativeApiEndian::RuntimeFlag);
            assert!(row
                .validation
                .error_codes
                .contains(&"TypeError:detached-buffer"));
            assert!(row.ihi.is_none(), "{} must not claim IHI", row.api);
            assert!(row.jit_ic.is_none(), "{} must not claim JIT", row.api);
            assert!(
                row.lejit_expectation.is_none(),
                "{} must not claim LeJIT",
                row.api
            );
            assert!(row.fixtures.contains(&"runtime-endian-flag"));
            assert!(row.fixtures.contains(&"consumer-ineligible"));
            assert!(MANIFEST_ROWS
                .iter()
                .any(|candidate| candidate.api == row.api));
        }

        assert_eq!(DATAVIEW_GET_FLOAT32.arity, 1);
        assert_eq!(DATAVIEW_SET_FLOAT32.arity, 2);
        assert_eq!(DATAVIEW_GET_FLOAT32.returns, &[NativeApiDomain::Number]);
        assert_eq!(DATAVIEW_SET_FLOAT64.returns, &[NativeApiDomain::Undefined]);
        assert_eq!(DATAVIEW_GET_FLOAT32.validation.byte_width, 4);
        assert_eq!(DATAVIEW_GET_FLOAT64.validation.byte_width, 8);
        assert_eq!(
            DATAVIEW_SET_FLOAT32.validation.effect,
            NativeApiEffect::MutatesReceiverBytes
        );
        assert_eq!(
            DATAVIEW_GET_FLOAT64.validation.effect,
            NativeApiEffect::Pure
        );
        assert_eq!(
            crate::native_api_manifest_generated::data_view_get_float32_generated_ihi_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::data_view_set_float64_generated_jit_ic_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::
                data_view_set_float32_generated_lejit_expectation_spec(),
            None
        );
    }

    #[test]
    fn dataview_float_getset_manifest_matches_live_registration_descriptors() {
        let mut rt = Runtime::new();
        rt.install_intrinsics();
        let dataview_ctor = match rt.global_get("DataView") {
            Value::Object(id) => id,
            other => panic!("expected global DataView constructor, got {other:?}"),
        };
        let dataview_proto = match rt.object_get(dataview_ctor, "prototype") {
            Value::Object(id) => id,
            other => panic!("expected DataView.prototype object, got {other:?}"),
        };

        for row in [
            DATAVIEW_GET_FLOAT32,
            DATAVIEW_GET_FLOAT64,
            DATAVIEW_SET_FLOAT32,
            DATAVIEW_SET_FLOAT64,
        ] {
            let desc = rt
                .obj(dataview_proto)
                .get_own(row.property)
                .unwrap_or_else(|| panic!("missing {} descriptor", row.api));
            assert_eq!(desc.writable, row.registration.writable, "{}", row.api);
            assert_eq!(desc.enumerable, row.registration.enumerable, "{}", row.api);
            assert_eq!(
                desc.configurable, row.registration.configurable,
                "{}",
                row.api
            );
            let fn_id = match rt.object_get(dataview_proto, row.property) {
                Value::Object(id) => id,
                other => panic!("expected {} function, got {other:?}", row.api),
            };
            assert_eq!(
                rt.object_get(fn_id, "name"),
                string_value(row.registration.display_name),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "length"),
                Value::Number(row.registration.length as f64),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "prototype"),
                Value::Undefined,
                "{}",
                row.api
            );
        }
    }

    #[test]
    fn dataview_float16_getset_manifest_rows_are_generated_checked_and_fast_path_ineligible() {
        for row in [DATAVIEW_GET_FLOAT16, DATAVIEW_SET_FLOAT16] {
            assert_eq!(row.receiver, NativeApiReceiver::DataView);
            assert_eq!(row.kind, NativeApiKind::Method);
            assert_eq!(row.registration.property, row.property);
            assert_eq!(row.registration.writable, true);
            assert_eq!(row.registration.enumerable, false);
            assert_eq!(row.registration.configurable, true);
            assert_eq!(row.registration.constructor, false);
            assert_eq!(row.registration.function_prototype, false);
            assert_eq!(row.validation.receiver, NativeApiReceiver::DataView);
            assert_eq!(
                row.validation.index_coercion,
                NativeApiIndexCoercion::ToIntegerOrInfinity
            );
            assert_eq!(
                row.validation.negative_or_infinite_result,
                NativeApiExceptionalResult::RangeError
            );
            assert_eq!(
                row.validation.out_of_range_result,
                NativeApiExceptionalResult::RangeError
            );
            assert_eq!(row.validation.receiver_semantics, "dataview-brand-detach");
            assert_eq!(row.validation.range_window, "byte-offset");
            assert_eq!(row.validation.byte_width, 2);
            assert_eq!(row.validation.endian, NativeApiEndian::RuntimeFlag);
            assert!(row
                .validation
                .error_codes
                .contains(&"TypeError:detached-buffer"));
            assert!(row.ihi.is_none(), "{} must not claim IHI", row.api);
            assert!(row.jit_ic.is_none(), "{} must not claim JIT", row.api);
            assert!(
                row.lejit_expectation.is_none(),
                "{} must not claim LeJIT",
                row.api
            );
            assert!(row.fixtures.contains(&"runtime-endian-flag"));
            assert!(row.fixtures.contains(&"consumer-ineligible"));
            assert!(MANIFEST_ROWS
                .iter()
                .any(|candidate| candidate.api == row.api));
        }

        assert_eq!(DATAVIEW_GET_FLOAT16.arity, 1);
        assert_eq!(DATAVIEW_SET_FLOAT16.arity, 2);
        assert_eq!(DATAVIEW_GET_FLOAT16.returns, &[NativeApiDomain::Number]);
        assert_eq!(DATAVIEW_SET_FLOAT16.returns, &[NativeApiDomain::Undefined]);
        assert_eq!(DATAVIEW_GET_FLOAT16.validation.byte_width, 2);
        assert_eq!(
            DATAVIEW_SET_FLOAT16.validation.effect,
            NativeApiEffect::MutatesReceiverBytes
        );
        assert_eq!(
            DATAVIEW_GET_FLOAT16.validation.effect,
            NativeApiEffect::Pure
        );
        assert_eq!(
            crate::native_api_manifest_generated::data_view_get_float16_generated_ihi_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::data_view_set_float16_generated_jit_ic_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::
                data_view_set_float16_generated_lejit_expectation_spec(),
            None
        );
    }

    #[test]
    fn dataview_float16_getset_manifest_matches_live_registration_descriptors() {
        let mut rt = Runtime::new();
        rt.install_intrinsics();
        let dataview_ctor = match rt.global_get("DataView") {
            Value::Object(id) => id,
            other => panic!("expected global DataView constructor, got {other:?}"),
        };
        let dataview_proto = match rt.object_get(dataview_ctor, "prototype") {
            Value::Object(id) => id,
            other => panic!("expected DataView.prototype object, got {other:?}"),
        };

        for row in [DATAVIEW_GET_FLOAT16, DATAVIEW_SET_FLOAT16] {
            let desc = rt
                .obj(dataview_proto)
                .get_own(row.property)
                .unwrap_or_else(|| panic!("missing {} descriptor", row.api));
            assert_eq!(desc.writable, row.registration.writable, "{}", row.api);
            assert_eq!(desc.enumerable, row.registration.enumerable, "{}", row.api);
            assert_eq!(
                desc.configurable, row.registration.configurable,
                "{}",
                row.api
            );
            let fn_id = match rt.object_get(dataview_proto, row.property) {
                Value::Object(id) => id,
                other => panic!("expected {} function, got {other:?}", row.api),
            };
            assert_eq!(
                rt.object_get(fn_id, "name"),
                string_value(row.registration.display_name),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "length"),
                Value::Number(row.registration.length as f64),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "prototype"),
                Value::Undefined,
                "{}",
                row.api
            );
        }
    }

    #[test]
    fn dataview_bigint_getset_manifest_rows_are_generated_checked_and_fast_path_ineligible() {
        for row in [
            DATAVIEW_GET_BIG_INT64,
            DATAVIEW_GET_BIG_UINT64,
            DATAVIEW_SET_BIG_INT64,
            DATAVIEW_SET_BIG_UINT64,
        ] {
            assert_eq!(row.receiver, NativeApiReceiver::DataView);
            assert_eq!(row.kind, NativeApiKind::Method);
            assert_eq!(row.registration.property, row.property);
            assert_eq!(row.registration.writable, true);
            assert_eq!(row.registration.enumerable, false);
            assert_eq!(row.registration.configurable, true);
            assert_eq!(row.registration.constructor, false);
            assert_eq!(row.registration.function_prototype, false);
            assert_eq!(row.validation.receiver, NativeApiReceiver::DataView);
            assert_eq!(
                row.validation.index_coercion,
                NativeApiIndexCoercion::ToIntegerOrInfinity
            );
            assert_eq!(
                row.validation.negative_or_infinite_result,
                NativeApiExceptionalResult::RangeError
            );
            assert_eq!(
                row.validation.out_of_range_result,
                NativeApiExceptionalResult::RangeError
            );
            assert_eq!(row.validation.receiver_semantics, "dataview-brand-detach");
            assert_eq!(row.validation.range_window, "byte-offset");
            assert_eq!(row.validation.byte_width, 8);
            assert_eq!(row.validation.endian, NativeApiEndian::RuntimeFlag);
            assert!(row
                .validation
                .error_codes
                .contains(&"TypeError:detached-buffer"));
            assert!(row.ihi.is_none(), "{} must not claim IHI", row.api);
            assert!(row.jit_ic.is_none(), "{} must not claim JIT", row.api);
            assert!(
                row.lejit_expectation.is_none(),
                "{} must not claim LeJIT",
                row.api
            );
            assert!(row.fixtures.contains(&"runtime-endian-flag"));
            assert!(row.fixtures.contains(&"consumer-ineligible"));
            assert!(MANIFEST_ROWS
                .iter()
                .any(|candidate| candidate.api == row.api));
        }

        assert_eq!(DATAVIEW_GET_BIG_INT64.arity, 1);
        assert_eq!(DATAVIEW_SET_BIG_INT64.arity, 2);
        assert_eq!(DATAVIEW_GET_BIG_INT64.returns, &[NativeApiDomain::BigInt]);
        assert_eq!(
            DATAVIEW_SET_BIG_UINT64.returns,
            &[NativeApiDomain::Undefined]
        );
        assert_eq!(
            DATAVIEW_SET_BIG_INT64.args,
            &[NativeApiDomain::Number, NativeApiDomain::BigInt]
        );
        assert_eq!(DATAVIEW_GET_BIG_INT64.validation.signed, true);
        assert_eq!(DATAVIEW_GET_BIG_UINT64.validation.signed, false);
        assert_eq!(
            DATAVIEW_SET_BIG_INT64.validation.effect,
            NativeApiEffect::MutatesReceiverBytes
        );
        assert_eq!(
            DATAVIEW_GET_BIG_UINT64.validation.effect,
            NativeApiEffect::Pure
        );
        assert!(DATAVIEW_SET_BIG_INT64
            .validation
            .error_codes
            .contains(&"TypeError:bigint-value-required"));
        assert_eq!(
            crate::native_api_manifest_generated::data_view_get_big_int64_generated_ihi_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::data_view_set_big_uint64_generated_jit_ic_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::
                data_view_set_big_int64_generated_lejit_expectation_spec(),
            None
        );
    }

    #[test]
    fn dataview_bigint_getset_manifest_matches_live_registration_descriptors() {
        let mut rt = Runtime::new();
        rt.install_intrinsics();
        let dataview_ctor = match rt.global_get("DataView") {
            Value::Object(id) => id,
            other => panic!("expected global DataView constructor, got {other:?}"),
        };
        let dataview_proto = match rt.object_get(dataview_ctor, "prototype") {
            Value::Object(id) => id,
            other => panic!("expected DataView.prototype object, got {other:?}"),
        };

        for row in [
            DATAVIEW_GET_BIG_INT64,
            DATAVIEW_GET_BIG_UINT64,
            DATAVIEW_SET_BIG_INT64,
            DATAVIEW_SET_BIG_UINT64,
        ] {
            let desc = rt
                .obj(dataview_proto)
                .get_own(row.property)
                .unwrap_or_else(|| panic!("missing {} descriptor", row.api));
            assert_eq!(desc.writable, row.registration.writable, "{}", row.api);
            assert_eq!(desc.enumerable, row.registration.enumerable, "{}", row.api);
            assert_eq!(
                desc.configurable, row.registration.configurable,
                "{}",
                row.api
            );
            let fn_id = match rt.object_get(dataview_proto, row.property) {
                Value::Object(id) => id,
                other => panic!("expected {} function, got {other:?}", row.api),
            };
            assert_eq!(
                rt.object_get(fn_id, "name"),
                string_value(row.registration.display_name),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "length"),
                Value::Number(row.registration.length as f64),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "prototype"),
                Value::Undefined,
                "{}",
                row.api
            );
        }
    }

    #[test]
    fn array_scalar_search_manifest_rows_are_generated_checked_and_fast_path_ineligible() {
        for row in [
            ARRAY_IS_ARRAY,
            ARRAY_AT,
            ARRAY_INCLUDES,
            ARRAY_INDEX_OF,
            ARRAY_LAST_INDEX_OF,
            ARRAY_FIND,
            ARRAY_FIND_INDEX,
            ARRAY_FIND_LAST,
            ARRAY_FIND_LAST_INDEX,
        ] {
            assert_eq!(row.receiver, NativeApiReceiver::Array);
            assert_eq!(row.arity, 1);
            assert_eq!(row.registration.property, row.property);
            assert_eq!(row.registration.length, 1);
            assert_eq!(row.registration.writable, true);
            assert_eq!(row.registration.enumerable, false);
            assert_eq!(row.registration.configurable, true);
            assert_eq!(row.registration.constructor, false);
            assert_eq!(row.registration.function_prototype, false);
            assert_eq!(row.validation.receiver, NativeApiReceiver::Array);
            assert_eq!(row.validation.arity, 1);
            assert_eq!(row.validation.effect, NativeApiEffect::Pure);
            assert_eq!(row.validation.argument_shape, "array-scalar-search");
            if row.kind == NativeApiKind::StaticMethod {
                assert_eq!(row.validation.receiver_semantics, "array-brand");
            } else {
                assert_eq!(row.validation.receiver_semantics, "array-like");
            }
            assert!(row.ihi.is_none());
            assert!(row.jit_ic.is_none());
            assert!(row.lejit_expectation.is_none());
            assert!(row.fixtures.contains(&"consumer-ineligible"));
        }

        assert_eq!(ARRAY_IS_ARRAY.kind, NativeApiKind::StaticMethod);
        assert_eq!(ARRAY_IS_ARRAY.args, &[NativeApiDomain::Unknown]);
        assert_eq!(ARRAY_IS_ARRAY.returns, &[NativeApiDomain::Boolean]);
        assert_eq!(ARRAY_AT.args, &[NativeApiDomain::Number]);
        assert_eq!(
            ARRAY_AT.returns,
            &[NativeApiDomain::Unknown, NativeApiDomain::Undefined]
        );
        assert_eq!(ARRAY_INCLUDES.returns, &[NativeApiDomain::Boolean]);
        assert_eq!(ARRAY_INDEX_OF.returns, &[NativeApiDomain::Number]);
        assert_eq!(ARRAY_LAST_INDEX_OF.returns, &[NativeApiDomain::Number]);
        assert_eq!(
            ARRAY_FIND.returns,
            &[NativeApiDomain::Unknown, NativeApiDomain::Undefined]
        );
        assert_eq!(
            ARRAY_FIND_LAST.returns,
            &[NativeApiDomain::Unknown, NativeApiDomain::Undefined]
        );
        assert_eq!(ARRAY_FIND.validation.callback_policy, "predicate-forward");
        assert_eq!(
            ARRAY_FIND_INDEX.validation.callback_policy,
            "predicate-forward"
        );
        assert_eq!(
            ARRAY_FIND_LAST.validation.callback_policy,
            "predicate-reverse"
        );
        assert_eq!(
            ARRAY_FIND_LAST_INDEX.validation.callback_policy,
            "predicate-reverse"
        );

        assert_eq!(
            crate::native_api_manifest_generated::array_includes_generated_ihi_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::array_find_generated_jit_ic_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::array_find_last_generated_lejit_expectation_spec(
            ),
            None
        );
    }

    #[test]
    fn array_scalar_search_manifest_matches_live_registration_descriptors() {
        let mut rt = Runtime::new();
        rt.install_intrinsics();
        let array_ctor = match rt.global_get("Array") {
            Value::Object(id) => id,
            other => panic!("expected global Array constructor, got {other:?}"),
        };
        let array_proto = match rt.object_get(array_ctor, "prototype") {
            Value::Object(id) => id,
            other => panic!("expected Array.prototype object, got {other:?}"),
        };

        for row in [
            ARRAY_IS_ARRAY,
            ARRAY_AT,
            ARRAY_INCLUDES,
            ARRAY_INDEX_OF,
            ARRAY_LAST_INDEX_OF,
            ARRAY_FIND,
            ARRAY_FIND_INDEX,
            ARRAY_FIND_LAST,
            ARRAY_FIND_LAST_INDEX,
        ] {
            let target = if row.kind == NativeApiKind::StaticMethod {
                array_ctor
            } else {
                array_proto
            };
            let desc = rt
                .obj(target)
                .get_own(row.property)
                .unwrap_or_else(|| panic!("missing {} descriptor", row.api));
            assert_eq!(desc.writable, row.registration.writable, "{}", row.api);
            assert_eq!(desc.enumerable, row.registration.enumerable, "{}", row.api);
            assert_eq!(
                desc.configurable, row.registration.configurable,
                "{}",
                row.api
            );
            let fn_id = match rt.object_get(target, row.property) {
                Value::Object(id) => id,
                other => panic!("expected {} function, got {other:?}", row.api),
            };
            assert_eq!(
                rt.object_get(fn_id, "name"),
                string_value(row.registration.display_name),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "length"),
                Value::Number(row.registration.length as f64),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "prototype"),
                Value::Undefined,
                "{}",
                row.api
            );
            match &rt.obj(fn_id).internal_kind {
                crate::value::InternalKind::Function(internals) => {
                    assert_eq!(internals.name, row.registration.display_name);
                    assert_eq!(internals.length, row.registration.length);
                    assert_eq!(internals.is_constructor, row.registration.constructor);
                }
                other => panic!("expected {} Function internals, got {other:?}", row.api),
            }
        }
    }

    #[test]
    fn array_iteration_callback_manifest_rows_are_generated_checked_and_fast_path_ineligible() {
        for row in [
            ARRAY_FOR_EACH,
            ARRAY_MAP,
            ARRAY_FILTER,
            ARRAY_SOME,
            ARRAY_EVERY,
            ARRAY_REDUCE,
            ARRAY_REDUCE_RIGHT,
            ARRAY_FLAT_MAP,
        ] {
            assert!(MANIFEST_ROWS
                .iter()
                .any(|candidate| candidate.api == row.api));
            assert_eq!(row.receiver, NativeApiReceiver::Array);
            assert_eq!(row.kind, NativeApiKind::Method);
            assert_eq!(row.arity, 1);
            assert_eq!(row.args, &[NativeApiDomain::Unknown]);
            assert_eq!(row.registration.property, row.property);
            assert_eq!(row.registration.length, 1);
            assert!(row.registration.writable);
            assert!(!row.registration.enumerable);
            assert!(row.registration.configurable);
            assert!(!row.registration.constructor);
            assert!(!row.registration.function_prototype);
            assert_eq!(row.validation.receiver, NativeApiReceiver::Array);
            assert_eq!(row.validation.arity, 1);
            assert_eq!(row.validation.args, &[NativeApiDomain::Unknown]);
            assert_eq!(row.validation.effect, NativeApiEffect::Pure);
            assert_eq!(row.validation.argument_shape, "array-iteration-callback");
            assert_eq!(row.validation.receiver_semantics, "array-like");
            assert_eq!(row.validation.index_coercion, NativeApiIndexCoercion::None);
            assert!(row.ihi.is_none(), "{} unexpectedly has IHI facts", row.api);
            assert!(
                row.jit_ic.is_none(),
                "{} unexpectedly has JIT IC facts",
                row.api
            );
            assert!(
                row.lejit_expectation.is_none(),
                "{} unexpectedly has LeJIT facts",
                row.api
            );
            assert!(row.fixtures.contains(&"consumer-ineligible"));
        }

        assert_eq!(ARRAY_FOR_EACH.returns, &[NativeApiDomain::Undefined]);
        assert_eq!(ARRAY_MAP.returns, &[NativeApiDomain::Unknown]);
        assert_eq!(ARRAY_FILTER.returns, &[NativeApiDomain::Unknown]);
        assert_eq!(ARRAY_SOME.returns, &[NativeApiDomain::Boolean]);
        assert_eq!(ARRAY_EVERY.returns, &[NativeApiDomain::Boolean]);
        assert_eq!(ARRAY_REDUCE.returns, &[NativeApiDomain::Unknown]);
        assert_eq!(ARRAY_REDUCE_RIGHT.returns, &[NativeApiDomain::Unknown]);
        assert_eq!(ARRAY_FLAT_MAP.returns, &[NativeApiDomain::Unknown]);
        assert_eq!(
            ARRAY_FOR_EACH.validation.callback_policy,
            "iteration-forward-side-effect"
        );
        assert_eq!(
            ARRAY_MAP.validation.callback_policy,
            "iteration-forward-map"
        );
        assert_eq!(
            ARRAY_FILTER.validation.callback_policy,
            "iteration-forward-filter"
        );
        assert_eq!(
            ARRAY_SOME.validation.callback_policy,
            "iteration-forward-some"
        );
        assert_eq!(
            ARRAY_EVERY.validation.callback_policy,
            "iteration-forward-every"
        );
        assert_eq!(ARRAY_REDUCE.validation.callback_policy, "reducer-forward");
        assert_eq!(
            ARRAY_REDUCE_RIGHT.validation.callback_policy,
            "reducer-reverse"
        );
        assert_eq!(
            ARRAY_FLAT_MAP.validation.callback_policy,
            "iteration-forward-flat-map"
        );
        assert_eq!(
            ARRAY_REDUCE.validation.negative_or_infinite_result,
            NativeApiExceptionalResult::None
        );
        assert_eq!(
            ARRAY_REDUCE_RIGHT.validation.negative_or_infinite_result,
            NativeApiExceptionalResult::None
        );
        assert!(ARRAY_REDUCE
            .validation
            .error_codes
            .contains(&"TypeError:empty-without-initial-value"));
        assert!(ARRAY_REDUCE_RIGHT
            .validation
            .error_codes
            .contains(&"TypeError:empty-without-initial-value"));
        assert_eq!(
            crate::native_api_manifest_generated::array_for_each_generated_ihi_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::array_flat_map_generated_jit_ic_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::array_reduce_right_generated_lejit_expectation_spec(),
            None
        );
    }

    #[test]
    fn array_iteration_callback_manifest_matches_live_registration_descriptors() {
        let mut rt = Runtime::new();
        rt.install_intrinsics();
        let array_ctor = match rt.global_get("Array") {
            Value::Object(id) => id,
            other => panic!("expected global Array constructor, got {other:?}"),
        };
        let array_proto = match rt.object_get(array_ctor, "prototype") {
            Value::Object(id) => id,
            other => panic!("expected Array.prototype object, got {other:?}"),
        };

        for row in [
            ARRAY_FOR_EACH,
            ARRAY_MAP,
            ARRAY_FILTER,
            ARRAY_SOME,
            ARRAY_EVERY,
            ARRAY_REDUCE,
            ARRAY_REDUCE_RIGHT,
            ARRAY_FLAT_MAP,
        ] {
            let desc = rt
                .obj(array_proto)
                .get_own(row.property)
                .unwrap_or_else(|| panic!("missing {} descriptor", row.api));
            assert_eq!(desc.writable, row.registration.writable, "{}", row.api);
            assert_eq!(desc.enumerable, row.registration.enumerable, "{}", row.api);
            assert_eq!(
                desc.configurable, row.registration.configurable,
                "{}",
                row.api
            );
            let fn_id = match rt.object_get(array_proto, row.property) {
                Value::Object(id) => id,
                other => panic!("expected {} function, got {other:?}", row.api),
            };
            assert_eq!(
                rt.object_get(fn_id, "name"),
                string_value(row.registration.display_name),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "length"),
                Value::Number(row.registration.length as f64),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "prototype"),
                Value::Undefined,
                "{}",
                row.api
            );
            match &rt.obj(fn_id).internal_kind {
                crate::value::InternalKind::Function(internals) => {
                    assert_eq!(internals.name, row.registration.display_name);
                    assert_eq!(internals.length, row.registration.length);
                    assert_eq!(internals.is_constructor, row.registration.constructor);
                }
                other => panic!("expected {} Function internals, got {other:?}", row.api),
            }
        }
    }

    #[test]
    fn array_length_mutation_manifest_rows_are_generated_checked_and_fast_path_ineligible() {
        for row in [ARRAY_PUSH, ARRAY_POP, ARRAY_SHIFT, ARRAY_UNSHIFT] {
            assert!(MANIFEST_ROWS
                .iter()
                .any(|candidate| candidate.api == row.api));
            assert_eq!(row.receiver, NativeApiReceiver::Array);
            assert_eq!(row.kind, NativeApiKind::Method);
            assert_eq!(row.registration.property, row.property);
            assert!(row.registration.writable);
            assert!(!row.registration.enumerable);
            assert!(row.registration.configurable);
            assert!(!row.registration.constructor);
            assert!(!row.registration.function_prototype);
            assert_eq!(row.validation.receiver, NativeApiReceiver::Array);
            assert_eq!(row.validation.effect, NativeApiEffect::MutatesReceiverBytes);
            assert_eq!(row.validation.argument_shape, "array-length-mutating");
            assert_eq!(row.validation.receiver_semantics, "array-like");
            assert_eq!(row.validation.index_coercion, NativeApiIndexCoercion::None);
            assert!(row.ihi.is_none(), "{} unexpectedly has IHI facts", row.api);
            assert!(
                row.jit_ic.is_none(),
                "{} unexpectedly has JIT IC facts",
                row.api
            );
            assert!(
                row.lejit_expectation.is_none(),
                "{} unexpectedly has LeJIT facts",
                row.api
            );
            assert!(row.fixtures.contains(&"consumer-ineligible"));
        }

        assert_eq!(ARRAY_PUSH.arity, 1);
        assert_eq!(ARRAY_PUSH.args, &[NativeApiDomain::Unknown]);
        assert_eq!(ARRAY_PUSH.returns, &[NativeApiDomain::Number]);
        assert_eq!(ARRAY_PUSH.validation.range_window, "length-append");
        assert_eq!(ARRAY_POP.arity, 0);
        assert_eq!(ARRAY_POP.args, &[]);
        assert_eq!(
            ARRAY_POP.returns,
            &[NativeApiDomain::Unknown, NativeApiDomain::Undefined]
        );
        assert_eq!(ARRAY_POP.validation.range_window, "length-tail-delete");
        assert_eq!(ARRAY_SHIFT.arity, 0);
        assert_eq!(
            ARRAY_SHIFT.returns,
            &[NativeApiDomain::Unknown, NativeApiDomain::Undefined]
        );
        assert_eq!(ARRAY_SHIFT.validation.range_window, "length-head-shift");
        assert_eq!(ARRAY_UNSHIFT.arity, 1);
        assert_eq!(ARRAY_UNSHIFT.args, &[NativeApiDomain::Unknown]);
        assert_eq!(ARRAY_UNSHIFT.returns, &[NativeApiDomain::Number]);
        assert_eq!(ARRAY_UNSHIFT.validation.range_window, "length-head-insert");
        assert_eq!(
            crate::native_api_manifest_generated::array_push_generated_ihi_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::array_unshift_generated_jit_ic_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::array_shift_generated_lejit_expectation_spec(),
            None
        );
    }

    #[test]
    fn array_length_mutation_manifest_matches_live_registration_descriptors() {
        let mut rt = Runtime::new();
        rt.install_intrinsics();
        let array_ctor = match rt.global_get("Array") {
            Value::Object(id) => id,
            other => panic!("expected global Array constructor, got {other:?}"),
        };
        let array_proto = match rt.object_get(array_ctor, "prototype") {
            Value::Object(id) => id,
            other => panic!("expected Array.prototype object, got {other:?}"),
        };

        for row in [ARRAY_PUSH, ARRAY_POP, ARRAY_SHIFT, ARRAY_UNSHIFT] {
            let desc = rt
                .obj(array_proto)
                .get_own(row.property)
                .unwrap_or_else(|| panic!("missing {} descriptor", row.api));
            assert_eq!(desc.writable, row.registration.writable, "{}", row.api);
            assert_eq!(desc.enumerable, row.registration.enumerable, "{}", row.api);
            assert_eq!(
                desc.configurable, row.registration.configurable,
                "{}",
                row.api
            );
            let fn_id = match rt.object_get(array_proto, row.property) {
                Value::Object(id) => id,
                other => panic!("expected {} function, got {other:?}", row.api),
            };
            assert_eq!(
                rt.object_get(fn_id, "name"),
                string_value(row.registration.display_name),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "length"),
                Value::Number(row.registration.length as f64),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "prototype"),
                Value::Undefined,
                "{}",
                row.api
            );
            match &rt.obj(fn_id).internal_kind {
                crate::value::InternalKind::Function(internals) => {
                    assert_eq!(internals.name, row.registration.display_name);
                    assert_eq!(internals.length, row.registration.length);
                    assert_eq!(internals.is_constructor, row.registration.constructor);
                }
                other => panic!("expected {} Function internals, got {other:?}", row.api),
            }
        }
    }

    #[test]
    fn array_length_mutation_runtime_shape_and_semantics_match_node_oracle() {
        let actual = run_manifest_json(
            r#"
            (() => {
              const desc = (name) => {
                const f = Array.prototype[name];
                const d = Object.getOwnPropertyDescriptor(Array.prototype, name);
                const ctor = (() => { try { new f(); return 'ok'; } catch (e) { return e.name; } })();
                return [name, f.length, d.writable, d.enumerable, d.configurable, ctor, Object.prototype.hasOwnProperty.call(f, 'prototype')];
              };
              return JSON.stringify({
                shape: ['push','pop','shift','unshift'].map(desc),
                basic: (() => {
                  const a = [1,2];
                  const r1 = a.push(3,4);
                  const r2 = a.pop();
                  const r3 = a.shift();
                  const r4 = a.unshift(9,8);
                  return [r1,r2,r3,r4,a.length,a.join('|')];
                })(),
                arrayLike: (() => {
                  const o = {length: 2, 0:'a', 1:'b'};
                  const p = Array.prototype.push.call(o, 'c');
                  const q = Array.prototype.pop.call(o);
                  const r = Array.prototype.unshift.call(o, 'z');
                  const s = Array.prototype.shift.call(o);
                  return [p,q,r,s,o.length,o[0],o[1],Object.keys(o).sort().join(',')];
                })(),
                holes: (() => {
                  const a = [,'b',, 'd'];
                  const s = a.shift();
                  const afterShift = [s, a.length, 0 in a, 1 in a, 2 in a, a[0], a[1], a[2]];
                  const b = [,'b'];
                  const u = b.unshift('x');
                  return [afterShift, [u, b.length, 0 in b, 1 in b, 2 in b, b[0], b[1], b[2]]];
                })(),
                protoReads: (() => {
                  Array.prototype[0] = 'proto0';
                  Array.prototype[1] = 'proto1';
                  try {
                    const a = [, 'own1'];
                    const shift = a.shift();
                    const b = [, 'own1'];
                    const pop = b.pop();
                    return [shift, a.length, 0 in a, a[0], pop, b.length, 0 in b, b[0]];
                  } finally {
                    delete Array.prototype[0]; delete Array.prototype[1];
                  }
                })(),
                errors: ['push','pop','shift','unshift'].map((m) => {
                  try { Array.prototype[m].call(null, 1); return 'ok'; } catch(e) { return e.name; }
                }),
                lengthLimit: (() => {
                  const o = {length: 9007199254740991};
                  try { Array.prototype.push.call(o, 1); return 'ok'; } catch(e) { return [e.name, o.length]; }
                })(),
                nonWritableLength: (() => {
                  const a = [1];
                  Object.defineProperty(a, 'length', {writable:false});
                  try { a.push(2); return 'ok'; } catch(e) { return [e.name, a.length, a[1]]; }
                })(),
                override: (() => {
                  const old = Array.prototype.push;
                  try { Array.prototype.push = function(x){ return 'override:' + x; }; return [ [1].push(2) ]; }
                  finally { Array.prototype.push = old; }
                })()
              });
            })()
            "#,
        );
        assert_eq!(
            actual,
            r#"{"shape":[["push",1,true,false,true,"TypeError",false],["pop",0,true,false,true,"TypeError",false],["shift",0,true,false,true,"TypeError",false],["unshift",1,true,false,true,"TypeError",false]],"basic":[4,4,1,4,4,"9|8|2|3"],"arrayLike":[3,"c",3,"z",2,"a","b","0,1,length"],"holes":[[null,3,true,false,true,"b",null,"d"],[3,3,true,false,true,"x",null,"b"]],"protoReads":["proto0",1,true,"own1","own1",1,true,"proto0"],"errors":["TypeError","TypeError","TypeError","TypeError"],"lengthLimit":["TypeError",9007199254740991],"nonWritableLength":["TypeError",1,null],"override":["override:2"]}"#
        );
    }

    #[test]
    fn array_inplace_mutation_manifest_rows_are_generated_checked_and_fast_path_ineligible() {
        for row in [ARRAY_REVERSE, ARRAY_COPY_WITHIN, ARRAY_FILL] {
            assert!(MANIFEST_ROWS
                .iter()
                .any(|candidate| candidate.api == row.api));
            assert_eq!(row.receiver, NativeApiReceiver::Array);
            assert_eq!(row.kind, NativeApiKind::Method);
            assert_eq!(row.registration.property, row.property);
            assert!(row.registration.writable);
            assert!(!row.registration.enumerable);
            assert!(row.registration.configurable);
            assert!(!row.registration.constructor);
            assert!(!row.registration.function_prototype);
            assert_eq!(row.validation.receiver, NativeApiReceiver::Array);
            assert_eq!(row.validation.effect, NativeApiEffect::MutatesReceiverBytes);
            assert_eq!(row.validation.argument_shape, "array-inplace-mutation");
            assert_eq!(row.validation.receiver_semantics, "array-like");
            assert!(row.ihi.is_none(), "{} unexpectedly has IHI facts", row.api);
            assert!(
                row.jit_ic.is_none(),
                "{} unexpectedly has JIT IC facts",
                row.api
            );
            assert!(
                row.lejit_expectation.is_none(),
                "{} unexpectedly has LeJIT facts",
                row.api
            );
            assert!(row.fixtures.contains(&"consumer-ineligible"));
        }

        assert_eq!(ARRAY_REVERSE.arity, 0);
        assert_eq!(ARRAY_REVERSE.args, &[]);
        assert_eq!(ARRAY_REVERSE.returns, &[NativeApiDomain::Unknown]);
        assert_eq!(ARRAY_REVERSE.validation.range_window, "whole-length");
        assert_eq!(ARRAY_COPY_WITHIN.arity, 2);
        assert_eq!(
            ARRAY_COPY_WITHIN.args,
            &[NativeApiDomain::Number, NativeApiDomain::Number]
        );
        assert_eq!(
            ARRAY_COPY_WITHIN.validation.index_coercion,
            NativeApiIndexCoercion::ToIntegerOrInfinity
        );
        assert_eq!(
            ARRAY_COPY_WITHIN.validation.negative_or_infinite_result,
            NativeApiExceptionalResult::ClampedIndex
        );
        assert_eq!(
            ARRAY_COPY_WITHIN.validation.range_window,
            "target-start-end"
        );
        assert_eq!(ARRAY_FILL.arity, 1);
        assert_eq!(ARRAY_FILL.args, &[NativeApiDomain::Unknown]);
        assert_eq!(
            ARRAY_FILL.validation.index_coercion,
            NativeApiIndexCoercion::ToIntegerOrInfinity
        );
        assert_eq!(ARRAY_FILL.validation.range_window, "start-end-fill");
        assert_eq!(
            crate::native_api_manifest_generated::array_reverse_generated_ihi_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::array_copy_within_generated_jit_ic_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::array_fill_generated_lejit_expectation_spec(),
            None
        );
    }

    #[test]
    fn array_inplace_mutation_manifest_matches_live_registration_descriptors() {
        let mut rt = Runtime::new();
        rt.install_intrinsics();
        let array_ctor = match rt.global_get("Array") {
            Value::Object(id) => id,
            other => panic!("expected global Array constructor, got {other:?}"),
        };
        let array_proto = match rt.object_get(array_ctor, "prototype") {
            Value::Object(id) => id,
            other => panic!("expected Array.prototype object, got {other:?}"),
        };

        for row in [ARRAY_REVERSE, ARRAY_COPY_WITHIN, ARRAY_FILL] {
            let desc = rt
                .obj(array_proto)
                .get_own(row.property)
                .unwrap_or_else(|| panic!("missing {} descriptor", row.api));
            assert_eq!(desc.writable, row.registration.writable, "{}", row.api);
            assert_eq!(desc.enumerable, row.registration.enumerable, "{}", row.api);
            assert_eq!(
                desc.configurable, row.registration.configurable,
                "{}",
                row.api
            );
            let fn_id = match rt.object_get(array_proto, row.property) {
                Value::Object(id) => id,
                other => panic!("expected {} function, got {other:?}", row.api),
            };
            assert_eq!(
                rt.object_get(fn_id, "name"),
                string_value(row.registration.display_name),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "length"),
                Value::Number(row.registration.length as f64),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "prototype"),
                Value::Undefined,
                "{}",
                row.api
            );
        }
    }

    #[test]
    fn array_inplace_mutation_runtime_shape_and_semantics_match_node_oracle() {
        let actual = run_manifest_json(
            r#"
            (() => {
              const desc = (name) => {
                const f = Array.prototype[name];
                const d = Object.getOwnPropertyDescriptor(Array.prototype, name);
                const ctor = (() => { try { new f(); return 'ok'; } catch (e) { return e.name; } })();
                return [name, f.length, d.writable, d.enumerable, d.configurable, ctor, Object.prototype.hasOwnProperty.call(f, 'prototype')];
              };
              const keys = (o) => Object.keys(o).sort().join(',');
              return JSON.stringify({
                shape: ['reverse','copyWithin','fill'].map(desc),
                basic: (() => {
                  const a = [1,2,3,4];
                  const r = a.reverse();
                  const b = [1,2,3,4,5];
                  const c = b.copyWithin(1,3);
                  const d = [1,2,3,4];
                  const e = d.fill(9,1,3);
                  return [r === a, a.join('|'), c === b, b.join('|'), e === d, d.join('|')];
                })(),
                arrayLike: (() => {
                  const o = {length:4,0:'a',1:'b',3:'d'};
                  const r = Array.prototype.reverse.call(o);
                  const afterReverse = [r === o, o.length, o[0], o[1], 2 in o, o[2], o[3], keys(o)];
                  const p = {length:5,0:'a',1:'b',3:'d'};
                  const c = Array.prototype.copyWithin.call(p,1,3,5);
                  const afterCopy = [c === p, p.length, p[0], p[1], 2 in p, p[2], p[3], p[4], keys(p)];
                  const q = {length:3,0:'a'};
                  const f = Array.prototype.fill.call(q,'x',1);
                  return [afterReverse, afterCopy, [f === q, q.length, q[0], q[1], q[2], keys(q)]];
                })(),
                ranges: (() => {
                  const a = [0,1,2,3,4];
                  a.copyWithin(-2, -4, -1);
                  const b = [0,1,2,3,4];
                  b.copyWithin(0, 1, Infinity);
                  const c = [0,1,2,3];
                  c.fill('z', -3, -1);
                  const d = [0,1,2];
                  d.fill('n', NaN, -Infinity);
                  return [a.join('|'), b.join('|'), c.join('|'), d.join('|')];
                })(),
                holes: (() => {
                  const a = [,'b',,'d'];
                  a.reverse();
                  const b = ['a',,'c',,'e'];
                  b.copyWithin(1,3,5);
                  const c = new Array(4);
                  c.fill('x',1,3);
                  return [[0 in a,a[0],1 in a,a[1],2 in a,a[2],3 in a,a[3]], [0 in b,b[0],1 in b,b[1],2 in b,b[2],3 in b,b[3],4 in b,b[4]], [0 in c,1 in c,c[1],2 in c,c[2],3 in c,keys(c)]];
                })(),
                proto: (() => {
                  Array.prototype[0] = 'p0'; Array.prototype[2] = 'p2';
                  try {
                    const a = [, 'b', , 'd'];
                    a.reverse();
                    const b = [, 'b', , 'd'];
                    b.copyWithin(1,0,3);
                    return [[0 in a,a[0],1 in a,a[1],2 in a,a[2],3 in a,a[3]], [0 in b,b[0],1 in b,b[1],2 in b,b[2],3 in b,b[3]]];
                  } finally { delete Array.prototype[0]; delete Array.prototype[2]; }
                })(),
                errors: ['reverse','copyWithin','fill'].map((m) => {
                  try { Array.prototype[m].call(null, 1); return 'ok'; } catch(e) { return e.name; }
                }),
                override: (() => {
                  const old = Array.prototype.fill;
                  try { Array.prototype.fill = function(x){ return 'override:' + x; }; return [[1,2].fill(7)]; }
                  finally { Array.prototype.fill = old; }
                })()
              });
            })()
            "#,
        );
        assert_eq!(
            actual,
            r#"{"shape":[["reverse",0,true,false,true,"TypeError",false],["copyWithin",2,true,false,true,"TypeError",false],["fill",1,true,false,true,"TypeError",false]],"basic":[true,"4|3|2|1",true,"1|4|5|4|5",true,"1|9|9|4"],"arrayLike":[[true,4,"d",null,true,"b","a","0,2,3,length"],[true,5,"a","d",false,null,"d",null,"0,1,3,length"],[true,3,"a","x","x","0,1,2,length"]],"ranges":["0|1|2|1|2","1|2|3|4|4","0|z|z|3","0|1|2"],"holes":[[true,"d",false,null,true,"b",false,null],[true,"a",false,null,true,"e",false,null,true,"e"],[false,true,"x",true,"x",false,"1,2"]],"proto":[[true,"d",true,"p2",true,"b",true,"p0"],[true,"p0",true,"p0",true,"b",true,"p2"]],"errors":["TypeError","TypeError","TypeError"],"override":["override:7"]}"#
        );
    }

    #[test]
    fn array_allocation_species_manifest_rows_are_generated_checked_and_fast_path_ineligible() {
        for row in [ARRAY_SLICE, ARRAY_CONCAT] {
            assert!(MANIFEST_ROWS
                .iter()
                .any(|candidate| candidate.api == row.api));
            assert_eq!(row.receiver, NativeApiReceiver::Array);
            assert_eq!(row.kind, NativeApiKind::Method);
            assert!(row.registration.writable);
            assert!(!row.registration.enumerable);
            assert!(row.registration.configurable);
            assert!(!row.registration.constructor);
            assert!(!row.registration.function_prototype);
            assert_eq!(row.validation.receiver, NativeApiReceiver::Array);
            assert_eq!(row.validation.effect, NativeApiEffect::Pure);
            assert_eq!(row.validation.argument_shape, "array-allocation-species");
            assert_eq!(row.validation.receiver_semantics, "array-like");
            assert!(row.ihi.is_none(), "{} unexpectedly has IHI facts", row.api);
            assert!(
                row.jit_ic.is_none(),
                "{} unexpectedly has JIT IC facts",
                row.api
            );
            assert!(
                row.lejit_expectation.is_none(),
                "{} unexpectedly has LeJIT facts",
                row.api
            );
            assert!(row.fixtures.contains(&"consumer-ineligible"));
        }

        assert_eq!(ARRAY_SLICE.arity, 2);
        assert_eq!(
            ARRAY_SLICE.args,
            &[NativeApiDomain::Number, NativeApiDomain::Number]
        );
        assert_eq!(
            ARRAY_SLICE.validation.index_coercion,
            NativeApiIndexCoercion::ToIntegerOrInfinity
        );
        assert_eq!(
            ARRAY_SLICE.validation.negative_or_infinite_result,
            NativeApiExceptionalResult::ClampedIndex
        );
        assert_eq!(ARRAY_SLICE.validation.range_window, "start-end-copy");
        assert_eq!(ARRAY_CONCAT.arity, 1);
        assert_eq!(ARRAY_CONCAT.args, &[NativeApiDomain::Unknown]);
        assert_eq!(ARRAY_CONCAT.validation.range_window, "concat-spread");
        assert_eq!(
            crate::native_api_manifest_generated::array_slice_generated_ihi_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::array_concat_generated_jit_ic_spec(),
            None
        );
    }

    #[test]
    fn array_allocation_species_manifest_matches_live_registration_descriptors() {
        let mut rt = Runtime::new();
        rt.install_intrinsics();
        let array_ctor = match rt.global_get("Array") {
            Value::Object(id) => id,
            other => panic!("expected global Array constructor, got {other:?}"),
        };
        let array_proto = match rt.object_get(array_ctor, "prototype") {
            Value::Object(id) => id,
            other => panic!("expected Array.prototype object, got {other:?}"),
        };

        for row in [ARRAY_SLICE, ARRAY_CONCAT] {
            let desc = rt
                .obj(array_proto)
                .get_own(row.property)
                .unwrap_or_else(|| panic!("missing {} descriptor", row.api));
            assert_eq!(desc.writable, row.registration.writable, "{}", row.api);
            assert_eq!(desc.enumerable, row.registration.enumerable, "{}", row.api);
            assert_eq!(
                desc.configurable, row.registration.configurable,
                "{}",
                row.api
            );
            let fn_id = match rt.object_get(array_proto, row.property) {
                Value::Object(id) => id,
                other => panic!("expected {} function, got {other:?}", row.api),
            };
            assert_eq!(
                rt.object_get(fn_id, "name"),
                string_value(row.registration.display_name),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "length"),
                Value::Number(row.registration.length as f64),
                "{}",
                row.api
            );
            assert_eq!(
                rt.object_get(fn_id, "prototype"),
                Value::Undefined,
                "{}",
                row.api
            );
        }
    }

    #[test]
    fn array_allocation_species_runtime_shape_and_semantics_match_node_oracle() {
        let actual = run_manifest_json(
            r#"
            (() => {
              const desc = (name) => {
                const f = Array.prototype[name];
                const d = Object.getOwnPropertyDescriptor(Array.prototype, name);
                const ctor = (() => { try { new f(); return 'ok'; } catch (e) { return e.name; } })();
                return [name, f.length, d.writable, d.enumerable, d.configurable, ctor, Object.prototype.hasOwnProperty.call(f, 'prototype')];
              };
              const keys = (o) => Object.keys(o).sort().join(',');
              return JSON.stringify({
                shape: ['slice','concat'].map(desc),
                basic: (() => {
                  const a = [1,2,3,4];
                  const s = a.slice(1,3);
                  const c = [1].concat([2,3], 4);
                  return [s !== a, s.length, s.join('|'), c.length, c.join('|')];
                })(),
                arrayLike: (() => {
                  const o = {length:4,0:'a',2:'c',3:'d'};
                  const s = Array.prototype.slice.call(o,1,4);
                  const c = Array.prototype.concat.call(o, ['x']);
                  return [[s.length, 0 in s, s[0], 1 in s, s[1], 2 in s, s[2], keys(s)], [c.length, c[0] === o, Array.isArray(c[1]), c[1][0], keys(c)]];
                })(),
                ranges: (() => {
                  const a = [0,1,2,3,4];
                  return [a.slice(-4,-1).join('|'), a.slice(NaN, Infinity).join('|'), a.slice(3,1).length, a.slice(-Infinity,2).join('|')];
                })(),
                holes: (() => {
                  const a = [,'b',,'d'];
                  const s = a.slice(0,4);
                  const c = [].concat(a);
                  return [[0 in s,s[0],1 in s,s[1],2 in s,s[2],3 in s,s[3],keys(s)], [0 in c,c[0],1 in c,c[1],2 in c,c[2],3 in c,c[3],keys(c)]];
                })(),
                proto: (() => {
                  Array.prototype[0] = 'p0'; Array.prototype[2] = 'p2';
                  try {
                    const a = [, 'b', , 'd'];
                    const s = a.slice(0,3);
                    const c = [].concat(a);
                    return [[0 in s,s[0],1 in s,s[1],2 in s,s[2],keys(s)], [0 in c,c[0],1 in c,c[1],2 in c,c[2],3 in c,c[3],keys(c)]];
                  } finally { delete Array.prototype[0]; delete Array.prototype[2]; }
                })(),
                spreadable: (() => {
                  const o = {length:2,0:'a',1:'b',[Symbol.isConcatSpreadable]:true};
                  const p = ['x']; p[Symbol.isConcatSpreadable] = false;
                  const r = [].concat(o,p);
                  return [r.length, r[0], r[1], r[2] === p];
                })(),
                species: (() => {
                  let called = 0;
                  class Sub extends Array { static get [Symbol.species]() { called++; return Array; } }
                  const s = new Sub(1,2,3).slice(1);
                  const c = new Sub(1).concat([2]);
                  return [called, s instanceof Array, s instanceof Sub, c instanceof Array, c instanceof Sub];
                })(),
                errors: ['slice','concat'].map((m) => {
                  try { Array.prototype[m].call(null, 1); return 'ok'; } catch(e) { return e.name; }
                }),
                override: (() => {
                  const old = Array.prototype.slice;
                  try { Array.prototype.slice = function(){ return 'override'; }; return [[1,2].slice(0)]; }
                  finally { Array.prototype.slice = old; }
                })()
              });
            })()
            "#,
        );
        assert_eq!(
            actual,
            r#"{"shape":[["slice",2,true,false,true,"TypeError",false],["concat",1,true,false,true,"TypeError",false]],"basic":[true,2,"2|3",4,"1|2|3|4"],"arrayLike":[[3,false,null,true,"c",true,"d","1,2"],[2,true,false,"x","0,1"]],"ranges":["1|2|3","0|1|2|3|4",0,"0|1"],"holes":[[false,null,true,"b",false,null,true,"d","1,3"],[false,null,true,"b",false,null,true,"d","1,3"]],"proto":[[true,"p0",true,"b",true,"p2","0,1,2"],[true,"p0",true,"b",true,"p2",true,"d","0,1,2,3"]],"spreadable":[3,"a","b",true],"species":[2,true,false,true,false],"errors":["TypeError","TypeError"],"override":["override"]}"#
        );
    }

    #[test]
    fn array_splice_manifest_row_is_generated_checked_and_fast_path_ineligible() {
        let row = ARRAY_SPLICE;
        assert!(MANIFEST_ROWS
            .iter()
            .any(|candidate| candidate.api == row.api));
        assert_eq!(row.receiver, NativeApiReceiver::Array);
        assert_eq!(row.kind, NativeApiKind::Method);
        assert_eq!(row.arity, 2);
        assert_eq!(
            row.args,
            &[NativeApiDomain::Number, NativeApiDomain::Number]
        );
        assert_eq!(row.returns, &[NativeApiDomain::Unknown]);
        assert!(row.registration.writable);
        assert!(!row.registration.enumerable);
        assert!(row.registration.configurable);
        assert!(!row.registration.constructor);
        assert!(!row.registration.function_prototype);
        assert_eq!(row.validation.receiver, NativeApiReceiver::Array);
        assert_eq!(row.validation.effect, NativeApiEffect::MutatesReceiverBytes);
        assert_eq!(row.validation.argument_shape, "array-structural-edit");
        assert_eq!(row.validation.receiver_semantics, "array-like");
        assert_eq!(
            row.validation.index_coercion,
            NativeApiIndexCoercion::ToIntegerOrInfinity
        );
        assert_eq!(
            row.validation.negative_or_infinite_result,
            NativeApiExceptionalResult::ClampedIndex
        );
        assert_eq!(row.validation.range_window, "start-delete-insert");
        assert!(row.ihi.is_none(), "{} unexpectedly has IHI facts", row.api);
        assert!(
            row.jit_ic.is_none(),
            "{} unexpectedly has JIT IC facts",
            row.api
        );
        assert!(
            row.lejit_expectation.is_none(),
            "{} unexpectedly has LeJIT facts",
            row.api
        );
        assert!(row.fixtures.contains(&"consumer-ineligible"));
        assert_eq!(
            crate::native_api_manifest_generated::array_splice_generated_ihi_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::array_splice_generated_jit_ic_spec(),
            None
        );
    }

    #[test]
    fn array_splice_manifest_matches_live_registration_descriptor() {
        let mut rt = Runtime::new();
        rt.install_intrinsics();
        let array_ctor = match rt.global_get("Array") {
            Value::Object(id) => id,
            other => panic!("expected global Array constructor, got {other:?}"),
        };
        let array_proto = match rt.object_get(array_ctor, "prototype") {
            Value::Object(id) => id,
            other => panic!("expected Array.prototype object, got {other:?}"),
        };

        let row = ARRAY_SPLICE;
        let desc = rt
            .obj(array_proto)
            .get_own(row.property)
            .unwrap_or_else(|| panic!("missing {} descriptor", row.api));
        assert_eq!(desc.writable, row.registration.writable);
        assert_eq!(desc.enumerable, row.registration.enumerable);
        assert_eq!(desc.configurable, row.registration.configurable);
        let fn_id = match rt.object_get(array_proto, row.property) {
            Value::Object(id) => id,
            other => panic!("expected {} function, got {other:?}", row.api),
        };
        assert_eq!(
            rt.object_get(fn_id, "name"),
            string_value(row.registration.display_name)
        );
        assert_eq!(
            rt.object_get(fn_id, "length"),
            Value::Number(row.registration.length as f64)
        );
        assert_eq!(rt.object_get(fn_id, "prototype"), Value::Undefined);
    }

    #[test]
    fn array_splice_runtime_shape_and_semantics_match_node_oracle() {
        let actual = run_manifest_json(
            r#"
            (() => {
              const own = (o, k) => Object.prototype.hasOwnProperty.call(o, k);
              const keys = (o) => Object.keys(o).join(',');
              const f = Array.prototype.splice;
              const d = Object.getOwnPropertyDescriptor(Array.prototype, 'splice');
              const ctor = (() => { try { new f(); return 'ok'; } catch (e) { return e.name; } })();
              return JSON.stringify({
                shape: [['splice', typeof f, f.length, d.writable, d.enumerable, d.configurable, ctor, Object.prototype.hasOwnProperty.call(f, 'prototype'), f.name]],
                basic: (() => {
                  const a = [0,1,2,3,4];
                  const r = a.splice(1,2,'x','y','z');
                  return [r instanceof Array, r.length, r.join('|'), a.length, a.join('|')];
                })(),
                ranges: (() => {
                  const b = [0,1,2,3];
                  const tail = b.splice(-2);
                  return [tail.join('|'), b.join('|'), [0,1,2].splice(1, Infinity, 'q').join('|')];
                })(),
                arrayLike: (() => {
                  const o = {length:4,0:'a',2:'c'};
                  const r = Array.prototype.splice.call(o,1,2,'x','y','z');
                  return [o.length,own(o,0),o[0],own(o,1),o[1],own(o,2),o[2],own(o,3),o[3],own(o,4),o[4],r.length,own(r,0),r[0],own(r,1),r[1],keys(o),keys(r)];
                })(),
                holes: (() => {
                  const a = [0,,2,,4];
                  const r = a.splice(1,3,'x');
                  return [a.length,own(a,0),a[0],own(a,1),a[1],own(a,2),a[2],keys(a),r.length,own(r,0),r[0],own(r,1),r[1],own(r,2),r[2],keys(r)];
                })(),
                proto: (() => {
                  const o = {length:4,1:'own'};
                  Object.setPrototypeOf(o, {0:'p0',2:'p2'});
                  const r = Array.prototype.splice.call(o,0,3);
                  return [o.length,own(r,0),r[0],own(r,1),r[1],own(r,2),r[2],keys(r),own(o,0),o[0],own(o,1),o[1],keys(o)];
                })(),
                species: (() => {
                  class SpeciesArray extends Array {
                    static get [Symbol.species]() {
                      return function(len) { const out = []; out.createdLength = len; return out; };
                    }
                  }
                  const source = new SpeciesArray(1,2,3);
                  const r = source.splice(1,1);
                  return [r.createdLength, r.length, r[0], r instanceof SpeciesArray, source instanceof SpeciesArray];
                })(),
                errors: (() => {
                  const out = [];
                  for (const value of [null, undefined]) {
                    try { Array.prototype.splice.call(value,0,1); } catch(e) { out.push(e.name); }
                  }
                  try { Array.prototype.splice.call({length:Number.MAX_SAFE_INTEGER,0:'a'},0,0,'x'); } catch(e) { out.push(e.name); }
                  return out;
                })(),
                override: (() => {
                  const old = Array.prototype.splice;
                  try { Array.prototype.splice = function(){ return 'override'; }; return [[1,2,3].splice(0,1)]; }
                  finally { Array.prototype.splice = old; }
                })()
              });
            })()
            "#,
        );
        assert_eq!(
            actual,
            r#"{"shape":[["splice","function",2,true,false,true,"TypeError",false,"splice"]],"basic":[true,2,"1|2",6,"0|x|y|z|3|4"],"ranges":["2|3","0|1","1|2"],"arrayLike":[5,true,"a",true,"x",true,"y",true,"z",false,null,2,false,null,true,"c","0,1,2,3,length","1"],"holes":[3,true,0,true,"x",true,4,"0,1,2",3,false,null,true,2,false,null,"1"],"proto":[1,true,"p0",true,"own",true,"p2","0,1,2",false,"p0",false,null,"length"],"species":[1,1,2,false,true],"errors":["TypeError","TypeError","TypeError"],"override":["override"]}"#
        );
    }

    #[test]
    fn array_sort_manifest_row_is_generated_checked_and_fast_path_ineligible() {
        let row = ARRAY_SORT;
        assert!(MANIFEST_ROWS
            .iter()
            .any(|candidate| candidate.api == row.api));
        assert_eq!(row.receiver, NativeApiReceiver::Array);
        assert_eq!(row.kind, NativeApiKind::Method);
        assert_eq!(row.arity, 1);
        assert_eq!(row.args, &[NativeApiDomain::Unknown]);
        assert_eq!(row.returns, &[NativeApiDomain::Unknown]);
        assert!(row.registration.writable);
        assert!(!row.registration.enumerable);
        assert!(row.registration.configurable);
        assert!(!row.registration.constructor);
        assert!(!row.registration.function_prototype);
        assert_eq!(row.validation.receiver, NativeApiReceiver::Array);
        assert_eq!(row.validation.effect, NativeApiEffect::MutatesReceiverBytes);
        assert_eq!(row.validation.argument_shape, "array-sort-comparator");
        assert_eq!(row.validation.receiver_semantics, "array-like");
        assert_eq!(row.validation.callback_policy, "sort-compare");
        assert_eq!(row.validation.range_window, "whole-length-sort");
        assert!(row.ihi.is_none(), "{} unexpectedly has IHI facts", row.api);
        assert!(
            row.jit_ic.is_none(),
            "{} unexpectedly has JIT IC facts",
            row.api
        );
        assert!(
            row.lejit_expectation.is_none(),
            "{} unexpectedly has LeJIT facts",
            row.api
        );
        assert!(row.fixtures.contains(&"consumer-ineligible"));
        assert_eq!(
            crate::native_api_manifest_generated::array_sort_generated_ihi_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::array_sort_generated_jit_ic_spec(),
            None
        );
    }

    #[test]
    fn array_sort_manifest_matches_live_registration_descriptor() {
        let mut rt = Runtime::new();
        rt.install_intrinsics();
        let array_ctor = match rt.global_get("Array") {
            Value::Object(id) => id,
            other => panic!("expected global Array constructor, got {other:?}"),
        };
        let array_proto = match rt.object_get(array_ctor, "prototype") {
            Value::Object(id) => id,
            other => panic!("expected Array.prototype object, got {other:?}"),
        };

        let row = ARRAY_SORT;
        let desc = rt
            .obj(array_proto)
            .get_own(row.property)
            .unwrap_or_else(|| panic!("missing {} descriptor", row.api));
        assert_eq!(desc.writable, row.registration.writable);
        assert_eq!(desc.enumerable, row.registration.enumerable);
        assert_eq!(desc.configurable, row.registration.configurable);
        let fn_id = match rt.object_get(array_proto, row.property) {
            Value::Object(id) => id,
            other => panic!("expected {} function, got {other:?}", row.api),
        };
        assert_eq!(
            rt.object_get(fn_id, "name"),
            string_value(row.registration.display_name)
        );
        assert_eq!(
            rt.object_get(fn_id, "length"),
            Value::Number(row.registration.length as f64)
        );
        assert_eq!(rt.object_get(fn_id, "prototype"), Value::Undefined);
    }

    #[test]
    fn array_sort_runtime_shape_and_semantics_match_node_oracle() {
        let actual = run_manifest_json(
            r#"
            (() => {
              const own = (o, k) => Object.prototype.hasOwnProperty.call(o, k);
              const keys = (o) => Object.keys(o).join(',');
              const f = Array.prototype.sort;
              const d = Object.getOwnPropertyDescriptor(Array.prototype, 'sort');
              const ctor = (() => { try { new f(); return 'ok'; } catch (e) { return e.name; } })();
              return JSON.stringify({
                shape: [['sort', typeof f, f.length, d.writable, d.enumerable, d.configurable, ctor, Object.prototype.hasOwnProperty.call(f, 'prototype'), f.name]],
                basic: (() => {
                  const a = [3,11,2,1];
                  const r = a.sort();
                  return [r === a, a.join('|')];
                })(),
                comparator: (() => {
                  const a = [3,11,2,1];
                  let calls = 0;
                  const r = a.sort((x, y) => { calls++; return x - y; });
                  return [r === a, a.join('|'), calls > 0];
                })(),
                stability: (() => {
                  const a = [{k:1,v:'a'}, {k:0,v:'b'}, {k:1,v:'c'}, {k:0,v:'d'}];
                  a.sort((x, y) => x.k - y.k);
                  return a.map((x) => x.v).join('');
                })(),
                holes: (() => {
                  const a = [3,,1,undefined,2,,undefined];
                  a.sort();
                  return [a.length,own(a,0),a[0],own(a,1),a[1],own(a,2),a[2],own(a,3),a[3],own(a,4),a[4],own(a,5),a[5],own(a,6),a[6],keys(a)];
                })(),
                arrayLike: (() => {
                  const o = {length:4,0:'b',2:'a'};
                  const r = Array.prototype.sort.call(o);
                  return [r === o,o.length,own(o,0),o[0],own(o,1),o[1],own(o,2),o[2],own(o,3),o[3],keys(o)];
                })(),
                proto: (() => {
                  const o = {length:4,1:'own'};
                  Object.setPrototypeOf(o, {0:'p0',2:'p2'});
                  Array.prototype.sort.call(o);
                  return [o.length,own(o,0),o[0],own(o,1),o[1],own(o,2),o[2],own(o,3),o[3],keys(o)];
                })(),
                undefinedOrder: (() => {
                  const a = [undefined,'b',undefined,'a'];
                  a.sort();
                  return a;
                })(),
                errors: (() => {
                  const out = [];
                  for (const value of [null, undefined]) {
                    try { Array.prototype.sort.call(value); } catch(e) { out.push(e.name); }
                  }
                  try { [1,2].sort(1); } catch(e) { out.push(e.name); }
                  try { [1,2].sort(() => Symbol('bad')); } catch(e) { out.push(e.name); }
                  return out;
                })(),
                override: (() => {
                  const old = Array.prototype.sort;
                  try { Array.prototype.sort = function(){ return 'override'; }; return [[2,1].sort()]; }
                  finally { Array.prototype.sort = old; }
                })()
              });
            })()
            "#,
        );
        assert_eq!(
            actual,
            r#"{"shape":[["sort","function",1,true,false,true,"TypeError",false,"sort"]],"basic":[true,"1|11|2|3"],"comparator":[true,"1|2|3|11",true],"stability":"bdac","holes":[7,true,1,true,2,true,3,true,null,true,null,false,null,false,null,"0,1,2,3,4"],"arrayLike":[true,4,true,"a",true,"b",false,null,false,null,"0,1,length"],"proto":[4,true,"own",true,"p0",true,"p2",false,null,"0,1,2,length"],"undefinedOrder":["a","b",null,null],"errors":["TypeError","TypeError","TypeError","TypeError"],"override":["override"]}"#
        );
    }

    #[test]
    fn array_flat_manifest_row_is_generated_checked_and_fast_path_ineligible() {
        let row = ARRAY_FLAT;
        assert!(MANIFEST_ROWS
            .iter()
            .any(|candidate| candidate.api == row.api));
        assert_eq!(row.receiver, NativeApiReceiver::Array);
        assert_eq!(row.kind, NativeApiKind::Method);
        assert_eq!(row.arity, 0);
        assert_eq!(row.args, &[]);
        assert_eq!(row.returns, &[NativeApiDomain::Unknown]);
        assert!(row.registration.writable);
        assert!(!row.registration.enumerable);
        assert!(row.registration.configurable);
        assert!(!row.registration.constructor);
        assert!(!row.registration.function_prototype);
        assert_eq!(row.validation.receiver, NativeApiReceiver::Array);
        assert_eq!(row.validation.effect, NativeApiEffect::Pure);
        assert_eq!(row.validation.argument_shape, "array-flattening");
        assert_eq!(row.validation.receiver_semantics, "array-like");
        assert_eq!(
            row.validation.index_coercion,
            NativeApiIndexCoercion::ToIntegerOrInfinity
        );
        assert_eq!(
            row.validation.negative_or_infinite_result,
            NativeApiExceptionalResult::ClampedIndex
        );
        assert_eq!(row.validation.range_window, "depth-flatten");
        assert!(row.ihi.is_none(), "{} unexpectedly has IHI facts", row.api);
        assert!(
            row.jit_ic.is_none(),
            "{} unexpectedly has JIT IC facts",
            row.api
        );
        assert!(
            row.lejit_expectation.is_none(),
            "{} unexpectedly has LeJIT facts",
            row.api
        );
        assert!(row.fixtures.contains(&"consumer-ineligible"));
        assert_eq!(
            crate::native_api_manifest_generated::array_flat_generated_ihi_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::array_flat_generated_jit_ic_spec(),
            None
        );
    }

    #[test]
    fn array_flat_manifest_matches_live_registration_descriptor() {
        let mut rt = Runtime::new();
        rt.install_intrinsics();
        let array_ctor = match rt.global_get("Array") {
            Value::Object(id) => id,
            other => panic!("expected global Array constructor, got {other:?}"),
        };
        let array_proto = match rt.object_get(array_ctor, "prototype") {
            Value::Object(id) => id,
            other => panic!("expected Array.prototype object, got {other:?}"),
        };

        let row = ARRAY_FLAT;
        let desc = rt
            .obj(array_proto)
            .get_own(row.property)
            .unwrap_or_else(|| panic!("missing {} descriptor", row.api));
        assert_eq!(desc.writable, row.registration.writable);
        assert_eq!(desc.enumerable, row.registration.enumerable);
        assert_eq!(desc.configurable, row.registration.configurable);
        let fn_id = match rt.object_get(array_proto, row.property) {
            Value::Object(id) => id,
            other => panic!("expected {} function, got {other:?}", row.api),
        };
        assert_eq!(
            rt.object_get(fn_id, "name"),
            string_value(row.registration.display_name)
        );
        assert_eq!(
            rt.object_get(fn_id, "length"),
            Value::Number(row.registration.length as f64)
        );
        assert_eq!(rt.object_get(fn_id, "prototype"), Value::Undefined);
    }

    #[test]
    fn array_flat_runtime_shape_and_semantics_match_node_oracle() {
        let actual = run_manifest_json(
            r#"
            (() => {
              const own = (o, k) => Object.prototype.hasOwnProperty.call(o, k);
              const keys = (o) => Object.keys(o).join(',');
              const f = Array.prototype.flat;
              const d = Object.getOwnPropertyDescriptor(Array.prototype, 'flat');
              const ctor = (() => { try { new f(); return 'ok'; } catch (e) { return e.name; } })();
              return JSON.stringify({
                shape: [['flat', typeof f, f.length, d.writable, d.enumerable, d.configurable, ctor, Object.prototype.hasOwnProperty.call(f, 'prototype'), f.name]],
                basic: (() => {
                  const a = [1,[2,[3,[4]]],5];
                  return [a.flat().join('|'), a.flat(2).join('|'), a.flat(Infinity).join('|'), a.flat(0).map((x) => Array.isArray(x) ? x.join(',') : x).join('|')];
                })(),
                depth: (() => {
                  return [[1,[2]].flat(undefined).join('|'), [1,[2,[3]]].flat('2').join('|'), [1,[2]].flat(-1).map((x) => Array.isArray(x) ? x.join(',') : x).join('|'), [1,[2]].flat(NaN).map((x) => Array.isArray(x) ? x.join(',') : x).join('|')];
                })(),
                holes: (() => {
                  const a = [1,,[2,,3],,[4,[5,,6]]];
                  const b = a.flat(1);
                  const c = a.flat(2);
                  return [b.length,keys(b),own(b,0),b[0],own(b,1),b[1],own(b,2),b[2],own(b,3),b[3],own(b,4),Array.isArray(b[4]),b[4] && b[4].length,c.length,keys(c),c.join('|')];
                })(),
                arrayLike: (() => {
                  const o = {length:3,0:['a'],2:['c',['d']]};
                  const r = Array.prototype.flat.call(o, 2);
                  return [r.length, r.join('|'), keys(r)];
                })(),
                proto: (() => {
                  const o = {length:3,1:['own']};
                  Object.setPrototypeOf(o, {0:['p0'],2:['p2',['p3']]});
                  const r = Array.prototype.flat.call(o, 2);
                  return [r.length, r.join('|'), keys(r)];
                })(),
                species: (() => {
                  class SpeciesArray extends Array {
                    static get [Symbol.species]() {
                      return function(len) { const out = []; out.createdLength = len; return out; };
                    }
                  }
                  const source = new SpeciesArray(1,[2]);
                  const r = source.flat();
                  return [r.createdLength, r.length, r.join('|'), r instanceof SpeciesArray, source instanceof SpeciesArray];
                })(),
                errors: (() => {
                  const out = [];
                  for (const value of [null, undefined]) {
                    try { Array.prototype.flat.call(value); } catch(e) { out.push(e.name); }
                  }
                  return out;
                })(),
                override: (() => {
                  const old = Array.prototype.flat;
                  try { Array.prototype.flat = function(){ return 'override'; }; return [[1,[2]].flat()]; }
                  finally { Array.prototype.flat = old; }
                })()
              });
            })()
            "#,
        );
        assert_eq!(
            actual,
            r#"{"shape":[["flat","function",0,true,false,true,"TypeError",false,"flat"]],"basic":["1|2|3,4|5","1|2|3|4|5","1|2|3|4|5","1|2,3,4|5"],"depth":["1|2","1|2|3","1|2","1|2"],"holes":[5,"0,1,2,3,4",true,1,true,2,true,3,true,4,true,true,3,6,"0,1,2,3,4,5","1|2|3|4|5|6"],"arrayLike":[3,"a|c|d","0,1,2"],"proto":[4,"p0|own|p2|p3","0,1,2,3"],"species":[0,2,"1|2",false,true],"errors":["TypeError","TypeError"],"override":["override"]}"#
        );
    }

    #[test]
    fn array_scalar_search_runtime_shape_and_semantics_match_node_oracle() {
        let actual = run_manifest_json(
            r#"
            JSON.stringify({
              shape: [
                ['isArray', Array.isArray.length,
                  Object.getOwnPropertyDescriptor(Array, 'isArray').writable,
                  Object.getOwnPropertyDescriptor(Array, 'isArray').enumerable,
                  Object.getOwnPropertyDescriptor(Array, 'isArray').configurable],
              ].concat(['at','includes','indexOf','lastIndexOf','find','findIndex','findLast','findLastIndex'].map((m) => {
                const f = Array.prototype[m];
                const d = Object.getOwnPropertyDescriptor(Array.prototype, m);
                const pd = Object.getOwnPropertyDescriptor(f, 'prototype');
                const ctor = (() => { try { new f(); return 'ok'; } catch (e) { return e.name; } })();
                return [m, f.length, d.writable, d.enumerable, d.configurable, ctor, !!pd];
              })),
              scalar: (() => {
                const like = { length: 2, 0: 'a', 1: NaN };
                const sparse = [, NaN, 0, -0, 'x'];
                Object.defineProperty(Array.prototype, '0', { get() { return 'proto'; }, configurable: true });
                const holeFacts = [sparse.includes('proto'), sparse.indexOf('proto'), sparse.includes(NaN), sparse.indexOf(NaN)];
                delete Array.prototype[0];
                return [
                  Array.prototype.at.call(like, 0),
                  Array.prototype.includes.call(like, NaN),
                  Array.prototype.indexOf.call(like, 'a'),
                  Array.prototype.lastIndexOf.call(like, NaN),
                  [1,2].at(Infinity),
                  [1,2].at(-Infinity),
                  [1,2].at(NaN),
                  [1,2].includes(1, Infinity),
                  [1,2].includes(1, -Infinity),
                  [1,2].indexOf(1, Infinity),
                  [1,2].indexOf(1, -Infinity),
                  [1,2].lastIndexOf(2, Infinity),
                  [1,2].lastIndexOf(2, -Infinity),
                  holeFacts
                ];
              })(),
              callbacks: (() => {
                const like = { length: 2, 0: 'a', 1: NaN };
                const b = [, 1, , 3];
                const ctx = { tag: 42 };
                return [
                  Array.prototype.find.call(like, (v, i, o) => v === 'a' && i === 0 && o === like),
                  Array.prototype.findIndex.call(like, (v, i, o) => Number.isNaN(v) && i === 1 && o === like),
                  Array.prototype.findLast.call(like, (v) => v === 'a'),
                  Array.prototype.findLastIndex.call(like, (v) => v === 'a'),
                  b.find((v, i, o) => { if (i === 0) o[2] = 2; return v === 2; }),
                  b.findIndex((v, i, o) => { if (i === 0) o[2] = 2; return v === 2; }),
                  b.findLast((v, i, o) => { if (i === 3) o[1] = 9; return v === 9; }),
                  b.findLastIndex((v, i, o) => { if (i === 3) o[1] = 9; return v === 9; }),
                  [1].find(function(v, i, o) { return this === ctx && v === 1 && i === 0 && o.length === 1; }, ctx)
                ];
              })()
            })
            "#,
        );
        assert_eq!(
            actual,
            r#"{"shape":[["isArray",1,true,false,true],["at",1,true,false,true,"TypeError",false],["includes",1,true,false,true,"TypeError",false],["indexOf",1,true,false,true,"TypeError",false],["lastIndexOf",1,true,false,true,"TypeError",false],["find",1,true,false,true,"TypeError",false],["findIndex",1,true,false,true,"TypeError",false],["findLast",1,true,false,true,"TypeError",false],["findLastIndex",1,true,false,true,"TypeError",false]],"scalar":["a",true,0,-1,null,null,1,false,true,-1,0,1,-1,[true,0,true,-1]],"callbacks":["a",1,"a",0,2,2,9,1,1]}"#
        );
    }

    #[test]
    fn array_iteration_callback_runtime_shape_and_semantics_match_node_oracle() {
        let actual = run_manifest_json(
            r#"
            JSON.stringify({
              shape: ['forEach','map','filter','some','every','reduce','reduceRight','flatMap'].map((m) => {
                const f = Array.prototype[m];
                const d = Object.getOwnPropertyDescriptor(Array.prototype, m);
                const pd = Object.getOwnPropertyDescriptor(f, 'prototype');
                const ctor = (() => { try { new f(); return 'ok'; } catch (e) { return e.name; } })();
                return [m, f.length, d.writable, d.enumerable, d.configurable, ctor, !!pd];
              }),
              iteration: (() => {
                const ctx = { tag: 7 };
                const sparse = [, 'a', , 'b'];
                const calls = [];
                const forEachReturn = sparse.forEach(function(v, i, o) {
                  calls.push([v, i, this === ctx, o === sparse]);
                  if (i === 1) o[2] = 'm';
                }, ctx);
                const mapOut = sparse.map(function(v, i, o) {
                  return v + ':' + i + ':' + (this === ctx) + ':' + (o === sparse);
                }, ctx);
                const filterOut = sparse.filter((v, i, o) => {
                  if (i === 1) o[0] = 'late';
                  return v !== 'b';
                });
                const someOut = sparse.some((v, i, o) => {
                  if (i === 1) o[3] = 'seen';
                  return v === 'seen';
                });
                const everyOut = sparse.every((v) => typeof v === 'string');
                return [
                  forEachReturn,
                  calls,
                  mapOut.length,
                  Object.prototype.hasOwnProperty.call(mapOut, '0'),
                  mapOut[1],
                  mapOut[2],
                  mapOut[3],
                  filterOut.join('|'),
                  someOut,
                  everyOut
                ];
              })(),
              reduction: (() => {
                const sparse = [, 2, , 4];
                const reduceCalls = [];
                const sum = sparse.reduce(function(acc, v, i, o) {
                  reduceCalls.push([acc, v, i, o === sparse]);
                  return acc + v;
                });
                const seeded = sparse.reduce((acc, v, i) => acc + ':' + v + '@' + i, 's');
                const rightCalls = [];
                const right = sparse.reduceRight(function(acc, v, i, o) {
                  rightCalls.push([acc, v, i, o === sparse]);
                  return acc - v;
                });
                const emptyErr = (() => { try { [].reduce(() => 0); return 'ok'; } catch (e) { return e.name; } })();
                const cbErr = (() => { try { [1].reduce(null); return 'ok'; } catch (e) { return e.name; } })();
                return [sum, reduceCalls, seeded, right, rightCalls, emptyErr, cbErr];
              })(),
              flatMap: (() => {
                const ctx = { tag: 9 };
                const source = [1, , 3];
                const seen = [];
                const out = source.flatMap(function(v, i, o) {
                  seen.push([v, i, this === ctx, o === source]);
                  if (i === 0) o[1] = 2;
                  return [v, [i]];
                }, ctx);
                const arrayLike = Array.prototype.flatMap.call({ length: 2, 0: 'x', 1: 'y' }, (v, i) => [v, i]);
                return [seen, out.length, out[0], Array.isArray(out[1]), out[1][0], out[2], out[3][0], arrayLike.join('|')];
              })()
            })
            "#,
        );
        assert_eq!(
            actual,
            r#"{"shape":[["forEach",1,true,false,true,"TypeError",false],["map",1,true,false,true,"TypeError",false],["filter",1,true,false,true,"TypeError",false],["some",1,true,false,true,"TypeError",false],["every",1,true,false,true,"TypeError",false],["reduce",1,true,false,true,"TypeError",false],["reduceRight",1,true,false,true,"TypeError",false],["flatMap",1,true,false,true,"TypeError",false]],"iteration":[null,[["a",1,true,true],["m",2,true,true],["b",3,true,true]],4,false,"a:1:true:true","m:2:true:true","b:3:true:true","a|m",true,true],"reduction":[6,[[2,4,3,true]],"s:2@1:4@3",2,[[4,2,1,true]],"TypeError","TypeError"],"flatMap":[[[1,0,true,true],[2,1,true,true],[3,2,true,true]],6,1,true,0,2,1,"x|0|y|1"]}"#
        );
    }

    #[test]
    fn typedarray_scalar_search_manifest_rows_are_generated_checked_and_fast_path_ineligible() {
        for row in [
            TYPEDARRAY_AT,
            TYPEDARRAY_INCLUDES,
            TYPEDARRAY_INDEX_OF,
            TYPEDARRAY_LAST_INDEX_OF,
        ] {
            assert_eq!(row.receiver, NativeApiReceiver::TypedArray);
            assert_eq!(row.kind, NativeApiKind::Method);
            assert_eq!(row.registration.property, row.property);
            assert_eq!(row.registration.writable, true);
            assert_eq!(row.registration.enumerable, false);
            assert_eq!(row.registration.configurable, true);
            assert_eq!(row.registration.constructor, false);
            assert_eq!(row.registration.function_prototype, false);
            assert_eq!(row.validation.receiver, NativeApiReceiver::TypedArray);
            assert_eq!(
                row.validation.index_coercion,
                NativeApiIndexCoercion::ToIntegerOrInfinity
            );
            assert_eq!(row.validation.receiver_semantics, "typedarray-brand-detach");
            assert_eq!(row.validation.range_window, "element-index");
            assert_eq!(row.validation.effect, NativeApiEffect::Pure);
            assert!(row
                .validation
                .error_codes
                .contains(&"TypeError:detached-buffer"));
            assert!(row.ihi.is_none(), "{} must not claim IHI", row.api);
            assert!(row.jit_ic.is_none(), "{} must not claim JIT", row.api);
            assert!(
                row.lejit_expectation.is_none(),
                "{} must not claim LeJIT",
                row.api
            );
            assert!(row.fixtures.contains(&"consumer-ineligible"));
            assert!(MANIFEST_ROWS
                .iter()
                .any(|candidate| candidate.api == row.api));
        }

        assert_eq!(
            TYPEDARRAY_AT.validation.negative_or_infinite_result,
            NativeApiExceptionalResult::Undefined
        );
        assert_eq!(
            TYPEDARRAY_AT.validation.out_of_range_result,
            NativeApiExceptionalResult::Undefined
        );
        assert_eq!(TYPEDARRAY_INCLUDES.returns, &[NativeApiDomain::Boolean]);
        assert_eq!(TYPEDARRAY_INDEX_OF.returns, &[NativeApiDomain::Number]);
        assert_eq!(TYPEDARRAY_LAST_INDEX_OF.returns, &[NativeApiDomain::Number]);
        assert_eq!(
            crate::native_api_manifest_generated::typed_array_at_generated_ihi_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::typed_array_includes_generated_jit_ic_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::
                typed_array_last_index_of_generated_lejit_expectation_spec(),
            None
        );
    }

    #[test]
    fn typedarray_scalar_search_runtime_shape_and_semantics_match_node_oracle() {
        let actual = run_manifest_json(
            r#"
            JSON.stringify({
              shape: ['at','includes','indexOf','lastIndexOf'].map((m) => {
                const proto = Object.getPrototypeOf(Uint8Array.prototype);
                const f = proto[m];
                const d = Object.getOwnPropertyDescriptor(proto, m);
                const pd = Object.getOwnPropertyDescriptor(f, 'prototype');
                const ctor = (() => { try { new f(); return 'ok'; } catch (e) { return e.name; } })();
                return [m, f.length, d.writable, d.enumerable, d.configurable, ctor, !!pd];
              }),
              numeric: (() => {
                const u = new Uint8Array([1, 2, 2, 255]);
                const f = new Float64Array([0, NaN, 2]);
                return [
                  u.at(0),
                  u.at(-1),
                  u.at(99),
                  u.includes(2),
                  u.includes(2, -2),
                  u.includes(1, Infinity),
                  f.includes(NaN),
                  f.indexOf(NaN),
                  u.indexOf(2),
                  u.indexOf(2, 2),
                  u.indexOf(2, -2),
                  u.lastIndexOf(2),
                  u.lastIndexOf(2, 1),
                  u.lastIndexOf(2, -3)
                ];
              })(),
              bigint: (() => {
                const b = new BigInt64Array([1n, -2n]);
                return [String(b.at(-1)), b.includes(-2n), b.indexOf(-2n), b.lastIndexOf(1n)];
              })(),
              detached: (() => {
                const ab = new ArrayBuffer(4);
                const u = new Uint8Array(ab);
                structuredClone(ab, { transfer: [ab] });
                return ['at','includes','indexOf','lastIndexOf'].map((m) => {
                  try { u[m](0); return 'ok'; } catch (e) { return e.name; }
                });
              })()
            })
            "#,
        );
        assert_eq!(
            actual,
            r#"{"shape":[["at",1,true,false,true,"TypeError",false],["includes",1,true,false,true,"TypeError",false],["indexOf",1,true,false,true,"TypeError",false],["lastIndexOf",1,true,false,true,"TypeError",false]],"numeric":[1,255,null,true,true,false,true,-1,1,2,2,2,1,1],"bigint":["-2",true,1,0],"detached":["TypeError","TypeError","TypeError","TypeError"]}"#
        );
    }

    #[test]
    fn typedarray_mutation_manifest_rows_are_generated_checked_and_fast_path_ineligible() {
        for row in [
            TYPEDARRAY_FILL,
            TYPEDARRAY_REVERSE,
            TYPEDARRAY_COPY_WITHIN,
            TYPEDARRAY_SET,
        ] {
            assert_eq!(row.receiver, NativeApiReceiver::TypedArray);
            assert_eq!(row.kind, NativeApiKind::Method);
            assert_eq!(row.registration.property, row.property);
            assert_eq!(row.registration.writable, true);
            assert_eq!(row.registration.enumerable, false);
            assert_eq!(row.registration.configurable, true);
            assert_eq!(row.registration.constructor, false);
            assert_eq!(row.registration.function_prototype, false);
            assert_eq!(row.validation.receiver, NativeApiReceiver::TypedArray);
            assert_eq!(row.validation.receiver_semantics, "typedarray-brand-detach");
            assert_eq!(row.validation.range_window, "element-index");
            assert_eq!(row.validation.effect, NativeApiEffect::MutatesReceiverBytes);
            assert!(row
                .validation
                .error_codes
                .contains(&"TypeError:detached-buffer"));
            assert!(row.ihi.is_none(), "{} must not claim IHI", row.api);
            assert!(row.jit_ic.is_none(), "{} must not claim JIT", row.api);
            assert!(
                row.lejit_expectation.is_none(),
                "{} must not claim LeJIT",
                row.api
            );
            assert!(row.fixtures.contains(&"consumer-ineligible"));
            assert!(MANIFEST_ROWS
                .iter()
                .any(|candidate| candidate.api == row.api));
        }

        assert_eq!(TYPEDARRAY_FILL.arity, 1);
        assert_eq!(TYPEDARRAY_REVERSE.arity, 0);
        assert_eq!(TYPEDARRAY_COPY_WITHIN.arity, 2);
        assert_eq!(TYPEDARRAY_SET.returns, &[NativeApiDomain::Undefined]);
        assert_eq!(
            TYPEDARRAY_SET.validation.negative_or_infinite_result,
            NativeApiExceptionalResult::RangeError
        );
        assert_eq!(
            TYPEDARRAY_COPY_WITHIN.validation.index_coercion,
            NativeApiIndexCoercion::ToIntegerOrInfinity
        );
        assert_eq!(
            crate::native_api_manifest_generated::typed_array_fill_generated_ihi_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::typed_array_set_generated_jit_ic_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::
                typed_array_copy_within_generated_lejit_expectation_spec(),
            None
        );
    }

    #[test]
    fn typedarray_mutation_runtime_shape_and_semantics_match_node_oracle() {
        let actual = run_manifest_json(
            r#"
            JSON.stringify({
              shape: ['fill','reverse','copyWithin','set'].map((m) => {
                const proto = Object.getPrototypeOf(Uint8Array.prototype);
                const f = proto[m];
                const d = Object.getOwnPropertyDescriptor(proto, m);
                const pd = Object.getOwnPropertyDescriptor(f, 'prototype');
                const ctor = (() => { try { new f(); return 'ok'; } catch (e) { return e.name; } })();
                return [m, f.length, d.writable, d.enumerable, d.configurable, ctor, !!pd];
              }),
              numeric: (() => {
                const a = new Uint8Array([1,2,3,4]);
                const fillRet = a.fill(9, -3, -1);
                const b = new Uint8Array([1,2,3,4]);
                const reverseRet = b.reverse();
                const c = new Uint8Array([1,2,3,4,5]);
                const copyRet = c.copyWithin(1, 3);
                const d = new Uint8Array([0,0,0,0]);
                const setRet = d.set([7,8], 1);
                const e = new Uint8Array([1,2,3,4]);
                e.set(e.subarray(0,3), 1);
                const rangeErr = (() => { try { d.set([1,2,3], 3); return 'ok'; } catch (err) { return err.name; } })();
                const coercion = new Uint8Array(2);
                coercion.fill(260);
                return [[...a], fillRet === a, [...b], reverseRet === b, [...c], copyRet === c, [...d], setRet, [...e], rangeErr, [...coercion]];
              })(),
              bigint: (() => {
                const b = new BigInt64Array([1n, 2n, 3n]);
                const fillErr = (() => { try { b.fill(1); return 'ok'; } catch (e) { return e.name; } })();
                b.fill(-2n, 1);
                const c = new BigInt64Array([0n,0n,0n]);
                c.set([4n, 5n], 1);
                return [fillErr, Array.from(b, String), Array.from(c, String)];
              })(),
              detached: (() => {
                const ab = new ArrayBuffer(8);
                const u = new Uint8Array(ab);
                structuredClone(ab, { transfer: [ab] });
                return ['fill','reverse','copyWithin','set'].map((m) => {
                  try {
                    if (m === 'set') u[m]([1], 0);
                    else if (m === 'copyWithin') u[m](0, 1);
                    else u[m](1);
                    return 'ok';
                  } catch (e) { return e.name; }
                });
              })()
            })
            "#,
        );
        assert_eq!(
            actual,
            r#"{"shape":[["fill",1,true,false,true,"TypeError",false],["reverse",0,true,false,true,"TypeError",false],["copyWithin",2,true,false,true,"TypeError",false],["set",1,true,false,true,"TypeError",false]],"numeric":[[1,9,9,4],true,[4,3,2,1],true,[1,4,5,4,5],true,[0,7,8,0],null,[1,1,2,3],"RangeError",[4,4]],"bigint":["TypeError",["1","-2","-2"],["0","4","5"]],"detached":["TypeError","TypeError","TypeError","TypeError"]}"#
        );
    }

    #[test]
    fn typedarray_allocation_manifest_rows_are_generated_checked_and_fast_path_ineligible() {
        for row in [
            TYPEDARRAY_SLICE,
            TYPEDARRAY_SUBARRAY,
            TYPEDARRAY_TO_REVERSED,
            TYPEDARRAY_TO_SORTED,
        ] {
            assert_eq!(row.receiver, NativeApiReceiver::TypedArray);
            assert_eq!(row.kind, NativeApiKind::Method);
            assert_eq!(row.registration.property, row.property);
            assert_eq!(row.registration.writable, true);
            assert_eq!(row.registration.enumerable, false);
            assert_eq!(row.registration.configurable, true);
            assert_eq!(row.registration.constructor, false);
            assert_eq!(row.registration.function_prototype, false);
            assert_eq!(row.validation.receiver, NativeApiReceiver::TypedArray);
            assert_eq!(row.validation.receiver_semantics, "typedarray-brand-detach");
            assert_eq!(row.validation.range_window, "element-index");
            assert_eq!(row.validation.effect, NativeApiEffect::Pure);
            assert!(row
                .validation
                .error_codes
                .contains(&"TypeError:detached-buffer"));
            assert!(row.ihi.is_none(), "{} must not claim IHI", row.api);
            assert!(row.jit_ic.is_none(), "{} must not claim JIT", row.api);
            assert!(
                row.lejit_expectation.is_none(),
                "{} must not claim LeJIT",
                row.api
            );
            assert!(row.fixtures.contains(&"consumer-ineligible"));
            assert!(MANIFEST_ROWS
                .iter()
                .any(|candidate| candidate.api == row.api));
        }

        assert_eq!(TYPEDARRAY_SLICE.arity, 2);
        assert_eq!(TYPEDARRAY_SUBARRAY.arity, 2);
        assert_eq!(TYPEDARRAY_TO_REVERSED.arity, 0);
        assert_eq!(TYPEDARRAY_TO_SORTED.arity, 1);
        assert_eq!(
            TYPEDARRAY_SLICE.validation.index_coercion,
            NativeApiIndexCoercion::ToIntegerOrInfinity
        );
        assert_eq!(
            TYPEDARRAY_TO_SORTED.validation.index_coercion,
            NativeApiIndexCoercion::None
        );
        assert_eq!(
            TYPEDARRAY_SLICE.validation.argument_shape,
            "typedarray-start-end-copy"
        );
        assert_eq!(
            TYPEDARRAY_SUBARRAY.validation.argument_shape,
            "typedarray-start-end-view"
        );
        assert_eq!(
            TYPEDARRAY_TO_REVERSED.validation.argument_shape,
            "typedarray-change-by-copy-reverse"
        );
        assert_eq!(
            TYPEDARRAY_TO_SORTED.validation.argument_shape,
            "typedarray-change-by-copy-sort"
        );
        assert!(TYPEDARRAY_SLICE
            .validation
            .error_codes
            .contains(&"TypeError:species-constructor"));
        assert!(TYPEDARRAY_TO_SORTED
            .validation
            .error_codes
            .contains(&"TypeError:callback-not-callable"));
        assert_eq!(
            crate::native_api_manifest_generated::typed_array_slice_generated_ihi_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::typed_array_subarray_generated_jit_ic_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::
                typed_array_to_reversed_generated_lejit_expectation_spec(),
            None
        );
    }

    #[test]
    fn typedarray_allocation_runtime_shape_and_semantics_match_node_oracle() {
        let actual = run_manifest_json(
            r#"
            (() => {
            const callOrName = (fn) => { try { return fn(); } catch(e) { return e.name; } };
            return JSON.stringify({
              shape: ['slice','subarray','toReversed','toSorted'].map((m) => {
                const proto = Object.getPrototypeOf(Uint8Array.prototype);
                const f = proto[m];
                const d = Object.getOwnPropertyDescriptor(proto, m);
                const pd = Object.getOwnPropertyDescriptor(f, 'prototype');
                const ctor = (() => { try { new f(); return 'ok'; } catch (e) { return e.name; } })();
                return [m, f.length, d.writable, d.enumerable, d.configurable, ctor, !!pd];
              }),
              basic: (() => {
                const a = new Uint8Array([4,3,2,1]);
                const s = callOrName(() => a.slice(-3, -1));
                const sub = callOrName(() => a.subarray(-3, -1));
                const rev = callOrName(() => a.toReversed());
                const sorted = callOrName(() => a.toSorted());
                return [
                  Array.from(s),
                  s.buffer === a.buffer,
                  Array.from(sub),
                  sub.buffer === a.buffer,
                  Array.from(rev),
                  rev.buffer === a.buffer,
                  Array.from(sorted),
                  sorted.buffer === a.buffer,
                  Array.from(a)
                ];
              })(),
              bigint: (() => {
                const b = new BigInt64Array([3n,1n,2n]);
                return [
                  Array.from(b.slice(1), String),
                  Array.from(b.subarray(0,2), String),
                  Array.from(b.toSorted(), String)
                ];
              })(),
              compare: (() => {
                const a = new Uint8Array([10, 2, 1]);
                let calls = [];
                const r = callOrName(() => a.toSorted((x,y) => { calls.push([x,y]); return y - x; }));
                return [Array.from(r), calls.length > 0, Array.from(a)];
              })(),
              speciesCtor: (() => {
                class MyTA extends Uint8Array {}
                const speciesCalls = [];
                Object.defineProperty(MyTA, Symbol.species, {
                  get(){ speciesCalls.push('get'); return Uint16Array; }
                });
                const a = new MyTA([1,2,3,4]);
                const sliced = callOrName(() => a.slice(1,3));
                const rev = callOrName(() => a.toReversed());
                const sorted = callOrName(() => a.toSorted((x,y)=>y-x));
                return [
                  speciesCalls,
                  sliced.constructor.name,
                  sliced.length,
                  Array.from(sliced),
                  rev.constructor.name,
                  Array.from(rev),
                  sorted.constructor.name,
                  Array.from(sorted)
                ];
              })(),
              detached: (() => {
                const ab = new ArrayBuffer(8);
                const u = new Uint8Array(ab);
                structuredClone(ab, { transfer: [ab] });
                return ['slice','subarray','toReversed','toSorted'].map((m) => callOrName(() => u[m](0,1)));
              })()
            });
            })()
            "#,
        );
        assert_eq!(
            actual,
            r#"{"shape":[["slice",2,true,false,true,"TypeError",false],["subarray",2,true,false,true,"TypeError",false],["toReversed",0,true,false,true,"TypeError",false],["toSorted",1,true,false,true,"TypeError",false]],"basic":[[3,2],false,[3,2],true,[1,2,3,4],false,[1,2,3,4],false,[4,3,2,1]],"bigint":[["1","2"],["3","1"],["1","2","3"]],"compare":[[10,2,1],true,[10,2,1]],"speciesCtor":[["get"],"Uint16Array",2,[2,3],"Uint8Array",[4,3,2,1],"Uint8Array",[4,3,2,1]],"detached":["TypeError","TypeError","TypeError","TypeError"]}"#
        );
    }

    #[test]
    fn arraybuffer_lifecycle_manifest_rows_are_generated_checked_and_fast_path_ineligible() {
        for row in [
            ARRAYBUFFER_IS_VIEW,
            ARRAYBUFFER_SLICE,
            ARRAYBUFFER_RESIZE,
            ARRAYBUFFER_TRANSFER,
            ARRAYBUFFER_TRANSFER_TO_FIXED_LENGTH,
        ] {
            assert_eq!(row.receiver, NativeApiReceiver::ArrayBuffer);
            assert_eq!(row.registration.property, row.property);
            assert_eq!(row.registration.writable, true);
            assert_eq!(row.registration.enumerable, false);
            assert_eq!(row.registration.configurable, true);
            assert_eq!(row.registration.constructor, false);
            assert_eq!(row.registration.function_prototype, false);
            assert_eq!(row.validation.receiver, NativeApiReceiver::ArrayBuffer);
            assert!(row.ihi.is_none(), "{} must not claim IHI", row.api);
            assert!(row.jit_ic.is_none(), "{} must not claim JIT", row.api);
            assert!(
                row.lejit_expectation.is_none(),
                "{} must not claim LeJIT",
                row.api
            );
            assert!(row.fixtures.contains(&"consumer-ineligible"));
            assert!(MANIFEST_ROWS
                .iter()
                .any(|candidate| candidate.api == row.api));
        }

        assert_eq!(ARRAYBUFFER_IS_VIEW.kind, NativeApiKind::StaticMethod);
        assert_eq!(
            ARRAYBUFFER_IS_VIEW.validation.receiver_semantics,
            "arraybuffer-static"
        );
        assert_eq!(ARRAYBUFFER_SLICE.kind, NativeApiKind::Method);
        assert_eq!(
            ARRAYBUFFER_SLICE.validation.index_coercion,
            NativeApiIndexCoercion::ToIntegerOrInfinity
        );
        assert_eq!(
            ARRAYBUFFER_SLICE.validation.receiver_semantics,
            "arraybuffer-brand-detach"
        );
        assert_eq!(
            ARRAYBUFFER_RESIZE.validation.effect,
            NativeApiEffect::MutatesReceiverBytes
        );
        assert_eq!(ARRAYBUFFER_RESIZE.returns, &[NativeApiDomain::Undefined]);
        assert_eq!(
            ARRAYBUFFER_TRANSFER.validation.receiver_semantics,
            "arraybuffer-transfer-detach"
        );
        assert_eq!(
            ARRAYBUFFER_TRANSFER_TO_FIXED_LENGTH
                .validation
                .receiver_semantics,
            "arraybuffer-transfer-fixed-detach"
        );
        assert_eq!(
            crate::native_api_manifest_generated::array_buffer_static_is_view_generated_ihi_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::array_buffer_slice_generated_jit_ic_spec(),
            None
        );
        assert_eq!(
            crate::native_api_manifest_generated::
                array_buffer_transfer_generated_lejit_expectation_spec(),
            None
        );
    }

    #[test]
    fn arraybuffer_lifecycle_runtime_shape_and_semantics_match_node_oracle() {
        let actual = run_manifest_json(
            r#"
            (() => {
            const desc = (obj, name) => {
              const d = Object.getOwnPropertyDescriptor(obj, name);
              const v = d && (d.get || d.value);
              const pd = typeof v === 'function' ? Object.getOwnPropertyDescriptor(v, 'prototype') : undefined;
              const ctor = typeof v === 'function' ? (() => { try { new v(); return 'ok'; } catch(e) { return e.name; } })() : null;
              return [name, typeof v, typeof v === 'function' ? v.length : null, !!d && d.writable === true, !!d && d.enumerable, !!d && d.configurable, ctor, !!pd, !!d && typeof d.get === 'function', !!d && typeof d.set === 'function'];
            };
            const callOrName = (fn) => { try { return fn(); } catch(e) { return e.name; } };
            return JSON.stringify({
              ctorShape: [
                desc(globalThis, 'ArrayBuffer'),
                desc(globalThis, 'SharedArrayBuffer'),
                desc(ArrayBuffer, 'isView')
              ],
              protoShape: ['slice','resize','transfer','transferToFixedLength'].map((m) => desc(ArrayBuffer.prototype, m)),
              getterShape: ['byteLength','maxByteLength','detached','resizable'].map((m) => desc(ArrayBuffer.prototype, m)),
              allocation: (() => {
                const a = new ArrayBuffer(4);
                const u = new Uint8Array(a);
                u.set([1,2,3,4]);
                const sliced = a.slice(1,3);
                return [a.byteLength, a instanceof ArrayBuffer, ArrayBuffer.isView(u), ArrayBuffer.isView(new DataView(a)), ArrayBuffer.isView(a), Array.from(new Uint8Array(sliced)), sliced === a, sliced.byteLength];
              })(),
              errors: [
                callOrName(() => ArrayBuffer()),
                callOrName(() => new ArrayBuffer(-1)),
                callOrName(() => new ArrayBuffer(2 ** 60)),
                callOrName(() => ArrayBuffer.prototype.slice.call({}, 0, 1)),
                callOrName(() => Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, 'byteLength').get.call({}))
              ],
              detach: (() => {
                const a = new ArrayBuffer(8);
                const u = new Uint8Array(a);
                u.set([9,8,7,6]);
                structuredClone(a, { transfer: [a] });
                return [a.byteLength, callOrName(() => a.slice(0)), callOrName(() => new Uint8Array(a)), ArrayBuffer.isView(u), u.byteLength, callOrName(() => u[0])];
              })(),
              lifecycle: (() => {
                const base = new ArrayBuffer(4, { maxByteLength: 12 });
                const initial = [base.byteLength, base.maxByteLength, base.resizable === true, base.detached === false];
                const resizeRet = callOrName(() => base.resize(8));
                const afterResize = [base.byteLength, base.maxByteLength, base.resizable === true, base.detached === false];
                const transferred = callOrName(() => base.transfer(6));
                const afterTransfer = [base.byteLength, base.detached === true];
                const transferShape = transferred instanceof ArrayBuffer ? [transferred.byteLength, transferred.maxByteLength, transferred.resizable === true, transferred.detached === false] : transferred;
                const fixedSrc = new ArrayBuffer(4, { maxByteLength: 8 });
                const fixed = callOrName(() => fixedSrc.transferToFixedLength(2));
                const fixedShape = fixed instanceof ArrayBuffer ? [fixed.byteLength, fixed.maxByteLength, fixed.resizable === true, fixed.detached === false, fixedSrc.detached === true] : fixed;
                return [initial, resizeRet, afterResize, transferShape, afterTransfer, fixedShape];
              })(),
              sab: (() => {
                const s = new SharedArrayBuffer(4);
                return [s.byteLength, ArrayBuffer.isView(new Uint8Array(s)), typeof s.slice, callOrName(() => structuredClone(s, { transfer: [s] }))];
              })()
            });
            })()
            "#,
        );
        assert_eq!(
            actual,
            r#"{"ctorShape":[["ArrayBuffer","function",1,true,false,true,"ok",true,false,false],["SharedArrayBuffer","function",1,true,false,true,"ok",true,false,false],["isView","function",1,true,false,true,"TypeError",false,false,false]],"protoShape":[["slice","function",2,true,false,true,"TypeError",false,false,false],["resize","function",1,true,false,true,"TypeError",false,false,false],["transfer","function",0,true,false,true,"TypeError",false,false,false],["transferToFixedLength","function",0,true,false,true,"TypeError",false,false,false]],"getterShape":[["byteLength","function",0,false,false,true,"TypeError",false,true,false],["maxByteLength","function",0,false,false,true,"TypeError",false,true,false],["detached","function",0,false,false,true,"TypeError",false,true,false],["resizable","function",0,false,false,true,"TypeError",false,true,false]],"allocation":[4,true,true,true,false,[2,3],false,2],"errors":["TypeError","RangeError","RangeError","TypeError","TypeError"],"detach":[0,"TypeError","TypeError",true,0,null],"lifecycle":[[4,12,true,true],null,[8,12,true,true],[6,12,true,true],[0,true],[2,2,false,true,true]],"sab":[4,true,"function","DataCloneError"]}"#
        );
    }
}
