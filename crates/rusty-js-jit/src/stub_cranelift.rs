
use crate::stub::ICSiteId;

pub fn emit_stub_pattern(
    builder: &mut cranelift_frontend::FunctionBuilder,
    recv_shape: cranelift_codegen::ir::Value,
    cached_shape: cranelift_codegen::ir::Value,
    cached_slot: cranelift_codegen::ir::Value,
    values_base: cranelift_codegen::ir::Value,
    slow_path_result: cranelift_codegen::ir::Value,
) -> cranelift_codegen::ir::Value {
    use cranelift_codegen::ir::{condcodes::IntCC, types::I64, InstBuilder, MemFlags};

    let hit_block = builder.create_block();
    let miss_block = builder.create_block();
    let merge_block = builder.create_block();
    builder.append_block_param(merge_block, I64);

    let eq = builder.ins().icmp(IntCC::Equal, recv_shape, cached_shape);
    builder.ins().brif(eq, hit_block, &[], miss_block, &[]);

    builder.switch_to_block(hit_block);
    builder.seal_block(hit_block);
    let eight = builder.ins().iconst(I64, 8);
    let offset = builder.ins().imul(cached_slot, eight);
    let addr = builder.ins().iadd(values_base, offset);
    let loaded = builder.ins().load(I64, MemFlags::trusted(), addr, 0);
    builder.ins().jump(merge_block, &[loaded]);

    builder.switch_to_block(miss_block);
    builder.seal_block(miss_block);
    builder.ins().jump(merge_block, &[slow_path_result]);

    builder.switch_to_block(merge_block);
    builder.seal_block(merge_block);
    builder.block_params(merge_block)[0]
}

pub fn build_stub_pattern_module() -> Result<extern "C" fn(i64, i64, i64, i64, i64) -> i64, String>
{
    use cranelift_codegen::ir::{
        types::I64, AbiParam, Function, InstBuilder, Signature, UserFuncName,
    };
    use cranelift_codegen::isa::CallConv;
    use cranelift_codegen::settings::{self, Configurable};
    use cranelift_codegen::Context;
    use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
    use cranelift_jit::{JITBuilder, JITModule};
    use cranelift_module::{Linkage, Module};

    let mut flag_builder = settings::builder();
    flag_builder
        .set("use_colocated_libcalls", "false")
        .map_err(|e| format!("flag: {e:?}"))?;
    flag_builder
        .set("is_pic", "false")
        .map_err(|e| format!("flag: {e:?}"))?;
    let isa_builder = cranelift_native::builder().map_err(|e| format!("isa: {e}"))?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|e| format!("isa: {e:?}"))?;
    let jit_builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
    let mut module = JITModule::new(jit_builder);

    let mut sig = Signature::new(CallConv::SystemV);
    for _ in 0..5 {
        sig.params.push(AbiParam::new(I64));
    }
    sig.returns.push(AbiParam::new(I64));

    let func_id = module
        .declare_function("stub_pattern", Linkage::Export, &sig)
        .map_err(|e| format!("declare_function: {e}"))?;

    let mut ctx = Context::new();
    ctx.func = Function::with_name_signature(UserFuncName::user(0, 0), sig);

    let mut fb_ctx = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fb_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let params: Vec<_> = builder.block_params(entry).to_vec();
        let result = emit_stub_pattern(
            &mut builder,
            params[0],
            params[1],
            params[2],
            params[3],
            params[4],
        );
        builder.ins().return_(&[result]);
        builder.finalize();
    }

    module
        .define_function(func_id, &mut ctx)
        .map_err(|e| format!("define_function: {e}"))?;
    module
        .finalize_definitions()
        .map_err(|e| format!("finalize_definitions: {e}"))?;
    let raw = module.get_finalized_function(func_id);
    let f: extern "C" fn(i64, i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(raw) };
    Ok(f)
}

#[allow(unused_variables)]
pub fn emit_getprop_stub(site_id: ICSiteId) {
    let _ = site_id;
}

pub fn emit_packed_array_read_probe(
    builder: &mut cranelift_frontend::FunctionBuilder,
    receiver: cranelift_codegen::ir::Value,
    index_i64: cranelift_codegen::ir::Value,
    giref: cranelift_codegen::ir::FuncRef,
    layout: &crate::deopt::InlineIcLayout,
) -> cranelift_codegen::ir::Value {
    use cranelift_codegen::ir::MemFlags;

    let m = MemFlags::trusted();
    let obj_addr = emit_array_object_addr(builder, receiver, layout);

    emit_packed_array_read_from_obj_addr(builder, receiver, index_i64, obj_addr, giref, layout, m)
}

pub fn emit_array_object_addr(
    builder: &mut cranelift_frontend::FunctionBuilder,
    receiver: cranelift_codegen::ir::Value,
    layout: &crate::deopt::InlineIcLayout,
) -> cranelift_codegen::ir::Value {
    let scaled = emit_array_object_scaled_off(builder, receiver, layout);
    emit_array_object_addr_from_scaled(builder, scaled)
}

pub fn emit_array_object_scaled_off(
    builder: &mut cranelift_frontend::FunctionBuilder,
    receiver: cranelift_codegen::ir::Value,
    layout: &crate::deopt::InlineIcLayout,
) -> cranelift_codegen::ir::Value {
    use cranelift_codegen::ir::{types::I64, InstBuilder};

    let mask = builder.ins().iconst(I64, 0x003F_FFFF);
    let id = builder.ins().band(receiver, mask);
    let stride = builder.ins().iconst(I64, layout.slot_stride as i64);
    let scaled = builder.ins().imul(id, stride);
    builder
        .ins()
        .iadd_imm(scaled, layout.slot_payload_off as i64)
}

pub fn emit_array_object_addr_from_scaled(
    builder: &mut cranelift_frontend::FunctionBuilder,
    scaled_off: cranelift_codegen::ir::Value,
) -> cranelift_codegen::ir::Value {
    use cranelift_codegen::ir::{types::I64, InstBuilder, MemFlags};

    let m = MemFlags::trusted();
    let hb_addr = builder
        .ins()
        .iconst(I64, crate::deopt::inline_ic_heap_base_addr() as i64);
    let heap_base = builder.ins().load(I64, m, hb_addr, 0);
    builder.ins().iadd(heap_base, scaled_off)
}

pub fn emit_packed_array_read_from_obj_addr(
    builder: &mut cranelift_frontend::FunctionBuilder,
    receiver: cranelift_codegen::ir::Value,
    index_i64: cranelift_codegen::ir::Value,
    obj_addr: cranelift_codegen::ir::Value,
    giref: cranelift_codegen::ir::FuncRef,
    layout: &crate::deopt::InlineIcLayout,
    m: cranelift_codegen::ir::MemFlags,
) -> cranelift_codegen::ir::Value {
    use cranelift_codegen::ir::{condcodes::IntCC, types::F64, types::I64, InstBuilder};

    let fast_blk = builder.create_block();
    let slow_blk = builder.create_block();
    let cont_blk = builder.create_block();
    builder.append_block_param(cont_blk, F64);

    let len = builder.ins().load(
        I64,
        m,
        obj_addr,
        layout.object_dense_doubles_off + layout.vec_len_off,
    );
    let in_bounds = builder.ins().icmp(IntCC::UnsignedLessThan, index_i64, len);
    builder.ins().brif(in_bounds, fast_blk, &[], slow_blk, &[]);

    builder.switch_to_block(fast_blk);
    builder.seal_block(fast_blk);
    let data = builder.ins().load(
        I64,
        m,
        obj_addr,
        layout.object_dense_doubles_off + layout.vec_ptr_off,
    );
    let eight = builder.ins().iconst(I64, 8);
    let elem_off = builder.ins().imul(index_i64, eight);
    let elem_addr = builder.ins().iadd(data, elem_off);
    let val = builder.ins().load(F64, m, elem_addr, 0);
    builder.ins().jump(cont_blk, &[val]);

    builder.switch_to_block(slow_blk);
    builder.seal_block(slow_blk);
    let call_inst = builder.ins().call(giref, &[receiver, index_i64]);
    let r_f64 = builder.inst_results(call_inst)[0];
    builder.ins().jump(cont_blk, &[r_f64]);

    builder.switch_to_block(cont_blk);
    builder.seal_block(cont_blk);
    builder.block_params(cont_blk)[0]
}

pub fn emit_packed_array_read_from_meta(
    builder: &mut cranelift_frontend::FunctionBuilder,
    receiver: cranelift_codegen::ir::Value,
    index_i64: cranelift_codegen::ir::Value,
    packed_len: cranelift_codegen::ir::Value,
    packed_data: cranelift_codegen::ir::Value,
    giref: cranelift_codegen::ir::FuncRef,
    m: cranelift_codegen::ir::MemFlags,
) -> cranelift_codegen::ir::Value {
    use cranelift_codegen::ir::{condcodes::IntCC, types::F64, types::I64, InstBuilder};

    let fast_blk = builder.create_block();
    let slow_blk = builder.create_block();
    let cont_blk = builder.create_block();
    builder.append_block_param(cont_blk, F64);

    let in_bounds = builder
        .ins()
        .icmp(IntCC::UnsignedLessThan, index_i64, packed_len);
    builder.ins().brif(in_bounds, fast_blk, &[], slow_blk, &[]);

    builder.switch_to_block(fast_blk);
    builder.seal_block(fast_blk);
    let eight = builder.ins().iconst(I64, 8);
    let elem_off = builder.ins().imul(index_i64, eight);
    let elem_addr = builder.ins().iadd(packed_data, elem_off);
    let val = builder.ins().load(F64, m, elem_addr, 0);
    builder.ins().jump(cont_blk, &[val]);

    builder.switch_to_block(slow_blk);
    builder.seal_block(slow_blk);
    let call_inst = builder.ins().call(giref, &[receiver, index_i64]);
    let r_f64 = builder.inst_results(call_inst)[0];
    builder.ins().jump(cont_blk, &[r_f64]);

    builder.switch_to_block(cont_blk);
    builder.seal_block(cont_blk);
    builder.block_params(cont_blk)[0]
}

pub fn emit_packed_array_read_inbounds_from_meta(
    builder: &mut cranelift_frontend::FunctionBuilder,
    index_i64: cranelift_codegen::ir::Value,
    packed_data: cranelift_codegen::ir::Value,
    m: cranelift_codegen::ir::MemFlags,
) -> cranelift_codegen::ir::Value {
    use cranelift_codegen::ir::{types::F64, types::I64, InstBuilder};

    let eight = builder.ins().iconst(I64, 8);
    let elem_off = builder.ins().imul(index_i64, eight);
    let elem_addr = builder.ins().iadd(packed_data, elem_off);
    builder.ins().load(F64, m, elem_addr, 0)
}

pub fn emit_packed_array_write_inbounds_from_meta(
    builder: &mut cranelift_frontend::FunctionBuilder,
    index_i64: cranelift_codegen::ir::Value,
    packed_data: cranelift_codegen::ir::Value,
    value_f64: cranelift_codegen::ir::Value,
    m: cranelift_codegen::ir::MemFlags,
) {
    use cranelift_codegen::ir::{types::I64, InstBuilder};

    let eight = builder.ins().iconst(I64, 8);
    let elem_off = builder.ins().imul(index_i64, eight);
    let elem_addr = builder.ins().iadd(packed_data, elem_off);
    builder.ins().store(m, value_f64, elem_addr, 0);
}

pub fn build_packed_array_write_probe_module() -> Result<extern "C" fn(i64, i64, f64), String> {
    use cranelift_codegen::ir::{
        types::{F64, I64},
        AbiParam, Function, InstBuilder, MemFlags, Signature, UserFuncName,
    };
    use cranelift_codegen::isa::CallConv;
    use cranelift_codegen::settings::{self, Configurable};
    use cranelift_codegen::Context;
    use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
    use cranelift_jit::{JITBuilder, JITModule};
    use cranelift_module::{Linkage, Module};

    let mut flag_builder = settings::builder();
    flag_builder
        .set("use_colocated_libcalls", "false")
        .map_err(|e| format!("flag: {e:?}"))?;
    flag_builder
        .set("is_pic", "false")
        .map_err(|e| format!("flag: {e:?}"))?;
    let isa_builder = cranelift_native::builder().map_err(|e| format!("isa: {e}"))?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|e| format!("isa: {e:?}"))?;
    let jit_builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
    let mut module = JITModule::new(jit_builder);

    let mut sig = Signature::new(CallConv::SystemV);
    sig.params.push(AbiParam::new(I64));
    sig.params.push(AbiParam::new(I64));
    sig.params.push(AbiParam::new(F64));

    let func_id = module
        .declare_function("packed_array_write_probe", Linkage::Export, &sig)
        .map_err(|e| format!("declare_function: {e}"))?;

    let mut ctx = Context::new();
    ctx.func = Function::with_name_signature(UserFuncName::user(0, 0), sig);

    let mut fb_ctx = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fb_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let params: Vec<_> = builder.block_params(entry).to_vec();
        emit_packed_array_write_inbounds_from_meta(
            &mut builder,
            params[1],
            params[0],
            params[2],
            MemFlags::trusted(),
        );
        builder.ins().return_(&[]);
        builder.finalize();
    }

    module
        .define_function(func_id, &mut ctx)
        .map_err(|e| format!("define_function: {e}"))?;
    module
        .finalize_definitions()
        .map_err(|e| format!("finalize_definitions: {e}"))?;
    let raw = module.get_finalized_function(func_id);
    let f: extern "C" fn(i64, i64, f64) = unsafe { std::mem::transmute(raw) };
    Box::leak(Box::new(module));
    Ok(f)
}

pub fn build_fannkuch_active_row_writeback_pc_probe_module(
) -> Result<extern "C" fn(i64, i64, i64, i64, i64, i64, i64) -> i64, String> {
    use cranelift_codegen::ir::{
        types::I64, AbiParam, Function, InstBuilder, MemFlags, Signature, UserFuncName,
    };
    use cranelift_codegen::isa::CallConv;
    use cranelift_codegen::settings::{self, Configurable};
    use cranelift_codegen::Context;
    use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
    use cranelift_jit::{JITBuilder, JITModule};
    use cranelift_module::{Linkage, Module};

    let mut flag_builder = settings::builder();
    flag_builder
        .set("use_colocated_libcalls", "false")
        .map_err(|e| format!("flag: {e:?}"))?;
    flag_builder
        .set("is_pic", "false")
        .map_err(|e| format!("flag: {e:?}"))?;
    let isa_builder = cranelift_native::builder().map_err(|e| format!("isa: {e}"))?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|e| format!("isa: {e:?}"))?;
    let jit_builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
    let mut module = JITModule::new(jit_builder);

    let mut sig = Signature::new(CallConv::SystemV);
    for _ in 0..7 {
        sig.params.push(AbiParam::new(I64));
    }
    sig.returns.push(AbiParam::new(I64));

    let func_id = module
        .declare_function(
            "fannkuch_active_row_writeback_pc_probe",
            Linkage::Export,
            &sig,
        )
        .map_err(|e| format!("declare_function: {e}"))?;

    let mut ctx = Context::new();
    ctx.func = Function::with_name_signature(UserFuncName::user(0, 0), sig);

    let mut fb_ctx = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fb_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let params: Vec<_> = builder.block_params(entry).to_vec();
        let read_data = params[0];
        let write_data = params[1];
        let locals = params[2];
        let read_index = params[3];
        let write_index = params[4];
        let storeback_slot = params[5];
        let next_pc = params[6];
        let m = MemFlags::trusted();

        let value =
            emit_packed_array_read_inbounds_from_meta(&mut builder, read_index, read_data, m);
        emit_packed_array_write_inbounds_from_meta(&mut builder, write_index, write_data, value, m);

        let eight = builder.ins().iconst(I64, 8);
        let local_off = builder.ins().imul(storeback_slot, eight);
        let local_addr = builder.ins().iadd(locals, local_off);
        builder.ins().store(m, value, local_addr, 0);
        builder.ins().return_(&[next_pc]);
        builder.finalize();
    }

    module
        .define_function(func_id, &mut ctx)
        .map_err(|e| format!("define_function: {e}"))?;
    module
        .finalize_definitions()
        .map_err(|e| format!("finalize_definitions: {e}"))?;
    let raw = module.get_finalized_function(func_id);
    let f: extern "C" fn(i64, i64, i64, i64, i64, i64, i64) -> i64 =
        unsafe { std::mem::transmute(raw) };
    Box::leak(Box::new(module));
    Ok(f)
}

pub fn build_fannkuch_site314_active_row_probe_module(
) -> Result<extern "C" fn(i64, i64, i64, i64, i64) -> i64, String> {
    use cranelift_codegen::ir::{
        types::I64, AbiParam, Function, InstBuilder, MemFlags, Signature, UserFuncName,
    };
    use cranelift_codegen::isa::CallConv;
    use cranelift_codegen::settings::{self, Configurable};
    use cranelift_codegen::Context;
    use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
    use cranelift_jit::{JITBuilder, JITModule};
    use cranelift_module::{Linkage, Module};

    let mut flag_builder = settings::builder();
    flag_builder
        .set("use_colocated_libcalls", "false")
        .map_err(|e| format!("flag: {e:?}"))?;
    flag_builder
        .set("is_pic", "false")
        .map_err(|e| format!("flag: {e:?}"))?;
    let isa_builder = cranelift_native::builder().map_err(|e| format!("isa: {e}"))?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|e| format!("isa: {e:?}"))?;
    let jit_builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
    let mut module = JITModule::new(jit_builder);

    let mut sig = Signature::new(CallConv::SystemV);
    for _ in 0..5 {
        sig.params.push(AbiParam::new(I64));
    }
    sig.returns.push(AbiParam::new(I64));

    let func_id = module
        .declare_function("fannkuch_site314_active_row_probe", Linkage::Export, &sig)
        .map_err(|e| format!("declare_function: {e}"))?;

    let mut ctx = Context::new();
    ctx.func = Function::with_name_signature(UserFuncName::user(0, 0), sig);

    let mut fb_ctx = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fb_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let params: Vec<_> = builder.block_params(entry).to_vec();
        let read_data = params[0];
        let write_data = params[1];
        let locals = params[2];
        let read_index = params[3];
        let write_index = params[4];
        let m = MemFlags::trusted();

        let value =
            emit_packed_array_read_inbounds_from_meta(&mut builder, read_index, read_data, m);
        emit_packed_array_write_inbounds_from_meta(&mut builder, write_index, write_data, value, m);

        let local_addr = builder.ins().iadd_imm(locals, 16 * 8);
        builder.ins().store(m, value, local_addr, 0);
        let next_pc = builder.ins().iconst(I64, 256);
        builder.ins().return_(&[next_pc]);
        builder.finalize();
    }

    module
        .define_function(func_id, &mut ctx)
        .map_err(|e| format!("define_function: {e}"))?;
    module
        .finalize_definitions()
        .map_err(|e| format!("finalize_definitions: {e}"))?;
    let raw = module.get_finalized_function(func_id);
    let f: extern "C" fn(i64, i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(raw) };
    Box::leak(Box::new(module));
    Ok(f)
}

pub fn build_fannkuch_site314_receiver_temp_probe_module(
) -> Result<extern "C" fn(i64, i64, i64) -> i64, String> {
    use cranelift_codegen::ir::{
        types::{F64, I64},
        AbiParam, Function, InstBuilder, MemFlags, Signature, UserFuncName,
    };
    use cranelift_codegen::isa::CallConv;
    use cranelift_codegen::settings::{self, Configurable};
    use cranelift_codegen::Context;
    use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
    use cranelift_jit::{JITBuilder, JITModule};
    use cranelift_module::{Linkage, Module};

    let mut flag_builder = settings::builder();
    flag_builder
        .set("use_colocated_libcalls", "false")
        .map_err(|e| format!("flag: {e:?}"))?;
    flag_builder
        .set("is_pic", "false")
        .map_err(|e| format!("flag: {e:?}"))?;
    let isa_builder = cranelift_native::builder().map_err(|e| format!("isa: {e}"))?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|e| format!("isa: {e:?}"))?;
    let jit_builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
    let mut module = JITModule::new(jit_builder);

    let mut sig = Signature::new(CallConv::SystemV);
    for _ in 0..3 {
        sig.params.push(AbiParam::new(I64));
    }
    sig.returns.push(AbiParam::new(I64));

    let func_id = module
        .declare_function(
            "fannkuch_site314_receiver_temp_probe",
            Linkage::Export,
            &sig,
        )
        .map_err(|e| format!("declare_function: {e}"))?;

    let mut ctx = Context::new();
    ctx.func = Function::with_name_signature(UserFuncName::user(0, 0), sig);

    let mut fb_ctx = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fb_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let params: Vec<_> = builder.block_params(entry).to_vec();
        let receiver_data_table = params[0];
        let locals = params[1];
        let scalar = params[2];
        let m = MemFlags::trusted();

        let write_data = builder.ins().load(I64, m, receiver_data_table, 2 * 8);
        let read_data = builder.ins().load(I64, m, receiver_data_table, 3 * 8);
        let value = emit_packed_array_read_inbounds_from_meta(&mut builder, scalar, read_data, m);
        emit_packed_array_write_inbounds_from_meta(&mut builder, scalar, write_data, value, m);

        let one = builder.ins().iconst(I64, 1);
        let next_induction = builder.ins().iadd(scalar, one);
        let pre_tmp = builder.ins().fcvt_from_sint(F64, scalar);
        let post_tmp = builder.ins().fcvt_from_sint(F64, next_induction);
        builder.ins().store(m, post_tmp, locals, 16 * 8);
        builder.ins().store(m, pre_tmp, locals, 17 * 8);
        builder.ins().store(m, post_tmp, locals, 18 * 8);

        let next_pc = builder.ins().iconst(I64, 256);
        builder.ins().return_(&[next_pc]);
        builder.finalize();
    }

    module
        .define_function(func_id, &mut ctx)
        .map_err(|e| format!("define_function: {e}"))?;
    module
        .finalize_definitions()
        .map_err(|e| format!("finalize_definitions: {e}"))?;
    let raw = module.get_finalized_function(func_id);
    let f: extern "C" fn(i64, i64, i64) -> i64 = unsafe { std::mem::transmute(raw) };
    Box::leak(Box::new(module));
    Ok(f)
}

pub fn build_fannkuch_site721_receiver_temp_probe_module(
) -> Result<extern "C" fn(i64, i64, i64) -> i64, String> {
    use cranelift_codegen::ir::{
        types::{F64, I64},
        AbiParam, Function, InstBuilder, MemFlags, Signature, UserFuncName,
    };
    use cranelift_codegen::isa::CallConv;
    use cranelift_codegen::settings::{self, Configurable};
    use cranelift_codegen::Context;
    use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
    use cranelift_jit::{JITBuilder, JITModule};
    use cranelift_module::{Linkage, Module};

    let mut flag_builder = settings::builder();
    flag_builder
        .set("use_colocated_libcalls", "false")
        .map_err(|e| format!("flag: {e:?}"))?;
    flag_builder
        .set("is_pic", "false")
        .map_err(|e| format!("flag: {e:?}"))?;
    let isa_builder = cranelift_native::builder().map_err(|e| format!("isa: {e}"))?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|e| format!("isa: {e:?}"))?;
    let jit_builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
    let mut module = JITModule::new(jit_builder);

    let mut sig = Signature::new(CallConv::SystemV);
    for _ in 0..3 {
        sig.params.push(AbiParam::new(I64));
    }
    sig.returns.push(AbiParam::new(I64));

    let func_id = module
        .declare_function(
            "fannkuch_site721_receiver_temp_probe",
            Linkage::Export,
            &sig,
        )
        .map_err(|e| format!("declare_function: {e}"))?;

    let mut ctx = Context::new();
    ctx.func = Function::with_name_signature(UserFuncName::user(0, 0), sig);

    let mut fb_ctx = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fb_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let params: Vec<_> = builder.block_params(entry).to_vec();
        let receiver_data_table = params[0];
        let locals = params[1];
        let scalar = params[2];
        let m = MemFlags::trusted();

        let receiver_data = builder.ins().load(I64, m, receiver_data_table, 3 * 8);
        let one = builder.ins().iconst(I64, 1);
        let next_induction = builder.ins().iadd(scalar, one);
        let value = emit_packed_array_read_inbounds_from_meta(
            &mut builder,
            next_induction,
            receiver_data,
            m,
        );
        emit_packed_array_write_inbounds_from_meta(&mut builder, scalar, receiver_data, value, m);

        let pre_tmp = builder.ins().fcvt_from_sint(F64, scalar);
        let post_tmp = builder.ins().fcvt_from_sint(F64, next_induction);
        builder.ins().store(m, post_tmp, locals, 29 * 8);
        builder.ins().store(m, pre_tmp, locals, 30 * 8);
        builder.ins().store(m, post_tmp, locals, 31 * 8);

        let next_pc = builder.ins().iconst(I64, 666);
        builder.ins().return_(&[next_pc]);
        builder.finalize();
    }

    module
        .define_function(func_id, &mut ctx)
        .map_err(|e| format!("define_function: {e}"))?;
    module
        .finalize_definitions()
        .map_err(|e| format!("finalize_definitions: {e}"))?;
    let raw = module.get_finalized_function(func_id);
    let f: extern "C" fn(i64, i64, i64) -> i64 = unsafe { std::mem::transmute(raw) };
    Box::leak(Box::new(module));
    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_array_write_probe_updates_dense_slot() {
        let write =
            build_packed_array_write_probe_module().expect("packed write probe should compile");
        let mut dense = vec![1.0_f64, 2.0, 3.0, 4.0];

        write(dense.as_mut_ptr() as i64, 2, 99.0);

        assert_eq!(dense, vec![1.0, 2.0, 99.0, 4.0]);
    }

    #[test]
    fn fannkuch_active_row_writeback_pc_probe_moves_value_and_returns_next_pc() {
        let active_row = build_fannkuch_active_row_writeback_pc_probe_module()
            .expect("active-row writeback pc probe should compile");
        let read_dense = [10.0_f64, 20.0, 30.0];
        let mut write_dense = vec![1.0_f64, 2.0, 3.0, 4.0];
        let mut locals = vec![0.0_f64, 0.0, 0.0, 0.0, 0.0];

        let next_pc = active_row(
            read_dense.as_ptr() as i64,
            write_dense.as_mut_ptr() as i64,
            locals.as_mut_ptr() as i64,
            1,
            3,
            4,
            256,
        );

        assert_eq!(next_pc, 256);
        assert_eq!(write_dense, vec![1.0, 2.0, 3.0, 20.0]);
        assert_eq!(locals, vec![0.0, 0.0, 0.0, 0.0, 20.0]);
    }

    #[test]
    fn fannkuch_site314_probe_binds_slot16_and_backedge_pc() {
        let site314 =
            build_fannkuch_site314_active_row_probe_module().expect("site314 probe should compile");
        let read_dense = [11.0_f64, 22.0, 33.0, 44.0];
        let mut write_dense = vec![0.0_f64, 0.0, 0.0, 0.0];
        let mut locals = vec![0.0_f64; 19];

        let next_pc = site314(
            read_dense.as_ptr() as i64,
            write_dense.as_mut_ptr() as i64,
            locals.as_mut_ptr() as i64,
            2,
            1,
        );

        assert_eq!(next_pc, 256);
        assert_eq!(write_dense, vec![0.0, 33.0, 0.0, 0.0]);
        assert_eq!(locals[16], 33.0);
        assert_eq!(locals[17], 0.0, "site314 temp slot 17 is not owned yet");
        assert_eq!(locals[18], 0.0, "site314 temp slot 18 is not owned yet");
    }

    #[test]
    fn fannkuch_site314_receiver_temp_probe_uses_slots_3_and_2() {
        let site314 = build_fannkuch_site314_receiver_temp_probe_module()
            .expect("site314 receiver/temp probe should compile");
        let read_dense = [11.0_f64, 22.0, 33.0, 44.0];
        let mut write_dense = vec![0.0_f64, 0.0, 0.0, 0.0];
        let mut receiver_data_table = vec![0_i64; 4];
        receiver_data_table[2] = write_dense.as_mut_ptr() as i64;
        receiver_data_table[3] = read_dense.as_ptr() as i64;
        let mut locals = vec![0.0_f64; 19];

        let next_pc = site314(
            receiver_data_table.as_ptr() as i64,
            locals.as_mut_ptr() as i64,
            2,
        );

        assert_eq!(next_pc, 256);
        assert_eq!(write_dense, vec![0.0, 0.0, 33.0, 0.0]);
        assert_eq!(locals[16], 3.0, "site314 induction slot gets post value");
        assert_eq!(locals[17], 2.0, "site314 temp slot 17 gets pre value");
        assert_eq!(locals[18], 3.0, "site314 temp slot 18 gets post value");
    }

    #[test]
    fn fannkuch_site721_receiver_temp_probe_uses_slot_3_with_shifted_read() {
        let site721 = build_fannkuch_site721_receiver_temp_probe_module()
            .expect("site721 receiver/temp probe should compile");
        let mut receiver = vec![10.0_f64, 20.0, 30.0, 40.0, 50.0];
        let mut receiver_data_table = vec![0_i64; 4];
        receiver_data_table[3] = receiver.as_mut_ptr() as i64;
        let mut locals = vec![0.0_f64; 32];

        let next_pc = site721(
            receiver_data_table.as_ptr() as i64,
            locals.as_mut_ptr() as i64,
            2,
        );

        assert_eq!(next_pc, 666);
        assert_eq!(receiver, vec![10.0, 20.0, 40.0, 40.0, 50.0]);
        assert_eq!(locals[29], 3.0, "site721 induction slot gets post value");
        assert_eq!(locals[30], 2.0, "site721 temp slot 30 gets pre value");
        assert_eq!(locals[31], 3.0, "site721 temp slot 31 gets post value");
    }
}
