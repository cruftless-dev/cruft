
pub mod deopt;
pub mod ic_table;
pub mod promote;
pub mod stub;
pub mod stub_aarch64;
pub mod stub_cranelift;
pub mod stub_raw_aarch64;
pub mod tiny_baseline;
pub mod translator;
pub use deopt::{
    clear_active_getindex_fn, clear_active_getprop_fn, clear_active_getprop_object_or_null_fn,
    clear_active_initprop_null_fn, clear_current_deopt_sites, clear_current_proto,
    clear_current_runtime, clear_osr_deopt_flag, deopt_trip, deopt_trip_with_frame_base,
    get_current_proto, get_current_runtime, get_force_shape_trip_addr, get_osr_deopt_flag_addr,
    jit_aria_parent_predicate_fusion, jit_aria_string_content_eq, jit_buffer_read_u32be,
    jit_buffer_write_u32be, jit_call_direct0, jit_call_direct0_objret, jit_call_direct1,
    jit_call_direct1_obj, jit_call_direct1_obj_objret, jit_call_direct1_void,
    jit_call_direct2_num_num, jit_call_direct2_obj_num, jit_call_direct2_obj_num_objret,
    jit_call_direct2_obj_num_void, jit_call_direct2_obj_obj_predicate,
    jit_call_direct2_prop_predicate, jit_call_direct4_value_store_dead,
    jit_call_direct4_value_store_dead_lanes_upvalue, jit_call_direct4_value_store_dead_upvalue,
    jit_call_indexed_direct1, jit_cwc_boolean_prop_as_number, jit_cwc_string_prop_len,
    jit_deopt_thunk, jit_getindex_on_object, jit_getprop_object_on_object, jit_getprop_on_object,
    jit_host_construct1_objret, jit_host_global_object, jit_host_method0_numret,
    jit_host_method1_numret, jit_host_method1_objret, jit_host_method1_string,
    jit_host_method2_objret, jit_host_method2_string, jit_host_method4_string,
    jit_host_object_method0_string, jit_host_object_method1_string, jit_host_object_prop_numret,
    jit_host_object_prop_string, jit_initindex_on_array, jit_initprop_null_on_object,
    jit_mwor_getprop_on_object, jit_mwor_setprop_on_object, jit_new_array, jit_new_object,
    jit_number_mod, jit_owned_string_result_char_code_at, jit_owned_string_result_len,
    jit_setglobal_var, jit_setindex_on_object, jit_setprop_fresh_data_add_on_object,
    jit_setprop_on_object, jit_string_prop_strict_eq, reconstruct_state,
    set_active_aria_parent_predicate_fusion_fn, set_active_aria_string_content_eq_fn,
    set_active_array_pop_fn, set_active_array_push1_fn, set_active_array_push1_obj_fn,
    set_active_buffer_read_u32be_fn, set_active_buffer_write_u32be_fn, set_active_call_direct0_fn,
    set_active_call_direct0_objret_fn, set_active_call_direct1_fn, set_active_call_direct1_obj_fn,
    set_active_call_direct1_obj_objret_fn, set_active_call_direct1_void_fn,
    set_active_call_direct2_num_num_fn, set_active_call_direct2_obj_num_fn,
    set_active_call_direct2_obj_num_objret_fn, set_active_call_direct2_obj_num_void_fn,
    set_active_call_direct2_obj_obj_predicate_fn, set_active_call_direct2_prop_predicate_fn,
    set_active_call_direct4_value_store_dead_fn, set_active_call_direct4_value_store_dead_lanes_fn,
    set_active_call_indexed_direct1_fn, set_active_cwc_boolean_prop_as_number_fn,
    set_active_cwc_string_prop_len_fn, set_active_getindex_fn, set_active_getprop_fn,
    set_active_getprop_object_fn, set_active_getprop_object_or_null_fn,
    set_active_getprop_truthy_fn, set_active_host_construct1_objret_fn,
    set_active_host_global_object_fn, set_active_host_method0_numret_fn,
    set_active_host_method1_numret_fn, set_active_host_method1_objret_fn,
    set_active_host_method1_string_fn, set_active_host_method2_objret_fn,
    set_active_host_method2_string_fn, set_active_host_method4_string_fn,
    set_active_host_object_method0_string_fn, set_active_host_object_method1_string_fn,
    set_active_host_object_prop_numret_fn, set_active_host_object_prop_string_fn,
    set_active_initindex_fn, set_active_initprop_null_fn, set_active_initprop_object_fn,
    set_active_mwor_getprop_fn, set_active_mwor_setprop_fn, set_active_newarray_fn,
    set_active_newobject_fn, set_active_owned_string_result_char_code_at_fn,
    set_active_owned_string_result_len_fn, set_active_setglobal_var_fn, set_active_setindex_fn,
    set_active_setprop_fn, set_active_setprop_fresh_data_add_fn,
    set_active_string_prop_strict_eq_fn, set_current_deopt_sites, set_current_proto,
    set_current_runtime, set_force_shape_trip, set_osr_deopt_flag, take_last_deopt,
    AriaParentPredicateFusionFn, AriaStringContentEqFn, ArrayPopFn, ArrayPush1Fn, ArrayPush1ObjFn,
    BufferReadU32BeFn, BufferWriteU32BeFn, CallDirect0Fn, CallDirect0ObjRetFn, CallDirect1Fn,
    CallDirect1ObjFn, CallDirect2NumNumFn, CallDirect2ObjNumFn, CallDirect2ObjObjPredicateFn,
    CallDirect2PropPredicateFn, CallDirect4ValueStoreDeadFn, CallDirect4ValueStoreDeadLanesFn,
    CallIndexedDirect1Fn, DeoptCallFrame, DeoptLiveLocal, DeoptReason, DeoptRecoveredState,
    DeoptSite, DeoptSiteTable, GetIndexFn, GetPropFn, GetPropObjectFn, GetPropObjectOrNullFn,
    HostConstruct1ObjRetFn, HostGlobalObjectFn, HostMethod0NumRetFn, HostMethod1NumRetFn,
    HostMethod1ObjRetFn, HostMethod1StringFn, HostMethod2ObjRetFn, HostMethod2StringFn,
    HostMethod4StringFn, HostObjectMethod1StringFn, HostObjectPropNumRetFn, InitIndexFn,
    InitPropNullFn, InitPropObjectFn, JitCallOutcome, JitLocation, MworGetPropFn, MworSetPropFn,
    NewArrayFn, NewObjectFn, OwnedStringResultCharCodeAtFn, OwnedStringResultLenFn, SetGlobalVarFn,
    SetIndexFn, SetPropFn, SetPropFreshDataAddFn, StringPropStrictEqFn,
};
pub use promote::promote_to_typed_i64;
pub use tiny_baseline::{lejit_tb_enabled, TinyBaselineMetadata, TB_BYTECODE_LEN_THRESHOLD};
pub use translator::{
    compile_function, compile_function_osr, compile_function_osr_direct_leaf_scalar_return,
    compile_function_osr_direct_leaf_scalar_return_with_stable_preloads, compile_function_osr_f64,
    compile_function_osr_mdoa_f64, compile_function_osr_mdoa_f64_mixed_scalar_storeback,
    compile_function_osr_mdoa_f64_with_stable_preloads,
    compile_function_osr_mixed_scalar_storeback, compile_function_osr_packed_array_scalar_return,
    compile_function_osr_typed_array_scalar_return,
    compile_function_osr_typed_array_scalar_storeback, compile_function_osr_with_direct_call_leafs,
    compile_function_osr_with_facts, compile_function_osr_with_facts_and_stable_preloads,
    compile_function_osr_with_facts_and_stable_preloads_and_string,
    compile_function_predicate_leaf_result_abi, compile_function_with_direct_call_leafs,
    compile_function_with_direct_call_leafs_and_stable_preloads,
    compile_function_with_stable_host_global_preloads,
    compile_function_with_stable_object_upvalue_preloads, compile_function_with_stable_preloads,
    compile_function_with_stable_preloads_and_string,
    compile_function_with_stable_preloads_object_and_string, predicate_leaf_result_abi_candidate,
    AriaCallsiteDirectEntryContract, CalleeBytecodeEffectDescriptor, CompiledFn, DirectCallLeaf,
    DirectCallLeafSpec, IcSiteDescriptor, IcSiteKind, IndexedDirectCallLeafArraySpec, JitFn,
    OsrScalarReturnContract, OsrScalarStorebackContract, PackedReadAffineIndex,
    PackedReadAffineIndexGroup, PackedReadAffineIndexSite, PackedReadAffineLoopBoundProof,
    PackedReadAffineUpperBound, PackedReadBoundedIndexSite, PackedReadIndexMask,
    StableCalleePreloadSpec, StableHostGlobalObjectPreloadSpec, StableHostObjectPreloadSpec,
    StableNumericUpvaluePreloadSpec, StableObjectUpvaluePreloadSpec,
    StableStringUpvaluePreloadSpec, TypedArrayKindSpec,
    JIT_OBJECT_OR_UNDEFINED_RETURN_UNDEFINED_BITS,
};

#[cfg(test)]
pub fn synthetic_trip_smoke() -> Result<extern "C" fn() -> i64, String> {
    let mut flag_builder = settings::builder();
    flag_builder
        .set("use_colocated_libcalls", "false")
        .map_err(|e| format!("flag use_colocated_libcalls: {e:?}"))?;
    flag_builder
        .set("is_pic", "false")
        .map_err(|e| format!("flag is_pic: {e:?}"))?;
    let isa_builder = cranelift_native::builder().map_err(|e| format!("isa builder: {e}"))?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|e| format!("isa finish: {e:?}"))?;

    let mut jit_builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());

    jit_builder.symbol("deopt_trip", deopt::deopt_trip as *const u8);
    let mut module = JITModule::new(jit_builder);

    let mut trip_sig = module.make_signature();
    for _ in 0..5 {
        trip_sig.params.push(AbiParam::new(I64));
    }
    trip_sig.returns.push(AbiParam::new(I64));
    let trip_id = module
        .declare_function("deopt_trip", Linkage::Import, &trip_sig)
        .map_err(|e| format!("declare trip: {e}"))?;

    let mut ctx = module.make_context();
    let mut fb_ctx = FunctionBuilderContext::new();

    ctx.func.signature.returns.push(AbiParam::new(I64));

    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fb_ctx);
        let entry = builder.create_block();
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let trip_ref = module.declare_func_in_func(trip_id, &mut builder.func);

        let site_id = builder.ins().iconst(I64, 0);
        let r0 = builder.ins().iconst(I64, 42);
        let r1 = builder.ins().iconst(I64, 0);
        let r2 = builder.ins().iconst(I64, 0);
        let r3 = builder.ins().iconst(I64, 0);
        let call_inst = builder.ins().call(trip_ref, &[site_id, r0, r1, r2, r3]);
        let ret = builder.inst_results(call_inst)[0];
        builder.ins().return_(&[ret]);
        builder.finalize();
    }

    let id = module
        .declare_function("synthetic_trip", Linkage::Export, &ctx.func.signature)
        .map_err(|e| format!("declare synthetic: {e}"))?;
    module
        .define_function(id, &mut ctx)
        .map_err(|e| format!("define synthetic: {e}"))?;
    module.clear_context(&mut ctx);
    module
        .finalize_definitions()
        .map_err(|e| format!("finalize: {e:?}"))?;

    let code_ptr = module.get_finalized_function(id);
    let f: extern "C" fn() -> i64 = unsafe { std::mem::transmute(code_ptr) };
    Box::leak(Box::new(module));
    Ok(f)
}

use cranelift_codegen::ir::types::I64;
use cranelift_codegen::ir::{AbiParam, InstBuilder};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};

pub fn smoke_test_add() -> Result<extern "C" fn(i64, i64) -> i64, String> {
    let mut flag_builder = settings::builder();
    flag_builder
        .set("use_colocated_libcalls", "false")
        .map_err(|e| format!("flag use_colocated_libcalls: {e:?}"))?;
    flag_builder
        .set("is_pic", "false")
        .map_err(|e| format!("flag is_pic: {e:?}"))?;
    let isa_builder = cranelift_native::builder().map_err(|e| format!("isa builder: {e}"))?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|e| format!("isa finish: {e:?}"))?;
    let builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
    let mut module = JITModule::new(builder);

    let mut ctx = module.make_context();
    let mut fb_ctx = FunctionBuilderContext::new();

    ctx.func.signature.params.push(AbiParam::new(I64));
    ctx.func.signature.params.push(AbiParam::new(I64));
    ctx.func.signature.returns.push(AbiParam::new(I64));

    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fb_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let a = builder.block_params(entry)[0];
        let b = builder.block_params(entry)[1];
        let sum = builder.ins().iadd(a, b);
        builder.ins().return_(&[sum]);
        builder.finalize();
    }

    let id = module
        .declare_function("smoke_add", Linkage::Export, &ctx.func.signature)
        .map_err(|e| format!("declare_function: {e}"))?;
    module
        .define_function(id, &mut ctx)
        .map_err(|e| format!("define_function: {e}"))?;
    module.clear_context(&mut ctx);
    module
        .finalize_definitions()
        .map_err(|e| format!("finalize_definitions: {e}"))?;

    let code_ptr = module.get_finalized_function(id);

    let f: extern "C" fn(i64, i64) -> i64 = unsafe { std::mem::transmute(code_ptr) };
    Box::leak(Box::new(module));
    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_add_works() {
        let add = smoke_test_add().expect("Cranelift smoke test failed");
        assert_eq!(add(2, 3), 5);
        assert_eq!(add(-10, 100), 90);
        assert_eq!(add(i64::MAX - 1, 1), i64::MAX);
    }

    #[test]
    fn synthetic_trip_calls_thunk_end_to_end() {

        let sites = vec![DeoptSite {
            reason: DeoptReason::IntegerOverflow { op_pc: 100 },
            resume_pc: 200,
            live_locals: vec![DeoptLiveLocal {
                interp_slot: 0,
                jit_location: JitLocation::Register(0),
            }],
            stack_depth: 0,
            stack_slots: vec![],
        }];
        set_current_deopt_sites(&sites);

        let trip_fn = synthetic_trip_smoke().expect("Cranelift extern wiring failed");
        let ret = trip_fn();
        assert_eq!(
            ret, 0,
            "thunk's sentinel propagates through Cranelift return"
        );

        let recovered = take_last_deopt().expect("trip should have recorded state");
        assert_eq!(
            recovered.reason,
            DeoptReason::IntegerOverflow { op_pc: 100 }
        );
        assert_eq!(recovered.resume_pc, 200);

        assert_eq!(recovered.local_values, vec![(0, 42)]);

        clear_current_deopt_sites();
    }
}
