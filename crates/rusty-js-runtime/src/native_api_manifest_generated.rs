
pub(crate) const fn string_char_code_at_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "charCodeAt",
        display_name: "charCodeAt",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn string_char_code_at_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::String,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::NumberNaN,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::NumberNaN,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn string_char_code_at_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "charCodeAt",
        receiver: crate::interp_ic_table::IhiReceiverKind::String,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::StringCharCodeAt,
        fast_fn: "fast_string_char_code_at",
    })
}

pub(crate) const fn generated_string_char_code_at_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match string_char_code_at_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn string_char_code_at_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = string_char_code_at_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn string_char_code_at_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    Some(crate::native_api_manifest::GeneratedJitIcSpec {
        key: "charCodeAt",
        receiver: "String",
        kind: "MethodCall",
        arity: Some(1),
        extern_name: "ic_string_char_code_at",
    })
}

pub(crate) const fn string_char_code_at_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    Some(crate::native_api_manifest::GeneratedLejitExpectationSpec {
        key: "charCodeAt",
        receiver: rusty_js_jit::ic_table::ReceiverKind::String,
        kind: rusty_js_jit::ic_table::IcEntryKind::MethodCall { arity: 1 },
        arity: Some(1),
        extern_name: "ic_string_char_code_at",
        arg_domains: &[rusty_js_jit::ic_table::LejitValueDomain::Number],
        return_domain: rusty_js_jit::ic_table::LejitValueDomain::Number,
        override_guard: rusty_js_jit::ic_table::LejitOverrideGuard::MethodIdentityUnchanged,
        deopt_bailouts: &[
            "receiver-not-string",
            "arity-mismatch",
            "argument-not-number-or-undefined",
            "method-overridden",
        ],
    })
}

pub(crate) const fn string_char_code_at_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "String.prototype.charCodeAt",
        receiver: "String",
        property: "charCodeAt",
        arity: 1,
        args: &["Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn string_to_lower_case_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "toLowerCase",
        display_name: "toLowerCase",
        length: 0,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn string_to_lower_case_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::String,
        arity: 0,
        args: &[],
        returns: &[crate::native_api_manifest::NativeApiDomain::String],
        missing_arg_defaults_to_undefined: false,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn string_to_lower_case_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "toLowerCase",
        receiver: crate::interp_ic_table::IhiReceiverKind::String,
        arity: Some(0),
        cached_id_field: crate::interp_ic_table::IhiCachedField::StringToLowerCase,
        fast_fn: "fast_string_to_lower_case",
    })
}

pub(crate) const fn generated_string_to_lower_case_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match string_to_lower_case_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn string_to_lower_case_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 0] {
    let spec = string_to_lower_case_generated_validation_spec();
    debug_assert_eq!(spec.arity, 0);
    []
}

pub(crate) const fn string_to_lower_case_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn string_to_lower_case_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn string_to_lower_case_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "String.prototype.toLowerCase",
        receiver: "String",
        property: "toLowerCase",
        arity: 0,
        args: &[],
        returns: &["String"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn string_to_upper_case_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "toUpperCase",
        display_name: "toUpperCase",
        length: 0,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn string_to_upper_case_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::String,
        arity: 0,
        args: &[],
        returns: &[crate::native_api_manifest::NativeApiDomain::String],
        missing_arg_defaults_to_undefined: false,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn string_to_upper_case_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "toUpperCase",
        receiver: crate::interp_ic_table::IhiReceiverKind::String,
        arity: Some(0),
        cached_id_field: crate::interp_ic_table::IhiCachedField::StringToUpperCase,
        fast_fn: "fast_string_to_upper_case",
    })
}

pub(crate) const fn generated_string_to_upper_case_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match string_to_upper_case_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn string_to_upper_case_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 0] {
    let spec = string_to_upper_case_generated_validation_spec();
    debug_assert_eq!(spec.arity, 0);
    []
}

pub(crate) const fn string_to_upper_case_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn string_to_upper_case_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn string_to_upper_case_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "String.prototype.toUpperCase",
        receiver: "String",
        property: "toUpperCase",
        arity: 0,
        args: &[],
        returns: &["String"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn string_trim_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "trim",
        display_name: "trim",
        length: 0,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn string_trim_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::String,
        arity: 0,
        args: &[],
        returns: &[crate::native_api_manifest::NativeApiDomain::String],
        missing_arg_defaults_to_undefined: false,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn string_trim_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "trim",
        receiver: crate::interp_ic_table::IhiReceiverKind::String,
        arity: Some(0),
        cached_id_field: crate::interp_ic_table::IhiCachedField::StringTrim,
        fast_fn: "fast_string_trim",
    })
}

pub(crate) const fn generated_string_trim_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match string_trim_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn string_trim_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 0] {
    let spec = string_trim_generated_validation_spec();
    debug_assert_eq!(spec.arity, 0);
    []
}

pub(crate) const fn string_trim_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn string_trim_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn string_trim_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "String.prototype.trim",
        receiver: "String",
        property: "trim",
        arity: 0,
        args: &[],
        returns: &["String"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn string_includes_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "includes",
        display_name: "includes",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn string_includes_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::String,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::String],
        returns: &[crate::native_api_manifest::NativeApiDomain::Boolean],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn string_includes_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "includes",
        receiver: crate::interp_ic_table::IhiReceiverKind::String,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::StringIncludes,
        fast_fn: "fast_string_includes",
    })
}

pub(crate) const fn generated_string_includes_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match string_includes_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn string_includes_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = string_includes_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn string_includes_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn string_includes_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn string_includes_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "String.prototype.includes",
        receiver: "String",
        property: "includes",
        arity: 1,
        args: &["String"],
        returns: &["Boolean"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn string_starts_with_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "startsWith",
        display_name: "startsWith",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn string_starts_with_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::String,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::String],
        returns: &[crate::native_api_manifest::NativeApiDomain::Boolean],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn string_starts_with_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "startsWith",
        receiver: crate::interp_ic_table::IhiReceiverKind::String,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::StringStartsWith,
        fast_fn: "fast_string_starts_with",
    })
}

pub(crate) const fn generated_string_starts_with_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match string_starts_with_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn string_starts_with_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = string_starts_with_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn string_starts_with_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn string_starts_with_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn string_starts_with_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "String.prototype.startsWith",
        receiver: "String",
        property: "startsWith",
        arity: 1,
        args: &["String"],
        returns: &["Boolean"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn string_ends_with_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "endsWith",
        display_name: "endsWith",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn string_ends_with_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::String,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::String],
        returns: &[crate::native_api_manifest::NativeApiDomain::Boolean],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn string_ends_with_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "endsWith",
        receiver: crate::interp_ic_table::IhiReceiverKind::String,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::StringEndsWith,
        fast_fn: "fast_string_ends_with",
    })
}

pub(crate) const fn generated_string_ends_with_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match string_ends_with_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn string_ends_with_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = string_ends_with_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn string_ends_with_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn string_ends_with_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn string_ends_with_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "String.prototype.endsWith",
        receiver: "String",
        property: "endsWith",
        arity: 1,
        args: &["String"],
        returns: &["Boolean"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn string_index_of_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "indexOf",
        display_name: "indexOf",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn string_index_of_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::String,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::String],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn string_index_of_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "indexOf",
        receiver: crate::interp_ic_table::IhiReceiverKind::String,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::StringIndexOf,
        fast_fn: "fast_string_index_of_1",
    })
}

pub(crate) const fn generated_string_index_of_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match string_index_of_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn string_index_of_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = string_index_of_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn string_index_of_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn string_index_of_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn string_index_of_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "String.prototype.indexOf",
        receiver: "String",
        property: "indexOf",
        arity: 1,
        args: &["String"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn string_last_index_of_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "lastIndexOf",
        display_name: "lastIndexOf",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn string_last_index_of_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::String,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::String],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn string_last_index_of_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn string_last_index_of_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = string_last_index_of_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn string_last_index_of_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn string_last_index_of_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn string_last_index_of_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn string_trim_start_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "trimStart",
        display_name: "trimStart",
        length: 0,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn string_trim_start_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::String,
        arity: 0,
        args: &[],
        returns: &[crate::native_api_manifest::NativeApiDomain::String],
        missing_arg_defaults_to_undefined: false,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn string_trim_start_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn string_trim_start_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 0] {
    let spec = string_trim_start_generated_validation_spec();
    debug_assert_eq!(spec.arity, 0);
    []
}

pub(crate) const fn string_trim_start_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn string_trim_start_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn string_trim_start_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn string_trim_end_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "trimEnd",
        display_name: "trimEnd",
        length: 0,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn string_trim_end_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::String,
        arity: 0,
        args: &[],
        returns: &[crate::native_api_manifest::NativeApiDomain::String],
        missing_arg_defaults_to_undefined: false,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn string_trim_end_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn string_trim_end_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 0] {
    let spec = string_trim_end_generated_validation_spec();
    debug_assert_eq!(spec.arity, 0);
    []
}

pub(crate) const fn string_trim_end_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn string_trim_end_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn string_trim_end_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn string_repeat_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "repeat",
        display_name: "repeat",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn string_repeat_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::String,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::String],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn string_repeat_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn string_repeat_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = string_repeat_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn string_repeat_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn string_repeat_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn string_repeat_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn string_pad_start_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "padStart",
        display_name: "padStart",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn string_pad_start_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::String,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::String],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn string_pad_start_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn string_pad_start_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = string_pad_start_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn string_pad_start_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn string_pad_start_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn string_pad_start_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn string_pad_end_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "padEnd",
        display_name: "padEnd",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn string_pad_end_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::String,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::String],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn string_pad_end_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn string_pad_end_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = string_pad_end_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn string_pad_end_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn string_pad_end_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn string_pad_end_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn string_char_at_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "charAt",
        display_name: "charAt",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn string_char_at_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::String,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::String],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::EmptyString,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::EmptyString,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn string_char_at_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn string_char_at_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = string_char_at_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn string_char_at_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn string_char_at_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn string_char_at_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn string_at_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "at",
        display_name: "at",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn string_at_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::String,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[
            crate::native_api_manifest::NativeApiDomain::String,
            crate::native_api_manifest::NativeApiDomain::Undefined,
        ],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::Undefined,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::Undefined,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn string_at_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn string_at_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = string_at_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn string_at_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn string_at_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn string_at_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn string_slice_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "slice",
        display_name: "slice",
        length: 2,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn string_slice_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::String,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::String],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::EmptyString,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::EmptyString,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn string_slice_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn string_slice_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 2] {
    let spec = string_slice_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn string_slice_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn string_slice_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn string_slice_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn string_substring_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "substring",
        display_name: "substring",
        length: 2,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn string_substring_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::String,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::String],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::EmptyString,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::EmptyString,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn string_substring_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn string_substring_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = string_substring_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn string_substring_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn string_substring_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn string_substring_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn string_substr_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "substr",
        display_name: "substr",
        length: 2,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn string_substr_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::String,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::String],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::EmptyString,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::EmptyString,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn string_substr_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn string_substr_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 2] {
    let spec = string_substr_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn string_substr_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn string_substr_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn string_substr_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn string_code_point_at_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "codePointAt",
        display_name: "codePointAt",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn string_code_point_at_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::String,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Undefined,
        ],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::Undefined,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::Undefined,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn string_code_point_at_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "codePointAt",
        receiver: crate::interp_ic_table::IhiReceiverKind::String,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::StringCodePointAt,
        fast_fn: "fast_string_code_point_at",
    })
}

pub(crate) const fn generated_string_code_point_at_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match string_code_point_at_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn string_code_point_at_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = string_code_point_at_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn string_code_point_at_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    Some(crate::native_api_manifest::GeneratedJitIcSpec {
        key: "codePointAt",
        receiver: "String",
        kind: "MethodCall",
        arity: Some(1),
        extern_name: "ic_string_code_point_at",
    })
}

pub(crate) const fn string_code_point_at_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn string_code_point_at_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "String.prototype.codePointAt",
        receiver: "String",
        property: "codePointAt",
        arity: 1,
        args: &["Number"],
        returns: &["Number", "Undefined"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_read_uint8_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readUInt8",
        display_name: "readUInt8",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_uint8_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 1,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::SingleByte,
    }
}

pub(crate) const fn buffer_read_uint8_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "readUInt8",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferReadUInt8,
        fast_fn: "fast_buffer_read_u8",
    })
}

pub(crate) const fn generated_buffer_read_u_int8_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_read_uint8_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_read_uint8_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_read_uint8_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_read_uint8_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    Some(crate::native_api_manifest::GeneratedJitIcSpec {
        key: "readUInt8",
        receiver: "Buffer",
        kind: "MethodCall",
        arity: Some(1),
        extern_name: "ic_buffer_read_u8",
    })
}

pub(crate) const fn buffer_read_uint8_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_uint8_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.readUInt8",
        receiver: "Buffer",
        property: "readUInt8",
        arity: 1,
        args: &["Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_read_int8_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readInt8",
        display_name: "readInt8",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_int8_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 1,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::SingleByte,
    }
}

pub(crate) const fn buffer_read_int8_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "readInt8",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferReadInt8,
        fast_fn: "fast_buffer_read_i8",
    })
}

pub(crate) const fn generated_buffer_read_int8_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_read_int8_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_read_int8_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_read_int8_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_read_int8_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_read_int8_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_int8_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.readInt8",
        receiver: "Buffer",
        property: "readInt8",
        arity: 1,
        args: &["Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_write_uint8_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeUInt8",
        display_name: "writeUInt8",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_uint8_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 1,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::SingleByte,
    }
}

pub(crate) const fn buffer_write_uint8_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "writeUInt8",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(2),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferWriteUInt8,
        fast_fn: "fast_buffer_write_u8",
    })
}

pub(crate) const fn generated_buffer_write_u_int8_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_write_uint8_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_write_uint8_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_write_uint8_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_uint8_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    Some(crate::native_api_manifest::GeneratedJitIcSpec {
        key: "writeUInt8",
        receiver: "Buffer",
        kind: "MethodCall",
        arity: Some(2),
        extern_name: "ic_buffer_write_u8",
    })
}

pub(crate) const fn buffer_write_uint8_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_uint8_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.writeUInt8",
        receiver: "Buffer",
        property: "writeUInt8",
        arity: 2,
        args: &["Number", "Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: false,
    })
}

pub(crate) const fn buffer_write_int8_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeInt8",
        display_name: "writeInt8",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_int8_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 1,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::SingleByte,
    }
}

pub(crate) const fn buffer_write_int8_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "writeInt8",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(2),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferWriteInt8,
        fast_fn: "fast_buffer_write_i8",
    })
}

pub(crate) const fn generated_buffer_write_int8_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_write_int8_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_write_int8_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_write_int8_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_int8_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_write_int8_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_int8_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.writeInt8",
        receiver: "Buffer",
        property: "writeInt8",
        arity: 2,
        args: &["Number", "Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: false,
    })
}

pub(crate) const fn buffer_read_uint16_le_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readUInt16LE",
        display_name: "readUInt16LE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_uint16_le_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 2,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::Little,
    }
}

pub(crate) const fn buffer_read_uint16_le_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "readUInt16LE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferReadUInt16LE,
        fast_fn: "fast_buffer_read_u16le",
    })
}

pub(crate) const fn generated_buffer_read_u_int16_le_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_read_uint16_le_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_read_uint16_le_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_read_uint16_le_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_read_uint16_le_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    Some(crate::native_api_manifest::GeneratedJitIcSpec {
        key: "readUInt16LE",
        receiver: "Buffer",
        kind: "MethodCall",
        arity: Some(1),
        extern_name: "ic_buffer_read_u16le",
    })
}

pub(crate) const fn buffer_read_uint16_le_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_uint16_le_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.readUInt16LE",
        receiver: "Buffer",
        property: "readUInt16LE",
        arity: 1,
        args: &["Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_read_uint16_be_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readUInt16BE",
        display_name: "readUInt16BE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_uint16_be_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 2,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::Big,
    }
}

pub(crate) const fn buffer_read_uint16_be_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "readUInt16BE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferReadUInt16BE,
        fast_fn: "fast_buffer_read_u16be",
    })
}

pub(crate) const fn generated_buffer_read_u_int16_be_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_read_uint16_be_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_read_uint16_be_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_read_uint16_be_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_read_uint16_be_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    Some(crate::native_api_manifest::GeneratedJitIcSpec {
        key: "readUInt16BE",
        receiver: "Buffer",
        kind: "MethodCall",
        arity: Some(1),
        extern_name: "ic_buffer_read_u16be",
    })
}

pub(crate) const fn buffer_read_uint16_be_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_uint16_be_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.readUInt16BE",
        receiver: "Buffer",
        property: "readUInt16BE",
        arity: 1,
        args: &["Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_read_int16_le_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readInt16LE",
        display_name: "readInt16LE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_int16_le_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 2,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Little,
    }
}

pub(crate) const fn buffer_read_int16_le_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "readInt16LE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferReadInt16LE,
        fast_fn: "fast_buffer_read_i16le",
    })
}

pub(crate) const fn generated_buffer_read_int16_le_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_read_int16_le_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_read_int16_le_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_read_int16_le_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_read_int16_le_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_read_int16_le_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_int16_le_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.readInt16LE",
        receiver: "Buffer",
        property: "readInt16LE",
        arity: 1,
        args: &["Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_read_int16_be_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readInt16BE",
        display_name: "readInt16BE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_int16_be_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 2,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Big,
    }
}

pub(crate) const fn buffer_read_int16_be_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "readInt16BE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferReadInt16BE,
        fast_fn: "fast_buffer_read_i16be",
    })
}

pub(crate) const fn generated_buffer_read_int16_be_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_read_int16_be_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_read_int16_be_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_read_int16_be_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_read_int16_be_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_read_int16_be_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_int16_be_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.readInt16BE",
        receiver: "Buffer",
        property: "readInt16BE",
        arity: 1,
        args: &["Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_write_uint16_le_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeUInt16LE",
        display_name: "writeUInt16LE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_uint16_le_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 2,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::Little,
    }
}

pub(crate) const fn buffer_write_uint16_le_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "writeUInt16LE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(2),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferWriteUInt16LE,
        fast_fn: "fast_buffer_write_u16le",
    })
}

pub(crate) const fn generated_buffer_write_u_int16_le_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_write_uint16_le_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_write_uint16_le_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_write_uint16_le_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_uint16_le_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    Some(crate::native_api_manifest::GeneratedJitIcSpec {
        key: "writeUInt16LE",
        receiver: "Buffer",
        kind: "MethodCall",
        arity: Some(2),
        extern_name: "ic_buffer_write_u16le",
    })
}

pub(crate) const fn buffer_write_uint16_le_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_uint16_le_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.writeUInt16LE",
        receiver: "Buffer",
        property: "writeUInt16LE",
        arity: 2,
        args: &["Number", "Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: false,
    })
}

pub(crate) const fn buffer_write_uint16_be_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeUInt16BE",
        display_name: "writeUInt16BE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_uint16_be_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 2,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::Big,
    }
}

pub(crate) const fn buffer_write_uint16_be_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "writeUInt16BE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(2),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferWriteUInt16BE,
        fast_fn: "fast_buffer_write_u16be",
    })
}

pub(crate) const fn generated_buffer_write_u_int16_be_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_write_uint16_be_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_write_uint16_be_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_write_uint16_be_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_uint16_be_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    Some(crate::native_api_manifest::GeneratedJitIcSpec {
        key: "writeUInt16BE",
        receiver: "Buffer",
        kind: "MethodCall",
        arity: Some(2),
        extern_name: "ic_buffer_write_u16be",
    })
}

pub(crate) const fn buffer_write_uint16_be_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_uint16_be_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.writeUInt16BE",
        receiver: "Buffer",
        property: "writeUInt16BE",
        arity: 2,
        args: &["Number", "Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: false,
    })
}

pub(crate) const fn buffer_write_int16_le_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeInt16LE",
        display_name: "writeInt16LE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_int16_le_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 2,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Little,
    }
}

pub(crate) const fn buffer_write_int16_le_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "writeInt16LE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(2),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferWriteInt16LE,
        fast_fn: "fast_buffer_write_i16le",
    })
}

pub(crate) const fn generated_buffer_write_int16_le_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_write_int16_le_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_write_int16_le_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_write_int16_le_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_int16_le_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_write_int16_le_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_int16_le_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.writeInt16LE",
        receiver: "Buffer",
        property: "writeInt16LE",
        arity: 2,
        args: &["Number", "Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: false,
    })
}

pub(crate) const fn buffer_write_int16_be_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeInt16BE",
        display_name: "writeInt16BE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_int16_be_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 2,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Big,
    }
}

pub(crate) const fn buffer_write_int16_be_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "writeInt16BE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(2),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferWriteInt16BE,
        fast_fn: "fast_buffer_write_i16be",
    })
}

pub(crate) const fn generated_buffer_write_int16_be_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_write_int16_be_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_write_int16_be_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_write_int16_be_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_int16_be_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_write_int16_be_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_int16_be_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.writeInt16BE",
        receiver: "Buffer",
        property: "writeInt16BE",
        arity: 2,
        args: &["Number", "Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: false,
    })
}

pub(crate) const fn buffer_read_uint32_le_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readUInt32LE",
        display_name: "readUInt32LE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_uint32_le_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 4,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::Little,
    }
}

pub(crate) const fn buffer_read_uint32_le_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "readUInt32LE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferReadUInt32LE,
        fast_fn: "fast_buffer_read_u32le",
    })
}

pub(crate) const fn generated_buffer_read_u_int32_le_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_read_uint32_le_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_read_uint32_le_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_read_uint32_le_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_read_uint32_le_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    Some(crate::native_api_manifest::GeneratedJitIcSpec {
        key: "readUInt32LE",
        receiver: "Buffer",
        kind: "MethodCall",
        arity: Some(1),
        extern_name: "ic_buffer_read_u32le",
    })
}

pub(crate) const fn buffer_read_uint32_le_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_uint32_le_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.readUInt32LE",
        receiver: "Buffer",
        property: "readUInt32LE",
        arity: 1,
        args: &["Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_read_uint32_be_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readUInt32BE",
        display_name: "readUInt32BE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_uint32_be_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 4,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::Big,
    }
}

pub(crate) const fn buffer_read_uint32_be_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "readUInt32BE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferReadUInt32BE,
        fast_fn: "fast_buffer_read_u32be",
    })
}

pub(crate) const fn generated_buffer_read_u_int32_be_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_read_uint32_be_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_read_uint32_be_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_read_uint32_be_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_read_uint32_be_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    Some(crate::native_api_manifest::GeneratedJitIcSpec {
        key: "readUInt32BE",
        receiver: "Buffer",
        kind: "MethodCall",
        arity: Some(1),
        extern_name: "jit_buffer_read_u32be",
    })
}

pub(crate) const fn buffer_read_uint32_be_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_uint32_be_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.readUInt32BE",
        receiver: "Buffer",
        property: "readUInt32BE",
        arity: 1,
        args: &["Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_read_int32_le_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readInt32LE",
        display_name: "readInt32LE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_int32_le_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 4,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Little,
    }
}

pub(crate) const fn buffer_read_int32_le_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "readInt32LE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferReadInt32LE,
        fast_fn: "fast_buffer_read_i32le",
    })
}

pub(crate) const fn generated_buffer_read_int32_le_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_read_int32_le_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_read_int32_le_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_read_int32_le_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_read_int32_le_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_read_int32_le_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_int32_le_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.readInt32LE",
        receiver: "Buffer",
        property: "readInt32LE",
        arity: 1,
        args: &["Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_read_int32_be_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readInt32BE",
        display_name: "readInt32BE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_int32_be_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 4,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Big,
    }
}

pub(crate) const fn buffer_read_int32_be_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "readInt32BE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferReadInt32BE,
        fast_fn: "fast_buffer_read_i32be",
    })
}

pub(crate) const fn generated_buffer_read_int32_be_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_read_int32_be_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_read_int32_be_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_read_int32_be_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_read_int32_be_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    Some(crate::native_api_manifest::GeneratedJitIcSpec {
        key: "readInt32BE",
        receiver: "Buffer",
        kind: "MethodCall",
        arity: Some(1),
        extern_name: "ic_buffer_read_i32be",
    })
}

pub(crate) const fn buffer_read_int32_be_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_int32_be_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.readInt32BE",
        receiver: "Buffer",
        property: "readInt32BE",
        arity: 1,
        args: &["Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_write_uint32_le_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeUInt32LE",
        display_name: "writeUInt32LE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_uint32_le_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 4,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::Little,
    }
}

pub(crate) const fn buffer_write_uint32_le_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "writeUInt32LE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(2),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferWriteUInt32LE,
        fast_fn: "fast_buffer_write_u32le",
    })
}

pub(crate) const fn generated_buffer_write_u_int32_le_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_write_uint32_le_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_write_uint32_le_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_write_uint32_le_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_uint32_le_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    Some(crate::native_api_manifest::GeneratedJitIcSpec {
        key: "writeUInt32LE",
        receiver: "Buffer",
        kind: "MethodCall",
        arity: Some(2),
        extern_name: "ic_buffer_write_u32le",
    })
}

pub(crate) const fn buffer_write_uint32_le_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_uint32_le_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.writeUInt32LE",
        receiver: "Buffer",
        property: "writeUInt32LE",
        arity: 2,
        args: &["Number", "Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_write_uint32_be_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeUInt32BE",
        display_name: "writeUInt32BE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_uint32_be_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 4,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::Big,
    }
}

pub(crate) const fn buffer_write_uint32_be_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "writeUInt32BE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(2),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferWriteUInt32BE,
        fast_fn: "fast_buffer_write_u32be",
    })
}

pub(crate) const fn generated_buffer_write_u_int32_be_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_write_uint32_be_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_write_uint32_be_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_write_uint32_be_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_uint32_be_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    Some(crate::native_api_manifest::GeneratedJitIcSpec {
        key: "writeUInt32BE",
        receiver: "Buffer",
        kind: "MethodCall",
        arity: Some(2),
        extern_name: "jit_buffer_write_u32be",
    })
}

pub(crate) const fn buffer_write_uint32_be_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_uint32_be_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.writeUInt32BE",
        receiver: "Buffer",
        property: "writeUInt32BE",
        arity: 2,
        args: &["Number", "Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_write_int32_le_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeInt32LE",
        display_name: "writeInt32LE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_int32_le_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 4,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Little,
    }
}

pub(crate) const fn buffer_write_int32_le_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "writeInt32LE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(2),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferWriteInt32LE,
        fast_fn: "fast_buffer_write_i32le",
    })
}

pub(crate) const fn generated_buffer_write_int32_le_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_write_int32_le_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_write_int32_le_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_write_int32_le_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_int32_le_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_write_int32_le_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_int32_le_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.writeInt32LE",
        receiver: "Buffer",
        property: "writeInt32LE",
        arity: 2,
        args: &["Number", "Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_write_int32_be_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeInt32BE",
        display_name: "writeInt32BE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_int32_be_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 4,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Big,
    }
}

pub(crate) const fn buffer_write_int32_be_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "writeInt32BE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(2),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferWriteInt32BE,
        fast_fn: "fast_buffer_write_i32be",
    })
}

pub(crate) const fn generated_buffer_write_int32_be_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_write_int32_be_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_write_int32_be_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_write_int32_be_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_int32_be_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_write_int32_be_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_int32_be_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.writeInt32BE",
        receiver: "Buffer",
        property: "writeInt32BE",
        arity: 2,
        args: &["Number", "Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_read_float_le_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readFloatLE",
        display_name: "readFloatLE",
        length: 0,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_float_le_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 4,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Little,
    }
}

pub(crate) const fn buffer_read_float_le_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "readFloatLE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferReadFloatLE,
        fast_fn: "fast_buffer_read_f32le",
    })
}

pub(crate) const fn generated_buffer_read_float_le_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_read_float_le_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_read_float_le_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_read_float_le_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_read_float_le_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_read_float_le_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_float_le_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.readFloatLE",
        receiver: "Buffer",
        property: "readFloatLE",
        arity: 1,
        args: &["Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_read_float_be_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readFloatBE",
        display_name: "readFloatBE",
        length: 0,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_float_be_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 4,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Big,
    }
}

pub(crate) const fn buffer_read_float_be_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "readFloatBE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferReadFloatBE,
        fast_fn: "fast_buffer_read_f32be",
    })
}

pub(crate) const fn generated_buffer_read_float_be_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_read_float_be_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_read_float_be_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_read_float_be_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_read_float_be_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_read_float_be_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_float_be_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.readFloatBE",
        receiver: "Buffer",
        property: "readFloatBE",
        arity: 1,
        args: &["Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_read_double_le_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readDoubleLE",
        display_name: "readDoubleLE",
        length: 0,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_double_le_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 8,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Little,
    }
}

pub(crate) const fn buffer_read_double_le_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "readDoubleLE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferReadDoubleLE,
        fast_fn: "fast_buffer_read_f64le",
    })
}

pub(crate) const fn generated_buffer_read_double_le_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_read_double_le_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_read_double_le_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_read_double_le_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_read_double_le_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_read_double_le_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_double_le_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.readDoubleLE",
        receiver: "Buffer",
        property: "readDoubleLE",
        arity: 1,
        args: &["Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_read_double_be_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readDoubleBE",
        display_name: "readDoubleBE",
        length: 0,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_double_be_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 8,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Big,
    }
}

pub(crate) const fn buffer_read_double_be_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "readDoubleBE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferReadDoubleBE,
        fast_fn: "fast_buffer_read_f64be",
    })
}

pub(crate) const fn generated_buffer_read_double_be_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_read_double_be_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_read_double_be_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_read_double_be_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_read_double_be_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_read_double_be_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_double_be_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.readDoubleBE",
        receiver: "Buffer",
        property: "readDoubleBE",
        arity: 1,
        args: &["Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_write_float_le_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeFloatLE",
        display_name: "writeFloatLE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_float_le_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 4,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Little,
    }
}

pub(crate) const fn buffer_write_float_le_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "writeFloatLE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(2),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferWriteFloatLE,
        fast_fn: "fast_buffer_write_f32le",
    })
}

pub(crate) const fn generated_buffer_write_float_le_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_write_float_le_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_write_float_le_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_write_float_le_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_float_le_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_write_float_le_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_float_le_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.writeFloatLE",
        receiver: "Buffer",
        property: "writeFloatLE",
        arity: 2,
        args: &["Number", "Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_write_float_be_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeFloatBE",
        display_name: "writeFloatBE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_float_be_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 4,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Big,
    }
}

pub(crate) const fn buffer_write_float_be_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "writeFloatBE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(2),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferWriteFloatBE,
        fast_fn: "fast_buffer_write_f32be",
    })
}

pub(crate) const fn generated_buffer_write_float_be_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_write_float_be_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_write_float_be_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_write_float_be_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_float_be_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_write_float_be_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_float_be_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.writeFloatBE",
        receiver: "Buffer",
        property: "writeFloatBE",
        arity: 2,
        args: &["Number", "Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_write_double_le_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeDoubleLE",
        display_name: "writeDoubleLE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_double_le_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 8,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Little,
    }
}

pub(crate) const fn buffer_write_double_le_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "writeDoubleLE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(2),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferWriteDoubleLE,
        fast_fn: "fast_buffer_write_f64le",
    })
}

pub(crate) const fn generated_buffer_write_double_le_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_write_double_le_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_write_double_le_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_write_double_le_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_double_le_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_write_double_le_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_double_le_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.writeDoubleLE",
        receiver: "Buffer",
        property: "writeDoubleLE",
        arity: 2,
        args: &["Number", "Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_write_double_be_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeDoubleBE",
        display_name: "writeDoubleBE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_double_be_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 8,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Big,
    }
}

pub(crate) const fn buffer_write_double_be_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "writeDoubleBE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(2),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferWriteDoubleBE,
        fast_fn: "fast_buffer_write_f64be",
    })
}

pub(crate) const fn generated_buffer_write_double_be_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_write_double_be_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_write_double_be_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_write_double_be_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_double_be_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_write_double_be_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_double_be_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.writeDoubleBE",
        receiver: "Buffer",
        property: "writeDoubleBE",
        arity: 2,
        args: &["Number", "Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_write_int_be_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeIntBE",
        display_name: "writeIntBE",
        length: 3,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_int_be_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 3,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: false,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "byteLength 1..6",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Big,
    }
}

pub(crate) const fn buffer_write_int_be_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_write_int_be_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 3] {
    let spec = buffer_write_int_be_generated_validation_spec();
    debug_assert_eq!(spec.arity, 3);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
        args.get(2).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_int_be_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_write_int_be_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_int_be_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_write_int_le_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeIntLE",
        display_name: "writeIntLE",
        length: 3,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_int_le_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 3,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: false,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "byteLength 1..6",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Little,
    }
}

pub(crate) const fn buffer_write_int_le_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_write_int_le_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 3] {
    let spec = buffer_write_int_le_generated_validation_spec();
    debug_assert_eq!(spec.arity, 3);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
        args.get(2).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_int_le_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_write_int_le_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_int_le_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_write_uint_be_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeUIntBE",
        display_name: "writeUIntBE",
        length: 3,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_uint_be_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 3,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: false,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "byteLength 1..6",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::Big,
    }
}

pub(crate) const fn buffer_write_uint_be_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_write_uint_be_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 3] {
    let spec = buffer_write_uint_be_generated_validation_spec();
    debug_assert_eq!(spec.arity, 3);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
        args.get(2).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_uint_be_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_write_uint_be_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_uint_be_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_write_uint_le_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeUIntLE",
        display_name: "writeUIntLE",
        length: 3,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_uint_le_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 3,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: false,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "byteLength 1..6",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::Little,
    }
}

pub(crate) const fn buffer_write_uint_le_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_write_uint_le_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 3] {
    let spec = buffer_write_uint_le_generated_validation_spec();
    debug_assert_eq!(spec.arity, 3);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
        args.get(2).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_uint_le_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_write_uint_le_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_uint_le_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_read_int_be_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readIntBE",
        display_name: "readIntBE",
        length: 2,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_int_be_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: false,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "byteLength 1..6",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Big,
    }
}

pub(crate) const fn buffer_read_int_be_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_read_int_be_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_read_int_be_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_read_int_be_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_read_int_be_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_int_be_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_read_int_le_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readIntLE",
        display_name: "readIntLE",
        length: 2,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_int_le_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: false,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "byteLength 1..6",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Little,
    }
}

pub(crate) const fn buffer_read_int_le_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_read_int_le_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_read_int_le_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_read_int_le_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_read_int_le_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_int_le_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_read_uint_be_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readUIntBE",
        display_name: "readUIntBE",
        length: 2,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_uint_be_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: false,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "byteLength 1..6",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::Big,
    }
}

pub(crate) const fn buffer_read_uint_be_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_read_uint_be_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_read_uint_be_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_read_uint_be_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_read_uint_be_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_uint_be_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_read_uint_le_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readUIntLE",
        display_name: "readUIntLE",
        length: 2,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_uint_le_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: false,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "byteLength 1..6",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::Little,
    }
}

pub(crate) const fn buffer_read_uint_le_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_read_uint_le_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_read_uint_le_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_read_uint_le_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_read_uint_le_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_uint_le_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_read_big_uint64_le_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readBigUInt64LE",
        display_name: "readBigUInt64LE",
        length: 0,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_big_uint64_le_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::BigInt],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 8,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::Little,
    }
}

pub(crate) const fn buffer_read_big_uint64_le_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "readBigUInt64LE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferReadBigUInt64LE,
        fast_fn: "fast_buffer_read_big_u64le",
    })
}

pub(crate) const fn generated_buffer_read_big_uint64_le_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_read_big_uint64_le_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_read_big_uint64_le_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_read_big_uint64_le_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_read_big_uint64_le_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_read_big_uint64_le_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_big_uint64_le_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.readBigUInt64LE",
        receiver: "Buffer",
        property: "readBigUInt64LE",
        arity: 1,
        args: &["Number"],
        returns: &["BigInt"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_read_big_uint64_be_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readBigUInt64BE",
        display_name: "readBigUInt64BE",
        length: 0,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_big_uint64_be_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::BigInt],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 8,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::Big,
    }
}

pub(crate) const fn buffer_read_big_uint64_be_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "readBigUInt64BE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferReadBigUInt64BE,
        fast_fn: "fast_buffer_read_big_u64be",
    })
}

pub(crate) const fn generated_buffer_read_big_uint64_be_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_read_big_uint64_be_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_read_big_uint64_be_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_read_big_uint64_be_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_read_big_uint64_be_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_read_big_uint64_be_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_big_uint64_be_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.readBigUInt64BE",
        receiver: "Buffer",
        property: "readBigUInt64BE",
        arity: 1,
        args: &["Number"],
        returns: &["BigInt"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_read_big_int64_le_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readBigInt64LE",
        display_name: "readBigInt64LE",
        length: 0,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_big_int64_le_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::BigInt],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 8,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Little,
    }
}

pub(crate) const fn buffer_read_big_int64_le_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "readBigInt64LE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferReadBigInt64LE,
        fast_fn: "fast_buffer_read_big_i64le",
    })
}

pub(crate) const fn generated_buffer_read_big_int64_le_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_read_big_int64_le_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_read_big_int64_le_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_read_big_int64_le_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_read_big_int64_le_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_read_big_int64_le_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_big_int64_le_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.readBigInt64LE",
        receiver: "Buffer",
        property: "readBigInt64LE",
        arity: 1,
        args: &["Number"],
        returns: &["BigInt"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_read_big_int64_be_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "readBigInt64BE",
        display_name: "readBigInt64BE",
        length: 0,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_read_big_int64_be_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::BigInt],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 8,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Big,
    }
}

pub(crate) const fn buffer_read_big_int64_be_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "readBigInt64BE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(1),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferReadBigInt64BE,
        fast_fn: "fast_buffer_read_big_i64be",
    })
}

pub(crate) const fn generated_buffer_read_big_int64_be_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_read_big_int64_be_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_read_big_int64_be_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_read_big_int64_be_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_read_big_int64_be_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_read_big_int64_be_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_read_big_int64_be_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.readBigInt64BE",
        receiver: "Buffer",
        property: "readBigInt64BE",
        arity: 1,
        args: &["Number"],
        returns: &["BigInt"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_write_big_uint64_le_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeBigUInt64LE",
        display_name: "writeBigUInt64LE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_big_uint64_le_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::BigInt,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 8,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::Little,
    }
}

pub(crate) const fn buffer_write_big_uint64_le_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "writeBigUInt64LE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(2),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferWriteBigUInt64LE,
        fast_fn: "fast_buffer_write_big_u64le",
    })
}

pub(crate) const fn generated_buffer_write_big_uint64_le_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_write_big_uint64_le_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_write_big_uint64_le_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_write_big_uint64_le_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_big_uint64_le_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_write_big_uint64_le_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_big_uint64_le_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.writeBigUInt64LE",
        receiver: "Buffer",
        property: "writeBigUInt64LE",
        arity: 2,
        args: &["BigInt", "Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_write_big_uint64_be_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeBigUInt64BE",
        display_name: "writeBigUInt64BE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_big_uint64_be_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::BigInt,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 8,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::Big,
    }
}

pub(crate) const fn buffer_write_big_uint64_be_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "writeBigUInt64BE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(2),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferWriteBigUInt64BE,
        fast_fn: "fast_buffer_write_big_u64be",
    })
}

pub(crate) const fn generated_buffer_write_big_uint64_be_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_write_big_uint64_be_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_write_big_uint64_be_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_write_big_uint64_be_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_big_uint64_be_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_write_big_uint64_be_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_big_uint64_be_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.writeBigUInt64BE",
        receiver: "Buffer",
        property: "writeBigUInt64BE",
        arity: 2,
        args: &["BigInt", "Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_write_big_int64_le_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeBigInt64LE",
        display_name: "writeBigInt64LE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_big_int64_le_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::BigInt,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 8,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Little,
    }
}

pub(crate) const fn buffer_write_big_int64_le_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "writeBigInt64LE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(2),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferWriteBigInt64LE,
        fast_fn: "fast_buffer_write_big_i64le",
    })
}

pub(crate) const fn generated_buffer_write_big_int64_le_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_write_big_int64_le_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_write_big_int64_le_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_write_big_int64_le_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_big_int64_le_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_write_big_int64_le_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_big_int64_le_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.writeBigInt64LE",
        receiver: "Buffer",
        property: "writeBigInt64LE",
        arity: 2,
        args: &["BigInt", "Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_write_big_int64_be_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "writeBigInt64BE",
        display_name: "writeBigInt64BE",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_big_int64_be_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::BigInt,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "fixed",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 8,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::Big,
    }
}

pub(crate) const fn buffer_write_big_int64_be_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    Some(crate::native_api_manifest::GeneratedIhiSpec {
        key: "writeBigInt64BE",
        receiver: crate::interp_ic_table::IhiReceiverKind::Buffer,
        arity: Some(2),
        cached_id_field: crate::interp_ic_table::IhiCachedField::BufferWriteBigInt64BE,
        fast_fn: "fast_buffer_write_big_i64be",
    })
}

pub(crate) const fn generated_buffer_write_big_int64_be_ihi_entry(
    fast: fn(&mut crate::Runtime, &crate::Value, &[crate::Value]) -> Option<crate::Value>,
) -> crate::interp_ic_table::IhiEntry {
    let spec = match buffer_write_big_int64_be_generated_ihi_spec() {
        Some(spec) => spec,
        None => panic!("generated IHI entry requested for a row without IHI facts"),
    };
    crate::interp_ic_table::IhiEntry {
        key: spec.key,
        receiver: spec.receiver,
        arity: spec.arity,
        cached_id_field: spec.cached_id_field,
        fast,
    }
}

pub(crate) fn buffer_write_big_int64_be_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_write_big_int64_be_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_big_int64_be_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_write_big_int64_be_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_big_int64_be_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    Some(GeneratedCruftScriptStdlibSignatureSpec {
        api: "Buffer.prototype.writeBigInt64BE",
        receiver: "Buffer",
        property: "writeBigInt64BE",
        arity: 2,
        args: &["BigInt", "Number"],
        returns: &["Number"],
        nullish_receiver_rejects: true,
        boundary_safe: true,
    })
}

pub(crate) const fn buffer_to_string_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "toString",
        display_name: "toString",
        length: 3,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_to_string_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 3,
        args: &[
            crate::native_api_manifest::NativeApiDomain::String,
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::String],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "optional-overload:encoding,start,end",
        encoding_policy: "strict-label-or-ERR_UNKNOWN_ENCODING",
        range_window: "start/end clamp to receiver byteLength",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &["ERR_UNKNOWN_ENCODING"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn buffer_to_string_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_to_string_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 3] {
    let spec = buffer_to_string_generated_validation_spec();
    debug_assert_eq!(spec.arity, 3);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
        args.get(2).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_to_string_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_to_string_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_to_string_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_write_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "write",
        display_name: "write",
        length: 4,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_write_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 4,
        args: &[
            crate::native_api_manifest::NativeApiDomain::String,
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::String,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "optional-overload:string,offset,length,encoding",
        encoding_policy: "strict-label-or-ERR_UNKNOWN_ENCODING",
        range_window: "offset/length bounded by receiver byteLength",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &["ERR_UNKNOWN_ENCODING"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn buffer_write_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_write_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 4] {
    let spec = buffer_write_generated_validation_spec();
    debug_assert_eq!(spec.arity, 4);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
        args.get(2).cloned().unwrap_or(crate::Value::Undefined),
        args.get(3).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_write_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_write_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_write_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_static_byte_length_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "byteLength",
        display_name: "byteLength",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: true,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_static_byte_length_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::String,
            crate::native_api_manifest::NativeApiDomain::String,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "string-or-buffer-arraybuffer-view,optional-encoding",
        encoding_policy: "nominal-size-unknown-falls-back-to-utf8",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &["ERR_INVALID_ARG_TYPE"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn buffer_static_byte_length_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_static_byte_length_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_static_byte_length_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_static_byte_length_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_static_byte_length_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_static_byte_length_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_static_is_encoding_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "isEncoding",
        display_name: "isEncoding",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: true,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_static_is_encoding_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::String],
        returns: &[crate::native_api_manifest::NativeApiDomain::Boolean],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "encoding-label-or-non-string-false",
        encoding_policy: "label-membership-predicate",
        range_window: "none",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn buffer_static_is_encoding_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_static_is_encoding_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_static_is_encoding_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_static_is_encoding_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_static_is_encoding_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_static_is_encoding_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_static_from_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "from",
        display_name: "from",
        length: 3,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: true,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_static_from_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 3,
        args: &[crate::native_api_manifest::NativeApiDomain::String, crate::native_api_manifest::NativeApiDomain::String, crate::native_api_manifest::NativeApiDomain::Undefined],
        returns: &[crate::native_api_manifest::NativeApiDomain::Buffer],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "string/array/array-like/ArrayBuffer/TypedArray/DataView/json-buffer, optional encoding/window overloads",
        encoding_policy: "strict-label-or-ERR_UNKNOWN_ENCODING",
        range_window: "allocates new Buffer except ArrayBuffer overload shares byte window",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[
            "ERR_UNKNOWN_ENCODING",
            "ERR_INVALID_ARG_TYPE",
        ],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn buffer_static_from_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_static_from_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 3] {
    let spec = buffer_static_from_generated_validation_spec();
    debug_assert_eq!(spec.arity, 3);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
        args.get(2).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_static_from_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_static_from_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_static_from_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_includes_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "includes",
        display_name: "includes",
        length: 4,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_includes_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 3,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Buffer,
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::String,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Boolean],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape:
            "number|string|Buffer|Uint8Array needle, optional offset, optional encoding",
        encoding_policy: "string needles use strict label or ERR_UNKNOWN_ENCODING",
        range_window: "relative search offset, negative-from-end",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &["ERR_UNKNOWN_ENCODING", "ERR_INVALID_ARG_TYPE"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn buffer_includes_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_includes_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 3] {
    let spec = buffer_includes_generated_validation_spec();
    debug_assert_eq!(spec.arity, 3);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
        args.get(2).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_includes_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_includes_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_includes_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_index_of_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "indexOf",
        display_name: "indexOf",
        length: 4,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_index_of_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 3,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Buffer,
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::String,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape:
            "number|string|Buffer|Uint8Array needle, optional offset, optional encoding",
        encoding_policy: "string needles use strict label or ERR_UNKNOWN_ENCODING",
        range_window: "relative search offset, negative-from-end, -1 on miss",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &["ERR_UNKNOWN_ENCODING", "ERR_INVALID_ARG_TYPE"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn buffer_index_of_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_index_of_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 3] {
    let spec = buffer_index_of_generated_validation_spec();
    debug_assert_eq!(spec.arity, 3);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
        args.get(2).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_index_of_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_index_of_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_index_of_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_last_index_of_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "lastIndexOf",
        display_name: "lastIndexOf",
        length: 4,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_last_index_of_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 3,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Buffer,
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::String,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape:
            "number|string|Buffer|Uint8Array needle, optional offset, optional encoding",
        encoding_policy: "string needles use strict label or ERR_UNKNOWN_ENCODING",
        range_window: "reverse relative search offset, negative-from-end, -1 on miss",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &["ERR_UNKNOWN_ENCODING", "ERR_INVALID_ARG_TYPE"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn buffer_last_index_of_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_last_index_of_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 3] {
    let spec = buffer_last_index_of_generated_validation_spec();
    debug_assert_eq!(spec.arity, 3);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
        args.get(2).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_last_index_of_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_last_index_of_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_last_index_of_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_equals_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "equals",
        display_name: "equals",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_equals_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Buffer],
        returns: &[crate::native_api_manifest::NativeApiDomain::Boolean],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "Buffer-or-Uint8Array target only",
        encoding_policy: "none",
        range_window: "full-buffer byte equality",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &["ERR_INVALID_ARG_TYPE"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn buffer_equals_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_equals_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = buffer_equals_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_equals_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_equals_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_equals_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_compare_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "compare",
        display_name: "compare",
        length: 5,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_compare_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 5,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Buffer,
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "Buffer-or-Uint8Array target plus optional target/source windows",
        encoding_policy: "none",
        range_window: "targetStart,targetEnd,sourceStart,sourceEnd byte windows",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &["ERR_INVALID_ARG_TYPE"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn buffer_compare_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_compare_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 5] {
    let spec = buffer_compare_generated_validation_spec();
    debug_assert_eq!(spec.arity, 5);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
        args.get(2).cloned().unwrap_or(crate::Value::Undefined),
        args.get(3).cloned().unwrap_or(crate::Value::Undefined),
        args.get(4).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_compare_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_compare_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_compare_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_static_compare_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "compare",
        display_name: "compare",
        length: 2,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: true,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_static_compare_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Buffer,
            crate::native_api_manifest::NativeApiDomain::Buffer,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "two Buffer-or-Uint8Array operands",
        encoding_policy: "none",
        range_window: "full-buffer byte lexicographic order",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &["ERR_INVALID_ARG_TYPE"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn buffer_static_compare_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_static_compare_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_static_compare_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_static_compare_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_static_compare_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_static_compare_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_copy_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "copy",
        display_name: "copy",
        length: 4,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_copy_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 4,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Buffer,
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "target Buffer/Uint8Array plus targetStart, sourceStart, sourceEnd",
        encoding_policy: "none",
        range_window: "target/source byte windows, clamped copy count",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &["ERR_INVALID_ARG_TYPE"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn buffer_copy_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_copy_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 4] {
    let spec = buffer_copy_generated_validation_spec();
    debug_assert_eq!(spec.arity, 4);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
        args.get(2).cloned().unwrap_or(crate::Value::Undefined),
        args.get(3).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_copy_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_copy_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_copy_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_fill_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "fill",
        display_name: "fill",
        length: 4,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_fill_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 4,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Buffer,
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::String,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Buffer],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "number|string|Buffer|Uint8Array value plus offset,end,encoding",
        encoding_policy: "string fill uses strict label or ERR_UNKNOWN_ENCODING",
        range_window: "offset/end byte window, fill bytes cycle",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &["ERR_UNKNOWN_ENCODING", "ERR_INVALID_ARG_TYPE"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn buffer_fill_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_fill_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 4] {
    let spec = buffer_fill_generated_validation_spec();
    debug_assert_eq!(spec.arity, 4);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
        args.get(2).cloned().unwrap_or(crate::Value::Undefined),
        args.get(3).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_fill_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_fill_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_fill_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_slice_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "slice",
        display_name: "slice",
        length: 2,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_slice_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Buffer],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "start,end relative indexes",
        encoding_policy: "none",
        range_window: "returns Buffer view over byte window; negative offsets clamp",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn buffer_slice_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_slice_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 2] {
    let spec = buffer_slice_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_slice_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_slice_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_slice_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_subarray_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "subarray",
        display_name: "subarray",
        length: 2,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_subarray_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Buffer],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "start,end relative indexes",
        encoding_policy: "none",
        range_window: "returns Buffer view over byte window; negative offsets clamp",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn buffer_subarray_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_subarray_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_subarray_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_subarray_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_subarray_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_subarray_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_static_alloc_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "alloc",
        display_name: "alloc",
        length: 3,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: true,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_static_alloc_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 3,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Buffer,
            crate::native_api_manifest::NativeApiDomain::String,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Buffer],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "size, optional fill value, optional encoding",
        encoding_policy: "string fill uses strict label or ERR_UNKNOWN_ENCODING",
        range_window: "allocates zero-filled or fill-cycled Buffer",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[
            "ERR_OUT_OF_RANGE",
            "ERR_INVALID_ARG_TYPE",
            "ERR_UNKNOWN_ENCODING",
        ],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn buffer_static_alloc_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_static_alloc_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 3] {
    let spec = buffer_static_alloc_generated_validation_spec();
    debug_assert_eq!(spec.arity, 3);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
        args.get(2).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_static_alloc_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_static_alloc_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_static_alloc_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_static_alloc_unsafe_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "allocUnsafe",
        display_name: "allocUnsafe",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: true,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_static_alloc_unsafe_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Buffer],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "size only",
        encoding_policy: "none",
        range_window: "allocates Buffer of requested byte length",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &["ERR_OUT_OF_RANGE", "ERR_INVALID_ARG_TYPE"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn buffer_static_alloc_unsafe_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_static_alloc_unsafe_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_static_alloc_unsafe_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_static_alloc_unsafe_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_static_alloc_unsafe_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_static_alloc_unsafe_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_static_alloc_unsafe_slow_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "allocUnsafeSlow",
        display_name: "allocUnsafeSlow",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: true,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_static_alloc_unsafe_slow_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Buffer],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "size only",
        encoding_policy: "none",
        range_window: "allocates Buffer of requested byte length",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &["ERR_OUT_OF_RANGE", "ERR_INVALID_ARG_TYPE"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn buffer_static_alloc_unsafe_slow_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_static_alloc_unsafe_slow_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_static_alloc_unsafe_slow_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_static_alloc_unsafe_slow_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_static_alloc_unsafe_slow_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_static_alloc_unsafe_slow_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_static_concat_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "concat",
        display_name: "concat",
        length: 2,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: true,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_static_concat_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Buffer,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Buffer],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "array of Buffer/Uint8Array plus optional totalLength",
        encoding_policy: "none",
        range_window: "concatenates, truncates, or zero-fills to totalLength",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &["ERR_INVALID_ARG_TYPE"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn buffer_static_concat_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_static_concat_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = buffer_static_concat_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn buffer_static_concat_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_static_concat_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_static_concat_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_static_is_buffer_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "isBuffer",
        display_name: "isBuffer",
        length: 1,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: true,
        function_prototype: true,
    }
}

pub(crate) const fn buffer_static_is_buffer_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Buffer],
        returns: &[crate::native_api_manifest::NativeApiDomain::Boolean],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "single value brand predicate",
        encoding_policy: "none",
        range_window: "true only for runtime Buffer brand",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn buffer_static_is_buffer_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_static_is_buffer_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = buffer_static_is_buffer_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn buffer_static_is_buffer_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_static_is_buffer_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_static_is_buffer_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn buffer_static_of_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "of",
        display_name: "of",
        length: 0,
        writable: true,
        enumerable: true,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn buffer_static_of_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Buffer,
        arity: 0,
        args: &[],
        returns: &[crate::native_api_manifest::NativeApiDomain::Buffer],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "variadic byte values",
        encoding_policy: "none",
        range_window: "allocates Buffer from ToUint8 argument bytes",
        receiver_semantics: "ordinary",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn buffer_static_of_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn buffer_static_of_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 0] {
    let spec = buffer_static_of_generated_validation_spec();
    debug_assert_eq!(spec.arity, 0);
    []
}

pub(crate) const fn buffer_static_of_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn buffer_static_of_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn buffer_static_of_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_static_is_array_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "isArray",
        display_name: "isArray",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_static_is_array_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Boolean],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "array-scalar-search",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "array-brand",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_static_is_array_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_static_is_array_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = array_static_is_array_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_static_is_array_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_static_is_array_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_static_is_array_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_at_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "at",
        display_name: "at",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_at_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[
            crate::native_api_manifest::NativeApiDomain::Unknown,
            crate::native_api_manifest::NativeApiDomain::Undefined,
        ],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::Undefined,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::Undefined,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "array-scalar-search",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "array-like",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_at_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_at_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = array_at_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_at_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_at_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_at_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_includes_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "includes",
        display_name: "includes",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_includes_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Boolean],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "array-scalar-search",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "array-like",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_includes_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_includes_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = array_includes_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_includes_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_includes_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_includes_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_index_of_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "indexOf",
        display_name: "indexOf",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_index_of_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "array-scalar-search",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "array-like",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_index_of_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_index_of_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = array_index_of_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_index_of_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_index_of_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_index_of_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_last_index_of_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "lastIndexOf",
        display_name: "lastIndexOf",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_last_index_of_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "array-scalar-search",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "array-like",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_last_index_of_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_last_index_of_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = array_last_index_of_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_last_index_of_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_last_index_of_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_last_index_of_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_find_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "find",
        display_name: "find",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_find_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[
            crate::native_api_manifest::NativeApiDomain::Unknown,
            crate::native_api_manifest::NativeApiDomain::Undefined,
        ],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "array-scalar-search",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "array-like",
        callback_policy: "predicate-forward",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_find_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_find_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = array_find_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_find_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_find_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_find_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_find_index_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "findIndex",
        display_name: "findIndex",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_find_index_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "array-scalar-search",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "array-like",
        callback_policy: "predicate-forward",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_find_index_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_find_index_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = array_find_index_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_find_index_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_find_index_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_find_index_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_find_last_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "findLast",
        display_name: "findLast",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_find_last_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[
            crate::native_api_manifest::NativeApiDomain::Unknown,
            crate::native_api_manifest::NativeApiDomain::Undefined,
        ],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "array-scalar-search",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "array-like",
        callback_policy: "predicate-reverse",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_find_last_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_find_last_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = array_find_last_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_find_last_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_find_last_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_find_last_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_find_last_index_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "findLastIndex",
        display_name: "findLastIndex",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_find_last_index_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "array-scalar-search",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "array-like",
        callback_policy: "predicate-reverse",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_find_last_index_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_find_last_index_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = array_find_last_index_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_find_last_index_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_find_last_index_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_find_last_index_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_for_each_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "forEach",
        display_name: "forEach",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_for_each_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Undefined],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "array-iteration-callback",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "array-like",
        callback_policy: "iteration-forward-side-effect",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_for_each_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_for_each_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = array_for_each_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_for_each_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_for_each_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_for_each_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_map_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "map",
        display_name: "map",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_map_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "array-iteration-callback",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "array-like",
        callback_policy: "iteration-forward-map",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_map_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_map_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = array_map_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_map_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_map_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_map_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_filter_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "filter",
        display_name: "filter",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_filter_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "array-iteration-callback",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "array-like",
        callback_policy: "iteration-forward-filter",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_filter_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_filter_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = array_filter_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_filter_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_filter_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_filter_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_some_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "some",
        display_name: "some",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_some_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Boolean],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "array-iteration-callback",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "array-like",
        callback_policy: "iteration-forward-some",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_some_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_some_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = array_some_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_some_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_some_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_some_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_every_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "every",
        display_name: "every",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_every_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Boolean],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "array-iteration-callback",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "array-like",
        callback_policy: "iteration-forward-every",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_every_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_every_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = array_every_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_every_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_every_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_every_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_reduce_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "reduce",
        display_name: "reduce",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_reduce_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "array-iteration-callback",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "array-like",
        callback_policy: "reducer-forward",
        error_codes: &[
            "TypeError:callback-not-callable",
            "TypeError:empty-without-initial-value",
        ],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_reduce_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_reduce_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = array_reduce_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_reduce_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_reduce_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_reduce_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_reduce_right_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "reduceRight",
        display_name: "reduceRight",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_reduce_right_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "array-iteration-callback",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "array-like",
        callback_policy: "reducer-reverse",
        error_codes: &[
            "TypeError:callback-not-callable",
            "TypeError:empty-without-initial-value",
        ],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_reduce_right_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_reduce_right_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = array_reduce_right_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_reduce_right_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_reduce_right_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_reduce_right_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_flat_map_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "flatMap",
        display_name: "flatMap",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_flat_map_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "array-iteration-callback",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "array-like",
        callback_policy: "iteration-forward-flat-map",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_flat_map_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_flat_map_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = array_flat_map_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_flat_map_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_flat_map_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_flat_map_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_push_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "push",
        display_name: "push",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_push_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "array-length-mutating",
        encoding_policy: "none",
        range_window: "length-append",
        receiver_semantics: "array-like",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_push_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_push_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = array_push_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_push_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_push_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_push_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_pop_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "pop",
        display_name: "pop",
        length: 0,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_pop_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 0,
        args: &[],
        returns: &[
            crate::native_api_manifest::NativeApiDomain::Unknown,
            crate::native_api_manifest::NativeApiDomain::Undefined,
        ],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::Undefined,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "array-length-mutating",
        encoding_policy: "none",
        range_window: "length-tail-delete",
        receiver_semantics: "array-like",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_pop_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_pop_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 0] {
    let spec = array_pop_generated_validation_spec();
    debug_assert_eq!(spec.arity, 0);
    []
}

pub(crate) const fn array_pop_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_pop_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_pop_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_shift_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "shift",
        display_name: "shift",
        length: 0,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_shift_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 0,
        args: &[],
        returns: &[
            crate::native_api_manifest::NativeApiDomain::Unknown,
            crate::native_api_manifest::NativeApiDomain::Undefined,
        ],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::Undefined,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "array-length-mutating",
        encoding_policy: "none",
        range_window: "length-head-shift",
        receiver_semantics: "array-like",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_shift_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_shift_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 0] {
    let spec = array_shift_generated_validation_spec();
    debug_assert_eq!(spec.arity, 0);
    []
}

pub(crate) const fn array_shift_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_shift_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_shift_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_unshift_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "unshift",
        display_name: "unshift",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_unshift_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "array-length-mutating",
        encoding_policy: "none",
        range_window: "length-head-insert",
        receiver_semantics: "array-like",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_unshift_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_unshift_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = array_unshift_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_unshift_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_unshift_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_unshift_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_reverse_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "reverse",
        display_name: "reverse",
        length: 0,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_reverse_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 0,
        args: &[],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "array-inplace-mutation",
        encoding_policy: "none",
        range_window: "whole-length",
        receiver_semantics: "array-like",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_reverse_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_reverse_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 0] {
    let spec = array_reverse_generated_validation_spec();
    debug_assert_eq!(spec.arity, 0);
    []
}

pub(crate) const fn array_reverse_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_reverse_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_reverse_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_splice_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "splice",
        display_name: "splice",
        length: 2,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_splice_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::ClampedIndex,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "array-structural-edit",
        encoding_policy: "none",
        range_window: "start-delete-insert",
        receiver_semantics: "array-like",
        callback_policy: "none",
        error_codes: &["TypeError:length-limit"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_splice_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_splice_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 2] {
    let spec = array_splice_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn array_splice_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_splice_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_splice_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_copy_within_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "copyWithin",
        display_name: "copyWithin",
        length: 2,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_copy_within_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::ClampedIndex,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "array-inplace-mutation",
        encoding_policy: "none",
        range_window: "target-start-end",
        receiver_semantics: "array-like",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_copy_within_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_copy_within_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = array_copy_within_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn array_copy_within_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_copy_within_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_copy_within_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_fill_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "fill",
        display_name: "fill",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_fill_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::ClampedIndex,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "array-inplace-mutation",
        encoding_policy: "none",
        range_window: "start-end-fill",
        receiver_semantics: "array-like",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_fill_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_fill_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = array_fill_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_fill_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_fill_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_fill_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_slice_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "slice",
        display_name: "slice",
        length: 2,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_slice_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::ClampedIndex,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "array-allocation-species",
        encoding_policy: "none",
        range_window: "start-end-copy",
        receiver_semantics: "array-like",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_slice_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_slice_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 2] {
    let spec = array_slice_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn array_slice_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_slice_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_slice_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_concat_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "concat",
        display_name: "concat",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_concat_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "array-allocation-species",
        encoding_policy: "none",
        range_window: "concat-spread",
        receiver_semantics: "array-like",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_concat_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_concat_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = array_concat_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_concat_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_concat_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_concat_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_sort_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "sort",
        display_name: "sort",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_sort_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "array-sort-comparator",
        encoding_policy: "none",
        range_window: "whole-length-sort",
        receiver_semantics: "array-like",
        callback_policy: "sort-compare",
        error_codes: &[
            "TypeError:compare-not-callable",
            "TypeError:compare-result-symbol",
        ],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_sort_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_sort_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = array_sort_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_sort_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_sort_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_sort_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_flat_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "flat",
        display_name: "flat",
        length: 0,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_flat_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::Array,
        arity: 0,
        args: &[],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::ClampedIndex,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "array-flattening",
        encoding_policy: "none",
        range_window: "depth-flatten",
        receiver_semantics: "array-like",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_flat_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_flat_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 0] {
    let spec = array_flat_generated_validation_spec();
    debug_assert_eq!(spec.arity, 0);
    []
}

pub(crate) const fn array_flat_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_flat_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_flat_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_get_int8_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "getInt8",
        display_name: "getInt8",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_get_int8_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "dataview-byte-get",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
        ],
        byte_width: 1,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::SingleByte,
    }
}

pub(crate) const fn data_view_get_int8_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_get_int8_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = data_view_get_int8_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn data_view_get_int8_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_get_int8_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_get_int8_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_get_uint8_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "getUint8",
        display_name: "getUint8",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_get_uint8_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "dataview-byte-get",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
        ],
        byte_width: 1,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::SingleByte,
    }
}

pub(crate) const fn data_view_get_uint8_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_get_uint8_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = data_view_get_uint8_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn data_view_get_uint8_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_get_uint8_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_get_uint8_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_set_int8_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "setInt8",
        display_name: "setInt8",
        length: 2,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_set_int8_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Undefined],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "dataview-byte-set",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
        ],
        byte_width: 1,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::SingleByte,
    }
}

pub(crate) const fn data_view_set_int8_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_set_int8_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = data_view_set_int8_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn data_view_set_int8_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_set_int8_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_set_int8_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_set_uint8_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "setUint8",
        display_name: "setUint8",
        length: 2,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_set_uint8_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Undefined],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "dataview-byte-set",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
        ],
        byte_width: 1,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::SingleByte,
    }
}

pub(crate) const fn data_view_set_uint8_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_set_uint8_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = data_view_set_uint8_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn data_view_set_uint8_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_set_uint8_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_set_uint8_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_get_int16_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "getInt16",
        display_name: "getInt16",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_get_int16_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "dataview-endian-get",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
        ],
        byte_width: 2,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::RuntimeFlag,
    }
}

pub(crate) const fn data_view_get_int16_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_get_int16_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = data_view_get_int16_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn data_view_get_int16_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_get_int16_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_get_int16_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_get_uint16_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "getUint16",
        display_name: "getUint16",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_get_uint16_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "dataview-endian-get",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
        ],
        byte_width: 2,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::RuntimeFlag,
    }
}

pub(crate) const fn data_view_get_uint16_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_get_uint16_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = data_view_get_uint16_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn data_view_get_uint16_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_get_uint16_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_get_uint16_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_set_int16_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "setInt16",
        display_name: "setInt16",
        length: 2,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_set_int16_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Undefined],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "dataview-endian-set",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
        ],
        byte_width: 2,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::RuntimeFlag,
    }
}

pub(crate) const fn data_view_set_int16_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_set_int16_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = data_view_set_int16_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn data_view_set_int16_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_set_int16_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_set_int16_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_set_uint16_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "setUint16",
        display_name: "setUint16",
        length: 2,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_set_uint16_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Undefined],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "dataview-endian-set",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
        ],
        byte_width: 2,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::RuntimeFlag,
    }
}

pub(crate) const fn data_view_set_uint16_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_set_uint16_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = data_view_set_uint16_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn data_view_set_uint16_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_set_uint16_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_set_uint16_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_get_int32_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "getInt32",
        display_name: "getInt32",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_get_int32_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "dataview-endian-get",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
        ],
        byte_width: 4,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::RuntimeFlag,
    }
}

pub(crate) const fn data_view_get_int32_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_get_int32_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = data_view_get_int32_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn data_view_get_int32_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_get_int32_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_get_int32_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_get_uint32_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "getUint32",
        display_name: "getUint32",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_get_uint32_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "dataview-endian-get",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
        ],
        byte_width: 4,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::RuntimeFlag,
    }
}

pub(crate) const fn data_view_get_uint32_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_get_uint32_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = data_view_get_uint32_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn data_view_get_uint32_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_get_uint32_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_get_uint32_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_set_int32_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "setInt32",
        display_name: "setInt32",
        length: 2,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_set_int32_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Undefined],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "dataview-endian-set",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
        ],
        byte_width: 4,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::RuntimeFlag,
    }
}

pub(crate) const fn data_view_set_int32_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_set_int32_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = data_view_set_int32_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn data_view_set_int32_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_set_int32_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_set_int32_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_set_uint32_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "setUint32",
        display_name: "setUint32",
        length: 2,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_set_uint32_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Undefined],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "dataview-endian-set",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
        ],
        byte_width: 4,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::RuntimeFlag,
    }
}

pub(crate) const fn data_view_set_uint32_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_set_uint32_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = data_view_set_uint32_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn data_view_set_uint32_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_set_uint32_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_set_uint32_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_get_float16_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "getFloat16",
        display_name: "getFloat16",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_get_float16_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "dataview-endian-get",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
        ],
        byte_width: 2,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::RuntimeFlag,
    }
}

pub(crate) const fn data_view_get_float16_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_get_float16_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = data_view_get_float16_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn data_view_get_float16_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_get_float16_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_get_float16_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_get_float32_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "getFloat32",
        display_name: "getFloat32",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_get_float32_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "dataview-endian-get",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
        ],
        byte_width: 4,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::RuntimeFlag,
    }
}

pub(crate) const fn data_view_get_float32_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_get_float32_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = data_view_get_float32_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn data_view_get_float32_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_get_float32_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_get_float32_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_get_float64_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "getFloat64",
        display_name: "getFloat64",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_get_float64_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "dataview-endian-get",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
        ],
        byte_width: 8,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::RuntimeFlag,
    }
}

pub(crate) const fn data_view_get_float64_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_get_float64_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = data_view_get_float64_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn data_view_get_float64_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_get_float64_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_get_float64_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_set_float16_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "setFloat16",
        display_name: "setFloat16",
        length: 2,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_set_float16_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Undefined],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "dataview-endian-set",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
        ],
        byte_width: 2,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::RuntimeFlag,
    }
}

pub(crate) const fn data_view_set_float16_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_set_float16_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = data_view_set_float16_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn data_view_set_float16_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_set_float16_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_set_float16_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_set_float32_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "setFloat32",
        display_name: "setFloat32",
        length: 2,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_set_float32_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Undefined],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "dataview-endian-set",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
        ],
        byte_width: 4,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::RuntimeFlag,
    }
}

pub(crate) const fn data_view_set_float32_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_set_float32_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = data_view_set_float32_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn data_view_set_float32_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_set_float32_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_set_float32_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_set_float64_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "setFloat64",
        display_name: "setFloat64",
        length: 2,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_set_float64_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Undefined],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "dataview-endian-set",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
        ],
        byte_width: 8,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::RuntimeFlag,
    }
}

pub(crate) const fn data_view_set_float64_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_set_float64_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = data_view_set_float64_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn data_view_set_float64_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_set_float64_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_set_float64_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_get_big_int64_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "getBigInt64",
        display_name: "getBigInt64",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_get_big_int64_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::BigInt],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "dataview-endian-get",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
        ],
        byte_width: 8,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::RuntimeFlag,
    }
}

pub(crate) const fn data_view_get_big_int64_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_get_big_int64_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = data_view_get_big_int64_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn data_view_get_big_int64_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_get_big_int64_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_get_big_int64_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_get_big_uint64_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "getBigUint64",
        display_name: "getBigUint64",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_get_big_uint64_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::BigInt],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "dataview-endian-get",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
        ],
        byte_width: 8,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::RuntimeFlag,
    }
}

pub(crate) const fn data_view_get_big_uint64_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_get_big_uint64_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = data_view_get_big_uint64_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn data_view_get_big_uint64_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_get_big_uint64_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_get_big_uint64_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_set_big_int64_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "setBigInt64",
        display_name: "setBigInt64",
        length: 2,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_set_big_int64_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::BigInt,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Undefined],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "dataview-endian-set",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
            "TypeError:bigint-value-required",
        ],
        byte_width: 8,
        signed: true,
        endian: crate::native_api_manifest::NativeApiEndian::RuntimeFlag,
    }
}

pub(crate) const fn data_view_set_big_int64_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_set_big_int64_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = data_view_set_big_int64_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn data_view_set_big_int64_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_set_big_int64_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_set_big_int64_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn data_view_set_big_uint64_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "setBigUint64",
        display_name: "setBigUint64",
        length: 2,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn data_view_set_big_uint64_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::DataView,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::BigInt,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Undefined],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "dataview-endian-set",
        encoding_policy: "none",
        range_window: "byte-offset",
        receiver_semantics: "dataview-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "RangeError:offset-out-of-range",
            "TypeError:detached-buffer",
            "TypeError:bigint-value-required",
        ],
        byte_width: 8,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::RuntimeFlag,
    }
}

pub(crate) const fn data_view_set_big_uint64_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn data_view_set_big_uint64_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = data_view_set_big_uint64_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn data_view_set_big_uint64_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn data_view_set_big_uint64_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn data_view_set_big_uint64_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn typed_array_at_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "at",
        display_name: "at",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn typed_array_at_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::TypedArray,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::BigInt,
            crate::native_api_manifest::NativeApiDomain::Undefined,
        ],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::Undefined,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::Undefined,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "typedarray-relative-index",
        encoding_policy: "none",
        range_window: "element-index",
        receiver_semantics: "typedarray-brand-detach",
        callback_policy: "none",
        error_codes: &["TypeError:invalid-receiver", "TypeError:detached-buffer"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn typed_array_at_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn typed_array_at_generated_validation_args(args: &[crate::Value]) -> [crate::Value; 1] {
    let spec = typed_array_at_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn typed_array_at_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn typed_array_at_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn typed_array_at_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn typed_array_includes_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "includes",
        display_name: "includes",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn typed_array_includes_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::TypedArray,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Boolean],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "typedarray-search-value-from-index",
        encoding_policy: "none",
        range_window: "element-index",
        receiver_semantics: "typedarray-brand-detach",
        callback_policy: "none",
        error_codes: &["TypeError:invalid-receiver", "TypeError:detached-buffer"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn typed_array_includes_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn typed_array_includes_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = typed_array_includes_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn typed_array_includes_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn typed_array_includes_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn typed_array_includes_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn typed_array_index_of_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "indexOf",
        display_name: "indexOf",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn typed_array_index_of_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::TypedArray,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "typedarray-search-value-from-index",
        encoding_policy: "none",
        range_window: "element-index",
        receiver_semantics: "typedarray-brand-detach",
        callback_policy: "none",
        error_codes: &["TypeError:invalid-receiver", "TypeError:detached-buffer"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn typed_array_index_of_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn typed_array_index_of_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = typed_array_index_of_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn typed_array_index_of_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn typed_array_index_of_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn typed_array_index_of_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn typed_array_last_index_of_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "lastIndexOf",
        display_name: "lastIndexOf",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn typed_array_last_index_of_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::TypedArray,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Number],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "typedarray-search-value-from-index",
        encoding_policy: "none",
        range_window: "element-index",
        receiver_semantics: "typedarray-brand-detach",
        callback_policy: "none",
        error_codes: &["TypeError:invalid-receiver", "TypeError:detached-buffer"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn typed_array_last_index_of_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn typed_array_last_index_of_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = typed_array_last_index_of_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn typed_array_last_index_of_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn typed_array_last_index_of_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn typed_array_last_index_of_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn typed_array_fill_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "fill",
        display_name: "fill",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn typed_array_fill_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::TypedArray,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "typedarray-value-start-end",
        encoding_policy: "none",
        range_window: "element-index",
        receiver_semantics: "typedarray-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "TypeError:detached-buffer",
            "TypeError:bigint-value-required",
        ],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn typed_array_fill_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn typed_array_fill_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = typed_array_fill_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn typed_array_fill_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn typed_array_fill_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn typed_array_fill_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn typed_array_reverse_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "reverse",
        display_name: "reverse",
        length: 0,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn typed_array_reverse_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::TypedArray,
        arity: 0,
        args: &[],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: false,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "typedarray-in-place-reorder",
        encoding_policy: "none",
        range_window: "element-index",
        receiver_semantics: "typedarray-brand-detach",
        callback_policy: "none",
        error_codes: &["TypeError:invalid-receiver", "TypeError:detached-buffer"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn typed_array_reverse_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn typed_array_reverse_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 0] {
    let spec = typed_array_reverse_generated_validation_spec();
    debug_assert_eq!(spec.arity, 0);
    []
}

pub(crate) const fn typed_array_reverse_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn typed_array_reverse_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn typed_array_reverse_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn typed_array_copy_within_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "copyWithin",
        display_name: "copyWithin",
        length: 2,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn typed_array_copy_within_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::TypedArray,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "typedarray-target-start-end",
        encoding_policy: "none",
        range_window: "element-index",
        receiver_semantics: "typedarray-brand-detach",
        callback_policy: "none",
        error_codes: &["TypeError:invalid-receiver", "TypeError:detached-buffer"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn typed_array_copy_within_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn typed_array_copy_within_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = typed_array_copy_within_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn typed_array_copy_within_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn typed_array_copy_within_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn typed_array_copy_within_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn typed_array_set_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "set",
        display_name: "set",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn typed_array_set_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::TypedArray,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Undefined],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "typedarray-source-offset",
        encoding_policy: "none",
        range_window: "element-index",
        receiver_semantics: "typedarray-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "TypeError:detached-buffer",
            "RangeError:offset-out-of-range",
            "TypeError:bigint-value-required",
        ],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn typed_array_set_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn typed_array_set_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = typed_array_set_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn typed_array_set_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn typed_array_set_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn typed_array_set_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn typed_array_slice_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "slice",
        display_name: "slice",
        length: 2,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn typed_array_slice_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::TypedArray,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "typedarray-start-end-copy",
        encoding_policy: "none",
        range_window: "element-index",
        receiver_semantics: "typedarray-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "TypeError:detached-buffer",
            "TypeError:species-constructor",
        ],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn typed_array_slice_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn typed_array_slice_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = typed_array_slice_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn typed_array_slice_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn typed_array_slice_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn typed_array_slice_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn typed_array_subarray_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "subarray",
        display_name: "subarray",
        length: 2,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn typed_array_subarray_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::TypedArray,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "typedarray-start-end-view",
        encoding_policy: "none",
        range_window: "element-index",
        receiver_semantics: "typedarray-brand-detach",
        callback_policy: "none",
        error_codes: &["TypeError:invalid-receiver", "TypeError:detached-buffer"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn typed_array_subarray_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn typed_array_subarray_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = typed_array_subarray_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn typed_array_subarray_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn typed_array_subarray_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn typed_array_subarray_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn typed_array_to_reversed_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "toReversed",
        display_name: "toReversed",
        length: 0,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn typed_array_to_reversed_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::TypedArray,
        arity: 0,
        args: &[],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: false,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "typedarray-change-by-copy-reverse",
        encoding_policy: "none",
        range_window: "element-index",
        receiver_semantics: "typedarray-brand-detach",
        callback_policy: "none",
        error_codes: &["TypeError:invalid-receiver", "TypeError:detached-buffer"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn typed_array_to_reversed_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn typed_array_to_reversed_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 0] {
    let spec = typed_array_to_reversed_generated_validation_spec();
    debug_assert_eq!(spec.arity, 0);
    []
}

pub(crate) const fn typed_array_to_reversed_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn typed_array_to_reversed_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn typed_array_to_reversed_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn typed_array_to_sorted_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "toSorted",
        display_name: "toSorted",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn typed_array_to_sorted_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::TypedArray,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "typedarray-change-by-copy-sort",
        encoding_policy: "none",
        range_window: "element-index",
        receiver_semantics: "typedarray-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "TypeError:detached-buffer",
            "TypeError:callback-not-callable",
        ],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn typed_array_to_sorted_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn typed_array_to_sorted_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = typed_array_to_sorted_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn typed_array_to_sorted_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn typed_array_to_sorted_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn typed_array_to_sorted_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_buffer_static_is_view_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "isView",
        display_name: "isView",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_buffer_static_is_view_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::ArrayBuffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        returns: &[crate::native_api_manifest::NativeApiDomain::Boolean],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::None,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "buffer-view-brand",
        encoding_policy: "none",
        range_window: "none",
        receiver_semantics: "arraybuffer-static",
        callback_policy: "none",
        error_codes: &[],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_buffer_static_is_view_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_buffer_static_is_view_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = array_buffer_static_is_view_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_buffer_static_is_view_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_buffer_static_is_view_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_buffer_static_is_view_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_buffer_slice_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "slice",
        display_name: "slice",
        length: 2,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_buffer_slice_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::ArrayBuffer,
        arity: 2,
        args: &[
            crate::native_api_manifest::NativeApiDomain::Number,
            crate::native_api_manifest::NativeApiDomain::Number,
        ],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::None,
        effect: crate::native_api_manifest::NativeApiEffect::Pure,
        argument_shape: "arraybuffer-start-end-copy",
        encoding_policy: "none",
        range_window: "byte-index",
        receiver_semantics: "arraybuffer-brand-detach",
        callback_policy: "none",
        error_codes: &["TypeError:invalid-receiver", "TypeError:detached-buffer"],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_buffer_slice_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_buffer_slice_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 2] {
    let spec = array_buffer_slice_generated_validation_spec();
    debug_assert_eq!(spec.arity, 2);
    [
        args.get(0).cloned().unwrap_or(crate::Value::Undefined),
        args.get(1).cloned().unwrap_or(crate::Value::Undefined),
    ]
}

pub(crate) const fn array_buffer_slice_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_buffer_slice_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_buffer_slice_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_buffer_resize_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "resize",
        display_name: "resize",
        length: 1,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_buffer_resize_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::ArrayBuffer,
        arity: 1,
        args: &[crate::native_api_manifest::NativeApiDomain::Number],
        returns: &[crate::native_api_manifest::NativeApiDomain::Undefined],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "arraybuffer-new-byte-length",
        encoding_policy: "none",
        range_window: "byte-index",
        receiver_semantics: "arraybuffer-resizable-brand-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "TypeError:detached-buffer",
            "TypeError:not-resizable",
            "RangeError:max-byte-length",
        ],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_buffer_resize_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_buffer_resize_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 1] {
    let spec = array_buffer_resize_generated_validation_spec();
    debug_assert_eq!(spec.arity, 1);
    [args.get(0).cloned().unwrap_or(crate::Value::Undefined)]
}

pub(crate) const fn array_buffer_resize_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_buffer_resize_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_buffer_resize_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_buffer_transfer_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "transfer",
        display_name: "transfer",
        length: 0,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_buffer_transfer_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::ArrayBuffer,
        arity: 0,
        args: &[],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "arraybuffer-optional-new-byte-length",
        encoding_policy: "none",
        range_window: "byte-index",
        receiver_semantics: "arraybuffer-transfer-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "TypeError:detached-buffer",
            "RangeError:max-byte-length",
        ],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_buffer_transfer_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_buffer_transfer_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 0] {
    let spec = array_buffer_transfer_generated_validation_spec();
    debug_assert_eq!(spec.arity, 0);
    []
}

pub(crate) const fn array_buffer_transfer_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_buffer_transfer_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_buffer_transfer_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

pub(crate) const fn array_buffer_transfer_to_fixed_length_generated_registration_spec(
) -> crate::native_api_manifest::GeneratedRegistrationSpec {
    crate::native_api_manifest::GeneratedRegistrationSpec {
        property: "transferToFixedLength",
        display_name: "transferToFixedLength",
        length: 0,
        writable: true,
        enumerable: false,
        configurable: true,
        constructor: false,
        function_prototype: false,
    }
}

pub(crate) const fn array_buffer_transfer_to_fixed_length_generated_validation_spec(
) -> crate::native_api_manifest::GeneratedValidationSpec {
    crate::native_api_manifest::GeneratedValidationSpec {
        receiver: crate::native_api_manifest::NativeApiReceiver::ArrayBuffer,
        arity: 0,
        args: &[],
        returns: &[crate::native_api_manifest::NativeApiDomain::Unknown],
        missing_arg_defaults_to_undefined: true,
        index_coercion: crate::native_api_manifest::NativeApiIndexCoercion::ToIntegerOrInfinity,
        negative_or_infinite_result:
            crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        out_of_range_result: crate::native_api_manifest::NativeApiExceptionalResult::RangeError,
        effect: crate::native_api_manifest::NativeApiEffect::MutatesReceiverBytes,
        argument_shape: "arraybuffer-optional-fixed-byte-length",
        encoding_policy: "none",
        range_window: "byte-index",
        receiver_semantics: "arraybuffer-transfer-fixed-detach",
        callback_policy: "none",
        error_codes: &[
            "TypeError:invalid-receiver",
            "TypeError:detached-buffer",
            "RangeError:max-byte-length",
        ],
        byte_width: 0,
        signed: false,
        endian: crate::native_api_manifest::NativeApiEndian::None,
    }
}

pub(crate) const fn array_buffer_transfer_to_fixed_length_generated_ihi_spec(
) -> Option<crate::native_api_manifest::GeneratedIhiSpec> {
    None
}

pub(crate) fn array_buffer_transfer_to_fixed_length_generated_validation_args(
    args: &[crate::Value],
) -> [crate::Value; 0] {
    let spec = array_buffer_transfer_to_fixed_length_generated_validation_spec();
    debug_assert_eq!(spec.arity, 0);
    []
}

pub(crate) const fn array_buffer_transfer_to_fixed_length_generated_jit_ic_spec(
) -> Option<crate::native_api_manifest::GeneratedJitIcSpec> {
    None
}

pub(crate) const fn array_buffer_transfer_to_fixed_length_generated_lejit_expectation_spec(
) -> Option<crate::native_api_manifest::GeneratedLejitExpectationSpec> {
    None
}

pub(crate) const fn array_buffer_transfer_to_fixed_length_generated_cruftscript_stdlib_signature_spec(
) -> Option<GeneratedCruftScriptStdlibSignatureSpec> {
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GeneratedCruftScriptStdlibSignatureSpec {
    pub api: &'static str,
    pub receiver: &'static str,
    pub property: &'static str,
    pub arity: u8,
    pub args: &'static [&'static str],
    pub returns: &'static [&'static str],
    pub nullish_receiver_rejects: bool,
    pub boundary_safe: bool,
}
