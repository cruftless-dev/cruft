
mod interp;
mod leb;
mod numeric;
mod parser;

use std::cell::RefCell;

use parser::{ElementItem, ElementMode, ExportKind, ImportKind, Instr};

pub use parser::Module;

thread_local! {
    static LAST_PARTIAL_INSTANCE: RefCell<Option<Instance>> = const { RefCell::new(None) };
}

const DEFAULT_MAX_MEMORY_PAGES: usize = 8192;

fn max_memory_pages() -> usize {
    std::env::var("CRUFT_WASM_MAX_MEMORY_PAGES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_MAX_MEMORY_PAGES)
}

pub fn take_last_partial_instance() -> Option<Instance> {
    LAST_PARTIAL_INSTANCE.with(|slot| slot.borrow_mut().take())
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ValType {
    I32,
    I64,
    F32,
    F64,
    V128,
    AnyRef,
    NonNullAnyRef,
    EqRef,
    NonNullEqRef,
    FuncRef,
    NonNullFuncRef,
    ExternRef,
    NonNullExternRef,
    StructRef,
    NonNullStructRef,
    ArrayRef,
    NonNullArrayRef,
    I31Ref,
    NonNullI31Ref,
    TypeRef(u32),
    NonNullTypeRef(u32),
    NullRef,
    NullFuncRef,
    NullExternRef,
    Unknown,
}

impl ValType {
    fn from_parser(t: parser::ValType) -> ValType {
        match t {
            parser::ValType::I32 => ValType::I32,
            parser::ValType::I64 => ValType::I64,
            parser::ValType::F32 => ValType::F32,
            parser::ValType::F64 => ValType::F64,
            parser::ValType::V128 => ValType::V128,
            parser::ValType::AnyRef => ValType::AnyRef,
            parser::ValType::NonNullAnyRef => ValType::NonNullAnyRef,
            parser::ValType::EqRef => ValType::EqRef,
            parser::ValType::NonNullEqRef => ValType::NonNullEqRef,
            parser::ValType::FuncRef => ValType::FuncRef,
            parser::ValType::NonNullFuncRef => ValType::NonNullFuncRef,
            parser::ValType::ExternRef => ValType::ExternRef,
            parser::ValType::NonNullExternRef => ValType::NonNullExternRef,
            parser::ValType::StructRef => ValType::StructRef,
            parser::ValType::NonNullStructRef => ValType::NonNullStructRef,
            parser::ValType::ArrayRef => ValType::ArrayRef,
            parser::ValType::NonNullArrayRef => ValType::NonNullArrayRef,
            parser::ValType::I31Ref => ValType::I31Ref,
            parser::ValType::NonNullI31Ref => ValType::NonNullI31Ref,
            parser::ValType::TypeRef(idx) => ValType::TypeRef(idx),
            parser::ValType::NonNullTypeRef(idx) => ValType::NonNullTypeRef(idx),
            parser::ValType::NullRef => ValType::NullRef,
            parser::ValType::NullFuncRef => ValType::NullFuncRef,
            parser::ValType::NullExternRef => ValType::NullExternRef,
            parser::ValType::Unknown => ValType::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WasmValue {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    V128([u8; 16]),
    RefNull,
    FuncRef(u32),
    ArrayRef(u32),
    StructRef(u32),
    I31Ref(i32),
    ExnRef(u32),

    ExternRef(u32),
    ExternI31Ref(i32),
    ExternStructRef(u32),
    ExternArrayRef(u32),
}

impl WasmValue {

    pub fn as_f64(&self) -> f64 {
        match *self {
            WasmValue::I32(v) => v as f64,
            WasmValue::I64(v) => v as f64,
            WasmValue::F32(v) => v as f64,
            WasmValue::F64(v) => v,
            WasmValue::V128(_) => 0.0,
            WasmValue::RefNull => 0.0,
            WasmValue::FuncRef(_) => 0.0,
            WasmValue::ArrayRef(_) => 0.0,
            WasmValue::StructRef(_) => 0.0,
            WasmValue::I31Ref(v) => v as f64,
            WasmValue::ExnRef(_) => 0.0,
            WasmValue::ExternRef(_) => 0.0,
            WasmValue::ExternI31Ref(v) => v as f64,
            WasmValue::ExternStructRef(_) => 0.0,
            WasmValue::ExternArrayRef(_) => 0.0,
        }
    }

    pub fn val_type(&self) -> ValType {
        match *self {
            WasmValue::I32(_) => ValType::I32,
            WasmValue::I64(_) => ValType::I64,
            WasmValue::F32(_) => ValType::F32,
            WasmValue::F64(_) => ValType::F64,
            WasmValue::V128(_) => ValType::V128,
            WasmValue::RefNull => ValType::ExternRef,
            WasmValue::FuncRef(_) => ValType::FuncRef,
            WasmValue::ArrayRef(_) => ValType::ArrayRef,
            WasmValue::StructRef(_) => ValType::StructRef,
            WasmValue::I31Ref(_) => ValType::I31Ref,
            WasmValue::ExnRef(_) => ValType::ExternRef,
            WasmValue::ExternRef(_) => ValType::ExternRef,
            WasmValue::ExternI31Ref(_) => ValType::ExternRef,
            WasmValue::ExternStructRef(_) => ValType::ExternRef,
            WasmValue::ExternArrayRef(_) => ValType::ExternRef,
        }
    }
}

pub fn parse_module(bytes: &[u8]) -> Result<Module, String> {
    parser::parse_module(bytes)
}

pub trait HostContext {

    fn mem_size(&self) -> usize;

    fn mem_read(&self, offset: usize, len: usize) -> Option<Vec<u8>>;

    fn mem_write(&mut self, offset: usize, data: &[u8]) -> bool;
}

pub type HostFn =
    Box<dyn FnMut(&mut dyn HostContext, &[WasmValue]) -> Result<Vec<WasmValue>, String>>;

#[derive(Clone, Debug)]
pub struct FuncImportDecl {
    pub module: String,
    pub name: String,
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
    pub type_final: bool,
    pub type_shape: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleImportDescriptor {
    pub module: String,
    pub name: String,
    pub kind: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleExportDescriptor {
    pub name: String,
    pub kind: &'static str,
}

pub type ExportFuncSpec = (
    String,
    u32,
    Vec<ValType>,
    Vec<ValType>,
    bool,
    bool,
    String,
    Vec<String>,
);

fn import_kind_name(kind: &ImportKind) -> &'static str {
    match kind {
        ImportKind::Func(_) => "function",
        ImportKind::Table(_) => "table",
        ImportKind::Memory(_) => "memory",
        ImportKind::Global { .. } => "global",
        ImportKind::Tag(_) => "tag",
    }
}

fn export_kind_name(kind: ExportKind) -> &'static str {
    match kind {
        ExportKind::Func => "function",
        ExportKind::Table => "table",
        ExportKind::Memory => "memory",
        ExportKind::Global => "global",
        ExportKind::Tag => "tag",
    }
}

pub fn module_import_descriptors(module: &Module) -> Vec<ModuleImportDescriptor> {
    module
        .imports
        .iter()
        .map(|imp| ModuleImportDescriptor {
            module: imp.module.clone(),
            name: imp.name.clone(),
            kind: import_kind_name(&imp.kind),
        })
        .collect()
}

pub fn module_export_descriptors(module: &Module) -> Vec<ModuleExportDescriptor> {
    module
        .exports
        .iter()
        .map(|ex| ModuleExportDescriptor {
            name: ex.name.clone(),
            kind: export_kind_name(ex.kind),
        })
        .collect()
}

pub fn module_export_func_specs(module: &Module) -> Vec<ExportFuncSpec> {
    module
        .exports
        .iter()
        .filter(|e| e.kind == ExportKind::Func)
        .filter_map(|e| {
            let type_idx = func_type_index_for_module_export(module, e.index as usize)?;
            let ft = module.types.get(type_idx as usize)?;
            let params: Vec<ValType> = ft.params.iter().map(|t| ValType::from_parser(*t)).collect();
            let results: Vec<ValType> = ft
                .results
                .iter()
                .map(|t| ValType::from_parser(*t))
                .collect();
            let type_final = module
                .type_is_final
                .get(type_idx as usize)
                .copied()
                .unwrap_or(false);
            let type_shape = type_shape_key(module, type_idx);
            let type_shapes = type_shape_closure_keys(module, type_idx);
            Some((
                e.name.clone(),
                e.index,
                params,
                results,
                module_func_may_write_memory(module, e.index as usize),
                type_final,
                type_shape,
                type_shapes,
            ))
        })
        .collect()
}

fn func_type_index_for_module_export(module: &Module, overall: usize) -> Option<u32> {
    let imported = module.imported_func_count;
    if overall < imported {
        let mut seen = 0usize;
        for imp in &module.imports {
            if let ImportKind::Func(type_idx) = &imp.kind {
                if seen == overall {
                    return Some(*type_idx);
                }
                seen += 1;
            }
        }
        None
    } else {
        module.func_types.get(overall - imported).copied()
    }
}

fn module_func_may_write_memory(module: &Module, overall: usize) -> bool {
    let imported = module.imported_func_count;
    if overall < imported {
        return true;
    }
    let Some(code) = module.code.get(overall - imported) else {
        return true;
    };
    code.body.iter().any(instr_may_write_memory)
}

pub fn func_imports(module: &Module) -> Vec<FuncImportDecl> {
    let mut out = Vec::new();
    for imp in &module.imports {
        if let ImportKind::Func(type_idx) = &imp.kind {
            let ft = match module.types.get(*type_idx as usize) {
                Some(ft) => ft,
                None => continue,
            };
            let params: Vec<ValType> = ft.params.iter().map(|t| ValType::from_parser(*t)).collect();
            let results: Vec<ValType> = ft
                .results
                .iter()
                .map(|t| ValType::from_parser(*t))
                .collect();
            out.push(FuncImportDecl {
                module: imp.module.clone(),
                name: imp.name.clone(),
                params,
                results,
                type_final: module
                    .type_is_final
                    .get(*type_idx as usize)
                    .copied()
                    .unwrap_or(false),
                type_shape: type_shape_key(module, *type_idx),
            });
        }
    }
    out
}

fn type_shape_key(module: &Module, type_idx: u32) -> String {
    type_shape_key_inner(module, type_idx, &mut Vec::new())
}

fn type_shape_closure_keys(module: &Module, type_idx: u32) -> Vec<String> {
    let mut out = Vec::new();
    type_shape_closure_keys_inner(module, type_idx, &mut Vec::new(), &mut out);
    out
}

fn type_shape_closure_keys_inner(
    module: &Module,
    type_idx: u32,
    seen: &mut Vec<u32>,
    out: &mut Vec<String>,
) {
    if seen.contains(&type_idx) {
        return;
    }
    seen.push(type_idx);
    let shape = type_shape_key(module, type_idx);
    if !out.contains(&shape) {
        out.push(shape);
    }
    if let Some(supertypes) = module.type_supertypes.get(type_idx as usize) {
        for super_idx in supertypes {
            type_shape_closure_keys_inner(module, *super_idx, seen, out);
        }
    }
    seen.pop();
}

fn type_shape_key_inner(module: &Module, type_idx: u32, seen: &mut Vec<u32>) -> String {
    if let Some(pos) = seen.iter().position(|idx| *idx == type_idx) {
        return format!("cycle:{pos}");
    }
    seen.push(type_idx);
    let group = module
        .type_rec_groups
        .get(type_idx as usize)
        .copied()
        .unwrap_or(type_idx);
    let members = type_group_members(module, group);
    let selected = members
        .iter()
        .position(|idx| *idx == type_idx)
        .map(|idx| idx.to_string())
        .unwrap_or_else(|| "?".to_string());
    let member_shapes = members
        .iter()
        .map(|idx| type_member_shape_key(module, *idx, &members, seen))
        .collect::<Vec<_>>()
        .join("|");
    seen.pop();
    format!("group[{selected}]{{{member_shapes}}}")
}

fn type_group_members(module: &Module, group: u32) -> Vec<u32> {
    module
        .type_rec_groups
        .iter()
        .enumerate()
        .filter_map(|(idx, candidate)| (*candidate == group).then_some(idx as u32))
        .collect()
}

fn type_member_shape_key(
    module: &Module,
    type_idx: u32,
    members: &[u32],
    seen: &mut Vec<u32>,
) -> String {
    let final_key = if module
        .type_is_final
        .get(type_idx as usize)
        .copied()
        .unwrap_or(false)
    {
        "final"
    } else {
        "open"
    };
    let supers = module
        .type_supertypes
        .get(type_idx as usize)
        .map(|items| {
            items
                .iter()
                .map(|idx| type_ref_shape_key(module, *idx, members, seen))
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    if module
        .type_is_func
        .get(type_idx as usize)
        .copied()
        .unwrap_or(false)
    {
        let Some(func) = module.types.get(type_idx as usize) else {
            return format!("func:{final_key}:missing");
        };
        let params = val_type_list_shape_key(module, &func.params, members, seen);
        let results = val_type_list_shape_key(module, &func.results, members, seen);
        return format!("func:{final_key}:sup[{supers}]:({params})->({results})");
    }
    if let Some(Some(st)) = module.struct_types.get(type_idx as usize) {
        let fields = st
            .fields
            .iter()
            .map(|field| {
                format!(
                    "{}:{}:{}",
                    if field.mutable { "mut" } else { "const" },
                    field
                        .packed_bits
                        .map(|bits| bits.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    val_type_shape_key(module, field.ty, members, seen)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        return format!("struct:{final_key}:sup[{supers}]:{{{fields}}}");
    }
    if let Some(Some(array)) = module.array_types.get(type_idx as usize) {
        return format!(
            "array:{final_key}:sup[{supers}]:{}:{}:{}",
            if array.mutable { "mut" } else { "const" },
            array
                .packed_bits
                .map(|bits| bits.to_string())
                .unwrap_or_else(|| "-".to_string()),
            val_type_shape_key(module, array.element, members, seen)
        );
    }
    format!("unknown:{final_key}:sup[{supers}]")
}

fn val_type_list_shape_key(
    module: &Module,
    types: &[parser::ValType],
    members: &[u32],
    seen: &mut Vec<u32>,
) -> String {
    types
        .iter()
        .map(|ty| val_type_shape_key(module, *ty, members, seen))
        .collect::<Vec<_>>()
        .join(",")
}

fn val_type_shape_key(
    module: &Module,
    ty: parser::ValType,
    members: &[u32],
    seen: &mut Vec<u32>,
) -> String {
    match ty {
        parser::ValType::TypeRef(idx) => {
            format!("ref:{}", type_ref_shape_key(module, idx, members, seen))
        }
        parser::ValType::NonNullTypeRef(idx) => {
            format!("ref!:{}", type_ref_shape_key(module, idx, members, seen))
        }
        other => format!("{other:?}"),
    }
}

fn type_ref_shape_key(module: &Module, idx: u32, members: &[u32], seen: &mut Vec<u32>) -> String {
    if let Some(pos) = members.iter().position(|member| *member == idx) {
        format!("rec:{pos}")
    } else {
        type_shape_key_inner(module, idx, seen)
    }
}

#[derive(Clone, Debug)]
pub struct GlobalImportDecl {
    pub module: String,
    pub name: String,
    pub ty: ValType,
    pub mutable: bool,
}

pub fn global_imports(module: &Module) -> Vec<GlobalImportDecl> {
    let mut out = Vec::new();
    for imp in &module.imports {
        if let ImportKind::Global { ty, mutable } = imp.kind {
            out.push(GlobalImportDecl {
                module: imp.module.clone(),
                name: imp.name.clone(),
                ty: ValType::from_parser(ty),
                mutable,
            });
        }
    }
    out
}

enum ImportValue {
    Func(HostFn),
    Global(WasmValue),
    GlobalFuncRef {
        params: Vec<ValType>,
        results: Vec<ValType>,
        f: HostFn,
    },
    Memory {
        pages: usize,
        max: Option<u32>,
        shared: bool,
        memory64: bool,
        bytes: Option<Vec<u8>>,
        alias_key: Option<u64>,
    },
    Table {
        elem: ValType,
        len: usize,
        max: Option<u32>,
        table64: bool,
        values: Vec<TableImportValue>,
    },
    Tag {
        identity: Option<String>,
    },
}

pub enum TableImportValue {
    Null,
    Func(WasmValue),
    ExternRef(WasmValue),
    HostFunc {
        params: Vec<ValType>,
        results: Vec<ValType>,
        f: HostFn,
    },
}

#[derive(Clone, Debug)]
pub struct MemoryImportDecl {
    pub module: String,
    pub name: String,
    pub min: u64,
    pub max: Option<u64>,
    pub shared: bool,
    pub memory64: bool,
}

pub fn memory_imports(module: &Module) -> Vec<MemoryImportDecl> {
    let mut out = Vec::new();
    for imp in &module.imports {
        if let ImportKind::Memory(lim) = imp.kind {
            out.push(MemoryImportDecl {
                module: imp.module.clone(),
                name: imp.name.clone(),
                min: lim.min,
                max: lim.max,
                shared: lim.shared,
                memory64: lim.memory64,
            });
        }
    }
    out
}

#[derive(Clone, Debug)]
pub struct TableImportDecl {
    pub module: String,
    pub name: String,
    pub elem: ValType,
    pub min: u64,
    pub max: Option<u64>,
    pub table64: bool,
}

pub fn table_imports(module: &Module) -> Vec<TableImportDecl> {
    let mut out = Vec::new();
    for imp in &module.imports {
        if let ImportKind::Table(ty) = imp.kind {
            out.push(TableImportDecl {
                module: imp.module.clone(),
                name: imp.name.clone(),
                elem: ValType::from_parser(ty.elem),
                min: ty.limits.min,
                max: ty.limits.max,
                table64: ty.limits.memory64,
            });
        }
    }
    out
}

#[derive(Clone, Debug)]
pub struct TagImportDecl {
    pub module: String,
    pub name: String,
    pub params: Vec<ValType>,
    pub type_shape: String,
}

pub fn tag_imports(module: &Module) -> Vec<TagImportDecl> {
    let mut out = Vec::new();
    for imp in &module.imports {
        if let ImportKind::Tag(type_idx) = imp.kind {
            let params = module
                .types
                .get(type_idx as usize)
                .map(|ft| {
                    ft.params
                        .iter()
                        .map(|ty| ValType::from_parser(*ty))
                        .collect()
                })
                .unwrap_or_default();
            out.push(TagImportDecl {
                module: imp.module.clone(),
                name: imp.name.clone(),
                params,
                type_shape: type_shape_key(module, type_idx),
            });
        }
    }
    out
}

pub struct Imports {
    entries: Vec<((String, String), ImportValue)>,
}

impl Default for Imports {
    fn default() -> Self {
        Imports::new()
    }
}

impl Imports {
    pub fn new() -> Self {
        Imports {
            entries: Vec::new(),
        }
    }

    pub fn func(&mut self, module: &str, name: &str, f: HostFn) {
        self.entries
            .push(((module.to_string(), name.to_string()), ImportValue::Func(f)));
    }

    pub fn global(&mut self, module: &str, name: &str, value: WasmValue) {
        self.entries.push((
            (module.to_string(), name.to_string()),
            ImportValue::Global(value),
        ));
    }

    pub fn global_func_ref(
        &mut self,
        module: &str,
        name: &str,
        params: Vec<ValType>,
        results: Vec<ValType>,
        f: HostFn,
    ) {
        self.entries.push((
            (module.to_string(), name.to_string()),
            ImportValue::GlobalFuncRef { params, results, f },
        ));
    }

    pub fn memory(&mut self, module: &str, name: &str, pages: usize, max: Option<u32>) {
        self.memory_with_shared(module, name, pages, max, false);
    }

    pub fn memory_with_shared(
        &mut self,
        module: &str,
        name: &str,
        pages: usize,
        max: Option<u32>,
        shared: bool,
    ) {
        self.memory_with_shared_bytes(module, name, pages, max, shared, None);
    }

    pub fn memory_with_shared_bytes(
        &mut self,
        module: &str,
        name: &str,
        pages: usize,
        max: Option<u32>,
        shared: bool,
        bytes: Option<Vec<u8>>,
    ) {
        self.memory_with_shared_bytes_address64(module, name, pages, max, shared, false, bytes);
    }

    pub fn memory_with_shared_bytes_address64(
        &mut self,
        module: &str,
        name: &str,
        pages: usize,
        max: Option<u32>,
        shared: bool,
        memory64: bool,
        bytes: Option<Vec<u8>>,
    ) {
        self.memory_with_shared_bytes_address64_alias(
            module, name, pages, max, shared, memory64, bytes, None,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn memory_with_shared_bytes_address64_alias(
        &mut self,
        module: &str,
        name: &str,
        pages: usize,
        max: Option<u32>,
        shared: bool,
        memory64: bool,
        bytes: Option<Vec<u8>>,
        alias_key: Option<u64>,
    ) {
        self.entries.push((
            (module.to_string(), name.to_string()),
            ImportValue::Memory {
                pages,
                max,
                shared,
                memory64,
                bytes,
                alias_key,
            },
        ));
    }

    pub fn table(&mut self, module: &str, name: &str, elem: ValType, len: usize, max: Option<u32>) {
        self.table_with_values(module, name, elem, len, max, Vec::new());
    }

    pub fn table_with_values(
        &mut self,
        module: &str,
        name: &str,
        elem: ValType,
        len: usize,
        max: Option<u32>,
        values: Vec<TableImportValue>,
    ) {
        self.table_with_values_address64(module, name, elem, len, max, false, values);
    }

    pub fn table_with_values_address64(
        &mut self,
        module: &str,
        name: &str,
        elem: ValType,
        len: usize,
        max: Option<u32>,
        table64: bool,
        values: Vec<TableImportValue>,
    ) {
        self.entries.push((
            (module.to_string(), name.to_string()),
            ImportValue::Table {
                elem,
                len,
                max,
                table64,
                values,
            },
        ));
    }

    pub fn tag(&mut self, module: &str, name: &str) {
        self.tag_with_identity(module, name, None);
    }

    pub fn tag_with_identity(&mut self, module: &str, name: &str, identity: Option<String>) {
        self.entries.push((
            (module.to_string(), name.to_string()),
            ImportValue::Tag { identity },
        ));
    }

    fn take(&mut self, module: &str, name: &str) -> Option<ImportValue> {
        if let Some(pos) = self
            .entries
            .iter()
            .position(|((m, n), _)| m == module && n == name)
        {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }

    fn memory_import(&self, module: &str, name: &str) -> Option<ImportValue> {
        self.entries.iter().find_map(|((m, n), value)| {
            if m != module || n != name {
                return None;
            }
            match value {
                ImportValue::Memory {
                    pages,
                    max,
                    shared,
                    memory64,
                    bytes,
                    alias_key,
                } => Some(ImportValue::Memory {
                    pages: *pages,
                    max: *max,
                    shared: *shared,
                    memory64: *memory64,
                    bytes: bytes.clone(),
                    alias_key: *alias_key,
                }),
                _ => None,
            }
        })
    }
}

pub use interp::Instance;
use interp::{Callable, GcArray, GcStruct};

const PAGE: usize = 65536;

fn eval_const_expr(
    instrs: &[Instr],
    globals: &[WasmValue],
    module: &Module,
    gc_arrays: &mut Vec<GcArray>,
    gc_structs: &mut Vec<GcStruct>,
) -> Result<WasmValue, String> {
    let mut stack = Vec::new();
    for ins in instrs {
        match ins {
            Instr::I32Const(v) => stack.push(WasmValue::I32(*v)),
            Instr::I64Const(v) => stack.push(WasmValue::I64(*v)),
            Instr::F32Const(v) => stack.push(WasmValue::F32(*v)),
            Instr::F64Const(v) => stack.push(WasmValue::F64(*v)),
            Instr::V128Const(bytes) => stack.push(WasmValue::V128(*bytes)),
            Instr::RefNull(_) => stack.push(WasmValue::RefNull),
            Instr::RefFunc(idx) => stack.push(WasmValue::FuncRef(*idx)),
            Instr::RefI31 => {
                let value = match stack.pop() {
                    Some(WasmValue::I32(v)) => v,
                    Some(other) => {
                        return Err(format!(
                            "const expr: expected i32 for ref.i31, got {:?}",
                            other
                        ))
                    }
                    None => return Err("const expr: ref.i31 operand underflow".to_string()),
                };
                stack.push(WasmValue::I31Ref(value & 0x7fff_ffff));
            }
            Instr::GlobalGet(i) => {
                let value = globals
                    .get(*i as usize)
                    .copied()
                    .ok_or_else(|| "const expr: global.get out of range".to_string())?;
                stack.push(value);
            }
            Instr::ArrayNew(type_idx) => {
                let array = module
                    .array_types
                    .get(*type_idx as usize)
                    .and_then(|ty| *ty)
                    .ok_or_else(|| {
                        format!("const expr: type index {} is not an array type", type_idx)
                    })?;
                let len = match stack.pop() {
                    Some(WasmValue::I32(v)) => v as u32 as usize,
                    Some(other) => {
                        return Err(format!(
                            "const expr: expected array length i32, got {:?}",
                            other
                        ))
                    }
                    None => return Err("const expr: array.new length underflow".to_string()),
                };
                let value = stack
                    .pop()
                    .ok_or_else(|| "const expr: array.new element underflow".to_string())?;
                let idx = gc_arrays.len() as u32;
                gc_arrays.push(GcArray {
                    type_idx: *type_idx,
                    elements: vec![value; len],
                    mutable: array.mutable,
                });
                stack.push(WasmValue::ArrayRef(idx));
            }
            Instr::ArrayNewDefault(type_idx) => {
                let array = module
                    .array_types
                    .get(*type_idx as usize)
                    .and_then(|ty| *ty)
                    .ok_or_else(|| {
                        format!("const expr: type index {} is not an array type", type_idx)
                    })?;
                let len = match stack.pop() {
                    Some(WasmValue::I32(v)) => v as u32 as usize,
                    Some(other) => {
                        return Err(format!(
                            "const expr: expected array length i32, got {:?}",
                            other
                        ))
                    }
                    None => {
                        return Err("const expr: array.new_default length underflow".to_string())
                    }
                };
                let idx = gc_arrays.len() as u32;
                gc_arrays.push(GcArray {
                    type_idx: *type_idx,
                    elements: vec![interp::zero_of(array.element); len],
                    mutable: array.mutable,
                });
                stack.push(WasmValue::ArrayRef(idx));
            }
            Instr::ArrayNewFixed(type_idx, count) => {
                let array = module
                    .array_types
                    .get(*type_idx as usize)
                    .and_then(|ty| *ty)
                    .ok_or_else(|| {
                        format!("const expr: type index {} is not an array type", type_idx)
                    })?;
                let count = *count as usize;
                if stack.len() < count {
                    return Err("const expr: array.new_fixed element underflow".to_string());
                }
                let elements = stack.split_off(stack.len() - count);
                let idx = gc_arrays.len() as u32;
                gc_arrays.push(GcArray {
                    type_idx: *type_idx,
                    elements,
                    mutable: array.mutable,
                });
                stack.push(WasmValue::ArrayRef(idx));
            }
            Instr::StructNew(type_idx) => {
                let ty = module
                    .struct_types
                    .get(*type_idx as usize)
                    .and_then(|ty| ty.as_ref())
                    .ok_or_else(|| {
                        format!("const expr: type index {} is not a struct type", type_idx)
                    })?;
                let field_count = ty.fields.len();
                if stack.len() < field_count {
                    return Err("const expr: struct.new field underflow".to_string());
                }
                let fields = stack
                    .split_off(stack.len() - field_count)
                    .into_iter()
                    .zip(ty.fields.iter())
                    .map(|(value, field)| interp::normalize_packed_value(field.packed_bits, value))
                    .collect();
                let idx = gc_structs.len() as u32;
                gc_structs.push(GcStruct {
                    type_idx: *type_idx,
                    fields,
                    mutable: ty.fields.iter().map(|field| field.mutable).collect(),
                });
                stack.push(WasmValue::StructRef(idx));
            }
            Instr::StructNewDefault(type_idx) => {
                let ty = module
                    .struct_types
                    .get(*type_idx as usize)
                    .and_then(|ty| ty.as_ref())
                    .ok_or_else(|| {
                        format!("const expr: type index {} is not a struct type", type_idx)
                    })?;
                let idx = gc_structs.len() as u32;
                gc_structs.push(GcStruct {
                    type_idx: *type_idx,
                    fields: ty
                        .fields
                        .iter()
                        .map(|field| interp::zero_of(field.ty))
                        .collect(),
                    mutable: ty.fields.iter().map(|field| field.mutable).collect(),
                });
                stack.push(WasmValue::StructRef(idx));
            }
            Instr::AnyConvertExtern => {
                let value = stack.pop().ok_or_else(|| {
                    "const expr: any.convert_extern operand underflow".to_string()
                })?;
                stack.push(value);
            }
            Instr::ExternConvertAny => {
                let value = stack.pop().ok_or_else(|| {
                    "const expr: extern.convert_any operand underflow".to_string()
                })?;
                stack.push(match value {
                    WasmValue::RefNull => WasmValue::RefNull,
                    WasmValue::ExternRef(id) => WasmValue::ExternRef(id),
                    WasmValue::I31Ref(value) => WasmValue::ExternI31Ref(value),
                    WasmValue::StructRef(idx) => WasmValue::ExternStructRef(idx),
                    WasmValue::ArrayRef(idx) => WasmValue::ExternArrayRef(idx),
                    _ => WasmValue::ExternRef(0),
                });
            }
            Instr::Num(op) => numeric::exec_num(*op, &mut stack)?,
            Instr::End => break,
            other => {
                return Err(format!("non-constant init expression: {:?}", other));
            }
        }
    }
    match stack.len() {
        0 => Err("empty constant expression".to_string()),
        1 => Ok(stack[0]),
        _ => Err("constant expression produces multiple values".to_string()),
    }
}

fn const_expr_mem_offset(
    instrs: &[Instr],
    globals: &[WasmValue],
    memory64: bool,
    module: &Module,
    gc_arrays: &mut Vec<GcArray>,
    gc_structs: &mut Vec<GcStruct>,
) -> Result<usize, String> {
    let offset = eval_const_expr(instrs, globals, module, gc_arrays, gc_structs)?;
    let raw = match (memory64, offset) {
        (false, WasmValue::I32(v)) if v >= 0 => v as u64,
        (false, WasmValue::I32(_)) => return Err("negative data segment offset".to_string()),
        (true, WasmValue::I64(v)) if v >= 0 => v as u64,
        (true, WasmValue::I64(_)) => return Err("negative data segment offset".to_string()),
        (false, other) => return Err(format!("expected i32 offset, got {:?}", other)),
        (true, other) => return Err(format!("expected i64 offset, got {:?}", other)),
    };
    usize::try_from(raw).map_err(|_| "data segment offset out of range".to_string())
}

fn const_expr_table_offset(
    instrs: &[Instr],
    globals: &[WasmValue],
    table64: bool,
    module: &Module,
    gc_arrays: &mut Vec<GcArray>,
    gc_structs: &mut Vec<GcStruct>,
) -> Result<usize, String> {
    let offset = eval_const_expr(instrs, globals, module, gc_arrays, gc_structs)?;
    let raw = match (table64, offset) {
        (false, WasmValue::I32(v)) if v >= 0 => v as u64,
        (false, WasmValue::I32(_)) => return Err("negative element segment offset".to_string()),
        (true, WasmValue::I64(v)) if v >= 0 => v as u64,
        (true, WasmValue::I64(_)) => return Err("negative element segment offset".to_string()),
        (false, other) => return Err(format!("expected i32 offset, got {:?}", other)),
        (true, other) => return Err(format!("expected i64 offset, got {:?}", other)),
    };
    usize::try_from(raw).map_err(|_| "element segment offset out of range".to_string())
}

pub fn instantiate(module: &Module, mut imports: Imports) -> Result<Instance, String> {
    LAST_PARTIAL_INSTANCE.with(|slot| {
        slot.borrow_mut().take();
    });

    let owned = clone_module(module);

    let mut funcs: Vec<Callable> = Vec::new();
    let mut imported_globals: Vec<WasmValue> = Vec::new();
    let mut pending_global_func_refs: Vec<(usize, Vec<ValType>, Vec<ValType>, HostFn)> = Vec::new();
    let mut imported_memories: Vec<(usize, Option<u64>, bool, bool, Option<Vec<u8>>, Option<u64>)> =
        Vec::new();
    let mut imported_tables: Vec<(ValType, usize, Option<u32>, Vec<TableImportValue>)> = Vec::new();
    let mut host_func_sigs: Vec<Option<(Vec<ValType>, Vec<ValType>)>> = Vec::new();
    let mut tag_identities: Vec<String> = Vec::new();

    for imp in &owned.imports {
        match &imp.kind {
            ImportKind::Func(_) => {
                let val = imports
                    .take(&imp.module, &imp.name)
                    .ok_or_else(|| format!("missing import: {}.{} (func)", imp.module, imp.name))?;
                match val {
                    ImportValue::Func(f) => {
                        funcs.push(Callable::Host(f));
                        host_func_sigs.push(None);
                    }
                    _ => {
                        return Err(format!(
                            "import {}.{} is not a function",
                            imp.module, imp.name
                        ))
                    }
                }
            }
            ImportKind::Global { .. } => {
                let val = imports.take(&imp.module, &imp.name).ok_or_else(|| {
                    format!("missing import: {}.{} (global)", imp.module, imp.name)
                })?;
                match val {
                    ImportValue::Global(v) => imported_globals.push(v),
                    ImportValue::GlobalFuncRef { params, results, f } => {
                        let global_idx = imported_globals.len();
                        imported_globals.push(WasmValue::RefNull);
                        pending_global_func_refs.push((global_idx, params, results, f));
                    }
                    _ => {
                        return Err(format!(
                            "import {}.{} is not a global",
                            imp.module, imp.name
                        ))
                    }
                }
            }
            ImportKind::Memory(lim) => {
                let val = imports.memory_import(&imp.module, &imp.name);
                match val {
                    Some(ImportValue::Memory {
                        pages,
                        max,
                        shared,
                        memory64,
                        bytes,
                        alias_key,
                    }) => {
                        if memory64 != lim.memory64 {
                            return Err(format!(
                                "import {}.{} memory address type mismatch: declared {}, imported {}",
                                imp.module, imp.name, lim.memory64, memory64
                            ));
                        }
                        if shared != lim.shared {
                            return Err(format!(
                                "import {}.{} memory shared mismatch: declared {}, imported {}",
                                imp.module, imp.name, lim.shared, shared
                            ));
                        }
                        if pages < lim.min as usize {
                            return Err(format!(
                                "import {}.{} memory has {} pages which is smaller than declared initial {}",
                                imp.module, imp.name, pages, lim.min
                            ));
                        }
                        imported_memories.push((
                            pages,
                            max.map(u64::from).or(lim.max),
                            lim.memory64,
                            shared,
                            bytes,
                            alias_key,
                        ));
                    }
                    None => {

                        imported_memories.push((
                            lim.min as usize,
                            lim.max,
                            lim.memory64,
                            lim.shared,
                            None,
                            None,
                        ));
                    }
                    Some(_) => {
                        return Err(format!(
                            "import {}.{} is not a memory",
                            imp.module, imp.name
                        ))
                    }
                }
            }
            ImportKind::Table(ty) => {
                let val = imports.take(&imp.module, &imp.name);
                match val {
                    Some(ImportValue::Table {
                        elem,
                        len,
                        max,
                        table64,
                        values,
                    }) => {
                        if table64 != ty.limits.memory64 {
                            return Err(format!(
                                "import {}.{} table address type mismatch: declared {}, imported {}",
                                imp.module, imp.name, ty.limits.memory64, table64
                            ));
                        }
                        if len < ty.limits.min as usize {
                            return Err(format!(
                                "import {}.{} table has {} elements which is smaller than declared initial {}",
                                imp.module, imp.name, len, ty.limits.min
                            ));
                        }
                        if let Some(declared_max) = ty.limits.max {
                            if max.map(|m| m as u64).unwrap_or(declared_max) > declared_max {
                                return Err(format!(
                                    "import {}.{} table maximum exceeds declared maximum {}",
                                    imp.module, imp.name, declared_max
                                ));
                            }
                        }
                        imported_tables.push((
                            elem,
                            len,
                            max.or(ty.limits.max.map(|m| m as u32)),
                            values,
                        ));
                    }
                    None => {
                        imported_tables.push((
                            ValType::from_parser(ty.elem),
                            ty.limits.min as usize,
                            ty.limits.max.map(|m| m as u32),
                            Vec::new(),
                        ));
                    }
                    Some(_) => {
                        return Err(format!("import {}.{} is not a table", imp.module, imp.name))
                    }
                }
            }
            ImportKind::Tag(_) => match imports.take(&imp.module, &imp.name) {
                Some(ImportValue::Tag { identity }) => {
                    let fallback = format!("import:{}:{}", imp.module, imp.name);
                    tag_identities.push(identity.unwrap_or(fallback));
                }
                None => {
                    return Err(format!("missing import: {}.{} (tag)", imp.module, imp.name));
                }
                Some(_) => return Err(format!("import {}.{} is not a tag", imp.module, imp.name)),
            },
        }
    }
    for tag_idx in tag_identities.len()..owned.tags.len() {
        tag_identities.push(format!("local:{tag_idx}"));
    }

    for d in 0..owned.func_types.len() {
        funcs.push(Callable::Defined(d));
        host_func_sigs.push(None);
    }
    for (global_idx, params, results, f) in pending_global_func_refs {
        let idx = funcs.len() as u32;
        funcs.push(Callable::Host(f));
        host_func_sigs.push(Some((params, results)));
        if let Some(global) = imported_globals.get_mut(global_idx) {
            *global = WasmValue::FuncRef(idx);
        }
    }

    let imported_tables: Vec<(ValType, usize, Option<u32>, Vec<WasmValue>)> = imported_tables
        .into_iter()
        .map(|(elem, len, max, values)| {
            let mut table_values = Vec::with_capacity(len);
            for value in values {
                match value {
                    TableImportValue::Null => table_values.push(WasmValue::RefNull),
                    TableImportValue::Func(value) => table_values.push(value),
                    TableImportValue::ExternRef(value) => table_values.push(value),
                    TableImportValue::HostFunc { params, results, f } => {
                        let idx = funcs.len() as u32;
                        funcs.push(Callable::Host(f));
                        host_func_sigs.push(Some((params, results)));
                        table_values.push(WasmValue::FuncRef(idx));
                    }
                }
            }
            (elem, len, max, table_values)
        })
        .collect();

    let mut globals: Vec<WasmValue> = imported_globals.clone();
    let mut global_mut: Vec<bool> = vec![false; imported_globals.len()];
    let mut gc_arrays: Vec<GcArray> = Vec::new();
    let mut gc_structs: Vec<GcStruct> = Vec::new();
    for g in &owned.globals {
        let v = eval_const_expr(&g.init, &globals, &owned, &mut gc_arrays, &mut gc_structs)?;
        globals.push(v);
        global_mut.push(g.mutable);
    }

    let imported_memory_count = imported_memories.len();
    let (mut memory, has_memory, mem_max, mem64, mem_shared) =
        if let Some((pages, max, memory64, shared, bytes, _)) = imported_memories.first() {
            let len = pages * PAGE;
            let mut memory = vec![0u8; len];
            if let Some(bytes) = bytes {
                let n = bytes.len().min(len);
                memory[..n].copy_from_slice(&bytes[..n]);
            }
            (memory, true, *max, *memory64, *shared)
        } else if let Some(lim) = owned.memories.first() {
            (
                vec![0u8; lim.min as usize * PAGE],
                true,
                lim.max,
                lim.memory64,
                lim.shared,
            )
        } else {
            (Vec::new(), false, None, false, false)
        };
    let mut extra_memories: Vec<Vec<u8>> = imported_memories
        .iter()
        .skip(1)
        .map(|(pages, _, _, _, bytes, _)| {
            let len = pages * PAGE;
            let mut memory = vec![0u8; len];
            if let Some(bytes) = bytes {
                let n = bytes.len().min(len);
                memory[..n].copy_from_slice(&bytes[..n]);
            }
            memory
        })
        .collect();
    let defined_extra_start = if imported_memory_count == 0 {
        1
    } else {
        imported_memory_count
    };
    extra_memories.extend(
        owned
            .memories
            .iter()
            .skip(defined_extra_start)
            .map(|lim| vec![0u8; lim.min as usize * PAGE]),
    );
    let mut extra_mem_max_pages: Vec<Option<u64>> = imported_memories
        .iter()
        .skip(1)
        .map(|(_, max, _, _, _, _)| *max)
        .collect();
    extra_mem_max_pages.extend(
        owned
            .memories
            .iter()
            .skip(defined_extra_start)
            .map(|lim| lim.max),
    );
    let mut memory_aliases: Vec<Option<u64>> = imported_memories
        .iter()
        .map(|(_, _, _, _, _, alias)| *alias)
        .collect();
    memory_aliases.extend(
        owned
            .memories
            .iter()
            .skip(defined_extra_start)
            .map(|_| None),
    );

    let mut table_types: Vec<ValType> = imported_tables
        .iter()
        .map(|(elem, _, _, _)| *elem)
        .collect();
    table_types.extend(owned.tables.iter().map(|t| ValType::from_parser(t.elem)));
    let mut tables: Vec<Vec<WasmValue>> = imported_tables
        .iter()
        .map(|(_, len, _, values)| {
            let mut table = vec![WasmValue::RefNull; *len];
            for (slot, value) in table.iter_mut().zip(values.iter().copied()) {
                *slot = value;
            }
            table
        })
        .collect();
    for (i, table_decl) in owned.tables.iter().enumerate() {
        let init = match owned.table_inits.get(i).and_then(|init| init.as_ref()) {
            Some(expr) => {
                let value =
                    eval_const_expr(expr, &globals, &owned, &mut gc_arrays, &mut gc_structs)?;
                let table_ty = ValType::from_parser(table_decl.elem);
                if !value_matches_ref_type_in_module(
                    value,
                    table_ty,
                    &owned,
                    &gc_arrays,
                    &gc_structs,
                ) {
                    return Err(format!(
                        "table {} initializer value {:?} incompatible with table type {:?}",
                        i, value, table_ty
                    ));
                }
                value
            }
            None => WasmValue::RefNull,
        };
        tables.push(vec![init; table_decl.limits.min as usize]);
    }
    let mut table_maxes: Vec<Option<u64>> = imported_tables
        .iter()
        .map(|(_, _, max, _)| max.map(u64::from))
        .collect();
    table_maxes.extend(owned.tables.iter().map(|t| t.limits.max));
    let mut table64s: Vec<bool> = imported_tables.iter().map(|_| false).collect();
    table64s.extend(owned.tables.iter().map(|t| t.limits.memory64));

    let mut elem_segments: Vec<Option<Vec<WasmValue>>> = Vec::with_capacity(owned.elements.len());
    for seg in &owned.elements {
        if seg.mode != ElementMode::Active {
            if seg.mode == ElementMode::Passive {
                let mut items = Vec::with_capacity(seg.items.len());
                for item in &seg.items {
                    items.push(match item {
                        ElementItem::Func(fidx) => WasmValue::FuncRef(*fidx),
                        ElementItem::Expr(expr) => {
                            match eval_const_expr(
                                expr,
                                &globals,
                                &owned,
                                &mut gc_arrays,
                                &mut gc_structs,
                            )? {
                                value
                                    if value_matches_ref_type_in_module(
                                        value,
                                        ValType::from_parser(seg.ty),
                                        &owned,
                                        &gc_arrays,
                                        &gc_structs,
                                    ) =>
                                {
                                    value
                                }
                                other => {
                                    return Err(format!(
                                "element segment expression produced incompatible reference {:?}",
                                other
                            ))
                                }
                            }
                        }
                    });
                }
                elem_segments.push(Some(items));
            } else {
                elem_segments.push(None);
            }
            continue;
        }
        elem_segments.push(None);
        let table64 = owned
            .tables
            .get(seg.table as usize)
            .map(|table| table.limits.memory64)
            .unwrap_or(false);
        let offset = const_expr_table_offset(
            seg.offset
                .as_ref()
                .ok_or("active element segment missing offset expr")?,
            &globals,
            table64,
            &owned,
            &mut gc_arrays,
            &mut gc_structs,
        )?;
        let table_ty = table_types
            .get(seg.table as usize)
            .copied()
            .ok_or_else(|| format!("element segment table {} out of bounds", seg.table))?;
        let table = tables
            .get_mut(seg.table as usize)
            .ok_or_else(|| format!("element segment table {} out of bounds", seg.table))?;
        let end = offset
            .checked_add(seg.items.len())
            .ok_or_else(|| "element segment out of table bounds".to_string())?;
        if end > table.len() {
            store_partial_instance(Instance::build(
                clone_module(&owned),
                funcs,
                host_func_sigs,
                globals,
                global_mut,
                memory,
                extra_memories,
                has_memory,
                mem_max,
                extra_mem_max_pages,
                memory_aliases,
                mem64,
                mem_shared,
                tables,
                table_maxes,
                table64s,
                placeholder_data_segments(&owned),
                elem_segments,
                tag_identities.clone(),
                gc_arrays,
                gc_structs,
                owned.func_types.len(),
            ));
            return Err("element segment out of table bounds".to_string());
        }
        for (k, item) in seg.items.iter().enumerate() {
            let slot = offset + k;
            let value = match item {
                ElementItem::Func(fidx) => WasmValue::FuncRef(*fidx),
                ElementItem::Expr(expr) => {
                    match eval_const_expr(expr, &globals, &owned, &mut gc_arrays, &mut gc_structs)?
                    {
                        value
                            if value_matches_ref_type_in_module(
                                value,
                                ValType::from_parser(seg.ty),
                                &owned,
                                &gc_arrays,
                                &gc_structs,
                            ) =>
                        {
                            value
                        }
                        other => {
                            return Err(format!(
                                "element segment expression produced incompatible reference {:?}",
                                other
                            ))
                        }
                    }
                }
            };
            if !value_matches_ref_type_in_module(value, table_ty, &owned, &gc_arrays, &gc_structs) {
                return Err(format!(
                    "element segment value {:?} incompatible with table type {:?}",
                    value, table_ty
                ));
            }
            table[slot] = value;
        }
    }

    let mut data_segments: Vec<Option<Vec<u8>>> = Vec::with_capacity(owned.data.len());
    for seg in &owned.data {
        if seg.passive {
            data_segments.push(Some(seg.bytes.clone()));
            continue;
        }
        data_segments.push(None);
        let target_len = if seg.memory == 0 {
            if !has_memory {
                return Err("data segment but no memory".to_string());
            }
            memory.len()
        } else {
            extra_memories
                .get(seg.memory as usize - 1)
                .map(|memory| memory.len())
                .ok_or_else(|| format!("data segment memory {} out of bounds", seg.memory))?
        };
        let offset_expr = seg
            .offset
            .as_ref()
            .ok_or("active data segment missing offset expr")?;
        let memory64 = owned
            .memories
            .get(seg.memory as usize)
            .map(|limits| limits.memory64)
            .unwrap_or(mem64);
        let offset = const_expr_mem_offset(
            offset_expr,
            &globals,
            memory64,
            &owned,
            &mut gc_arrays,
            &mut gc_structs,
        )?;
        let end = offset
            .checked_add(seg.bytes.len())
            .ok_or("data segment offset overflow")?;
        if end > target_len {
            store_partial_instance(Instance::build(
                clone_module(&owned),
                funcs,
                host_func_sigs,
                globals,
                global_mut,
                memory,
                extra_memories,
                has_memory,
                mem_max,
                extra_mem_max_pages,
                memory_aliases,
                mem64,
                mem_shared,
                tables,
                table_maxes,
                table64s,
                data_segments,
                elem_segments,
                tag_identities.clone(),
                gc_arrays,
                gc_structs,
                owned.func_types.len(),
            ));
            return Err("data segment out of memory bounds".to_string());
        }
        if seg.memory == 0 {
            memory[offset..end].copy_from_slice(&seg.bytes);
        } else {
            extra_memories[seg.memory as usize - 1][offset..end].copy_from_slice(&seg.bytes);
        }
        sync_memory_aliases_after_write(
            seg.memory as usize,
            &memory_aliases,
            &mut memory,
            &mut extra_memories,
        );
    }

    let n_defined = owned.func_types.len();
    let start = owned.start;

    let mut instance = Instance::build(
        owned,
        funcs,
        host_func_sigs,
        globals,
        global_mut,
        memory,
        extra_memories,
        has_memory,
        mem_max,
        extra_mem_max_pages,
        memory_aliases,
        mem64,
        mem_shared,
        tables,
        table_maxes,
        table64s,
        data_segments,
        elem_segments,
        tag_identities,
        gc_arrays,
        gc_structs,
        n_defined,
    );

    if let Some(start_idx) = start {
        if let Err(err) = instance.call_overall(start_idx as usize, &[]) {
            store_partial_instance(instance);
            return Err(err);
        }
    }

    Ok(instance)
}

fn placeholder_data_segments(module: &Module) -> Vec<Option<Vec<u8>>> {
    module
        .data
        .iter()
        .map(|seg| seg.passive.then(|| seg.bytes.clone()))
        .collect()
}

fn store_partial_instance(instance: Instance) {
    LAST_PARTIAL_INSTANCE.with(|slot| {
        *slot.borrow_mut() = Some(instance);
    });
}

impl Instance {
    pub fn has_func_imports(&self) -> bool {
        self.module.imported_func_count > 0
    }

    pub fn export_func_specs(&self) -> Vec<ExportFuncSpec> {
        module_export_func_specs(&self.module)
    }

    pub fn func_may_write_memory(&self, overall: usize) -> bool {
        let imported = self.module.imported_func_count;
        if overall < imported {
            return true;
        }
        let Some(code) = self.module.code.get(overall - imported) else {
            return true;
        };
        code.body.iter().any(instr_may_write_memory)
    }

    pub fn non_host_func_count(&self) -> usize {
        self.module.imported_func_count + self.module.func_types.len()
    }

    pub fn export_func_names(&self) -> Vec<String> {
        self.module
            .exports
            .iter()
            .filter(|e| e.kind == ExportKind::Func)
            .map(|e| e.name.clone())
            .collect()
    }

    pub fn export_func_sig(&self, name: &str) -> Option<(Vec<ValType>, Vec<ValType>)> {
        let export = self
            .module
            .exports
            .iter()
            .find(|e| e.kind == ExportKind::Func && e.name == name)?;

        self.func_sig_by_index(export.index as usize)
    }

    pub fn func_sig_by_index(&self, overall: usize) -> Option<(Vec<ValType>, Vec<ValType>)> {
        if let Some(Some(sig)) = self.host_func_sigs.get(overall) {
            return Some(sig.clone());
        }
        let imported = self.module.imported_func_count;
        let type_idx = if overall < imported {

            let mut seen = 0usize;
            let mut found = None;
            for imp in &self.module.imports {
                if let ImportKind::Func(ti) = &imp.kind {
                    if seen == overall {
                        found = Some(*ti as usize);
                        break;
                    }
                    seen += 1;
                }
            }
            found?
        } else {

            *self.module.func_types.get(overall - imported)? as usize
        };
        let ft = self.module.types.get(type_idx)?;
        let params: Vec<ValType> = ft.params.iter().map(|t| ValType::from_parser(*t)).collect();
        let results: Vec<ValType> = ft
            .results
            .iter()
            .map(|t| ValType::from_parser(*t))
            .collect();
        Some((params, results))
    }

    pub fn call_func_index(
        &mut self,
        overall: usize,
        args: &[WasmValue],
    ) -> Result<Vec<WasmValue>, String> {
        self.call_overall(overall, args)
    }

    pub fn export_memory_names(&self) -> Vec<String> {
        self.module
            .exports
            .iter()
            .filter(|e| e.kind == ExportKind::Memory)
            .map(|e| e.name.clone())
            .collect()
    }

    pub fn export_memory_specs(
        &self,
    ) -> Vec<(String, usize, usize, Option<u64>, bool, bool, Vec<u8>)> {
        self.module
            .exports
            .iter()
            .filter(|e| e.kind == ExportKind::Memory)
            .filter_map(|e| {
                let index = e.index as usize;
                let bytes = self.memory_read_at(index, 0, self.memory_size_at(index)?)?;
                let (max, shared, memory64) = self.memory_type_at(index)?;
                Some((
                    e.name.clone(),
                    index,
                    bytes.len(),
                    max,
                    shared,
                    memory64,
                    bytes,
                ))
            })
            .collect()
    }

    pub fn memory_size_at(&self, index: usize) -> Option<usize> {
        if index == 0 {
            self.has_memory.then_some(self.memory.len())
        } else {
            self.extra_memories.get(index - 1).map(Vec::len)
        }
    }

    pub fn memory_read_at(&self, index: usize, offset: usize, len: usize) -> Option<Vec<u8>> {
        let memory = if index == 0 {
            self.has_memory.then_some(self.memory.as_slice())?
        } else {
            self.extra_memories.get(index - 1)?.as_slice()
        };
        if offset.checked_add(len)? > memory.len() {
            return None;
        }
        Some(memory[offset..offset + len].to_vec())
    }

    pub fn take_memory_dirty_range_at(&mut self, index: usize) -> Option<(usize, usize)> {
        self.memory_dirty_ranges.get_mut(index)?.take()
    }

    pub fn clear_memory_dirty_range_at(&mut self, index: usize) {
        if let Some(range) = self.memory_dirty_ranges.get_mut(index) {
            *range = None;
        }
    }

    fn memory_type_at(&self, index: usize) -> Option<(Option<u64>, bool, bool)> {
        let mut memory_index = 0usize;
        for import in &self.module.imports {
            if let ImportKind::Memory(limits) = import.kind {
                if memory_index == index {
                    return Some((limits.max, limits.shared, limits.memory64));
                }
                memory_index += 1;
            }
        }
        self.module
            .memories
            .get(index)
            .map(|limits| (limits.max, limits.shared, limits.memory64))
    }

    pub fn export_table_names(&self) -> Vec<String> {
        self.module
            .exports
            .iter()
            .filter(|e| e.kind == ExportKind::Table)
            .map(|e| e.name.clone())
            .collect()
    }

    pub fn export_table_specs(
        &self,
    ) -> Vec<(
        String,
        usize,
        usize,
        Option<u64>,
        Vec<WasmValue>,
        Option<ValType>,
        bool,
    )> {
        self.module
            .exports
            .iter()
            .filter(|e| e.kind == ExportKind::Table)
            .map(|e| {
                let index = e.index as usize;
                (
                    e.name.clone(),
                    index,
                    self.table_size_at(index),
                    self.table_max_at(index),
                    self.table_values_at(index),
                    self.table_element_type_at(index),
                    self.table_address64_at(index),
                )
            })
            .collect()
    }

    pub fn export_tag_specs(&self) -> Vec<(String, u32, Vec<ValType>, String)> {
        self.module
            .exports
            .iter()
            .filter(|e| e.kind == ExportKind::Tag)
            .map(|e| {
                let (params, type_shape) = self
                    .module
                    .tags
                    .get(e.index as usize)
                    .map(|tag| {
                        let params = self
                            .module
                            .types
                            .get(tag.type_idx as usize)
                            .map(|ft| {
                                ft.params
                                    .iter()
                                    .map(|ty| ValType::from_parser(*ty))
                                    .collect()
                            })
                            .unwrap_or_default();
                        (params, type_shape_key(&self.module, tag.type_idx))
                    })
                    .unwrap_or_default();
                (e.name.clone(), e.index, params, type_shape)
            })
            .collect()
    }

    pub fn table_size(&self) -> usize {
        self.table_size_at(0)
    }

    pub fn table_max(&self) -> Option<u64> {
        self.table_max_at(0)
    }

    pub fn table_func_indices(&self) -> Vec<Option<u32>> {
        self.table_func_indices_at(0)
    }

    pub fn table_size_at(&self, index: usize) -> usize {
        self.tables.get(index).map(|table| table.len()).unwrap_or(0)
    }

    pub fn table_max_at(&self, index: usize) -> Option<u64> {
        self.table_maxes.get(index).copied().flatten()
    }

    pub fn table_address64_at(&self, index: usize) -> bool {
        self.table64s.get(index).copied().unwrap_or(false)
    }

    pub fn table_func_indices_at(&self, index: usize) -> Vec<Option<u32>> {
        self.tables
            .get(index)
            .map(|table| table.as_slice())
            .unwrap_or(&[])
            .iter()
            .map(|value| match value {
                WasmValue::FuncRef(idx) => Some(*idx),
                _ => None,
            })
            .collect()
    }

    pub fn table_values_at(&self, index: usize) -> Vec<WasmValue> {
        self.tables.get(index).cloned().unwrap_or_default()
    }

    pub fn table_element_type_at(&self, index: usize) -> Option<ValType> {
        let mut table_idx = 0usize;
        for import in &self.module.imports {
            if let ImportKind::Table(ty) = import.kind {
                if table_idx == index {
                    return Some(ValType::from_parser(ty.elem));
                }
                table_idx += 1;
            }
        }
        let defined_idx = index.checked_sub(table_idx)?;
        self.module
            .tables
            .get(defined_idx)
            .map(|table| ValType::from_parser(table.elem))
    }

    pub fn set_table_values_at(
        &mut self,
        index: usize,
        values: Vec<WasmValue>,
    ) -> Result<(), String> {
        let table = self
            .tables
            .get_mut(index)
            .ok_or_else(|| format!("table index {} out of bounds", index))?;
        if values.len() > table.len() {
            return Err(format!(
                "table {} update has {} elements for table length {}",
                index,
                values.len(),
                table.len()
            ));
        }
        for (slot, value) in table.iter_mut().zip(values.into_iter()) {
            *slot = value;
        }
        Ok(())
    }

    pub fn push_host_table_func(
        &mut self,
        params: Vec<ValType>,
        results: Vec<ValType>,
        f: HostFn,
    ) -> u32 {
        let idx = self.funcs.len() as u32;
        self.funcs.push(Callable::Host(f));
        self.host_func_sigs.push(Some((params, results)));
        idx
    }

    pub fn global_type(&self, idx: usize) -> Option<ValType> {
        let mut global_idx = 0usize;
        for import in &self.module.imports {
            if let ImportKind::Global { ty, .. } = import.kind {
                if global_idx == idx {
                    return Some(ValType::from_parser(ty));
                }
                global_idx += 1;
            }
        }
        let defined_idx = idx.checked_sub(global_idx)?;
        self.module
            .globals
            .get(defined_idx)
            .map(|global| ValType::from_parser(global.ty))
    }

    pub fn export_globals(&self) -> Vec<(String, usize, WasmValue, bool, ValType)> {
        self.module
            .exports
            .iter()
            .filter(|e| e.kind == ExportKind::Global)
            .filter_map(|e| {
                let idx = e.index as usize;
                Some((
                    e.name.clone(),
                    idx,
                    *self.globals.get(idx)?,
                    *self.global_mut.get(idx).unwrap_or(&false),
                    self.global_type(idx)?,
                ))
            })
            .collect()
    }

    pub fn global_value(&self, idx: usize) -> Option<WasmValue> {
        self.globals.get(idx).copied()
    }

    pub fn set_global_value(&mut self, idx: usize, value: WasmValue) -> Result<(), String> {
        if idx >= self.globals.len() {
            return Err("global.set out of range".to_string());
        }
        if !self.global_mut.get(idx).copied().unwrap_or(false) {
            return Err("global.set on immutable global".to_string());
        }
        self.globals[idx] = value;
        Ok(())
    }

    pub fn call(&mut self, name: &str, args: &[WasmValue]) -> Result<Vec<WasmValue>, String> {
        let export = self
            .module
            .exports
            .iter()
            .find(|e| e.kind == ExportKind::Func && e.name == name)
            .ok_or_else(|| format!("no exported function named '{}'", name))?;
        let overall = export.index as usize;
        self.call_overall(overall, args)
    }

    pub fn memory_size(&self) -> usize {
        self.memory.len()
    }

    pub fn memory_max_pages(&self) -> Option<u64> {
        self.mem_max_pages
    }

    pub fn memory_is_shared(&self) -> bool {
        self.memory_shared
    }

    pub fn memory_grow(&mut self, delta_pages: usize) -> Result<usize, String> {
        if !self.has_memory {
            return Err("no memory".to_string());
        }
        let old_pages = self.memory.len() / PAGE;
        let new_pages = old_pages
            .checked_add(delta_pages)
            .ok_or_else(|| "maximum memory size exceeded".to_string())?;
        let allowed = match self.mem_max_pages {
            Some(mx) => new_pages <= mx as usize && new_pages <= max_memory_pages(),
            None => new_pages <= 65536 && new_pages <= max_memory_pages(),
        };
        if !allowed {
            return Err("maximum memory size exceeded".to_string());
        }
        self.memory.resize(new_pages * PAGE, 0);
        Ok(old_pages)
    }

    pub fn memory_read(&self, offset: usize, len: usize) -> Option<Vec<u8>> {
        if offset.checked_add(len)? > self.memory.len() {
            return None;
        }
        Some(self.memory[offset..offset + len].to_vec())
    }

    pub fn memory_write(&mut self, offset: usize, data: &[u8]) -> bool {
        self.memory_write_at(0, offset, data)
    }

    pub fn memory_write_at(&mut self, index: usize, offset: usize, data: &[u8]) -> bool {
        let Some(memory) = (if index == 0 {
            self.has_memory.then_some(&mut self.memory)
        } else {
            self.extra_memories.get_mut(index - 1)
        }) else {
            return false;
        };
        match offset.checked_add(data.len()) {
            Some(end) if end <= memory.len() => {
                memory[offset..end].copy_from_slice(data);
                if self.memory_dirty_ranges.len() <= index {
                    self.memory_dirty_ranges.resize(index + 1, None);
                }
                self.memory_dirty_ranges[index] = Some(match self.memory_dirty_ranges[index] {
                    Some((old_start, old_end)) => (old_start.min(offset), old_end.max(end)),
                    None => (offset, end),
                });
                true
            }
            _ => false,
        }
    }
}

fn value_matches_ref_type(value: WasmValue, ty: ValType) -> bool {
    matches!(
        (value, ty),
        (
            WasmValue::RefNull,
            ValType::AnyRef
                | ValType::EqRef
                | ValType::FuncRef
                | ValType::NonNullFuncRef
                | ValType::ExternRef
                | ValType::NonNullExternRef
                | ValType::StructRef
                | ValType::ArrayRef
                | ValType::I31Ref
                | ValType::TypeRef(_)
                | ValType::NonNullTypeRef(_)
                | ValType::NullRef
                | ValType::NullFuncRef
                | ValType::NullExternRef
                | ValType::Unknown
        ) | (WasmValue::FuncRef(_), ValType::AnyRef | ValType::FuncRef)
            | (WasmValue::FuncRef(_), ValType::NonNullFuncRef)
            | (
                WasmValue::FuncRef(_),
                ValType::TypeRef(_) | ValType::NonNullTypeRef(_)
            )
            | (
                WasmValue::ArrayRef(_),
                ValType::AnyRef | ValType::EqRef | ValType::ArrayRef | ValType::NonNullArrayRef
            )
            | (
                WasmValue::StructRef(_),
                ValType::AnyRef | ValType::EqRef | ValType::StructRef | ValType::NonNullStructRef
            )
            | (
                WasmValue::I31Ref(_),
                ValType::AnyRef | ValType::EqRef | ValType::I31Ref | ValType::NonNullI31Ref
            )
            | (WasmValue::ExternRef(_), ValType::ExternRef)
            | (WasmValue::ExternI31Ref(_), ValType::ExternRef)
            | (WasmValue::ExternStructRef(_), ValType::ExternRef)
            | (WasmValue::ExternArrayRef(_), ValType::ExternRef)
    )
}

fn value_matches_ref_type_in_module(
    value: WasmValue,
    ty: ValType,
    module: &Module,
    gc_arrays: &[GcArray],
    gc_structs: &[GcStruct],
) -> bool {
    if value_matches_ref_type(value, ty) {
        return true;
    }
    match (value, ty) {
        (
            WasmValue::ArrayRef(idx),
            ValType::TypeRef(expected) | ValType::NonNullTypeRef(expected),
        ) => gc_arrays
            .get(idx as usize)
            .map(|array| type_ref_matches_module(array.type_idx, expected, module, &mut Vec::new()))
            .unwrap_or(false),
        (
            WasmValue::StructRef(idx),
            ValType::TypeRef(expected) | ValType::NonNullTypeRef(expected),
        ) => gc_structs
            .get(idx as usize)
            .map(|st| type_ref_matches_module(st.type_idx, expected, module, &mut Vec::new()))
            .unwrap_or(false),
        _ => false,
    }
}

fn memory_bytes_mut<'a>(
    memory_index: usize,
    memory: &'a mut Vec<u8>,
    extra_memories: &'a mut [Vec<u8>],
) -> Option<&'a mut Vec<u8>> {
    if memory_index == 0 {
        Some(memory)
    } else {
        extra_memories.get_mut(memory_index - 1)
    }
}

fn memory_bytes<'a>(
    memory_index: usize,
    memory: &'a [u8],
    extra_memories: &'a [Vec<u8>],
) -> Option<&'a [u8]> {
    if memory_index == 0 {
        Some(memory)
    } else {
        extra_memories
            .get(memory_index - 1)
            .map(|memory| memory.as_slice())
    }
}

fn sync_memory_aliases_after_write(
    written_index: usize,
    memory_aliases: &[Option<u64>],
    memory: &mut Vec<u8>,
    extra_memories: &mut [Vec<u8>],
) {
    let Some(Some(alias)) = memory_aliases.get(written_index).copied() else {
        return;
    };
    let Some(bytes) =
        memory_bytes(written_index, memory, extra_memories).map(|bytes| bytes.to_vec())
    else {
        return;
    };
    for alias_index in 0..memory_aliases.len() {
        if alias_index == written_index
            || memory_aliases.get(alias_index).copied().flatten() != Some(alias)
        {
            continue;
        }
        if let Some(target) = memory_bytes_mut(alias_index, memory, extra_memories) {
            if target.len() == bytes.len() {
                target.copy_from_slice(&bytes);
            }
        }
    }
}

fn type_ref_matches_module(
    actual: u32,
    expected: u32,
    module: &Module,
    seen: &mut Vec<u32>,
) -> bool {
    if actual == expected || type_shape_key(module, actual) == type_shape_key(module, expected) {
        return true;
    }
    if seen.contains(&actual) {
        return false;
    }
    seen.push(actual);
    module
        .type_supertypes
        .get(actual as usize)
        .map(|supertypes| {
            supertypes
                .iter()
                .any(|super_idx| type_ref_matches_module(*super_idx, expected, module, seen))
        })
        .unwrap_or(false)
}

fn instr_may_write_memory(instr: &Instr) -> bool {
    matches!(
        instr,
        Instr::Call(_)
            | Instr::ReturnCall(_)
            | Instr::CallIndirect(_, _)
            | Instr::CallRef(_)
            | Instr::Store(_, _)
            | Instr::AtomicStore(_, _)
            | Instr::AtomicRmw(_, _)
            | Instr::AtomicNotify(_)
            | Instr::MemoryGrow(_)
            | Instr::V128Store(_)
            | Instr::V128Store8Lane(_, _)
            | Instr::MemoryInit(_, _)
            | Instr::MemoryCopy(_, _)
            | Instr::MemoryFill(_)
    )
}

fn clone_module(m: &Module) -> Module {
    Module {
        custom_sections: m.custom_sections.clone(),
        types: m.types.clone(),
        type_is_func: m.type_is_func.clone(),
        type_supertypes: m.type_supertypes.clone(),
        type_is_final: m.type_is_final.clone(),
        type_rec_groups: m.type_rec_groups.clone(),
        array_types: m.array_types.clone(),
        struct_types: m.struct_types.clone(),
        imports: m.imports.clone(),
        func_types: m.func_types.clone(),
        tables: m.tables.clone(),
        table_inits: m.table_inits.clone(),
        memories: m.memories.clone(),
        globals: m.globals.clone(),
        tags: m.tags.clone(),
        exports: m.exports.clone(),
        start: m.start,
        elements: m.elements.clone(),
        data_count: m.data_count,
        code: m.code.clone(),
        data: m.data.clone(),
        imported_func_count: m.imported_func_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uleb(mut v: u64, out: &mut Vec<u8>) {
        loop {
            let mut byte = (v & 0x7f) as u8;
            v >>= 7;
            if v != 0 {
                byte |= 0x80;
            }
            out.push(byte);
            if v == 0 {
                break;
            }
        }
    }

    fn sleb(mut v: i64, out: &mut Vec<u8>) {
        loop {
            let mut byte = (v & 0x7f) as u8;
            v >>= 7;
            let sign_bit = byte & 0x40;
            let done = (v == 0 && sign_bit == 0) || (v == -1 && sign_bit != 0);
            if !done {
                byte |= 0x80;
            }
            out.push(byte);
            if done {
                break;
            }
        }
    }

    fn section(id: u8, body: Vec<u8>) -> Vec<u8> {
        let mut out = vec![id];
        uleb(body.len() as u64, &mut out);
        out.extend(body);
        out
    }

    fn header() -> Vec<u8> {
        vec![0, 0x61, 0x73, 0x6d, 1, 0, 0, 0]
    }

    #[test]
    fn test_add_module_literal_bytes() {
        let bytes: Vec<u8> = vec![
            0, 97, 115, 109, 1, 0, 0, 0, 1, 7, 1, 96, 2, 127, 127, 1, 127, 3, 2, 1, 0, 7, 7, 1, 3,
            97, 100, 100, 0, 0, 10, 9, 1, 7, 0, 32, 0, 32, 1, 106, 11,
        ];
        let m = parse_module(&bytes).expect("parse add");
        let mut inst = instantiate(&m, Imports::new()).expect("instantiate add");
        assert_eq!(inst.export_func_names(), vec!["add".to_string()]);

        let r = inst
            .call("add", &[WasmValue::I32(2), WasmValue::I32(3)])
            .unwrap();
        assert_eq!(r, vec![WasmValue::I32(5)]);

        let r = inst
            .call("add", &[WasmValue::I32(-1), WasmValue::I32(1)])
            .unwrap();
        assert_eq!(r, vec![WasmValue::I32(0)]);

        let r = inst
            .call("add", &[WasmValue::I32(i32::MAX), WasmValue::I32(1)])
            .unwrap();
        assert_eq!(r, vec![WasmValue::I32(i32::MIN)]);
    }

    #[test]
    fn test_void_leaf_module_returns_no_values() {
        let mut bytes = header();
        bytes.extend(section(1, vec![1, 0x60, 0, 0]));
        bytes.extend(section(3, vec![1, 0]));
        bytes.extend(section(7, vec![1, 3, b'r', b'u', b'n', 0, 0]));
        bytes.extend(section(10, vec![1, 3, 0, 0x01, 0x0b]));
        let m = parse_module(&bytes).expect("parse void leaf");
        let mut inst = instantiate(&m, Imports::new()).expect("instantiate void leaf");
        assert_eq!(inst.call("run", &[]).unwrap(), Vec::new());
    }

    #[test]
    fn test_i32_const_add_leaf_executes() {
        let mut bytes = header();
        bytes.extend(section(1, vec![1, 0x60, 1, 0x7f, 1, 0x7f]));
        bytes.extend(section(3, vec![1, 0]));
        bytes.extend(section(7, vec![1, 3, b'r', b'u', b'n', 0, 0]));
        bytes.extend(section(10, vec![1, 7, 0, 0x20, 0, 0x41, 41, 0x6a, 0x0b]));
        let m = parse_module(&bytes).expect("parse const-add leaf");
        let mut inst = instantiate(&m, Imports::new()).expect("instantiate const-add leaf");
        assert_eq!(
            inst.call("run", &[WasmValue::I32(1)]).unwrap(),
            vec![WasmValue::I32(42)]
        );
    }

    fn imported_memory_load_module(with_data: bool) -> Vec<u8> {
        let mut bytes = header();
        bytes.extend(section(1, vec![1, 0x60, 0, 1, 0x7f]));
        bytes.extend(section(
            2,
            vec![
                1,
                3, b'e', b'n', b'v',
                3, b'm', b'e', b'm',
                2,
                0,
                1,
            ],
        ));
        bytes.extend(section(3, vec![1, 0]));
        bytes.extend(section(7, vec![1, 4, b'l', b'o', b'a', b'd', 0, 0]));
        bytes.extend(section(10, vec![1, 7, 0, 0x41, 0, 0x28, 2, 0, 0x0b]));
        if with_data {
            bytes.extend(section(
                11,
                vec![
                    1,
                    0,
                    0x41, 0, 0x0b,
                    4, 9, 0, 0, 0,
                ],
            ));
        }
        bytes
    }

    #[test]
    fn test_imported_memory_seed_bytes_visible_to_load() {
        let module =
            parse_module(&imported_memory_load_module(false)).expect("parse imported memory load");
        let mut seed = vec![0; PAGE];
        seed[..4].copy_from_slice(&55i32.to_le_bytes());
        let mut imports = Imports::new();
        imports.memory_with_shared_bytes("env", "mem", 1, None, false, Some(seed));

        let mut inst = instantiate(&module, imports).expect("instantiate imported memory load");
        let result = inst.call("load", &[]).expect("call load");
        assert_eq!(result, vec![WasmValue::I32(55)]);
    }

    #[test]
    fn test_imported_memory_active_data_overlays_seed_bytes() {
        let module =
            parse_module(&imported_memory_load_module(true)).expect("parse imported memory data");
        let mut seed = vec![0; PAGE];
        seed[..4].copy_from_slice(&55i32.to_le_bytes());
        let mut imports = Imports::new();
        imports.memory_with_shared_bytes("env", "mem", 1, None, false, Some(seed));

        let mut inst = instantiate(&module, imports).expect("instantiate imported memory data");
        let result = inst.call("load", &[]).expect("call load");
        assert_eq!(result, vec![WasmValue::I32(9)]);
        assert_eq!(inst.memory_read(0, 4), Some(9i32.to_le_bytes().to_vec()));
    }

    #[test]
    fn test_ref_eq_null_refs_execute() {
        let mut typesec = Vec::new();
        uleb(1, &mut typesec);
        typesec.push(0x60);
        uleb(0, &mut typesec);
        uleb(1, &mut typesec);
        typesec.push(0x7f);

        let mut funcsec = Vec::new();
        uleb(1, &mut funcsec);
        uleb(0, &mut funcsec);

        let mut exportsec = Vec::new();
        uleb(1, &mut exportsec);
        uleb(2, &mut exportsec);
        exportsec.extend_from_slice(b"eq");
        exportsec.push(0x00);
        uleb(0, &mut exportsec);

        let body = vec![0, 0xd0, 0x6d, 0xd0, 0x6d, 0xd3, 0x0b];
        let mut codesec = Vec::new();
        uleb(1, &mut codesec);
        uleb(body.len() as u64, &mut codesec);
        codesec.extend(body);

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(3, funcsec));
        bytes.extend(section(7, exportsec));
        bytes.extend(section(10, codesec));

        let module = parse_module(&bytes).expect("parse ref.eq module");
        let mut inst = instantiate(&module, Imports::new()).expect("instantiate ref.eq module");
        let result = inst.call("eq", &[]).expect("call ref.eq");
        assert_eq!(result, vec![WasmValue::I32(1)]);
    }

    #[test]
    fn test_extended_const_global_initializer_executes() {
        let mut bytes = header();
        bytes.extend(section(1, vec![1, 0x60, 0, 1, 0x7f]));
        bytes.extend(section(3, vec![1, 0]));
        bytes.extend(section(
            6,
            vec![
                1,
                0x7f, 0,
                0x41, 1,
                0x41, 2,
                0x6a,
                0x0b,
            ],
        ));
        bytes.extend(section(7, vec![1, 3, b'r', b'u', b'n', 0, 0]));
        bytes.extend(section(10, vec![1, 4, 0, 0x23, 0, 0x0b]));

        let module = parse_module(&bytes).expect("parse extended const global module");
        let mut inst = instantiate(&module, Imports::new()).expect("instantiate extended const");
        let result = inst.call("run", &[]).expect("call run");
        assert_eq!(result, vec![WasmValue::I32(3)]);
    }

    #[test]
    fn test_memory64_memory_size_returns_i64() {
        let mut bytes = header();
        bytes.extend(section(1, vec![1, 0x60, 0, 1, 0x7e]));
        bytes.extend(section(3, vec![1, 0]));
        bytes.extend(section(5, vec![1, 0x04, 1]));
        bytes.extend(section(7, vec![1, 3, b'r', b'u', b'n', 0, 0]));
        bytes.extend(section(10, vec![1, 4, 0, 0x3f, 0, 0x0b]));

        let module = parse_module(&bytes).expect("parse memory64 memory.size module");
        let mut inst = instantiate(&module, Imports::new()).expect("instantiate memory64");
        let result = inst.call("run", &[]).expect("call run");
        assert_eq!(result, vec![WasmValue::I64(1)]);
    }

    #[test]
    fn test_memory64_load_uses_i64_address() {
        let mut bytes = header();
        bytes.extend(section(1, vec![1, 0x60, 0, 1, 0x7f]));
        bytes.extend(section(3, vec![1, 0]));
        bytes.extend(section(5, vec![1, 0x04, 1]));
        bytes.extend(section(7, vec![1, 4, b'l', b'o', b'a', b'd', 0, 0]));
        bytes.extend(section(10, vec![1, 7, 0, 0x42, 0, 0x2d, 0, 0, 0x0b]));
        bytes.extend(section(11, vec![1, 0, 0x42, 0, 0x0b, 1, 97]));

        let module = parse_module(&bytes).expect("parse memory64 load module");
        let mut inst = instantiate(&module, Imports::new()).expect("instantiate memory64 load");
        let result = inst.call("load", &[]).expect("call load");
        assert_eq!(result, vec![WasmValue::I32(97)]);
    }

    #[test]
    fn test_memory64_store_overflow_traps_instead_of_panicking() {
        let mut bytes = header();
        bytes.extend(section(1, vec![1, 0x60, 0, 0]));
        bytes.extend(section(3, vec![1, 0]));
        bytes.extend(section(5, vec![1, 0x04, 1]));
        bytes.extend(section(7, vec![1, 5, b's', b't', b'o', b'r', b'e', 0, 0]));
        bytes.extend(section(
            10,
            vec![
                1,
                9,
                0,
                0x42, 0x7f,
                0x41, 0,
                0x36, 2, 0,
                0x0b,
            ],
        ));

        let module = parse_module(&bytes).expect("parse memory64 store overflow module");
        let mut inst = instantiate(&module, Imports::new()).expect("instantiate memory64 store");
        let err = inst.call("store", &[]).expect_err("store should trap");
        assert!(
            err.contains("out of bounds memory access"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_i32_atomic_store_then_load_executes_on_shared_memory() {
        let mut bytes = header();
        bytes.extend(section(
            1,
            vec![
                2,
                0x60, 0, 0,
                0x60, 0, 1, 0x7f,
            ],
        ));
        bytes.extend(section(3, vec![2, 0, 1]));
        bytes.extend(section(5, vec![1, 0x03, 1, 1]));
        bytes.extend(section(
            7,
            vec![
                2,
                5, b's', b't', b'o', b'r', b'e', 0, 0,
                4, b'l', b'o', b'a', b'd', 0, 1,
            ],
        ));
        bytes.extend(section(
            10,
            vec![
                2,
                10, 0, 0x41, 0, 0x41, 42, 0xfe, 0x17, 2, 0, 0x0b,
                8, 0, 0x41, 0, 0xfe, 0x10, 2, 0, 0x0b,
            ],
        ));

        let module = parse_module(&bytes).expect("parse atomic load/store module");
        let mut inst = instantiate(&module, Imports::new()).expect("instantiate atomic module");
        assert_eq!(
            inst.call("load", &[]).expect("initial load"),
            vec![WasmValue::I32(0)]
        );
        assert_eq!(
            inst.call("store", &[]).expect("store"),
            Vec::<WasmValue>::new()
        );
        assert_eq!(
            inst.call("load", &[]).expect("load"),
            vec![WasmValue::I32(42)]
        );
    }

    #[test]
    fn test_i64_atomic_store_load_and_rmw_add_execute_on_shared_memory() {
        let mut bytes = header();
        bytes.extend(section(
            1,
            vec![
                3,
                0x60, 0, 0,
                0x60, 0, 1, 0x7e,
                0x60, 0, 1, 0x7e,
            ],
        ));
        bytes.extend(section(3, vec![3, 0, 1, 2]));
        bytes.extend(section(5, vec![1, 0x03, 1, 1]));
        bytes.extend(section(
            7,
            vec![
                4,
                5, b's', b't', b'o', b'r', b'e', 0, 0,
                4, b'l', b'o', b'a', b'd', 0, 1,
                3, b'a', b'd', b'd', 0, 2,
                3, b'm', b'e', b'm', 2, 0,
            ],
        ));

        let mut store_body = vec![0, 0x41, 0, 0x42];
        sleb(0x0102030405060708, &mut store_body);
        store_body.extend([0xfe, 0x18, 3, 0, 0x0b]);

        let load_body = vec![0, 0x41, 0, 0xfe, 0x11, 3, 0, 0x0b];

        let mut add_body = vec![0, 0x41, 0, 0x42];
        sleb(2, &mut add_body);
        add_body.extend([0xfe, 0x1f, 3, 0, 0x0b]);

        let mut code = vec![3];
        for body in [store_body, load_body, add_body] {
            uleb(body.len() as u64, &mut code);
            code.extend(body);
        }
        bytes.extend(section(10, code));

        let module = parse_module(&bytes).expect("parse i64 atomic module");
        let mut inst = instantiate(&module, Imports::new()).expect("instantiate i64 atomic module");
        assert_eq!(
            inst.call("store", &[]).expect("store"),
            Vec::<WasmValue>::new()
        );
        assert_eq!(
            inst.call("load", &[]).expect("load"),
            vec![WasmValue::I64(0x0102030405060708)]
        );
        assert_eq!(
            inst.call("add", &[]).expect("rmw add"),
            vec![WasmValue::I64(0x0102030405060708)]
        );
        assert_eq!(
            inst.memory_read(0, 8),
            Some(0x010203040506070a_i64.to_le_bytes().to_vec())
        );
    }

    fn i64_atomic_rmw_module(subopcode: u8, initial: i64, operand: i64) -> Vec<u8> {
        let mut bytes = header();
        bytes.extend(section(1, vec![1, 0x60, 0, 1, 0x7e]));
        bytes.extend(section(3, vec![1, 0]));
        bytes.extend(section(5, vec![1, 0x03, 1, 1]));
        bytes.extend(section(7, vec![1, 3, b'r', b'u', b'n', 0, 0]));
        let mut body = vec![0, 0x41, 0, 0x42];
        sleb(operand, &mut body);
        body.extend([0xfe, subopcode, 3, 0, 0x0b]);
        let mut code = vec![1];
        uleb(body.len() as u64, &mut code);
        code.extend(body);
        bytes.extend(section(10, code));
        let mut data = vec![1, 0, 0x41, 0, 0x0b, 8];
        data.extend(initial.to_le_bytes());
        bytes.extend(section(11, data));
        bytes
    }

    #[test]
    fn test_i64_atomic_rmw_variants_execute_on_shared_memory() {
        let cases = [
            (0x26, 1_i64, 2_i64, 1_i64, (-1_i64).to_le_bytes().to_vec()),
            (
                0x2d,
                0x0f0f0f0f0f0f0f0f_i64,
                0x00ff00ff00ff00ff_i64,
                0x0f0f0f0f0f0f0f0f_i64,
                0x000f000f000f000f_i64.to_le_bytes().to_vec(),
            ),
            (
                0x34,
                0x00ff00ff00ff00ff_i64,
                0x0f000f000f000f00_i64,
                0x00ff00ff00ff00ff_i64,
                0x0fff0fff0fff0fff_i64.to_le_bytes().to_vec(),
            ),
            (
                0x3b,
                -281470681808896_i64,
                0x0f0f0f0f0f0f0f0f_i64,
                -281470681808896_i64,
                (-1085350949055099121_i64).to_le_bytes().to_vec(),
            ),
            (
                0x42,
                0x0102030405060708_i64,
                -2_i64,
                0x0102030405060708_i64,
                (-2_i64).to_le_bytes().to_vec(),
            ),
        ];

        for (subopcode, initial, operand, result, bytes) in cases {
            let module_bytes = i64_atomic_rmw_module(subopcode, initial, operand);
            let module = parse_module(&module_bytes).expect("parse i64 atomic rmw variant module");
            let mut inst =
                instantiate(&module, Imports::new()).expect("instantiate i64 rmw variant");
            assert_eq!(
                inst.call("run", &[]).expect("run i64 rmw variant"),
                vec![WasmValue::I64(result)],
                "subopcode {subopcode:#x}"
            );
            assert_eq!(
                inst.memory_read(0, 8),
                Some(bytes),
                "subopcode {subopcode:#x}"
            );
        }
    }

    fn i64_atomic_cmpxchg_module(initial: i64, expected: i64, replacement: i64) -> Vec<u8> {
        let mut bytes = header();
        bytes.extend(section(1, vec![1, 0x60, 0, 1, 0x7e]));
        bytes.extend(section(3, vec![1, 0]));
        bytes.extend(section(5, vec![1, 0x03, 1, 1]));
        bytes.extend(section(7, vec![1, 3, b'r', b'u', b'n', 0, 0]));
        let mut body = vec![0, 0x41, 0, 0x42];
        sleb(expected, &mut body);
        body.push(0x42);
        sleb(replacement, &mut body);
        body.extend([0xfe, 0x49, 3, 0, 0x0b]);
        let mut code = vec![1];
        uleb(body.len() as u64, &mut code);
        code.extend(body);
        bytes.extend(section(10, code));
        let mut data = vec![1, 0, 0x41, 0, 0x0b, 8];
        data.extend(initial.to_le_bytes());
        bytes.extend(section(11, data));
        bytes
    }

    #[test]
    fn test_i64_atomic_cmpxchg_executes_on_shared_memory() {
        let cases = [
            (
                0x0102030405060708_i64,
                0x0102030405060708_i64,
                -2_i64,
                0x0102030405060708_i64,
                (-2_i64).to_le_bytes().to_vec(),
            ),
            (
                -2_i64,
                -3_i64,
                0x0102030405060708_i64,
                -2_i64,
                (-2_i64).to_le_bytes().to_vec(),
            ),
        ];

        for (initial, expected, replacement, result, bytes) in cases {
            let module_bytes = i64_atomic_cmpxchg_module(initial, expected, replacement);
            let module = parse_module(&module_bytes).expect("parse i64 atomic cmpxchg module");
            let mut inst = instantiate(&module, Imports::new()).expect("instantiate i64 cmpxchg");
            assert_eq!(
                inst.call("run", &[]).expect("run i64 cmpxchg"),
                vec![WasmValue::I64(result)]
            );
            assert_eq!(inst.memory_read(0, 8), Some(bytes));
        }
    }

    #[test]
    fn test_i32_atomic_narrow_load_store_executes_on_shared_memory() {
        let mut bytes = header();
        bytes.extend(section(
            1,
            vec![
                3,
                0x60, 0, 1, 0x7f,
                0x60, 0, 0,
                0x60, 0, 1, 0x7f,
            ],
        ));
        bytes.extend(section(3, vec![4, 0, 0, 1, 1]));
        bytes.extend(section(5, vec![1, 0x03, 1, 1]));
        bytes.extend(section(
            7,
            vec![
                4,
                5, b'l', b'o', b'a', b'd', b'8', 0, 0,
                6, b'l', b'o', b'a', b'd', b'1', b'6', 0, 1,
                6, b's', b't', b'o', b'r', b'e', b'8', 0, 2,
                7, b's', b't', b'o', b'r', b'e', b'1', b'6', 0, 3,
            ],
        ));
        bytes.extend(section(
            10,
            vec![
                4,
                8, 0, 0x41, 0, 0xfe, 0x12, 0, 0, 0x0b,
                8, 0, 0x41, 0, 0xfe, 0x13, 1, 0, 0x0b,
                11, 0, 0x41, 0, 0x41, 0xb4, 0x24, 0xfe, 0x19, 0, 0, 0x0b,
                12, 0, 0x41, 0, 0x41, 0xc5, 0xc6, 0x04, 0xfe, 0x1a, 1, 0, 0x0b,
            ],
        ));
        bytes.extend(section(11, vec![1, 0, 0x41, 0, 0x0b, 2, 0xff, 0x12]));

        let module = parse_module(&bytes).expect("parse atomic narrow module");
        let mut inst = instantiate(&module, Imports::new()).expect("instantiate atomic narrow");
        assert_eq!(
            inst.call("load8", &[]).expect("load8"),
            vec![WasmValue::I32(255)]
        );
        assert_eq!(
            inst.call("load16", &[]).expect("load16"),
            vec![WasmValue::I32(0x12ff)]
        );
        assert_eq!(
            inst.call("store8", &[]).expect("store8"),
            Vec::<WasmValue>::new()
        );
        assert_eq!(inst.memory_read(0, 4), Some(vec![0x34, 0x12, 0, 0]));
        assert_eq!(
            inst.call("store16", &[]).expect("store16"),
            Vec::<WasmValue>::new()
        );
        assert_eq!(inst.memory_read(0, 4), Some(vec![0x45, 0x23, 0, 0]));
    }

    #[test]
    fn test_i64_atomic_narrow_load_store_executes_on_shared_memory() {
        let mut bytes = header();
        bytes.extend(section(
            1,
            vec![
                3,
                0x60, 0, 1, 0x7e,
                0x60, 0, 0,
                0x60, 0, 1, 0x7e,
            ],
        ));
        bytes.extend(section(3, vec![6, 0, 0, 0, 1, 1, 1]));
        bytes.extend(section(5, vec![1, 0x03, 1, 1]));
        bytes.extend(section(
            7,
            vec![
                6,
                5, b'l', b'o', b'a', b'd', b'8', 0, 0,
                6, b'l', b'o', b'a', b'd', b'1', b'6', 0, 1,
                6, b'l', b'o', b'a', b'd', b'3', b'2', 0, 2,
                6, b's', b't', b'o', b'r', b'e', b'8', 0, 3,
                7, b's', b't', b'o', b'r', b'e', b'1', b'6', 0, 4,
                7, b's', b't', b'o', b'r', b'e', b'3', b'2', 0, 5,
            ],
        ));

        let mut store8_body = vec![0, 0x41, 0, 0x42];
        sleb(-2, &mut store8_body);
        store8_body.extend([0xfe, 0x1b, 0, 0, 0x0b]);

        let mut store16_body = vec![0, 0x41, 0, 0x42];
        sleb(0x0102030405060708, &mut store16_body);
        store16_body.extend([0xfe, 0x1c, 1, 0, 0x0b]);

        let mut store32_body = vec![0, 0x41, 0, 0x42];
        sleb(-2, &mut store32_body);
        store32_body.extend([0xfe, 0x1d, 2, 0, 0x0b]);

        let mut code = vec![6];
        for body in [
            vec![0, 0x41, 0, 0xfe, 0x14, 0, 0, 0x0b],
            vec![0, 0x41, 0, 0xfe, 0x15, 1, 0, 0x0b],
            vec![0, 0x41, 0, 0xfe, 0x16, 2, 0, 0x0b],
            store8_body,
            store16_body,
            store32_body,
        ] {
            uleb(body.len() as u64, &mut code);
            code.extend(body);
        }
        bytes.extend(section(10, code));
        bytes.extend(section(
            11,
            vec![1, 0, 0x41, 0, 0x0b, 4, 0xff, 0xff, 0xff, 0x80],
        ));

        let module = parse_module(&bytes).expect("parse i64 atomic narrow module");
        let mut inst = instantiate(&module, Imports::new()).expect("instantiate i64 narrow");
        assert_eq!(
            inst.call("load8", &[]).expect("load8"),
            vec![WasmValue::I64(255)]
        );
        assert_eq!(
            inst.call("load16", &[]).expect("load16"),
            vec![WasmValue::I64(65535)]
        );
        assert_eq!(
            inst.call("load32", &[]).expect("load32"),
            vec![WasmValue::I64(2164260863)]
        );
        assert_eq!(
            inst.call("store8", &[]).expect("store8"),
            Vec::<WasmValue>::new()
        );
        assert_eq!(
            inst.memory_read(0, 8),
            Some(vec![254, 255, 255, 128, 0, 0, 0, 0])
        );
        assert_eq!(
            inst.call("store16", &[]).expect("store16"),
            Vec::<WasmValue>::new()
        );
        assert_eq!(
            inst.memory_read(0, 8),
            Some(vec![8, 7, 255, 128, 0, 0, 0, 0])
        );
        assert_eq!(
            inst.call("store32", &[]).expect("store32"),
            Vec::<WasmValue>::new()
        );
        assert_eq!(
            inst.memory_read(0, 8),
            Some(vec![254, 255, 255, 255, 0, 0, 0, 0])
        );
    }

    fn i64_atomic_narrow_rmw_module(
        subopcode: u8,
        align: u8,
        initial: u32,
        operand: i64,
        width: usize,
    ) -> Vec<u8> {
        let mut bytes = header();
        bytes.extend(section(1, vec![1, 0x60, 0, 1, 0x7e]));
        bytes.extend(section(3, vec![1, 0]));
        bytes.extend(section(5, vec![1, 0x03, 1, 1]));
        bytes.extend(section(7, vec![1, 3, b'r', b'u', b'n', 0, 0]));
        let mut body = vec![0, 0x41, 0, 0x42];
        sleb(operand, &mut body);
        body.extend([0xfe, subopcode, align, 0, 0x0b]);
        let mut code = vec![1];
        uleb(body.len() as u64, &mut code);
        code.extend(body);
        bytes.extend(section(10, code));
        let mut data = vec![1, 0, 0x41, 0, 0x0b, width as u8];
        data.extend(&initial.to_le_bytes()[..width]);
        bytes.extend(section(11, data));
        bytes
    }

    #[test]
    fn test_i64_atomic_narrow_rmw_variants_execute_on_shared_memory() {
        let cases = [
            (
                0x22,
                0,
                0xfe,
                0x05,
                1,
                254_i64,
                vec![3, 0, 0, 0, 0, 0, 0, 0],
            ),
            (
                0x23,
                1,
                0xfffe,
                0x05,
                2,
                65534_i64,
                vec![3, 0, 0, 0, 0, 0, 0, 0],
            ),
            (
                0x24,
                2,
                0xfffffffe,
                0x05,
                4,
                4294967294_i64,
                vec![3, 0, 0, 0, 0, 0, 0, 0],
            ),
            (
                0x29,
                0,
                0x02,
                0x05,
                1,
                2_i64,
                vec![253, 0, 0, 0, 0, 0, 0, 0],
            ),
            (
                0x2a,
                1,
                0x0002,
                0x05,
                2,
                2_i64,
                vec![253, 255, 0, 0, 0, 0, 0, 0],
            ),
            (
                0x2b,
                2,
                0x00000002,
                0x05,
                4,
                2_i64,
                vec![253, 255, 255, 255, 0, 0, 0, 0],
            ),
            (
                0x30,
                0,
                0xf3,
                0x0f,
                1,
                243_i64,
                vec![3, 0, 0, 0, 0, 0, 0, 0],
            ),
            (
                0x31,
                1,
                0xff33,
                0x0ff0,
                2,
                65331_i64,
                vec![48, 15, 0, 0, 0, 0, 0, 0],
            ),
            (
                0x32,
                2,
                0xffff0033,
                0x0ffff0f0,
                4,
                4294901811_i64,
                vec![48, 0, 255, 15, 0, 0, 0, 0],
            ),
            (
                0x37,
                0,
                0xf0,
                0x0f,
                1,
                240_i64,
                vec![255, 0, 0, 0, 0, 0, 0, 0],
            ),
            (
                0x38,
                1,
                0xf000,
                0x0f0f,
                2,
                61440_i64,
                vec![15, 255, 0, 0, 0, 0, 0, 0],
            ),
            (
                0x39,
                2,
                0xf0000000,
                0x0f0f0f0f,
                4,
                4026531840_i64,
                vec![15, 15, 15, 255, 0, 0, 0, 0],
            ),
            (
                0x3e,
                0,
                0xf0,
                0xff,
                1,
                240_i64,
                vec![15, 0, 0, 0, 0, 0, 0, 0],
            ),
            (
                0x3f,
                1,
                0xf0f0,
                0x0fff,
                2,
                61680_i64,
                vec![15, 255, 0, 0, 0, 0, 0, 0],
            ),
            (
                0x40,
                2,
                0xf0f0f0f0,
                0x0fffffff,
                4,
                4042322160_i64,
                vec![15, 15, 15, 255, 0, 0, 0, 0],
            ),
            (
                0x45,
                0,
                0xaa,
                0x55,
                1,
                170_i64,
                vec![85, 0, 0, 0, 0, 0, 0, 0],
            ),
            (
                0x46,
                1,
                0xaabb,
                0x1234,
                2,
                43707_i64,
                vec![52, 18, 0, 0, 0, 0, 0, 0],
            ),
            (
                0x47,
                2,
                0xaabbccdd,
                0x12345678,
                4,
                2864434397_i64,
                vec![120, 86, 52, 18, 0, 0, 0, 0],
            ),
        ];

        for (subopcode, align, initial, operand, width, result, bytes) in cases {
            let module_bytes =
                i64_atomic_narrow_rmw_module(subopcode, align, initial, operand, width);
            let module =
                parse_module(&module_bytes).expect("parse i64 atomic narrow rmw variant module");
            let mut inst =
                instantiate(&module, Imports::new()).expect("instantiate i64 narrow rmw variant");
            assert_eq!(
                inst.call("run", &[]).expect("run i64 narrow rmw variant"),
                vec![WasmValue::I64(result)],
                "subopcode {subopcode:#x}"
            );
            assert_eq!(
                inst.memory_read(0, 8),
                Some(bytes),
                "subopcode {subopcode:#x}"
            );
        }
    }

    fn i64_atomic_narrow_cmpxchg_module(
        subopcode: u8,
        align: u8,
        initial: u32,
        expected: i64,
        replacement: i64,
        width: usize,
    ) -> Vec<u8> {
        let mut bytes = header();
        bytes.extend(section(1, vec![1, 0x60, 0, 1, 0x7e]));
        bytes.extend(section(3, vec![1, 0]));
        bytes.extend(section(5, vec![1, 0x03, 1, 1]));
        bytes.extend(section(7, vec![1, 3, b'r', b'u', b'n', 0, 0]));
        let mut body = vec![0, 0x41, 0, 0x42];
        sleb(expected, &mut body);
        body.push(0x42);
        sleb(replacement, &mut body);
        body.extend([0xfe, subopcode, align, 0, 0x0b]);
        let mut code = vec![1];
        uleb(body.len() as u64, &mut code);
        code.extend(body);
        bytes.extend(section(10, code));
        let mut data = vec![1, 0, 0x41, 0, 0x0b, width as u8];
        data.extend(&initial.to_le_bytes()[..width]);
        bytes.extend(section(11, data));
        bytes
    }

    #[test]
    fn test_i64_atomic_narrow_cmpxchg_executes_on_shared_memory() {
        let cases = [
            (
                0x4c,
                0,
                0xaa,
                0xaa,
                0x55,
                1,
                170_i64,
                vec![85, 0, 0, 0, 0, 0, 0, 0],
            ),
            (
                0x4c,
                0,
                0xaa,
                0xbb,
                0x55,
                1,
                170_i64,
                vec![170, 0, 0, 0, 0, 0, 0, 0],
            ),
            (
                0x4d,
                1,
                0xaabb,
                0xaabb,
                0x1234,
                2,
                43707_i64,
                vec![52, 18, 0, 0, 0, 0, 0, 0],
            ),
            (
                0x4d,
                1,
                0xaabb,
                0xbbcc,
                0x1234,
                2,
                43707_i64,
                vec![187, 170, 0, 0, 0, 0, 0, 0],
            ),
            (
                0x4e,
                2,
                0xaabbccdd,
                0xaabbccdd,
                0x12345678,
                4,
                2864434397_i64,
                vec![120, 86, 52, 18, 0, 0, 0, 0],
            ),
            (
                0x4e,
                2,
                0xaabbccdd,
                0xbbccddee,
                0x12345678,
                4,
                2864434397_i64,
                vec![221, 204, 187, 170, 0, 0, 0, 0],
            ),
        ];

        for (subopcode, align, initial, expected, replacement, width, result, bytes) in cases {
            let module_bytes = i64_atomic_narrow_cmpxchg_module(
                subopcode,
                align,
                initial,
                expected,
                replacement,
                width,
            );
            let module =
                parse_module(&module_bytes).expect("parse i64 atomic narrow cmpxchg module");
            let mut inst =
                instantiate(&module, Imports::new()).expect("instantiate i64 narrow cmpxchg");
            assert_eq!(
                inst.call("run", &[]).expect("run i64 narrow cmpxchg"),
                vec![WasmValue::I64(result)],
                "subopcode {subopcode:#x}"
            );
            assert_eq!(
                inst.memory_read(0, 8),
                Some(bytes),
                "subopcode {subopcode:#x}"
            );
        }
    }

    #[test]
    fn test_atomic_fence_executes_on_shared_memory() {
        let mut bytes = header();
        bytes.extend(section(1, vec![1, 0x60, 0, 1, 0x7f]));
        bytes.extend(section(3, vec![1, 0]));
        bytes.extend(section(5, vec![1, 0x03, 1, 1]));
        bytes.extend(section(7, vec![1, 3, b'r', b'u', b'n', 0, 0]));
        bytes.extend(section(10, vec![1, 7, 0, 0xfe, 0x03, 0x00, 0x41, 42, 0x0b]));

        let module = parse_module(&bytes).expect("parse atomic fence module");
        let mut inst = instantiate(&module, Imports::new()).expect("instantiate atomic fence");
        assert_eq!(
            inst.call("run", &[]).expect("run atomic fence"),
            vec![WasmValue::I32(42)]
        );

        let mut bad = header();
        bad.extend(section(1, vec![1, 0x60, 0, 1, 0x7f]));
        bad.extend(section(3, vec![1, 0]));
        bad.extend(section(5, vec![1, 0x03, 1, 1]));
        bad.extend(section(10, vec![1, 7, 0, 0xfe, 0x03, 0x01, 0x41, 42, 0x0b]));
        let err = match parse_module(&bad) {
            Ok(_) => panic!("nonzero fence operand accepted"),
            Err(err) => err,
        };
        assert!(err.contains("invalid atomic operand"), "{err}");
    }

    #[test]
    fn test_i32_atomic_rmw_add_executes_on_shared_memory() {
        let mut bytes = header();
        bytes.extend(section(1, vec![1, 0x60, 0, 1, 0x7f]));
        bytes.extend(section(3, vec![1, 0]));
        bytes.extend(section(5, vec![1, 0x03, 1, 1]));
        bytes.extend(section(7, vec![1, 3, b'r', b'u', b'n', 0, 0]));
        bytes.extend(section(
            10,
            vec![
                1, 18, 0, 0x41, 0, 0x41, 40, 0xfe, 0x17, 2, 0, 0x41, 0, 0x41, 2, 0xfe, 0x1e, 2, 0,
                0x0b,
            ],
        ));

        let module = parse_module(&bytes).expect("parse atomic rmw module");
        let mut inst = instantiate(&module, Imports::new()).expect("instantiate atomic rmw module");
        assert_eq!(
            inst.call("run", &[]).expect("run rmw.add"),
            vec![WasmValue::I32(40)]
        );
        assert_eq!(inst.memory_read(0, 4), Some(vec![42, 0, 0, 0]));
    }

    fn i32_atomic_narrow_rmw_add_module(
        subopcode: u8,
        align: u8,
        seed: &[u8],
        operand: u32,
    ) -> Vec<u8> {
        let mut bytes = header();
        bytes.extend(section(1, vec![1, 0x60, 0, 1, 0x7f]));
        bytes.extend(section(3, vec![1, 0]));
        bytes.extend(section(5, vec![1, 0x03, 1, 1]));
        bytes.extend(section(7, vec![1, 3, b'r', b'u', b'n', 0, 0]));
        let mut body = vec![0, 0x41, 0, 0x41];
        sleb(operand as i64, &mut body);
        body.extend([0xfe, subopcode, align, 0, 0x0b]);
        let mut code = vec![1];
        uleb(body.len() as u64, &mut code);
        code.extend(body);
        bytes.extend(section(10, code));
        let mut data = vec![1, 0, 0x41, 0, 0x0b];
        uleb(seed.len() as u64, &mut data);
        data.extend(seed);
        bytes.extend(section(11, data));
        bytes
    }

    #[test]
    fn test_i32_atomic_narrow_rmw_add_executes_on_shared_memory() {
        let cases = [
            (0x20, 0, vec![0xfe], 5, 254, vec![3, 0, 0, 0]),
            (0x21, 1, vec![0xfe, 0xff], 5, 65534, vec![3, 0, 0, 0]),
        ];

        for (subopcode, align, seed, operand, result, bytes) in cases {
            let module_bytes = i32_atomic_narrow_rmw_add_module(subopcode, align, &seed, operand);
            let module = parse_module(&module_bytes).expect("parse narrow atomic rmw.add module");
            let mut inst =
                instantiate(&module, Imports::new()).expect("instantiate narrow rmw.add");
            assert_eq!(
                inst.call("run", &[]).expect("run narrow rmw.add"),
                vec![WasmValue::I32(result)],
                "subopcode {subopcode:#x}"
            );
            assert_eq!(
                inst.memory_read(0, 4),
                Some(bytes),
                "subopcode {subopcode:#x}"
            );
        }
    }

    #[test]
    fn test_i32_atomic_narrow_rmw_variants_execute_on_shared_memory() {
        let cases = [
            (0x27, 0, vec![0x02], 0x05, 2, vec![253, 0, 0, 0]),
            (0x28, 1, vec![0x02, 0x00], 0x05, 2, vec![253, 255, 0, 0]),
            (0x2e, 0, vec![0xf3], 0x0f, 243, vec![3, 0, 0, 0]),
            (0x2f, 1, vec![0x33, 0xff], 0x0ff0, 65331, vec![48, 15, 0, 0]),
            (0x35, 0, vec![0xf0], 0x0f, 240, vec![255, 0, 0, 0]),
            (
                0x36,
                1,
                vec![0x00, 0xf0],
                0x0f0f,
                61440,
                vec![15, 255, 0, 0],
            ),
            (0x3c, 0, vec![0xf0], 0xff, 240, vec![15, 0, 0, 0]),
            (
                0x3d,
                1,
                vec![0xf0, 0xf0],
                0x0fff,
                61680,
                vec![15, 255, 0, 0],
            ),
            (0x43, 0, vec![0xaa], 0x55, 170, vec![85, 0, 0, 0]),
            (0x44, 1, vec![0xbb, 0xaa], 0x1234, 43707, vec![52, 18, 0, 0]),
        ];

        for (subopcode, align, seed, operand, result, bytes) in cases {
            let module_bytes = i32_atomic_narrow_rmw_add_module(subopcode, align, &seed, operand);
            let module =
                parse_module(&module_bytes).expect("parse narrow atomic rmw variant module");
            let mut inst =
                instantiate(&module, Imports::new()).expect("instantiate narrow rmw variant");
            assert_eq!(
                inst.call("run", &[]).expect("run narrow rmw variant"),
                vec![WasmValue::I32(result)],
                "subopcode {subopcode:#x}"
            );
            assert_eq!(
                inst.memory_read(0, 4),
                Some(bytes),
                "subopcode {subopcode:#x}"
            );
        }
    }

    fn i32_atomic_rmw_module(subopcode: u8, initial: u8, operand: u8) -> Vec<u8> {
        let mut bytes = header();
        bytes.extend(section(1, vec![1, 0x60, 0, 1, 0x7f]));
        bytes.extend(section(3, vec![1, 0]));
        bytes.extend(section(5, vec![1, 0x03, 1, 1]));
        bytes.extend(section(7, vec![1, 3, b'r', b'u', b'n', 0, 0]));
        bytes.extend(section(
            10,
            vec![
                1, 18, 0, 0x41, 0, 0x41, initial, 0xfe, 0x17, 2, 0, 0x41, 0, 0x41, operand, 0xfe,
                subopcode, 2, 0, 0x0b,
            ],
        ));
        bytes
    }

    #[test]
    fn test_i32_atomic_rmw_variants_execute_on_shared_memory() {
        let cases = [
            (0x25, 40, 2, 40, vec![38, 0, 0, 0]),
            (0x2c, 42, 10, 42, vec![10, 0, 0, 0]),
            (0x33, 40, 2, 40, vec![42, 0, 0, 0]),
            (0x3a, 40, 2, 40, vec![42, 0, 0, 0]),
            (0x41, 40, 2, 40, vec![2, 0, 0, 0]),
        ];

        for (subopcode, initial, operand, result, bytes) in cases {
            let module_bytes = i32_atomic_rmw_module(subopcode, initial, operand);
            let module = parse_module(&module_bytes).expect("parse atomic rmw variant module");
            let mut inst =
                instantiate(&module, Imports::new()).expect("instantiate atomic rmw variant");
            assert_eq!(
                inst.call("run", &[]).expect("run rmw variant"),
                vec![WasmValue::I32(result)]
            );
            assert_eq!(
                inst.memory_read(0, 4),
                Some(bytes),
                "subopcode {subopcode:#x}"
            );
        }
    }

    fn i32_atomic_cmpxchg_module(initial: u8, expected: u8, replacement: u8) -> Vec<u8> {
        let mut bytes = header();
        bytes.extend(section(1, vec![1, 0x60, 0, 1, 0x7f]));
        bytes.extend(section(3, vec![1, 0]));
        bytes.extend(section(5, vec![1, 0x03, 1, 1]));
        bytes.extend(section(7, vec![1, 3, b'r', b'u', b'n', 0, 0]));
        bytes.extend(section(
            10,
            vec![
                1,
                20,
                0,
                0x41,
                0,
                0x41,
                initial,
                0xfe,
                0x17,
                2,
                0,
                0x41,
                0,
                0x41,
                expected,
                0x41,
                replacement,
                0xfe,
                0x48,
                2,
                0,
                0x0b,
            ],
        ));
        bytes
    }

    #[test]
    fn test_i32_atomic_cmpxchg_executes_on_shared_memory() {
        let cases = [
            (40, 40, 7, 40, vec![7, 0, 0, 0]),
            (40, 41, 7, 40, vec![40, 0, 0, 0]),
        ];

        for (initial, expected, replacement, result, bytes) in cases {
            let module_bytes = i32_atomic_cmpxchg_module(initial, expected, replacement);
            let module = parse_module(&module_bytes).expect("parse atomic cmpxchg module");
            let mut inst = instantiate(&module, Imports::new()).expect("instantiate cmpxchg");
            assert_eq!(
                inst.call("run", &[]).expect("run cmpxchg"),
                vec![WasmValue::I32(result)]
            );
            assert_eq!(inst.memory_read(0, 4), Some(bytes));
        }
    }

    fn i32_atomic_narrow_cmpxchg_module(
        subopcode: u8,
        align: u8,
        seed: &[u8],
        expected: u32,
        replacement: u32,
    ) -> Vec<u8> {
        let mut bytes = header();
        bytes.extend(section(1, vec![1, 0x60, 0, 1, 0x7f]));
        bytes.extend(section(3, vec![1, 0]));
        bytes.extend(section(5, vec![1, 0x03, 1, 1]));
        bytes.extend(section(7, vec![1, 3, b'r', b'u', b'n', 0, 0]));
        let mut body = vec![0, 0x41, 0, 0x41];
        sleb(expected as i64, &mut body);
        body.push(0x41);
        sleb(replacement as i64, &mut body);
        body.extend([0xfe, subopcode, align, 0, 0x0b]);
        let mut code = vec![1];
        uleb(body.len() as u64, &mut code);
        code.extend(body);
        bytes.extend(section(10, code));
        let mut data = vec![1, 0, 0x41, 0, 0x0b];
        uleb(seed.len() as u64, &mut data);
        data.extend(seed);
        bytes.extend(section(11, data));
        bytes
    }

    #[test]
    fn test_i32_atomic_narrow_cmpxchg_executes_on_shared_memory() {
        let cases = [
            (0x4a, 0, vec![0xaa], 0xaa, 0x55, 170, vec![85, 0, 0, 0]),
            (0x4a, 0, vec![0xaa], 0xab, 0x55, 170, vec![170, 0, 0, 0]),
            (
                0x4b,
                1,
                vec![0xbb, 0xaa],
                0xaabb,
                0x1234,
                43707,
                vec![52, 18, 0, 0],
            ),
            (
                0x4b,
                1,
                vec![0xbb, 0xaa],
                0xaabc,
                0x1234,
                43707,
                vec![187, 170, 0, 0],
            ),
        ];

        for (subopcode, align, seed, expected, replacement, result, bytes) in cases {
            let module_bytes =
                i32_atomic_narrow_cmpxchg_module(subopcode, align, &seed, expected, replacement);
            let module = parse_module(&module_bytes).expect("parse narrow cmpxchg module");
            let mut inst =
                instantiate(&module, Imports::new()).expect("instantiate narrow cmpxchg");
            assert_eq!(
                inst.call("run", &[]).expect("run narrow cmpxchg"),
                vec![WasmValue::I32(result)],
                "subopcode {subopcode:#x}"
            );
            assert_eq!(
                inst.memory_read(0, 4),
                Some(bytes),
                "subopcode {subopcode:#x}"
            );
        }
    }

    #[test]
    fn test_return_call_executes_as_returning_call() {
        let mut bytes = header();
        bytes.extend(section(1, vec![1, 0x60, 0, 1, 0x7f]));
        bytes.extend(section(3, vec![2, 0, 0]));
        bytes.extend(section(7, vec![1, 3, b'r', b'u', b'n', 0, 1]));
        bytes.extend(section(
            10,
            vec![
                2,
                4, 0, 0x41, 9, 0x0b,
                4, 0, 0x12, 0, 0x0b,
            ],
        ));

        let module = parse_module(&bytes).expect("parse return_call module");
        let mut inst = instantiate(&module, Imports::new()).expect("instantiate return_call");
        let result = inst.call("run", &[]).expect("call run");
        assert_eq!(result, vec![WasmValue::I32(9)]);
    }

    #[test]
    fn test_return_call_indirect_executes_as_tail_call() {
        let mut bytes = header();
        bytes.extend(section(1, vec![1, 0x60, 0, 1, 0x7f]));
        bytes.extend(section(3, vec![2, 0, 0]));
        bytes.extend(section(4, vec![1, 0x70, 0, 1]));
        bytes.extend(section(7, vec![1, 3, b'r', b'u', b'n', 0, 1]));
        bytes.extend(section(9, vec![1, 0, 0x41, 0, 0x0b, 1, 0]));
        bytes.extend(section(
            10,
            vec![
                2,
                4, 0, 0x41, 9, 0x0b,
                7, 0, 0x41, 0, 0x13, 0, 0, 0x0b,
            ],
        ));

        let module = parse_module(&bytes).expect("parse return_call_indirect module");
        let mut inst =
            instantiate(&module, Imports::new()).expect("instantiate return_call_indirect");
        let result = inst.call("run", &[]).expect("call run");
        assert_eq!(result, vec![WasmValue::I32(9)]);
    }

    #[test]
    fn test_return_call_tail_recursion_does_not_consume_call_depth() {
        let mut bytes = header();
        bytes.extend(section(1, vec![1, 0x60, 1, 0x7f, 1, 0x7f]));
        bytes.extend(section(3, vec![1, 0]));
        bytes.extend(section(7, vec![1, 3, b'r', b'u', b'n', 0, 0]));
        let body = vec![
            0,
            0x20, 0,
            0x45,
            0x04, 0x7f,
            0x41, 7,
            0x05,
            0x20, 0,
            0x41, 1,
            0x6b,
            0x12, 0,
            0x0b,
            0x0b,
        ];
        let mut code = vec![1];
        uleb(body.len() as u64, &mut code);
        code.extend(body);
        bytes.extend(section(10, code));

        let module = parse_module(&bytes).expect("parse recursive return_call module");
        let mut inst =
            instantiate(&module, Imports::new()).expect("instantiate recursive return_call");
        let result = inst
            .call("run", &[WasmValue::I32(20_000)])
            .expect("tail-recursive return_call should not exhaust call depth");
        assert_eq!(result, vec![WasmValue::I32(7)]);
    }

    #[test]
    fn test_exception_throw_catches_try_table_payload() {
        let bytes: &[u8] = &[
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x23, 0x08, 0x60, 0x00, 0x00,
            0x60, 0x01, 0x7f, 0x00, 0x60, 0x01, 0x7d, 0x00, 0x60, 0x01, 0x7e, 0x00, 0x60, 0x01,
            0x7c, 0x00, 0x60, 0x02, 0x7f, 0x7f, 0x00, 0x60, 0x01, 0x7f, 0x01, 0x7f, 0x60, 0x00,
            0x02, 0x7f, 0x7f, 0x03, 0x09, 0x08, 0x06, 0x02, 0x03, 0x04, 0x00, 0x00, 0x00, 0x00,
            0x0d, 0x0d, 0x06, 0x00, 0x00, 0x00, 0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x04, 0x00,
            0x05, 0x07, 0x81, 0x01, 0x07, 0x08, 0x74, 0x68, 0x72, 0x6f, 0x77, 0x2d, 0x69, 0x66,
            0x00, 0x00, 0x0f, 0x74, 0x68, 0x72, 0x6f, 0x77, 0x2d, 0x70, 0x61, 0x72, 0x61, 0x6d,
            0x2d, 0x66, 0x33, 0x32, 0x00, 0x01, 0x0f, 0x74, 0x68, 0x72, 0x6f, 0x77, 0x2d, 0x70,
            0x61, 0x72, 0x61, 0x6d, 0x2d, 0x69, 0x36, 0x34, 0x00, 0x02, 0x0f, 0x74, 0x68, 0x72,
            0x6f, 0x77, 0x2d, 0x70, 0x61, 0x72, 0x61, 0x6d, 0x2d, 0x66, 0x36, 0x34, 0x00, 0x03,
            0x11, 0x74, 0x68, 0x72, 0x6f, 0x77, 0x2d, 0x70, 0x6f, 0x6c, 0x79, 0x6d, 0x6f, 0x72,
            0x70, 0x68, 0x69, 0x63, 0x00, 0x04, 0x17, 0x74, 0x68, 0x72, 0x6f, 0x77, 0x2d, 0x70,
            0x6f, 0x6c, 0x79, 0x6d, 0x6f, 0x72, 0x70, 0x68, 0x69, 0x63, 0x2d, 0x62, 0x6c, 0x6f,
            0x63, 0x6b, 0x00, 0x05, 0x0e, 0x74, 0x65, 0x73, 0x74, 0x2d, 0x74, 0x68, 0x72, 0x6f,
            0x77, 0x2d, 0x31, 0x2d, 0x32, 0x00, 0x07, 0x0a, 0x5d, 0x08, 0x0e, 0x00, 0x20, 0x00,
            0x41, 0x00, 0x47, 0x04, 0x40, 0x08, 0x00, 0x0b, 0x41, 0x00, 0x0b, 0x06, 0x00, 0x20,
            0x00, 0x08, 0x02, 0x0b, 0x06, 0x00, 0x20, 0x00, 0x08, 0x03, 0x0b, 0x06, 0x00, 0x20,
            0x00, 0x08, 0x04, 0x0b, 0x06, 0x00, 0x08, 0x00, 0x08, 0x01, 0x0b, 0x09, 0x00, 0x02,
            0x7f, 0x08, 0x00, 0x0b, 0x08, 0x01, 0x0b, 0x08, 0x00, 0x41, 0x01, 0x41, 0x02, 0x08,
            0x05, 0x0b, 0x1d, 0x00, 0x02, 0x07, 0x1f, 0x40, 0x01, 0x00, 0x05, 0x00, 0x10, 0x06,
            0x0b, 0x0f, 0x0b, 0x41, 0x02, 0x47, 0x04, 0x40, 0x00, 0x0b, 0x41, 0x01, 0x47, 0x04,
            0x40, 0x00, 0x0b, 0x0b,
        ];
        let module = parse_module(bytes).expect("parse exceptions/throw command-0 module");
        let mut inst = instantiate(&module, Imports::new()).expect("instantiate exception module");
        let result = inst
            .call("test-throw-1-2", &[])
            .expect("try_table should catch the thrown payload");
        assert_eq!(result, Vec::<WasmValue>::new());
    }

    #[test]
    fn test_exception_ref_catch_and_rethrow_paths() {
        let bytes: &[u8] = &[
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x0d, 0x03, 0x60, 0x00, 0x00,
            0x60, 0x01, 0x7f, 0x01, 0x7f, 0x60, 0x01, 0x69, 0x00, 0x03, 0x08, 0x07, 0x00, 0x01,
            0x00, 0x01, 0x01, 0x01, 0x00, 0x0d, 0x05, 0x02, 0x00, 0x00, 0x00, 0x00, 0x07, 0x9d,
            0x01, 0x07, 0x11, 0x63, 0x61, 0x74, 0x63, 0x68, 0x2d, 0x74, 0x68, 0x72, 0x6f, 0x77,
            0x5f, 0x72, 0x65, 0x66, 0x2d, 0x30, 0x00, 0x00, 0x11, 0x63, 0x61, 0x74, 0x63, 0x68,
            0x2d, 0x74, 0x68, 0x72, 0x6f, 0x77, 0x5f, 0x72, 0x65, 0x66, 0x2d, 0x31, 0x00, 0x01,
            0x14, 0x63, 0x61, 0x74, 0x63, 0x68, 0x61, 0x6c, 0x6c, 0x2d, 0x74, 0x68, 0x72, 0x6f,
            0x77, 0x5f, 0x72, 0x65, 0x66, 0x2d, 0x30, 0x00, 0x02, 0x14, 0x63, 0x61, 0x74, 0x63,
            0x68, 0x61, 0x6c, 0x6c, 0x2d, 0x74, 0x68, 0x72, 0x6f, 0x77, 0x5f, 0x72, 0x65, 0x66,
            0x2d, 0x31, 0x00, 0x03, 0x10, 0x74, 0x68, 0x72, 0x6f, 0x77, 0x5f, 0x72, 0x65, 0x66,
            0x2d, 0x6e, 0x65, 0x73, 0x74, 0x65, 0x64, 0x00, 0x04, 0x11, 0x74, 0x68, 0x72, 0x6f,
            0x77, 0x5f, 0x72, 0x65, 0x66, 0x2d, 0x72, 0x65, 0x63, 0x61, 0x74, 0x63, 0x68, 0x00,
            0x05, 0x1c, 0x74, 0x68, 0x72, 0x6f, 0x77, 0x5f, 0x72, 0x65, 0x66, 0x2d, 0x73, 0x74,
            0x61, 0x63, 0x6b, 0x2d, 0x70, 0x6f, 0x6c, 0x79, 0x6d, 0x6f, 0x72, 0x70, 0x68, 0x69,
            0x73, 0x6d, 0x00, 0x06, 0x0a, 0xd7, 0x01, 0x07, 0x10, 0x00, 0x02, 0x69, 0x1f, 0x40,
            0x01, 0x01, 0x00, 0x00, 0x08, 0x00, 0x0b, 0x00, 0x0b, 0x0a, 0x0b, 0x1a, 0x00, 0x02,
            0x69, 0x1f, 0x7f, 0x01, 0x01, 0x00, 0x00, 0x08, 0x00, 0x0b, 0x0f, 0x0b, 0x20, 0x00,
            0x45, 0x04, 0x02, 0x0a, 0x05, 0x1a, 0x0b, 0x41, 0x17, 0x0b, 0x0e, 0x00, 0x02, 0x69,
            0x1f, 0x69, 0x01, 0x03, 0x00, 0x08, 0x00, 0x0b, 0x0b, 0x0a, 0x0b, 0x19, 0x00, 0x02,
            0x69, 0x1f, 0x7f, 0x01, 0x03, 0x00, 0x08, 0x00, 0x0b, 0x0f, 0x0b, 0x20, 0x00, 0x45,
            0x04, 0x02, 0x0a, 0x05, 0x1a, 0x0b, 0x41, 0x17, 0x0b, 0x3a, 0x01, 0x02, 0x69, 0x02,
            0x69, 0x1f, 0x7f, 0x01, 0x01, 0x01, 0x00, 0x08, 0x01, 0x0b, 0x0f, 0x0b, 0x21, 0x01,
            0x02, 0x69, 0x1f, 0x7f, 0x01, 0x01, 0x00, 0x00, 0x08, 0x00, 0x0b, 0x0f, 0x0b, 0x21,
            0x02, 0x20, 0x00, 0x41, 0x00, 0x46, 0x04, 0x40, 0x20, 0x01, 0x0a, 0x0b, 0x20, 0x00,
            0x41, 0x01, 0x46, 0x04, 0x40, 0x20, 0x02, 0x0a, 0x0b, 0x41, 0x17, 0x0b, 0x2c, 0x01,
            0x01, 0x69, 0x02, 0x69, 0x1f, 0x7f, 0x01, 0x01, 0x00, 0x00, 0x08, 0x00, 0x0b, 0x0f,
            0x0b, 0x21, 0x01, 0x02, 0x69, 0x1f, 0x7f, 0x01, 0x01, 0x00, 0x00, 0x20, 0x00, 0x45,
            0x04, 0x40, 0x20, 0x01, 0x0a, 0x0b, 0x41, 0x2a, 0x0b, 0x0f, 0x0b, 0x1a, 0x41, 0x17,
            0x0b, 0x18, 0x01, 0x01, 0x69, 0x02, 0x69, 0x1f, 0x7c, 0x01, 0x01, 0x00, 0x00, 0x08,
            0x00, 0x0b, 0x00, 0x0b, 0x21, 0x00, 0x41, 0x01, 0x20, 0x00, 0x0a, 0x0b,
        ];
        let module = parse_module(bytes).expect("parse exceptions/throw_ref command-0 module");
        let mut inst = instantiate(&module, Imports::new()).expect("instantiate throw_ref module");
        assert_eq!(
            inst.call("catch-throw_ref-1", &[WasmValue::I32(1)])
                .expect("catch_ref drop path"),
            vec![WasmValue::I32(23)]
        );
        assert_eq!(
            inst.call("catchall-throw_ref-1", &[WasmValue::I32(1)])
                .expect("catch_all_ref drop path"),
            vec![WasmValue::I32(23)]
        );
        assert_eq!(
            inst.call("throw_ref-recatch", &[WasmValue::I32(0)])
                .expect("throw_ref should recatch stored exception"),
            vec![WasmValue::I32(23)]
        );
    }

    #[test]
    fn test_export_func_sig_add() {
        let bytes: Vec<u8> = vec![
            0, 97, 115, 109, 1, 0, 0, 0, 1, 7, 1, 96, 2, 127, 127, 1, 127, 3, 2, 1, 0, 7, 7, 1, 3,
            97, 100, 100, 0, 0, 10, 9, 1, 7, 0, 32, 0, 32, 1, 106, 11,
        ];
        let m = parse_module(&bytes).expect("parse add");
        let inst = instantiate(&m, Imports::new()).expect("instantiate add");
        assert_eq!(
            inst.export_func_sig("add"),
            Some((vec![ValType::I32, ValType::I32], vec![ValType::I32]))
        );
        assert_eq!(
            inst.export_func_specs(),
            vec![(
                "add".to_string(),
                0,
                vec![ValType::I32, ValType::I32],
                vec![ValType::I32],
                false,
                true,
                "group[0]{func:final:sup[]:(I32,I32)->(I32)}".to_string(),
                vec!["group[0]{func:final:sup[]:(I32,I32)->(I32)}".to_string()]
            )]
        );
        assert!(!inst.func_may_write_memory(0));
        assert_eq!(inst.export_func_sig("nonexistent"), None);

        assert_eq!(WasmValue::I32(5).as_f64(), 5.0);
        assert_eq!(WasmValue::F64(3.75).as_f64(), 3.75);
        assert_eq!(WasmValue::I32(5).val_type(), ValType::I32);
        assert_eq!(WasmValue::F32(1.0).val_type(), ValType::F32);
    }

    #[test]
    fn test_func_may_write_memory_detects_store() {
        let mut bytes = header();
        bytes.extend(section(1, vec![1, 0x60, 0, 0]));
        bytes.extend(section(3, vec![1, 0]));
        bytes.extend(section(5, vec![1, 0, 1]));
        bytes.extend(section(7, vec![1, 3, b'r', b'u', b'n', 0, 0]));
        let body = vec![0, 0x41, 0, 0x41, 7, 0x36, 2, 0, 0x0b];
        let mut code = vec![1];
        uleb(body.len() as u64, &mut code);
        code.extend(body);
        bytes.extend(section(10, code));
        let module = parse_module(&bytes).expect("parse store module");
        let inst = instantiate(&module, Imports::new()).expect("instantiate store module");
        assert!(inst.func_may_write_memory(0));
        assert_eq!(inst.export_func_specs()[0].4, true);
    }

    #[test]
    fn test_factorial_loop() {

        let mut typesec = Vec::new();
        uleb(1, &mut typesec);
        typesec.push(0x60);
        uleb(1, &mut typesec);
        typesec.push(0x7f);
        uleb(1, &mut typesec);
        typesec.push(0x7f);

        let mut funcsec = Vec::new();
        uleb(1, &mut funcsec);
        uleb(0, &mut funcsec);

        let mut exportsec = Vec::new();
        uleb(1, &mut exportsec);
        let name = b"fac";
        uleb(name.len() as u64, &mut exportsec);
        exportsec.extend_from_slice(name);
        exportsec.push(0x00);
        uleb(0, &mut exportsec);

        let mut body = Vec::new();

        uleb(1, &mut body);
        uleb(2, &mut body);
        body.push(0x7f);

        body.push(0x41);
        sleb(1, &mut body);
        body.push(0x21);
        uleb(1, &mut body);

        body.push(0x41);
        sleb(1, &mut body);
        body.push(0x21);
        uleb(2, &mut body);

        body.push(0x02);
        body.push(0x40);

        body.push(0x03);
        body.push(0x40);

        body.push(0x20);
        uleb(2, &mut body);
        body.push(0x20);
        uleb(0, &mut body);
        body.push(0x4a);
        body.push(0x0d);
        uleb(1, &mut body);

        body.push(0x20);
        uleb(1, &mut body);
        body.push(0x20);
        uleb(2, &mut body);
        body.push(0x6c);
        body.push(0x21);
        uleb(1, &mut body);

        body.push(0x20);
        uleb(2, &mut body);
        body.push(0x41);
        sleb(1, &mut body);
        body.push(0x6a);
        body.push(0x21);
        uleb(2, &mut body);

        body.push(0x0c);
        uleb(0, &mut body);

        body.push(0x0b);

        body.push(0x0b);

        body.push(0x20);
        uleb(1, &mut body);

        body.push(0x0b);

        let mut codesec = Vec::new();
        uleb(1, &mut codesec);
        uleb(body.len() as u64, &mut codesec);
        codesec.extend(body);

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(3, funcsec));
        bytes.extend(section(7, exportsec));
        bytes.extend(section(10, codesec));

        let m = parse_module(&bytes).expect("parse fac");
        let mut inst = instantiate(&m, Imports::new()).expect("inst fac");

        for (n, expect) in [(0, 1), (1, 1), (3, 6), (5, 120), (6, 720)] {
            let r = inst.call("fac", &[WasmValue::I32(n)]).unwrap();
            assert_eq!(r, vec![WasmValue::I32(expect)], "fac({})", n);
        }
    }

    #[test]
    fn test_counted_i32_sum_loop() {
        let mut typesec = Vec::new();
        uleb(1, &mut typesec);
        typesec.push(0x60);
        uleb(1, &mut typesec);
        typesec.push(0x7f);
        uleb(1, &mut typesec);
        typesec.push(0x7f);

        let mut funcsec = Vec::new();
        uleb(1, &mut funcsec);
        uleb(0, &mut funcsec);

        let mut exportsec = Vec::new();
        uleb(1, &mut exportsec);
        let name = b"sum";
        uleb(name.len() as u64, &mut exportsec);
        exportsec.extend_from_slice(name);
        exportsec.push(0x00);
        uleb(0, &mut exportsec);

        let mut body = Vec::new();
        uleb(1, &mut body);
        uleb(2, &mut body);
        body.push(0x7f);
        body.push(0x03);
        body.push(0x40);
        body.push(0x20);
        uleb(2, &mut body);
        body.push(0x20);
        uleb(1, &mut body);
        body.push(0x6a);
        body.push(0x21);
        uleb(2, &mut body);
        body.push(0x20);
        uleb(1, &mut body);
        body.push(0x41);
        sleb(1, &mut body);
        body.push(0x6a);
        body.push(0x22);
        uleb(1, &mut body);
        body.push(0x20);
        uleb(0, &mut body);
        body.push(0x48);
        body.push(0x0d);
        uleb(0, &mut body);
        body.push(0x0b);
        body.push(0x20);
        uleb(2, &mut body);
        body.push(0x0b);

        let mut codesec = Vec::new();
        uleb(1, &mut codesec);
        uleb(body.len() as u64, &mut codesec);
        codesec.extend(body);

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(3, funcsec));
        bytes.extend(section(7, exportsec));
        bytes.extend(section(10, codesec));

        let m = parse_module(&bytes).expect("parse counted sum");
        let mut inst = instantiate(&m, Imports::new()).expect("instantiate counted sum");
        for (n, expect) in [(0, 0), (1, 0), (2, 1), (5, 10), (4000, 7_998_000)] {
            let r = inst.call("sum", &[WasmValue::I32(n)]).unwrap();
            assert_eq!(r, vec![WasmValue::I32(expect)], "sum({})", n);
        }
    }

    #[test]
    fn test_i32_store_sum_loop_preserves_memory() {
        let mut typesec = Vec::new();
        uleb(2, &mut typesec);
        typesec.push(0x60);
        uleb(1, &mut typesec);
        typesec.push(0x7f);
        uleb(1, &mut typesec);
        typesec.push(0x7f);
        typesec.push(0x60);
        uleb(1, &mut typesec);
        typesec.push(0x7f);
        uleb(1, &mut typesec);
        typesec.push(0x7f);

        let mut funcsec = Vec::new();
        uleb(2, &mut funcsec);
        uleb(0, &mut funcsec);
        uleb(1, &mut funcsec);

        let memorysec = vec![1, 0x00, 1];

        let mut exportsec = Vec::new();
        uleb(3, &mut exportsec);
        let store_name = b"store_sum";
        uleb(store_name.len() as u64, &mut exportsec);
        exportsec.extend_from_slice(store_name);
        exportsec.push(0x00);
        uleb(0, &mut exportsec);
        let load_name = b"load";
        uleb(load_name.len() as u64, &mut exportsec);
        exportsec.extend_from_slice(load_name);
        exportsec.push(0x00);
        uleb(1, &mut exportsec);
        let memory_name = b"memory";
        uleb(memory_name.len() as u64, &mut exportsec);
        exportsec.extend_from_slice(memory_name);
        exportsec.push(0x02);
        uleb(0, &mut exportsec);

        let mut store_body = Vec::new();
        uleb(1, &mut store_body);
        uleb(2, &mut store_body);
        store_body.push(0x7f);
        store_body.push(0x03);
        store_body.push(0x40);
        store_body.push(0x20);
        uleb(1, &mut store_body);
        store_body.push(0x20);
        uleb(1, &mut store_body);
        store_body.push(0x36);
        store_body.push(2);
        store_body.push(0);
        store_body.push(0x20);
        uleb(2, &mut store_body);
        store_body.push(0x20);
        uleb(1, &mut store_body);
        store_body.push(0x6a);
        store_body.push(0x21);
        uleb(2, &mut store_body);
        store_body.push(0x20);
        uleb(1, &mut store_body);
        store_body.push(0x41);
        sleb(4, &mut store_body);
        store_body.push(0x6a);
        store_body.push(0x22);
        uleb(1, &mut store_body);
        store_body.push(0x20);
        uleb(0, &mut store_body);
        store_body.push(0x48);
        store_body.push(0x0d);
        uleb(0, &mut store_body);
        store_body.push(0x0b);
        store_body.push(0x20);
        uleb(2, &mut store_body);
        store_body.push(0x0b);

        let mut load_body = Vec::new();
        uleb(0, &mut load_body);
        load_body.push(0x20);
        uleb(0, &mut load_body);
        load_body.push(0x28);
        load_body.push(2);
        load_body.push(0);
        load_body.push(0x0b);

        let mut codesec = Vec::new();
        uleb(2, &mut codesec);
        uleb(store_body.len() as u64, &mut codesec);
        codesec.extend(store_body);
        uleb(load_body.len() as u64, &mut codesec);
        codesec.extend(load_body);

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(3, funcsec));
        bytes.extend(section(5, memorysec));
        bytes.extend(section(7, exportsec));
        bytes.extend(section(10, codesec));

        let m = parse_module(&bytes).expect("parse store sum");
        let mut inst = instantiate(&m, Imports::new()).expect("instantiate store sum");
        let r = inst.call("store_sum", &[WasmValue::I32(16)]).unwrap();
        assert_eq!(r, vec![WasmValue::I32(24)]);
        for offset in [0, 4, 8, 12] {
            let r = inst.call("load", &[WasmValue::I32(offset)]).unwrap();
            assert_eq!(r, vec![WasmValue::I32(offset)], "load({})", offset);
        }
    }

    #[test]
    fn test_i32_store_load_xor_sweep_loop_preserves_memory() {
        let bytes = vec![
            0, 97, 115, 109, 1, 0, 0, 0, 1, 6, 1, 96, 1, 127, 1, 127, 3, 2, 1, 0, 5, 3, 1, 0, 2, 7,
            18, 2, 6, 109, 101, 109, 111, 114, 121, 2, 0, 5, 115, 119, 101, 101, 112, 0, 0, 10, 47,
            1, 45, 1, 2, 127, 3, 64, 32, 1, 32, 1, 65, 177, 243, 221, 241, 121, 108, 54, 2, 0, 32,
            2, 32, 1, 40, 2, 0, 115, 33, 2, 32, 1, 65, 4, 106, 34, 1, 32, 0, 72, 13, 0, 11, 32, 2,
            11, 0, 32, 4, 110, 97, 109, 101, 2, 14, 1, 0, 3, 0, 1, 110, 1, 1, 105, 2, 3, 97, 99,
            99, 3, 9, 1, 0, 1, 0, 4, 108, 111, 111, 112,
        ];
        let module = parse_module(&bytes).expect("parse sweep module");
        let mut inst = instantiate(&module, Imports::new()).expect("instantiate sweep module");
        let result = inst
            .call("sweep", &[WasmValue::I32(64 * 1024)])
            .expect("call sweep");
        assert_eq!(result, vec![WasmValue::I32(348_913_664)]);
        assert_eq!(
            inst.memory_read(0, 4),
            Some(0i32.wrapping_mul(-1_640_531_535).to_le_bytes().to_vec())
        );
        assert_eq!(
            inst.memory_read(4, 4),
            Some(4i32.wrapping_mul(-1_640_531_535).to_le_bytes().to_vec())
        );
    }

    #[test]
    fn test_fnv1a_i32_load8_hash_loop_reads_seeded_memory() {
        let bytes = vec![
            0, 97, 115, 109, 1, 0, 0, 0, 1, 7, 1, 96, 2, 127, 127, 1, 127, 3, 2, 1, 0, 5, 3, 1, 0,
            2, 7, 17, 2, 6, 109, 101, 109, 111, 114, 121, 2, 0, 4, 104, 97, 115, 104, 0, 0, 10, 50,
            1, 48, 1, 2, 127, 65, 197, 187, 242, 136, 120, 33, 3, 3, 64, 32, 3, 32, 0, 32, 2, 106,
            45, 0, 0, 115, 65, 147, 131, 128, 8, 108, 33, 3, 32, 2, 65, 1, 106, 34, 2, 32, 1, 72,
            13, 0, 11, 32, 3, 11, 0, 35, 4, 110, 97, 109, 101, 2, 17, 1, 0, 4, 0, 3, 112, 116, 114,
            1, 1, 110, 2, 1, 105, 3, 1, 104, 3, 9, 1, 0, 1, 0, 4, 108, 111, 111, 112,
        ];
        let module = parse_module(&bytes).expect("parse fnv module");
        let mut inst = instantiate(&module, Imports::new()).expect("instantiate fnv module");
        let mut seed = vec![0u8; 32_768];
        for (i, byte) in seed.iter_mut().enumerate() {
            *byte = ((i * 131 + 17) & 255) as u8;
        }
        assert!(inst.memory_write(0, &seed));
        let result = inst
            .call("hash", &[WasmValue::I32(0), WasmValue::I32(32_768)])
            .expect("call hash");
        assert_eq!(result, vec![WasmValue::I32(363_503_045)]);
    }

    #[test]
    fn test_memory_module() {

        let mut typesec = Vec::new();
        uleb(2, &mut typesec);

        typesec.push(0x60);
        uleb(2, &mut typesec);
        typesec.push(0x7f);
        typesec.push(0x7f);
        uleb(0, &mut typesec);

        typesec.push(0x60);
        uleb(1, &mut typesec);
        typesec.push(0x7f);
        uleb(1, &mut typesec);
        typesec.push(0x7f);

        let mut funcsec = Vec::new();
        uleb(3, &mut funcsec);
        uleb(0, &mut funcsec);
        uleb(1, &mut funcsec);
        uleb(1, &mut funcsec);

        let mut memsec = Vec::new();
        uleb(1, &mut memsec);
        memsec.push(0x00);
        uleb(1, &mut memsec);

        let mut exportsec = Vec::new();
        uleb(3, &mut exportsec);
        for (nm, idx) in [("store", 0u64), ("load", 1), ("grow", 2)] {
            uleb(nm.len() as u64, &mut exportsec);
            exportsec.extend_from_slice(nm.as_bytes());
            exportsec.push(0x00);
            uleb(idx, &mut exportsec);
        }

        let mut store_body = Vec::new();
        uleb(0, &mut store_body);
        store_body.push(0x20);
        uleb(0, &mut store_body);
        store_body.push(0x20);
        uleb(1, &mut store_body);
        store_body.push(0x36);
        uleb(2, &mut store_body);
        uleb(0, &mut store_body);
        store_body.push(0x0b);

        let mut load_body = Vec::new();
        uleb(0, &mut load_body);
        load_body.push(0x20);
        uleb(0, &mut load_body);
        load_body.push(0x28);
        uleb(2, &mut load_body);
        uleb(0, &mut load_body);
        load_body.push(0x0b);

        let mut grow_body = Vec::new();
        uleb(0, &mut grow_body);
        grow_body.push(0x20);
        uleb(0, &mut grow_body);
        grow_body.push(0x40);
        grow_body.push(0x00);
        grow_body.push(0x0b);

        let mut codesec = Vec::new();
        uleb(3, &mut codesec);
        for body in [store_body, load_body, grow_body] {
            uleb(body.len() as u64, &mut codesec);
            codesec.extend(body);
        }

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(3, funcsec));
        bytes.extend(section(5, memsec));
        bytes.extend(section(7, exportsec));
        bytes.extend(section(10, codesec));

        let m = parse_module(&bytes).expect("parse mem");
        let mut inst = instantiate(&m, Imports::new()).expect("inst mem");

        inst.call(
            "store",
            &[WasmValue::I32(16), WasmValue::I32(0xdead_beefu32 as i32)],
        )
        .unwrap();
        let r = inst.call("load", &[WasmValue::I32(16)]).unwrap();
        assert_eq!(r, vec![WasmValue::I32(0xdead_beefu32 as i32)]);

        let raw = inst.memory_read(16, 4).unwrap();
        assert_eq!(raw, vec![0xef, 0xbe, 0xad, 0xde]);

        assert_eq!(inst.memory_size(), PAGE);
        let r = inst.call("grow", &[WasmValue::I32(2)]).unwrap();
        assert_eq!(r, vec![WasmValue::I32(1)]);
        assert_eq!(inst.memory_size(), 3 * PAGE);

        assert!(inst.memory_write(100, &[1, 2, 3, 4]));
        assert_eq!(inst.memory_read(100, 4).unwrap(), vec![1, 2, 3, 4]);
        assert!(!inst.memory_write(3 * PAGE - 2, &[1, 2, 3, 4]));
    }

    #[test]
    fn infinite_loop_exhausts_execution_fuel() {

        let mut typesec = Vec::new();
        uleb(1, &mut typesec);
        typesec.push(0x60);
        uleb(0, &mut typesec);
        uleb(0, &mut typesec);

        let mut funcsec = Vec::new();
        uleb(1, &mut funcsec);
        uleb(0, &mut funcsec);

        let mut exportsec = Vec::new();
        uleb(1, &mut exportsec);
        uleb(3, &mut exportsec);
        exportsec.extend_from_slice(b"run");
        exportsec.push(0x00);
        uleb(0, &mut exportsec);

        let mut body = Vec::new();
        uleb(0, &mut body);
        body.push(0x03);
        body.push(0x40);
        body.push(0x0c);
        uleb(0, &mut body);
        body.push(0x0b);
        body.push(0x0b);

        let mut codesec = Vec::new();
        uleb(1, &mut codesec);
        uleb(body.len() as u64, &mut codesec);
        codesec.extend(body);

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(3, funcsec));
        bytes.extend(section(7, exportsec));
        bytes.extend(section(10, codesec));

        let module = parse_module(&bytes).expect("parse infinite-loop module");
        let mut inst = instantiate(&module, Imports::new()).expect("instantiate loop module");
        let err = inst.call("run", &[]).expect_err("loop must exhaust fuel");
        assert!(err.contains("execution fuel exhausted"), "{err}");
    }

    #[test]
    fn memory_grow_over_default_cap_returns_minus_one_without_allocating() {
        let mut typesec = Vec::new();
        uleb(1, &mut typesec);
        typesec.push(0x60);
        uleb(1, &mut typesec);
        typesec.push(0x7f);
        uleb(1, &mut typesec);
        typesec.push(0x7f);

        let mut funcsec = Vec::new();
        uleb(1, &mut funcsec);
        uleb(0, &mut funcsec);

        let mut memsec = Vec::new();
        uleb(1, &mut memsec);
        memsec.push(0x00);
        uleb(1, &mut memsec);

        let mut exportsec = Vec::new();
        uleb(1, &mut exportsec);
        uleb(4, &mut exportsec);
        exportsec.extend_from_slice(b"grow");
        exportsec.push(0x00);
        uleb(0, &mut exportsec);

        let mut body = Vec::new();
        uleb(0, &mut body);
        body.push(0x20);
        uleb(0, &mut body);
        body.push(0x40);
        body.push(0x00);
        body.push(0x0b);

        let mut codesec = Vec::new();
        uleb(1, &mut codesec);
        uleb(body.len() as u64, &mut codesec);
        codesec.extend(body);

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(3, funcsec));
        bytes.extend(section(5, memsec));
        bytes.extend(section(7, exportsec));
        bytes.extend(section(10, codesec));

        let module = parse_module(&bytes).expect("parse capped memory.grow module");
        let mut inst =
            instantiate(&module, Imports::new()).expect("instantiate capped memory module");
        let result = inst.call("grow", &[WasmValue::I32(8192)]).unwrap();
        assert_eq!(result, vec![WasmValue::I32(-1)]);
        assert_eq!(inst.memory_size(), PAGE);
    }

    #[test]
    fn parser_rejects_excessive_expression_nesting() {
        let mut typesec = Vec::new();
        uleb(1, &mut typesec);
        typesec.push(0x60);
        uleb(0, &mut typesec);
        uleb(0, &mut typesec);

        let mut funcsec = Vec::new();
        uleb(1, &mut funcsec);
        uleb(0, &mut funcsec);

        let mut body = Vec::new();
        uleb(0, &mut body);
        for _ in 0..1025 {
            body.push(0x02);
            body.push(0x40);
        }
        for _ in 0..1026 {
            body.push(0x0b);
        }

        let mut codesec = Vec::new();
        uleb(1, &mut codesec);
        uleb(body.len() as u64, &mut codesec);
        codesec.extend(body);

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(3, funcsec));
        bytes.extend(section(10, codesec));

        let err = match parse_module(&bytes) {
            Ok(_) => panic!("deep nesting must reject"),
            Err(err) => err,
        };
        assert!(err.contains("expression nesting depth exceeded"), "{err}");
    }

    #[test]
    fn test_import_hostfn() {
        let mut typesec = Vec::new();
        uleb(1, &mut typesec);
        typesec.push(0x60);
        uleb(1, &mut typesec);
        typesec.push(0x7f);
        uleb(1, &mut typesec);
        typesec.push(0x7f);

        let mut importsec = Vec::new();
        uleb(1, &mut importsec);
        let m = b"env";
        uleb(m.len() as u64, &mut importsec);
        importsec.extend_from_slice(m);
        let n = b"add1";
        uleb(n.len() as u64, &mut importsec);
        importsec.extend_from_slice(n);
        importsec.push(0x00);
        uleb(0, &mut importsec);

        let mut funcsec = Vec::new();
        uleb(1, &mut funcsec);
        uleb(0, &mut funcsec);

        let mut exportsec = Vec::new();
        uleb(1, &mut exportsec);
        let nm = b"run";
        uleb(nm.len() as u64, &mut exportsec);
        exportsec.extend_from_slice(nm);
        exportsec.push(0x00);
        uleb(1, &mut exportsec);

        let mut body = Vec::new();
        uleb(0, &mut body);
        body.push(0x20);
        uleb(0, &mut body);
        body.push(0x10);
        uleb(0, &mut body);
        body.push(0x0b);

        let mut codesec = Vec::new();
        uleb(1, &mut codesec);
        uleb(body.len() as u64, &mut codesec);
        codesec.extend(body);

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(2, importsec));
        bytes.extend(section(3, funcsec));
        bytes.extend(section(7, exportsec));
        bytes.extend(section(10, codesec));

        let module = parse_module(&bytes).expect("parse import");

        let mut imports = Imports::new();
        imports.func(
            "env",
            "add1",
            Box::new(|_ctx: &mut dyn HostContext, args: &[WasmValue]| {
                let x = match args[0] {
                    WasmValue::I32(v) => v,
                    _ => 0,
                };
                Ok(vec![WasmValue::I32(x + 1)])
            }),
        );

        let mut inst = instantiate(&module, imports).expect("inst import");
        let r = inst.call("run", &[WasmValue::I32(41)]).unwrap();
        assert_eq!(r, vec![WasmValue::I32(42)]);
    }

    #[test]
    fn test_active_elem_initializes_imported_table() {
        let mut typesec = Vec::new();
        uleb(1, &mut typesec);
        typesec.push(0x60);
        uleb(0, &mut typesec);
        uleb(0, &mut typesec);

        let mut importsec = Vec::new();
        uleb(1, &mut importsec);
        let m = b"spectest";
        uleb(m.len() as u64, &mut importsec);
        importsec.extend_from_slice(m);
        let n = b"table";
        uleb(n.len() as u64, &mut importsec);
        importsec.extend_from_slice(n);
        importsec.push(0x01);
        importsec.push(0x70);
        importsec.push(0x00);
        uleb(10, &mut importsec);

        let mut funcsec = Vec::new();
        uleb(1, &mut funcsec);
        uleb(0, &mut funcsec);

        let mut elemsec = Vec::new();
        uleb(1, &mut elemsec);
        elemsec.push(0x00);
        elemsec.push(0x41);
        sleb(3, &mut elemsec);
        elemsec.push(0x0b);
        uleb(1, &mut elemsec);
        uleb(0, &mut elemsec);

        let mut body = Vec::new();
        uleb(0, &mut body);
        body.push(0x0b);
        let mut codesec = Vec::new();
        uleb(1, &mut codesec);
        uleb(body.len() as u64, &mut codesec);
        codesec.extend(body);

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(2, importsec));
        bytes.extend(section(3, funcsec));
        bytes.extend(section(9, elemsec));
        bytes.extend(section(10, codesec));

        let module = parse_module(&bytes).expect("parse imported table elem module");
        let mut imports = Imports::new();
        imports.table("spectest", "table", ValType::FuncRef, 10, None);
        let inst = instantiate(&module, imports).expect("instantiate imported table elem");

        assert_eq!(inst.table_size_at(0), 10);
        let entries = inst.table_func_indices_at(0);
        assert_eq!(entries.get(3).copied().flatten(), Some(0));
    }

    #[test]
    fn test_empty_active_elem_checks_offset_bounds() {
        let mut tablesec = Vec::new();
        uleb(1, &mut tablesec);
        tablesec.push(0x70);
        tablesec.push(0x00);
        uleb(0, &mut tablesec);

        let mut elemsec = Vec::new();
        uleb(1, &mut elemsec);
        elemsec.push(0x00);
        elemsec.push(0x41);
        sleb(1, &mut elemsec);
        elemsec.push(0x0b);
        uleb(0, &mut elemsec);

        let mut bytes = header();
        bytes.extend(section(4, tablesec));
        bytes.extend(section(9, elemsec));

        let module = parse_module(&bytes).expect("parse empty active elem module");
        let err = match instantiate(&module, Imports::new()) {
            Ok(_) => panic!("instantiate should trap"),
            Err(err) => err,
        };
        assert!(err.contains("element segment out of table bounds"), "{err}");
    }

    fn build_binop_i32(opcode: u8) -> Vec<u8> {

        let mut typesec = Vec::new();
        uleb(1, &mut typesec);
        typesec.push(0x60);
        uleb(2, &mut typesec);
        typesec.push(0x7f);
        typesec.push(0x7f);
        uleb(1, &mut typesec);
        typesec.push(0x7f);

        let mut funcsec = Vec::new();
        uleb(1, &mut funcsec);
        uleb(0, &mut funcsec);

        let mut exportsec = Vec::new();
        uleb(1, &mut exportsec);
        let nm = b"op";
        uleb(nm.len() as u64, &mut exportsec);
        exportsec.extend_from_slice(nm);
        exportsec.push(0x00);
        uleb(0, &mut exportsec);

        let mut body = Vec::new();
        uleb(0, &mut body);
        body.push(0x20);
        uleb(0, &mut body);
        body.push(0x20);
        uleb(1, &mut body);
        body.push(opcode);
        body.push(0x0b);

        let mut codesec = Vec::new();
        uleb(1, &mut codesec);
        uleb(body.len() as u64, &mut codesec);
        codesec.extend(body);

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(3, funcsec));
        bytes.extend(section(7, exportsec));
        bytes.extend(section(10, codesec));
        bytes
    }

    #[test]
    fn test_div_by_zero_traps() {
        let bytes = build_binop_i32(0x6d);
        let m = parse_module(&bytes).unwrap();
        let mut inst = instantiate(&m, Imports::new()).unwrap();
        let r = inst.call("op", &[WasmValue::I32(10), WasmValue::I32(0)]);
        assert!(r.is_err(), "div by zero must trap");

        let ok = inst
            .call("op", &[WasmValue::I32(-9), WasmValue::I32(2)])
            .unwrap();
        assert_eq!(ok, vec![WasmValue::I32(-4)]);
    }

    #[test]
    fn test_rem_s_and_overflow() {
        let bytes = build_binop_i32(0x6f);
        let m = parse_module(&bytes).unwrap();
        let mut inst = instantiate(&m, Imports::new()).unwrap();
        let r = inst
            .call("op", &[WasmValue::I32(-7), WasmValue::I32(3)])
            .unwrap();
        assert_eq!(r, vec![WasmValue::I32(-1)]);

        let r = inst
            .call("op", &[WasmValue::I32(i32::MIN), WasmValue::I32(-1)])
            .unwrap();
        assert_eq!(r, vec![WasmValue::I32(0)]);

        let dbytes = build_binop_i32(0x6d);
        let dm = parse_module(&dbytes).unwrap();
        let mut di = instantiate(&dm, Imports::new()).unwrap();
        assert!(di
            .call("op", &[WasmValue::I32(i32::MIN), WasmValue::I32(-1)])
            .is_err());
    }

    #[test]
    fn test_signed_unsigned_compare() {

        let bs = build_binop_i32(0x48);
        let m = parse_module(&bs).unwrap();
        let mut inst = instantiate(&m, Imports::new()).unwrap();
        let r = inst
            .call("op", &[WasmValue::I32(-1), WasmValue::I32(1)])
            .unwrap();
        assert_eq!(r, vec![WasmValue::I32(1)]);

        let bu = build_binop_i32(0x49);
        let mu = parse_module(&bu).unwrap();
        let mut iu = instantiate(&mu, Imports::new()).unwrap();
        let r = iu
            .call("op", &[WasmValue::I32(-1), WasmValue::I32(1)])
            .unwrap();
        assert_eq!(r, vec![WasmValue::I32(0)]);
    }

    #[test]
    fn test_f64_op() {

        let mut typesec = Vec::new();
        uleb(1, &mut typesec);
        typesec.push(0x60);
        uleb(2, &mut typesec);
        typesec.push(0x7c);
        typesec.push(0x7c);
        uleb(1, &mut typesec);
        typesec.push(0x7c);

        let mut funcsec = Vec::new();
        uleb(1, &mut funcsec);
        uleb(0, &mut funcsec);

        let mut exportsec = Vec::new();
        uleb(1, &mut exportsec);
        let nm = b"op";
        uleb(nm.len() as u64, &mut exportsec);
        exportsec.extend_from_slice(nm);
        exportsec.push(0x00);
        uleb(0, &mut exportsec);

        let mut body = Vec::new();
        uleb(0, &mut body);
        body.push(0x20);
        uleb(0, &mut body);
        body.push(0x20);
        uleb(1, &mut body);
        body.push(0xa0);
        body.push(0x0b);

        let mut codesec = Vec::new();
        uleb(1, &mut codesec);
        uleb(body.len() as u64, &mut codesec);
        codesec.extend(body);

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(3, funcsec));
        bytes.extend(section(7, exportsec));
        bytes.extend(section(10, codesec));

        let m = parse_module(&bytes).unwrap();
        let mut inst = instantiate(&m, Imports::new()).unwrap();
        let r = inst
            .call("op", &[WasmValue::F64(1.5), WasmValue::F64(2.25)])
            .unwrap();
        assert_eq!(r, vec![WasmValue::F64(3.75)]);
    }

    #[test]
    fn test_call_indirect() {

        let mut typesec = Vec::new();
        uleb(1, &mut typesec);
        typesec.push(0x60);
        uleb(0, &mut typesec);
        uleb(1, &mut typesec);
        typesec.push(0x7f);

        let mut funcsec = Vec::new();
        uleb(2, &mut funcsec);
        uleb(0, &mut funcsec);
        uleb(0, &mut funcsec);

        let mut tablesec = Vec::new();
        uleb(1, &mut tablesec);
        tablesec.push(0x70);
        tablesec.push(0x00);
        uleb(1, &mut tablesec);

        let mut exportsec = Vec::new();
        uleb(1, &mut exportsec);
        let nm = b"run";
        uleb(nm.len() as u64, &mut exportsec);
        exportsec.extend_from_slice(nm);
        exportsec.push(0x00);
        uleb(1, &mut exportsec);

        let mut elemsec = Vec::new();
        uleb(1, &mut elemsec);
        uleb(0, &mut elemsec);
        elemsec.push(0x41);
        sleb(0, &mut elemsec);
        elemsec.push(0x0b);
        uleb(1, &mut elemsec);
        uleb(0, &mut elemsec);

        let mut f0 = Vec::new();
        uleb(0, &mut f0);
        f0.push(0x41);
        sleb(7, &mut f0);
        f0.push(0x0b);

        let mut f1 = Vec::new();
        uleb(0, &mut f1);
        f1.push(0x41);
        sleb(0, &mut f1);
        f1.push(0x11);
        uleb(0, &mut f1);
        uleb(0, &mut f1);
        f1.push(0x0b);

        let mut codesec = Vec::new();
        uleb(2, &mut codesec);
        for body in [f0, f1] {
            uleb(body.len() as u64, &mut codesec);
            codesec.extend(body);
        }

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(3, funcsec));
        bytes.extend(section(4, tablesec));
        bytes.extend(section(7, exportsec));
        bytes.extend(section(9, elemsec));
        bytes.extend(section(10, codesec));

        let m = parse_module(&bytes).unwrap();
        let mut inst = instantiate(&m, Imports::new()).unwrap();
        let r = inst.call("run", &[]).unwrap();
        assert_eq!(r, vec![WasmValue::I32(7)]);
    }

    #[test]
    fn table_grow_oversized_delta_returns_minus_one() {
        let mut typesec = Vec::new();
        uleb(1, &mut typesec);
        typesec.push(0x60);
        uleb(1, &mut typesec);
        typesec.push(0x7f);
        uleb(1, &mut typesec);
        typesec.push(0x7f);

        let mut funcsec = Vec::new();
        uleb(1, &mut funcsec);
        uleb(0, &mut funcsec);

        let mut tablesec = Vec::new();
        uleb(1, &mut tablesec);
        tablesec.push(0x6f);
        tablesec.push(0x00);
        uleb(0, &mut tablesec);

        let mut exportsec = Vec::new();
        uleb(1, &mut exportsec);
        let nm = b"grow";
        uleb(nm.len() as u64, &mut exportsec);
        exportsec.extend_from_slice(nm);
        exportsec.push(0x00);
        uleb(0, &mut exportsec);

        let mut body = Vec::new();
        uleb(0, &mut body);
        body.push(0xd0);
        body.push(0x6f);
        body.push(0x20);
        uleb(0, &mut body);
        body.push(0xfc);
        uleb(15, &mut body);
        uleb(0, &mut body);
        body.push(0x0b);

        let mut codesec = Vec::new();
        uleb(1, &mut codesec);
        uleb(body.len() as u64, &mut codesec);
        codesec.extend(body);

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(3, funcsec));
        bytes.extend(section(4, tablesec));
        bytes.extend(section(7, exportsec));
        bytes.extend(section(10, codesec));

        let module = parse_module(&bytes).expect("parse oversized table.grow module");
        let mut instance = instantiate(&module, Imports::new()).expect("instantiate table.grow");
        let result = instance.call("grow", &[WasmValue::I32(i32::MAX)]).unwrap();
        assert_eq!(result, vec![WasmValue::I32(-1)]);
    }

    #[test]
    fn call_indirect_accepts_structurally_duplicate_types() {

        let mut typesec = Vec::new();
        uleb(2, &mut typesec);
        for _ in 0..2 {
            typesec.push(0x60);
            uleb(0, &mut typesec);
            uleb(1, &mut typesec);
            typesec.push(0x7f);
        }

        let mut funcsec = Vec::new();
        uleb(2, &mut funcsec);
        uleb(1, &mut funcsec);
        uleb(0, &mut funcsec);

        let mut tablesec = Vec::new();
        uleb(1, &mut tablesec);
        tablesec.push(0x70);
        tablesec.push(0x00);
        uleb(1, &mut tablesec);

        let mut exportsec = Vec::new();
        uleb(1, &mut exportsec);
        let nm = b"run";
        uleb(nm.len() as u64, &mut exportsec);
        exportsec.extend_from_slice(nm);
        exportsec.push(0x00);
        uleb(1, &mut exportsec);

        let mut elemsec = Vec::new();
        uleb(1, &mut elemsec);
        uleb(0, &mut elemsec);
        elemsec.push(0x41);
        sleb(0, &mut elemsec);
        elemsec.push(0x0b);
        uleb(1, &mut elemsec);
        uleb(0, &mut elemsec);

        let mut f0 = Vec::new();
        uleb(0, &mut f0);
        f0.push(0x41);
        sleb(9, &mut f0);
        f0.push(0x0b);

        let mut f1 = Vec::new();
        uleb(0, &mut f1);
        f1.push(0x41);
        sleb(0, &mut f1);
        f1.push(0x11);
        uleb(0, &mut f1);
        uleb(0, &mut f1);
        f1.push(0x0b);

        let mut codesec = Vec::new();
        uleb(2, &mut codesec);
        for body in [f0, f1] {
            uleb(body.len() as u64, &mut codesec);
            codesec.extend(body);
        }

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(3, funcsec));
        bytes.extend(section(4, tablesec));
        bytes.extend(section(7, exportsec));
        bytes.extend(section(9, elemsec));
        bytes.extend(section(10, codesec));

        let m = parse_module(&bytes).unwrap();
        let mut inst = instantiate(&m, Imports::new()).unwrap();
        let r = inst.call("run", &[]).unwrap();
        assert_eq!(r, vec![WasmValue::I32(9)]);
    }

    #[test]
    fn test_hostfn_writes_memory() {

        let mut typesec = Vec::new();
        uleb(2, &mut typesec);
        typesec.push(0x60);
        uleb(2, &mut typesec);
        typesec.push(0x7f);
        typesec.push(0x7f);
        uleb(0, &mut typesec);
        typesec.push(0x60);
        uleb(0, &mut typesec);
        uleb(1, &mut typesec);
        typesec.push(0x7f);

        let mut importsec = Vec::new();
        uleb(1, &mut importsec);
        let m = b"env";
        uleb(m.len() as u64, &mut importsec);
        importsec.extend_from_slice(m);
        let n = b"poke";
        uleb(n.len() as u64, &mut importsec);
        importsec.extend_from_slice(n);
        importsec.push(0x00);
        uleb(0, &mut importsec);

        let mut funcsec = Vec::new();
        uleb(1, &mut funcsec);
        uleb(1, &mut funcsec);

        let mut memsec = Vec::new();
        uleb(1, &mut memsec);
        memsec.push(0x00);
        uleb(1, &mut memsec);

        let mut exportsec = Vec::new();
        uleb(1, &mut exportsec);
        let nm = b"run";
        uleb(nm.len() as u64, &mut exportsec);
        exportsec.extend_from_slice(nm);
        exportsec.push(0x00);
        uleb(1, &mut exportsec);

        let mut body = Vec::new();
        uleb(0, &mut body);
        body.push(0x41);
        sleb(0, &mut body);
        body.push(0x41);
        sleb(12345, &mut body);
        body.push(0x10);
        uleb(0, &mut body);
        body.push(0x41);
        sleb(0, &mut body);
        body.push(0x28);
        uleb(2, &mut body);
        uleb(0, &mut body);
        body.push(0x0b);

        let mut codesec = Vec::new();
        uleb(1, &mut codesec);
        uleb(body.len() as u64, &mut codesec);
        codesec.extend(body);

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(2, importsec));
        bytes.extend(section(3, funcsec));
        bytes.extend(section(5, memsec));
        bytes.extend(section(7, exportsec));
        bytes.extend(section(10, codesec));

        let module = parse_module(&bytes).expect("parse poke");

        let mut imports = Imports::new();
        imports.func(
            "env",
            "poke",
            Box::new(|ctx: &mut dyn HostContext, args: &[WasmValue]| {
                let ptr = match args[0] {
                    WasmValue::I32(v) => v as usize,
                    _ => 0,
                };
                let val = match args[1] {
                    WasmValue::I32(v) => v,
                    _ => 0,
                };
                assert!(ctx.mem_size() >= 4);
                let ok = ctx.mem_write(ptr, &val.to_le_bytes());
                assert!(ok, "host mem_write should succeed");
                Ok(vec![])
            }),
        );

        let mut inst = instantiate(&module, imports).expect("inst poke");
        let r = inst.call("run", &[]).unwrap();
        assert_eq!(r, vec![WasmValue::I32(12345)]);
    }

    #[test]
    fn test_func_imports_introspection() {

        let mut typesec = Vec::new();
        uleb(1, &mut typesec);
        typesec.push(0x60);
        uleb(1, &mut typesec);
        typesec.push(0x7f);
        uleb(1, &mut typesec);
        typesec.push(0x7f);

        let mut importsec = Vec::new();
        uleb(1, &mut importsec);
        let m = b"env";
        uleb(m.len() as u64, &mut importsec);
        importsec.extend_from_slice(m);
        let n = b"foo";
        uleb(n.len() as u64, &mut importsec);
        importsec.extend_from_slice(n);
        importsec.push(0x00);
        uleb(0, &mut importsec);

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(2, importsec));

        let module = parse_module(&bytes).expect("parse foo import");
        let decls = func_imports(&module);
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].module, "env");
        assert_eq!(decls[0].name, "foo");
        assert_eq!(decls[0].params, vec![ValType::I32]);
        assert_eq!(decls[0].results, vec![ValType::I32]);
    }

    #[test]
    fn module_static_import_export_descriptors_preserve_declaration_shape() {

        let mut typesec = Vec::new();
        uleb(2, &mut typesec);
        typesec.push(0x60);
        uleb(0, &mut typesec);
        uleb(0, &mut typesec);
        typesec.push(0x60);
        uleb(0, &mut typesec);
        uleb(1, &mut typesec);
        typesec.push(0x7f);

        let mut importsec = Vec::new();
        uleb(1, &mut importsec);
        let module_name = b"env";
        uleb(module_name.len() as u64, &mut importsec);
        importsec.extend_from_slice(module_name);
        let import_name = b"imp";
        uleb(import_name.len() as u64, &mut importsec);
        importsec.extend_from_slice(import_name);
        importsec.push(0x00);
        uleb(0, &mut importsec);

        let mut funcsec = Vec::new();
        uleb(1, &mut funcsec);
        uleb(1, &mut funcsec);

        let mut exportsec = Vec::new();
        uleb(1, &mut exportsec);
        let export_name = b"run";
        uleb(export_name.len() as u64, &mut exportsec);
        exportsec.extend_from_slice(export_name);
        exportsec.push(0x00);
        uleb(1, &mut exportsec);

        let mut body = Vec::new();
        uleb(0, &mut body);
        body.push(0x41);
        sleb(7, &mut body);
        body.push(0x0b);
        let mut codesec = Vec::new();
        uleb(1, &mut codesec);
        uleb(body.len() as u64, &mut codesec);
        codesec.extend(body);

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(2, importsec));
        bytes.extend(section(3, funcsec));
        bytes.extend(section(7, exportsec));
        bytes.extend(section(10, codesec));

        let module = parse_module(&bytes).expect("parse import/export descriptor module");
        let imports = module_import_descriptors(&module);
        assert_eq!(
            imports,
            vec![ModuleImportDescriptor {
                module: "env".to_string(),
                name: "imp".to_string(),
                kind: "function",
            }]
        );
        let exports = module_export_descriptors(&module);
        assert_eq!(
            exports,
            vec![ModuleExportDescriptor {
                name: "run".to_string(),
                kind: "function",
            }]
        );
    }

    fn build_single_fn(
        params: &[u8],
        results: &[u8],
        body_instrs: &[u8],
        mem_min: Option<u32>,

        data: &[(bool, i32, Vec<u8>)],
        export_name: &str,
    ) -> Vec<u8> {
        let mut typesec = Vec::new();
        uleb(1, &mut typesec);
        typesec.push(0x60);
        uleb(params.len() as u64, &mut typesec);
        typesec.extend_from_slice(params);
        uleb(results.len() as u64, &mut typesec);
        typesec.extend_from_slice(results);

        let mut funcsec = Vec::new();
        uleb(1, &mut funcsec);
        uleb(0, &mut funcsec);

        let mut exportsec = Vec::new();
        uleb(1, &mut exportsec);
        uleb(export_name.len() as u64, &mut exportsec);
        exportsec.extend_from_slice(export_name.as_bytes());
        exportsec.push(0x00);
        uleb(0, &mut exportsec);

        let mut body = Vec::new();
        uleb(0, &mut body);
        body.extend_from_slice(body_instrs);
        body.push(0x0b);

        let mut codesec = Vec::new();
        uleb(1, &mut codesec);
        uleb(body.len() as u64, &mut codesec);
        codesec.extend(body);

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(3, funcsec));
        if let Some(min) = mem_min {
            let mut memsec = Vec::new();
            uleb(1, &mut memsec);
            memsec.push(0x00);
            uleb(min as u64, &mut memsec);
            bytes.extend(section(5, memsec));
        }
        bytes.extend(section(7, exportsec));
        if !data.is_empty() {
            let mut datacountsec = Vec::new();
            uleb(data.len() as u64, &mut datacountsec);
            bytes.extend(section(12, datacountsec));
        }
        bytes.extend(section(10, codesec));
        if !data.is_empty() {
            let mut datasec = Vec::new();
            uleb(data.len() as u64, &mut datasec);
            for (passive, offset, segbytes) in data {
                if *passive {
                    uleb(1, &mut datasec);
                    uleb(segbytes.len() as u64, &mut datasec);
                    datasec.extend_from_slice(segbytes);
                } else {
                    uleb(0, &mut datasec);
                    datasec.push(0x41);
                    sleb(*offset as i64, &mut datasec);
                    datasec.push(0x0b);
                    uleb(segbytes.len() as u64, &mut datasec);
                    datasec.extend_from_slice(segbytes);
                }
            }
            bytes.extend(section(11, datasec));
        }
        bytes
    }

    #[test]
    fn test_sign_extension() {

        let m = parse_module(&build_single_fn(
            &[0x7f],
            &[0x7f],
            &[0x20, 0x00, 0xc0],
            None,
            &[],
            "ext8",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(
            i.call("ext8", &[WasmValue::I32(200)]).unwrap(),
            vec![WasmValue::I32(-56)]
        );
        assert_eq!(
            i.call("ext8", &[WasmValue::I32(127)]).unwrap(),
            vec![WasmValue::I32(127)]
        );

        let m = parse_module(&build_single_fn(
            &[0x7f],
            &[0x7f],
            &[0x20, 0x00, 0xc1],
            None,
            &[],
            "ext16",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(
            i.call("ext16", &[WasmValue::I32(40000)]).unwrap(),
            vec![WasmValue::I32(-25536)]
        );
    }

    #[test]
    fn test_trunc_sat() {

        let m = parse_module(&build_single_fn(
            &[0x7c],
            &[0x7f],
            &[0x20, 0x00, 0xfc, 0x02],
            None,
            &[],
            "sat",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(
            i.call("sat", &[WasmValue::F64(1e20)]).unwrap(),
            vec![WasmValue::I32(2147483647)]
        );
        assert_eq!(
            i.call("sat", &[WasmValue::F64(-1e20)]).unwrap(),
            vec![WasmValue::I32(-2147483648)]
        );
        assert_eq!(
            i.call("sat", &[WasmValue::F64(3.7)]).unwrap(),
            vec![WasmValue::I32(3)]
        );
        assert_eq!(
            i.call("sat", &[WasmValue::F64(f64::NAN)]).unwrap(),
            vec![WasmValue::I32(0)]
        );
    }

    #[test]
    fn test_memory_fill() {

        let body = vec![
            0x20, 0x00, 0x20, 0x01, 0x20, 0x02, 0xfc, 0x0b, 0x00,
            0x20, 0x00, 0x2d, 0x00, 0x00,
        ];
        let m = parse_module(&build_single_fn(
            &[0x7f, 0x7f, 0x7f],
            &[0x7f],
            &body,
            Some(1),
            &[],
            "fill",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();

        let r = i
            .call(
                "fill",
                &[WasmValue::I32(8), WasmValue::I32(0xAB), WasmValue::I32(10)],
            )
            .unwrap();
        assert_eq!(r, vec![WasmValue::I32(0xAB)]);
        assert_eq!(i.memory_read(8, 10).unwrap(), vec![0xABu8; 10]);
        assert_eq!(i.memory_read(7, 1).unwrap(), vec![0x00]);

        let r = i.call(
            "fill",
            &[
                WasmValue::I32(PAGE as i32 - 2),
                WasmValue::I32(1),
                WasmValue::I32(10),
            ],
        );
        assert!(r.is_err(), "OOB memory.fill must trap");
    }

    #[test]
    fn test_memory_copy() {

        let body = vec![
            0x20, 0x00, 0x20, 0x01, 0x20, 0x02, 0xfc, 0x0a, 0x00, 0x00,
            0x20, 0x00, 0x2d, 0x00, 0x00,
        ];
        let m = parse_module(&build_single_fn(
            &[0x7f, 0x7f, 0x7f],
            &[0x7f],
            &body,
            Some(1),
            &[],
            "copy",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();

        i.memory_write(0, &[1, 2, 3, 4, 5]);

        let r = i
            .call(
                "copy",
                &[WasmValue::I32(100), WasmValue::I32(0), WasmValue::I32(5)],
            )
            .unwrap();
        assert_eq!(r, vec![WasmValue::I32(1)]);
        assert_eq!(i.memory_read(100, 5).unwrap(), vec![1, 2, 3, 4, 5]);

        i.memory_write(0, &[1, 2, 3, 4, 5, 0, 0]);
        i.call(
            "copy",
            &[WasmValue::I32(2), WasmValue::I32(0), WasmValue::I32(5)],
        )
        .unwrap();

        assert_eq!(i.memory_read(0, 7).unwrap(), vec![1, 2, 1, 2, 3, 4, 5]);

        assert!(i
            .call(
                "copy",
                &[
                    WasmValue::I32(PAGE as i32 - 2),
                    WasmValue::I32(0),
                    WasmValue::I32(10)
                ]
            )
            .is_err());
    }

    #[test]
    fn test_v128_load32_zero() {
        let m = parse_module(&build_single_fn(
            &[0x7f],
            &[0x7b],
            &[
                0x20, 0x00,
                0xfd, 0x5c, 0x02, 0x00,
            ],
            Some(1),
            &[],
            "load_zero",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        i.memory_write(0, &[1, 2, 3, 4, 5, 6, 7, 8]);
        let mut expected = [0u8; 16];
        expected[..4].copy_from_slice(&[1, 2, 3, 4]);
        assert_eq!(
            i.call("load_zero", &[WasmValue::I32(0)]).unwrap(),
            vec![WasmValue::V128(expected)]
        );
    }

    #[test]
    fn test_v128_load16_lane() {
        let m = parse_module(&build_single_fn(
            &[0x7f, 0x7b],
            &[0x7b],
            &[
                0x20, 0x00,
                0x20, 0x01,
                0xfd, 0x55, 0x01, 0x00, 0x03,
            ],
            Some(1),
            &[],
            "load_lane",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        i.memory_write(0, &[0xaa, 0xbb]);
        let mut input = [0u8; 16];
        input[6] = 0x11;
        input[7] = 0x22;
        let mut expected = input;
        expected[6] = 0xaa;
        expected[7] = 0xbb;
        assert_eq!(
            i.call("load_lane", &[WasmValue::I32(0), WasmValue::V128(input)])
                .unwrap(),
            vec![WasmValue::V128(expected)]
        );
    }

    #[test]
    fn test_v128_store32_lane() {
        let m = parse_module(&build_single_fn(
            &[0x7f, 0x7b],
            &[],
            &[
                0x20, 0x00,
                0x20, 0x01,
                0xfd, 0x5a, 0x02, 0x00, 0x02,
            ],
            Some(1),
            &[],
            "store_lane",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        let input = i32x4_bytes([0x11121314, 0x21222324, 0x31323334, 0x41424344]);
        i.call("store_lane", &[WasmValue::I32(8), WasmValue::V128(input)])
            .unwrap();
        assert_eq!(i.memory_read(8, 4).unwrap(), vec![0x34, 0x33, 0x32, 0x31]);
    }

    #[test]
    fn test_passive_data_and_memory_init() {

        let body = vec![
            0x20, 0x00, 0x20, 0x01, 0x20, 0x02, 0xfc, 0x08, 0x00, 0x00,
            0x20, 0x00, 0x2d, 0x00, 0x00,
        ];

        let m = parse_module(&build_single_fn(
            &[0x7f, 0x7f, 0x7f],
            &[0x7f],
            &body,
            Some(1),
            &[(true, 0, vec![0xDE, 0xAD, 0xBE, 0xEF])],
            "init",
        ))
        .unwrap();

        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(i.memory_read(0, 4).unwrap(), vec![0, 0, 0, 0]);

        let r = i
            .call(
                "init",
                &[WasmValue::I32(64), WasmValue::I32(0), WasmValue::I32(4)],
            )
            .unwrap();
        assert_eq!(r, vec![WasmValue::I32(0xDE)]);
        assert_eq!(i.memory_read(64, 4).unwrap(), vec![0xDE, 0xAD, 0xBE, 0xEF]);

        assert!(i
            .call(
                "init",
                &[WasmValue::I32(0), WasmValue::I32(0), WasmValue::I32(5)]
            )
            .is_err());
    }

    #[test]
    fn test_data_drop() {

        let body = vec![
            0xfc, 0x09, 0x00,
            0x41, 0x00,
            0x41, 0x00,
            0x20, 0x00,
            0xfc, 0x08, 0x00, 0x00,
            0x41, 0x00,
        ];
        let m = parse_module(&build_single_fn(
            &[0x7f],
            &[0x7f],
            &body,
            Some(1),
            &[(true, 0, vec![0x11, 0x22, 0x33, 0x44])],
            "go",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();

        assert_eq!(
            i.call("go", &[WasmValue::I32(0)]).unwrap(),
            vec![WasmValue::I32(0)]
        );

        assert!(i.call("go", &[WasmValue::I32(1)]).is_err());
    }

    #[test]
    fn test_active_data_still_copied() {

        let m = parse_module(&build_single_fn(
            &[],
            &[0x7f],
            &[0x41, 0x00, 0x2d, 0x00, 0x00],
            Some(1),
            &[(false, 5, vec![0xAA, 0xBB])],
            "read0",
        ))
        .unwrap();
        let i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(i.memory_read(5, 2).unwrap(), vec![0xAA, 0xBB]);
    }

    #[test]
    fn test_multi_memory_explicit_memarg_and_active_data() {
        let mut typesec = Vec::new();
        uleb(1, &mut typesec);
        typesec.push(0x60);
        uleb(1, &mut typesec);
        typesec.push(0x7f);
        uleb(1, &mut typesec);
        typesec.push(0x7f);

        let mut funcsec = Vec::new();
        uleb(1, &mut funcsec);
        uleb(0, &mut funcsec);

        let mut memsec = Vec::new();
        uleb(2, &mut memsec);
        memsec.push(0x00);
        uleb(0, &mut memsec);
        memsec.push(0x00);
        uleb(1, &mut memsec);

        let mut exportsec = Vec::new();
        uleb(1, &mut exportsec);
        uleb(4, &mut exportsec);
        exportsec.extend_from_slice(b"load");
        exportsec.push(0x00);
        uleb(0, &mut exportsec);

        let mut body = Vec::new();
        uleb(0, &mut body);
        body.extend_from_slice(&[
            0x20, 0x00,
            0x2d, 0x40, 0x01, 0x00,
            0x0b,
        ]);
        let mut codesec = Vec::new();
        uleb(1, &mut codesec);
        uleb(body.len() as u64, &mut codesec);
        codesec.extend(body);

        let mut datasec = Vec::new();
        uleb(1, &mut datasec);
        uleb(2, &mut datasec);
        uleb(1, &mut datasec);
        datasec.push(0x41);
        sleb(0, &mut datasec);
        datasec.push(0x0b);
        uleb(1, &mut datasec);
        datasec.push(0x61);

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(3, funcsec));
        bytes.extend(section(5, memsec));
        bytes.extend(section(7, exportsec));
        bytes.extend(section(10, codesec));
        bytes.extend(section(11, datasec));

        let m = parse_module(&bytes).unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(
            i.call("load", &[WasmValue::I32(0)]).unwrap(),
            vec![WasmValue::I32(0x61)]
        );
        assert_eq!(i.memory_read(0, 0).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn test_multi_memory_size_and_grow_use_explicit_memory_index() {
        let mut typesec = Vec::new();
        uleb(2, &mut typesec);
        typesec.push(0x60);
        uleb(0, &mut typesec);
        uleb(1, &mut typesec);
        typesec.push(0x7f);
        typesec.push(0x60);
        uleb(1, &mut typesec);
        typesec.push(0x7f);
        uleb(0, &mut typesec);

        let mut funcsec = Vec::new();
        uleb(3, &mut funcsec);
        uleb(0, &mut funcsec);
        uleb(1, &mut funcsec);
        uleb(0, &mut funcsec);

        let mut memsec = Vec::new();
        uleb(5, &mut memsec);
        for _ in 0..4 {
            memsec.push(0x00);
            uleb(0, &mut memsec);
        }
        memsec.push(0x01);
        uleb(0, &mut memsec);
        uleb(2, &mut memsec);

        let mut exportsec = Vec::new();
        uleb(3, &mut exportsec);
        uleb(4, &mut exportsec);
        exportsec.extend_from_slice(b"size");
        exportsec.push(0x00);
        uleb(0, &mut exportsec);
        uleb(4, &mut exportsec);
        exportsec.extend_from_slice(b"grow");
        exportsec.push(0x00);
        uleb(1, &mut exportsec);
        uleb(5, &mut exportsec);
        exportsec.extend_from_slice(b"sizen");
        exportsec.push(0x00);
        uleb(2, &mut exportsec);

        let mut codesec = Vec::new();
        uleb(3, &mut codesec);
        let size_m = vec![0x00, 0x3f, 0x04, 0x0b];
        uleb(size_m.len() as u64, &mut codesec);
        codesec.extend(size_m);
        let grow_m = vec![0x00, 0x20, 0x00, 0x40, 0x04, 0x1a, 0x0b];
        uleb(grow_m.len() as u64, &mut codesec);
        codesec.extend(grow_m);
        let size_n = vec![0x00, 0x3f, 0x02, 0x0b];
        uleb(size_n.len() as u64, &mut codesec);
        codesec.extend(size_n);

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(3, funcsec));
        bytes.extend(section(5, memsec));
        bytes.extend(section(7, exportsec));
        bytes.extend(section(10, codesec));

        let m = parse_module(&bytes).unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(i.call("size", &[]).unwrap(), vec![WasmValue::I32(0)]);
        assert_eq!(i.call("sizen", &[]).unwrap(), vec![WasmValue::I32(0)]);
        assert_eq!(i.call("grow", &[WasmValue::I32(3)]).unwrap(), Vec::new());
        assert_eq!(i.call("size", &[]).unwrap(), vec![WasmValue::I32(0)]);
        assert_eq!(i.call("grow", &[WasmValue::I32(1)]).unwrap(), Vec::new());
        assert_eq!(i.call("size", &[]).unwrap(), vec![WasmValue::I32(1)]);
        assert_eq!(i.call("sizen", &[]).unwrap(), vec![WasmValue::I32(0)]);
        assert_eq!(i.call("grow", &[WasmValue::I32(4)]).unwrap(), Vec::new());
        assert_eq!(i.call("size", &[]).unwrap(), vec![WasmValue::I32(1)]);
    }

    #[test]
    fn test_i16x8_relaxed_q15mulr_s() {
        let m = parse_module(&build_single_fn(
            &[0x7b, 0x7b],
            &[0x7b],
            &[
                0x20, 0x00,
                0x20, 0x01,
                0xfd, 0x91, 0x02,
            ],
            None,
            &[],
            "q15",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        let a = i16x8_bytes([-32768, -32767, 32767, 0, 0, 0, 0, 0]);
        let b = i16x8_bytes([-32768, -32768, 32767, 0, 0, 0, 0, 0]);
        assert_eq!(
            i.call("q15", &[WasmValue::V128(a), WasmValue::V128(b)])
                .unwrap(),
            vec![WasmValue::V128(i16x8_bytes([
                32767, 32767, 32766, 0, 0, 0, 0, 0
            ]))]
        );
    }

    #[test]
    fn test_i16x8_q15mulr_sat_s() {
        let m = parse_module(&build_single_fn(
            &[0x7b, 0x7b],
            &[0x7b],
            &[
                0x20, 0x00,
                0x20, 0x01,
                0xfd, 0x82, 0x01,
            ],
            None,
            &[],
            "q15",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        let a = i16x8_bytes([-32768, -32767, 32767, 0, 0, 0, 0, 0]);
        let b = i16x8_bytes([-32768, -32768, 32767, 0, 0, 0, 0, 0]);
        assert_eq!(
            i.call("q15", &[WasmValue::V128(a), WasmValue::V128(b)])
                .unwrap(),
            vec![WasmValue::V128(i16x8_bytes([
                32767, 32767, 32766, 0, 0, 0, 0, 0
            ]))]
        );
    }

    #[test]
    fn test_i8x16_relaxed_swizzle() {
        let m = parse_module(&build_single_fn(
            &[0x7b, 0x7b],
            &[0x7b],
            &[
                0x20, 0x00,
                0x20, 0x01,
                0xfd, 0x80, 0x02,
            ],
            None,
            &[],
            "swizzle",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        let a = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let indices = [15, 0, 16, 1, 17, 2, 18, 3, 19, 4, 20, 5, 21, 6, 22, 7];
        assert_eq!(
            i.call("swizzle", &[WasmValue::V128(a), WasmValue::V128(indices)])
                .unwrap(),
            vec![WasmValue::V128([
                15, 0, 0, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7
            ])]
        );
    }

    #[test]
    fn test_i32x4_relaxed_trunc_f32x4_s() {
        let m = parse_module(&build_single_fn(
            &[0x7b],
            &[0x7b],
            &[
                0x20, 0x00,
                0xfd, 0x81, 0x02,
            ],
            None,
            &[],
            "trunc",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        let input = f32x4_bytes([1.9, -1.9, 2147483648.0, f32::NAN]);
        assert_eq!(
            i.call("trunc", &[WasmValue::V128(input)]).unwrap(),
            vec![WasmValue::V128(i32x4_bytes([1, -1, i32::MAX, 0]))]
        );
    }

    #[test]
    fn test_i8x16_relaxed_laneselect() {
        let m = parse_module(&build_single_fn(
            &[0x7b, 0x7b, 0x7b],
            &[0x7b],
            &[
                0x20, 0x00,
                0x20, 0x01,
                0x20, 0x02,
                0xfd, 0x89, 0x02,
            ],
            None,
            &[],
            "laneselect",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        let a = [0x10; 16];
        let b = [0x20; 16];
        let mask = [
            0xff, 0, 0xf0, 0x0f, 0xff, 0, 0xf0, 0x0f, 0xff, 0, 0xf0, 0x0f, 0xff, 0, 0xf0, 0x0f,
        ];
        assert_eq!(
            i.call(
                "laneselect",
                &[
                    WasmValue::V128(a),
                    WasmValue::V128(b),
                    WasmValue::V128(mask)
                ]
            )
            .unwrap(),
            vec![WasmValue::V128([
                0x10, 0x20, 0x10, 0x20, 0x10, 0x20, 0x10, 0x20, 0x10, 0x20, 0x10, 0x20, 0x10, 0x20,
                0x10, 0x20
            ])]
        );
    }

    #[test]
    fn test_f32x4_relaxed_min() {
        let m = parse_module(&build_single_fn(
            &[0x7b, 0x7b],
            &[0x7b],
            &[
                0x20, 0x00,
                0x20, 0x01,
                0xfd, 0x8d, 0x02,
            ],
            None,
            &[],
            "min",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        let a = f32x4_bytes([3.0, -2.0, 5.0, 8.0]);
        let b = f32x4_bytes([4.0, -4.0, 1.0, 9.0]);
        assert_eq!(
            i.call("min", &[WasmValue::V128(a), WasmValue::V128(b)])
                .unwrap(),
            vec![WasmValue::V128(f32x4_bytes([3.0, -4.0, 1.0, 8.0]))]
        );
    }

    #[test]
    fn test_relaxed_dot_product_i8x16() {
        let m = parse_module(&build_single_fn(
            &[0x7b, 0x7b],
            &[0x7b],
            &[
                0x20, 0x00,
                0x20, 0x01,
                0xfd, 0x92, 0x02,
            ],
            None,
            &[],
            "dot",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        let a = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        assert_eq!(
            i.call("dot", &[WasmValue::V128(a), WasmValue::V128(a)])
                .unwrap(),
            vec![WasmValue::V128(i16x8_bytes([
                1, 13, 41, 85, 145, 221, 313, 421
            ]))]
        );
    }

    #[test]
    fn test_relaxed_dot_product_i8x16_add() {
        let m = parse_module(&build_single_fn(
            &[0x7b, 0x7b, 0x7b],
            &[0x7b],
            &[
                0x20, 0x00,
                0x20, 0x01,
                0x20, 0x02,
                0xfd, 0x93, 0x02,
            ],
            None,
            &[],
            "dot_add",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        let a = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let addend = i32x4_bytes([0, 1, 2, 3]);
        assert_eq!(
            i.call(
                "dot_add",
                &[
                    WasmValue::V128(a),
                    WasmValue::V128(a),
                    WasmValue::V128(addend)
                ]
            )
            .unwrap(),
            vec![WasmValue::V128(i32x4_bytes([14, 127, 368, 737]))]
        );
    }

    #[test]
    fn test_i32x4_extmul_high_i16x8_s() {
        let m = parse_module(&build_single_fn(
            &[0x7b, 0x7b],
            &[0x7b],
            &[
                0x20, 0x00,
                0x20, 0x01,
                0xfd, 0xbd, 0x01,
            ],
            None,
            &[],
            "extmul",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        let a = i16x8_bytes([1, 2, 3, 4, -5, -6, 7, 8]);
        let b = i16x8_bytes([9, 10, 11, 12, 13, -14, -15, 16]);
        assert_eq!(
            i.call("extmul", &[WasmValue::V128(a), WasmValue::V128(b)])
                .unwrap(),
            vec![WasmValue::V128(i32x4_bytes([-65, 84, -105, 128]))]
        );
    }

    #[test]
    fn test_i64x2_extmul_high_i32x4_u() {
        let m = parse_module(&build_single_fn(
            &[0x7b, 0x7b],
            &[0x7b],
            &[
                0x20, 0x00,
                0x20, 0x01,
                0xfd, 0xdf, 0x01,
            ],
            None,
            &[],
            "extmul",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        let a = i32x4_bytes([1, 2, -1, 8]);
        let b = i32x4_bytes([9, 10, 2, 16]);
        assert_eq!(
            i.call("extmul", &[WasmValue::V128(a), WasmValue::V128(b)])
                .unwrap(),
            vec![WasmValue::V128(i64x2_bytes([8589934590, 128]))]
        );
    }

    #[test]
    fn test_i32x4_extadd_pairwise_i16x8_s() {
        let m = parse_module(&build_single_fn(
            &[0x7b],
            &[0x7b],
            &[
                0x20, 0x00,
                0xfd, 0x7e,
            ],
            None,
            &[],
            "extadd",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        let input = i16x8_bytes([1, -2, 300, 400, -32768, 10, 1234, -34]);
        assert_eq!(
            i.call("extadd", &[WasmValue::V128(input)]).unwrap(),
            vec![WasmValue::V128(i32x4_bytes([-1, 700, -32758, 1200]))]
        );
    }

    #[test]
    fn test_f32x4_nearest_ties_even() {
        let m = parse_module(&build_single_fn(
            &[0x7b],
            &[0x7b],
            &[
                0x20, 0x00,
                0xfd, 0x6a,
            ],
            None,
            &[],
            "nearest",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        let a = f32x4_bytes([1.5, 2.5, -1.5, -2.5]);
        assert_eq!(
            i.call("nearest", &[WasmValue::V128(a)]).unwrap(),
            vec![WasmValue::V128(f32x4_bytes([2.0, 2.0, -2.0, -2.0]))]
        );
    }

    #[test]
    fn test_f64x2_ceil() {
        let m = parse_module(&build_single_fn(
            &[0x7b],
            &[0x7b],
            &[
                0x20, 0x00,
                0xfd, 0x74,
            ],
            None,
            &[],
            "ceil",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        let a = f64x2_bytes([-1.25, 2.25]);
        assert_eq!(
            i.call("ceil", &[WasmValue::V128(a)]).unwrap(),
            vec![WasmValue::V128(f64x2_bytes([-1.0, 3.0]))]
        );
    }

    #[test]
    fn test_f32x4_relaxed_madd() {
        let m = parse_module(&build_single_fn(
            &[0x7b, 0x7b, 0x7b],
            &[0x7b],
            &[
                0x20, 0x00,
                0x20, 0x01,
                0x20, 0x02,
                0xfd, 0x85, 0x02,
            ],
            None,
            &[],
            "madd",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        let x = f32x4_bytes([2.0, 3.0, -4.0, -5.0]);
        let y = f32x4_bytes([4.0, -2.0, 3.0, -6.0]);
        let z = f32x4_bytes([1.0, 10.0, -1.0, 2.0]);
        assert_eq!(
            i.call(
                "madd",
                &[WasmValue::V128(x), WasmValue::V128(y), WasmValue::V128(z)]
            )
            .unwrap(),
            vec![WasmValue::V128(f32x4_bytes([9.0, 4.0, -13.0, 32.0]))]
        );
    }

    #[test]
    fn test_f64x2_relaxed_nmadd() {
        let m = parse_module(&build_single_fn(
            &[0x7b, 0x7b, 0x7b],
            &[0x7b],
            &[
                0x20, 0x00,
                0x20, 0x01,
                0x20, 0x02,
                0xfd, 0x88, 0x02,
            ],
            None,
            &[],
            "nmadd",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        let x = f64x2_bytes([2.0, -3.0]);
        let y = f64x2_bytes([4.0, 5.0]);
        let z = f64x2_bytes([1.0, 10.0]);
        assert_eq!(
            i.call(
                "nmadd",
                &[WasmValue::V128(x), WasmValue::V128(y), WasmValue::V128(z)]
            )
            .unwrap(),
            vec![WasmValue::V128(f64x2_bytes([-7.0, 25.0]))]
        );
    }

    #[test]
    fn test_gc_array_command5_basic_instructions() {
        let bytes: &[u8] = &[
            0, 97, 115, 109, 1, 0, 0, 0, 1, 48, 9, 94, 125, 0, 94, 125, 1, 96, 0, 1, 100, 0, 96, 2,
            127, 100, 0, 1, 125, 96, 1, 127, 1, 125, 96, 3, 127, 100, 1, 125, 1, 125, 96, 2, 127,
            125, 1, 125, 96, 1, 100, 106, 1, 127, 96, 0, 1, 127, 3, 8, 7, 2, 3, 4, 5, 6, 7, 8, 6,
            24, 2, 100, 0, 0, 67, 0, 0, 128, 63, 65, 3, 251, 6, 0, 11, 100, 0, 0, 65, 3, 251, 7, 0,
            11, 7, 29, 4, 3, 110, 101, 119, 0, 0, 3, 103, 101, 116, 0, 2, 7, 115, 101, 116, 95,
            103, 101, 116, 0, 4, 3, 108, 101, 110, 0, 6, 10, 75, 7, 7, 0, 65, 3, 251, 7, 0, 11, 9,
            0, 32, 1, 32, 0, 251, 11, 0, 11, 8, 0, 32, 0, 16, 0, 16, 1, 11, 18, 0, 32, 1, 32, 0,
            32, 2, 251, 14, 1, 32, 1, 32, 0, 251, 11, 1, 11, 13, 0, 32, 0, 65, 3, 251, 7, 1, 32, 1,
            16, 3, 11, 6, 0, 32, 0, 251, 15, 11, 6, 0, 16, 0, 16, 5, 11,
        ];
        let m = parse_module(bytes).unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(
            i.call("get", &[WasmValue::I32(0)]).unwrap(),
            vec![WasmValue::F32(0.0)]
        );
        assert_eq!(
            i.call("set_get", &[WasmValue::I32(1), WasmValue::F32(4.5)])
                .unwrap(),
            vec![WasmValue::F32(4.5)]
        );
        assert_eq!(i.call("len", &[]).unwrap(), vec![WasmValue::I32(3)]);
    }

    #[test]
    fn test_gc_array_command13_new_fixed() {
        let bytes: &[u8] = &[
            0, 97, 115, 109, 1, 0, 0, 0, 1, 48, 9, 94, 125, 0, 94, 125, 1, 96, 0, 1, 100, 0, 96, 2,
            127, 100, 0, 1, 125, 96, 1, 127, 1, 125, 96, 3, 127, 100, 1, 125, 1, 125, 96, 2, 127,
            125, 1, 125, 96, 1, 100, 106, 1, 127, 96, 0, 1, 127, 3, 8, 7, 2, 3, 4, 5, 6, 7, 8, 6,
            19, 1, 100, 0, 0, 67, 0, 0, 128, 63, 67, 0, 0, 0, 64, 251, 8, 0, 2, 11, 7, 29, 4, 3,
            110, 101, 119, 0, 0, 3, 103, 101, 116, 0, 2, 7, 115, 101, 116, 95, 103, 101, 116, 0, 4,
            3, 108, 101, 110, 0, 6, 10, 98, 7, 16, 0, 67, 0, 0, 128, 63, 67, 0, 0, 0, 64, 251, 8,
            0, 2, 11, 9, 0, 32, 1, 32, 0, 251, 11, 0, 11, 8, 0, 32, 0, 16, 0, 16, 1, 11, 18, 0, 32,
            1, 32, 0, 32, 2, 251, 14, 1, 32, 1, 32, 0, 251, 11, 1, 11, 27, 0, 32, 0, 67, 0, 0, 128,
            63, 67, 0, 0, 0, 64, 67, 0, 0, 64, 64, 251, 8, 1, 3, 32, 1, 16, 3, 11, 6, 0, 32, 0,
            251, 15, 11, 6, 0, 16, 0, 16, 5, 11,
        ];
        let m = parse_module(bytes).unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(
            i.call("get", &[WasmValue::I32(0)]).unwrap(),
            vec![WasmValue::F32(1.0)]
        );
        assert_eq!(
            i.call("get", &[WasmValue::I32(1)]).unwrap(),
            vec![WasmValue::F32(2.0)]
        );
        assert_eq!(
            i.call("set_get", &[WasmValue::I32(1), WasmValue::F32(7.0)])
                .unwrap(),
            vec![WasmValue::F32(7.0)]
        );
        assert_eq!(i.call("len", &[]).unwrap(), vec![WasmValue::I32(2)]);
    }

    #[test]
    fn test_gc_array_command21_new_data_i8() {
        let bytes: &[u8] = &[
            0, 97, 115, 109, 1, 0, 0, 0, 1, 51, 10, 94, 120, 0, 94, 120, 1, 96, 0, 1, 100, 0, 96,
            2, 127, 100, 0, 1, 127, 96, 1, 127, 1, 127, 96, 3, 127, 100, 1, 127, 1, 127, 96, 2,
            127, 127, 1, 127, 96, 1, 100, 106, 1, 127, 96, 0, 1, 127, 96, 0, 0, 3, 12, 11, 2, 2, 3,
            4, 3, 4, 5, 6, 7, 8, 9, 7, 66, 7, 3, 110, 101, 119, 0, 0, 12, 110, 101, 119, 45, 111,
            118, 101, 114, 102, 108, 111, 119, 0, 1, 5, 103, 101, 116, 95, 117, 0, 3, 5, 103, 101,
            116, 95, 115, 0, 5, 7, 115, 101, 116, 95, 103, 101, 116, 0, 7, 3, 108, 101, 110, 0, 9,
            9, 100, 114, 111, 112, 95, 115, 101, 103, 115, 0, 10, 12, 1, 1, 10, 125, 11, 10, 0, 65,
            1, 65, 3, 251, 9, 0, 0, 11, 18, 0, 65, 128, 128, 128, 128, 120, 65, 128, 128, 128, 128,
            120, 251, 9, 0, 0, 11, 9, 0, 32, 1, 32, 0, 251, 13, 0, 11, 8, 0, 32, 0, 16, 0, 16, 2,
            11, 9, 0, 32, 1, 32, 0, 251, 12, 0, 11, 8, 0, 32, 0, 16, 0, 16, 4, 11, 18, 0, 32, 1,
            32, 0, 32, 2, 251, 14, 1, 32, 1, 32, 0, 251, 13, 1, 11, 16, 0, 32, 0, 65, 1, 65, 3,
            251, 9, 1, 0, 32, 1, 16, 6, 11, 6, 0, 32, 0, 251, 15, 11, 6, 0, 16, 0, 16, 8, 11, 5, 0,
            252, 9, 0, 11, 11, 8, 1, 1, 5, 0, 1, 2, 255, 4,
        ];
        let m = parse_module(bytes).unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(
            i.call("get_u", &[WasmValue::I32(0)]).unwrap(),
            vec![WasmValue::I32(1)]
        );
        assert_eq!(
            i.call("get_u", &[WasmValue::I32(2)]).unwrap(),
            vec![WasmValue::I32(255)]
        );
        assert_eq!(
            i.call("get_s", &[WasmValue::I32(2)]).unwrap(),
            vec![WasmValue::I32(-1)]
        );
        assert_eq!(
            i.call("set_get", &[WasmValue::I32(1), WasmValue::I32(0x1234)])
                .unwrap(),
            vec![WasmValue::I32(0x34)]
        );
        assert_eq!(i.call("len", &[]).unwrap(), vec![WasmValue::I32(3)]);
    }

    #[test]
    fn test_gc_array_new_data_i32_little_endian() {
        let m = parse_module(&[
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x5e, 0x7f, 0x01,
            0x60, 0x00, 0x01, 0x7f, 0x03, 0x02, 0x01, 0x01, 0x07, 0x07, 0x01, 0x03, 0x67, 0x65,
            0x74, 0x00, 0x00, 0x0c, 0x01, 0x01, 0x0a, 0x11, 0x01, 0x0f, 0x00, 0x41, 0x00, 0x41,
            0x01, 0xfb, 0x09, 0x00, 0x00, 0x41, 0x00, 0xfb, 0x0b, 0x00, 0x0b, 0x0b, 0x07, 0x01,
            0x01, 0x04, 0xaa, 0xbb, 0xcc, 0xdd, 0x00, 0x13, 0x04, 0x6e, 0x61, 0x6d, 0x65, 0x04,
            0x06, 0x01, 0x00, 0x03, 0x61, 0x72, 0x72, 0x09, 0x04, 0x01, 0x00, 0x01, 0x64,
        ])
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(
            i.call("get", &[]).unwrap(),
            vec![WasmValue::I32(0xddccbbaa_u32 as i32)]
        );
    }

    #[test]
    fn test_gc_array_command35_new_elem() {
        let bytes: &[u8] = &[
            0, 97, 115, 109, 1, 0, 0, 0, 1, 77, 15, 94, 120, 0, 94, 100, 0, 0, 94, 100, 0, 1, 94,
            99, 0, 0, 94, 110, 1, 96, 0, 1, 100, 1, 96, 0, 1, 100, 3, 96, 0, 1, 100, 4, 96, 3, 127,
            127, 100, 1, 1, 127, 96, 2, 127, 127, 1, 127, 96, 4, 127, 127, 100, 2, 127, 1, 127, 96,
            3, 127, 127, 127, 1, 127, 96, 1, 100, 106, 1, 127, 96, 0, 1, 127, 96, 0, 0, 3, 12, 11,
            5, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 7, 56, 6, 3, 110, 101, 119, 0, 0, 12, 110, 101,
            119, 45, 111, 118, 101, 114, 102, 108, 111, 119, 0, 1, 3, 103, 101, 116, 0, 5, 7, 115,
            101, 116, 95, 103, 101, 116, 0, 7, 3, 108, 101, 110, 0, 9, 9, 100, 114, 111, 112, 95,
            115, 101, 103, 115, 0, 10, 9, 22, 1, 5, 100, 0, 2, 65, 7, 65, 3, 251, 6, 0, 11, 65, 1,
            65, 2, 251, 8, 0, 2, 11, 10, 147, 1, 11, 10, 0, 65, 0, 65, 2, 251, 10, 1, 0, 11, 18, 0,
            65, 128, 128, 128, 128, 120, 65, 128, 128, 128, 128, 120, 251, 10, 1, 0, 11, 10, 0, 65,
            0, 65, 2, 251, 10, 3, 0, 11, 10, 0, 65, 0, 65, 2, 251, 10, 4, 0, 11, 14, 0, 32, 2, 32,
            0, 251, 11, 1, 32, 1, 251, 13, 0, 11, 10, 0, 32, 0, 32, 1, 16, 0, 16, 4, 11, 28, 0, 32,
            2, 32, 0, 32, 2, 32, 3, 251, 11, 2, 251, 14, 2, 32, 2, 32, 0, 251, 11, 2, 32, 1, 251,
            13, 0, 11, 18, 0, 32, 0, 32, 1, 65, 0, 65, 2, 251, 10, 2, 0, 32, 2, 16, 6, 11, 6, 0,
            32, 0, 251, 15, 11, 6, 0, 16, 0, 16, 8, 11, 5, 0, 252, 13, 0, 11,
        ];
        let m = parse_module(bytes).unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(
            i.call("get", &[WasmValue::I32(0), WasmValue::I32(0)])
                .unwrap(),
            vec![WasmValue::I32(7)]
        );
        assert_eq!(
            i.call("get", &[WasmValue::I32(1), WasmValue::I32(1)])
                .unwrap(),
            vec![WasmValue::I32(2)]
        );
        assert_eq!(
            i.call(
                "set_get",
                &[WasmValue::I32(0), WasmValue::I32(1), WasmValue::I32(1)]
            )
            .unwrap(),
            vec![WasmValue::I32(2)]
        );
        assert_eq!(i.call("len", &[]).unwrap(), vec![WasmValue::I32(2)]);
    }

    fn build_array_func(
        storage: u8,
        mutable: bool,
        params: &[u8],
        results: &[u8],
        locals: &[&[u8]],
        body_instrs: &[u8],
        data: Option<&[u8]>,
        export_name: &str,
    ) -> Vec<u8> {
        let mut typesec = Vec::new();
        uleb(2, &mut typesec);
        typesec.push(0x5e);
        typesec.push(storage);
        typesec.push(mutable as u8);
        typesec.push(0x60);
        uleb(params.len() as u64, &mut typesec);
        typesec.extend_from_slice(params);
        uleb(results.len() as u64, &mut typesec);
        typesec.extend_from_slice(results);

        let mut funcsec = Vec::new();
        uleb(1, &mut funcsec);
        uleb(1, &mut funcsec);

        let mut exportsec = Vec::new();
        uleb(1, &mut exportsec);
        uleb(export_name.len() as u64, &mut exportsec);
        exportsec.extend_from_slice(export_name.as_bytes());
        exportsec.push(0x00);
        uleb(0, &mut exportsec);

        let mut body = Vec::new();
        uleb(locals.len() as u64, &mut body);
        for local_ty in locals {
            uleb(1, &mut body);
            body.extend_from_slice(local_ty);
        }
        body.extend_from_slice(body_instrs);
        body.push(0x0b);

        let mut codesec = Vec::new();
        uleb(1, &mut codesec);
        uleb(body.len() as u64, &mut codesec);
        codesec.extend(body);

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(3, funcsec));
        bytes.extend(section(7, exportsec));
        if data.is_some() {
            let mut datacountsec = Vec::new();
            uleb(1, &mut datacountsec);
            bytes.extend(section(12, datacountsec));
        }
        bytes.extend(section(10, codesec));
        if let Some(data) = data {
            let mut datasec = Vec::new();
            uleb(1, &mut datasec);
            uleb(1, &mut datasec);
            uleb(data.len() as u64, &mut datasec);
            datasec.extend_from_slice(data);
            bytes.extend(section(11, datasec));
        }
        bytes
    }

    fn build_struct_func(
        field_storage: u8,
        mutable: bool,
        params: &[u8],
        results: &[u8],
        body_instrs: &[u8],
        export_name: &str,
    ) -> Vec<u8> {
        let mut typesec = Vec::new();
        uleb(2, &mut typesec);
        typesec.push(0x5f);
        uleb(1, &mut typesec);
        typesec.push(field_storage);
        typesec.push(mutable as u8);
        typesec.push(0x60);
        uleb(params.len() as u64, &mut typesec);
        typesec.extend_from_slice(params);
        uleb(results.len() as u64, &mut typesec);
        typesec.extend_from_slice(results);

        let mut funcsec = Vec::new();
        uleb(1, &mut funcsec);
        uleb(1, &mut funcsec);

        let mut exportsec = Vec::new();
        uleb(1, &mut exportsec);
        uleb(export_name.len() as u64, &mut exportsec);
        exportsec.extend_from_slice(export_name.as_bytes());
        exportsec.push(0x00);
        uleb(0, &mut exportsec);

        let mut body = Vec::new();
        uleb(0, &mut body);
        body.extend_from_slice(body_instrs);
        body.push(0x0b);

        let mut codesec = Vec::new();
        uleb(1, &mut codesec);
        uleb(body.len() as u64, &mut codesec);
        codesec.extend(body);

        let mut bytes = header();
        bytes.extend(section(1, typesec));
        bytes.extend(section(3, funcsec));
        bytes.extend(section(7, exportsec));
        bytes.extend(section(10, codesec));
        bytes
    }

    #[test]
    fn test_gc_struct_new_get_s_i16_executes() {
        let m = parse_module(&build_struct_func(
            0x77,
            false,
            &[0x7f],
            &[0x7f],
            &[
                0x20, 0x00,
                0xfb, 0x00, 0x00,
                0xfb, 0x03, 0x00, 0x00,
            ],
            "field",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(
            i.call("field", &[WasmValue::I32(0xffff)]).unwrap(),
            vec![WasmValue::I32(-1)]
        );
    }

    #[test]
    fn test_gc_array_fill_executes() {
        let m = parse_module(&build_array_func(
            0x7f,
            true,
            &[0x7f, 0x7f],
            &[0x7f],
            &[&[0x64, 0x00]],
            &[
                0x41, 0x00, 0x41, 0x03, 0xfb, 0x06, 0x00,
                0x21, 0x02,
                0x20, 0x02, 0x41, 0x01, 0x20, 0x00, 0x41, 0x02, 0xfb, 0x10,
                0x00,
                0x20, 0x02, 0x20, 0x01, 0xfb, 0x0b, 0x00,
            ],
            None,
            "fill_get",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(
            i.call("fill_get", &[WasmValue::I32(9), WasmValue::I32(2)])
                .unwrap(),
            vec![WasmValue::I32(9)]
        );
    }

    #[test]
    fn test_gc_array_copy_executes_overlapping_copy() {
        let m = parse_module(&build_array_func(
            0x7f,
            true,
            &[],
            &[0x7f],
            &[&[0x64, 0x00]],
            &[
                0x41, 0x01, 0x41, 0x02, 0x41, 0x03, 0x41, 0x04, 0xfb, 0x08, 0x00,
                0x04,
                0x21, 0x00,
                0x20, 0x00, 0x41, 0x02, 0x20, 0x00, 0x41, 0x00, 0x41, 0x02, 0xfb, 0x11, 0x00,
                0x00,
                0x20, 0x00, 0x41, 0x03, 0xfb, 0x0b, 0x00,
            ],
            None,
            "copy_get",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(i.call("copy_get", &[]).unwrap(), vec![WasmValue::I32(2)]);
    }

    #[test]
    fn test_gc_array_init_data_executes() {
        let m = parse_module(&build_array_func(
            0x78,
            true,
            &[],
            &[0x7f],
            &[&[0x64, 0x00]],
            &[
                0x41, 0x00, 0x41, 0x03, 0xfb, 0x06, 0x00,
                0x21, 0x00,
                0x20, 0x00, 0x41, 0x01, 0x41, 0x01, 0x41, 0x02, 0xfb, 0x12, 0x00,
                0x00,
                0x20, 0x00, 0x41, 0x02, 0xfb, 0x0d, 0x00,
            ],
            Some(&[5, 6, 7]),
            "init_data_get",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(
            i.call("init_data_get", &[]).unwrap(),
            vec![WasmValue::I32(7)]
        );
    }

    #[test]
    fn test_gc_i31_get_signed_and_unsigned() {
        let m = parse_module(&build_single_fn(
            &[0x7f],
            &[0x7f],
            &[
                0x20, 0x00,
                0xfb, 0x1c,
                0xfb, 0x1e,
            ],
            None,
            &[],
            "get_u",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(
            i.call("get_u", &[WasmValue::I32(-1)]).unwrap(),
            vec![WasmValue::I32(0x7fff_ffff)]
        );

        let m = parse_module(&build_single_fn(
            &[0x7f],
            &[0x7f],
            &[
                0x20, 0x00,
                0xfb, 0x1c,
                0xfb, 0x1d,
            ],
            None,
            &[],
            "get_s",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(
            i.call("get_s", &[WasmValue::I32(-1)]).unwrap(),
            vec![WasmValue::I32(-1)]
        );
        assert_eq!(
            i.call("get_s", &[WasmValue::I32(0x4000_0000)]).unwrap(),
            vec![WasmValue::I32(-0x4000_0000)]
        );
    }

    #[test]
    fn test_gc_i31_get_null_traps() {
        let m = parse_module(&build_single_fn(
            &[],
            &[0x7f],
            &[
                0xd0, 0x69,
                0xfb, 0x1e,
            ],
            None,
            &[],
            "get_null",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        let err = i.call("get_null", &[]).unwrap_err();
        assert!(err.contains("null i31 reference"), "{err}");
    }

    #[test]
    fn test_gc_ref_test_i31_executes() {
        let m = parse_module(&build_single_fn(
            &[0x7f],
            &[0x7f],
            &[
                0x20, 0x00,
                0xfb, 0x1c,
                0xfb, 0x15, 0x6c,
            ],
            None,
            &[],
            "is_i31",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(
            i.call("is_i31", &[WasmValue::I32(7)]).unwrap(),
            vec![WasmValue::I32(1)]
        );
    }

    #[test]
    fn test_gc_ref_test_accepts_type_index_heap_immediate() {
        let m = parse_module(&build_array_func(
            0x7f,
            false,
            &[],
            &[0x7f],
            &[],
            &[
                0x41, 0x00, 0x41, 0x00, 0xfb, 0x06, 0x00,
                0xfb, 0x15, 0x00,
            ],
            None,
            "is_type0",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(i.call("is_type0", &[]).unwrap(), vec![WasmValue::I32(1)]);
    }

    #[test]
    fn test_gc_ref_test_any_accepts_arrayref() {
        let m = parse_module(&build_array_func(
            0x7f,
            false,
            &[],
            &[0x7f],
            &[],
            &[
                0x41, 0x00, 0x41, 0x00, 0xfb, 0x06, 0x00,
                0xfb, 0x15, 0x6e,
            ],
            None,
            "is_any",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(i.call("is_any", &[]).unwrap(), vec![WasmValue::I32(1)]);
    }

    #[test]
    fn test_gc_any_convert_extern_preserves_null_reference() {
        let m = parse_module(&build_single_fn(
            &[0x6f],
            &[0x7f],
            &[
                0x20, 0x00,
                0xfb, 0x1a,
                0xd1,
            ],
            None,
            &[],
            "is_null",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(
            i.call("is_null", &[WasmValue::RefNull]).unwrap(),
            vec![WasmValue::I32(1)]
        );
    }

    #[test]
    fn test_gc_any_convert_extern_matches_anyref() {
        let m = parse_module(&build_single_fn(
            &[0x6f],
            &[0x7f],
            &[
                0x20, 0x00,
                0xfb, 0x1a,
                0xfb, 0x15, 0x6e,
            ],
            None,
            &[],
            "is_any",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(
            i.call("is_any", &[WasmValue::ExternRef(7)]).unwrap(),
            vec![WasmValue::I32(1)]
        );
    }

    #[test]
    fn test_gc_extern_convert_any_accepts_i31_reference() {
        let m = parse_module(&build_single_fn(
            &[],
            &[0x7f],
            &[
                0x41, 0x09,
                0xfb, 0x1c,
                0xfb, 0x1b,
                0xd1,
            ],
            None,
            &[],
            "converted_is_null",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(
            i.call("converted_is_null", &[]).unwrap(),
            vec![WasmValue::I32(0)]
        );
    }

    #[test]
    fn test_gc_extern_convert_any_matches_externref() {
        let m = parse_module(&build_single_fn(
            &[],
            &[0x7f],
            &[
                0x41, 0x09,
                0xfb, 0x1c,
                0xfb, 0x1b,
                0xfb, 0x15, 0x6f,
            ],
            None,
            &[],
            "is_extern",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(i.call("is_extern", &[]).unwrap(), vec![WasmValue::I32(1)]);
    }

    #[test]
    fn test_gc_ref_cast_i31_rejects_arrayref() {
        let m = parse_module(&build_array_func(
            0x7f,
            false,
            &[],
            &[],
            &[],
            &[
                0x41, 0x00, 0x41, 0x00, 0xfb, 0x06, 0x00,
                0xfb, 0x17, 0x6c,
                0x1a,
            ],
            None,
            "cast",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        let err = i.call("cast", &[]).unwrap_err();
        assert!(err.contains("ref.cast: cast failure"), "{err}");
    }

    #[test]
    fn test_i16x8_narrow_i32x4_s() {
        let m = parse_module(&build_single_fn(
            &[0x7b, 0x7b],
            &[0x7b],
            &[
                0x20, 0x00,
                0x20, 0x01,
                0xfd, 0x85, 0x01,
            ],
            None,
            &[],
            "narrow",
        ))
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        let a = i32x4_bytes([i32::MIN, -123, 123, i32::MAX]);
        let b = i32x4_bytes([-40000, -32768, 32767, 40000]);
        assert_eq!(
            i.call("narrow", &[WasmValue::V128(a), WasmValue::V128(b)])
                .unwrap(),
            vec![WasmValue::V128(i16x8_bytes([
                -32768, -123, 123, 32767, -32768, -32768, 32767, 32767
            ]))]
        );
    }

    #[test]
    fn test_gc_struct_const_expr_global_initializer() {
        let m = parse_module(&[
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x0b, 0x02, 0x5f, 0x02, 0x7f,
            0x00, 0x7f, 0x01, 0x60, 0x00, 0x01, 0x7f, 0x03, 0x02, 0x01, 0x01, 0x06, 0x0c, 0x01,
            0x64, 0x00, 0x00, 0x41, 0x07, 0x41, 0x09, 0xfb, 0x00, 0x00, 0x0b, 0x07, 0x07, 0x01,
            0x03, 0x67, 0x65, 0x74, 0x00, 0x00, 0x0a, 0x0a, 0x01, 0x08, 0x00, 0x23, 0x00, 0xfb,
            0x02, 0x00, 0x00, 0x0b, 0x00, 0x11, 0x04, 0x6e, 0x61, 0x6d, 0x65, 0x04, 0x04, 0x01,
            0x00, 0x01, 0x73, 0x07, 0x04, 0x01, 0x00, 0x01, 0x67,
        ])
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(i.call("get", &[]).unwrap(), vec![WasmValue::I32(7)]);
    }

    #[test]
    fn test_gc_extern_convert_const_expr_global_initializer() {
        let m = parse_module(&[
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x05, 0x01, 0x60, 0x00, 0x01,
            0x7f, 0x03, 0x02, 0x01, 0x00, 0x06, 0x0f, 0x02, 0x6f, 0x00, 0xd0, 0x6e, 0xfb, 0x1b,
            0x0b, 0x6e, 0x00, 0xd0, 0x6f, 0xfb, 0x1a, 0x0b, 0x07, 0x0b, 0x01, 0x07, 0x69, 0x73,
            0x2d, 0x6e, 0x75, 0x6c, 0x6c, 0x00, 0x00, 0x0a, 0x07, 0x01, 0x05, 0x00, 0x23, 0x00,
            0xd1, 0x0b,
        ])
        .unwrap();
        let mut i = instantiate(&m, Imports::new()).unwrap();
        assert_eq!(i.call("is-null", &[]).unwrap(), vec![WasmValue::I32(1)]);
    }

    fn i16x8_bytes(lanes: [i16; 8]) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        for (i, lane) in lanes.iter().enumerate() {
            bytes[i * 2..i * 2 + 2].copy_from_slice(&lane.to_le_bytes());
        }
        bytes
    }

    fn i32x4_bytes(lanes: [i32; 4]) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        for (i, lane) in lanes.iter().enumerate() {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&lane.to_le_bytes());
        }
        bytes
    }

    fn i64x2_bytes(lanes: [i64; 2]) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        for (i, lane) in lanes.iter().enumerate() {
            bytes[i * 8..i * 8 + 8].copy_from_slice(&lane.to_le_bytes());
        }
        bytes
    }

    fn f32x4_bytes(lanes: [f32; 4]) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        for (i, lane) in lanes.iter().enumerate() {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&lane.to_le_bytes());
        }
        bytes
    }

    fn f64x2_bytes(lanes: [f64; 2]) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        for (i, lane) in lanes.iter().enumerate() {
            bytes[i * 8..i * 8 + 8].copy_from_slice(&lane.to_le_bytes());
        }
        bytes
    }
}
