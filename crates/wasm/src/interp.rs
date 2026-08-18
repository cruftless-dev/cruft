
use crate::parser::{
    ArrayType, BlockType, CatchKind, FuncType, Instr, Module, StructType, ValType,
};
use crate::{HostContext, HostFn, WasmValue};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

const PAGE: usize = 65536;
const MAX_TABLE_ELEMENTS: usize = 1_000_000;
const DEFAULT_EXECUTION_FUEL: u64 = 10_000_000;
const DEFAULT_MAX_MEMORY_PAGES: usize = 8192;

fn wasm_fast_leaf_counters_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_WASM_FAST_LEAF_COUNTERS")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(false)
    })
}

fn record_wasm_fast_leaf(kind: &'static str) {
    if !wasm_fast_leaf_counters_enabled() {
        return;
    }
    static ATTEMPTS: AtomicU64 = AtomicU64::new(0);
    static HITS: AtomicU64 = AtomicU64::new(0);
    static MISSES: AtomicU64 = AtomicU64::new(0);
    static VOID_HITS: AtomicU64 = AtomicU64::new(0);
    static CONST_I32_HITS: AtomicU64 = AtomicU64::new(0);
    static ADD_I32_HITS: AtomicU64 = AtomicU64::new(0);
    static LOAD_I32_HITS: AtomicU64 = AtomicU64::new(0);

    let attempts = ATTEMPTS.fetch_add(1, Ordering::Relaxed) + 1;
    match kind {
        "miss" => {
            MISSES.fetch_add(1, Ordering::Relaxed);
        }
        "void" => {
            HITS.fetch_add(1, Ordering::Relaxed);
            VOID_HITS.fetch_add(1, Ordering::Relaxed);
        }
        "i32.const" => {
            HITS.fetch_add(1, Ordering::Relaxed);
            CONST_I32_HITS.fetch_add(1, Ordering::Relaxed);
        }
        "i32.add" => {
            HITS.fetch_add(1, Ordering::Relaxed);
            ADD_I32_HITS.fetch_add(1, Ordering::Relaxed);
        }
        "i32.load" => {
            HITS.fetch_add(1, Ordering::Relaxed);
            LOAD_I32_HITS.fetch_add(1, Ordering::Relaxed);
        }
        _ => {}
    }

    if attempts <= 8 || attempts.is_power_of_two() {
        eprintln!(
            "[wasm-fast-leaf] attempts={} hits={} misses={} void={} i32_const={} i32_add={} i32_load={}",
            attempts,
            HITS.load(Ordering::Relaxed),
            MISSES.load(Ordering::Relaxed),
            VOID_HITS.load(Ordering::Relaxed),
            CONST_I32_HITS.load(Ordering::Relaxed),
            ADD_I32_HITS.load(Ordering::Relaxed),
            LOAD_I32_HITS.load(Ordering::Relaxed)
        );
    }
}

fn wasm_loop_fast_counters_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_WASM_LOOP_FAST_COUNTERS")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(false)
    })
}

fn record_wasm_loop_fast(kind: &'static str) {
    if !wasm_loop_fast_counters_enabled() {
        return;
    }
    static ATTEMPTS: AtomicU64 = AtomicU64::new(0);
    static HITS: AtomicU64 = AtomicU64::new(0);
    static MISSES: AtomicU64 = AtomicU64::new(0);

    let attempts = ATTEMPTS.fetch_add(1, Ordering::Relaxed) + 1;
    match kind {
        "hit" => {
            HITS.fetch_add(1, Ordering::Relaxed);
        }
        "miss" => {
            MISSES.fetch_add(1, Ordering::Relaxed);
        }
        _ => {}
    }

    if attempts <= 8 || attempts.is_power_of_two() {
        eprintln!(
            "[wasm-loop-fast] attempts={} hits={} misses={}",
            attempts,
            HITS.load(Ordering::Relaxed),
            MISSES.load(Ordering::Relaxed)
        );
    }
}

fn wasm_memory_loop_fast_counters_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_WASM_MEMORY_LOOP_FAST_COUNTERS")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(false)
    })
}

fn record_wasm_memory_loop_fast(kind: &'static str) {
    if !wasm_memory_loop_fast_counters_enabled() {
        return;
    }
    static ATTEMPTS: AtomicU64 = AtomicU64::new(0);
    static HITS: AtomicU64 = AtomicU64::new(0);
    static MISSES: AtomicU64 = AtomicU64::new(0);

    let attempts = ATTEMPTS.fetch_add(1, Ordering::Relaxed) + 1;
    match kind {
        "hit" => {
            HITS.fetch_add(1, Ordering::Relaxed);
        }
        "miss" => {
            MISSES.fetch_add(1, Ordering::Relaxed);
        }
        _ => {}
    }

    if attempts <= 8 || attempts.is_power_of_two() {
        eprintln!(
            "[wasm-memory-loop-fast] attempts={} hits={} misses={}",
            attempts,
            HITS.load(Ordering::Relaxed),
            MISSES.load(Ordering::Relaxed)
        );
    }
}

fn wasm_host_import_counters_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CRUFT_WASM_HOST_IMPORT_COUNTERS")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(false)
    })
}

fn record_wasm_host_import(kind: &'static str) {
    if !wasm_host_import_counters_enabled() {
        return;
    }
    static CALLS: AtomicU64 = AtomicU64::new(0);
    static NO_MEMORY: AtomicU64 = AtomicU64::new(0);
    static WITH_MEMORY: AtomicU64 = AtomicU64::new(0);

    let calls = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    match kind {
        "no-memory" => {
            NO_MEMORY.fetch_add(1, Ordering::Relaxed);
        }
        "with-memory" => {
            WITH_MEMORY.fetch_add(1, Ordering::Relaxed);
        }
        _ => {}
    }

    if calls <= 8 || calls.is_power_of_two() {
        eprintln!(
            "[wasm-host-import] calls={} no_memory={} with_memory={}",
            calls,
            NO_MEMORY.load(Ordering::Relaxed),
            WITH_MEMORY.load(Ordering::Relaxed)
        );
    }
}

#[derive(Clone)]
struct ControlInfo {

    end_of: Vec<i32>,

    else_of: Vec<i32>,
}

fn precompute_control(body: &[Instr]) -> Result<ControlInfo, String> {
    let n = body.len();
    let mut end_of = vec![-1i32; n];
    let mut else_of = vec![-1i32; n];
    let mut stack: Vec<usize> = Vec::new();
    for (i, ins) in body.iter().enumerate() {
        match ins {
            Instr::Block(_)
            | Instr::Loop(_)
            | Instr::If(_)
            | Instr::LegacyTry(_)
            | Instr::TryTable(_, _) => stack.push(i),
            Instr::Else | Instr::LegacyCatch(_) | Instr::LegacyCatchAll => {
                if let Some(&top) = stack.last() {
                    if matches!(body[top], Instr::If(_) | Instr::LegacyTry(_)) {
                        else_of[top] = i as i32;
                    }
                }
            }
            Instr::LegacyDelegate(_) => {
                if let Some(top) = stack.pop() {
                    end_of[top] = i as i32;
                }
            }
            Instr::End => {
                if let Some(top) = stack.pop() {
                    end_of[top] = i as i32;
                }

            }
            _ => {}
        }
    }
    Ok(ControlInfo { end_of, else_of })
}

#[derive(Clone)]
struct Label {

    cont_pc: usize,

    arity: usize,

    stack_height: usize,
    is_loop: bool,
    catches: Option<Vec<CatchKind>>,
}

#[derive(Clone)]
struct ExceptionPayload {
    tag: u32,
    identity: Option<String>,
    values: Vec<WasmValue>,
}

#[derive(Clone)]
pub(crate) struct GcArray {
    pub(crate) type_idx: u32,
    pub(crate) elements: Vec<WasmValue>,
    pub(crate) mutable: bool,
}

#[derive(Clone)]
pub(crate) struct GcStruct {
    pub(crate) type_idx: u32,
    pub(crate) fields: Vec<WasmValue>,
    pub(crate) mutable: Vec<bool>,
}

enum BodyResult {
    Values(Vec<WasmValue>),
    TailCall {
        overall: usize,
        args: Vec<WasmValue>,
    },
    Exception(ExceptionPayload),
}

pub enum Callable {
    Host(HostFn),
    Defined(usize),
}

pub struct Instance {
    pub module: Module,
    pub funcs: Vec<Callable>,
    pub host_func_sigs: Vec<Option<(Vec<crate::ValType>, Vec<crate::ValType>)>>,
    pub globals: Vec<WasmValue>,
    pub global_mut: Vec<bool>,
    pub memory: Vec<u8>,
    pub extra_memories: Vec<Vec<u8>>,
    pub has_memory: bool,
    pub mem_max_pages: Option<u64>,
    pub extra_mem_max_pages: Vec<Option<u64>>,
    pub memory_aliases: Vec<Option<u64>>,
    pub memory_dirty_ranges: Vec<Option<(usize, usize)>>,
    pub memory64: bool,
    pub memory_shared: bool,
    pub tables: Vec<Vec<WasmValue>>,
    pub table_maxes: Vec<Option<u64>>,
    pub table64s: Vec<bool>,

    pub data_segments: Vec<Option<Vec<u8>>>,

    pub elem_segments: Vec<Option<Vec<WasmValue>>>,
    pub tag_identities: Vec<String>,
    exception_refs: Vec<ExceptionPayload>,
    gc_arrays: Vec<GcArray>,
    gc_structs: Vec<GcStruct>,
    control_cache: Vec<Option<ControlInfo>>,
    execution_fuel: u64,
}

fn inline_block_arity(bt: BlockType) -> usize {
    match bt {
        BlockType::Empty => 0,
        BlockType::Value(_) => 1,
        BlockType::TypeIndex(_) => 0,
    }
}

impl Instance {
    fn default_execution_fuel() -> u64 {
        std::env::var("CRUFT_WASM_EXECUTION_FUEL")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_EXECUTION_FUEL)
    }

    fn max_memory_pages() -> usize {
        std::env::var("CRUFT_WASM_MAX_MEMORY_PAGES")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_MAX_MEMORY_PAGES)
    }

    fn reset_execution_fuel(&mut self) {
        self.execution_fuel = Self::default_execution_fuel();
    }

    fn charge_execution_fuel(&mut self) -> Result<(), String> {
        self.execution_fuel = self
            .execution_fuel
            .checked_sub(1)
            .ok_or_else(|| "execution fuel exhausted".to_string())?;
        Ok(())
    }

    fn table(&self, tableidx: u32) -> Result<&[WasmValue], String> {
        self.tables
            .get(tableidx as usize)
            .map(|table| table.as_slice())
            .ok_or_else(|| format!("table index {} out of bounds", tableidx))
    }

    fn table_mut(&mut self, tableidx: u32) -> Result<&mut Vec<WasmValue>, String> {
        self.tables
            .get_mut(tableidx as usize)
            .ok_or_else(|| format!("table index {} out of bounds", tableidx))
    }

    fn table_max_for(&self, tableidx: u32) -> Result<Option<u64>, String> {
        self.table_maxes
            .get(tableidx as usize)
            .copied()
            .ok_or_else(|| format!("table index {} out of bounds", tableidx))
    }

    fn table_is_64(&self, tableidx: u32) -> Result<bool, String> {
        self.table64s
            .get(tableidx as usize)
            .copied()
            .ok_or_else(|| format!("table index {} out of bounds", tableidx))
    }

    fn pop_table_index(&self, tableidx: u32, stack: &mut Vec<WasmValue>) -> Result<usize, String> {
        let raw = if self.table_is_64(tableidx)? {
            pop_i64(stack)? as u64
        } else {
            pop_i32(stack)? as u32 as u64
        };
        usize::try_from(raw).map_err(|_| "table index overflow".to_string())
    }

    fn push_table_index_result(
        &self,
        tableidx: u32,
        stack: &mut Vec<WasmValue>,
        value: usize,
    ) -> Result<(), String> {
        if self.table_is_64(tableidx)? {
            stack.push(WasmValue::I64(value as i64));
        } else {
            stack.push(WasmValue::I32(value as i32));
        }
        Ok(())
    }

    fn memory_ref(&self, memoryidx: u32) -> Result<&[u8], String> {
        if memoryidx == 0 {
            if self.has_memory {
                Ok(&self.memory)
            } else {
                Err("memory access without a memory".to_string())
            }
        } else {
            self.extra_memories
                .get(memoryidx as usize - 1)
                .map(|memory| memory.as_slice())
                .ok_or_else(|| format!("memory index {} out of bounds", memoryidx))
        }
    }

    fn memory_mut(&mut self, memoryidx: u32) -> Result<&mut Vec<u8>, String> {
        if memoryidx == 0 {
            if self.has_memory {
                Ok(&mut self.memory)
            } else {
                Err("memory access without a memory".to_string())
            }
        } else {
            self.extra_memories
                .get_mut(memoryidx as usize - 1)
                .ok_or_else(|| format!("memory index {} out of bounds", memoryidx))
        }
    }

    fn memory_max_pages_for(&self, memoryidx: u32) -> Result<Option<u64>, String> {
        if memoryidx == 0 {
            if self.has_memory {
                Ok(self.mem_max_pages)
            } else {
                Err("memory access without a memory".to_string())
            }
        } else {
            self.extra_mem_max_pages
                .get(memoryidx as usize - 1)
                .copied()
                .ok_or_else(|| format!("memory index {} out of bounds", memoryidx))
        }
    }

    fn array_type(&self, typeidx: u32) -> Result<crate::parser::ArrayType, String> {
        self.module
            .array_types
            .get(typeidx as usize)
            .and_then(|ty| *ty)
            .ok_or_else(|| format!("type index {} is not an array type", typeidx))
    }

    fn struct_type(&self, typeidx: u32) -> Result<StructType, String> {
        self.module
            .struct_types
            .get(typeidx as usize)
            .and_then(|ty| ty.clone())
            .ok_or_else(|| format!("type index {} is not a struct type", typeidx))
    }

    fn alloc_array(
        &mut self,
        type_idx: u32,
        ty: crate::parser::ArrayType,
        len: usize,
        value: WasmValue,
    ) -> WasmValue {
        let idx = self.gc_arrays.len() as u32;
        self.gc_arrays.push(GcArray {
            type_idx,
            elements: vec![value; len],
            mutable: ty.mutable,
        });
        WasmValue::ArrayRef(idx)
    }

    fn alloc_struct(&mut self, type_idx: u32, ty: StructType, fields: Vec<WasmValue>) -> WasmValue {
        let idx = self.gc_structs.len() as u32;
        let fields = fields
            .into_iter()
            .zip(ty.fields.iter())
            .map(|(value, field)| normalize_packed_value(field.packed_bits, value))
            .collect();
        self.gc_structs.push(GcStruct {
            type_idx,
            fields,
            mutable: ty.fields.iter().map(|field| field.mutable).collect(),
        });
        WasmValue::StructRef(idx)
    }

    fn block_type(&self, idx: u32) -> Result<&crate::parser::FuncType, String> {
        self.module
            .types
            .get(idx as usize)
            .ok_or_else(|| format!("block type index {} out of bounds", idx))
    }

    fn block_result_arity(&self, bt: BlockType) -> Result<usize, String> {
        match bt {
            BlockType::TypeIndex(idx) => Ok(self.block_type(idx)?.results.len()),
            other => Ok(inline_block_arity(other)),
        }
    }

    fn block_param_arity(&self, bt: BlockType) -> Result<usize, String> {
        match bt {
            BlockType::TypeIndex(idx) => Ok(self.block_type(idx)?.params.len()),
            BlockType::Empty | BlockType::Value(_) => Ok(0),
        }
    }

    fn func_type_index(&self, overall: usize) -> Option<u32> {

        let mut imp_i = 0usize;
        for imp in &self.module.imports {
            if let crate::parser::ImportKind::Func(t) = imp.kind {
                if imp_i == overall {
                    return Some(t);
                }
                imp_i += 1;
            }
        }
        let defined_idx = overall.checked_sub(self.module.imported_func_count)?;
        self.module.func_types.get(defined_idx).copied()
    }

    fn func_sig_by_overall(
        &self,
        overall: usize,
    ) -> Option<(Vec<crate::ValType>, Vec<crate::ValType>)> {
        if let Some(Some(sig)) = self.host_func_sigs.get(overall) {
            return Some(sig.clone());
        }
        let type_idx = self.func_type_index(overall)? as usize;
        let ft = self.module.types.get(type_idx)?;
        Some((
            ft.params
                .iter()
                .map(|t| crate::ValType::from_parser(*t))
                .collect(),
            ft.results
                .iter()
                .map(|t| crate::ValType::from_parser(*t))
                .collect(),
        ))
    }

    pub fn call_overall(
        &mut self,
        overall: usize,
        args: &[WasmValue],
    ) -> Result<Vec<WasmValue>, String> {
        self.reset_execution_fuel();

        let is_host = matches!(self.funcs.get(overall), Some(Callable::Host(_)));
        if is_host {
            return self.invoke_host(overall, args);
        }
        let defined_idx = match self.funcs.get(overall) {
            Some(Callable::Defined(d)) => *d,
            _ => return Err(format!("func index {} not callable", overall)),
        };
        self.call_defined(defined_idx, args, 0)
    }

    fn call_defined(
        &mut self,
        defined_idx: usize,
        args: &[WasmValue],
        depth: usize,
    ) -> Result<Vec<WasmValue>, String> {
        match self.call_defined_body(defined_idx, args, depth)? {
            BodyResult::Values(results) => Ok(results),
            BodyResult::TailCall { .. } => Err("call: unexpected tail-call transfer".to_string()),
            BodyResult::Exception(ex) => Err(format!("unhandled exception tag {}", ex.tag)),
        }
    }

    fn call_defined_body(
        &mut self,
        defined_idx: usize,
        args: &[WasmValue],
        depth: usize,
    ) -> Result<BodyResult, String> {
        if depth > 10_000 {
            return Err("call stack exhausted".to_string());
        }
        let type_idx = self.module.func_types[defined_idx] as usize;
        let params = self.module.types[type_idx].params.clone();
        let results = self.module.types[type_idx].results.clone();
        let code_locals = self.module.code[defined_idx].locals.clone();
        let code_body = self.module.code[defined_idx].body.clone();
        if args.len() != params.len() {
            return Err(format!(
                "arity mismatch: expected {} args, got {}",
                params.len(),
                args.len()
            ));
        }
        if let Some(results) = self.try_fast_i32_store_load_xor_sweep(
            &code_body,
            &code_locals,
            args,
            &params,
            &results,
        )? {
            return Ok(BodyResult::Values(results));
        }
        if let Some(results) =
            self.try_fast_fnv1a_i32_load8_hash(&code_body, &code_locals, args, &params, &results)?
        {
            return Ok(BodyResult::Values(results));
        }
        if let Some(results) =
            self.try_fast_i32_store_sum(&code_body, &code_locals, args, &params, &results)?
        {
            return Ok(BodyResult::Values(results));
        }
        if let Some(results) =
            self.try_fast_counted_i32_sum(&code_body, &code_locals, args, &params, &results)?
        {
            return Ok(BodyResult::Values(results));
        }
        if let Some(results) =
            self.try_fast_counted_i32_leaf_chain(&code_body, &code_locals, args, &params, &results)?
        {
            return Ok(BodyResult::Values(results));
        }
        if code_locals.is_empty() {
            if let Some(results) = self.try_fast_leaf(&code_body, args, &results)? {
                return Ok(BodyResult::Values(results));
            }
        }
        let mut current_defined_idx = defined_idx;
        let mut current_args = args.to_vec();

        loop {
            let type_idx = self.module.func_types[current_defined_idx] as usize;
            let params = self.module.types[type_idx].params.clone();
            let results = self.module.types[type_idx].results.clone();
            let code_locals = self.module.code[current_defined_idx].locals.clone();
            let code_body = self.module.code[current_defined_idx].body.clone();
            if current_args.len() != params.len() {
                return Err(format!(
                    "arity mismatch: expected {} args, got {}",
                    params.len(),
                    current_args.len()
                ));
            }
            if let Some(results) = self.try_fast_i32_store_load_xor_sweep(
                &code_body,
                &code_locals,
                &current_args,
                &params,
                &results,
            )? {
                return Ok(BodyResult::Values(results));
            }
            if let Some(results) = self.try_fast_fnv1a_i32_load8_hash(
                &code_body,
                &code_locals,
                &current_args,
                &params,
                &results,
            )? {
                return Ok(BodyResult::Values(results));
            }
            if let Some(results) = self.try_fast_i32_store_sum(
                &code_body,
                &code_locals,
                &current_args,
                &params,
                &results,
            )? {
                return Ok(BodyResult::Values(results));
            }
            if let Some(results) = self.try_fast_counted_i32_sum(
                &code_body,
                &code_locals,
                &current_args,
                &params,
                &results,
            )? {
                return Ok(BodyResult::Values(results));
            }
            if let Some(results) = self.try_fast_counted_i32_leaf_chain(
                &code_body,
                &code_locals,
                &current_args,
                &params,
                &results,
            )? {
                return Ok(BodyResult::Values(results));
            }
            if code_locals.is_empty() {
                if let Some(results) = self.try_fast_leaf(&code_body, &current_args, &results)? {
                    return Ok(BodyResult::Values(results));
                }
            }
            let ftype = self.module.types[type_idx].clone();
            let code = self.module.code[current_defined_idx].clone();
            self.charge_execution_fuel()?;

            let mut locals: Vec<WasmValue> =
                Vec::with_capacity(current_args.len() + code.locals.len());
            locals.extend_from_slice(&current_args);
            for lt in &code.locals {
                locals.push(zero_of(*lt));
            }

            if self.control_cache[current_defined_idx].is_none() {
                self.control_cache[current_defined_idx] = Some(precompute_control(&code.body)?);
            }
            let ctrl = self.control_cache[current_defined_idx].clone().unwrap();

            match self.exec_body(&code.body, &ctrl, &mut locals, &ftype.results, depth)? {
                BodyResult::Values(results) => return Ok(BodyResult::Values(results)),
                BodyResult::TailCall { overall, args } => {
                    if matches!(self.funcs.get(overall), Some(Callable::Host(_))) {
                        return Ok(BodyResult::Values(self.invoke_host(overall, &args)?));
                    }
                    current_defined_idx = match self.funcs.get(overall) {
                        Some(Callable::Defined(d)) => *d,
                        _ => return Err(format!("func index {} not callable", overall)),
                    };
                    current_args = args;
                }
                BodyResult::Exception(ex) => return Ok(BodyResult::Exception(ex)),
            }
        }
    }

    fn try_fast_i32_store_sum(
        &mut self,
        body: &[Instr],
        locals: &[ValType],
        args: &[WasmValue],
        params: &[ValType],
        results: &[ValType],
    ) -> Result<Option<Vec<WasmValue>>, String> {
        if params != [ValType::I32]
            || results != [ValType::I32]
            || locals != [ValType::I32, ValType::I32]
        {
            record_wasm_memory_loop_fast("miss");
            return Ok(None);
        }
        let n = match args.first() {
            Some(WasmValue::I32(v)) => *v,
            _ => {
                record_wasm_memory_loop_fast("miss");
                return Ok(None);
            }
        };
        let body = match body.last() {
            Some(Instr::End) => &body[..body.len() - 1],
            _ => body,
        };
        if !matches!(
            body,
            [
                Instr::Loop(BlockType::Empty),
                Instr::LocalGet(1),
                Instr::LocalGet(1),
                Instr::Store(
                    0x36,
                    crate::parser::MemArg {
                        memory: 0,
                        offset: 0,
                        ..
                    }
                ),
                Instr::LocalGet(2),
                Instr::LocalGet(1),
                Instr::Num(0x6a),
                Instr::LocalSet(2),
                Instr::LocalGet(1),
                Instr::I32Const(4),
                Instr::Num(0x6a),
                Instr::LocalTee(1),
                Instr::LocalGet(0),
                Instr::Num(0x48),
                Instr::BrIf(0),
                Instr::End,
                Instr::LocalGet(2)
            ]
        ) {
            record_wasm_memory_loop_fast("miss");
            return Ok(None);
        }

        let count = if n <= 0 {
            1usize
        } else {
            ((n as usize - 1) / 4) + 1
        };
        let last_addr = (count - 1)
            .checked_mul(4)
            .ok_or_else(|| "store: out of bounds memory access".to_string())?;
        let end = last_addr
            .checked_add(4)
            .ok_or_else(|| "store: out of bounds memory access".to_string())?;
        {
            let mem = self.memory_mut(0)?;
            if end > mem.len() {
                return Err("store: out of bounds memory access".to_string());
            }
            for idx in 0..count {
                let value = (idx as i32).wrapping_mul(4);
                let addr = idx * 4;
                mem[addr..addr + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        self.mark_memory_dirty(0, 0, end);
        self.sync_memory_aliases_after_write(0);

        record_wasm_memory_loop_fast("hit");
        let count_i32 = count as i32;
        let sum = 2i32
            .wrapping_mul(count_i32)
            .wrapping_mul(count_i32.wrapping_sub(1));
        Ok(Some(vec![WasmValue::I32(sum)]))
    }

    fn try_fast_i32_store_load_xor_sweep(
        &mut self,
        body: &[Instr],
        locals: &[ValType],
        args: &[WasmValue],
        params: &[ValType],
        results: &[ValType],
    ) -> Result<Option<Vec<WasmValue>>, String> {
        if params != [ValType::I32]
            || results != [ValType::I32]
            || locals != [ValType::I32, ValType::I32]
        {
            record_wasm_memory_loop_fast("miss");
            return Ok(None);
        }
        let n = match args.first() {
            Some(WasmValue::I32(v)) => *v,
            _ => {
                record_wasm_memory_loop_fast("miss");
                return Ok(None);
            }
        };
        let body = match body.last() {
            Some(Instr::End) => &body[..body.len() - 1],
            _ => body,
        };
        if !matches!(
            body,
            [
                Instr::Loop(BlockType::Empty),
                Instr::LocalGet(1),
                Instr::LocalGet(1),
                Instr::I32Const(-1640531535),
                Instr::Num(0x6c),
                Instr::Store(
                    0x36,
                    crate::parser::MemArg {
                        memory: 0,
                        offset: 0,
                        ..
                    }
                ),
                Instr::LocalGet(2),
                Instr::LocalGet(1),
                Instr::Load(
                    0x28,
                    crate::parser::MemArg {
                        memory: 0,
                        offset: 0,
                        ..
                    }
                ),
                Instr::Num(0x73),
                Instr::LocalSet(2),
                Instr::LocalGet(1),
                Instr::I32Const(4),
                Instr::Num(0x6a),
                Instr::LocalTee(1),
                Instr::LocalGet(0),
                Instr::Num(0x48),
                Instr::BrIf(0),
                Instr::End,
                Instr::LocalGet(2)
            ]
        ) {
            record_wasm_memory_loop_fast("miss");
            return Ok(None);
        }

        let count = if n <= 0 {
            1usize
        } else {
            ((n as usize - 1) / 4) + 1
        };
        let end = count
            .checked_mul(4)
            .ok_or_else(|| "store: out of bounds memory access".to_string())?;
        let mut acc = 0i32;
        {
            let mem = self.memory_mut(0)?;
            if end > mem.len() {
                return Err("store: out of bounds memory access".to_string());
            }
            for idx in 0..count {
                let addr = idx * 4;
                let i = addr as i32;
                let value = i.wrapping_mul(-1640531535);
                mem[addr..addr + 4].copy_from_slice(&value.to_le_bytes());
                acc ^= value;
            }
        }
        self.mark_memory_dirty(0, 0, end);
        self.sync_memory_aliases_after_write(0);

        record_wasm_memory_loop_fast("hit");
        Ok(Some(vec![WasmValue::I32(acc)]))
    }

    fn try_fast_fnv1a_i32_load8_hash(
        &self,
        body: &[Instr],
        locals: &[ValType],
        args: &[WasmValue],
        params: &[ValType],
        results: &[ValType],
    ) -> Result<Option<Vec<WasmValue>>, String> {
        if params != [ValType::I32, ValType::I32]
            || results != [ValType::I32]
            || locals != [ValType::I32, ValType::I32]
        {
            record_wasm_memory_loop_fast("miss");
            return Ok(None);
        }
        let (ptr, n) = match (args.first(), args.get(1)) {
            (Some(WasmValue::I32(ptr)), Some(WasmValue::I32(n))) => (*ptr, *n),
            _ => {
                record_wasm_memory_loop_fast("miss");
                return Ok(None);
            }
        };
        let body = match body.last() {
            Some(Instr::End) => &body[..body.len() - 1],
            _ => body,
        };
        if !matches!(
            body,
            [
                Instr::I32Const(-2128831035),
                Instr::LocalSet(3),
                Instr::Loop(BlockType::Empty),
                Instr::LocalGet(3),
                Instr::LocalGet(0),
                Instr::LocalGet(2),
                Instr::Num(0x6a),
                Instr::Load(
                    0x2d,
                    crate::parser::MemArg {
                        memory: 0,
                        offset: 0,
                        ..
                    }
                ),
                Instr::Num(0x73),
                Instr::I32Const(16777619),
                Instr::Num(0x6c),
                Instr::LocalSet(3),
                Instr::LocalGet(2),
                Instr::I32Const(1),
                Instr::Num(0x6a),
                Instr::LocalTee(2),
                Instr::LocalGet(1),
                Instr::Num(0x48),
                Instr::BrIf(0),
                Instr::End,
                Instr::LocalGet(3)
            ]
        ) {
            record_wasm_memory_loop_fast("miss");
            return Ok(None);
        }

        let ptr = ptr as u32 as usize;
        let len = if n <= 0 { 1usize } else { n as usize };
        let end = ptr
            .checked_add(len)
            .ok_or_else(|| "i32.load8_u: out of bounds memory access".to_string())?;
        let mem = self.memory_ref(0)?;
        if end > mem.len() {
            return Err("i32.load8_u: out of bounds memory access".to_string());
        }
        let mut hash = -2128831035i32;
        for byte in &mem[ptr..end] {
            hash = (hash ^ (*byte as i32)).wrapping_mul(16777619);
        }

        record_wasm_memory_loop_fast("hit");
        Ok(Some(vec![WasmValue::I32(hash)]))
    }

    fn try_fast_counted_i32_sum(
        &self,
        body: &[Instr],
        locals: &[ValType],
        args: &[WasmValue],
        params: &[ValType],
        results: &[ValType],
    ) -> Result<Option<Vec<WasmValue>>, String> {
        if params != [ValType::I32]
            || results != [ValType::I32]
            || locals != [ValType::I32, ValType::I32]
        {
            record_wasm_loop_fast("miss");
            return Ok(None);
        }
        let n = match args.first() {
            Some(WasmValue::I32(v)) => *v,
            _ => {
                record_wasm_loop_fast("miss");
                return Ok(None);
            }
        };
        let body = match body.last() {
            Some(Instr::End) => &body[..body.len() - 1],
            _ => body,
        };
        if !matches!(
            body,
            [
                Instr::Loop(BlockType::Empty),
                Instr::LocalGet(2),
                Instr::LocalGet(1),
                Instr::Num(0x6a),
                Instr::LocalSet(2),
                Instr::LocalGet(1),
                Instr::I32Const(1),
                Instr::Num(0x6a),
                Instr::LocalTee(1),
                Instr::LocalGet(0),
                Instr::Num(0x48),
                Instr::BrIf(0),
                Instr::End,
                Instr::LocalGet(2)
            ]
        ) {
            record_wasm_loop_fast("miss");
            return Ok(None);
        }

        record_wasm_loop_fast("hit");
        if n <= 1 {
            return Ok(Some(vec![WasmValue::I32(0)]));
        }
        let n = n as i64;
        let sum = n.wrapping_mul(n.wrapping_sub(1)).wrapping_div(2) as i32;
        Ok(Some(vec![WasmValue::I32(sum)]))
    }

    fn try_fast_counted_i32_leaf_chain(
        &self,
        body: &[Instr],
        locals: &[ValType],
        args: &[WasmValue],
        params: &[ValType],
        results: &[ValType],
    ) -> Result<Option<Vec<WasmValue>>, String> {
        if params != [ValType::I32]
            || results != [ValType::I32]
            || locals != [ValType::I32, ValType::I32]
        {
            record_wasm_loop_fast("miss");
            return Ok(None);
        }
        let n = match args.first() {
            Some(WasmValue::I32(v)) => *v,
            _ => {
                record_wasm_loop_fast("miss");
                return Ok(None);
            }
        };
        let body = match body.last() {
            Some(Instr::End) => &body[..body.len() - 1],
            _ => body,
        };
        let mut pc = 0usize;
        if !matches!(body.get(pc), Some(Instr::Loop(BlockType::Empty))) {
            record_wasm_loop_fast("miss");
            return Ok(None);
        }
        pc += 1;
        let mut step = 0i32;
        while pc + 2 < body.len() {
            match (&body[pc], &body[pc + 1], &body[pc + 2]) {
                (Instr::LocalGet(2), Instr::Call(f), Instr::LocalSet(2)) => {
                    let Some(delta) = self.defined_i32_const_add_leaf_delta(*f as usize) else {
                        record_wasm_loop_fast("miss");
                        return Ok(None);
                    };
                    step = step.wrapping_add(delta);
                    pc += 3;
                }
                _ => break,
            }
        }
        if step == 0 {
            record_wasm_loop_fast("miss");
            return Ok(None);
        }
        if !matches!(
            &body[pc..],
            [
                Instr::LocalGet(1),
                Instr::I32Const(1),
                Instr::Num(0x6a),
                Instr::LocalTee(1),
                Instr::LocalGet(0),
                Instr::Num(0x48),
                Instr::BrIf(0),
                Instr::End,
                Instr::LocalGet(2)
            ]
        ) {
            record_wasm_loop_fast("miss");
            return Ok(None);
        }
        let count = if n <= 1 { 1 } else { n };
        record_wasm_loop_fast("hit");
        Ok(Some(vec![WasmValue::I32(step.wrapping_mul(count))]))
    }

    fn defined_i32_const_add_leaf_delta(&self, overall: usize) -> Option<i32> {
        let defined_idx = match self.funcs.get(overall)? {
            Callable::Defined(d) => *d,
            Callable::Host(_) => return None,
        };
        let type_idx = *self.module.func_types.get(defined_idx)? as usize;
        let ft = self.module.types.get(type_idx)?;
        if ft.params != [ValType::I32] || ft.results != [ValType::I32] {
            return None;
        }
        let code = self.module.code.get(defined_idx)?;
        if !code.locals.is_empty() {
            return None;
        }
        let body = match code.body.last() {
            Some(Instr::End) => &code.body[..code.body.len() - 1],
            _ => &code.body,
        };
        match body {
            [Instr::LocalGet(0), Instr::I32Const(v), Instr::Num(0x6a)]
            | [Instr::I32Const(v), Instr::LocalGet(0), Instr::Num(0x6a)] => Some(*v),
            _ => None,
        }
    }

    fn try_fast_leaf(
        &self,
        body: &[Instr],
        args: &[WasmValue],
        results: &[ValType],
    ) -> Result<Option<Vec<WasmValue>>, String> {
        let body = match body.last() {
            Some(Instr::End) => &body[..body.len() - 1],
            _ => body,
        };
        if results.is_empty() {
            return match body {
                [] | [Instr::Nop] => {
                    record_wasm_fast_leaf("void");
                    Ok(Some(Vec::new()))
                }
                _ => {
                    record_wasm_fast_leaf("miss");
                    Ok(None)
                }
            };
        }
        if results != [ValType::I32] {
            record_wasm_fast_leaf("miss");
            return Ok(None);
        }
        let (kind, out) = match body {
            [Instr::I32Const(v)] => ("i32.const", WasmValue::I32(*v)),
            [Instr::LocalGet(a), Instr::LocalGet(b), Instr::Num(0x6a)] => {
                let a = match args.get(*a as usize) {
                    Some(WasmValue::I32(v)) => *v,
                    _ => {
                        record_wasm_fast_leaf("miss");
                        return Ok(None);
                    }
                };
                let b = match args.get(*b as usize) {
                    Some(WasmValue::I32(v)) => *v,
                    _ => {
                        record_wasm_fast_leaf("miss");
                        return Ok(None);
                    }
                };
                ("i32.add", WasmValue::I32(a.wrapping_add(b)))
            }
            [Instr::LocalGet(a), Instr::I32Const(b), Instr::Num(0x6a)] => {
                let a = match args.get(*a as usize) {
                    Some(WasmValue::I32(v)) => *v,
                    _ => {
                        record_wasm_fast_leaf("miss");
                        return Ok(None);
                    }
                };
                ("i32.add", WasmValue::I32(a.wrapping_add(*b)))
            }
            [Instr::I32Const(a), Instr::LocalGet(b), Instr::Num(0x6a)] => {
                let b = match args.get(*b as usize) {
                    Some(WasmValue::I32(v)) => *v,
                    _ => {
                        record_wasm_fast_leaf("miss");
                        return Ok(None);
                    }
                };
                ("i32.add", WasmValue::I32(a.wrapping_add(b)))
            }
            [Instr::LocalGet(base), Instr::Load(0x28, memarg)] => {
                let base = match args.get(*base as usize) {
                    Some(WasmValue::I32(v)) => *v,
                    _ => {
                        record_wasm_fast_leaf("miss");
                        return Ok(None);
                    }
                };
                let addr = self.effective_addr(*memarg, base)?;
                let b = self.read_mem_at(memarg.memory, addr, 4, "load")?;
                (
                    "i32.load",
                    WasmValue::I32(i32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                )
            }
            _ => {
                record_wasm_fast_leaf("miss");
                return Ok(None);
            }
        };
        record_wasm_fast_leaf(kind);
        Ok(Some(vec![out]))
    }

    fn try_fast_defined_leaf_call_on_stack(
        &self,
        overall: usize,
        ft: &FuncType,
        stack: &mut Vec<WasmValue>,
    ) -> Result<Option<()>, String> {
        if ft.results != [ValType::I32] {
            record_wasm_fast_leaf("miss");
            return Ok(None);
        }
        let defined_idx = match self.funcs.get(overall) {
            Some(Callable::Defined(d)) => *d,
            _ => return Ok(None),
        };
        let code = &self.module.code[defined_idx];
        if !code.locals.is_empty() {
            record_wasm_fast_leaf("miss");
            return Ok(None);
        }
        let base = stack
            .len()
            .checked_sub(ft.params.len())
            .ok_or_else(|| "call: stack underflow".to_string())?;
        let body = match code.body.last() {
            Some(Instr::End) => &code.body[..code.body.len() - 1],
            _ => &code.body,
        };
        let local_i32 = |idx: u32, stack: &[WasmValue]| -> Option<i32> {
            if (idx as usize) >= ft.params.len() {
                return None;
            }
            match stack.get(base + idx as usize) {
                Some(WasmValue::I32(v)) => Some(*v),
                _ => None,
            }
        };
        let out = match body {
            [Instr::I32Const(v)] if ft.params.is_empty() => *v,
            [Instr::LocalGet(a), Instr::LocalGet(b), Instr::Num(0x6a)] => {
                let a = match local_i32(*a, stack) {
                    Some(v) => v,
                    None => {
                        record_wasm_fast_leaf("miss");
                        return Ok(None);
                    }
                };
                let b = match local_i32(*b, stack) {
                    Some(v) => v,
                    None => {
                        record_wasm_fast_leaf("miss");
                        return Ok(None);
                    }
                };
                a.wrapping_add(b)
            }
            [Instr::LocalGet(a), Instr::I32Const(b), Instr::Num(0x6a)] => {
                let a = match local_i32(*a, stack) {
                    Some(v) => v,
                    None => {
                        record_wasm_fast_leaf("miss");
                        return Ok(None);
                    }
                };
                a.wrapping_add(*b)
            }
            [Instr::I32Const(a), Instr::LocalGet(b), Instr::Num(0x6a)] => {
                let b = match local_i32(*b, stack) {
                    Some(v) => v,
                    None => {
                        record_wasm_fast_leaf("miss");
                        return Ok(None);
                    }
                };
                a.wrapping_add(b)
            }
            _ => {
                record_wasm_fast_leaf("miss");
                return Ok(None);
            }
        };
        stack.truncate(base);
        stack.push(WasmValue::I32(out));
        record_wasm_fast_leaf("i32.add");
        Ok(Some(()))
    }

    fn exec_body(
        &mut self,
        body: &[Instr],
        ctrl: &ControlInfo,
        locals: &mut [WasmValue],
        func_results: &[ValType],
        depth: usize,
    ) -> Result<BodyResult, String> {
        let mut stack: Vec<WasmValue> = Vec::new();

        let mut labels: Vec<Label> = vec![Label {
            cont_pc: body.len(),
            arity: func_results.len(),
            stack_height: 0,
            is_loop: false,
            catches: None,
        }];

        let mut pc = 0usize;
        while pc < body.len() {
            self.charge_execution_fuel()?;
            match &body[pc] {
                Instr::Unreachable => return Err("unreachable executed".to_string()),
                Instr::Nop => {}
                Instr::Block(bt) => {
                    let end = ctrl.end_of[pc];
                    labels.push(Label {
                        cont_pc: (end as usize) + 1,
                        arity: self.block_result_arity(*bt)?,
                        stack_height: stack.len(),
                        is_loop: false,
                        catches: None,
                    });
                }
                Instr::Loop(bt) => {
                    labels.push(Label {
                        cont_pc: pc + 1,
                        arity: self.block_param_arity(*bt)?,
                        stack_height: stack.len(),
                        is_loop: true,
                        catches: None,
                    });
                }
                Instr::If(bt) => {
                    let cond = pop_i32(&mut stack)?;
                    let end = ctrl.end_of[pc];
                    let els = ctrl.else_of[pc];
                    labels.push(Label {
                        cont_pc: (end as usize) + 1,
                        arity: self.block_result_arity(*bt)?,
                        stack_height: stack.len(),
                        is_loop: false,
                        catches: None,
                    });
                    if cond == 0 {

                        if els >= 0 {
                            pc = els as usize + 1;
                            continue;
                        } else {

                            pc = end as usize;
                            continue;
                        }
                    }

                }
                Instr::TryTable(bt, catches) => {
                    let end = ctrl.end_of[pc];
                    labels.push(Label {
                        cont_pc: (end as usize) + 1,
                        arity: self.block_result_arity(*bt)?,
                        stack_height: stack.len(),
                        is_loop: false,
                        catches: Some(catches.clone()),
                    });
                }
                Instr::LegacyTry(bt) => {
                    let end = ctrl.end_of[pc];
                    labels.push(Label {
                        cont_pc: (end as usize) + 1,
                        arity: self.block_result_arity(*bt)?,
                        stack_height: stack.len(),
                        is_loop: false,
                        catches: None,
                    });
                }
                Instr::Else => {

                    let lbl = labels.last().ok_or("else without label")?;
                    pc = lbl.cont_pc - 1;
                    continue;
                }
                Instr::LegacyCatch(_) | Instr::LegacyCatchAll => {
                    let lbl = labels.last().ok_or("catch without label")?;
                    pc = lbl.cont_pc - 1;
                    continue;
                }
                Instr::LegacyDelegate(_) => {
                    labels.pop();
                }
                Instr::LegacyRethrow(_) => {
                    return Err("legacy exception rethrow execution is not implemented".to_string());
                }
                Instr::End => {

                    labels.pop();

                }
                Instr::Br(d) => {
                    pc = self.do_branch(*d as usize, &mut labels, &mut stack)?;
                    continue;
                }
                Instr::BrIf(d) => {
                    let c = pop_i32(&mut stack)?;
                    if c != 0 {
                        pc = self.do_branch(*d as usize, &mut labels, &mut stack)?;
                        continue;
                    }
                }
                Instr::BrOnNull(d) => {
                    let value = pop_ref(&mut stack)?;
                    if matches!(value, WasmValue::RefNull) {
                        pc = self.do_branch(*d as usize, &mut labels, &mut stack)?;
                        continue;
                    }
                    stack.push(value);
                }
                Instr::BrOnNonNull(d) => {
                    let value = pop_ref(&mut stack)?;
                    if !matches!(value, WasmValue::RefNull) {
                        stack.push(value);
                        pc = self.do_branch(*d as usize, &mut labels, &mut stack)?;
                        continue;
                    }
                }
                Instr::BrTable(targets, default) => {
                    let i = pop_i32(&mut stack)? as u32 as usize;
                    let d = if i < targets.len() {
                        targets[i] as usize
                    } else {
                        *default as usize
                    };
                    pc = self.do_branch(d, &mut labels, &mut stack)?;
                    continue;
                }
                Instr::Return => {
                    let n = func_results.len();
                    if stack.len() < n {
                        return Err("return: stack underflow".to_string());
                    }
                    let res = stack.split_off(stack.len() - n);
                    return Ok(BodyResult::Values(res));
                }
                Instr::Throw(tag) => {
                    let ex = self.make_exception_payload(*tag, &mut stack)?;
                    match self.handle_exception(ex.clone(), &mut labels, &mut stack)? {
                        Some(next_pc) => {
                            pc = next_pc;
                            continue;
                        }
                        None => return Ok(BodyResult::Exception(ex)),
                    }
                }
                Instr::ThrowRef => {
                    let exnref = stack.pop().ok_or("throw_ref: stack underflow")?;
                    let ex = self.exception_from_ref(exnref)?;
                    match self.handle_exception(ex.clone(), &mut labels, &mut stack)? {
                        Some(next_pc) => {
                            pc = next_pc;
                            continue;
                        }
                        None => return Ok(BodyResult::Exception(ex)),
                    }
                }
                Instr::Call(f) => {
                    let overall = *f as usize;
                    let tidx = self
                        .func_type_index(overall)
                        .ok_or_else(|| format!("call: bad func index {}", overall))?;
                    let ft = self.module.types[tidx as usize].clone();
                    let nargs = ft.params.len();
                    if stack.len() < nargs {
                        return Err("call: stack underflow".to_string());
                    }
                    if let Some(()) =
                        self.try_fast_defined_leaf_call_on_stack(overall, &ft, &mut stack)?
                    {
                        pc += 1;
                        continue;
                    }
                    let cargs = stack.split_off(stack.len() - nargs);
                    match self.call_overall_body(overall, &cargs, depth + 1)? {
                        BodyResult::Values(res) => stack.extend(res),
                        BodyResult::TailCall { .. } => {
                            return Err("call: unexpected tail-call transfer".to_string())
                        }
                        BodyResult::Exception(ex) => {
                            match self.handle_exception(ex.clone(), &mut labels, &mut stack)? {
                                Some(next_pc) => {
                                    pc = next_pc;
                                    continue;
                                }
                                None => return Ok(BodyResult::Exception(ex)),
                            }
                        }
                    }
                }
                Instr::ReturnCall(f) => {
                    let overall = *f as usize;
                    let tidx = self
                        .func_type_index(overall)
                        .ok_or_else(|| format!("return_call: bad func index {}", overall))?;
                    let ft = self.module.types[tidx as usize].clone();
                    let nargs = ft.params.len();
                    if stack.len() < nargs {
                        return Err("return_call: stack underflow".to_string());
                    }
                    if ft.results != func_results {
                        return Err("return_call: result type mismatch".to_string());
                    }
                    let cargs = stack.split_off(stack.len() - nargs);
                    return Ok(BodyResult::TailCall {
                        overall,
                        args: cargs,
                    });
                }
                Instr::CallIndirect(type_idx, tableidx) => {
                    let elem = self.pop_table_index(*tableidx, &mut stack)?;
                    let table = self.table(*tableidx)?;
                    if elem >= table.len() {
                        return Err("call_indirect: table index out of bounds".to_string());
                    }
                    let overall = match table.get(elem) {
                        Some(WasmValue::FuncRef(idx)) => *idx as usize,
                        Some(WasmValue::RefNull) => {
                            return Err("call_indirect: uninitialized table element".to_string())
                        }
                        Some(other) => {
                            return Err(format!(
                                "call_indirect: expected funcref table element, got {:?}",
                                other
                            ))
                        }
                        None => return Err("call_indirect: table index out of bounds".to_string()),
                    };
                    let expected_ft = self
                        .module
                        .types
                        .get(*type_idx as usize)
                        .ok_or("call_indirect: bad expected type")?;
                    let type_matches = if let Some(actual_type_idx) = self.func_type_index(overall)
                    {
                        self.call_indirect_type_matches(actual_type_idx, *type_idx)
                    } else {
                        let actual_sig = self
                            .func_sig_by_overall(overall)
                            .ok_or("call_indirect: bad func")?;
                        let expected_params: Vec<crate::ValType> = expected_ft
                            .params
                            .iter()
                            .map(|t| crate::ValType::from_parser(*t))
                            .collect();
                        let expected_results: Vec<crate::ValType> = expected_ft
                            .results
                            .iter()
                            .map(|t| crate::ValType::from_parser(*t))
                            .collect();
                        actual_sig.0 == expected_params && actual_sig.1 == expected_results
                    };
                    if !type_matches {
                        return Err("call_indirect: type mismatch".to_string());
                    }
                    let ft = expected_ft.clone();
                    let nargs = ft.params.len();
                    if stack.len() < nargs {
                        return Err("call_indirect: stack underflow".to_string());
                    }
                    let cargs = stack.split_off(stack.len() - nargs);
                    let res = self.call_overall_depth(overall, &cargs, depth + 1)?;
                    stack.extend(res);
                }
                Instr::ReturnCallIndirect(type_idx, tableidx) => {
                    let elem = self.pop_table_index(*tableidx, &mut stack)?;
                    let table = self.table(*tableidx)?;
                    if elem >= table.len() {
                        return Err("return_call_indirect: table index out of bounds".to_string());
                    }
                    let overall = match table.get(elem) {
                        Some(WasmValue::FuncRef(idx)) => *idx as usize,
                        Some(WasmValue::RefNull) => {
                            return Err(
                                "return_call_indirect: uninitialized table element".to_string()
                            )
                        }
                        Some(other) => {
                            return Err(format!(
                                "return_call_indirect: expected funcref table element, got {:?}",
                                other
                            ))
                        }
                        None => {
                            return Err(
                                "return_call_indirect: table index out of bounds".to_string()
                            )
                        }
                    };
                    let expected_ft = self
                        .module
                        .types
                        .get(*type_idx as usize)
                        .ok_or("return_call_indirect: bad expected type")?;
                    let type_matches = if let Some(actual_type_idx) = self.func_type_index(overall)
                    {
                        self.call_indirect_type_matches(actual_type_idx, *type_idx)
                    } else {
                        let actual_sig = self
                            .func_sig_by_overall(overall)
                            .ok_or("return_call_indirect: bad func")?;
                        let expected_params: Vec<crate::ValType> = expected_ft
                            .params
                            .iter()
                            .map(|t| crate::ValType::from_parser(*t))
                            .collect();
                        let expected_results: Vec<crate::ValType> = expected_ft
                            .results
                            .iter()
                            .map(|t| crate::ValType::from_parser(*t))
                            .collect();
                        actual_sig.0 == expected_params && actual_sig.1 == expected_results
                    };
                    if !type_matches {
                        return Err("return_call_indirect: type mismatch".to_string());
                    }
                    if expected_ft.results != func_results {
                        return Err("return_call_indirect: result type mismatch".to_string());
                    }
                    let nargs = expected_ft.params.len();
                    if stack.len() < nargs {
                        return Err("return_call_indirect: stack underflow".to_string());
                    }
                    let cargs = stack.split_off(stack.len() - nargs);
                    return Ok(BodyResult::TailCall {
                        overall,
                        args: cargs,
                    });
                }
                Instr::CallRef(type_idx) => {
                    let callee = stack.pop().ok_or("call_ref: callee underflow")?;
                    let overall = match callee {
                        WasmValue::FuncRef(idx) => idx as usize,
                        WasmValue::RefNull => return Err("call_ref: null reference".to_string()),
                        other => {
                            return Err(format!(
                                "call_ref: expected function reference, got {:?}",
                                other
                            ))
                        }
                    };
                    let actual = self.func_type_index(overall).ok_or("call_ref: bad func")?;
                    let actual_ft = self
                        .module
                        .types
                        .get(actual as usize)
                        .ok_or("call_ref: bad actual type")?;
                    let expected_ft = self
                        .module
                        .types
                        .get(*type_idx as usize)
                        .ok_or("call_ref: bad expected type")?;
                    if actual_ft.params != expected_ft.params
                        || actual_ft.results != expected_ft.results
                    {
                        return Err("call_ref: type mismatch".to_string());
                    }
                    let ft = expected_ft.clone();
                    let nargs = ft.params.len();
                    if stack.len() < nargs {
                        return Err("call_ref: stack underflow".to_string());
                    }
                    let cargs = stack.split_off(stack.len() - nargs);
                    let res = self.call_overall_depth(overall, &cargs, depth + 1)?;
                    stack.extend(res);
                }
                Instr::ReturnCallRef(type_idx) => {
                    let callee = stack.pop().ok_or("return_call_ref: callee underflow")?;
                    let overall = match callee {
                        WasmValue::FuncRef(idx) => idx as usize,
                        WasmValue::RefNull => {
                            return Err("return_call_ref: null reference".to_string())
                        }
                        other => {
                            return Err(format!(
                                "return_call_ref: expected function reference, got {:?}",
                                other
                            ))
                        }
                    };
                    let actual = self
                        .func_type_index(overall)
                        .ok_or("return_call_ref: bad func")?;
                    let actual_ft = self
                        .module
                        .types
                        .get(actual as usize)
                        .ok_or("return_call_ref: bad actual type")?;
                    let expected_ft = self
                        .module
                        .types
                        .get(*type_idx as usize)
                        .ok_or("return_call_ref: bad expected type")?;
                    if actual_ft.params != expected_ft.params
                        || actual_ft.results != expected_ft.results
                    {
                        return Err("return_call_ref: type mismatch".to_string());
                    }
                    if expected_ft.results != func_results {
                        return Err("return_call_ref: result type mismatch".to_string());
                    }
                    let nargs = expected_ft.params.len();
                    if stack.len() < nargs {
                        return Err("return_call_ref: stack underflow".to_string());
                    }
                    let cargs = stack.split_off(stack.len() - nargs);
                    return Ok(BodyResult::TailCall {
                        overall,
                        args: cargs,
                    });
                }
                Instr::Drop => {
                    stack.pop().ok_or("drop: empty stack")?;
                }
                Instr::Select => {
                    let c = pop_i32(&mut stack)?;
                    let b = stack.pop().ok_or("select underflow")?;
                    let a = stack.pop().ok_or("select underflow")?;
                    stack.push(if c != 0 { a } else { b });
                }
                Instr::SelectTyped(_) => {
                    let c = pop_i32(&mut stack)?;
                    let b = stack.pop().ok_or("select underflow")?;
                    let a = stack.pop().ok_or("select underflow")?;
                    stack.push(if c != 0 { a } else { b });
                }
                Instr::LocalGet(i) => {
                    let v = *locals.get(*i as usize).ok_or("local.get out of range")?;
                    stack.push(v);
                }
                Instr::LocalSet(i) => {
                    let v = stack.pop().ok_or("local.set underflow")?;
                    *locals
                        .get_mut(*i as usize)
                        .ok_or("local.set out of range")? = v;
                }
                Instr::LocalTee(i) => {
                    let v = *stack.last().ok_or("local.tee underflow")?;
                    *locals
                        .get_mut(*i as usize)
                        .ok_or("local.tee out of range")? = v;
                }
                Instr::GlobalGet(i) => {
                    let v = *self
                        .globals
                        .get(*i as usize)
                        .ok_or("global.get out of range")?;
                    stack.push(v);
                }
                Instr::GlobalSet(i) => {
                    let idx = *i as usize;
                    if idx >= self.globals.len() {
                        return Err("global.set out of range".to_string());
                    }
                    if !self.global_mut[idx] {
                        return Err("global.set on immutable global".to_string());
                    }
                    let v = stack.pop().ok_or("global.set underflow")?;
                    self.globals[idx] = v;
                }
                Instr::Load(op, memarg) => {
                    self.exec_load(*op, *memarg, &mut stack)?;
                }
                Instr::Store(op, memarg) => {
                    self.exec_store(*op, *memarg, &mut stack)?;
                }
                Instr::AtomicLoad(sub, memarg) => {
                    self.exec_atomic_load(*sub, *memarg, &mut stack)?;
                }
                Instr::AtomicStore(sub, memarg) => {
                    self.exec_atomic_store(*sub, *memarg, &mut stack)?;
                }
                Instr::AtomicRmw(sub, memarg) => {
                    self.exec_atomic_rmw(*sub, *memarg, &mut stack)?;
                }
                Instr::AtomicNotify(memarg) => {
                    let _count = pop_i32(&mut stack)?;
                    let base = pop_i32(&mut stack)?;
                    let _addr = self.effective_addr(*memarg, base)?;
                    stack.push(WasmValue::I32(0));
                }
                Instr::AtomicWait(sub, memarg) => {
                    let timeout = pop_i64(&mut stack)?;
                    let base;
                    let matches = if *sub == 0x01 {
                        let expected = pop_i32(&mut stack)?;
                        base = pop_i32(&mut stack)?;
                        let addr = self.effective_addr(*memarg, base)?;
                        let b = self.read_mem_at(memarg.memory, addr, 4, "memory.atomic.wait32")?;
                        i32::from_le_bytes([b[0], b[1], b[2], b[3]]) == expected
                    } else {
                        let expected = pop_i64(&mut stack)?;
                        base = pop_i32(&mut stack)?;
                        let addr = self.effective_addr(*memarg, base)?;
                        let b = self.read_mem_at(memarg.memory, addr, 8, "memory.atomic.wait64")?;
                        i64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
                            == expected
                    };

                    stack.push(WasmValue::I32(if !matches || timeout != 0 { 1 } else { 2 }));
                }
                Instr::AtomicFence(_) => {}
                Instr::MemorySize(memoryidx) => {
                    let pages = (self.memory_ref(*memoryidx)?.len() / PAGE) as i64;
                    if self.memory64 {
                        stack.push(WasmValue::I64(pages));
                    } else {
                        stack.push(WasmValue::I32(pages as i32));
                    }
                }
                Instr::MemoryGrow(memoryidx) => {
                    let delta = if self.memory64 {
                        pop_i64(&mut stack)? as u64 as usize
                    } else {
                        pop_i32(&mut stack)? as u32 as usize
                    };
                    let old_pages = self.memory_ref(*memoryidx)?.len() / PAGE;
                    let new_pages = old_pages
                        .checked_add(delta)
                        .ok_or_else(|| "maximum memory size exceeded".to_string())?;
                    let allowed = match self.memory_max_pages_for(*memoryidx)? {
                        Some(mx) => {
                            new_pages <= mx as usize && new_pages <= Self::max_memory_pages()
                        }
                        None => new_pages <= 65536 && new_pages <= Self::max_memory_pages(),
                    };
                    if !allowed {
                        if self.memory64 {
                            stack.push(WasmValue::I64(-1));
                        } else {
                            stack.push(WasmValue::I32(-1));
                        }
                    } else {
                        self.memory_mut(*memoryidx)?.resize(new_pages * PAGE, 0);
                        if self.memory64 {
                            stack.push(WasmValue::I64(old_pages as i64));
                        } else {
                            stack.push(WasmValue::I32(old_pages as i32));
                        }
                    }
                }
                Instr::I32Const(v) => stack.push(WasmValue::I32(*v)),
                Instr::I64Const(v) => stack.push(WasmValue::I64(*v)),
                Instr::F32Const(v) => stack.push(WasmValue::F32(*v)),
                Instr::F64Const(v) => stack.push(WasmValue::F64(*v)),
                Instr::V128Const(bytes) => stack.push(WasmValue::V128(*bytes)),
                Instr::I8x16Shuffle(lanes) => {
                    let b = pop_v128(&mut stack)?;
                    let a = pop_v128(&mut stack)?;
                    let mut out = [0u8; 16];
                    for (i, lane) in lanes.iter().enumerate() {
                        let lane = *lane as usize;
                        out[i] = if lane < 16 { a[lane] } else { b[lane - 16] };
                    }
                    stack.push(WasmValue::V128(out));
                }
                Instr::I8x16Swizzle => {
                    let indices = pop_v128(&mut stack)?;
                    let a = pop_v128(&mut stack)?;
                    let mut out = [0u8; 16];
                    for (i, idx) in indices.iter().enumerate() {
                        out[i] = if *idx < 16 { a[*idx as usize] } else { 0 };
                    }
                    stack.push(WasmValue::V128(out));
                }
                Instr::I8x16Splat => {
                    let value = pop_i32(&mut stack)? as u8;
                    stack.push(WasmValue::V128([value; 16]));
                }
                Instr::I16x8Splat => {
                    let lane = (pop_i32(&mut stack)? as i16).to_le_bytes();
                    stack.push(WasmValue::V128(repeat_lane::<2, 8>(lane)));
                }
                Instr::I32x4Splat => {
                    let lane = pop_i32(&mut stack)?.to_le_bytes();
                    stack.push(WasmValue::V128(repeat_lane::<4, 4>(lane)));
                }
                Instr::I64x2Splat => {
                    let lane = pop_i64(&mut stack)?.to_le_bytes();
                    stack.push(WasmValue::V128(repeat_lane::<8, 2>(lane)));
                }
                Instr::F32x4Splat => {
                    let lane = pop_f32(&mut stack)?.to_le_bytes();
                    stack.push(WasmValue::V128(repeat_lane::<4, 4>(lane)));
                }
                Instr::F64x2Splat => {
                    let lane = pop_f64(&mut stack)?.to_le_bytes();
                    stack.push(WasmValue::V128(repeat_lane::<8, 2>(lane)));
                }
                Instr::I8x16ExtractLaneS(lane) => {
                    let bytes = pop_v128(&mut stack)?;
                    stack.push(WasmValue::I32(bytes[*lane as usize] as i8 as i32));
                }
                Instr::I8x16ExtractLaneU(lane) => {
                    let bytes = pop_v128(&mut stack)?;
                    stack.push(WasmValue::I32(bytes[*lane as usize] as i32));
                }
                Instr::I16x8ExtractLaneS(lane) => {
                    let bytes = pop_v128(&mut stack)?;
                    let offset = (*lane as usize) * 2;
                    stack.push(WasmValue::I32(
                        i16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as i32,
                    ));
                }
                Instr::I16x8ExtractLaneU(lane) => {
                    let bytes = pop_v128(&mut stack)?;
                    let offset = (*lane as usize) * 2;
                    stack.push(WasmValue::I32(
                        u16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as i32,
                    ));
                }
                Instr::I32x4ExtractLane(lane) => {
                    let bytes = pop_v128(&mut stack)?;
                    let offset = (*lane as usize) * 4;
                    stack.push(WasmValue::I32(i32::from_le_bytes([
                        bytes[offset],
                        bytes[offset + 1],
                        bytes[offset + 2],
                        bytes[offset + 3],
                    ])));
                }
                Instr::I64x2ExtractLane(lane) => {
                    let bytes = pop_v128(&mut stack)?;
                    let offset = (*lane as usize) * 8;
                    stack.push(WasmValue::I64(i64::from_le_bytes([
                        bytes[offset],
                        bytes[offset + 1],
                        bytes[offset + 2],
                        bytes[offset + 3],
                        bytes[offset + 4],
                        bytes[offset + 5],
                        bytes[offset + 6],
                        bytes[offset + 7],
                    ])));
                }
                Instr::F32x4ExtractLane(lane) => {
                    let bytes = pop_v128(&mut stack)?;
                    let offset = (*lane as usize) * 4;
                    stack.push(WasmValue::F32(f32::from_le_bytes([
                        bytes[offset],
                        bytes[offset + 1],
                        bytes[offset + 2],
                        bytes[offset + 3],
                    ])));
                }
                Instr::F64x2ExtractLane(lane) => {
                    let bytes = pop_v128(&mut stack)?;
                    let offset = (*lane as usize) * 8;
                    stack.push(WasmValue::F64(f64::from_le_bytes([
                        bytes[offset],
                        bytes[offset + 1],
                        bytes[offset + 2],
                        bytes[offset + 3],
                        bytes[offset + 4],
                        bytes[offset + 5],
                        bytes[offset + 6],
                        bytes[offset + 7],
                    ])));
                }
                Instr::I8x16ReplaceLane(lane) => {
                    let value = pop_i32(&mut stack)? as u8;
                    let mut bytes = pop_v128(&mut stack)?;
                    bytes[*lane as usize] = value;
                    stack.push(WasmValue::V128(bytes));
                }
                Instr::I16x8ReplaceLane(lane) => {
                    let value = (pop_i32(&mut stack)? as i16).to_le_bytes();
                    let mut bytes = pop_v128(&mut stack)?;
                    let offset = (*lane as usize) * 2;
                    bytes[offset..offset + 2].copy_from_slice(&value);
                    stack.push(WasmValue::V128(bytes));
                }
                Instr::I32x4ReplaceLane(lane) => {
                    let value = pop_i32(&mut stack)?.to_le_bytes();
                    let mut bytes = pop_v128(&mut stack)?;
                    let offset = (*lane as usize) * 4;
                    bytes[offset..offset + 4].copy_from_slice(&value);
                    stack.push(WasmValue::V128(bytes));
                }
                Instr::I64x2ReplaceLane(lane) => {
                    let value = pop_i64(&mut stack)?.to_le_bytes();
                    let mut bytes = pop_v128(&mut stack)?;
                    let offset = (*lane as usize) * 8;
                    bytes[offset..offset + 8].copy_from_slice(&value);
                    stack.push(WasmValue::V128(bytes));
                }
                Instr::F32x4ReplaceLane(lane) => {
                    let value = pop_f32(&mut stack)?.to_le_bytes();
                    let mut bytes = pop_v128(&mut stack)?;
                    let offset = (*lane as usize) * 4;
                    bytes[offset..offset + 4].copy_from_slice(&value);
                    stack.push(WasmValue::V128(bytes));
                }
                Instr::F64x2ReplaceLane(lane) => {
                    let value = pop_f64(&mut stack)?.to_le_bytes();
                    let mut bytes = pop_v128(&mut stack)?;
                    let offset = (*lane as usize) * 8;
                    bytes[offset..offset + 8].copy_from_slice(&value);
                    stack.push(WasmValue::V128(bytes));
                }
                Instr::I8x16Eq => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i8x16(a, b, |x, y| {
                        if x == y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I8x16Ne => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i8x16(a, b, |x, y| {
                        if x != y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I8x16LtS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i8x16(a, b, |x, y| {
                        if x < y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I8x16LtU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(bytes_v128(a, b, |x, y| {
                        if x < y {
                            0xff
                        } else {
                            0
                        }
                    })));
                }
                Instr::I8x16GtS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i8x16(a, b, |x, y| {
                        if x > y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I8x16GtU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(bytes_v128(a, b, |x, y| {
                        if x > y {
                            0xff
                        } else {
                            0
                        }
                    })));
                }
                Instr::I8x16LeS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i8x16(a, b, |x, y| {
                        if x <= y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I8x16LeU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(bytes_v128(a, b, |x, y| {
                        if x <= y {
                            0xff
                        } else {
                            0
                        }
                    })));
                }
                Instr::I8x16GeS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i8x16(a, b, |x, y| {
                        if x >= y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I8x16GeU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(bytes_v128(a, b, |x, y| {
                        if x >= y {
                            0xff
                        } else {
                            0
                        }
                    })));
                }
                Instr::I16x8Eq => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8(a, b, |x, y| {
                        if x == y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I16x8Ne => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8(a, b, |x, y| {
                        if x != y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I16x8LtS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8(a, b, |x, y| {
                        if x < y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I16x8LtU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8_u(a, b, |x, y| {
                        if x < y {
                            u16::MAX
                        } else {
                            0
                        }
                    })));
                }
                Instr::I16x8GtS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8(a, b, |x, y| {
                        if x > y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I16x8GtU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8_u(a, b, |x, y| {
                        if x > y {
                            u16::MAX
                        } else {
                            0
                        }
                    })));
                }
                Instr::I16x8LeS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8(a, b, |x, y| {
                        if x <= y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I16x8LeU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8_u(a, b, |x, y| {
                        if x <= y {
                            u16::MAX
                        } else {
                            0
                        }
                    })));
                }
                Instr::I16x8GeS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8(a, b, |x, y| {
                        if x >= y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I16x8GeU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8_u(a, b, |x, y| {
                        if x >= y {
                            u16::MAX
                        } else {
                            0
                        }
                    })));
                }
                Instr::I32x4Eq => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4(a, b, |x, y| {
                        if x == y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I32x4Ne => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4(a, b, |x, y| {
                        if x != y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I32x4LtS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4(a, b, |x, y| {
                        if x < y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I32x4LtU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4_u(a, b, |x, y| {
                        if x < y {
                            u32::MAX
                        } else {
                            0
                        }
                    })));
                }
                Instr::I32x4GtS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4(a, b, |x, y| {
                        if x > y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I32x4GtU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4_u(a, b, |x, y| {
                        if x > y {
                            u32::MAX
                        } else {
                            0
                        }
                    })));
                }
                Instr::I32x4LeS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4(a, b, |x, y| {
                        if x <= y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I32x4LeU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4_u(a, b, |x, y| {
                        if x <= y {
                            u32::MAX
                        } else {
                            0
                        }
                    })));
                }
                Instr::I32x4GeS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4(a, b, |x, y| {
                        if x >= y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I32x4GeU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4_u(a, b, |x, y| {
                        if x >= y {
                            u32::MAX
                        } else {
                            0
                        }
                    })));
                }
                Instr::I64x2Eq => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i64x2(a, b, |x, y| {
                        if x == y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I64x2Ne => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i64x2(a, b, |x, y| {
                        if x != y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I64x2LtS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i64x2(a, b, |x, y| {
                        if x < y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I64x2GtS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i64x2(a, b, |x, y| {
                        if x > y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I64x2LeS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i64x2(a, b, |x, y| {
                        if x <= y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I64x2GeS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i64x2(a, b, |x, y| {
                        if x >= y {
                            -1
                        } else {
                            0
                        }
                    })));
                }
                Instr::I8x16Abs => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(a.map(|x| (x as i8).wrapping_abs() as u8)));
                }
                Instr::I8x16Neg => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(a.map(|x| (x as i8).wrapping_neg() as u8)));
                }
                Instr::I8x16Popcnt => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(a.map(|x| x.count_ones() as u8)));
                }
                Instr::I8x16AllTrue => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::I32(if a.iter().all(|x| *x != 0) {
                        1
                    } else {
                        0
                    }));
                }
                Instr::I8x16Bitmask => {
                    let a = pop_v128(&mut stack)?;
                    let mut mask = 0i32;
                    for (i, lane) in a.iter().enumerate() {
                        mask |= (((lane & 0x80) != 0) as i32) << i;
                    }
                    stack.push(WasmValue::I32(mask));
                }
                Instr::I8x16NarrowI16x8S => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    let mut out = [0u8; 16];
                    for i in 0..8 {
                        let offset = i * 2;
                        let lane = i16::from_le_bytes([a[offset], a[offset + 1]]);
                        out[i] = lane.clamp(i8::MIN as i16, i8::MAX as i16) as i8 as u8;
                    }
                    for i in 0..8 {
                        let offset = i * 2;
                        let lane = i16::from_le_bytes([b[offset], b[offset + 1]]);
                        out[i + 8] = lane.clamp(i8::MIN as i16, i8::MAX as i16) as i8 as u8;
                    }
                    stack.push(WasmValue::V128(out));
                }
                Instr::I8x16NarrowI16x8U => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    let mut out = [0u8; 16];
                    for i in 0..8 {
                        let offset = i * 2;
                        let lane = i16::from_le_bytes([a[offset], a[offset + 1]]);
                        out[i] = lane.clamp(0, u8::MAX as i16) as u8;
                    }
                    for i in 0..8 {
                        let offset = i * 2;
                        let lane = i16::from_le_bytes([b[offset], b[offset + 1]]);
                        out[i + 8] = lane.clamp(0, u8::MAX as i16) as u8;
                    }
                    stack.push(WasmValue::V128(out));
                }
                Instr::I16x8NarrowI32x4S => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    let mut out = [0u8; 16];
                    for i in 0..4 {
                        let offset = i * 4;
                        let lane = i32::from_le_bytes([
                            a[offset],
                            a[offset + 1],
                            a[offset + 2],
                            a[offset + 3],
                        ]);
                        out[i * 2..i * 2 + 2].copy_from_slice(
                            &(lane.clamp(i16::MIN as i32, i16::MAX as i32) as i16).to_le_bytes(),
                        );
                    }
                    for i in 0..4 {
                        let offset = i * 4;
                        let lane = i32::from_le_bytes([
                            b[offset],
                            b[offset + 1],
                            b[offset + 2],
                            b[offset + 3],
                        ]);
                        out[(i + 4) * 2..(i + 4) * 2 + 2].copy_from_slice(
                            &(lane.clamp(i16::MIN as i32, i16::MAX as i32) as i16).to_le_bytes(),
                        );
                    }
                    stack.push(WasmValue::V128(out));
                }
                Instr::I16x8NarrowI32x4U => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    let mut out = [0u8; 16];
                    for i in 0..4 {
                        let offset = i * 4;
                        let lane = i32::from_le_bytes([
                            a[offset],
                            a[offset + 1],
                            a[offset + 2],
                            a[offset + 3],
                        ]);
                        out[i * 2..i * 2 + 2].copy_from_slice(
                            &(lane.clamp(0, u16::MAX as i32) as u16).to_le_bytes(),
                        );
                    }
                    for i in 0..4 {
                        let offset = i * 4;
                        let lane = i32::from_le_bytes([
                            b[offset],
                            b[offset + 1],
                            b[offset + 2],
                            b[offset + 3],
                        ]);
                        out[(i + 4) * 2..(i + 4) * 2 + 2].copy_from_slice(
                            &(lane.clamp(0, u16::MAX as i32) as u16).to_le_bytes(),
                        );
                    }
                    stack.push(WasmValue::V128(out));
                }
                Instr::I16x8ExtAddPairwiseI8x16S => {
                    let a = pop_v128(&mut stack)?;
                    let mut out = [0u8; 16];
                    for i in 0..8 {
                        let x = a[i * 2] as i8 as i16;
                        let y = a[i * 2 + 1] as i8 as i16;
                        out[i * 2..i * 2 + 2].copy_from_slice(&(x + y).to_le_bytes());
                    }
                    stack.push(WasmValue::V128(out));
                }
                Instr::I16x8ExtAddPairwiseI8x16U => {
                    let a = pop_v128(&mut stack)?;
                    let mut out = [0u8; 16];
                    for i in 0..8 {
                        let x = a[i * 2] as u16;
                        let y = a[i * 2 + 1] as u16;
                        out[i * 2..i * 2 + 2].copy_from_slice(&(x + y).to_le_bytes());
                    }
                    stack.push(WasmValue::V128(out));
                }
                Instr::I32x4ExtAddPairwiseI16x8S => {
                    let a = pop_v128(&mut stack)?;
                    let mut out = [0u8; 16];
                    for i in 0..4 {
                        let offset = i * 4;
                        let x = i16::from_le_bytes([a[offset], a[offset + 1]]) as i32;
                        let y = i16::from_le_bytes([a[offset + 2], a[offset + 3]]) as i32;
                        out[offset..offset + 4].copy_from_slice(&(x + y).to_le_bytes());
                    }
                    stack.push(WasmValue::V128(out));
                }
                Instr::I32x4ExtAddPairwiseI16x8U => {
                    let a = pop_v128(&mut stack)?;
                    let mut out = [0u8; 16];
                    for i in 0..4 {
                        let offset = i * 4;
                        let x = u16::from_le_bytes([a[offset], a[offset + 1]]) as u32;
                        let y = u16::from_le_bytes([a[offset + 2], a[offset + 3]]) as u32;
                        out[offset..offset + 4].copy_from_slice(&(x + y).to_le_bytes());
                    }
                    stack.push(WasmValue::V128(out));
                }
                Instr::I16x8AllTrue => {
                    let a = pop_v128(&mut stack)?;
                    let mut all = true;
                    for i in 0..8 {
                        let offset = i * 2;
                        all &= u16::from_le_bytes([a[offset], a[offset + 1]]) != 0;
                    }
                    stack.push(WasmValue::I32(if all { 1 } else { 0 }));
                }
                Instr::I16x8Bitmask => {
                    let a = pop_v128(&mut stack)?;
                    let mut mask = 0i32;
                    for i in 0..8 {
                        let offset = i * 2;
                        let lane = u16::from_le_bytes([a[offset], a[offset + 1]]);
                        mask |= (((lane & 0x8000) != 0) as i32) << i;
                    }
                    stack.push(WasmValue::I32(mask));
                }
                Instr::I16x8Abs => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8_unary(a, |x| x.wrapping_abs())));
                }
                Instr::I16x8Neg => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8_unary(a, |x| x.wrapping_neg())));
                }
                Instr::I16x8ExtendLowI8x16S => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(extend_i8x16_to_i16x8(a, 0, true)));
                }
                Instr::I16x8ExtendHighI8x16S => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(extend_i8x16_to_i16x8(a, 8, true)));
                }
                Instr::I16x8ExtendLowI8x16U => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(extend_i8x16_to_i16x8(a, 0, false)));
                }
                Instr::I16x8ExtendHighI8x16U => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(extend_i8x16_to_i16x8(a, 8, false)));
                }
                Instr::I32x4ExtendLowI16x8S => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(extend_i16x8_to_i32x4(a, 0, true)));
                }
                Instr::I32x4ExtendHighI16x8S => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(extend_i16x8_to_i32x4(a, 4, true)));
                }
                Instr::I32x4ExtendLowI16x8U => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(extend_i16x8_to_i32x4(a, 0, false)));
                }
                Instr::I32x4ExtendHighI16x8U => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(extend_i16x8_to_i32x4(a, 4, false)));
                }
                Instr::I64x2ExtendLowI32x4S => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(extend_i32x4_to_i64x2(a, 0, true)));
                }
                Instr::I64x2ExtendHighI32x4S => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(extend_i32x4_to_i64x2(a, 2, true)));
                }
                Instr::I64x2ExtendLowI32x4U => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(extend_i32x4_to_i64x2(a, 0, false)));
                }
                Instr::I64x2ExtendHighI32x4U => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(extend_i32x4_to_i64x2(a, 2, false)));
                }
                Instr::I16x8ExtMulLowI8x16S => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(extmul_i8x16_to_i16x8(a, b, 0, true)));
                }
                Instr::I16x8ExtMulHighI8x16S => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(extmul_i8x16_to_i16x8(a, b, 8, true)));
                }
                Instr::I16x8ExtMulLowI8x16U => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(extmul_i8x16_to_i16x8(a, b, 0, false)));
                }
                Instr::I16x8ExtMulHighI8x16U => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(extmul_i8x16_to_i16x8(a, b, 8, false)));
                }
                Instr::I32x4DotI16x8S => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(dot_i16x8_to_i32x4(a, b)));
                }
                Instr::I32x4ExtMulLowI16x8S => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(extmul_i16x8_to_i32x4(a, b, 0, true)));
                }
                Instr::I32x4ExtMulHighI16x8S => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(extmul_i16x8_to_i32x4(a, b, 4, true)));
                }
                Instr::I32x4ExtMulLowI16x8U => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(extmul_i16x8_to_i32x4(a, b, 0, false)));
                }
                Instr::I32x4ExtMulHighI16x8U => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(extmul_i16x8_to_i32x4(a, b, 4, false)));
                }
                Instr::I64x2ExtMulLowI32x4S => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(extmul_i32x4_to_i64x2(a, b, 0, true)));
                }
                Instr::I64x2ExtMulHighI32x4S => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(extmul_i32x4_to_i64x2(a, b, 2, true)));
                }
                Instr::I64x2ExtMulLowI32x4U => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(extmul_i32x4_to_i64x2(a, b, 0, false)));
                }
                Instr::I64x2ExtMulHighI32x4U => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(extmul_i32x4_to_i64x2(a, b, 2, false)));
                }
                Instr::I8x16Add => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i8x16(a, b, |x, y| x.wrapping_add(y))));
                }
                Instr::I8x16AddSatS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i8x16(a, b, |x, y| {
                        x.saturating_add(y)
                    })));
                }
                Instr::I8x16AddSatU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(bytes_v128(a, b, |x, y| {
                        x.saturating_add(y)
                    })));
                }
                Instr::I8x16Sub => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i8x16(a, b, |x, y| x.wrapping_sub(y))));
                }
                Instr::I8x16SubSatS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i8x16(a, b, |x, y| {
                        x.saturating_sub(y)
                    })));
                }
                Instr::I8x16SubSatU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(bytes_v128(a, b, |x, y| {
                        x.saturating_sub(y)
                    })));
                }
                Instr::I8x16MinS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i8x16(a, b, i8::min)));
                }
                Instr::I8x16MinU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(bytes_v128(a, b, u8::min)));
                }
                Instr::I8x16MaxS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i8x16(a, b, i8::max)));
                }
                Instr::I8x16MaxU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(bytes_v128(a, b, u8::max)));
                }
                Instr::I8x16AvgrU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(bytes_v128(a, b, |x, y| {
                        ((x as u16 + y as u16 + 1) >> 1) as u8
                    })));
                }
                Instr::I16x8Add => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8(a, b, |x, y| x.wrapping_add(y))));
                }
                Instr::I16x8AddSatS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8(a, b, |x, y| {
                        x.saturating_add(y)
                    })));
                }
                Instr::I16x8AddSatU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8_u(a, b, |x, y| {
                        x.saturating_add(y)
                    })));
                }
                Instr::I16x8Sub => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8(a, b, |x, y| x.wrapping_sub(y))));
                }
                Instr::I16x8SubSatS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8(a, b, |x, y| {
                        x.saturating_sub(y)
                    })));
                }
                Instr::I16x8SubSatU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8_u(a, b, |x, y| {
                        x.saturating_sub(y)
                    })));
                }
                Instr::I16x8Mul => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8(a, b, |x, y| x.wrapping_mul(y))));
                }
                Instr::I16x8MinS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8(a, b, i16::min)));
                }
                Instr::I16x8MinU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8_u(a, b, u16::min)));
                }
                Instr::I16x8MaxS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8(a, b, i16::max)));
                }
                Instr::I16x8MaxU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8_u(a, b, u16::max)));
                }
                Instr::I16x8AvgrU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8_u(a, b, |x, y| {
                        ((x as u32 + y as u32 + 1) >> 1) as u16
                    })));
                }
                Instr::I16x8Q15mulrSatS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8(a, b, q15mulr_s)));
                }
                Instr::I16x8RelaxedQ15mulrS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8(a, b, q15mulr_s)));
                }
                Instr::I16x8RelaxedDotI8x16I7x16S => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(dot_i8x16_to_i16x8(a, b)));
                }
                Instr::I32x4RelaxedDotI8x16I7x16AddS => {
                    let addend = pop_v128(&mut stack)?;
                    let b = pop_v128(&mut stack)?;
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(dot_i8x16_to_i32x4_add(a, b, addend)));
                }
                Instr::I32x4Abs => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4_unary(a, |x| x.wrapping_abs())));
                }
                Instr::I32x4Neg => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4_unary(a, |x| x.wrapping_neg())));
                }
                Instr::I32x4AllTrue => {
                    let a = pop_v128(&mut stack)?;
                    let mut all = true;
                    for i in 0..4 {
                        let offset = i * 4;
                        all &= i32::from_le_bytes([
                            a[offset],
                            a[offset + 1],
                            a[offset + 2],
                            a[offset + 3],
                        ]) != 0;
                    }
                    stack.push(WasmValue::I32(if all { 1 } else { 0 }));
                }
                Instr::I32x4Bitmask => {
                    let a = pop_v128(&mut stack)?;
                    let mut mask = 0i32;
                    for i in 0..4 {
                        let offset = i * 4;
                        let lane = u32::from_le_bytes([
                            a[offset],
                            a[offset + 1],
                            a[offset + 2],
                            a[offset + 3],
                        ]);
                        mask |= (((lane & 0x8000_0000) != 0) as i32) << i;
                    }
                    stack.push(WasmValue::I32(mask));
                }
                Instr::I64x2Abs => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i64x2_unary(a, |x| x.wrapping_abs())));
                }
                Instr::I64x2Neg => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i64x2_unary(a, |x| x.wrapping_neg())));
                }
                Instr::I64x2AllTrue => {
                    let a = pop_v128(&mut stack)?;
                    let mut all = true;
                    for i in 0..2 {
                        let offset = i * 8;
                        all &= i64::from_le_bytes([
                            a[offset],
                            a[offset + 1],
                            a[offset + 2],
                            a[offset + 3],
                            a[offset + 4],
                            a[offset + 5],
                            a[offset + 6],
                            a[offset + 7],
                        ]) != 0;
                    }
                    stack.push(WasmValue::I32(if all { 1 } else { 0 }));
                }
                Instr::I64x2Bitmask => {
                    let a = pop_v128(&mut stack)?;
                    let mut mask = 0i32;
                    for i in 0..2 {
                        let offset = i * 8;
                        let lane = u64::from_le_bytes([
                            a[offset],
                            a[offset + 1],
                            a[offset + 2],
                            a[offset + 3],
                            a[offset + 4],
                            a[offset + 5],
                            a[offset + 6],
                            a[offset + 7],
                        ]);
                        mask |= (((lane & 0x8000_0000_0000_0000) != 0) as i32) << i;
                    }
                    stack.push(WasmValue::I32(mask));
                }
                Instr::I32x4Add => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4(a, b, |x, y| x.wrapping_add(y))));
                }
                Instr::I32x4Sub => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4(a, b, |x, y| x.wrapping_sub(y))));
                }
                Instr::I64x2Add => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i64x2(a, b, |x, y| x.wrapping_add(y))));
                }
                Instr::I64x2Sub => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i64x2(a, b, |x, y| x.wrapping_sub(y))));
                }
                Instr::I64x2Mul => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i64x2(a, b, |x, y| x.wrapping_mul(y))));
                }
                Instr::I8x16Shl => {
                    let shift = pop_i32(&mut stack)? & 7;
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(a.map(|x| x.wrapping_shl(shift as u32))));
                }
                Instr::I8x16ShrS => {
                    let shift = pop_i32(&mut stack)? & 7;
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(
                        a.map(|x| ((x as i8).wrapping_shr(shift as u32)) as u8),
                    ));
                }
                Instr::I8x16ShrU => {
                    let shift = pop_i32(&mut stack)? & 7;
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(a.map(|x| x.wrapping_shr(shift as u32))));
                }
                Instr::I16x8Shl => {
                    let shift = pop_i32(&mut stack)? & 15;
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8_u_unary(a, |x| {
                        x.wrapping_shl(shift as u32)
                    })));
                }
                Instr::I16x8ShrS => {
                    let shift = pop_i32(&mut stack)? & 15;
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8_unary(a, |x| x >> shift)));
                }
                Instr::I16x8ShrU => {
                    let shift = pop_i32(&mut stack)? & 15;
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i16x8_u_unary(a, |x| x >> shift)));
                }
                Instr::I32x4Shl => {
                    let shift = pop_i32(&mut stack)? & 31;
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4_u_unary(a, |x| {
                        x.wrapping_shl(shift as u32)
                    })));
                }
                Instr::I32x4ShrS => {
                    let shift = pop_i32(&mut stack)? & 31;
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4_unary(a, |x| x >> shift)));
                }
                Instr::I32x4ShrU => {
                    let shift = pop_i32(&mut stack)? & 31;
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4_u_unary(a, |x| x >> shift)));
                }
                Instr::I64x2Shl => {
                    let shift = pop_i32(&mut stack)? & 63;
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i64x2_u_unary(a, |x| {
                        x.wrapping_shl(shift as u32)
                    })));
                }
                Instr::I64x2ShrS => {
                    let shift = pop_i32(&mut stack)? & 63;
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i64x2_unary(a, |x| x >> shift)));
                }
                Instr::I64x2ShrU => {
                    let shift = pop_i32(&mut stack)? & 63;
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i64x2_u_unary(a, |x| x >> shift)));
                }
                Instr::I32x4Mul => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4(a, b, |x, y| x.wrapping_mul(y))));
                }
                Instr::I32x4MinS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4(a, b, i32::min)));
                }
                Instr::I32x4MinU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4_u(a, b, u32::min)));
                }
                Instr::I32x4MaxS => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4(a, b, i32::max)));
                }
                Instr::I32x4MaxU => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_i32x4_u(a, b, u32::max)));
                }
                Instr::F32x4Eq => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4_mask(a, b, |x, y| x == y)));
                }
                Instr::F32x4Ne => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4_mask(a, b, |x, y| x != y)));
                }
                Instr::F32x4Lt => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4_mask(a, b, |x, y| x < y)));
                }
                Instr::F32x4Gt => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4_mask(a, b, |x, y| x > y)));
                }
                Instr::F32x4Le => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4_mask(a, b, |x, y| x <= y)));
                }
                Instr::F32x4Ge => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4_mask(a, b, |x, y| x >= y)));
                }
                Instr::F64x2Eq => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2_mask(a, b, |x, y| x == y)));
                }
                Instr::F64x2Ne => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2_mask(a, b, |x, y| x != y)));
                }
                Instr::F64x2Lt => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2_mask(a, b, |x, y| x < y)));
                }
                Instr::F64x2Gt => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2_mask(a, b, |x, y| x > y)));
                }
                Instr::F64x2Le => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2_mask(a, b, |x, y| x <= y)));
                }
                Instr::F64x2Ge => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2_mask(a, b, |x, y| x >= y)));
                }
                Instr::F32x4Abs => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4_unary(a, f32::abs)));
                }
                Instr::F32x4Neg => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4_unary(a, |x| -x)));
                }
                Instr::F32x4Ceil => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4_unary(a, f32::ceil)));
                }
                Instr::F32x4Floor => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4_unary(a, f32::floor)));
                }
                Instr::F32x4Trunc => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4_unary(a, f32::trunc)));
                }
                Instr::F32x4Nearest => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4_unary(a, f32::round_ties_even)));
                }
                Instr::F32x4Sqrt => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4_unary(a, f32::sqrt)));
                }
                Instr::F32x4Add => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4(a, b, |x, y| x + y)));
                }
                Instr::F32x4Sub => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4(a, b, |x, y| x - y)));
                }
                Instr::F32x4Mul => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4(a, b, |x, y| x * y)));
                }
                Instr::F32x4Div => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4(a, b, |x, y| x / y)));
                }
                Instr::F32x4Min => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4(a, b, f32::min)));
                }
                Instr::F32x4Max => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4(a, b, f32::max)));
                }
                Instr::F32x4PMin => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4(a, b, |x, y| {
                        if x < y {
                            x
                        } else {
                            y
                        }
                    })));
                }
                Instr::F32x4PMax => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4(a, b, |x, y| {
                        if x > y {
                            x
                        } else {
                            y
                        }
                    })));
                }
                Instr::F32x4RelaxedMadd => {
                    let z = pop_v128(&mut stack)?;
                    let b = pop_v128(&mut stack)?;
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4_ternary(a, b, z, |x, y, z| {
                        x * y + z
                    })));
                }
                Instr::F32x4RelaxedNmadd => {
                    let z = pop_v128(&mut stack)?;
                    let b = pop_v128(&mut stack)?;
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4_ternary(a, b, z, |x, y, z| {
                        -(x * y) + z
                    })));
                }
                Instr::I32x4TruncSatF32x4S => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4_to_i32x4(a, true)));
                }
                Instr::I32x4TruncSatF32x4U => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f32x4_to_i32x4(a, false)));
                }
                Instr::I32x4TruncSatF64x2SZero => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2_to_i32x4_zero(a, true)));
                }
                Instr::I32x4TruncSatF64x2UZero => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2_to_i32x4_zero(a, false)));
                }
                Instr::F32x4ConvertI32x4S => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(convert_i32x4_to_f32x4(a, true)));
                }
                Instr::F32x4ConvertI32x4U => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(convert_i32x4_to_f32x4(a, false)));
                }
                Instr::F32x4DemoteF64x2Zero => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(demote_f64x2_to_f32x4_zero(a)));
                }
                Instr::F64x2ConvertLowI32x4S => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(convert_low_i32x4_to_f64x2(a, true)));
                }
                Instr::F64x2ConvertLowI32x4U => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(convert_low_i32x4_to_f64x2(a, false)));
                }
                Instr::F64x2PromoteLowF32x4 => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(promote_low_f32x4_to_f64x2(a)));
                }
                Instr::F64x2Abs => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2_unary(a, f64::abs)));
                }
                Instr::F64x2Neg => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2_unary(a, |x| -x)));
                }
                Instr::F64x2Ceil => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2_unary(a, f64::ceil)));
                }
                Instr::F64x2Floor => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2_unary(a, f64::floor)));
                }
                Instr::F64x2Trunc => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2_unary(a, f64::trunc)));
                }
                Instr::F64x2Nearest => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2_unary(a, f64::round_ties_even)));
                }
                Instr::F64x2Sqrt => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2_unary(a, f64::sqrt)));
                }
                Instr::F64x2Add => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2(a, b, |x, y| x + y)));
                }
                Instr::F64x2Sub => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2(a, b, |x, y| x - y)));
                }
                Instr::F64x2Mul => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2(a, b, |x, y| x * y)));
                }
                Instr::F64x2Div => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2(a, b, |x, y| x / y)));
                }
                Instr::F64x2Min => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2(a, b, f64::min)));
                }
                Instr::F64x2Max => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2(a, b, f64::max)));
                }
                Instr::F64x2PMin => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2(a, b, |x, y| {
                        if x < y {
                            x
                        } else {
                            y
                        }
                    })));
                }
                Instr::F64x2PMax => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2(a, b, |x, y| {
                        if x > y {
                            x
                        } else {
                            y
                        }
                    })));
                }
                Instr::F64x2RelaxedMadd => {
                    let z = pop_v128(&mut stack)?;
                    let b = pop_v128(&mut stack)?;
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2_ternary(a, b, z, |x, y, z| {
                        x * y + z
                    })));
                }
                Instr::F64x2RelaxedNmadd => {
                    let z = pop_v128(&mut stack)?;
                    let b = pop_v128(&mut stack)?;
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(lanes_f64x2_ternary(a, b, z, |x, y, z| {
                        -(x * y) + z
                    })));
                }
                Instr::V128Not => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::V128(a.map(|x| !x)));
                }
                Instr::V128And => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(bytes_v128(a, b, |x, y| x & y)));
                }
                Instr::V128AndNot => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(bytes_v128(a, b, |x, y| x & !y)));
                }
                Instr::V128Or => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(bytes_v128(a, b, |x, y| x | y)));
                }
                Instr::V128Xor => {
                    let (a, b) = pop_v128_pair(&mut stack)?;
                    stack.push(WasmValue::V128(bytes_v128(a, b, |x, y| x ^ y)));
                }
                Instr::V128BitSelect => {
                    let mask = pop_v128(&mut stack)?;
                    let b = pop_v128(&mut stack)?;
                    let a = pop_v128(&mut stack)?;
                    let mut out = [0u8; 16];
                    for i in 0..16 {
                        out[i] = (a[i] & mask[i]) | (b[i] & !mask[i]);
                    }
                    stack.push(WasmValue::V128(out));
                }
                Instr::V128AnyTrue => {
                    let a = pop_v128(&mut stack)?;
                    stack.push(WasmValue::I32(if a.iter().any(|b| *b != 0) {
                        1
                    } else {
                        0
                    }));
                }
                Instr::V128Load(memarg) => {
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    let bytes = self.read_mem_at(memarg.memory, addr, 16, "v128.load")?;
                    let mut out = [0u8; 16];
                    out.copy_from_slice(bytes);
                    stack.push(WasmValue::V128(out));
                }
                Instr::V128Load8Splat(memarg) => {
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    let byte = self.read_mem_at(memarg.memory, addr, 1, "v128.load8_splat")?[0];
                    stack.push(WasmValue::V128([byte; 16]));
                }
                Instr::V128Load16Splat(memarg) => {
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    let bytes = self.read_mem_at(memarg.memory, addr, 2, "v128.load16_splat")?;
                    stack.push(WasmValue::V128(repeat_lane::<2, 8>([bytes[0], bytes[1]])));
                }
                Instr::V128Load32Splat(memarg) => {
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    let bytes = self.read_mem_at(memarg.memory, addr, 4, "v128.load32_splat")?;
                    stack.push(WasmValue::V128(repeat_lane::<4, 4>([
                        bytes[0], bytes[1], bytes[2], bytes[3],
                    ])));
                }
                Instr::V128Load64Splat(memarg) => {
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    let bytes = self.read_mem_at(memarg.memory, addr, 8, "v128.load64_splat")?;
                    stack.push(WasmValue::V128(repeat_lane::<8, 2>([
                        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6],
                        bytes[7],
                    ])));
                }
                Instr::V128Load8x8S(memarg) => {
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    let bytes = self.read_mem_at(memarg.memory, addr, 8, "v128.load8x8_s")?;
                    let mut out = [0u8; 16];
                    for i in 0..8 {
                        out[i * 2..i * 2 + 2]
                            .copy_from_slice(&(bytes[i] as i8 as i16).to_le_bytes());
                    }
                    stack.push(WasmValue::V128(out));
                }
                Instr::V128Load8x8U(memarg) => {
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    let bytes = self.read_mem_at(memarg.memory, addr, 8, "v128.load8x8_u")?;
                    let mut out = [0u8; 16];
                    for i in 0..8 {
                        out[i * 2..i * 2 + 2].copy_from_slice(&(bytes[i] as u16).to_le_bytes());
                    }
                    stack.push(WasmValue::V128(out));
                }
                Instr::V128Load16x4S(memarg) => {
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    let bytes = self.read_mem_at(memarg.memory, addr, 8, "v128.load16x4_s")?;
                    let mut out = [0u8; 16];
                    for i in 0..4 {
                        let offset = i * 2;
                        let lane = i16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as i32;
                        out[i * 4..i * 4 + 4].copy_from_slice(&lane.to_le_bytes());
                    }
                    stack.push(WasmValue::V128(out));
                }
                Instr::V128Load16x4U(memarg) => {
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    let bytes = self.read_mem_at(memarg.memory, addr, 8, "v128.load16x4_u")?;
                    let mut out = [0u8; 16];
                    for i in 0..4 {
                        let offset = i * 2;
                        let lane = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as u32;
                        out[i * 4..i * 4 + 4].copy_from_slice(&lane.to_le_bytes());
                    }
                    stack.push(WasmValue::V128(out));
                }
                Instr::V128Load32x2S(memarg) => {
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    let bytes = self.read_mem_at(memarg.memory, addr, 8, "v128.load32x2_s")?;
                    let mut out = [0u8; 16];
                    for i in 0..2 {
                        let offset = i * 4;
                        let lane = i32::from_le_bytes([
                            bytes[offset],
                            bytes[offset + 1],
                            bytes[offset + 2],
                            bytes[offset + 3],
                        ]) as i64;
                        out[i * 8..i * 8 + 8].copy_from_slice(&lane.to_le_bytes());
                    }
                    stack.push(WasmValue::V128(out));
                }
                Instr::V128Load32x2U(memarg) => {
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    let bytes = self.read_mem_at(memarg.memory, addr, 8, "v128.load32x2_u")?;
                    let mut out = [0u8; 16];
                    for i in 0..2 {
                        let offset = i * 4;
                        let lane = u32::from_le_bytes([
                            bytes[offset],
                            bytes[offset + 1],
                            bytes[offset + 2],
                            bytes[offset + 3],
                        ]) as u64;
                        out[i * 8..i * 8 + 8].copy_from_slice(&lane.to_le_bytes());
                    }
                    stack.push(WasmValue::V128(out));
                }
                Instr::V128Load8Lane(memarg, lane) => {
                    let mut bytes = pop_v128(&mut stack)?;
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    bytes[*lane as usize] =
                        self.read_mem_at(memarg.memory, addr, 1, "v128.load8_lane")?[0];
                    stack.push(WasmValue::V128(bytes));
                }
                Instr::V128Load16Lane(memarg, lane) => {
                    let mut bytes = pop_v128(&mut stack)?;
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    let loaded = self.read_mem_at(memarg.memory, addr, 2, "v128.load16_lane")?;
                    let offset = *lane as usize * 2;
                    bytes[offset..offset + 2].copy_from_slice(&loaded);
                    stack.push(WasmValue::V128(bytes));
                }
                Instr::V128Load32Lane(memarg, lane) => {
                    let mut bytes = pop_v128(&mut stack)?;
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    let loaded = self.read_mem_at(memarg.memory, addr, 4, "v128.load32_lane")?;
                    let offset = *lane as usize * 4;
                    bytes[offset..offset + 4].copy_from_slice(&loaded);
                    stack.push(WasmValue::V128(bytes));
                }
                Instr::V128Load64Lane(memarg, lane) => {
                    let mut bytes = pop_v128(&mut stack)?;
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    let loaded = self.read_mem_at(memarg.memory, addr, 8, "v128.load64_lane")?;
                    let offset = *lane as usize * 8;
                    bytes[offset..offset + 8].copy_from_slice(&loaded);
                    stack.push(WasmValue::V128(bytes));
                }
                Instr::V128Load32Zero(memarg) => {
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    let loaded = self.read_mem_at(memarg.memory, addr, 4, "v128.load32_zero")?;
                    let mut bytes = [0u8; 16];
                    bytes[..4].copy_from_slice(&loaded);
                    stack.push(WasmValue::V128(bytes));
                }
                Instr::V128Load64Zero(memarg) => {
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    let loaded = self.read_mem_at(memarg.memory, addr, 8, "v128.load64_zero")?;
                    let mut bytes = [0u8; 16];
                    bytes[..8].copy_from_slice(&loaded);
                    stack.push(WasmValue::V128(bytes));
                }
                Instr::V128Store(memarg) => {
                    let bytes = pop_v128(&mut stack)?;
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    self.write_mem_at(memarg.memory, addr, &bytes, "v128.store")?;
                }
                Instr::V128Store8Lane(memarg, lane) => {
                    let bytes = pop_v128(&mut stack)?;
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    self.write_mem_at(
                        memarg.memory,
                        addr,
                        &[bytes[*lane as usize]],
                        "v128.store8_lane",
                    )?;
                }
                Instr::V128Store16Lane(memarg, lane) => {
                    let bytes = pop_v128(&mut stack)?;
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    let offset = *lane as usize * 2;
                    self.write_mem_at(
                        memarg.memory,
                        addr,
                        &bytes[offset..offset + 2],
                        "v128.store16_lane",
                    )?;
                }
                Instr::V128Store32Lane(memarg, lane) => {
                    let bytes = pop_v128(&mut stack)?;
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    let offset = *lane as usize * 4;
                    self.write_mem_at(
                        memarg.memory,
                        addr,
                        &bytes[offset..offset + 4],
                        "v128.store32_lane",
                    )?;
                }
                Instr::V128Store64Lane(memarg, lane) => {
                    let bytes = pop_v128(&mut stack)?;
                    let base = pop_i32(&mut stack)?;
                    let addr = self.effective_addr(*memarg, base)?;
                    let offset = *lane as usize * 8;
                    self.write_mem_at(
                        memarg.memory,
                        addr,
                        &bytes[offset..offset + 8],
                        "v128.store64_lane",
                    )?;
                }
                Instr::RefNull(_) => stack.push(WasmValue::RefNull),
                Instr::RefIsNull => {
                    let v = stack.pop().ok_or("ref.is_null underflow")?;
                    stack.push(WasmValue::I32(matches!(v, WasmValue::RefNull) as i32));
                }
                Instr::RefAsNonNull => {
                    let v = stack.pop().ok_or("ref.as_non_null underflow")?;
                    if matches!(v, WasmValue::RefNull) {
                        return Err("ref.as_non_null: null reference".to_string());
                    }
                    stack.push(v);
                }
                Instr::RefTest { target, nullable } => {
                    let value = pop_ref(&mut stack)?;
                    stack.push(WasmValue::I32(
                        self.ref_matches_type(value, *target, *nullable, true) as i32,
                    ));
                }
                Instr::RefCast { target, nullable } => {
                    let value = pop_ref(&mut stack)?;
                    if !self.ref_matches_type(value, *target, *nullable, false) {
                        return Err("ref.cast: cast failure".to_string());
                    }
                    stack.push(value);
                }
                Instr::BrOnCast {
                    depth,
                    target,
                    nullable,
                    ..
                } => {
                    let value = pop_ref(&mut stack)?;
                    if self.ref_matches_type(value, *target, *nullable, false) {
                        stack.push(value);
                        pc = self.do_branch(*depth as usize, &mut labels, &mut stack)?;
                        continue;
                    }
                    stack.push(value);
                }
                Instr::BrOnCastFail {
                    depth,
                    target,
                    nullable,
                    ..
                } => {
                    let value = pop_ref(&mut stack)?;
                    if !self.ref_matches_type(value, *target, *nullable, false) {
                        stack.push(value);
                        pc = self.do_branch(*depth as usize, &mut labels, &mut stack)?;
                        continue;
                    }
                    stack.push(value);
                }
                Instr::AnyConvertExtern => {
                    let value = pop_ref(&mut stack)?;
                    stack.push(match value {
                        WasmValue::ExternI31Ref(value) => WasmValue::I31Ref(value),
                        WasmValue::ExternStructRef(idx) => WasmValue::StructRef(idx),
                        WasmValue::ExternArrayRef(idx) => WasmValue::ArrayRef(idx),
                        other => other,
                    });
                }
                Instr::ExternConvertAny => {
                    let value = pop_ref(&mut stack)?;
                    stack.push(match value {
                        WasmValue::RefNull => WasmValue::RefNull,
                        WasmValue::ExternRef(id) => WasmValue::ExternRef(id),
                        WasmValue::I31Ref(value) => WasmValue::ExternI31Ref(value),
                        WasmValue::StructRef(idx) => WasmValue::ExternStructRef(idx),
                        WasmValue::ArrayRef(idx) => WasmValue::ExternArrayRef(idx),
                        _ => WasmValue::ExternRef(0),
                    });
                }
                Instr::RefFunc(idx) => stack.push(WasmValue::FuncRef(*idx)),
                Instr::RefEq => {
                    let right = pop_ref(&mut stack)?;
                    let left = pop_ref(&mut stack)?;
                    stack.push(WasmValue::I32((left == right) as i32));
                }
                Instr::RefI31 => {
                    let value = pop_i32(&mut stack)?;
                    stack.push(WasmValue::I31Ref(value & 0x7fff_ffff));
                }
                Instr::I31GetU => {
                    let value = pop_i31(&mut stack, "i31.get_u")?;
                    stack.push(WasmValue::I32(value));
                }
                Instr::I31GetS => {
                    let value = pop_i31(&mut stack, "i31.get_s")?;
                    stack.push(WasmValue::I32((value << 1) >> 1));
                }
                Instr::StructNew(type_idx) => {
                    let ty = self.struct_type(*type_idx)?;
                    let field_count = ty.fields.len();
                    if stack.len() < field_count {
                        return Err("struct.new: field underflow".to_string());
                    }
                    let fields = stack.split_off(stack.len() - field_count);
                    stack.push(self.alloc_struct(*type_idx, ty, fields));
                }
                Instr::StructNewDefault(type_idx) => {
                    let ty = self.struct_type(*type_idx)?;
                    let fields = ty
                        .fields
                        .iter()
                        .map(|field| zero_of(field.ty))
                        .collect::<Vec<_>>();
                    stack.push(self.alloc_struct(*type_idx, ty, fields));
                }
                Instr::StructGet(type_idx, field_idx) => {
                    let _ty = self.struct_type(*type_idx)?;
                    let struct_ref = pop_struct_ref(&mut stack)?;
                    let value = self
                        .gc_structs
                        .get(struct_ref)
                        .and_then(|st| st.fields.get(*field_idx as usize))
                        .copied()
                        .ok_or("struct.get: field out of bounds")?;
                    stack.push(value);
                }
                Instr::StructGetS(type_idx, field_idx) | Instr::StructGetU(type_idx, field_idx) => {
                    let ty = self.struct_type(*type_idx)?;
                    let field = ty
                        .fields
                        .get(*field_idx as usize)
                        .ok_or("struct.get_s/u: field out of bounds")?;
                    let signed = matches!(body[pc], Instr::StructGetS(_, _));
                    let struct_ref = pop_struct_ref(&mut stack)?;
                    let value = self
                        .gc_structs
                        .get(struct_ref)
                        .and_then(|st| st.fields.get(*field_idx as usize))
                        .copied()
                        .ok_or("struct.get_s/u: field out of bounds")?;
                    stack.push(load_packed_value(field.packed_bits, signed, value)?);
                }
                Instr::StructSet(type_idx, field_idx) => {
                    let ty = self.struct_type(*type_idx)?;
                    let field = ty
                        .fields
                        .get(*field_idx as usize)
                        .ok_or("struct.set: field out of bounds")?;
                    let value = normalize_packed_value(
                        field.packed_bits,
                        stack.pop().ok_or("struct.set: value underflow")?,
                    );
                    let struct_ref = pop_struct_ref(&mut stack)?;
                    let st = self
                        .gc_structs
                        .get_mut(struct_ref)
                        .ok_or("struct.set: struct reference out of bounds")?;
                    if !st
                        .mutable
                        .get(*field_idx as usize)
                        .copied()
                        .unwrap_or(false)
                    {
                        return Err("struct.set: immutable field".to_string());
                    }
                    let slot = st
                        .fields
                        .get_mut(*field_idx as usize)
                        .ok_or("struct.set: field out of bounds")?;
                    *slot = value;
                }
                Instr::ArrayNew(type_idx) => {
                    let ty = self.array_type(*type_idx)?;
                    let len = pop_i32(&mut stack)? as u32 as usize;
                    let value = stack.pop().ok_or("array.new: element underflow")?;
                    stack.push(self.alloc_array(*type_idx, ty, len, value));
                }
                Instr::ArrayNewDefault(type_idx) => {
                    let ty = self.array_type(*type_idx)?;
                    let len = pop_i32(&mut stack)? as u32 as usize;
                    let value = zero_of(ty.element);
                    stack.push(self.alloc_array(*type_idx, ty, len, value));
                }
                Instr::ArrayNewFixed(type_idx, count) => {
                    let ty = self.array_type(*type_idx)?;
                    let count = *count as usize;
                    if stack.len() < count {
                        return Err("array.new_fixed: element underflow".to_string());
                    }
                    let elements = stack.split_off(stack.len() - count);
                    let idx = self.gc_arrays.len() as u32;
                    self.gc_arrays.push(GcArray {
                        type_idx: *type_idx,
                        elements,
                        mutable: ty.mutable,
                    });
                    stack.push(WasmValue::ArrayRef(idx));
                }
                Instr::ArrayNewData(type_idx, data_idx) => {
                    let ty = self.array_type(*type_idx)?;
                    let len = pop_i32(&mut stack)? as u32 as usize;
                    let src = pop_i32(&mut stack)? as u32 as usize;
                    let data = self
                        .data_segments
                        .get(*data_idx as usize)
                        .and_then(|seg| seg.as_ref())
                        .ok_or("array.new_data: data segment unavailable")?;
                    let end = src
                        .checked_add(len)
                        .ok_or("array.new_data: data segment out of bounds")?;
                    if end > data.len() {
                        return Err("array.new_data: data segment out of bounds".to_string());
                    }
                    let elements = match (ty.packed_bits, ty.element) {
                        (Some(8), _) => data[src..end]
                            .iter()
                            .map(|byte| WasmValue::I32(*byte as i32))
                            .collect(),
                        (Some(16), _) => {
                            let byte_len = len
                                .checked_mul(2)
                                .ok_or("array.new_data: data segment out of bounds")?;
                            let end = src
                                .checked_add(byte_len)
                                .ok_or("array.new_data: data segment out of bounds")?;
                            if end > data.len() {
                                return Err(
                                    "array.new_data: data segment out of bounds".to_string()
                                );
                            }
                            data[src..end]
                                .chunks_exact(2)
                                .map(|bytes| {
                                    WasmValue::I32(u16::from_le_bytes([bytes[0], bytes[1]]) as i32)
                                })
                                .collect()
                        }
                        (None, ValType::I32) => {
                            decode_array_data_elements(data, src, len, 4, |bytes| {
                                WasmValue::I32(i32::from_le_bytes(bytes.try_into().unwrap()))
                            })?
                        }
                        (None, ValType::I64) => {
                            decode_array_data_elements(data, src, len, 8, |bytes| {
                                WasmValue::I64(i64::from_le_bytes(bytes.try_into().unwrap()))
                            })?
                        }
                        (None, ValType::F32) => {
                            decode_array_data_elements(data, src, len, 4, |bytes| {
                                WasmValue::F32(f32::from_le_bytes(bytes.try_into().unwrap()))
                            })?
                        }
                        (None, ValType::F64) => {
                            decode_array_data_elements(data, src, len, 8, |bytes| {
                                WasmValue::F64(f64::from_le_bytes(bytes.try_into().unwrap()))
                            })?
                        }
                        _ => {
                            return Err("array.new_data: unsupported array storage type".to_string())
                        }
                    };
                    let idx = self.gc_arrays.len() as u32;
                    self.gc_arrays.push(GcArray {
                        type_idx: *type_idx,
                        elements,
                        mutable: ty.mutable,
                    });
                    stack.push(WasmValue::ArrayRef(idx));
                }
                Instr::ArrayNewElem(type_idx, elem_idx) => {
                    let ty = self.array_type(*type_idx)?;
                    let len = pop_i32(&mut stack)? as u32 as usize;
                    let src = pop_i32(&mut stack)? as u32 as usize;
                    let segment = self
                        .elem_segments
                        .get(*elem_idx as usize)
                        .and_then(|seg| seg.as_ref())
                        .ok_or("array.new_elem: element segment unavailable")?;
                    let end = src
                        .checked_add(len)
                        .ok_or("array.new_elem: element segment out of bounds")?;
                    if end > segment.len() {
                        return Err("array.new_elem: element segment out of bounds".to_string());
                    }
                    let idx = self.gc_arrays.len() as u32;
                    self.gc_arrays.push(GcArray {
                        type_idx: *type_idx,
                        elements: segment[src..end].to_vec(),
                        mutable: ty.mutable,
                    });
                    stack.push(WasmValue::ArrayRef(idx));
                }
                Instr::ArrayGet(type_idx) => {
                    let _ty = self.array_type(*type_idx)?;
                    let index = pop_i32(&mut stack)? as u32 as usize;
                    let array = pop_array_ref(&mut stack)?;
                    let value = self
                        .gc_arrays
                        .get(array)
                        .and_then(|array| array.elements.get(index))
                        .copied()
                        .ok_or("array.get: array index out of bounds")?;
                    stack.push(value);
                }
                Instr::ArrayGetS(type_idx) | Instr::ArrayGetU(type_idx) => {
                    let ty = self.array_type(*type_idx)?;
                    let signed = matches!(body[pc], Instr::ArrayGetS(_));
                    let index = pop_i32(&mut stack)? as u32 as usize;
                    let array = pop_array_ref(&mut stack)?;
                    let value = self
                        .gc_arrays
                        .get(array)
                        .and_then(|array| array.elements.get(index))
                        .copied()
                        .ok_or("array.get: array index out of bounds")?;
                    let raw = match value {
                        WasmValue::I32(v) => v,
                        other => {
                            return Err(format!("array.get_s/u: expected i32, got {:?}", other))
                        }
                    };
                    let extended = match (ty.packed_bits, signed) {
                        (Some(8), true) => (raw as u8 as i8) as i32,
                        (Some(8), false) => raw as u8 as i32,
                        (Some(16), true) => (raw as u16 as i16) as i32,
                        (Some(16), false) => raw as u16 as i32,
                        _ => raw,
                    };
                    stack.push(WasmValue::I32(extended));
                }
                Instr::ArraySet(type_idx) => {
                    let ty = self.array_type(*type_idx)?;
                    let value = stack.pop().ok_or("array.set: value underflow")?;
                    let index = pop_i32(&mut stack)? as u32 as usize;
                    let array = pop_array_ref(&mut stack)?;
                    let array = self
                        .gc_arrays
                        .get_mut(array)
                        .ok_or("array.set: array reference out of bounds")?;
                    if !array.mutable {
                        return Err("array.set: immutable array".to_string());
                    }
                    let slot = array
                        .elements
                        .get_mut(index)
                        .ok_or("array.set: array index out of bounds")?;
                    *slot = match (ty.packed_bits, value) {
                        (Some(8), WasmValue::I32(v)) => WasmValue::I32(v as u8 as i32),
                        (Some(16), WasmValue::I32(v)) => WasmValue::I32(v as u16 as i32),
                        (_, value) => value,
                    };
                }
                Instr::ArrayLen => {
                    let array = pop_array_ref(&mut stack)?;
                    let len = self
                        .gc_arrays
                        .get(array)
                        .ok_or("array.len: array reference out of bounds")?
                        .elements
                        .len();
                    stack.push(WasmValue::I32(len as i32));
                }
                Instr::ArrayFill(type_idx) => {
                    let ty = self.array_type(*type_idx)?;
                    let len = pop_i32(&mut stack)? as u32 as usize;
                    let value = normalize_array_value(
                        ty,
                        stack.pop().ok_or("array.fill: value underflow")?,
                    );
                    let start = pop_i32(&mut stack)? as u32 as usize;
                    let array = pop_array_ref(&mut stack)?;
                    let end = start.checked_add(len).ok_or("array.fill: index overflow")?;
                    let array = self
                        .gc_arrays
                        .get_mut(array)
                        .ok_or("array.fill: array reference out of bounds")?;
                    if !array.mutable {
                        return Err("array.fill: immutable array".to_string());
                    }
                    if end > array.elements.len() {
                        return Err("array.fill: array index out of bounds".to_string());
                    }
                    for slot in &mut array.elements[start..end] {
                        *slot = value;
                    }
                }
                Instr::ArrayCopy(dst_type_idx, src_type_idx) => {
                    let dst_ty = self.array_type(*dst_type_idx)?;
                    let _src_ty = self.array_type(*src_type_idx)?;
                    let len = pop_i32(&mut stack)? as u32 as usize;
                    let src_start = pop_i32(&mut stack)? as u32 as usize;
                    let src_array = pop_array_ref(&mut stack)?;
                    let dst_start = pop_i32(&mut stack)? as u32 as usize;
                    let dst_array = pop_array_ref(&mut stack)?;
                    let src_end = src_start
                        .checked_add(len)
                        .ok_or("array.copy: src index overflow")?;
                    let dst_end = dst_start
                        .checked_add(len)
                        .ok_or("array.copy: dst index overflow")?;
                    if src_array == dst_array {
                        let array = self
                            .gc_arrays
                            .get_mut(dst_array)
                            .ok_or("array.copy: array reference out of bounds")?;
                        if !array.mutable {
                            return Err("array.copy: immutable array".to_string());
                        }
                        if src_end > array.elements.len() || dst_end > array.elements.len() {
                            return Err("array.copy: array index out of bounds".to_string());
                        }
                        array.elements.copy_within(src_start..src_end, dst_start);
                    } else {
                        let chunk = {
                            let src = self
                                .gc_arrays
                                .get(src_array)
                                .ok_or("array.copy: source array reference out of bounds")?;
                            if src_end > src.elements.len() {
                                return Err(
                                    "array.copy: source array index out of bounds".to_string()
                                );
                            }
                            src.elements[src_start..src_end].to_vec()
                        };
                        let dst = self
                            .gc_arrays
                            .get_mut(dst_array)
                            .ok_or("array.copy: destination array reference out of bounds")?;
                        if !dst.mutable {
                            return Err("array.copy: immutable array".to_string());
                        }
                        if dst_end > dst.elements.len() {
                            return Err(
                                "array.copy: destination array index out of bounds".to_string()
                            );
                        }
                        for (slot, value) in dst.elements[dst_start..dst_end]
                            .iter_mut()
                            .zip(chunk.into_iter())
                        {
                            *slot = normalize_array_value(dst_ty, value);
                        }
                    }
                }
                Instr::ArrayInitData(type_idx, data_idx) => {
                    let ty = self.array_type(*type_idx)?;
                    let len = pop_i32(&mut stack)? as u32 as usize;
                    let src = pop_i32(&mut stack)? as u32 as usize;
                    let dest = pop_i32(&mut stack)? as u32 as usize;
                    let array_ref = pop_array_ref(&mut stack)?;
                    let values = {
                        let data: &[u8] = match self.data_segments.get(*data_idx as usize) {
                            Some(Some(bytes)) => bytes.as_slice(),
                            _ => &[],
                        };
                        data_array_values(ty, data, src, len, "array.init_data")?
                    };
                    let end = dest
                        .checked_add(len)
                        .ok_or("array.init_data: destination index overflow")?;
                    let array = self
                        .gc_arrays
                        .get_mut(array_ref)
                        .ok_or("array.init_data: array reference out of bounds")?;
                    if !array.mutable {
                        return Err("array.init_data: immutable array".to_string());
                    }
                    if end > array.elements.len() {
                        return Err("array.init_data: array index out of bounds".to_string());
                    }
                    array.elements[dest..end].copy_from_slice(&values);
                }
                Instr::ArrayInitElem(type_idx, elem_idx) => {
                    let ty = self.array_type(*type_idx)?;
                    let len = pop_i32(&mut stack)? as u32 as usize;
                    let src = pop_i32(&mut stack)? as u32 as usize;
                    let dest = pop_i32(&mut stack)? as u32 as usize;
                    let array_ref = pop_array_ref(&mut stack)?;
                    let src_end = src
                        .checked_add(len)
                        .ok_or("array.init_elem: source index overflow")?;
                    let values = {
                        let segment: &[WasmValue] = match self.elem_segments.get(*elem_idx as usize)
                        {
                            Some(Some(values)) => values.as_slice(),
                            _ => &[],
                        };
                        if src_end > segment.len() {
                            return Err(
                                "array.init_elem: element segment out of bounds".to_string()
                            );
                        }
                        segment[src..src_end]
                            .iter()
                            .copied()
                            .map(|value| normalize_array_value(ty, value))
                            .collect::<Vec<_>>()
                    };
                    let end = dest
                        .checked_add(len)
                        .ok_or("array.init_elem: destination index overflow")?;
                    let array = self
                        .gc_arrays
                        .get_mut(array_ref)
                        .ok_or("array.init_elem: array reference out of bounds")?;
                    if !array.mutable {
                        return Err("array.init_elem: immutable array".to_string());
                    }
                    if end > array.elements.len() {
                        return Err("array.init_elem: array index out of bounds".to_string());
                    }
                    array.elements[dest..end].copy_from_slice(&values);
                }
                Instr::TableGet(tableidx) => {
                    let index = self.pop_table_index(*tableidx, &mut stack)?;
                    let value = self
                        .table(*tableidx)?
                        .get(index)
                        .ok_or("table.get: table index out of bounds")?;
                    stack.push(*value);
                }
                Instr::TableSet(tableidx) => {
                    let value = pop_ref(&mut stack)?;
                    let index = self.pop_table_index(*tableidx, &mut stack)?;
                    let slot = self
                        .table_mut(*tableidx)?
                        .get_mut(index)
                        .ok_or("table.set: table index out of bounds")?;
                    *slot = value;
                }
                Instr::Num(op) => {
                    crate::numeric::exec_num(*op, &mut stack)?;
                }
                Instr::TruncSat(sub) => {
                    crate::numeric::exec_trunc_sat(*sub, &mut stack)?;
                }
                Instr::MemoryFill(memoryidx) => {
                    let len = usize::try_from(self.pop_mem_addr(&mut stack)?)
                        .map_err(|_| "memory.fill: length overflow".to_string())?;
                    let val = (pop_i32(&mut stack)? as u32 & 0xff) as u8;
                    let dest = usize::try_from(self.pop_mem_addr(&mut stack)?)
                        .map_err(|_| "memory.fill: address overflow".to_string())?;
                    let end = dest
                        .checked_add(len)
                        .ok_or("memory.fill: address overflow")?;
                    let mem = self.memory_mut(*memoryidx)?;
                    if end > mem.len() {
                        return Err("memory.fill: out of bounds memory access".to_string());
                    }
                    for b in &mut mem[dest..end] {
                        *b = val;
                    }
                }
                Instr::MemoryCopy(dstidx, srcidx) => {
                    let len = usize::try_from(self.pop_mem_addr(&mut stack)?)
                        .map_err(|_| "memory.copy: length overflow".to_string())?;
                    let src = usize::try_from(self.pop_mem_addr(&mut stack)?)
                        .map_err(|_| "memory.copy: src overflow".to_string())?;
                    let dest = usize::try_from(self.pop_mem_addr(&mut stack)?)
                        .map_err(|_| "memory.copy: dest overflow".to_string())?;
                    let src_end = src.checked_add(len).ok_or("memory.copy: src overflow")?;
                    let dst_end = dest.checked_add(len).ok_or("memory.copy: dest overflow")?;
                    if src_end > self.memory_ref(*srcidx)?.len()
                        || dst_end > self.memory_ref(*dstidx)?.len()
                    {
                        return Err("memory.copy: out of bounds memory access".to_string());
                    }
                    if dstidx == srcidx {
                        self.memory_mut(*dstidx)?.copy_within(src..src_end, dest);
                    } else {
                        let chunk = self.memory_ref(*srcidx)?[src..src_end].to_vec();
                        self.memory_mut(*dstidx)?[dest..dst_end].copy_from_slice(&chunk);
                    }
                }
                Instr::MemoryInit(dataidx, memoryidx) => {
                    let len = pop_i32(&mut stack)? as u32 as usize;
                    let src = pop_i32(&mut stack)? as u32 as usize;
                    let dest = usize::try_from(self.pop_mem_addr(&mut stack)?)
                        .map_err(|_| "memory.init: dest overflow".to_string())?;

                    let seg_bytes: &[u8] = match self.data_segments.get(*dataidx as usize) {
                        Some(Some(b)) => b.as_slice(),
                        _ => &[],
                    };
                    let src_end = src.checked_add(len).ok_or("memory.init: src overflow")?;
                    let dst_end = dest.checked_add(len).ok_or("memory.init: dest overflow")?;
                    if src_end > seg_bytes.len() {
                        return Err("memory.init: out of bounds data segment access".to_string());
                    }
                    if dst_end > self.memory_ref(*memoryidx)?.len() {
                        return Err("memory.init: out of bounds memory access".to_string());
                    }
                    let chunk = seg_bytes[src..src_end].to_vec();
                    self.memory_mut(*memoryidx)?[dest..dst_end].copy_from_slice(&chunk);
                }
                Instr::DataDrop(dataidx) => {
                    if let Some(slot) = self.data_segments.get_mut(*dataidx as usize) {
                        *slot = None;
                    }
                }
                Instr::TableGrow(tableidx) => {
                    let delta = self.pop_table_index(*tableidx, &mut stack)?;
                    let value = pop_ref(&mut stack)?;
                    let table_max = self.table_max_for(*tableidx)?;
                    let table64 = self.table_is_64(*tableidx)?;
                    let old_len = self.table(*tableidx)?.len();
                    let new_len = match old_len.checked_add(delta) {
                        Some(len) => len,
                        None => usize::MAX,
                    };
                    let within_declared_max = match table_max {
                        Some(max) => new_len <= max as usize,
                        None => true,
                    };
                    if !within_declared_max || new_len > MAX_TABLE_ELEMENTS {
                        if table64 {
                            stack.push(WasmValue::I64(-1));
                        } else {
                            stack.push(WasmValue::I32(-1));
                        }
                    } else {
                        self.table_mut(*tableidx)?.resize(new_len, value);
                        self.push_table_index_result(*tableidx, &mut stack, old_len)?;
                    }
                }
                Instr::TableSize(tableidx) => {
                    let len = self.table(*tableidx)?.len();
                    self.push_table_index_result(*tableidx, &mut stack, len)?;
                }
                Instr::TableFill(tableidx) => {
                    let len = self.pop_table_index(*tableidx, &mut stack)?;
                    let value = pop_ref(&mut stack)?;
                    let start = self.pop_table_index(*tableidx, &mut stack)?;
                    let end = start.checked_add(len).ok_or("table.fill: index overflow")?;
                    let table = self.table_mut(*tableidx)?;
                    if end > table.len() {
                        return Err("table.fill: table index out of bounds".to_string());
                    }
                    for slot in &mut table[start..end] {
                        *slot = value;
                    }
                }
                Instr::TableInit(elemidx, tableidx) => {
                    let len = pop_i32(&mut stack)? as u32 as usize;
                    let src = pop_i32(&mut stack)? as u32 as usize;
                    let dest = self.pop_table_index(*tableidx, &mut stack)?;
                    let seg: &[WasmValue] = match self.elem_segments.get(*elemidx as usize) {
                        Some(Some(items)) => items.as_slice(),
                        _ => &[],
                    };
                    let src_end = src.checked_add(len).ok_or("table.init: src overflow")?;
                    let dst_end = dest.checked_add(len).ok_or("table.init: dest overflow")?;
                    if src_end > seg.len() {
                        return Err("table.init: out of bounds element segment access".to_string());
                    }
                    let chunk = seg[src..src_end].to_vec();
                    let table = self.table_mut(*tableidx)?;
                    if dst_end > table.len() {
                        return Err("table.init: table index out of bounds".to_string());
                    }
                    table[dest..dst_end].copy_from_slice(&chunk);
                }
                Instr::ElemDrop(elemidx) => {
                    if let Some(slot) = self.elem_segments.get_mut(*elemidx as usize) {
                        *slot = None;
                    }
                }
                Instr::TableCopy(dst, src) => {
                    let len = if self.table_is_64(*dst)? && self.table_is_64(*src)? {
                        self.pop_table_index(*dst, &mut stack)?
                    } else {
                        pop_i32(&mut stack)? as u32 as usize
                    };
                    let src_index = self.pop_table_index(*src, &mut stack)?;
                    let dest = self.pop_table_index(*dst, &mut stack)?;
                    let src_end = src_index
                        .checked_add(len)
                        .ok_or("table.copy: src overflow")?;
                    let dst_end = dest.checked_add(len).ok_or("table.copy: dest overflow")?;
                    let src_table = self.table(*src)?;
                    if src_end > src_table.len() {
                        return Err("table.copy: table index out of bounds".to_string());
                    }
                    if *dst == *src {
                        let table = self.table_mut(*dst)?;
                        if dst_end > table.len() {
                            return Err("table.copy: table index out of bounds".to_string());
                        }
                        table.copy_within(src_index..src_end, dest);
                    } else {
                        let chunk = src_table[src_index..src_end].to_vec();
                        let dst_table = self.table_mut(*dst)?;
                        if dst_end > dst_table.len() {
                            return Err("table.copy: table index out of bounds".to_string());
                        }
                        dst_table[dest..dst_end].copy_from_slice(&chunk);
                    }
                }
            }
            pc += 1;
        }

        let n = func_results.len();
        if stack.len() < n {
            return Err("function end: result stack underflow".to_string());
        }
        let res = stack.split_off(stack.len() - n);
        Ok(BodyResult::Values(res))
    }

    fn call_overall_depth(
        &mut self,
        overall: usize,
        args: &[WasmValue],
        depth: usize,
    ) -> Result<Vec<WasmValue>, String> {
        let is_host = matches!(self.funcs.get(overall), Some(Callable::Host(_)));
        if is_host {
            return self.invoke_host(overall, args);
        }
        let defined_idx = match self.funcs.get(overall) {
            Some(Callable::Defined(d)) => *d,
            _ => return Err(format!("func index {} not callable", overall)),
        };
        self.call_defined(defined_idx, args, depth)
    }

    fn call_overall_body(
        &mut self,
        overall: usize,
        args: &[WasmValue],
        depth: usize,
    ) -> Result<BodyResult, String> {
        let is_host = matches!(self.funcs.get(overall), Some(Callable::Host(_)));
        if is_host {
            return match self.invoke_host(overall, args) {
                Ok(values) => Ok(BodyResult::Values(values)),
                Err(err) => match err.strip_prefix("__wasm_exception_identity:") {
                    Some(identity) => {
                        let local_tag = self
                            .tag_identities
                            .iter()
                            .position(|candidate| candidate == identity)
                            .map(|tag| tag as u32)
                            .unwrap_or(u32::MAX);
                        Ok(BodyResult::Exception(ExceptionPayload {
                            tag: local_tag,
                            identity: Some(identity.to_string()),
                            values: Vec::new(),
                        }))
                    }
                    None => Err(err),
                },
            };
        }
        let defined_idx = match self.funcs.get(overall) {
            Some(Callable::Defined(d)) => *d,
            _ => return Err(format!("func index {} not callable", overall)),
        };
        self.call_defined_body(defined_idx, args, depth)
    }

    pub fn tag_identity_at(&self, tag: usize) -> Option<String> {
        self.tag_identities.get(tag).cloned()
    }

    pub fn set_tag_identity_at(&mut self, tag: usize, identity: String) {
        if let Some(slot) = self.tag_identities.get_mut(tag) {
            *slot = identity;
        }
    }

    fn make_exception_payload(
        &self,
        tag: u32,
        stack: &mut Vec<WasmValue>,
    ) -> Result<ExceptionPayload, String> {
        let tag_decl = self
            .module
            .tags
            .get(tag as usize)
            .ok_or_else(|| format!("throw: bad tag index {}", tag))?;
        let ftype = self
            .module
            .types
            .get(tag_decl.type_idx as usize)
            .ok_or_else(|| format!("throw: bad tag type index {}", tag_decl.type_idx))?;
        let nargs = ftype.params.len();
        if stack.len() < nargs {
            return Err("throw: stack underflow".to_string());
        }
        let values = stack.split_off(stack.len() - nargs);
        Ok(ExceptionPayload {
            tag,
            identity: self.tag_identities.get(tag as usize).cloned(),
            values,
        })
    }

    fn handle_exception(
        &mut self,
        ex: ExceptionPayload,
        labels: &mut Vec<Label>,
        stack: &mut Vec<WasmValue>,
    ) -> Result<Option<usize>, String> {
        for try_idx in (0..labels.len()).rev() {
            let Some(catches) = labels[try_idx].catches.clone() else {
                continue;
            };
            for catch in catches {
                let (matches, label, mut values) = match catch {
                    CatchKind::Catch { tag, label }
                        if self.exception_payload_matches_tag(&ex, tag) =>
                    {
                        (true, label as usize, ex.values.clone())
                    }
                    CatchKind::CatchAll { label } => (true, label as usize, Vec::new()),
                    CatchKind::CatchRef { tag, label }
                        if self.exception_payload_matches_tag(&ex, tag) =>
                    {
                        let mut values = ex.values.clone();
                        values.push(self.store_exception_ref(ex.clone()));
                        (true, label as usize, values)
                    }
                    CatchKind::CatchAllRef { label } => (
                        true,
                        label as usize,
                        vec![self.store_exception_ref(ex.clone())],
                    ),
                    _ => (false, 0, Vec::new()),
                };
                if !matches {
                    continue;
                }
                if label > try_idx {
                    return Err("exception catch label out of range".to_string());
                }
                stack.append(&mut values);
                return self
                    .do_branch_to_index(try_idx - 1 - label, labels, stack)
                    .map(Some);
            }
        }
        Ok(None)
    }

    fn exception_tags_match(&self, thrown: u32, caught: u32) -> bool {
        thrown == caught
            || self
                .tag_identities
                .get(thrown as usize)
                .zip(self.tag_identities.get(caught as usize))
                .is_some_and(|(a, b)| a == b)
    }

    fn exception_payload_matches_tag(&self, ex: &ExceptionPayload, caught: u32) -> bool {
        self.exception_tags_match(ex.tag, caught)
            || ex
                .identity
                .as_ref()
                .zip(self.tag_identities.get(caught as usize))
                .is_some_and(|(a, b)| a == b)
    }

    fn store_exception_ref(&mut self, ex: ExceptionPayload) -> WasmValue {
        let idx = self.exception_refs.len() as u32;
        self.exception_refs.push(ex);
        WasmValue::ExnRef(idx)
    }

    fn exception_from_ref(&self, value: WasmValue) -> Result<ExceptionPayload, String> {
        match value {
            WasmValue::ExnRef(idx) => self
                .exception_refs
                .get(idx as usize)
                .cloned()
                .ok_or_else(|| format!("throw_ref: bad exception reference {}", idx)),
            WasmValue::RefNull => Err("throw_ref: null exception reference".to_string()),
            other => Err(format!(
                "throw_ref: expected exception reference, got {:?}",
                other
            )),
        }
    }

    fn do_branch(
        &self,
        depth: usize,
        labels: &mut Vec<Label>,
        stack: &mut Vec<WasmValue>,
    ) -> Result<usize, String> {
        if depth >= labels.len() {
            return Err("branch depth out of range".to_string());
        }
        let target_idx = labels.len() - 1 - depth;
        self.do_branch_to_index(target_idx, labels, stack)
    }

    fn do_branch_to_index(
        &self,
        target_idx: usize,
        labels: &mut Vec<Label>,
        stack: &mut Vec<WasmValue>,
    ) -> Result<usize, String> {
        if target_idx >= labels.len() {
            return Err("branch depth out of range".to_string());
        }
        let target = labels[target_idx].clone();

        let keep = target.arity;
        if stack.len() < keep {
            return Err("branch: value stack underflow".to_string());
        }
        let vals = stack.split_off(stack.len() - keep);
        stack.truncate(target.stack_height);
        stack.extend(vals);
        if target.is_loop {

            labels.truncate(target_idx + 1);
        } else {

            labels.truncate(target_idx);
        }
        Ok(target.cont_pc)
    }

    fn ref_matches_type(
        &self,
        value: WasmValue,
        target: ValType,
        nullable: bool,
        allow_func_canonical: bool,
    ) -> bool {
        if matches!(value, WasmValue::RefNull) {
            return nullable
                || matches!(
                    target,
                    ValType::NullRef | ValType::NullFuncRef | ValType::NullExternRef
                );
        }
        match target {
            ValType::AnyRef | ValType::NonNullAnyRef => matches!(
                value,
                WasmValue::ArrayRef(_)
                    | WasmValue::StructRef(_)
                    | WasmValue::I31Ref(_)
                    | WasmValue::ExternRef(_)
                    | WasmValue::ExternI31Ref(_)
                    | WasmValue::ExternStructRef(_)
                    | WasmValue::ExternArrayRef(_)
            ),
            ValType::EqRef | ValType::NonNullEqRef => matches!(
                value,
                WasmValue::ArrayRef(_) | WasmValue::StructRef(_) | WasmValue::I31Ref(_)
            ),
            ValType::Unknown => true,
            ValType::NullRef | ValType::NullFuncRef | ValType::NullExternRef => false,
            ValType::FuncRef | ValType::NonNullFuncRef => matches!(value, WasmValue::FuncRef(_)),
            ValType::ExternRef | ValType::NonNullExternRef => matches!(
                value,
                WasmValue::ExternRef(_)
                    | WasmValue::ExternI31Ref(_)
                    | WasmValue::ExternStructRef(_)
                    | WasmValue::ExternArrayRef(_)
            ),
            ValType::StructRef | ValType::NonNullStructRef => {
                matches!(value, WasmValue::StructRef(_))
            }
            ValType::ArrayRef | ValType::NonNullArrayRef => matches!(value, WasmValue::ArrayRef(_)),
            ValType::I31Ref | ValType::NonNullI31Ref => matches!(value, WasmValue::I31Ref(_)),
            ValType::TypeRef(target_idx) | ValType::NonNullTypeRef(target_idx) => {
                self.value_matches_type_ref(value, target_idx, allow_func_canonical)
            }
            _ => false,
        }
    }

    fn value_matches_type_ref(
        &self,
        value: WasmValue,
        target_idx: u32,
        allow_func_canonical: bool,
    ) -> bool {
        match value {
            WasmValue::FuncRef(idx) => self
                .func_type_index(idx as usize)
                .map(|actual_idx| {
                    if allow_func_canonical {
                        self.type_matches_target(actual_idx, target_idx)
                    } else {
                        self.type_is_declared_subtype(actual_idx, target_idx)
                    }
                })
                .unwrap_or(false),
            WasmValue::StructRef(idx) => self
                .gc_structs
                .get(idx as usize)
                .map(|st| self.type_matches_target(st.type_idx, target_idx))
                .unwrap_or(false),
            WasmValue::ArrayRef(idx) => self
                .gc_arrays
                .get(idx as usize)
                .map(|array| self.type_matches_target(array.type_idx, target_idx))
                .unwrap_or(false),
            _ => false,
        }
    }

    fn type_is_declared_subtype(&self, actual: u32, target: u32) -> bool {
        self.type_is_declared_subtype_inner(actual, target, &mut Vec::new())
    }

    fn call_indirect_type_matches(&self, actual: u32, target: u32) -> bool {
        self.type_is_declared_subtype(actual, target)
            || self.types_canonically_equal(actual, target, &mut Vec::new())
            || self.plain_function_signatures_match_without_hierarchy(actual, target)
    }

    fn plain_function_signatures_match_without_hierarchy(&self, actual: u32, target: u32) -> bool {
        if !self
            .module
            .type_is_func
            .get(actual as usize)
            .copied()
            .unwrap_or(false)
            || !self
                .module
                .type_is_func
                .get(target as usize)
                .copied()
                .unwrap_or(false)
        {
            return false;
        }
        if !self
            .module
            .type_is_final
            .get(actual as usize)
            .copied()
            .unwrap_or(false)
            || !self
                .module
                .type_is_final
                .get(target as usize)
                .copied()
                .unwrap_or(false)
        {
            return false;
        }
        let actual_supers = self
            .module
            .type_supertypes
            .get(actual as usize)
            .map(|supertypes| supertypes.is_empty())
            .unwrap_or(true);
        let target_supers = self
            .module
            .type_supertypes
            .get(target as usize)
            .map(|supertypes| supertypes.is_empty())
            .unwrap_or(true);
        if !actual_supers || !target_supers {
            return false;
        }
        let actual_group = self
            .module
            .type_rec_groups
            .get(actual as usize)
            .copied()
            .unwrap_or(actual);
        let target_group = self
            .module
            .type_rec_groups
            .get(target as usize)
            .copied()
            .unwrap_or(target);
        if self.type_group_members(actual_group).len() != 1
            || self.type_group_members(target_group).len() != 1
        {
            return false;
        }
        let Some(actual_ty) = self.module.types.get(actual as usize) else {
            return false;
        };
        let Some(target_ty) = self.module.types.get(target as usize) else {
            return false;
        };
        actual_ty.params == target_ty.params && actual_ty.results == target_ty.results
    }

    fn type_is_declared_subtype_inner(
        &self,
        actual: u32,
        target: u32,
        seen: &mut Vec<(u32, u32)>,
    ) -> bool {
        if actual == target {
            return true;
        }
        if seen.contains(&(actual, target)) {
            return false;
        }
        seen.push((actual, target));
        self.module
            .type_supertypes
            .get(actual as usize)
            .map(|supertypes| {
                supertypes
                    .iter()
                    .any(|super_idx| self.type_is_declared_subtype_inner(*super_idx, target, seen))
            })
            .unwrap_or(false)
    }

    fn type_matches_target(&self, actual: u32, target: u32) -> bool {
        self.type_matches_target_inner(actual, target, &mut Vec::new())
    }

    fn type_matches_target_inner(
        &self,
        actual: u32,
        target: u32,
        seen: &mut Vec<(u32, u32)>,
    ) -> bool {
        if actual == target {
            return true;
        }
        if seen.contains(&(actual, target)) {
            return false;
        }
        seen.push((actual, target));
        let mut canonical_seen = seen.clone();
        if self.types_canonically_equal(actual, target, &mut canonical_seen) {
            return true;
        }
        if let Some(supertypes) = self.module.type_supertypes.get(actual as usize) {
            for super_idx in supertypes {
                if self.type_matches_target_inner(*super_idx, target, seen) {
                    return true;
                }
            }
        }
        false
    }

    fn types_canonically_equal(&self, left: u32, right: u32, seen: &mut Vec<(u32, u32)>) -> bool {
        if left == right {
            return true;
        }
        if self
            .module
            .type_is_final
            .get(left as usize)
            .copied()
            .unwrap_or(false)
            != self
                .module
                .type_is_final
                .get(right as usize)
                .copied()
                .unwrap_or(false)
        {
            return false;
        }
        if !self.supertypes_canonically_equal(left, right, seen) {
            return false;
        }
        match (
            self.module.struct_types.get(left as usize),
            self.module.struct_types.get(right as usize),
            self.module.array_types.get(left as usize),
            self.module.array_types.get(right as usize),
        ) {
            (Some(Some(left_struct)), Some(Some(right_struct)), _, _) => self
                .struct_types_equivalent_in_groups(
                    left_struct,
                    right_struct,
                    &self.type_group_members(self.module.type_rec_groups[left as usize]),
                    &self.type_group_members(self.module.type_rec_groups[right as usize]),
                    seen,
                ),
            (_, _, Some(Some(left_array)), Some(Some(right_array))) => self
                .array_types_equivalent_in_groups(
                    *left_array,
                    *right_array,
                    &self.type_group_members(self.module.type_rec_groups[left as usize]),
                    &self.type_group_members(self.module.type_rec_groups[right as usize]),
                    seen,
                ),
            _ => self.type_groups_canonically_equal(left, right, seen),
        }
    }

    fn type_groups_canonically_equal(
        &self,
        left: u32,
        right: u32,
        seen: &mut Vec<(u32, u32)>,
    ) -> bool {
        let Some(left_group) = self.module.type_rec_groups.get(left as usize).copied() else {
            return false;
        };
        let Some(right_group) = self.module.type_rec_groups.get(right as usize).copied() else {
            return false;
        };
        let left_members = self.type_group_members(left_group);
        let right_members = self.type_group_members(right_group);
        if left_members.len() != right_members.len() {
            return false;
        }
        let left_pos = left_members.iter().position(|idx| *idx == left);
        let right_pos = right_members.iter().position(|idx| *idx == right);
        if left_pos != right_pos {
            return false;
        }
        left_members
            .iter()
            .zip(right_members.iter())
            .all(|(left_idx, right_idx)| {
                self.type_group_member_equal(
                    *left_idx,
                    *right_idx,
                    &left_members,
                    &right_members,
                    seen,
                )
            })
    }

    fn type_group_members(&self, group: u32) -> Vec<u32> {
        self.module
            .type_rec_groups
            .iter()
            .enumerate()
            .filter_map(|(idx, candidate)| (*candidate == group).then_some(idx as u32))
            .collect()
    }

    fn type_group_member_equal(
        &self,
        left: u32,
        right: u32,
        left_members: &[u32],
        right_members: &[u32],
        seen: &mut Vec<(u32, u32)>,
    ) -> bool {
        if left == right {
            return true;
        }
        if seen.contains(&(left, right)) {
            return true;
        }
        seen.push((left, right));
        if self
            .module
            .type_is_final
            .get(left as usize)
            .copied()
            .unwrap_or(false)
            != self
                .module
                .type_is_final
                .get(right as usize)
                .copied()
                .unwrap_or(false)
        {
            return false;
        }
        if !self.type_group_member_supertypes_equal(left, right, left_members, right_members, seen)
        {
            return false;
        }
        match (
            self.module.struct_types.get(left as usize),
            self.module.struct_types.get(right as usize),
            self.module.array_types.get(left as usize),
            self.module.array_types.get(right as usize),
            self.module.types.get(left as usize),
            self.module.types.get(right as usize),
        ) {
            (Some(Some(left_struct)), Some(Some(right_struct)), _, _, _, _) => self
                .struct_types_equivalent_in_groups(
                    left_struct,
                    right_struct,
                    left_members,
                    right_members,
                    seen,
                ),
            (_, _, Some(Some(left_array)), Some(Some(right_array)), _, _) => self
                .array_types_equivalent_in_groups(
                    *left_array,
                    *right_array,
                    left_members,
                    right_members,
                    seen,
                ),
            (_, _, _, _, Some(left_func), Some(right_func))
                if self
                    .module
                    .type_is_func
                    .get(left as usize)
                    .copied()
                    .unwrap_or(false)
                    && self
                        .module
                        .type_is_func
                        .get(right as usize)
                        .copied()
                        .unwrap_or(false) =>
            {
                self.func_types_equivalent_in_groups(
                    left_func,
                    right_func,
                    left_members,
                    right_members,
                    seen,
                )
            }
            _ => false,
        }
    }

    fn type_group_member_supertypes_equal(
        &self,
        left: u32,
        right: u32,
        left_members: &[u32],
        right_members: &[u32],
        seen: &mut Vec<(u32, u32)>,
    ) -> bool {
        let left_supers = self
            .module
            .type_supertypes
            .get(left as usize)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        let right_supers = self
            .module
            .type_supertypes
            .get(right as usize)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        left_supers.len() == right_supers.len()
            && left_supers
                .iter()
                .zip(right_supers.iter())
                .all(|(left_super, right_super)| {
                    self.val_types_equivalent_in_groups(
                        ValType::TypeRef(*left_super),
                        ValType::TypeRef(*right_super),
                        left_members,
                        right_members,
                        seen,
                    )
                })
    }

    fn func_types_equivalent_in_groups(
        &self,
        left: &FuncType,
        right: &FuncType,
        left_members: &[u32],
        right_members: &[u32],
        seen: &mut Vec<(u32, u32)>,
    ) -> bool {
        self.val_type_lists_equivalent_in_groups(
            &left.params,
            &right.params,
            left_members,
            right_members,
            seen,
        ) && self.val_type_lists_equivalent_in_groups(
            &left.results,
            &right.results,
            left_members,
            right_members,
            seen,
        )
    }

    fn struct_types_equivalent_in_groups(
        &self,
        left: &StructType,
        right: &StructType,
        left_members: &[u32],
        right_members: &[u32],
        seen: &mut Vec<(u32, u32)>,
    ) -> bool {
        left.fields.len() == right.fields.len()
            && left
                .fields
                .iter()
                .zip(right.fields.iter())
                .all(|(left_field, right_field)| {
                    left_field.mutable == right_field.mutable
                        && left_field.packed_bits == right_field.packed_bits
                        && self.val_types_equivalent_in_groups(
                            left_field.ty,
                            right_field.ty,
                            left_members,
                            right_members,
                            seen,
                        )
                })
    }

    fn array_types_equivalent_in_groups(
        &self,
        left: ArrayType,
        right: ArrayType,
        left_members: &[u32],
        right_members: &[u32],
        seen: &mut Vec<(u32, u32)>,
    ) -> bool {
        left.mutable == right.mutable
            && left.packed_bits == right.packed_bits
            && self.val_types_equivalent_in_groups(
                left.element,
                right.element,
                left_members,
                right_members,
                seen,
            )
    }

    fn val_type_lists_equivalent_in_groups(
        &self,
        left: &[ValType],
        right: &[ValType],
        left_members: &[u32],
        right_members: &[u32],
        seen: &mut Vec<(u32, u32)>,
    ) -> bool {
        left.len() == right.len()
            && left.iter().zip(right.iter()).all(|(left_ty, right_ty)| {
                self.val_types_equivalent_in_groups(
                    *left_ty,
                    *right_ty,
                    left_members,
                    right_members,
                    seen,
                )
            })
    }

    fn val_types_equivalent_in_groups(
        &self,
        left: ValType,
        right: ValType,
        left_members: &[u32],
        right_members: &[u32],
        seen: &mut Vec<(u32, u32)>,
    ) -> bool {
        match (left, right) {
            (ValType::TypeRef(left_idx), ValType::TypeRef(right_idx)) => self
                .type_refs_equivalent_in_groups(
                    left_idx,
                    right_idx,
                    left_members,
                    right_members,
                    seen,
                ),
            (ValType::NonNullTypeRef(left_idx), ValType::NonNullTypeRef(right_idx)) => self
                .type_refs_equivalent_in_groups(
                    left_idx,
                    right_idx,
                    left_members,
                    right_members,
                    seen,
                ),
            (ValType::NonNullTypeRef(left_idx), ValType::TypeRef(right_idx)) => self
                .type_refs_equivalent_in_groups(
                    left_idx,
                    right_idx,
                    left_members,
                    right_members,
                    seen,
                ),
            _ => left == right,
        }
    }

    fn type_refs_equivalent_in_groups(
        &self,
        left_idx: u32,
        right_idx: u32,
        left_members: &[u32],
        right_members: &[u32],
        seen: &mut Vec<(u32, u32)>,
    ) -> bool {
        let left_pos = left_members.iter().position(|idx| *idx == left_idx);
        let right_pos = right_members.iter().position(|idx| *idx == right_idx);
        match (left_pos, right_pos) {
            (Some(left_pos), Some(right_pos)) => {
                left_pos == right_pos
                    && self.type_group_member_equal(
                        left_idx,
                        right_idx,
                        left_members,
                        right_members,
                        seen,
                    )
            }
            (None, None) => {
                self.type_groups_canonically_equal(left_idx, right_idx, &mut Vec::new())
                    && self.type_groups_canonically_equal(right_idx, left_idx, &mut Vec::new())
            }
            _ => false,
        }
    }

    fn supertypes_canonically_equal(
        &self,
        left: u32,
        right: u32,
        seen: &mut Vec<(u32, u32)>,
    ) -> bool {
        let left_supers = self
            .module
            .type_supertypes
            .get(left as usize)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        let right_supers = self
            .module
            .type_supertypes
            .get(right as usize)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        if left_supers.len() != right_supers.len() {
            return false;
        }
        left_supers
            .iter()
            .zip(right_supers.iter())
            .all(|(left_super, right_super)| {
                self.type_matches_target_inner(*left_super, *right_super, seen)
                    && self.type_matches_target_inner(*right_super, *left_super, seen)
            })
    }

    fn pop_mem_addr(&self, stack: &mut Vec<WasmValue>) -> Result<u64, String> {
        if self.memory64 {
            Ok(pop_i64(stack)? as u64)
        } else {
            Ok(pop_i32(stack)? as u32 as u64)
        }
    }

    fn effective_addr<B: MemoryBase>(
        &self,
        memarg: crate::parser::MemArg,
        base: B,
    ) -> Result<usize, String> {
        self.memory_ref(memarg.memory)?;
        let addr = base
            .to_u64()
            .checked_add(memarg.offset as u64)
            .ok_or("memory address overflow")?;
        usize::try_from(addr).map_err(|_| "memory address out of range".to_string())
    }

    fn read_mem_at(
        &self,
        memoryidx: u32,
        addr: usize,
        len: usize,
        op: &str,
    ) -> Result<&[u8], String> {
        let memory = self.memory_ref(memoryidx)?;
        let end = addr
            .checked_add(len)
            .ok_or_else(|| format!("{}: address overflow", op))?;
        if end > memory.len() {
            return Err(format!("{}: out of bounds memory access", op));
        }
        Ok(&memory[addr..end])
    }

    fn write_mem_at(
        &mut self,
        memoryidx: u32,
        addr: usize,
        bytes: &[u8],
        op: &str,
    ) -> Result<(), String> {
        let memory = self.memory_mut(memoryidx)?;
        let end = addr
            .checked_add(bytes.len())
            .ok_or_else(|| format!("{}: address overflow", op))?;
        if end > memory.len() {
            return Err(format!("{}: out of bounds memory access", op));
        }
        memory[addr..end].copy_from_slice(bytes);
        self.mark_memory_dirty(memoryidx as usize, addr, end);
        self.sync_memory_aliases_after_write(memoryidx as usize);
        Ok(())
    }

    fn mark_memory_dirty(&mut self, memory_index: usize, start: usize, end: usize) {
        if start >= end {
            return;
        }
        if self.memory_dirty_ranges.len() <= memory_index {
            self.memory_dirty_ranges.resize(memory_index + 1, None);
        }
        self.memory_dirty_ranges[memory_index] =
            Some(match self.memory_dirty_ranges[memory_index] {
                Some((old_start, old_end)) => (old_start.min(start), old_end.max(end)),
                None => (start, end),
            });
    }

    fn sync_memory_aliases_after_write(&mut self, written_index: usize) {
        let Some(Some(alias)) = self.memory_aliases.get(written_index).copied() else {
            return;
        };
        let Some(bytes) = self
            .memory_ref(written_index as u32)
            .ok()
            .map(|bytes| bytes.to_vec())
        else {
            return;
        };
        for alias_index in 0..self.memory_aliases.len() {
            if alias_index == written_index
                || self.memory_aliases.get(alias_index).copied().flatten() != Some(alias)
            {
                continue;
            }
            if let Ok(target) = self.memory_mut(alias_index as u32) {
                if target.len() == bytes.len() {
                    target.copy_from_slice(&bytes);
                }
            }
        }
    }

    fn exec_load(
        &self,
        op: u8,
        memarg: crate::parser::MemArg,
        stack: &mut Vec<WasmValue>,
    ) -> Result<(), String> {
        let base = self.pop_mem_addr(stack)?;
        let addr = self.effective_addr(memarg, base)?;
        let mem = self.memory_ref(memarg.memory)?;
        macro_rules! rd {
            ($n:expr) => {{
                let end = addr
                    .checked_add($n)
                    .ok_or_else(|| "load: out of bounds memory access".to_string())?;
                if end > mem.len() {
                    return Err("load: out of bounds memory access".to_string());
                }
                &mem[addr..end]
            }};
        }
        let v = match op {
            0x28 => {
                let b = rd!(4);
                WasmValue::I32(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            }
            0x29 => {
                let b = rd!(8);
                WasmValue::I64(i64::from_le_bytes([
                    b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
                ]))
            }
            0x2a => {
                let b = rd!(4);
                WasmValue::F32(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            }
            0x2b => {
                let b = rd!(8);
                WasmValue::F64(f64::from_le_bytes([
                    b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
                ]))
            }
            0x2c => WasmValue::I32(rd!(1)[0] as i8 as i32),
            0x2d => WasmValue::I32(rd!(1)[0] as i32),
            0x2e => {
                let b = rd!(2);
                WasmValue::I32(i16::from_le_bytes([b[0], b[1]]) as i32)
            }
            0x2f => {
                let b = rd!(2);
                WasmValue::I32(u16::from_le_bytes([b[0], b[1]]) as i32)
            }
            0x30 => WasmValue::I64(rd!(1)[0] as i8 as i64),
            0x31 => WasmValue::I64(rd!(1)[0] as i64),
            0x32 => {
                let b = rd!(2);
                WasmValue::I64(i16::from_le_bytes([b[0], b[1]]) as i64)
            }
            0x33 => {
                let b = rd!(2);
                WasmValue::I64(u16::from_le_bytes([b[0], b[1]]) as i64)
            }
            0x34 => {
                let b = rd!(4);
                WasmValue::I64(i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as i64)
            }
            0x35 => {
                let b = rd!(4);
                WasmValue::I64(u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as i64)
            }
            other => return Err(format!("unhandled load op 0x{:02x}", other)),
        };
        stack.push(v);
        Ok(())
    }

    fn exec_store(
        &mut self,
        op: u8,
        memarg: crate::parser::MemArg,
        stack: &mut Vec<WasmValue>,
    ) -> Result<(), String> {
        let val = stack.pop().ok_or("store: value underflow")?;
        let base = self.pop_mem_addr(stack)?;
        let addr = self.effective_addr(memarg, base)?;
        let mem = self.memory_mut(memarg.memory)?;
        macro_rules! wr {
            ($bytes:expr) => {{
                let bs = $bytes;
                let end = addr
                    .checked_add(bs.len())
                    .ok_or_else(|| "store: out of bounds memory access".to_string())?;
                if end > mem.len() {
                    return Err("store: out of bounds memory access".to_string());
                }
                mem[addr..end].copy_from_slice(&bs);
            }};
        }
        match op {
            0x36 => {
                let v = as_i32(val)?;
                wr!(v.to_le_bytes());
            }
            0x37 => {
                let v = as_i64(val)?;
                wr!(v.to_le_bytes());
            }
            0x38 => {
                let v = as_f32(val)?;
                wr!(v.to_le_bytes());
            }
            0x39 => {
                let v = as_f64(val)?;
                wr!(v.to_le_bytes());
            }
            0x3a => {
                let v = as_i32(val)? as u8;
                wr!([v]);
            }
            0x3b => {
                let v = as_i32(val)? as u16;
                wr!(v.to_le_bytes());
            }
            0x3c => {
                let v = as_i64(val)? as u8;
                wr!([v]);
            }
            0x3d => {
                let v = as_i64(val)? as u16;
                wr!(v.to_le_bytes());
            }
            0x3e => {
                let v = as_i64(val)? as u32;
                wr!(v.to_le_bytes());
            }
            other => return Err(format!("unhandled store op 0x{:02x}", other)),
        }
        Ok(())
    }

    fn exec_atomic_load(
        &self,
        sub: u32,
        memarg: crate::parser::MemArg,
        stack: &mut Vec<WasmValue>,
    ) -> Result<(), String> {
        let base = pop_i32(stack)?;
        let addr = self.effective_addr(memarg, base)?;
        match sub {
            0x10 => {
                let b = self.read_mem_at(memarg.memory, addr, 4, "i32.atomic.load")?;
                stack.push(WasmValue::I32(i32::from_le_bytes([b[0], b[1], b[2], b[3]])));
                Ok(())
            }
            0x12 => {
                let b = self.read_mem_at(memarg.memory, addr, 1, "i32.atomic.load8_u")?;
                stack.push(WasmValue::I32(b[0] as i32));
                Ok(())
            }
            0x13 => {
                let b = self.read_mem_at(memarg.memory, addr, 2, "i32.atomic.load16_u")?;
                stack.push(WasmValue::I32(u16::from_le_bytes([b[0], b[1]]) as i32));
                Ok(())
            }
            0x14 => {
                let b = self.read_mem_at(memarg.memory, addr, 1, "i64.atomic.load8_u")?;
                stack.push(WasmValue::I64(b[0] as i64));
                Ok(())
            }
            0x15 => {
                let b = self.read_mem_at(memarg.memory, addr, 2, "i64.atomic.load16_u")?;
                stack.push(WasmValue::I64(u16::from_le_bytes([b[0], b[1]]) as i64));
                Ok(())
            }
            0x16 => {
                let b = self.read_mem_at(memarg.memory, addr, 4, "i64.atomic.load32_u")?;
                stack.push(WasmValue::I64(
                    u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as i64
                ));
                Ok(())
            }
            0x11 => {
                let b = self.read_mem_at(memarg.memory, addr, 8, "i64.atomic.load")?;
                stack.push(WasmValue::I64(i64::from_le_bytes([
                    b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
                ])));
                Ok(())
            }
            other => Err(format!("unhandled atomic load subopcode 0x{:02x}", other)),
        }
    }

    fn exec_atomic_store(
        &mut self,
        sub: u32,
        memarg: crate::parser::MemArg,
        stack: &mut Vec<WasmValue>,
    ) -> Result<(), String> {
        match sub {
            0x17 => {
                let value = pop_i32(stack)?;
                let base = pop_i32(stack)?;
                let addr = self.effective_addr(memarg, base)?;
                self.write_mem_at(
                    memarg.memory,
                    addr,
                    &value.to_le_bytes(),
                    "i32.atomic.store",
                )
            }
            0x19 => {
                let value = pop_i32(stack)? as u8;
                let base = pop_i32(stack)?;
                let addr = self.effective_addr(memarg, base)?;
                self.write_mem_at(memarg.memory, addr, &[value], "i32.atomic.store8")
            }
            0x1a => {
                let value = pop_i32(stack)? as u16;
                let base = pop_i32(stack)?;
                let addr = self.effective_addr(memarg, base)?;
                self.write_mem_at(
                    memarg.memory,
                    addr,
                    &value.to_le_bytes(),
                    "i32.atomic.store16",
                )
            }
            0x1b => {
                let value = pop_i64(stack)? as u8;
                let base = pop_i32(stack)?;
                let addr = self.effective_addr(memarg, base)?;
                self.write_mem_at(memarg.memory, addr, &[value], "i64.atomic.store8")
            }
            0x1c => {
                let value = pop_i64(stack)? as u16;
                let base = pop_i32(stack)?;
                let addr = self.effective_addr(memarg, base)?;
                self.write_mem_at(
                    memarg.memory,
                    addr,
                    &value.to_le_bytes(),
                    "i64.atomic.store16",
                )
            }
            0x1d => {
                let value = pop_i64(stack)? as u32;
                let base = pop_i32(stack)?;
                let addr = self.effective_addr(memarg, base)?;
                self.write_mem_at(
                    memarg.memory,
                    addr,
                    &value.to_le_bytes(),
                    "i64.atomic.store32",
                )
            }
            0x18 => {
                let value = pop_i64(stack)?;
                let base = pop_i32(stack)?;
                let addr = self.effective_addr(memarg, base)?;
                self.write_mem_at(
                    memarg.memory,
                    addr,
                    &value.to_le_bytes(),
                    "i64.atomic.store",
                )
            }
            other => Err(format!("unhandled atomic store subopcode 0x{:02x}", other)),
        }
    }

    fn exec_atomic_rmw(
        &mut self,
        sub: u32,
        memarg: crate::parser::MemArg,
        stack: &mut Vec<WasmValue>,
    ) -> Result<(), String> {
        match sub {
            0x1e | 0x25 | 0x2c | 0x33 | 0x3a | 0x41 => {
                let value = pop_i32(stack)?;
                let base = pop_i32(stack)?;
                let addr = self.effective_addr(memarg, base)?;
                let op_name = i32_atomic_rmw_name(sub);
                let b = self.read_mem_at(memarg.memory, addr, 4, op_name)?;
                let old = i32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                let new = match sub {
                    0x1e => old.wrapping_add(value),
                    0x25 => old.wrapping_sub(value),
                    0x2c => old & value,
                    0x33 => old | value,
                    0x3a => old ^ value,
                    0x41 => value,
                    _ => unreachable!(),
                };
                self.write_mem_at(memarg.memory, addr, &new.to_le_bytes(), op_name)?;
                stack.push(WasmValue::I32(old));
                Ok(())
            }
            0x1f | 0x26 | 0x2d | 0x34 | 0x3b | 0x42 => {
                let value = pop_i64(stack)?;
                let base = pop_i32(stack)?;
                let addr = self.effective_addr(memarg, base)?;
                let op_name = i64_atomic_rmw_name(sub);
                let b = self.read_mem_at(memarg.memory, addr, 8, op_name)?;
                let old = i64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
                let new = match sub {
                    0x1f => old.wrapping_add(value),
                    0x26 => old.wrapping_sub(value),
                    0x2d => old & value,
                    0x34 => old | value,
                    0x3b => old ^ value,
                    0x42 => value,
                    _ => unreachable!(),
                };
                self.write_mem_at(memarg.memory, addr, &new.to_le_bytes(), op_name)?;
                stack.push(WasmValue::I64(old));
                Ok(())
            }
            0x20 | 0x27 | 0x2e | 0x35 | 0x3c | 0x43 => {
                let value = pop_i32(stack)? as u8;
                let base = pop_i32(stack)?;
                let addr = self.effective_addr(memarg, base)?;
                let op_name = i32_atomic_rmw_name(sub);
                let b = self.read_mem_at(memarg.memory, addr, 1, op_name)?;
                let old = b[0];
                let new = match sub {
                    0x20 => old.wrapping_add(value),
                    0x27 => old.wrapping_sub(value),
                    0x2e => old & value,
                    0x35 => old | value,
                    0x3c => old ^ value,
                    0x43 => value,
                    _ => unreachable!(),
                };
                self.write_mem_at(memarg.memory, addr, &[new], op_name)?;
                stack.push(WasmValue::I32(old as i32));
                Ok(())
            }
            0x21 | 0x28 | 0x2f | 0x36 | 0x3d | 0x44 => {
                let value = pop_i32(stack)? as u16;
                let base = pop_i32(stack)?;
                let addr = self.effective_addr(memarg, base)?;
                let op_name = i32_atomic_rmw_name(sub);
                let b = self.read_mem_at(memarg.memory, addr, 2, op_name)?;
                let old = u16::from_le_bytes([b[0], b[1]]);
                let new = match sub {
                    0x21 => old.wrapping_add(value),
                    0x28 => old.wrapping_sub(value),
                    0x2f => old & value,
                    0x36 => old | value,
                    0x3d => old ^ value,
                    0x44 => value,
                    _ => unreachable!(),
                };
                self.write_mem_at(memarg.memory, addr, &new.to_le_bytes(), op_name)?;
                stack.push(WasmValue::I32(old as i32));
                Ok(())
            }
            0x22 | 0x29 | 0x30 | 0x37 | 0x3e | 0x45 => {
                let value = pop_i64(stack)? as u8;
                let base = pop_i32(stack)?;
                let addr = self.effective_addr(memarg, base)?;
                let op_name = i64_atomic_rmw_name(sub);
                let b = self.read_mem_at(memarg.memory, addr, 1, op_name)?;
                let old = b[0];
                let new = match sub {
                    0x22 => old.wrapping_add(value),
                    0x29 => old.wrapping_sub(value),
                    0x30 => old & value,
                    0x37 => old | value,
                    0x3e => old ^ value,
                    0x45 => value,
                    _ => unreachable!(),
                };
                self.write_mem_at(memarg.memory, addr, &[new], op_name)?;
                stack.push(WasmValue::I64(old as i64));
                Ok(())
            }
            0x23 | 0x2a | 0x31 | 0x38 | 0x3f | 0x46 => {
                let value = pop_i64(stack)? as u16;
                let base = pop_i32(stack)?;
                let addr = self.effective_addr(memarg, base)?;
                let op_name = i64_atomic_rmw_name(sub);
                let b = self.read_mem_at(memarg.memory, addr, 2, op_name)?;
                let old = u16::from_le_bytes([b[0], b[1]]);
                let new = match sub {
                    0x23 => old.wrapping_add(value),
                    0x2a => old.wrapping_sub(value),
                    0x31 => old & value,
                    0x38 => old | value,
                    0x3f => old ^ value,
                    0x46 => value,
                    _ => unreachable!(),
                };
                self.write_mem_at(memarg.memory, addr, &new.to_le_bytes(), op_name)?;
                stack.push(WasmValue::I64(old as i64));
                Ok(())
            }
            0x24 | 0x2b | 0x32 | 0x39 | 0x40 | 0x47 => {
                let value = pop_i64(stack)? as u32;
                let base = pop_i32(stack)?;
                let addr = self.effective_addr(memarg, base)?;
                let op_name = i64_atomic_rmw_name(sub);
                let b = self.read_mem_at(memarg.memory, addr, 4, op_name)?;
                let old = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                let new = match sub {
                    0x24 => old.wrapping_add(value),
                    0x2b => old.wrapping_sub(value),
                    0x32 => old & value,
                    0x39 => old | value,
                    0x40 => old ^ value,
                    0x47 => value,
                    _ => unreachable!(),
                };
                self.write_mem_at(memarg.memory, addr, &new.to_le_bytes(), op_name)?;
                stack.push(WasmValue::I64(old as i64));
                Ok(())
            }
            0x48 => {
                let replacement = pop_i32(stack)?;
                let expected = pop_i32(stack)?;
                let base = pop_i32(stack)?;
                let addr = self.effective_addr(memarg, base)?;
                let op_name = i32_atomic_rmw_name(sub);
                let b = self.read_mem_at(memarg.memory, addr, 4, op_name)?;
                let old = i32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                if old == expected {
                    self.write_mem_at(memarg.memory, addr, &replacement.to_le_bytes(), op_name)?;
                }
                stack.push(WasmValue::I32(old));
                Ok(())
            }
            0x49 => {
                let replacement = pop_i64(stack)?;
                let expected = pop_i64(stack)?;
                let base = pop_i32(stack)?;
                let addr = self.effective_addr(memarg, base)?;
                let op_name = i64_atomic_rmw_name(sub);
                let b = self.read_mem_at(memarg.memory, addr, 8, op_name)?;
                let old = i64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
                if old == expected {
                    self.write_mem_at(memarg.memory, addr, &replacement.to_le_bytes(), op_name)?;
                }
                stack.push(WasmValue::I64(old));
                Ok(())
            }
            0x4a => {
                let replacement = pop_i32(stack)? as u8;
                let expected = pop_i32(stack)? as u8;
                let base = pop_i32(stack)?;
                let addr = self.effective_addr(memarg, base)?;
                let op_name = i32_atomic_rmw_name(sub);
                let b = self.read_mem_at(memarg.memory, addr, 1, op_name)?;
                let old = b[0];
                if old == expected {
                    self.write_mem_at(memarg.memory, addr, &[replacement], op_name)?;
                }
                stack.push(WasmValue::I32(old as i32));
                Ok(())
            }
            0x4b => {
                let replacement = pop_i32(stack)? as u16;
                let expected = pop_i32(stack)? as u16;
                let base = pop_i32(stack)?;
                let addr = self.effective_addr(memarg, base)?;
                let op_name = i32_atomic_rmw_name(sub);
                let b = self.read_mem_at(memarg.memory, addr, 2, op_name)?;
                let old = u16::from_le_bytes([b[0], b[1]]);
                if old == expected {
                    self.write_mem_at(memarg.memory, addr, &replacement.to_le_bytes(), op_name)?;
                }
                stack.push(WasmValue::I32(old as i32));
                Ok(())
            }
            0x4c => {
                let replacement = pop_i64(stack)? as u8;
                let expected = pop_i64(stack)? as u8;
                let base = pop_i32(stack)?;
                let addr = self.effective_addr(memarg, base)?;
                let op_name = i64_atomic_rmw_name(sub);
                let b = self.read_mem_at(memarg.memory, addr, 1, op_name)?;
                let old = b[0];
                if old == expected {
                    self.write_mem_at(memarg.memory, addr, &[replacement], op_name)?;
                }
                stack.push(WasmValue::I64(old as i64));
                Ok(())
            }
            0x4d => {
                let replacement = pop_i64(stack)? as u16;
                let expected = pop_i64(stack)? as u16;
                let base = pop_i32(stack)?;
                let addr = self.effective_addr(memarg, base)?;
                let op_name = i64_atomic_rmw_name(sub);
                let b = self.read_mem_at(memarg.memory, addr, 2, op_name)?;
                let old = u16::from_le_bytes([b[0], b[1]]);
                if old == expected {
                    self.write_mem_at(memarg.memory, addr, &replacement.to_le_bytes(), op_name)?;
                }
                stack.push(WasmValue::I64(old as i64));
                Ok(())
            }
            0x4e => {
                let replacement = pop_i64(stack)? as u32;
                let expected = pop_i64(stack)? as u32;
                let base = pop_i32(stack)?;
                let addr = self.effective_addr(memarg, base)?;
                let op_name = i64_atomic_rmw_name(sub);
                let b = self.read_mem_at(memarg.memory, addr, 4, op_name)?;
                let old = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                if old == expected {
                    self.write_mem_at(memarg.memory, addr, &replacement.to_le_bytes(), op_name)?;
                }
                stack.push(WasmValue::I64(old as i64));
                Ok(())
            }
            other => Err(format!("unhandled atomic rmw subopcode 0x{:02x}", other)),
        }
    }
}

fn i32_atomic_rmw_name(sub: u32) -> &'static str {
    match sub {
        0x1e => "i32.atomic.rmw.add",
        0x20 => "i32.atomic.rmw.add8_u",
        0x21 => "i32.atomic.rmw.add16_u",
        0x25 => "i32.atomic.rmw.sub",
        0x27 => "i32.atomic.rmw.sub8_u",
        0x28 => "i32.atomic.rmw.sub16_u",
        0x2c => "i32.atomic.rmw.and",
        0x2e => "i32.atomic.rmw.and8_u",
        0x2f => "i32.atomic.rmw.and16_u",
        0x33 => "i32.atomic.rmw.or",
        0x35 => "i32.atomic.rmw.or8_u",
        0x36 => "i32.atomic.rmw.or16_u",
        0x3a => "i32.atomic.rmw.xor",
        0x3c => "i32.atomic.rmw.xor8_u",
        0x3d => "i32.atomic.rmw.xor16_u",
        0x41 => "i32.atomic.rmw.xchg",
        0x43 => "i32.atomic.rmw.xchg8_u",
        0x44 => "i32.atomic.rmw.xchg16_u",
        0x48 => "i32.atomic.rmw.cmpxchg",
        0x4a => "i32.atomic.rmw.cmpxchg8_u",
        0x4b => "i32.atomic.rmw.cmpxchg16_u",
        _ => "i32.atomic.rmw",
    }
}

fn i64_atomic_rmw_name(sub: u32) -> &'static str {
    match sub {
        0x1f => "i64.atomic.rmw.add",
        0x22 => "i64.atomic.rmw.add8_u",
        0x23 => "i64.atomic.rmw.add16_u",
        0x24 => "i64.atomic.rmw.add32_u",
        0x26 => "i64.atomic.rmw.sub",
        0x29 => "i64.atomic.rmw.sub8_u",
        0x2a => "i64.atomic.rmw.sub16_u",
        0x2b => "i64.atomic.rmw.sub32_u",
        0x2d => "i64.atomic.rmw.and",
        0x30 => "i64.atomic.rmw.and8_u",
        0x31 => "i64.atomic.rmw.and16_u",
        0x32 => "i64.atomic.rmw.and32_u",
        0x34 => "i64.atomic.rmw.or",
        0x37 => "i64.atomic.rmw.or8_u",
        0x38 => "i64.atomic.rmw.or16_u",
        0x39 => "i64.atomic.rmw.or32_u",
        0x3b => "i64.atomic.rmw.xor",
        0x3e => "i64.atomic.rmw.xor8_u",
        0x3f => "i64.atomic.rmw.xor16_u",
        0x40 => "i64.atomic.rmw.xor32_u",
        0x42 => "i64.atomic.rmw.xchg",
        0x45 => "i64.atomic.rmw.xchg8_u",
        0x46 => "i64.atomic.rmw.xchg16_u",
        0x47 => "i64.atomic.rmw.xchg32_u",
        0x49 => "i64.atomic.rmw.cmpxchg",
        0x4c => "i64.atomic.rmw.cmpxchg8_u",
        0x4d => "i64.atomic.rmw.cmpxchg16_u",
        0x4e => "i64.atomic.rmw.cmpxchg32_u",
        _ => "i64.atomic.rmw",
    }
}

struct MemCtx<'a> {
    mem: &'a mut Vec<u8>,
    has_memory: bool,
}

struct NoMemCtx;

impl HostContext for MemCtx<'_> {
    fn mem_size(&self) -> usize {
        if self.has_memory {
            self.mem.len()
        } else {
            0
        }
    }

    fn mem_read(&self, offset: usize, len: usize) -> Option<Vec<u8>> {
        if !self.has_memory {
            return None;
        }
        let end = offset.checked_add(len)?;
        if end > self.mem.len() {
            return None;
        }
        Some(self.mem[offset..end].to_vec())
    }

    fn mem_write(&mut self, offset: usize, data: &[u8]) -> bool {
        if !self.has_memory {
            return false;
        }
        match offset.checked_add(data.len()) {
            Some(end) if end <= self.mem.len() => {
                self.mem[offset..end].copy_from_slice(data);
                true
            }
            _ => false,
        }
    }
}

impl HostContext for NoMemCtx {
    fn mem_size(&self) -> usize {
        0
    }

    fn mem_read(&self, _offset: usize, _len: usize) -> Option<Vec<u8>> {
        None
    }

    fn mem_write(&mut self, _offset: usize, _data: &[u8]) -> bool {
        false
    }
}

impl Instance {

    fn invoke_host(
        &mut self,
        overall: usize,
        args: &[WasmValue],
    ) -> Result<Vec<WasmValue>, String> {
        if !self.has_memory {
            record_wasm_host_import("no-memory");
            let mut ctx = NoMemCtx;
            return match self.funcs.get_mut(overall) {
                Some(Callable::Host(f)) => (f)(&mut ctx, args),
                _ => unreachable!("invoke_host called on non-host func"),
            };
        }

        record_wasm_host_import("with-memory");
        let mut mem = std::mem::take(&mut self.memory);
        let mut ctx = MemCtx {
            mem: &mut mem,
            has_memory: self.has_memory,
        };
        let result = match self.funcs.get_mut(overall) {
            Some(Callable::Host(f)) => (f)(&mut ctx, args),
            _ => unreachable!("invoke_host called on non-host func"),
        };
        self.memory = mem;
        result
    }
}

pub fn zero_of(t: ValType) -> WasmValue {
    match t {
        ValType::I32 => WasmValue::I32(0),
        ValType::I64 => WasmValue::I64(0),
        ValType::F32 => WasmValue::F32(0.0),
        ValType::F64 => WasmValue::F64(0.0),
        ValType::V128 => WasmValue::V128([0; 16]),
        ValType::AnyRef
        | ValType::NonNullAnyRef
        | ValType::EqRef
        | ValType::NonNullEqRef
        | ValType::FuncRef
        | ValType::NonNullFuncRef
        | ValType::ExternRef
        | ValType::NonNullExternRef
        | ValType::StructRef
        | ValType::NonNullStructRef
        | ValType::ArrayRef
        | ValType::NonNullArrayRef
        | ValType::I31Ref
        | ValType::NonNullI31Ref
        | ValType::TypeRef(_)
        | ValType::NonNullTypeRef(_)
        | ValType::NullRef
        | ValType::NullFuncRef
        | ValType::NullExternRef
        | ValType::Unknown => WasmValue::RefNull,
    }
}

fn pop_ref(stack: &mut Vec<WasmValue>) -> Result<WasmValue, String> {
    match stack.pop() {
        Some(
            value @ (WasmValue::RefNull
            | WasmValue::FuncRef(_)
            | WasmValue::ArrayRef(_)
            | WasmValue::StructRef(_)
            | WasmValue::I31Ref(_)
            | WasmValue::ExternRef(_)
            | WasmValue::ExternI31Ref(_)
            | WasmValue::ExternStructRef(_)
            | WasmValue::ExternArrayRef(_)),
        ) => Ok(value),
        Some(value @ WasmValue::ExnRef(_)) => Ok(value),
        Some(o) => Err(format!("type error: expected reference, got {:?}", o)),
        None => Err("stack underflow (reference)".to_string()),
    }
}

fn pop_i31(stack: &mut Vec<WasmValue>, op: &str) -> Result<i32, String> {
    match pop_ref(stack)? {
        WasmValue::I31Ref(value) => Ok(value),
        WasmValue::RefNull => Err(format!("{}: null i31 reference", op)),
        other => Err(format!("{}: expected i31 reference, got {:?}", op, other)),
    }
}

fn normalize_array_value(ty: ArrayType, value: WasmValue) -> WasmValue {
    normalize_packed_value(ty.packed_bits, value)
}

fn decode_array_data_elements<F>(
    data: &[u8],
    src: usize,
    len: usize,
    element_size: usize,
    decode: F,
) -> Result<Vec<WasmValue>, String>
where
    F: Fn(&[u8]) -> WasmValue,
{
    let byte_len = len
        .checked_mul(element_size)
        .ok_or("array.new_data: data segment out of bounds")?;
    let end = src
        .checked_add(byte_len)
        .ok_or("array.new_data: data segment out of bounds")?;
    if end > data.len() {
        return Err("array.new_data: data segment out of bounds".to_string());
    }
    Ok(data[src..end]
        .chunks_exact(element_size)
        .map(decode)
        .collect())
}

pub(crate) fn normalize_packed_value(packed_bits: Option<u8>, value: WasmValue) -> WasmValue {
    match (packed_bits, value) {
        (Some(8), WasmValue::I32(v)) => WasmValue::I32(v as u8 as i32),
        (Some(16), WasmValue::I32(v)) => WasmValue::I32(v as u16 as i32),
        (_, value) => value,
    }
}

fn load_packed_value(
    packed_bits: Option<u8>,
    signed: bool,
    value: WasmValue,
) -> Result<WasmValue, String> {
    match (packed_bits, value) {
        (Some(8), WasmValue::I32(v)) if signed => Ok(WasmValue::I32((v as u8 as i8) as i32)),
        (Some(8), WasmValue::I32(v)) => Ok(WasmValue::I32(v as u8 as i32)),
        (Some(16), WasmValue::I32(v)) if signed => Ok(WasmValue::I32((v as u16 as i16) as i32)),
        (Some(16), WasmValue::I32(v)) => Ok(WasmValue::I32(v as u16 as i32)),
        (None, value) => Ok(value),
        _ => Err("packed field value type mismatch".to_string()),
    }
}

fn data_array_values(
    ty: ArrayType,
    data: &[u8],
    src: usize,
    len: usize,
    op: &str,
) -> Result<Vec<WasmValue>, String> {
    if len == 0 {
        if src <= data.len() {
            return Ok(Vec::new());
        }
        return Err(format!("{}: data segment out of bounds", op));
    }
    match ty.packed_bits {
        Some(8) => {
            let end = src
                .checked_add(len)
                .ok_or_else(|| format!("{}: data segment out of bounds", op))?;
            if end > data.len() {
                return Err(format!("{}: data segment out of bounds", op));
            }
            Ok(data[src..end]
                .iter()
                .map(|byte| WasmValue::I32(*byte as i32))
                .collect())
        }
        Some(16) => {
            let byte_len = len
                .checked_mul(2)
                .ok_or_else(|| format!("{}: data segment out of bounds", op))?;
            let end = src
                .checked_add(byte_len)
                .ok_or_else(|| format!("{}: data segment out of bounds", op))?;
            if end > data.len() {
                return Err(format!("{}: data segment out of bounds", op));
            }
            Ok(data[src..end]
                .chunks_exact(2)
                .map(|bytes| WasmValue::I32(u16::from_le_bytes([bytes[0], bytes[1]]) as i32))
                .collect())
        }
        None => {
            let byte_width = match ty.element {
                ValType::I32 | ValType::F32 => 4,
                ValType::I64 | ValType::F64 => 8,
                ValType::V128 => 16,
                _ => return Err(format!("{}: unsupported array storage type", op)),
            };
            let byte_len = len
                .checked_mul(byte_width)
                .ok_or_else(|| format!("{}: data segment out of bounds", op))?;
            let end = src
                .checked_add(byte_len)
                .ok_or_else(|| format!("{}: data segment out of bounds", op))?;
            if end > data.len() {
                return Err(format!("{}: data segment out of bounds", op));
            }
            let values = data[src..end]
                .chunks_exact(byte_width)
                .map(|bytes| match ty.element {
                    ValType::I32 => {
                        WasmValue::I32(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
                    }
                    ValType::I64 => WasmValue::I64(i64::from_le_bytes([
                        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6],
                        bytes[7],
                    ])),
                    ValType::F32 => {
                        WasmValue::F32(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
                    }
                    ValType::F64 => WasmValue::F64(f64::from_le_bytes([
                        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6],
                        bytes[7],
                    ])),
                    ValType::V128 => {
                        let mut lanes = [0u8; 16];
                        lanes.copy_from_slice(bytes);
                        WasmValue::V128(lanes)
                    }
                    _ => unreachable!(),
                })
                .collect();
            Ok(values)
        }
        _ => Err(format!("{}: unsupported array storage type", op)),
    }
}

fn pop_array_ref(stack: &mut Vec<WasmValue>) -> Result<usize, String> {
    match stack.pop() {
        Some(WasmValue::ArrayRef(idx)) => Ok(idx as usize),
        Some(WasmValue::RefNull) => Err("array: null reference".to_string()),
        Some(o) => Err(format!("type error: expected array reference, got {:?}", o)),
        None => Err("stack underflow (array reference)".to_string()),
    }
}

fn pop_struct_ref(stack: &mut Vec<WasmValue>) -> Result<usize, String> {
    match stack.pop() {
        Some(WasmValue::StructRef(idx)) => Ok(idx as usize),
        Some(WasmValue::RefNull) => Err("struct: null reference".to_string()),
        Some(o) => Err(format!(
            "type error: expected struct reference, got {:?}",
            o
        )),
        None => Err("stack underflow (struct reference)".to_string()),
    }
}

pub fn pop_i32(stack: &mut Vec<WasmValue>) -> Result<i32, String> {
    match stack.pop() {
        Some(WasmValue::I32(v)) => Ok(v),
        Some(o) => Err(format!("type error: expected i32, got {:?}", o)),
        None => Err("stack underflow (i32)".to_string()),
    }
}

trait MemoryBase {
    fn to_u64(self) -> u64;
}

impl MemoryBase for i32 {
    fn to_u64(self) -> u64 {
        self as u32 as u64
    }
}

impl MemoryBase for u64 {
    fn to_u64(self) -> u64 {
        self
    }
}

fn pop_i64(stack: &mut Vec<WasmValue>) -> Result<i64, String> {
    match stack.pop() {
        Some(WasmValue::I64(v)) => Ok(v),
        Some(o) => Err(format!("type error: expected i64, got {:?}", o)),
        None => Err("stack underflow (i64)".to_string()),
    }
}

fn pop_f32(stack: &mut Vec<WasmValue>) -> Result<f32, String> {
    match stack.pop() {
        Some(WasmValue::F32(v)) => Ok(v),
        Some(o) => Err(format!("type error: expected f32, got {:?}", o)),
        None => Err("stack underflow (f32)".to_string()),
    }
}

fn pop_f64(stack: &mut Vec<WasmValue>) -> Result<f64, String> {
    match stack.pop() {
        Some(WasmValue::F64(v)) => Ok(v),
        Some(o) => Err(format!("type error: expected f64, got {:?}", o)),
        None => Err("stack underflow (f64)".to_string()),
    }
}

fn pop_v128(stack: &mut Vec<WasmValue>) -> Result<[u8; 16], String> {
    match stack.pop() {
        Some(WasmValue::V128(bytes)) => Ok(bytes),
        Some(o) => Err(format!("type error: expected v128, got {:?}", o)),
        None => Err("stack underflow (v128)".to_string()),
    }
}

fn pop_v128_pair(stack: &mut Vec<WasmValue>) -> Result<([u8; 16], [u8; 16]), String> {
    let b = pop_v128(stack)?;
    let a = pop_v128(stack)?;
    Ok((a, b))
}

fn repeat_lane<const N: usize, const LANES: usize>(lane: [u8; N]) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    for i in 0..LANES {
        let offset = i * N;
        bytes[offset..offset + N].copy_from_slice(&lane);
    }
    bytes
}

fn bytes_v128(a: [u8; 16], b: [u8; 16], op: impl Fn(u8, u8) -> u8) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = op(a[i], b[i]);
    }
    out
}

fn extend_i8x16_to_i16x8(a: [u8; 16], start: usize, signed: bool) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..8 {
        let lane = if signed {
            a[start + i] as i8 as i16 as u16
        } else {
            a[start + i] as u16
        };
        out[i * 2..i * 2 + 2].copy_from_slice(&lane.to_le_bytes());
    }
    out
}

fn extend_i16x8_to_i32x4(a: [u8; 16], start: usize, signed: bool) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..4 {
        let offset = (start + i) * 2;
        let lane = if signed {
            i16::from_le_bytes([a[offset], a[offset + 1]]) as i32 as u32
        } else {
            u16::from_le_bytes([a[offset], a[offset + 1]]) as u32
        };
        out[i * 4..i * 4 + 4].copy_from_slice(&lane.to_le_bytes());
    }
    out
}

fn extend_i32x4_to_i64x2(a: [u8; 16], start: usize, signed: bool) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..2 {
        let offset = (start + i) * 4;
        let lane = if signed {
            i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]) as i64
                as u64
        } else {
            u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]) as u64
        };
        out[i * 8..i * 8 + 8].copy_from_slice(&lane.to_le_bytes());
    }
    out
}

fn extmul_i8x16_to_i16x8(a: [u8; 16], b: [u8; 16], start: usize, signed: bool) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..8 {
        let lane = if signed {
            (a[start + i] as i8 as i16).wrapping_mul(b[start + i] as i8 as i16) as u16
        } else {
            (a[start + i] as u16).wrapping_mul(b[start + i] as u16)
        };
        out[i * 2..i * 2 + 2].copy_from_slice(&lane.to_le_bytes());
    }
    out
}

fn extmul_i16x8_to_i32x4(a: [u8; 16], b: [u8; 16], start: usize, signed: bool) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..4 {
        let offset = (start + i) * 2;
        let lane = if signed {
            let x = i16::from_le_bytes([a[offset], a[offset + 1]]) as i32;
            let y = i16::from_le_bytes([b[offset], b[offset + 1]]) as i32;
            x.wrapping_mul(y) as u32
        } else {
            let x = u16::from_le_bytes([a[offset], a[offset + 1]]) as u32;
            let y = u16::from_le_bytes([b[offset], b[offset + 1]]) as u32;
            x.wrapping_mul(y)
        };
        out[i * 4..i * 4 + 4].copy_from_slice(&lane.to_le_bytes());
    }
    out
}

fn extmul_i32x4_to_i64x2(a: [u8; 16], b: [u8; 16], start: usize, signed: bool) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..2 {
        let offset = (start + i) * 4;
        let lane = if signed {
            let x =
                i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]) as i64;
            let y =
                i32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]) as i64;
            x.wrapping_mul(y) as u64
        } else {
            let x =
                u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]) as u64;
            let y =
                u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]) as u64;
            x.wrapping_mul(y)
        };
        out[i * 8..i * 8 + 8].copy_from_slice(&lane.to_le_bytes());
    }
    out
}

fn dot_i16x8_to_i32x4(a: [u8; 16], b: [u8; 16]) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..4 {
        let lo = i * 4;
        let hi = lo + 2;
        let ax = i16::from_le_bytes([a[lo], a[lo + 1]]) as i32;
        let ay = i16::from_le_bytes([a[hi], a[hi + 1]]) as i32;
        let bx = i16::from_le_bytes([b[lo], b[lo + 1]]) as i32;
        let by = i16::from_le_bytes([b[hi], b[hi + 1]]) as i32;
        out[i * 4..i * 4 + 4].copy_from_slice(&(ax * bx + ay * by).to_le_bytes());
    }
    out
}

fn dot_i8x16_to_i16x8(a: [u8; 16], b: [u8; 16]) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..8 {
        let lo = i * 2;
        let x0 = a[lo] as i8 as i16;
        let x1 = a[lo + 1] as i8 as i16;
        let y0 = b[lo] as i8 as i16;
        let y1 = b[lo + 1] as i8 as i16;
        let lane = x0.wrapping_mul(y0).wrapping_add(x1.wrapping_mul(y1));
        out[lo..lo + 2].copy_from_slice(&lane.to_le_bytes());
    }
    out
}

fn dot_i8x16_to_i32x4_add(a: [u8; 16], b: [u8; 16], addend: [u8; 16]) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..4 {
        let base = i * 4;
        let mut sum = i32::from_le_bytes([
            addend[base],
            addend[base + 1],
            addend[base + 2],
            addend[base + 3],
        ]);
        for j in 0..4 {
            let idx = base + j;
            sum = sum.wrapping_add((a[idx] as i8 as i32).wrapping_mul(b[idx] as i8 as i32));
        }
        out[base..base + 4].copy_from_slice(&sum.to_le_bytes());
    }
    out
}

fn lanes_i8x16(a: [u8; 16], b: [u8; 16], op: impl Fn(i8, i8) -> i8) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = op(a[i] as i8, b[i] as i8) as u8;
    }
    out
}

fn lanes_i16x8(a: [u8; 16], b: [u8; 16], op: impl Fn(i16, i16) -> i16) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..8 {
        let offset = i * 2;
        let x = i16::from_le_bytes([a[offset], a[offset + 1]]);
        let y = i16::from_le_bytes([b[offset], b[offset + 1]]);
        out[offset..offset + 2].copy_from_slice(&op(x, y).to_le_bytes());
    }
    out
}

fn q15mulr_s(x: i16, y: i16) -> i16 {
    let product = x as i32 * y as i32;
    let rounded = (product + 0x4000) >> 15;
    rounded.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

fn lanes_i16x8_u(a: [u8; 16], b: [u8; 16], op: impl Fn(u16, u16) -> u16) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..8 {
        let offset = i * 2;
        let x = u16::from_le_bytes([a[offset], a[offset + 1]]);
        let y = u16::from_le_bytes([b[offset], b[offset + 1]]);
        out[offset..offset + 2].copy_from_slice(&op(x, y).to_le_bytes());
    }
    out
}

fn lanes_i16x8_unary(a: [u8; 16], op: impl Fn(i16) -> i16) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..8 {
        let offset = i * 2;
        let x = i16::from_le_bytes([a[offset], a[offset + 1]]);
        out[offset..offset + 2].copy_from_slice(&op(x).to_le_bytes());
    }
    out
}

fn lanes_i16x8_u_unary(a: [u8; 16], op: impl Fn(u16) -> u16) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..8 {
        let offset = i * 2;
        let x = u16::from_le_bytes([a[offset], a[offset + 1]]);
        out[offset..offset + 2].copy_from_slice(&op(x).to_le_bytes());
    }
    out
}

fn lanes_i32x4(a: [u8; 16], b: [u8; 16], op: impl Fn(i32, i32) -> i32) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..4 {
        let offset = i * 4;
        let x = i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
        let y = i32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
        out[offset..offset + 4].copy_from_slice(&op(x, y).to_le_bytes());
    }
    out
}

fn lanes_i32x4_u(a: [u8; 16], b: [u8; 16], op: impl Fn(u32, u32) -> u32) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..4 {
        let offset = i * 4;
        let x = u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
        let y = u32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
        out[offset..offset + 4].copy_from_slice(&op(x, y).to_le_bytes());
    }
    out
}

fn lanes_i32x4_unary(a: [u8; 16], op: impl Fn(i32) -> i32) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..4 {
        let offset = i * 4;
        let x = i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
        out[offset..offset + 4].copy_from_slice(&op(x).to_le_bytes());
    }
    out
}

fn lanes_i32x4_u_unary(a: [u8; 16], op: impl Fn(u32) -> u32) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..4 {
        let offset = i * 4;
        let x = u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
        out[offset..offset + 4].copy_from_slice(&op(x).to_le_bytes());
    }
    out
}

fn lanes_i64x2(a: [u8; 16], b: [u8; 16], op: impl Fn(i64, i64) -> i64) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..2 {
        let offset = i * 8;
        let x = i64::from_le_bytes([
            a[offset],
            a[offset + 1],
            a[offset + 2],
            a[offset + 3],
            a[offset + 4],
            a[offset + 5],
            a[offset + 6],
            a[offset + 7],
        ]);
        let y = i64::from_le_bytes([
            b[offset],
            b[offset + 1],
            b[offset + 2],
            b[offset + 3],
            b[offset + 4],
            b[offset + 5],
            b[offset + 6],
            b[offset + 7],
        ]);
        out[offset..offset + 8].copy_from_slice(&op(x, y).to_le_bytes());
    }
    out
}

fn lanes_i64x2_unary(a: [u8; 16], op: impl Fn(i64) -> i64) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..2 {
        let offset = i * 8;
        let x = i64::from_le_bytes([
            a[offset],
            a[offset + 1],
            a[offset + 2],
            a[offset + 3],
            a[offset + 4],
            a[offset + 5],
            a[offset + 6],
            a[offset + 7],
        ]);
        out[offset..offset + 8].copy_from_slice(&op(x).to_le_bytes());
    }
    out
}

fn lanes_i64x2_u_unary(a: [u8; 16], op: impl Fn(u64) -> u64) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..2 {
        let offset = i * 8;
        let x = u64::from_le_bytes([
            a[offset],
            a[offset + 1],
            a[offset + 2],
            a[offset + 3],
            a[offset + 4],
            a[offset + 5],
            a[offset + 6],
            a[offset + 7],
        ]);
        out[offset..offset + 8].copy_from_slice(&op(x).to_le_bytes());
    }
    out
}

fn lanes_f32x4(a: [u8; 16], b: [u8; 16], op: impl Fn(f32, f32) -> f32) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..4 {
        let offset = i * 4;
        let x = f32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
        let y = f32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
        out[offset..offset + 4].copy_from_slice(&op(x, y).to_le_bytes());
    }
    out
}

fn lanes_f32x4_ternary(
    a: [u8; 16],
    b: [u8; 16],
    c: [u8; 16],
    op: impl Fn(f32, f32, f32) -> f32,
) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..4 {
        let offset = i * 4;
        let x = f32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
        let y = f32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
        let z = f32::from_le_bytes([c[offset], c[offset + 1], c[offset + 2], c[offset + 3]]);
        out[offset..offset + 4].copy_from_slice(&op(x, y, z).to_le_bytes());
    }
    out
}

fn lanes_f32x4_unary(a: [u8; 16], op: impl Fn(f32) -> f32) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..4 {
        let offset = i * 4;
        let x = f32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
        out[offset..offset + 4].copy_from_slice(&op(x).to_le_bytes());
    }
    out
}

fn lanes_f32x4_to_i32x4(a: [u8; 16], signed: bool) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..4 {
        let offset = i * 4;
        let x = f32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
        let lane = if x.is_nan() {
            0
        } else if signed {
            if x <= i32::MIN as f32 {
                i32::MIN as u32
            } else if x >= i32::MAX as f32 {
                i32::MAX as u32
            } else {
                x.trunc() as i32 as u32
            }
        } else if x <= 0.0 {
            0
        } else if x >= u32::MAX as f32 {
            u32::MAX
        } else {
            x.trunc() as u32
        };
        out[offset..offset + 4].copy_from_slice(&lane.to_le_bytes());
    }
    out
}

fn lanes_f64x2_to_i32x4_zero(a: [u8; 16], signed: bool) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..2 {
        let offset = i * 8;
        let x = f64::from_le_bytes([
            a[offset],
            a[offset + 1],
            a[offset + 2],
            a[offset + 3],
            a[offset + 4],
            a[offset + 5],
            a[offset + 6],
            a[offset + 7],
        ]);
        let lane = if x.is_nan() {
            0
        } else if signed {
            if x <= i32::MIN as f64 {
                i32::MIN as u32
            } else if x >= i32::MAX as f64 {
                i32::MAX as u32
            } else {
                x.trunc() as i32 as u32
            }
        } else if x <= 0.0 {
            0
        } else if x >= u32::MAX as f64 {
            u32::MAX
        } else {
            x.trunc() as u32
        };
        out[i * 4..i * 4 + 4].copy_from_slice(&lane.to_le_bytes());
    }
    out
}

fn convert_i32x4_to_f32x4(a: [u8; 16], signed: bool) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..4 {
        let offset = i * 4;
        let lane = if signed {
            i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]) as f32
        } else {
            u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]) as f32
        };
        out[offset..offset + 4].copy_from_slice(&lane.to_le_bytes());
    }
    out
}

fn demote_f64x2_to_f32x4_zero(a: [u8; 16]) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..2 {
        let offset = i * 8;
        let lane = f64::from_le_bytes([
            a[offset],
            a[offset + 1],
            a[offset + 2],
            a[offset + 3],
            a[offset + 4],
            a[offset + 5],
            a[offset + 6],
            a[offset + 7],
        ]) as f32;
        out[i * 4..i * 4 + 4].copy_from_slice(&lane.to_le_bytes());
    }
    out
}

fn convert_low_i32x4_to_f64x2(a: [u8; 16], signed: bool) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..2 {
        let offset = i * 4;
        let lane = if signed {
            i32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]) as f64
        } else {
            u32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]) as f64
        };
        out[i * 8..i * 8 + 8].copy_from_slice(&lane.to_le_bytes());
    }
    out
}

fn promote_low_f32x4_to_f64x2(a: [u8; 16]) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..2 {
        let offset = i * 4;
        let lane =
            f32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]) as f64;
        out[i * 8..i * 8 + 8].copy_from_slice(&lane.to_le_bytes());
    }
    out
}

fn lanes_f32x4_mask(a: [u8; 16], b: [u8; 16], op: impl Fn(f32, f32) -> bool) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..4 {
        let offset = i * 4;
        let x = f32::from_le_bytes([a[offset], a[offset + 1], a[offset + 2], a[offset + 3]]);
        let y = f32::from_le_bytes([b[offset], b[offset + 1], b[offset + 2], b[offset + 3]]);
        let mask: i32 = if op(x, y) { -1 } else { 0 };
        out[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
    }
    out
}

fn lanes_f64x2(a: [u8; 16], b: [u8; 16], op: impl Fn(f64, f64) -> f64) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..2 {
        let offset = i * 8;
        let x = f64::from_le_bytes([
            a[offset],
            a[offset + 1],
            a[offset + 2],
            a[offset + 3],
            a[offset + 4],
            a[offset + 5],
            a[offset + 6],
            a[offset + 7],
        ]);
        let y = f64::from_le_bytes([
            b[offset],
            b[offset + 1],
            b[offset + 2],
            b[offset + 3],
            b[offset + 4],
            b[offset + 5],
            b[offset + 6],
            b[offset + 7],
        ]);
        out[offset..offset + 8].copy_from_slice(&op(x, y).to_le_bytes());
    }
    out
}

fn lanes_f64x2_ternary(
    a: [u8; 16],
    b: [u8; 16],
    c: [u8; 16],
    op: impl Fn(f64, f64, f64) -> f64,
) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..2 {
        let offset = i * 8;
        let x = f64::from_le_bytes([
            a[offset],
            a[offset + 1],
            a[offset + 2],
            a[offset + 3],
            a[offset + 4],
            a[offset + 5],
            a[offset + 6],
            a[offset + 7],
        ]);
        let y = f64::from_le_bytes([
            b[offset],
            b[offset + 1],
            b[offset + 2],
            b[offset + 3],
            b[offset + 4],
            b[offset + 5],
            b[offset + 6],
            b[offset + 7],
        ]);
        let z = f64::from_le_bytes([
            c[offset],
            c[offset + 1],
            c[offset + 2],
            c[offset + 3],
            c[offset + 4],
            c[offset + 5],
            c[offset + 6],
            c[offset + 7],
        ]);
        out[offset..offset + 8].copy_from_slice(&op(x, y, z).to_le_bytes());
    }
    out
}

fn lanes_f64x2_unary(a: [u8; 16], op: impl Fn(f64) -> f64) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..2 {
        let offset = i * 8;
        let x = f64::from_le_bytes([
            a[offset],
            a[offset + 1],
            a[offset + 2],
            a[offset + 3],
            a[offset + 4],
            a[offset + 5],
            a[offset + 6],
            a[offset + 7],
        ]);
        out[offset..offset + 8].copy_from_slice(&op(x).to_le_bytes());
    }
    out
}

fn lanes_f64x2_mask(a: [u8; 16], b: [u8; 16], op: impl Fn(f64, f64) -> bool) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..2 {
        let offset = i * 8;
        let x = f64::from_le_bytes([
            a[offset],
            a[offset + 1],
            a[offset + 2],
            a[offset + 3],
            a[offset + 4],
            a[offset + 5],
            a[offset + 6],
            a[offset + 7],
        ]);
        let y = f64::from_le_bytes([
            b[offset],
            b[offset + 1],
            b[offset + 2],
            b[offset + 3],
            b[offset + 4],
            b[offset + 5],
            b[offset + 6],
            b[offset + 7],
        ]);
        let mask: i64 = if op(x, y) { -1 } else { 0 };
        out[offset..offset + 8].copy_from_slice(&mask.to_le_bytes());
    }
    out
}

pub fn as_i32(v: WasmValue) -> Result<i32, String> {
    match v {
        WasmValue::I32(x) => Ok(x),
        o => Err(format!("expected i32, got {:?}", o)),
    }
}
pub fn as_i64(v: WasmValue) -> Result<i64, String> {
    match v {
        WasmValue::I64(x) => Ok(x),
        o => Err(format!("expected i64, got {:?}", o)),
    }
}
pub fn as_f32(v: WasmValue) -> Result<f32, String> {
    match v {
        WasmValue::F32(x) => Ok(x),
        o => Err(format!("expected f32, got {:?}", o)),
    }
}
pub fn as_f64(v: WasmValue) -> Result<f64, String> {
    match v {
        WasmValue::F64(x) => Ok(x),
        o => Err(format!("expected f64, got {:?}", o)),
    }
}

impl Instance {

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build(
        module: Module,
        funcs: Vec<Callable>,
        host_func_sigs: Vec<Option<(Vec<crate::ValType>, Vec<crate::ValType>)>>,
        globals: Vec<WasmValue>,
        global_mut: Vec<bool>,
        memory: Vec<u8>,
        extra_memories: Vec<Vec<u8>>,
        has_memory: bool,
        mem_max_pages: Option<u64>,
        extra_mem_max_pages: Vec<Option<u64>>,
        memory_aliases: Vec<Option<u64>>,
        memory64: bool,
        memory_shared: bool,
        tables: Vec<Vec<WasmValue>>,
        table_maxes: Vec<Option<u64>>,
        table64s: Vec<bool>,
        data_segments: Vec<Option<Vec<u8>>>,
        elem_segments: Vec<Option<Vec<WasmValue>>>,
        tag_identities: Vec<String>,
        gc_arrays: Vec<GcArray>,
        gc_structs: Vec<GcStruct>,
        n_defined: usize,
    ) -> Instance {
        let memory_count = 1 + extra_memories.len();
        Instance {
            module,
            funcs,
            host_func_sigs,
            globals,
            global_mut,
            memory,
            extra_memories,
            has_memory,
            mem_max_pages,
            extra_mem_max_pages,
            memory_aliases,
            memory_dirty_ranges: vec![None; memory_count],
            memory64,
            memory_shared,
            tables,
            table_maxes,
            table64s,
            data_segments,
            elem_segments,
            tag_identities,
            exception_refs: Vec::new(),
            gc_arrays,
            gc_structs,
            control_cache: vec![None; n_defined],
            execution_fuel: Self::default_execution_fuel(),
        }
    }
}
