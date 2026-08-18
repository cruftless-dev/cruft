
use crate::leb::Reader;

const DEFAULT_MAX_EXPR_NESTING: i64 = 1024;

fn max_expr_nesting() -> i64 {
    std::env::var("CRUFT_WASM_MAX_EXPR_NESTING")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_MAX_EXPR_NESTING)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    pub fn from_byte(b: u8) -> Result<ValType, String> {
        match b {
            0x7f => Ok(ValType::I32),
            0x7e => Ok(ValType::I64),
            0x7d => Ok(ValType::F32),
            0x7c => Ok(ValType::F64),
            0x7b => Ok(ValType::V128),
            0x6e => Ok(ValType::AnyRef),
            0x6d => Ok(ValType::EqRef),
            0x70 => Ok(ValType::FuncRef),
            0x6f => Ok(ValType::ExternRef),
            0x6a => Ok(ValType::ArrayRef),
            0x6c => Ok(ValType::I31Ref),
            0x71 | 0x74 => Ok(ValType::NullRef),
            0x72 => Ok(ValType::NullExternRef),
            0x73 => Ok(ValType::NullFuncRef),
            0x69 => Ok(ValType::NullRef),
            0x6b => Ok(ValType::StructRef),
            other => Err(format!("unknown valtype 0x{:02x}", other)),
        }
    }
}

fn read_heap_type(r: &mut Reader) -> Result<ValType, String> {
    match r.byte()? {
        0x70 => Ok(ValType::FuncRef),
        0x6f => Ok(ValType::ExternRef),
        0x6e => Ok(ValType::AnyRef),
        0x6d => Ok(ValType::EqRef),
        0x6b => Ok(ValType::StructRef),
        0x6a => Ok(ValType::ArrayRef),
        0x6c => Ok(ValType::I31Ref),
        0x71 | 0x74 => Ok(ValType::NullRef),
        0x72 => Ok(ValType::NullExternRef),
        0x73 => Ok(ValType::NullFuncRef),
        0x69 => Ok(ValType::NullRef),

        b @ 0x00..=0x3f => Ok(ValType::TypeRef(b as u32)),
        other => Err(format!("unsupported reference heap type 0x{:02x}", other)),
    }
}

fn read_non_null_heap_type(r: &mut Reader) -> Result<ValType, String> {
    Ok(non_null_heap_type(read_heap_type(r)?))
}

fn non_null_heap_type(ty: ValType) -> ValType {
    match ty {
        ValType::FuncRef => ValType::NonNullFuncRef,
        ValType::ExternRef => ValType::NonNullExternRef,
        ValType::AnyRef => ValType::NonNullAnyRef,
        ValType::EqRef => ValType::NonNullEqRef,
        ValType::StructRef => ValType::NonNullStructRef,
        ValType::ArrayRef => ValType::NonNullArrayRef,
        ValType::I31Ref => ValType::NonNullI31Ref,
        ValType::TypeRef(idx) => ValType::NonNullTypeRef(idx),
        other => other,
    }
}

fn nullable_cast_complement_type(source: ValType, target: ValType) -> ValType {
    if nullable_abstract_cast_excludes_null(target) {
        non_null_heap_type(source)
    } else {
        source
    }
}

fn nullable_abstract_cast_excludes_null(ty: ValType) -> bool {
    matches!(
        ty,
        ValType::AnyRef
            | ValType::EqRef
            | ValType::FuncRef
            | ValType::ExternRef
            | ValType::StructRef
            | ValType::ArrayRef
            | ValType::I31Ref
    )
}

fn heap_type_with_nullability(ty: ValType, nullable: bool) -> ValType {
    if nullable {
        ty
    } else {
        non_null_heap_type(ty)
    }
}

fn read_val_type(r: &mut Reader) -> Result<ValType, String> {
    read_val_type_from_first(r.byte()?, r)
}

fn read_val_type_from_first(first: u8, r: &mut Reader) -> Result<ValType, String> {
    match first {
        0x64 => read_non_null_heap_type(r),
        0x63 => read_heap_type(r),
        b => ValType::from_byte(b),
    }
}

#[derive(Clone, Debug)]
pub struct FuncType {
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
}

#[derive(Clone, Copy, Debug)]
pub struct ArrayType {
    pub element: ValType,
    pub mutable: bool,
    pub packed_bits: Option<u8>,
}

#[derive(Clone, Copy, Debug)]
pub struct StructField {
    pub ty: ValType,
    pub mutable: bool,
    pub packed_bits: Option<u8>,
}

#[derive(Clone, Debug)]
pub struct StructType {
    pub fields: Vec<StructField>,
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub min: u64,
    pub max: Option<u64>,
    pub memory64: bool,
    pub shared: bool,
}

#[derive(Clone, Debug)]
pub enum ImportKind {
    Func(u32),
    Table(TableType),
    Memory(Limits),
    Global { ty: ValType, mutable: bool },
    Tag(u32),
}

#[derive(Clone, Debug)]
pub struct Import {
    pub module: String,
    pub name: String,
    pub kind: ImportKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportKind {
    Func,
    Table,
    Memory,
    Global,
    Tag,
}

#[derive(Clone, Debug)]
pub struct Export {
    pub name: String,
    pub kind: ExportKind,
    pub index: u32,
}

#[derive(Clone, Debug)]
pub struct Global {
    pub ty: ValType,
    pub mutable: bool,
    pub init: Vec<Instr>,
}

#[derive(Clone, Debug)]
pub struct Tag {
    pub attribute: u32,
    pub type_idx: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct TableType {
    pub elem: ValType,
    pub limits: Limits,
}

#[derive(Clone, Debug)]
pub struct ElementSegment {
    pub table: u32,
    pub offset: Option<Vec<Instr>>,
    pub items: Vec<ElementItem>,
    pub mode: ElementMode,
    pub ty: ValType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementMode {
    Active,
    Passive,
    Declarative,
}

#[derive(Clone, Debug)]
pub enum ElementItem {
    Func(u32),
    Expr(Vec<Instr>),
}

#[derive(Clone, Debug)]
pub struct DataSegment {
    pub memory: u32,

    pub offset: Option<Vec<Instr>>,
    pub bytes: Vec<u8>,

    pub passive: bool,
}

#[derive(Clone, Debug)]
pub struct Code {

    pub locals: Vec<ValType>,
    pub body: Vec<Instr>,
}

#[derive(Clone, Copy, Debug)]
pub enum BlockType {
    Empty,
    Value(ValType),
    TypeIndex(u32),
}

#[derive(Clone, Copy, Debug)]
pub struct MemArg {
    pub align: u32,
    pub memory: u32,
    pub offset: u64,
}

#[derive(Clone, Copy, Debug)]
pub enum CatchKind {
    Catch { tag: u32, label: u32 },
    CatchRef { tag: u32, label: u32 },
    CatchAll { label: u32 },
    CatchAllRef { label: u32 },
}

#[derive(Clone, Debug)]
pub enum Instr {
    Unreachable,
    Nop,
    Block(BlockType),
    Loop(BlockType),
    If(BlockType),
    LegacyTry(BlockType),
    LegacyCatch(u32),
    LegacyCatchAll,
    LegacyRethrow(u32),
    LegacyDelegate(u32),
    TryTable(BlockType, Vec<CatchKind>),
    Else,
    End,
    Br(u32),
    BrIf(u32),
    BrOnNull(u32),
    BrOnNonNull(u32),
    BrTable(Vec<u32>, u32),
    Return,
    Throw(u32),
    ThrowRef,
    Call(u32),
    ReturnCall(u32),
    CallIndirect(u32  , u32  ),
    ReturnCallIndirect(u32  , u32  ),
    CallRef(u32  ),
    ReturnCallRef(u32  ),
    ArrayNew(u32  ),
    ArrayNewDefault(u32  ),
    ArrayNewFixed(u32  , u32  ),
    ArrayNewData(u32  , u32  ),
    ArrayNewElem(u32  , u32  ),
    ArrayGet(u32  ),
    ArrayGetS(u32  ),
    ArrayGetU(u32  ),
    ArraySet(u32  ),
    ArrayLen,
    ArrayFill(u32  ),
    ArrayCopy(u32  , u32  ),
    ArrayInitData(u32  , u32  ),
    ArrayInitElem(u32  , u32  ),
    RefI31,
    I31GetS,
    I31GetU,

    Drop,
    Select,
    SelectTyped(ValType),

    LocalGet(u32),
    LocalSet(u32),
    LocalTee(u32),
    GlobalGet(u32),
    GlobalSet(u32),

    Load(u8  , MemArg),
    Store(u8, MemArg),
    AtomicLoad(u32  , MemArg),
    AtomicStore(u32, MemArg),
    AtomicRmw(u32, MemArg),
    AtomicNotify(MemArg),
    AtomicWait(u32  , MemArg),
    AtomicFence(u8  ),
    MemorySize(u32  ),
    MemoryGrow(u32  ),

    I32Const(i32),
    I64Const(i64),
    F32Const(f32),
    F64Const(f64),
    V128Const([u8; 16]),
    I8x16Shuffle([u8; 16]),
    I8x16Swizzle,
    I8x16Splat,
    I16x8Splat,
    I32x4Splat,
    I64x2Splat,
    F32x4Splat,
    F64x2Splat,
    I8x16ExtractLaneS(u8),
    I8x16ExtractLaneU(u8),
    I16x8ExtractLaneS(u8),
    I16x8ExtractLaneU(u8),
    I32x4ExtractLane(u8),
    I64x2ExtractLane(u8),
    F32x4ExtractLane(u8),
    F64x2ExtractLane(u8),
    I8x16ReplaceLane(u8),
    I16x8ReplaceLane(u8),
    I32x4ReplaceLane(u8),
    I64x2ReplaceLane(u8),
    F32x4ReplaceLane(u8),
    F64x2ReplaceLane(u8),
    I8x16Eq,
    I8x16Ne,
    I8x16LtS,
    I8x16LtU,
    I8x16GtS,
    I8x16GtU,
    I8x16LeS,
    I8x16LeU,
    I8x16GeS,
    I8x16GeU,
    I16x8Eq,
    I16x8Ne,
    I16x8LtS,
    I16x8LtU,
    I16x8GtS,
    I16x8GtU,
    I16x8LeS,
    I16x8LeU,
    I16x8GeS,
    I16x8GeU,
    I32x4Eq,
    I32x4Ne,
    I32x4LtS,
    I32x4LtU,
    I32x4GtS,
    I32x4GtU,
    I32x4LeS,
    I32x4LeU,
    I32x4GeS,
    I32x4GeU,
    I64x2Eq,
    I64x2Ne,
    I64x2LtS,
    I64x2GtS,
    I64x2LeS,
    I64x2GeS,
    F32x4Ne,
    F32x4Lt,
    F32x4Gt,
    F32x4Le,
    F32x4Ge,
    F64x2Ne,
    F64x2Lt,
    F64x2Gt,
    F64x2Le,
    F64x2Ge,
    I8x16Abs,
    I8x16Neg,
    I8x16Popcnt,
    I8x16AllTrue,
    I8x16Bitmask,
    I8x16NarrowI16x8S,
    I8x16NarrowI16x8U,
    I16x8NarrowI32x4S,
    I16x8NarrowI32x4U,
    I16x8ExtAddPairwiseI8x16S,
    I16x8ExtAddPairwiseI8x16U,
    I32x4ExtAddPairwiseI16x8S,
    I32x4ExtAddPairwiseI16x8U,
    I16x8AllTrue,
    I16x8Bitmask,
    I16x8Abs,
    I16x8Neg,
    I16x8ExtendLowI8x16S,
    I16x8ExtendHighI8x16S,
    I16x8ExtendLowI8x16U,
    I16x8ExtendHighI8x16U,
    I32x4ExtendLowI16x8S,
    I32x4ExtendHighI16x8S,
    I32x4ExtendLowI16x8U,
    I32x4ExtendHighI16x8U,
    I64x2ExtendLowI32x4S,
    I64x2ExtendHighI32x4S,
    I64x2ExtendLowI32x4U,
    I64x2ExtendHighI32x4U,
    I16x8ExtMulLowI8x16S,
    I16x8ExtMulHighI8x16S,
    I16x8ExtMulLowI8x16U,
    I16x8ExtMulHighI8x16U,
    I32x4DotI16x8S,
    I32x4ExtMulLowI16x8S,
    I32x4ExtMulHighI16x8S,
    I32x4ExtMulLowI16x8U,
    I32x4ExtMulHighI16x8U,
    I64x2ExtMulLowI32x4S,
    I64x2ExtMulHighI32x4S,
    I64x2ExtMulLowI32x4U,
    I64x2ExtMulHighI32x4U,
    I8x16Shl,
    I8x16ShrS,
    I8x16ShrU,
    I16x8Shl,
    I16x8ShrS,
    I16x8ShrU,
    I8x16Add,
    I8x16AddSatS,
    I8x16AddSatU,
    I8x16Sub,
    I8x16SubSatS,
    I8x16SubSatU,
    I8x16MinS,
    I8x16MinU,
    I8x16MaxS,
    I8x16MaxU,
    I8x16AvgrU,
    I16x8Add,
    I16x8AddSatS,
    I16x8AddSatU,
    I16x8Sub,
    I16x8SubSatS,
    I16x8SubSatU,
    I16x8Mul,
    I16x8MinS,
    I16x8MinU,
    I16x8MaxS,
    I16x8MaxU,
    I16x8AvgrU,
    I16x8Q15mulrSatS,
    I16x8RelaxedQ15mulrS,
    I16x8RelaxedDotI8x16I7x16S,
    I32x4RelaxedDotI8x16I7x16AddS,
    I32x4Abs,
    I32x4Neg,
    I32x4AllTrue,
    I32x4Bitmask,
    I64x2Abs,
    I64x2Neg,
    I64x2AllTrue,
    I64x2Bitmask,
    I32x4Shl,
    I64x2Shl,
    I32x4Add,
    I64x2Add,
    I32x4Sub,
    I64x2Sub,
    I32x4ShrS,
    I32x4ShrU,
    I64x2ShrS,
    I64x2ShrU,
    I64x2Mul,
    I32x4Mul,
    I32x4MinS,
    I32x4MinU,
    I32x4MaxS,
    I32x4MaxU,
    F32x4Eq,
    F64x2Eq,
    F32x4Ceil,
    F32x4Floor,
    F32x4Trunc,
    F32x4Nearest,
    F64x2Ceil,
    F64x2Floor,
    F64x2Trunc,
    F64x2Nearest,
    F32x4Abs,
    F32x4Neg,
    F32x4Sqrt,
    F32x4Add,
    F32x4Sub,
    F32x4Mul,
    F32x4Div,
    F32x4Min,
    F32x4Max,
    F32x4PMin,
    F32x4PMax,
    F32x4RelaxedMadd,
    F32x4RelaxedNmadd,
    I32x4TruncSatF32x4S,
    I32x4TruncSatF32x4U,
    I32x4TruncSatF64x2SZero,
    I32x4TruncSatF64x2UZero,
    F32x4ConvertI32x4S,
    F32x4ConvertI32x4U,
    F32x4DemoteF64x2Zero,
    F64x2ConvertLowI32x4S,
    F64x2ConvertLowI32x4U,
    F64x2PromoteLowF32x4,
    F64x2Abs,
    F64x2Neg,
    F64x2Sqrt,
    F64x2Add,
    F64x2Sub,
    F64x2Mul,
    F64x2Div,
    F64x2Min,
    F64x2Max,
    F64x2PMin,
    F64x2PMax,
    F64x2RelaxedMadd,
    F64x2RelaxedNmadd,
    V128Not,
    V128And,
    V128AndNot,
    V128Or,
    V128Xor,
    V128BitSelect,
    V128AnyTrue,
    V128Load(MemArg),
    V128Store(MemArg),
    V128Load8Splat(MemArg),
    V128Load16Splat(MemArg),
    V128Load32Splat(MemArg),
    V128Load64Splat(MemArg),
    V128Load8x8S(MemArg),
    V128Load8x8U(MemArg),
    V128Load16x4S(MemArg),
    V128Load16x4U(MemArg),
    V128Load32x2S(MemArg),
    V128Load32x2U(MemArg),
    V128Load8Lane(MemArg, u8),
    V128Load16Lane(MemArg, u8),
    V128Load32Lane(MemArg, u8),
    V128Load64Lane(MemArg, u8),
    V128Load32Zero(MemArg),
    V128Load64Zero(MemArg),
    V128Store8Lane(MemArg, u8),
    V128Store16Lane(MemArg, u8),
    V128Store32Lane(MemArg, u8),
    V128Store64Lane(MemArg, u8),
    RefNull(ValType),
    RefIsNull,
    RefAsNonNull,
    RefTest {
        target: ValType,
        nullable: bool,
    },
    RefCast {
        target: ValType,
        nullable: bool,
    },
    BrOnCast {
        depth: u32,
        source: ValType,
        target: ValType,
        nullable: bool,
    },
    BrOnCastFail {
        depth: u32,
        source: ValType,
        target: ValType,
        nullable: bool,
    },
    StructNew(u32),
    StructNewDefault(u32),
    StructGet(u32, u32),
    StructGetS(u32, u32),
    StructGetU(u32, u32),
    StructSet(u32, u32),
    AnyConvertExtern,
    ExternConvertAny,
    RefFunc(u32),
    RefEq,
    TableGet(u32),
    TableSet(u32),

    Num(u8),

    TruncSat(u32),

    MemoryInit(u32, u32),

    DataDrop(u32),

    MemoryCopy(u32, u32),

    MemoryFill(u32),

    TableInit(u32, u32),

    ElemDrop(u32),

    TableCopy(u32, u32),

    TableGrow(u32),

    TableSize(u32),

    TableFill(u32),
}

#[derive(Clone, Default)]
pub struct Module {
    pub custom_sections: Vec<CustomSection>,
    pub types: Vec<FuncType>,
    pub type_is_func: Vec<bool>,
    pub type_supertypes: Vec<Vec<u32>>,
    pub type_is_final: Vec<bool>,
    pub type_rec_groups: Vec<u32>,
    pub array_types: Vec<Option<ArrayType>>,
    pub struct_types: Vec<Option<StructType>>,
    pub imports: Vec<Import>,

    pub func_types: Vec<u32>,
    pub tables: Vec<TableType>,
    pub table_inits: Vec<Option<Vec<Instr>>>,
    pub memories: Vec<Limits>,
    pub globals: Vec<Global>,
    pub tags: Vec<Tag>,
    pub exports: Vec<Export>,
    pub start: Option<u32>,
    pub elements: Vec<ElementSegment>,

    pub data_count: Option<u32>,
    pub code: Vec<Code>,
    pub data: Vec<DataSegment>,

    pub imported_func_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomSection {
    pub name: String,
    pub payload: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LimitContext {
    Table,
    Memory,
}

const MAX_TABLE_INITIAL_ELEMENTS: u64 = 10_000_000;

fn read_limits(
    r: &mut Reader,
    context: LimitContext,
    allow_memory64: bool,
) -> Result<Limits, String> {
    let flag = r.byte()?;
    let has_max = flag & 0x01 != 0;
    let shared = flag & 0x02 != 0;
    let memory64 = flag & 0x04 != 0;
    if memory64 && !allow_memory64 {
        return Err(format!("bad limits flag 0x{:02x}", flag));
    }
    if flag & !0x07 != 0 {
        return Err(format!("bad limits flag 0x{:02x}", flag));
    };
    if shared && !has_max {
        return Err("shared memory must have a maximum defined".to_string());
    }
    let min = if memory64 { r.u64()? } else { r.u32()? as u64 };
    let max = if has_max {
        Some(if memory64 { r.u64()? } else { r.u32()? as u64 })
    } else {
        None
    };
    if let Some(max) = max {
        if max < min {
            return Err(format!("limits max {} is less than min {}", max, min));
        }
    }
    if context == LimitContext::Memory {
        const MAX_32BIT_MEMORY_PAGES: u64 = 65_536;
        const MAX_64BIT_MEMORY_PAGES: u64 = 262_144;
        let max_pages = if memory64 {
            MAX_64BIT_MEMORY_PAGES
        } else {
            MAX_32BIT_MEMORY_PAGES
        };
        if min > max_pages {
            return Err(format!(
                "memory min {} exceeds implementation limit {}",
                min, max_pages
            ));
        }
        if let Some(max) = max {
            if max > max_pages {
                return Err(format!(
                    "memory max {} exceeds implementation limit {}",
                    max, max_pages
                ));
            }
        }
    }
    if context == LimitContext::Table {
        if min > MAX_TABLE_INITIAL_ELEMENTS {
            return Err(format!(
                "table min {} exceeds implementation limit {}",
                min, MAX_TABLE_INITIAL_ELEMENTS
            ));
        }
    }
    Ok(Limits {
        min,
        max,
        memory64,
        shared,
    })
}

fn read_blocktype(r: &mut Reader) -> Result<BlockType, String> {
    let b = r.byte()?;
    match b {
        0x40 => Ok(BlockType::Empty),
        0x7f => Ok(BlockType::Value(ValType::I32)),
        0x7e => Ok(BlockType::Value(ValType::I64)),
        0x7d => Ok(BlockType::Value(ValType::F32)),
        0x7c => Ok(BlockType::Value(ValType::F64)),
        0x7b => Ok(BlockType::Value(ValType::V128)),
        0x70 => Ok(BlockType::Value(ValType::FuncRef)),
        0x6f => Ok(BlockType::Value(ValType::ExternRef)),
        0x6e => Ok(BlockType::Value(ValType::AnyRef)),
        0x6d => Ok(BlockType::Value(ValType::EqRef)),
        0x6c => Ok(BlockType::Value(ValType::I31Ref)),
        0x69 => Ok(BlockType::Value(ValType::Unknown)),
        0x6b => Ok(BlockType::Value(ValType::StructRef)),
        0x6a => Ok(BlockType::Value(ValType::ArrayRef)),
        0x64 | 0x63 => Ok(BlockType::Value(read_val_type_from_first(b, r)?)),
        _ => read_blocktype_index_from_first(r, b).map(BlockType::TypeIndex),
    }
}

fn read_catch_kind(r: &mut Reader) -> Result<CatchKind, String> {
    match r.byte()? {
        0x00 => Ok(CatchKind::Catch {
            tag: r.u32()?,
            label: r.u32()?,
        }),
        0x01 => Ok(CatchKind::CatchRef {
            tag: r.u32()?,
            label: r.u32()?,
        }),
        0x02 => Ok(CatchKind::CatchAll { label: r.u32()? }),
        0x03 => Ok(CatchKind::CatchAllRef { label: r.u32()? }),
        other => Err(format!("bad try_table catch kind 0x{:02x}", other)),
    }
}

fn read_blocktype_index_from_first(r: &mut Reader, first: u8) -> Result<u32, String> {
    let mut result = (first & 0x7f) as u64;
    let mut shift = 7u32;
    let mut byte = first;
    while (byte & 0x80) != 0 {
        byte = r.byte()?;
        result |= ((byte & 0x7f) as u64) << shift;
        shift += 7;
        if shift > 35 {
            return Err("block type index LEB128 too long".to_string());
        }
    }
    if (byte & 0x40) != 0 {
        return Err(format!("unsupported negative block type 0x{:02x}", first));
    }
    if result > u32::MAX as u64 {
        return Err("block type index out of u32 range".to_string());
    }
    Ok(result as u32)
}

fn read_memarg(r: &mut Reader) -> Result<MemArg, String> {
    let flags = r.u32()?;
    let memory = if flags & 0x40 != 0 { r.u32()? } else { 0 };
    let align = flags & !0x40;
    let offset = r.u64()?;
    Ok(MemArg {
        align,
        memory,
        offset,
    })
}

fn decode_expr(r: &mut Reader) -> Result<Vec<Instr>, String> {
    let mut out = Vec::new();
    let mut depth: i64 = 0;
    let max_depth = max_expr_nesting();
    loop {
        let op = r.byte()?;
        match op {
            0x00 => out.push(Instr::Unreachable),
            0x01 => out.push(Instr::Nop),
            0x02 => {
                out.push(Instr::Block(read_blocktype(r)?));
                depth += 1;
                if depth > max_depth {
                    return Err("expression nesting depth exceeded".to_string());
                }
            }
            0x03 => {
                out.push(Instr::Loop(read_blocktype(r)?));
                depth += 1;
                if depth > max_depth {
                    return Err("expression nesting depth exceeded".to_string());
                }
            }
            0x04 => {
                out.push(Instr::If(read_blocktype(r)?));
                depth += 1;
                if depth > max_depth {
                    return Err("expression nesting depth exceeded".to_string());
                }
            }
            0x05 => out.push(Instr::Else),
            0x06 => {
                out.push(Instr::LegacyTry(read_blocktype(r)?));
                depth += 1;
                if depth > max_depth {
                    return Err("expression nesting depth exceeded".to_string());
                }
            }
            0x07 => out.push(Instr::LegacyCatch(r.u32()?)),
            0x08 => out.push(Instr::Throw(r.u32()?)),
            0x09 => out.push(Instr::LegacyRethrow(r.u32()?)),
            0x0b => {
                out.push(Instr::End);
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            0x0c => out.push(Instr::Br(r.u32()?)),
            0x0d => out.push(Instr::BrIf(r.u32()?)),
            0x0e => {
                let n = r.u32()? as usize;
                let mut targets = Vec::with_capacity(n);
                for _ in 0..n {
                    targets.push(r.u32()?);
                }
                let default = r.u32()?;
                out.push(Instr::BrTable(targets, default));
            }
            0x0f => out.push(Instr::Return),
            0x0a => out.push(Instr::ThrowRef),
            0x10 => out.push(Instr::Call(r.u32()?)),
            0x12 => out.push(Instr::ReturnCall(r.u32()?)),
            0x11 => {
                let type_idx = r.u32()?;
                let table_idx = r.u32()?;
                out.push(Instr::CallIndirect(type_idx, table_idx));
            }
            0x13 => {
                let type_idx = r.u32()?;
                let table_idx = r.u32()?;
                out.push(Instr::ReturnCallIndirect(type_idx, table_idx));
            }
            0x14 => out.push(Instr::CallRef(r.u32()?)),
            0x15 => out.push(Instr::ReturnCallRef(r.u32()?)),
            0x1f => {
                let block_type = read_blocktype(r)?;
                let n = r.u32()? as usize;
                let mut handlers = Vec::with_capacity(n);
                for _ in 0..n {
                    handlers.push(read_catch_kind(r)?);
                }
                out.push(Instr::TryTable(block_type, handlers));
                depth += 1;
                if depth > max_depth {
                    return Err("expression nesting depth exceeded".to_string());
                }
            }
            0x18 => {
                out.push(Instr::LegacyDelegate(r.u32()?));
                if depth == 0 {
                    return Err("delegate without enclosing try".to_string());
                }
                depth -= 1;
            }
            0x19 => out.push(Instr::LegacyCatchAll),
            0x1a => out.push(Instr::Drop),
            0x1b => out.push(Instr::Select),
            0x1c => {
                let n = r.u32()?;
                if n != 1 {
                    return Err(format!("typed select expects one result type, got {}", n));
                }
                out.push(Instr::SelectTyped(read_val_type(r)?));
            }
            0x20 => out.push(Instr::LocalGet(r.u32()?)),
            0x21 => out.push(Instr::LocalSet(r.u32()?)),
            0x22 => out.push(Instr::LocalTee(r.u32()?)),
            0x23 => out.push(Instr::GlobalGet(r.u32()?)),
            0x24 => out.push(Instr::GlobalSet(r.u32()?)),
            0x25 => out.push(Instr::TableGet(r.u32()?)),
            0x26 => out.push(Instr::TableSet(r.u32()?)),

            0x28..=0x35 => out.push(Instr::Load(op, read_memarg(r)?)),

            0x36..=0x3e => out.push(Instr::Store(op, read_memarg(r)?)),
            0x3f => {
                out.push(Instr::MemorySize(r.u32()?));
            }
            0x40 => {
                out.push(Instr::MemoryGrow(r.u32()?));
            }

            0x41 => out.push(Instr::I32Const(r.i32()?)),
            0x42 => out.push(Instr::I64Const(r.i64()?)),
            0x43 => out.push(Instr::F32Const(r.f32()?)),
            0x44 => out.push(Instr::F64Const(r.f64()?)),
            0xd0 => out.push(Instr::RefNull(read_heap_type(r)?)),
            0xd1 => out.push(Instr::RefIsNull),
            0xd2 => out.push(Instr::RefFunc(r.u32()?)),
            0xd3 => out.push(Instr::RefEq),
            0xd4 => out.push(Instr::RefAsNonNull),
            0xd5 => out.push(Instr::BrOnNull(r.u32()?)),
            0xd6 => out.push(Instr::BrOnNonNull(r.u32()?)),

            0x45..=0xc4 => out.push(Instr::Num(op)),

            0xfb => {
                let sub = r.u32()?;
                match sub {
                    0x00 => out.push(Instr::StructNew(r.u32()?)),
                    0x01 => out.push(Instr::StructNewDefault(r.u32()?)),
                    0x02 => out.push(Instr::StructGet(r.u32()?, r.u32()?)),
                    0x03 => out.push(Instr::StructGetS(r.u32()?, r.u32()?)),
                    0x04 => out.push(Instr::StructGetU(r.u32()?, r.u32()?)),
                    0x05 => out.push(Instr::StructSet(r.u32()?, r.u32()?)),
                    0x06 => out.push(Instr::ArrayNew(r.u32()?)),
                    0x07 => out.push(Instr::ArrayNewDefault(r.u32()?)),
                    0x08 => out.push(Instr::ArrayNewFixed(r.u32()?, r.u32()?)),
                    0x09 => out.push(Instr::ArrayNewData(r.u32()?, r.u32()?)),
                    0x0a => out.push(Instr::ArrayNewElem(r.u32()?, r.u32()?)),
                    0x0b => out.push(Instr::ArrayGet(r.u32()?)),
                    0x0c => out.push(Instr::ArrayGetS(r.u32()?)),
                    0x0d => out.push(Instr::ArrayGetU(r.u32()?)),
                    0x0e => out.push(Instr::ArraySet(r.u32()?)),
                    0x0f => out.push(Instr::ArrayLen),
                    0x10 => out.push(Instr::ArrayFill(r.u32()?)),
                    0x11 => out.push(Instr::ArrayCopy(r.u32()?, r.u32()?)),
                    0x12 => out.push(Instr::ArrayInitData(r.u32()?, r.u32()?)),
                    0x13 => out.push(Instr::ArrayInitElem(r.u32()?, r.u32()?)),
                    0x14 | 0x15 => out.push(Instr::RefTest {
                        target: read_heap_type(r)?,
                        nullable: sub == 0x15,
                    }),
                    0x16 | 0x17 => out.push(Instr::RefCast {
                        target: read_heap_type(r)?,
                        nullable: sub == 0x17,
                    }),
                    0x18 => {
                        let flags = r.u32()?;
                        let depth = r.u32()?;
                        let source =
                            heap_type_with_nullability(read_heap_type(r)?, flags & 0x01 != 0);
                        let target =
                            heap_type_with_nullability(read_heap_type(r)?, flags & 0x02 != 0);
                        out.push(Instr::BrOnCast {
                            depth,
                            source,
                            target,
                            nullable: flags & 0x02 != 0,
                        });
                    }
                    0x19 => {
                        let flags = r.u32()?;
                        let depth = r.u32()?;
                        let source =
                            heap_type_with_nullability(read_heap_type(r)?, flags & 0x01 != 0);
                        let target =
                            heap_type_with_nullability(read_heap_type(r)?, flags & 0x02 != 0);
                        out.push(Instr::BrOnCastFail {
                            depth,
                            source,
                            target,
                            nullable: flags & 0x02 != 0,
                        });
                    }
                    0x1a => out.push(Instr::AnyConvertExtern),
                    0x1b => out.push(Instr::ExternConvertAny),
                    0x1c => out.push(Instr::RefI31),
                    0x1d => out.push(Instr::I31GetS),
                    0x1e => out.push(Instr::I31GetU),
                    other => return Err(format!("unsupported 0xfb GC subopcode {}", other)),
                }
            }

            0xfc => {
                let sub = r.u32()?;
                match sub {

                    0..=7 => out.push(Instr::TruncSat(sub)),

                    8 => {
                        let dataidx = r.u32()?;
                        let memidx = r.u32()?;
                        out.push(Instr::MemoryInit(dataidx, memidx));
                    }

                    9 => out.push(Instr::DataDrop(r.u32()?)),

                    10 => {
                        let dst = r.u32()?;
                        let src = r.u32()?;
                        out.push(Instr::MemoryCopy(dst, src));
                    }

                    11 => {
                        out.push(Instr::MemoryFill(r.u32()?));
                    }

                    12 => {
                        let elemidx = r.u32()?;
                        let tableidx = r.u32()?;
                        out.push(Instr::TableInit(elemidx, tableidx));
                    }

                    13 => out.push(Instr::ElemDrop(r.u32()?)),

                    14 => {
                        let dst = r.u32()?;
                        let src = r.u32()?;
                        out.push(Instr::TableCopy(dst, src));
                    }

                    15 => out.push(Instr::TableGrow(r.u32()?)),

                    16 => out.push(Instr::TableSize(r.u32()?)),

                    17 => out.push(Instr::TableFill(r.u32()?)),
                    other => {
                        return Err(format!("unsupported 0xfc subopcode {}", other));
                    }
                }
            }

            0xfe => {
                let sub = r.u32()?;
                match sub {
                    0x00 => out.push(Instr::AtomicNotify(read_memarg(r)?)),
                    0x01 | 0x02 => out.push(Instr::AtomicWait(sub, read_memarg(r)?)),
                    0x03 => out.push(Instr::AtomicFence(r.byte()?)),

                    0x10 | 0x11 | 0x12 | 0x13 | 0x14 | 0x15 | 0x16 => {
                        out.push(Instr::AtomicLoad(sub, read_memarg(r)?))
                    }

                    0x17 | 0x18 | 0x19 | 0x1a | 0x1b | 0x1c | 0x1d => {
                        out.push(Instr::AtomicStore(sub, read_memarg(r)?))
                    }

                    0x1e | 0x1f | 0x20 | 0x21 | 0x22 | 0x23 | 0x24 | 0x25 | 0x26 | 0x27 | 0x28
                    | 0x29 | 0x2a | 0x2b | 0x2c | 0x2d | 0x2e | 0x2f | 0x30 | 0x31 | 0x32
                    | 0x33 | 0x34 | 0x35 | 0x36 | 0x37 | 0x38 | 0x39 | 0x3a | 0x3b | 0x3c
                    | 0x3d | 0x3e | 0x3f | 0x40 | 0x41 | 0x42 | 0x43 | 0x44 | 0x45 | 0x46
                    | 0x47 | 0x48 | 0x49 | 0x4a | 0x4b | 0x4c | 0x4d | 0x4e => {
                        out.push(Instr::AtomicRmw(sub, read_memarg(r)?))
                    }
                    other => {
                        return Err(format!("unsupported 0xfe subopcode {}", other));
                    }
                }
            }

            0xfd => {
                let sub = r.u32()?;
                match sub {
                    0 => out.push(Instr::V128Load(read_memarg(r)?)),
                    1 => out.push(Instr::V128Load8x8S(read_memarg(r)?)),
                    2 => out.push(Instr::V128Load8x8U(read_memarg(r)?)),
                    3 => out.push(Instr::V128Load16x4S(read_memarg(r)?)),
                    4 => out.push(Instr::V128Load16x4U(read_memarg(r)?)),
                    5 => out.push(Instr::V128Load32x2S(read_memarg(r)?)),
                    6 => out.push(Instr::V128Load32x2U(read_memarg(r)?)),
                    12 => {
                        let bytes = r.bytes_n(16)?;
                        let mut lanes = [0u8; 16];
                        lanes.copy_from_slice(bytes);
                        out.push(Instr::V128Const(lanes));
                    }
                    13 => {
                        let bytes = r.bytes_n(16)?;
                        let mut lanes = [0u8; 16];
                        lanes.copy_from_slice(bytes);
                        out.push(Instr::I8x16Shuffle(lanes));
                    }
                    14 => out.push(Instr::I8x16Swizzle),
                    7 => out.push(Instr::V128Load8Splat(read_memarg(r)?)),
                    8 => out.push(Instr::V128Load16Splat(read_memarg(r)?)),
                    9 => out.push(Instr::V128Load32Splat(read_memarg(r)?)),
                    10 => out.push(Instr::V128Load64Splat(read_memarg(r)?)),
                    11 => out.push(Instr::V128Store(read_memarg(r)?)),
                    15 => out.push(Instr::I8x16Splat),
                    16 => out.push(Instr::I16x8Splat),
                    17 => out.push(Instr::I32x4Splat),
                    18 => out.push(Instr::I64x2Splat),
                    19 => out.push(Instr::F32x4Splat),
                    20 => out.push(Instr::F64x2Splat),
                    21 => out.push(Instr::I8x16ExtractLaneS(r.byte()?)),
                    22 => out.push(Instr::I8x16ExtractLaneU(r.byte()?)),
                    23 => out.push(Instr::I8x16ReplaceLane(r.byte()?)),
                    24 => out.push(Instr::I16x8ExtractLaneS(r.byte()?)),
                    25 => out.push(Instr::I16x8ExtractLaneU(r.byte()?)),
                    26 => out.push(Instr::I16x8ReplaceLane(r.byte()?)),
                    27 => out.push(Instr::I32x4ExtractLane(r.byte()?)),
                    28 => out.push(Instr::I32x4ReplaceLane(r.byte()?)),
                    29 => out.push(Instr::I64x2ExtractLane(r.byte()?)),
                    30 => out.push(Instr::I64x2ReplaceLane(r.byte()?)),
                    31 => out.push(Instr::F32x4ExtractLane(r.byte()?)),
                    32 => out.push(Instr::F32x4ReplaceLane(r.byte()?)),
                    33 => out.push(Instr::F64x2ExtractLane(r.byte()?)),
                    34 => out.push(Instr::F64x2ReplaceLane(r.byte()?)),
                    35 => out.push(Instr::I8x16Eq),
                    36 => out.push(Instr::I8x16Ne),
                    37 => out.push(Instr::I8x16LtS),
                    38 => out.push(Instr::I8x16LtU),
                    39 => out.push(Instr::I8x16GtS),
                    40 => out.push(Instr::I8x16GtU),
                    41 => out.push(Instr::I8x16LeS),
                    42 => out.push(Instr::I8x16LeU),
                    43 => out.push(Instr::I8x16GeS),
                    44 => out.push(Instr::I8x16GeU),
                    45 => out.push(Instr::I16x8Eq),
                    46 => out.push(Instr::I16x8Ne),
                    47 => out.push(Instr::I16x8LtS),
                    48 => out.push(Instr::I16x8LtU),
                    49 => out.push(Instr::I16x8GtS),
                    50 => out.push(Instr::I16x8GtU),
                    51 => out.push(Instr::I16x8LeS),
                    52 => out.push(Instr::I16x8LeU),
                    53 => out.push(Instr::I16x8GeS),
                    54 => out.push(Instr::I16x8GeU),
                    55 => out.push(Instr::I32x4Eq),
                    56 => out.push(Instr::I32x4Ne),
                    57 => out.push(Instr::I32x4LtS),
                    58 => out.push(Instr::I32x4LtU),
                    59 => out.push(Instr::I32x4GtS),
                    60 => out.push(Instr::I32x4GtU),
                    61 => out.push(Instr::I32x4LeS),
                    62 => out.push(Instr::I32x4LeU),
                    63 => out.push(Instr::I32x4GeS),
                    64 => out.push(Instr::I32x4GeU),
                    65 => out.push(Instr::F32x4Eq),
                    66 => out.push(Instr::F32x4Ne),
                    67 => out.push(Instr::F32x4Lt),
                    68 => out.push(Instr::F32x4Gt),
                    69 => out.push(Instr::F32x4Le),
                    70 => out.push(Instr::F32x4Ge),
                    71 => out.push(Instr::F64x2Eq),
                    72 => out.push(Instr::F64x2Ne),
                    73 => out.push(Instr::F64x2Lt),
                    74 => out.push(Instr::F64x2Gt),
                    75 => out.push(Instr::F64x2Le),
                    76 => out.push(Instr::F64x2Ge),
                    77 => out.push(Instr::V128Not),
                    78 => out.push(Instr::V128And),
                    79 => out.push(Instr::V128AndNot),
                    80 => out.push(Instr::V128Or),
                    81 => out.push(Instr::V128Xor),
                    82 => out.push(Instr::V128BitSelect),
                    83 => out.push(Instr::V128AnyTrue),
                    84 => {
                        let memarg = read_memarg(r)?;
                        out.push(Instr::V128Load8Lane(memarg, r.byte()?));
                    }
                    85 => {
                        let memarg = read_memarg(r)?;
                        out.push(Instr::V128Load16Lane(memarg, r.byte()?));
                    }
                    86 => {
                        let memarg = read_memarg(r)?;
                        out.push(Instr::V128Load32Lane(memarg, r.byte()?));
                    }
                    87 => {
                        let memarg = read_memarg(r)?;
                        out.push(Instr::V128Load64Lane(memarg, r.byte()?));
                    }
                    88 => {
                        let memarg = read_memarg(r)?;
                        out.push(Instr::V128Store8Lane(memarg, r.byte()?));
                    }
                    89 => {
                        let memarg = read_memarg(r)?;
                        out.push(Instr::V128Store16Lane(memarg, r.byte()?));
                    }
                    90 => {
                        let memarg = read_memarg(r)?;
                        out.push(Instr::V128Store32Lane(memarg, r.byte()?));
                    }
                    91 => {
                        let memarg = read_memarg(r)?;
                        out.push(Instr::V128Store64Lane(memarg, r.byte()?));
                    }
                    92 => {
                        let memarg = read_memarg(r)?;
                        out.push(Instr::V128Load32Zero(memarg));
                    }
                    93 => {
                        let memarg = read_memarg(r)?;
                        out.push(Instr::V128Load64Zero(memarg));
                    }
                    94 => out.push(Instr::F32x4DemoteF64x2Zero),
                    95 => out.push(Instr::F64x2PromoteLowF32x4),
                    96 => out.push(Instr::I8x16Abs),
                    97 => out.push(Instr::I8x16Neg),
                    98 => out.push(Instr::I8x16Popcnt),
                    99 => out.push(Instr::I8x16AllTrue),
                    100 => out.push(Instr::I8x16Bitmask),
                    101 => out.push(Instr::I8x16NarrowI16x8S),
                    102 => out.push(Instr::I8x16NarrowI16x8U),
                    103 => out.push(Instr::F32x4Ceil),
                    104 => out.push(Instr::F32x4Floor),
                    105 => out.push(Instr::F32x4Trunc),
                    106 => out.push(Instr::F32x4Nearest),
                    107 => out.push(Instr::I8x16Shl),
                    108 => out.push(Instr::I8x16ShrS),
                    109 => out.push(Instr::I8x16ShrU),
                    110 => out.push(Instr::I8x16Add),
                    111 => out.push(Instr::I8x16AddSatS),
                    112 => out.push(Instr::I8x16AddSatU),
                    113 => out.push(Instr::I8x16Sub),
                    114 => out.push(Instr::I8x16SubSatS),
                    115 => out.push(Instr::I8x16SubSatU),
                    116 => out.push(Instr::F64x2Ceil),
                    117 => out.push(Instr::F64x2Floor),
                    118 => out.push(Instr::I8x16MinS),
                    119 => out.push(Instr::I8x16MinU),
                    120 => out.push(Instr::I8x16MaxS),
                    121 => out.push(Instr::I8x16MaxU),
                    122 => out.push(Instr::F64x2Trunc),
                    123 => out.push(Instr::I8x16AvgrU),
                    124 => out.push(Instr::I16x8ExtAddPairwiseI8x16S),
                    125 => out.push(Instr::I16x8ExtAddPairwiseI8x16U),
                    126 => out.push(Instr::I32x4ExtAddPairwiseI16x8S),
                    127 => out.push(Instr::I32x4ExtAddPairwiseI16x8U),
                    128 => out.push(Instr::I16x8Abs),
                    129 => out.push(Instr::I16x8Neg),
                    130 => out.push(Instr::I16x8Q15mulrSatS),
                    131 => out.push(Instr::I16x8AllTrue),
                    132 => out.push(Instr::I16x8Bitmask),
                    133 => out.push(Instr::I16x8NarrowI32x4S),
                    134 => out.push(Instr::I16x8NarrowI32x4U),
                    135 => out.push(Instr::I16x8ExtendLowI8x16S),
                    136 => out.push(Instr::I16x8ExtendHighI8x16S),
                    137 => out.push(Instr::I16x8ExtendLowI8x16U),
                    138 => out.push(Instr::I16x8ExtendHighI8x16U),
                    139 => out.push(Instr::I16x8Shl),
                    140 => out.push(Instr::I16x8ShrS),
                    141 => out.push(Instr::I16x8ShrU),
                    142 => out.push(Instr::I16x8Add),
                    143 => out.push(Instr::I16x8AddSatS),
                    144 => out.push(Instr::I16x8AddSatU),
                    145 => out.push(Instr::I16x8Sub),
                    146 => out.push(Instr::I16x8SubSatS),
                    147 => out.push(Instr::I16x8SubSatU),
                    148 => out.push(Instr::F64x2Nearest),
                    149 => out.push(Instr::I16x8Mul),
                    150 => out.push(Instr::I16x8MinS),
                    151 => out.push(Instr::I16x8MinU),
                    152 => out.push(Instr::I16x8MaxS),
                    153 => out.push(Instr::I16x8MaxU),
                    155 => out.push(Instr::I16x8AvgrU),
                    156 => out.push(Instr::I16x8ExtMulLowI8x16S),
                    157 => out.push(Instr::I16x8ExtMulHighI8x16S),
                    158 => out.push(Instr::I16x8ExtMulLowI8x16U),
                    159 => out.push(Instr::I16x8ExtMulHighI8x16U),
                    160 => out.push(Instr::I32x4Abs),
                    161 => out.push(Instr::I32x4Neg),
                    163 => out.push(Instr::I32x4AllTrue),
                    164 => out.push(Instr::I32x4Bitmask),
                    167 => out.push(Instr::I32x4ExtendLowI16x8S),
                    168 => out.push(Instr::I32x4ExtendHighI16x8S),
                    169 => out.push(Instr::I32x4ExtendLowI16x8U),
                    170 => out.push(Instr::I32x4ExtendHighI16x8U),
                    171 => out.push(Instr::I32x4Shl),
                    172 => out.push(Instr::I32x4ShrS),
                    173 => out.push(Instr::I32x4ShrU),
                    174 => out.push(Instr::I32x4Add),
                    177 => out.push(Instr::I32x4Sub),
                    181 => out.push(Instr::I32x4Mul),
                    182 => out.push(Instr::I32x4MinS),
                    183 => out.push(Instr::I32x4MinU),
                    184 => out.push(Instr::I32x4MaxS),
                    185 => out.push(Instr::I32x4MaxU),
                    186 => out.push(Instr::I32x4DotI16x8S),
                    188 => out.push(Instr::I32x4ExtMulLowI16x8S),
                    189 => out.push(Instr::I32x4ExtMulHighI16x8S),
                    190 => out.push(Instr::I32x4ExtMulLowI16x8U),
                    191 => out.push(Instr::I32x4ExtMulHighI16x8U),
                    192 => out.push(Instr::I64x2Abs),
                    193 => out.push(Instr::I64x2Neg),
                    195 => out.push(Instr::I64x2AllTrue),
                    196 => out.push(Instr::I64x2Bitmask),
                    199 => out.push(Instr::I64x2ExtendLowI32x4S),
                    200 => out.push(Instr::I64x2ExtendHighI32x4S),
                    201 => out.push(Instr::I64x2ExtendLowI32x4U),
                    202 => out.push(Instr::I64x2ExtendHighI32x4U),
                    203 => out.push(Instr::I64x2Shl),
                    204 => out.push(Instr::I64x2ShrS),
                    205 => out.push(Instr::I64x2ShrU),
                    206 => out.push(Instr::I64x2Add),
                    209 => out.push(Instr::I64x2Sub),
                    213 => out.push(Instr::I64x2Mul),
                    214 => out.push(Instr::I64x2Eq),
                    215 => out.push(Instr::I64x2Ne),
                    216 => out.push(Instr::I64x2LtS),
                    217 => out.push(Instr::I64x2GtS),
                    218 => out.push(Instr::I64x2LeS),
                    219 => out.push(Instr::I64x2GeS),
                    220 => out.push(Instr::I64x2ExtMulLowI32x4S),
                    221 => out.push(Instr::I64x2ExtMulHighI32x4S),
                    222 => out.push(Instr::I64x2ExtMulLowI32x4U),
                    223 => out.push(Instr::I64x2ExtMulHighI32x4U),
                    224 => out.push(Instr::F32x4Abs),
                    225 => out.push(Instr::F32x4Neg),
                    227 => out.push(Instr::F32x4Sqrt),
                    228 => out.push(Instr::F32x4Add),
                    229 => out.push(Instr::F32x4Sub),
                    230 => out.push(Instr::F32x4Mul),
                    231 => out.push(Instr::F32x4Div),
                    232 => out.push(Instr::F32x4Min),
                    233 => out.push(Instr::F32x4Max),
                    234 => out.push(Instr::F32x4PMin),
                    235 => out.push(Instr::F32x4PMax),
                    236 => out.push(Instr::F64x2Abs),
                    237 => out.push(Instr::F64x2Neg),
                    239 => out.push(Instr::F64x2Sqrt),
                    240 => out.push(Instr::F64x2Add),
                    241 => out.push(Instr::F64x2Sub),
                    242 => out.push(Instr::F64x2Mul),
                    243 => out.push(Instr::F64x2Div),
                    244 => out.push(Instr::F64x2Min),
                    245 => out.push(Instr::F64x2Max),
                    246 => out.push(Instr::F64x2PMin),
                    247 => out.push(Instr::F64x2PMax),
                    248 => out.push(Instr::I32x4TruncSatF32x4S),
                    249 => out.push(Instr::I32x4TruncSatF32x4U),
                    250 => out.push(Instr::F32x4ConvertI32x4S),
                    251 => out.push(Instr::F32x4ConvertI32x4U),
                    252 => out.push(Instr::I32x4TruncSatF64x2SZero),
                    253 => out.push(Instr::I32x4TruncSatF64x2UZero),
                    254 => out.push(Instr::F64x2ConvertLowI32x4S),
                    255 => out.push(Instr::F64x2ConvertLowI32x4U),
                    256 => out.push(Instr::I8x16Swizzle),
                    257 => out.push(Instr::I32x4TruncSatF32x4S),
                    258 => out.push(Instr::I32x4TruncSatF32x4U),
                    259 => out.push(Instr::I32x4TruncSatF64x2SZero),
                    260 => out.push(Instr::I32x4TruncSatF64x2UZero),
                    261 => out.push(Instr::F32x4RelaxedMadd),
                    262 => out.push(Instr::F32x4RelaxedNmadd),
                    263 => out.push(Instr::F64x2RelaxedMadd),
                    264 => out.push(Instr::F64x2RelaxedNmadd),
                    265 | 266 | 267 | 268 => out.push(Instr::V128BitSelect),
                    269 => out.push(Instr::F32x4Min),
                    270 => out.push(Instr::F32x4Max),
                    271 => out.push(Instr::F64x2Min),
                    272 => out.push(Instr::F64x2Max),
                    273 => out.push(Instr::I16x8RelaxedQ15mulrS),
                    274 => out.push(Instr::I16x8RelaxedDotI8x16I7x16S),
                    275 => out.push(Instr::I32x4RelaxedDotI8x16I7x16AddS),
                    other => {
                        return Err(format!("unsupported 0xfd SIMD subopcode {}", other));
                    }
                }
            }

            other => {
                return Err(format!("unsupported opcode 0x{:02x}", other));
            }
        }
    }
    Ok(out)
}

fn read_ref_type(r: &mut Reader) -> Result<ValType, String> {
    read_ref_type_from_first(r.byte()?, r)
}

fn read_ref_type_from_first(first: u8, r: &mut Reader) -> Result<ValType, String> {
    let ty = read_val_type_from_first(first, r)?;
    match ty {
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
        | ValType::Unknown => Ok(ty),
        other => Err(format!(
            "non-reference type {:?} where reference type expected",
            other
        )),
    }
}

pub fn parse_module(bytes: &[u8]) -> Result<Module, String> {
    let mut r = Reader::new(bytes);
    let magic = r.bytes_n(4)?;
    if magic != b"\0asm" {
        return Err("bad magic (not a wasm module)".to_string());
    }
    let version = r.bytes_n(4)?;
    if version != [1, 0, 0, 0] {
        return Err(format!("unsupported wasm version {:?}", version));
    }

    let mut m = Module::default();
    let mut last_section_order: i32 = -1;

    while !r.eof() {
        let id = r.byte()?;
        let size = r.u32()? as usize;
        let section_bytes = r.bytes_n(size)?;
        let mut sr = Reader::new(section_bytes);

        if id != 0 {
            let order = section_order(id)?;
            if order <= last_section_order {
                return Err(format!(
                    "section {} out of order (after {})",
                    id, last_section_order
                ));
            }
            last_section_order = order;
        }

        match id {
            0 => parse_custom_section(&mut sr, &mut m)?,
            1 => parse_type_section(&mut sr, &mut m)?,
            2 => parse_import_section(&mut sr, &mut m)?,
            3 => parse_function_section(&mut sr, &mut m)?,
            4 => parse_table_section(&mut sr, &mut m)?,
            5 => parse_memory_section(&mut sr, &mut m)?,
            6 => parse_global_section(&mut sr, &mut m)?,
            13 => parse_tag_section(&mut sr, &mut m)?,
            7 => parse_export_section(&mut sr, &mut m)?,
            8 => {
                m.start = Some(sr.u32()?);
            }
            9 => parse_element_section(&mut sr, &mut m)?,
            12 => {
                m.data_count = Some(sr.u32()?);
            }
            10 => parse_code_section(&mut sr, &mut m)?,
            11 => parse_data_section(&mut sr, &mut m)?,
            other => return Err(format!("unknown section id {}", other)),
        }
    }

    if m.code.len() != m.func_types.len() {
        return Err(format!(
            "function/code count mismatch: {} funcs vs {} code entries",
            m.func_types.len(),
            m.code.len()
        ));
    }
    if let Some(data_count) = m.data_count {
        if data_count as usize != m.data.len() {
            return Err(format!(
                "data count mismatch: declared {} vs {} data entries",
                data_count,
                m.data.len()
            ));
        }
    }
    validate_type_index_space(&m)?;
    validate_import_export_start(&m)?;
    validate_initializer_expressions(&m)?;
    validate_limits_and_index_bounds(&m)?;
    validate_operand_stack_control_flow(&m)?;
    validate_bulk_memory_datacount(&m)?;

    Ok(m)
}

fn parse_custom_section(r: &mut Reader, m: &mut Module) -> Result<(), String> {
    let name = r.name()?;
    let payload = r.bytes_n(r.remaining())?.to_vec();
    m.custom_sections.push(CustomSection { name, payload });
    Ok(())
}

fn validate_type_index_space(m: &Module) -> Result<(), String> {
    let type_count = m.types.len();
    let mut imported_funcs = 0usize;
    let mut imported_globals = 0usize;
    validate_type_ref_index_space(m, type_count)?;
    validate_type_declaration_ref_visibility(m)?;
    validate_declared_supertypes(m)?;
    for imp in &m.imports {
        match imp.kind {
            ImportKind::Func(type_idx) => {
                if type_idx as usize >= type_count {
                    return Err(format!(
                        "function import type index {} out of bounds ({} types)",
                        type_idx, type_count
                    ));
                }
                if !m
                    .type_is_func
                    .get(type_idx as usize)
                    .copied()
                    .unwrap_or(false)
                {
                    return Err(format!(
                        "function import type index {} is not a function type",
                        type_idx
                    ));
                }
                imported_funcs += 1;
            }
            ImportKind::Global { .. } => imported_globals += 1,
            ImportKind::Tag(type_idx) => {
                if type_idx as usize >= type_count {
                    return Err(format!(
                        "tag import type index {} out of bounds ({} types)",
                        type_idx, type_count
                    ));
                }
                if !m
                    .type_is_func
                    .get(type_idx as usize)
                    .copied()
                    .unwrap_or(false)
                {
                    return Err(format!(
                        "tag import type index {} is not a function type",
                        type_idx
                    ));
                }
                if !m.types[type_idx as usize].results.is_empty() {
                    return Err(format!(
                        "tag import type index {} has non-empty result type",
                        type_idx
                    ));
                }
            }
            _ => {}
        }
    }

    for (i, type_idx) in m.func_types.iter().enumerate() {
        if *type_idx as usize >= type_count {
            return Err(format!(
                "function {} type index {} out of bounds ({} types)",
                i, type_idx, type_count
            ));
        }
        if !m
            .type_is_func
            .get(*type_idx as usize)
            .copied()
            .unwrap_or(false)
        {
            return Err(format!(
                "function {} type index {} is not a function type",
                i, type_idx
            ));
        }
    }

    for (i, tag) in m.tags.iter().enumerate() {
        if tag.attribute != 0 {
            return Err(format!(
                "tag {} has unsupported attribute {}",
                i, tag.attribute
            ));
        }
        if tag.type_idx as usize >= type_count {
            return Err(format!(
                "tag {} type index {} out of bounds ({} types)",
                i, tag.type_idx, type_count
            ));
        }
        if !m
            .type_is_func
            .get(tag.type_idx as usize)
            .copied()
            .unwrap_or(false)
        {
            return Err(format!(
                "tag {} type index {} is not a function type",
                i, tag.type_idx
            ));
        }
        if !m.types[tag.type_idx as usize].results.is_empty() {
            return Err(format!(
                "tag {} type index {} has non-empty result type",
                i, tag.type_idx
            ));
        }
    }

    let func_count = imported_funcs + m.func_types.len();
    let global_count = imported_globals + m.globals.len();
    let table_count = m.tables.len();
    let table_tys = m.tables.clone();
    let element_count = m.elements.len();
    let declared_funcs = declared_function_indices(m, func_count)?;

    for (defined_idx, code) in m.code.iter().enumerate() {
        let type_idx = m.func_types[defined_idx] as usize;
        let params = m.types[type_idx].params.len();
        let local_count = params + code.locals.len();
        validate_instr_indices(
            &code.body,
            local_count,
            func_count,
            global_count,
            &table_tys,
            &m.memories,
            element_count,
            m.data.len(),
            type_count,
            &m.array_types,
            &m.struct_types,
            &m.tags,
            &declared_funcs,
        )?;
    }

    for (global_idx, global) in m.globals.iter().enumerate() {
        validate_ref_func_declared(&global.init, func_count, &declared_funcs)
            .map_err(|err| format!("global {} initializer: {}", global_idx, err))?;
    }

    for (seg_idx, seg) in m.elements.iter().enumerate() {
        if seg.mode == ElementMode::Active && seg.table as usize >= table_count {
            return Err(format!(
                "element segment {} table index {} out of bounds ({} tables)",
                seg_idx, seg.table, table_count
            ));
        }
        if seg.mode == ElementMode::Active {
            let table_ty = m.tables[seg.table as usize].elem;
            if !active_element_segment_matches_table(
                seg,
                table_ty,
                &m.type_supertypes,
                &m.array_types,
                &m.struct_types,
                &m.types,
            ) {
                return Err(format!(
                    "element segment {} type {:?} does not match table {} type {:?}",
                    seg_idx, seg.ty, seg.table, table_ty
                ));
            }
        }
        for item in &seg.items {
            match item {
                ElementItem::Func(func_idx) => {
                    validate_element_func_index(seg_idx, *func_idx, func_count)?
                }
                ElementItem::Expr(expr) => {
                    for ins in expr {
                        if let Instr::RefFunc(func_idx) = ins {
                            validate_element_func_index(seg_idx, *func_idx, func_count)?;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn validate_declared_supertypes(m: &Module) -> Result<(), String> {
    let type_count = m.types.len();
    for (type_idx, supertypes) in m.type_supertypes.iter().enumerate() {
        for super_idx in supertypes {
            if *super_idx as usize >= type_count {
                return Err(format!(
                    "type {} explicit supertype {} out of bounds ({} types)",
                    type_idx, super_idx, type_count
                ));
            }
            if m.type_is_final
                .get(*super_idx as usize)
                .copied()
                .unwrap_or(false)
            {
                return Err(format!(
                    "type {} has final explicit supertype {}",
                    type_idx, super_idx
                ));
            }
            if !type_decl_is_valid_subtype(type_idx as u32, *super_idx, m) {
                return Err(format!(
                    "type {} has invalid explicit supertype {}",
                    type_idx, super_idx
                ));
            }
        }
    }
    Ok(())
}

fn validate_type_ref_index_space(m: &Module, type_count: usize) -> Result<(), String> {
    for (type_idx, ft) in m.types.iter().enumerate() {
        for ty in ft.params.iter().chain(ft.results.iter()) {
            validate_val_type_ref(*ty, type_count)
                .map_err(|err| format!("type {}: {}", type_idx, err))?;
        }
    }
    for (type_idx, array) in m.array_types.iter().enumerate() {
        if let Some(array) = array {
            validate_val_type_ref(array.element, type_count)
                .map_err(|err| format!("array type {}: {}", type_idx, err))?;
        }
    }
    for (type_idx, strukt) in m.struct_types.iter().enumerate() {
        if let Some(strukt) = strukt {
            for (field_idx, field) in strukt.fields.iter().enumerate() {
                validate_val_type_ref(field.ty, type_count).map_err(|err| {
                    format!("struct type {} field {}: {}", type_idx, field_idx, err)
                })?;
            }
        }
    }
    for imp in &m.imports {
        match imp.kind {
            ImportKind::Global { ty, .. } => validate_val_type_ref(ty, type_count)
                .map_err(|err| format!("global import {}.{}: {}", imp.module, imp.name, err))?,
            ImportKind::Table(table) => validate_val_type_ref(table.elem, type_count)
                .map_err(|err| format!("table import {}.{}: {}", imp.module, imp.name, err))?,
            _ => {}
        }
    }
    for (table_idx, table) in m.tables.iter().enumerate() {
        validate_val_type_ref(table.elem, type_count)
            .map_err(|err| format!("table {}: {}", table_idx, err))?;
    }
    for (global_idx, global) in m.globals.iter().enumerate() {
        validate_val_type_ref(global.ty, type_count)
            .map_err(|err| format!("global {}: {}", global_idx, err))?;
    }
    for (seg_idx, seg) in m.elements.iter().enumerate() {
        validate_val_type_ref(seg.ty, type_count)
            .map_err(|err| format!("element segment {}: {}", seg_idx, err))?;
    }
    for (func_idx, code) in m.code.iter().enumerate() {
        for (local_idx, ty) in code.locals.iter().enumerate() {
            validate_val_type_ref(*ty, type_count)
                .map_err(|err| format!("function {} local {}: {}", func_idx, local_idx, err))?;
        }
    }
    Ok(())
}

fn validate_type_declaration_ref_visibility(m: &Module) -> Result<(), String> {
    for type_idx in 0..m.types.len() {
        let current_group = m
            .type_rec_groups
            .get(type_idx)
            .copied()
            .unwrap_or(type_idx as u32);
        let mut refs = Vec::new();
        refs.extend(
            m.types[type_idx]
                .params
                .iter()
                .chain(m.types[type_idx].results.iter())
                .copied(),
        );
        if let Some(Some(array)) = m.array_types.get(type_idx) {
            refs.push(array.element);
        }
        if let Some(Some(strukt)) = m.struct_types.get(type_idx) {
            refs.extend(strukt.fields.iter().map(|field| field.ty));
        }
        for ty in refs {
            let Some(referenced) = val_type_ref_index(ty) else {
                continue;
            };
            if referenced as usize <= type_idx {
                continue;
            }
            if m.type_rec_groups
                .get(referenced as usize)
                .copied()
                .is_some_and(|referenced_group| referenced_group == current_group)
            {
                continue;
            }
            return Err(format!(
                "type {} references future type {} outside its recursion group",
                type_idx, referenced
            ));
        }
    }
    Ok(())
}

fn validate_val_type_ref(ty: ValType, type_count: usize) -> Result<(), String> {
    let Some(idx) = val_type_ref_index(ty) else {
        return Ok(());
    };
    if idx as usize >= type_count {
        return Err(format!(
            "type index {} out of bounds ({} types)",
            idx, type_count
        ));
    }
    Ok(())
}

fn val_type_ref_index(ty: ValType) -> Option<u32> {
    match ty {
        ValType::TypeRef(idx) | ValType::NonNullTypeRef(idx) => Some(idx),
        _ => None,
    }
}

fn active_element_segment_matches_table(
    seg: &ElementSegment,
    table_ty: ValType,
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
) -> bool {
    if types_compatible_in_module(
        seg.ty,
        table_ty,
        type_supertypes,
        array_types,
        struct_types,
        types,
    ) {
        return true;
    }
    !seg.items.is_empty()
        && seg
            .items
            .iter()
            .all(|item| matches!(item, ElementItem::Func(_)))
        && types_compatible_in_module(
            ValType::NonNullFuncRef,
            table_ty,
            type_supertypes,
            array_types,
            struct_types,
            types,
        )
}

fn validate_block_type_ref(block_type: BlockType, type_count: usize) -> Result<(), String> {
    match block_type {
        BlockType::Value(ty) => validate_val_type_ref(ty, type_count),
        BlockType::Empty | BlockType::TypeIndex(_) => Ok(()),
    }
}

fn val_type_is_defaultable(ty: ValType) -> bool {
    !matches!(
        ty,
        ValType::NonNullFuncRef
            | ValType::NonNullExternRef
            | ValType::NonNullTypeRef(_)
            | ValType::NonNullAnyRef
            | ValType::NonNullEqRef
            | ValType::NonNullStructRef
            | ValType::NonNullArrayRef
            | ValType::NonNullI31Ref
    )
}

fn initial_local_initialized(locals: &[ValType], param_count: usize) -> Vec<bool> {
    locals
        .iter()
        .enumerate()
        .map(|(idx, ty)| idx < param_count || val_type_is_defaultable(*ty))
        .collect()
}

fn ensure_local_initialized(
    initialized: &[bool],
    locals: &[ValType],
    idx: u32,
) -> Result<(), String> {
    let local_idx = idx as usize;
    if local_idx >= locals.len() {
        return Err(format!("local index {} out of bounds", idx));
    }
    if !initialized.get(local_idx).copied().unwrap_or(false) {
        return Err(format!("uninitialized non-defaultable local: {}", idx));
    }
    Ok(())
}

fn declared_function_indices(m: &Module, func_count: usize) -> Result<Vec<bool>, String> {
    let mut declared = vec![false; func_count];
    for ex in &m.exports {
        if ex.kind == ExportKind::Func {
            if (ex.index as usize) < func_count {
                declared[ex.index as usize] = true;
            }
        }
    }
    for (seg_idx, seg) in m.elements.iter().enumerate() {
        for item in &seg.items {
            match item {
                ElementItem::Func(func_idx) => {
                    validate_element_func_index(seg_idx, *func_idx, func_count)?;
                    declared[*func_idx as usize] = true;
                }
                ElementItem::Expr(expr) => {
                    for ins in expr {
                        if let Instr::RefFunc(func_idx) = ins {
                            validate_element_func_index(seg_idx, *func_idx, func_count)?;
                            declared[*func_idx as usize] = true;
                        }
                    }
                }
            }
        }
    }
    for (global_idx, global) in m.globals.iter().enumerate() {
        for ins in &global.init {
            if let Instr::RefFunc(func_idx) = ins {
                if *func_idx as usize >= func_count {
                    return Err(format!(
                        "global {} initializer: ref.func function index {} out of bounds ({} funcs)",
                        global_idx, func_idx, func_count
                    ));
                }
                declared[*func_idx as usize] = true;
            }
        }
    }
    Ok(declared)
}

fn validate_element_func_index(
    seg_idx: usize,
    func_idx: u32,
    func_count: usize,
) -> Result<(), String> {
    if func_idx as usize >= func_count {
        return Err(format!(
            "element segment {} function index {} out of bounds ({} funcs)",
            seg_idx, func_idx, func_count
        ));
    }
    Ok(())
}

fn validate_ref_func_declared(
    body: &[Instr],
    func_count: usize,
    declared_funcs: &[bool],
) -> Result<(), String> {
    for ins in body {
        if let Instr::RefFunc(idx) = ins {
            if *idx as usize >= func_count {
                return Err(format!(
                    "ref.func function index {} out of bounds ({} funcs)",
                    idx, func_count
                ));
            }
            if !declared_funcs.get(*idx as usize).copied().unwrap_or(false) {
                return Err(format!("undeclared ref.func function index {}", idx));
            }
        }
    }
    Ok(())
}

fn function_type_indices(m: &Module) -> Vec<u32> {
    let mut indices = Vec::with_capacity(m.imported_func_count + m.func_types.len());
    for imp in &m.imports {
        if let ImportKind::Func(type_idx) = imp.kind {
            indices.push(type_idx);
        }
    }
    indices.extend(m.func_types.iter().copied());
    indices
}

fn global_count(m: &Module) -> usize {
    m.imports
        .iter()
        .filter(|imp| matches!(imp.kind, ImportKind::Global { .. }))
        .count()
        + m.globals.len()
}

fn validate_import_export_start(m: &Module) -> Result<(), String> {
    let funcs = function_type_indices(m);
    if let Some(start) = m.start {
        let type_idx = funcs.get(start as usize).copied().ok_or_else(|| {
            format!(
                "start function index {} out of bounds ({} funcs)",
                start,
                funcs.len()
            )
        })? as usize;
        let ftype = &m.types[type_idx];
        if !ftype.params.is_empty() || !ftype.results.is_empty() {
            return Err(format!(
                "start function {} must have empty params/results (got {}/{})",
                start,
                ftype.params.len(),
                ftype.results.len()
            ));
        }
    }

    let mut seen_exports: Vec<&str> = Vec::new();
    let globals = global_count(m);
    for ex in &m.exports {
        if seen_exports.iter().any(|name| *name == ex.name) {
            return Err(format!("duplicate export name '{}'", ex.name));
        }
        seen_exports.push(&ex.name);
        match ex.kind {
            ExportKind::Func => {
                if ex.index as usize >= funcs.len() {
                    return Err(format!(
                        "function export '{}' index {} out of bounds ({} funcs)",
                        ex.name,
                        ex.index,
                        funcs.len()
                    ));
                }
            }
            ExportKind::Table => {
                if ex.index as usize >= m.tables.len() {
                    return Err(format!(
                        "table export '{}' index {} out of bounds ({} tables)",
                        ex.name,
                        ex.index,
                        m.tables.len()
                    ));
                }
            }
            ExportKind::Memory => {
                if ex.index as usize >= m.memories.len() {
                    return Err(format!(
                        "memory export '{}' index {} out of bounds ({} memories)",
                        ex.name,
                        ex.index,
                        m.memories.len()
                    ));
                }
            }
            ExportKind::Global => {
                if ex.index as usize >= globals {
                    return Err(format!(
                        "global export '{}' index {} out of bounds ({} globals)",
                        ex.name, ex.index, globals
                    ));
                }
            }
            ExportKind::Tag => {
                if ex.index as usize >= m.tags.len() {
                    return Err(format!(
                        "tag export '{}' index {} out of bounds ({} tags)",
                        ex.name,
                        ex.index,
                        m.tags.len()
                    ));
                }
            }
        }
    }

    Ok(())
}

fn imported_global_types(m: &Module) -> Vec<(ValType, bool)> {
    let mut globals = Vec::new();
    for imp in &m.imports {
        if let ImportKind::Global { ty, mutable } = imp.kind {
            globals.push((ty, mutable));
        }
    }
    globals
}

fn validate_initializer_expressions(m: &Module) -> Result<(), String> {
    let mut globals = imported_global_types(m);
    let funcs = function_type_indices(m);

    for (i, table) in m.tables.iter().enumerate() {
        match m.table_inits.get(i).and_then(|init| init.as_ref()) {
            Some(init) => {
                let init_ty = const_expr_type(init, &globals, &m.array_types, &m.struct_types)?;
                if !types_compatible_in_module(
                    init_ty,
                    table.elem,
                    &m.type_supertypes,
                    &m.array_types,
                    &m.struct_types,
                    &m.types,
                ) && !ref_func_initializer_matches(init, table.elem, &funcs, m)
                {
                    return Err(format!(
                        "table {} initializer type {:?} does not match element {:?}",
                        i, init_ty, table.elem
                    ));
                }
            }
            None if !val_type_is_defaultable(table.elem) => {
                return Err(format!(
                    "table {} element type {:?} is not defaultable and has no initializer",
                    i, table.elem
                ));
            }
            None => {}
        }
    }

    for (i, global) in m.globals.iter().enumerate() {
        let init_ty = const_expr_type(&global.init, &globals, &m.array_types, &m.struct_types)?;
        if !types_compatible_in_module(
            init_ty,
            global.ty,
            &m.type_supertypes,
            &m.array_types,
            &m.struct_types,
            &m.types,
        ) && !ref_func_initializer_matches(&global.init, global.ty, &funcs, m)
        {
            return Err(format!(
                "global {} initializer type {:?} does not match declared {:?}",
                i, init_ty, global.ty
            ));
        }
        globals.push((global.ty, global.mutable));
    }

    for (i, seg) in m.elements.iter().enumerate() {
        if let Some(offset) = &seg.offset {
            let offset_ty = const_expr_type(offset, &globals, &m.array_types, &m.struct_types)?;
            let expected = m
                .tables
                .get(seg.table as usize)
                .map(|table| table_index_type(*table))
                .unwrap_or(ValType::I32);
            if offset_ty != expected {
                return Err(format!(
                    "element segment {} offset initializer type {:?} does not match {:?}",
                    i, offset_ty, expected
                ));
            }
        }
        for item in &seg.items {
            if let ElementItem::Expr(expr) = item {
                let item_ty = const_expr_type(expr, &globals, &m.array_types, &m.struct_types)?;
                if !types_compatible_in_module(
                    item_ty,
                    seg.ty,
                    &m.type_supertypes,
                    &m.array_types,
                    &m.struct_types,
                    &m.types,
                ) && !ref_func_initializer_matches(expr, seg.ty, &funcs, m)
                {
                    return Err(format!(
                        "element segment {} item type {:?} does not match declared {:?}",
                        i, item_ty, seg.ty
                    ));
                }
            }
        }
    }

    for (i, seg) in m.data.iter().enumerate() {
        if let Some(offset) = &seg.offset {
            let offset_ty = const_expr_type(offset, &globals, &m.array_types, &m.struct_types)?;
            let expected = m
                .memories
                .get(seg.memory as usize)
                .map(|limits| {
                    if limits.memory64 {
                        ValType::I64
                    } else {
                        ValType::I32
                    }
                })
                .unwrap_or(ValType::I32);
            if offset_ty != expected {
                return Err(format!(
                    "data segment {} offset initializer type {:?} does not match {:?}",
                    i, offset_ty, expected
                ));
            }
        }
    }

    Ok(())
}

fn validate_limits_and_index_bounds(m: &Module) -> Result<(), String> {
    let memory_count = m.memories.len();
    for (i, seg) in m.data.iter().enumerate() {
        if !seg.passive && seg.memory as usize >= memory_count {
            return Err(format!(
                "data segment {} memory index {} out of bounds ({} memories)",
                i, seg.memory, memory_count
            ));
        }
    }
    Ok(())
}

fn validate_bulk_memory_datacount(m: &Module) -> Result<(), String> {
    for (defined_idx, code) in m.code.iter().enumerate() {
        for ins in &code.body {
            match ins {
                Instr::MemoryInit(dataidx, _) => {
                    validate_bulk_data_index(m, defined_idx, "memory.init", *dataidx)?;
                }
                Instr::DataDrop(dataidx) => {
                    validate_bulk_data_index(m, defined_idx, "data.drop", *dataidx)?;
                }
                Instr::TableGrow(tableidx) => {
                    validate_table_index(m, defined_idx, "table.grow", *tableidx)?
                }
                Instr::TableSize(tableidx) => {
                    validate_table_index(m, defined_idx, "table.size", *tableidx)?
                }
                Instr::TableFill(tableidx) => {
                    validate_table_index(m, defined_idx, "table.fill", *tableidx)?
                }
                Instr::TableInit(elemidx, tableidx) => {
                    validate_elem_index(m, defined_idx, "table.init", *elemidx)?;
                    validate_table_index(m, defined_idx, "table.init", *tableidx)?;
                    validate_elem_table_compat(m, defined_idx, "table.init", *elemidx, *tableidx)?;
                }
                Instr::ElemDrop(elemidx) => {
                    validate_elem_index(m, defined_idx, "elem.drop", *elemidx)?;
                }
                Instr::TableCopy(dst, src) => {
                    validate_table_index(m, defined_idx, "table.copy", *dst)?;
                    validate_table_index(m, defined_idx, "table.copy", *src)?;
                    let dst_ty = m.tables[*dst as usize].elem;
                    let src_ty = m.tables[*src as usize].elem;
                    if !types_compatible_in_module(
                        src_ty,
                        dst_ty,
                        &m.type_supertypes,
                        &m.array_types,
                        &m.struct_types,
                        &m.types,
                    ) {
                        return Err(format!(
                            "function {}: table.copy table {} type {:?} does not match table {} type {:?}",
                            defined_idx, src, src_ty, dst, dst_ty
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn validate_bulk_data_index(
    m: &Module,
    defined_idx: usize,
    op: &str,
    dataidx: u32,
) -> Result<(), String> {
    if m.data_count.is_none() {
        return Err(format!(
            "function {}: {} requires DataCount section",
            defined_idx, op
        ));
    }
    if dataidx as usize >= m.data.len() {
        return Err(format!(
            "function {}: {} data index {} out of bounds ({} data segments)",
            defined_idx,
            op,
            dataidx,
            m.data.len()
        ));
    }
    Ok(())
}

fn validate_table_index(
    m: &Module,
    defined_idx: usize,
    op: &str,
    tableidx: u32,
) -> Result<(), String> {
    if tableidx as usize >= m.tables.len() {
        return Err(format!(
            "function {}: {} table index {} out of bounds ({} tables)",
            defined_idx,
            op,
            tableidx,
            m.tables.len()
        ));
    }
    Ok(())
}

fn validate_elem_index(
    m: &Module,
    defined_idx: usize,
    op: &str,
    elemidx: u32,
) -> Result<(), String> {
    if elemidx as usize >= m.elements.len() {
        return Err(format!(
            "function {}: {} element index {} out of bounds ({} element segments)",
            defined_idx,
            op,
            elemidx,
            m.elements.len()
        ));
    }
    Ok(())
}

fn validate_elem_table_compat(
    m: &Module,
    defined_idx: usize,
    op: &str,
    elemidx: u32,
    tableidx: u32,
) -> Result<(), String> {
    let elem_ty = m.elements[elemidx as usize].ty;
    let table_ty = m.tables[tableidx as usize].elem;
    if !types_compatible_in_module(
        elem_ty,
        table_ty,
        &m.type_supertypes,
        &m.array_types,
        &m.struct_types,
        &m.types,
    ) {
        return Err(format!(
            "function {}: {} element segment {} type {:?} does not match table {} type {:?}",
            defined_idx, op, elemidx, elem_ty, tableidx, table_ty
        ));
    }
    Ok(())
}

fn validate_operand_stack_control_flow(m: &Module) -> Result<(), String> {
    let funcs = function_type_indices(m);
    let global_tys = all_global_types(m);
    let global_mutability = all_global_mutability(m);
    let table_tys = m.tables.clone();
    let memory_index_ty = default_memory_index_type(m);
    let memory_shared = m.memories.first().map(|lim| lim.shared).unwrap_or(false);
    for (defined_idx, code) in m.code.iter().enumerate() {
        let type_idx = m.func_types[defined_idx] as usize;
        let ftype = &m.types[type_idx];
        let mut locals = ftype.params.clone();
        locals.extend(code.locals.iter().copied());
        validate_branch_depths(&code.body)?;
        if is_straight_line_body(&code.body) {
            validate_straight_line_stack(
                &code.body,
                &locals,
                &global_tys,
                &global_mutability,
                &funcs,
                &m.types,
                &table_tys,
                &m.array_types,
                &m.struct_types,
                &m.type_supertypes,
                &m.elements,
                &m.tags,
                &m.memories,
                memory_index_ty,
                memory_shared,
                &ftype.results,
                ftype.params.len(),
            )
            .map_err(|err| format!("function {}: {}", defined_idx, err))?;
        } else {
            validate_control_flow_stack(
                &code.body,
                &locals,
                &global_tys,
                &global_mutability,
                &funcs,
                &m.types,
                &table_tys,
                &m.array_types,
                &m.struct_types,
                &m.type_supertypes,
                &m.elements,
                &m.tags,
                &m.memories,
                memory_index_ty,
                memory_shared,
                &ftype.results,
                ftype.params.len(),
            )
            .map_err(|err| format!("function {}: {}", defined_idx, err))?;
        }
    }
    Ok(())
}

fn default_memory_index_type(m: &Module) -> ValType {
    if m.memories.first().map(|lim| lim.memory64).unwrap_or(false) {
        ValType::I64
    } else {
        ValType::I32
    }
}

fn all_global_types(m: &Module) -> Vec<ValType> {
    let mut globals = Vec::new();
    for imp in &m.imports {
        if let ImportKind::Global { ty, .. } = imp.kind {
            globals.push(ty);
        }
    }
    globals.extend(m.globals.iter().map(|g| g.ty));
    globals
}

fn all_global_mutability(m: &Module) -> Vec<bool> {
    let mut globals = Vec::new();
    for imp in &m.imports {
        if let ImportKind::Global { mutable, .. } = imp.kind {
            globals.push(mutable);
        }
    }
    globals.extend(m.globals.iter().map(|g| g.mutable));
    globals
}

fn validate_branch_depths(body: &[Instr]) -> Result<(), String> {
    let mut depth = 0usize;
    for ins in body {
        match ins {
            Instr::LegacyTry(_) => depth += 1,
            Instr::TryTable(_, handlers) => {
                for handler in handlers {
                    let label = match handler {
                        CatchKind::Catch { label, .. }
                        | CatchKind::CatchRef { label, .. }
                        | CatchKind::CatchAll { label }
                        | CatchKind::CatchAllRef { label } => *label,
                    };
                    if label as usize > depth {
                        return Err(format!(
                            "try_table handler branch depth {} out of bounds ({} labels)",
                            label,
                            depth + 1
                        ));
                    }
                }
                depth += 1;
            }
            Instr::Block(_) | Instr::Loop(_) | Instr::If(_) => depth += 1,
            Instr::LegacyDelegate(label) | Instr::LegacyRethrow(label) => {
                if *label as usize > depth {
                    return Err(format!(
                        "legacy exception branch depth {} out of bounds ({} labels)",
                        label,
                        depth + 1
                    ));
                }
                if matches!(ins, Instr::LegacyDelegate(_)) {
                    depth = depth.saturating_sub(1);
                }
            }
            Instr::End => {
                depth = depth.saturating_sub(1);
            }
            Instr::Br(label) | Instr::BrIf(label) => {
                if *label as usize > depth {
                    return Err(format!(
                        "branch depth {} out of bounds ({} labels)",
                        label,
                        depth + 1
                    ));
                }
            }
            Instr::BrTable(targets, default) => {
                for label in targets.iter().chain(std::iter::once(default)) {
                    if *label as usize > depth {
                        return Err(format!(
                            "branch depth {} out of bounds ({} labels)",
                            label,
                            depth + 1
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn is_straight_line_body(body: &[Instr]) -> bool {
    !body.iter().any(|ins| {
        matches!(
            ins,
            Instr::Block(_)
                | Instr::Loop(_)
                | Instr::If(_)
                | Instr::LegacyTry(_)
                | Instr::TryTable(_, _)
                | Instr::Else
                | Instr::LegacyCatch(_)
                | Instr::LegacyCatchAll
                | Instr::Br(_)
                | Instr::BrIf(_)
                | Instr::BrTable(_, _)
                | Instr::Unreachable
                | Instr::Return
                | Instr::ReturnCall(_)
                | Instr::ReturnCallIndirect(_, _)
                | Instr::ReturnCallRef(_)
                | Instr::Throw(_)
                | Instr::LegacyRethrow(_)
                | Instr::LegacyDelegate(_)
                | Instr::ThrowRef
        )
    })
}

fn validate_straight_line_stack(
    body: &[Instr],
    locals: &[ValType],
    globals: &[ValType],
    global_mutability: &[bool],
    funcs: &[u32],
    types: &[FuncType],
    table_types: &[TableType],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    type_supertypes: &[Vec<u32>],
    elements: &[ElementSegment],
    tags: &[Tag],
    memories: &[Limits],
    memory_index_ty: ValType,
    memory_shared: bool,
    results: &[ValType],
    param_count: usize,
) -> Result<(), String> {
    let mut stack = Vec::new();
    let mut returned = false;
    let mut local_initialized = initial_local_initialized(locals, param_count);
    for ins in body {
        if returned {
            if matches!(ins, Instr::End) {
                break;
            }
            continue;
        }
        match ins {
            Instr::End => break,
            Instr::Nop => {}
            Instr::I32Const(_) => stack.push(ValType::I32),
            Instr::I64Const(_) => stack.push(ValType::I64),
            Instr::F32Const(_) => stack.push(ValType::F32),
            Instr::F64Const(_) => stack.push(ValType::F64),
            Instr::V128Const(_) => stack.push(ValType::V128),
            Instr::I8x16Shuffle(lanes) => {
                for lane in lanes {
                    validate_lane(*lane, 32, "i8x16.shuffle")?;
                }
                pop_expect(&mut stack, ValType::V128)?;
                pop_expect(&mut stack, ValType::V128)?;
                stack.push(ValType::V128);
            }
            Instr::I8x16Swizzle => {
                pop_expect(&mut stack, ValType::V128)?;
                pop_expect(&mut stack, ValType::V128)?;
                stack.push(ValType::V128);
            }
            Instr::I8x16Splat | Instr::I16x8Splat | Instr::I32x4Splat => {
                pop_expect(&mut stack, memory_index_ty)?;
                stack.push(ValType::V128);
            }
            Instr::I64x2Splat => {
                pop_expect(&mut stack, ValType::I64)?;
                stack.push(ValType::V128);
            }
            Instr::F32x4Splat => {
                pop_expect(&mut stack, ValType::F32)?;
                stack.push(ValType::V128);
            }
            Instr::F64x2Splat => {
                pop_expect(&mut stack, ValType::F64)?;
                stack.push(ValType::V128);
            }
            Instr::I8x16ExtractLaneS(lane) | Instr::I8x16ExtractLaneU(lane) => {
                validate_lane(*lane, 16, "i8x16.extract_lane")?;
                pop_expect(&mut stack, ValType::V128)?;
                stack.push(ValType::I32);
            }
            Instr::I16x8ExtractLaneS(lane) | Instr::I16x8ExtractLaneU(lane) => {
                validate_lane(*lane, 8, "i16x8.extract_lane")?;
                pop_expect(&mut stack, ValType::V128)?;
                stack.push(ValType::I32);
            }
            Instr::I32x4ExtractLane(lane) => {
                validate_lane(*lane, 4, "i32x4.extract_lane")?;
                pop_expect(&mut stack, ValType::V128)?;
                stack.push(ValType::I32);
            }
            Instr::I64x2ExtractLane(lane) => {
                validate_lane(*lane, 2, "i64x2.extract_lane")?;
                pop_expect(&mut stack, ValType::V128)?;
                stack.push(ValType::I64);
            }
            Instr::F32x4ExtractLane(lane) => {
                validate_lane(*lane, 4, "f32x4.extract_lane")?;
                pop_expect(&mut stack, ValType::V128)?;
                stack.push(ValType::F32);
            }
            Instr::F64x2ExtractLane(lane) => {
                validate_lane(*lane, 2, "f64x2.extract_lane")?;
                pop_expect(&mut stack, ValType::V128)?;
                stack.push(ValType::F64);
            }
            Instr::I8x16ReplaceLane(lane) => {
                validate_replace_lane(&mut stack, *lane, 16, ValType::I32, "i8x16.replace_lane")?;
            }
            Instr::I16x8ReplaceLane(lane) => {
                validate_replace_lane(&mut stack, *lane, 8, ValType::I32, "i16x8.replace_lane")?;
            }
            Instr::I32x4ReplaceLane(lane) => {
                validate_replace_lane(&mut stack, *lane, 4, ValType::I32, "i32x4.replace_lane")?;
            }
            Instr::I64x2ReplaceLane(lane) => {
                validate_replace_lane(&mut stack, *lane, 2, ValType::I64, "i64x2.replace_lane")?;
            }
            Instr::F32x4ReplaceLane(lane) => {
                validate_replace_lane(&mut stack, *lane, 4, ValType::F32, "f32x4.replace_lane")?;
            }
            Instr::F64x2ReplaceLane(lane) => {
                validate_replace_lane(&mut stack, *lane, 2, ValType::F64, "f64x2.replace_lane")?;
            }
            Instr::I8x16Eq
            | Instr::I8x16Ne
            | Instr::I8x16LtS
            | Instr::I8x16LtU
            | Instr::I8x16GtS
            | Instr::I8x16GtU
            | Instr::I8x16LeS
            | Instr::I8x16LeU
            | Instr::I8x16GeS
            | Instr::I8x16GeU
            | Instr::I16x8Eq
            | Instr::I16x8Ne
            | Instr::I16x8LtS
            | Instr::I16x8LtU
            | Instr::I16x8GtS
            | Instr::I16x8GtU
            | Instr::I16x8LeS
            | Instr::I16x8LeU
            | Instr::I16x8GeS
            | Instr::I16x8GeU
            | Instr::I32x4Eq
            | Instr::I32x4Ne
            | Instr::I32x4LtS
            | Instr::I32x4LtU
            | Instr::I32x4GtS
            | Instr::I32x4GtU
            | Instr::I32x4LeS
            | Instr::I32x4LeU
            | Instr::I32x4GeS
            | Instr::I32x4GeU
            | Instr::I64x2Eq
            | Instr::I64x2Ne
            | Instr::I64x2LtS
            | Instr::I64x2GtS
            | Instr::I64x2LeS
            | Instr::I64x2GeS
            | Instr::F32x4Ne
            | Instr::F32x4Lt
            | Instr::F32x4Gt
            | Instr::F32x4Le
            | Instr::F32x4Ge
            | Instr::F64x2Ne
            | Instr::F64x2Lt
            | Instr::F64x2Gt
            | Instr::F64x2Le
            | Instr::F64x2Ge
            | Instr::I8x16NarrowI16x8S
            | Instr::I8x16NarrowI16x8U
            | Instr::I16x8NarrowI32x4S
            | Instr::I16x8NarrowI32x4U
            | Instr::I16x8ExtMulLowI8x16S
            | Instr::I16x8ExtMulHighI8x16S
            | Instr::I16x8ExtMulLowI8x16U
            | Instr::I16x8ExtMulHighI8x16U
            | Instr::I32x4DotI16x8S
            | Instr::I32x4ExtMulLowI16x8S
            | Instr::I32x4ExtMulHighI16x8S
            | Instr::I32x4ExtMulLowI16x8U
            | Instr::I32x4ExtMulHighI16x8U
            | Instr::I64x2ExtMulLowI32x4S
            | Instr::I64x2ExtMulHighI32x4S
            | Instr::I64x2ExtMulLowI32x4U
            | Instr::I64x2ExtMulHighI32x4U
            | Instr::I8x16Add
            | Instr::I8x16AddSatS
            | Instr::I8x16AddSatU
            | Instr::I8x16Sub
            | Instr::I8x16SubSatS
            | Instr::I8x16SubSatU
            | Instr::I8x16MinS
            | Instr::I8x16MinU
            | Instr::I8x16MaxS
            | Instr::I8x16MaxU
            | Instr::I8x16AvgrU
            | Instr::I16x8Add
            | Instr::I16x8AddSatS
            | Instr::I16x8AddSatU
            | Instr::I16x8Sub
            | Instr::I16x8SubSatS
            | Instr::I16x8SubSatU
            | Instr::I16x8Mul
            | Instr::I16x8MinS
            | Instr::I16x8MinU
            | Instr::I16x8MaxS
            | Instr::I16x8MaxU
            | Instr::I16x8AvgrU
            | Instr::I16x8Q15mulrSatS
            | Instr::I16x8RelaxedQ15mulrS
            | Instr::I16x8RelaxedDotI8x16I7x16S
            | Instr::I32x4Add
            | Instr::I32x4Sub
            | Instr::I32x4Mul
            | Instr::I32x4MinS
            | Instr::I32x4MinU
            | Instr::I32x4MaxS
            | Instr::I32x4MaxU
            | Instr::I64x2Add
            | Instr::I64x2Sub
            | Instr::I64x2Mul
            | Instr::F32x4Eq
            | Instr::F64x2Eq
            | Instr::F32x4Add
            | Instr::F32x4Sub
            | Instr::F32x4Mul
            | Instr::F32x4Div
            | Instr::F32x4Min
            | Instr::F32x4Max
            | Instr::F32x4PMin
            | Instr::F32x4PMax
            | Instr::F64x2Add
            | Instr::F64x2Sub
            | Instr::F64x2Mul
            | Instr::F64x2Div
            | Instr::F64x2Min
            | Instr::F64x2Max
            | Instr::F64x2PMin
            | Instr::F64x2PMax
            | Instr::V128And
            | Instr::V128AndNot
            | Instr::V128Or
            | Instr::V128Xor => {
                pop_expect(&mut stack, ValType::V128)?;
                pop_expect(&mut stack, ValType::V128)?;
                stack.push(ValType::V128);
            }
            Instr::I8x16Shl
            | Instr::I8x16ShrS
            | Instr::I8x16ShrU
            | Instr::I16x8Shl
            | Instr::I16x8ShrS
            | Instr::I16x8ShrU
            | Instr::I32x4Shl
            | Instr::I32x4ShrS
            | Instr::I32x4ShrU
            | Instr::I64x2Shl
            | Instr::I64x2ShrS
            | Instr::I64x2ShrU => {
                pop_expect(&mut stack, ValType::I32)?;
                pop_expect(&mut stack, ValType::V128)?;
                stack.push(ValType::V128);
            }
            Instr::V128BitSelect => {
                pop_expect(&mut stack, ValType::V128)?;
                pop_expect(&mut stack, ValType::V128)?;
                pop_expect(&mut stack, ValType::V128)?;
                stack.push(ValType::V128);
            }
            Instr::I32x4RelaxedDotI8x16I7x16AddS => {
                pop_expect(&mut stack, ValType::V128)?;
                pop_expect(&mut stack, ValType::V128)?;
                pop_expect(&mut stack, ValType::V128)?;
                stack.push(ValType::V128);
            }
            Instr::F32x4RelaxedMadd
            | Instr::F32x4RelaxedNmadd
            | Instr::F64x2RelaxedMadd
            | Instr::F64x2RelaxedNmadd => {
                pop_expect(&mut stack, ValType::V128)?;
                pop_expect(&mut stack, ValType::V128)?;
                pop_expect(&mut stack, ValType::V128)?;
                stack.push(ValType::V128);
            }
            Instr::I16x8ExtAddPairwiseI8x16S
            | Instr::I16x8ExtAddPairwiseI8x16U
            | Instr::I32x4ExtAddPairwiseI16x8S
            | Instr::I32x4ExtAddPairwiseI16x8U
            | Instr::I8x16Abs
            | Instr::I8x16Neg
            | Instr::I8x16Popcnt
            | Instr::I16x8Abs
            | Instr::I16x8Neg
            | Instr::I32x4Abs
            | Instr::I32x4Neg
            | Instr::I64x2Abs
            | Instr::I64x2Neg
            | Instr::F32x4Abs
            | Instr::F32x4Neg
            | Instr::F32x4Ceil
            | Instr::F32x4Floor
            | Instr::F32x4Trunc
            | Instr::F32x4Nearest
            | Instr::F64x2Abs
            | Instr::F64x2Neg
            | Instr::F64x2Ceil
            | Instr::F64x2Floor
            | Instr::F64x2Trunc
            | Instr::F64x2Nearest
            | Instr::I16x8ExtendLowI8x16S
            | Instr::I16x8ExtendHighI8x16S
            | Instr::I16x8ExtendLowI8x16U
            | Instr::I16x8ExtendHighI8x16U
            | Instr::I32x4ExtendLowI16x8S
            | Instr::I32x4ExtendHighI16x8S
            | Instr::I32x4ExtendLowI16x8U
            | Instr::I32x4ExtendHighI16x8U
            | Instr::I64x2ExtendLowI32x4S
            | Instr::I64x2ExtendHighI32x4S
            | Instr::I64x2ExtendLowI32x4U
            | Instr::I64x2ExtendHighI32x4U
            | Instr::I32x4TruncSatF32x4S
            | Instr::I32x4TruncSatF32x4U
            | Instr::I32x4TruncSatF64x2SZero
            | Instr::I32x4TruncSatF64x2UZero
            | Instr::F32x4ConvertI32x4S
            | Instr::F32x4ConvertI32x4U
            | Instr::F32x4DemoteF64x2Zero
            | Instr::F64x2ConvertLowI32x4S
            | Instr::F64x2ConvertLowI32x4U
            | Instr::F64x2PromoteLowF32x4
            | Instr::F32x4Sqrt
            | Instr::F64x2Sqrt
            | Instr::V128Not => {
                pop_expect(&mut stack, ValType::V128)?;
                stack.push(ValType::V128);
            }
            Instr::V128AnyTrue => {
                pop_expect(&mut stack, ValType::V128)?;
                stack.push(ValType::I32);
            }
            Instr::I8x16AllTrue
            | Instr::I8x16Bitmask
            | Instr::I16x8AllTrue
            | Instr::I16x8Bitmask
            | Instr::I32x4AllTrue
            | Instr::I32x4Bitmask
            | Instr::I64x2AllTrue
            | Instr::I64x2Bitmask => {
                pop_expect(&mut stack, ValType::V128)?;
                stack.push(ValType::I32);
            }
            Instr::V128Load(_)
            | Instr::V128Load8Splat(_)
            | Instr::V128Load16Splat(_)
            | Instr::V128Load32Splat(_)
            | Instr::V128Load64Splat(_)
            | Instr::V128Load8x8S(_)
            | Instr::V128Load8x8U(_)
            | Instr::V128Load16x4S(_)
            | Instr::V128Load16x4U(_)
            | Instr::V128Load32x2S(_)
            | Instr::V128Load32x2U(_)
            | Instr::V128Load32Zero(_)
            | Instr::V128Load64Zero(_) => {
                validate_simd_memarg(ins, memories)?;
                pop_expect(&mut stack, memory_index_ty)?;
                stack.push(ValType::V128);
            }
            Instr::V128Store(_) => {
                validate_simd_memarg(ins, memories)?;
                pop_expect(&mut stack, ValType::V128)?;
                pop_expect(&mut stack, memory_index_ty)?;
            }
            Instr::V128Load8Lane(_, lane) => {
                validate_simd_memarg(ins, memories)?;
                validate_lane(*lane, 16, "v128.load8_lane")?;
                pop_expect(&mut stack, ValType::V128)?;
                pop_expect(&mut stack, memory_index_ty)?;
                stack.push(ValType::V128);
            }
            Instr::V128Load16Lane(_, lane) => {
                validate_simd_memarg(ins, memories)?;
                validate_lane(*lane, 8, "v128.load16_lane")?;
                pop_expect(&mut stack, ValType::V128)?;
                pop_expect(&mut stack, memory_index_ty)?;
                stack.push(ValType::V128);
            }
            Instr::V128Load32Lane(_, lane) => {
                validate_simd_memarg(ins, memories)?;
                validate_lane(*lane, 4, "v128.load32_lane")?;
                pop_expect(&mut stack, ValType::V128)?;
                pop_expect(&mut stack, memory_index_ty)?;
                stack.push(ValType::V128);
            }
            Instr::V128Load64Lane(_, lane) => {
                validate_simd_memarg(ins, memories)?;
                validate_lane(*lane, 2, "v128.load64_lane")?;
                pop_expect(&mut stack, ValType::V128)?;
                pop_expect(&mut stack, memory_index_ty)?;
                stack.push(ValType::V128);
            }
            Instr::V128Store8Lane(_, lane) => {
                validate_simd_memarg(ins, memories)?;
                validate_lane(*lane, 16, "v128.store8_lane")?;
                pop_expect(&mut stack, ValType::V128)?;
                pop_expect(&mut stack, memory_index_ty)?;
            }
            Instr::V128Store16Lane(_, lane) => {
                validate_simd_memarg(ins, memories)?;
                validate_lane(*lane, 8, "v128.store16_lane")?;
                pop_expect(&mut stack, ValType::V128)?;
                pop_expect(&mut stack, memory_index_ty)?;
            }
            Instr::V128Store32Lane(_, lane) => {
                validate_simd_memarg(ins, memories)?;
                validate_lane(*lane, 4, "v128.store32_lane")?;
                pop_expect(&mut stack, ValType::V128)?;
                pop_expect(&mut stack, memory_index_ty)?;
            }
            Instr::V128Store64Lane(_, lane) => {
                validate_simd_memarg(ins, memories)?;
                validate_lane(*lane, 2, "v128.store64_lane")?;
                pop_expect(&mut stack, ValType::V128)?;
                pop_expect(&mut stack, memory_index_ty)?;
            }
            Instr::RefNull(ty) => stack.push(*ty),
            Instr::RefIsNull => {
                let ty = stack
                    .pop()
                    .ok_or_else(|| "operand stack underflow".to_string())?;
                if !is_ref_type(ty) {
                    return Err(format!(
                        "ref.is_null operand must be reference, got {:?}",
                        ty
                    ));
                }
                stack.push(ValType::I32);
            }
            Instr::RefAsNonNull => {
                let ty = stack
                    .pop()
                    .ok_or_else(|| "operand stack underflow".to_string())?;
                if !is_ref_type(ty) {
                    return Err(format!(
                        "ref.as_non_null operand must be reference, got {:?}",
                        ty
                    ));
                }
                stack.push(ty);
            }
            Instr::BrOnNull(_) => {
                let ty = pop_ref(&mut stack, "br_on_null")?;
                stack.push(ty);
            }
            Instr::BrOnNonNull(_) => {
                pop_ref(&mut stack, "br_on_non_null")?;
            }
            Instr::RefTest { .. } => {
                pop_ref(&mut stack, "ref.test")?;
                stack.push(ValType::I32);
            }
            Instr::RefCast { target, .. } => {
                pop_ref(&mut stack, "ref.cast")?;
                stack.push(*target);
            }
            Instr::BrOnCast { source, target, .. } => {
                pop_ref(&mut stack, "br_on_cast")?;
                stack.push(nullable_cast_complement_type(*source, *target));
            }
            Instr::BrOnCastFail { target, .. } => {
                pop_ref(&mut stack, "br_on_cast_fail")?;
                stack.push(*target);
            }
            Instr::AnyConvertExtern => {
                pop_ref(&mut stack, "any.convert_extern")?;
                stack.push(ValType::Unknown);
            }
            Instr::ExternConvertAny => {
                pop_ref(&mut stack, "extern.convert_any")?;
                stack.push(ValType::ExternRef);
            }
            Instr::StructNew(type_idx) => {
                let ty = struct_type(*type_idx, struct_types)?;
                for field in ty.fields.iter().rev() {
                    pop_expect(&mut stack, field.ty)?;
                }
                stack.push(ValType::NonNullTypeRef(*type_idx));
            }
            Instr::StructNewDefault(type_idx) => {
                let _ty = struct_type(*type_idx, struct_types)?;
                stack.push(ValType::NonNullTypeRef(*type_idx));
            }
            Instr::StructGet(type_idx, field_idx) => {
                let ty = struct_type(*type_idx, struct_types)?;
                let field = ty
                    .fields
                    .get(*field_idx as usize)
                    .ok_or_else(|| format!("struct field index {} out of bounds", field_idx))?;
                pop_ref(&mut stack, "struct.get")?;
                stack.push(field.ty);
            }
            Instr::StructGetS(type_idx, field_idx) | Instr::StructGetU(type_idx, field_idx) => {
                let ty = struct_type(*type_idx, struct_types)?;
                let field = ty
                    .fields
                    .get(*field_idx as usize)
                    .ok_or_else(|| format!("struct field index {} out of bounds", field_idx))?;
                if field.packed_bits.is_none() {
                    return Err(format!("struct.get_s/u field {} is not packed", field_idx));
                }
                pop_ref(&mut stack, "struct.get_s/u")?;
                stack.push(ValType::I32);
            }
            Instr::StructSet(type_idx, field_idx) => {
                let ty = struct_type(*type_idx, struct_types)?;
                let field = ty
                    .fields
                    .get(*field_idx as usize)
                    .ok_or_else(|| format!("struct field index {} out of bounds", field_idx))?;
                if !field.mutable {
                    return Err(format!("struct.set field {} is immutable", field_idx));
                }
                pop_expect(&mut stack, field.ty)?;
                pop_ref(&mut stack, "struct.set")?;
            }
            Instr::RefFunc(idx) => {
                let type_idx = funcs
                    .get(*idx as usize)
                    .ok_or_else(|| format!("ref.func function index {} out of bounds", idx))?;
                stack.push(ValType::NonNullTypeRef(*type_idx));
            }
            Instr::RefEq => {
                pop_expect_typed(
                    &mut stack,
                    ValType::EqRef,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                pop_expect_typed(
                    &mut stack,
                    ValType::EqRef,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                stack.push(ValType::I32);
            }
            Instr::ArrayNew(type_idx) => {
                let array = array_type(*type_idx, array_types)?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_expect(&mut stack, array.element)?;
                stack.push(ValType::Unknown);
            }
            Instr::ArrayNewDefault(type_idx) => {
                let _array = array_type(*type_idx, array_types)?;
                pop_expect(&mut stack, ValType::I32)?;
                stack.push(ValType::Unknown);
            }
            Instr::ArrayNewFixed(type_idx, count) => {
                let array = array_type(*type_idx, array_types)?;
                for _ in 0..*count {
                    pop_expect(&mut stack, array.element)?;
                }
                stack.push(ValType::Unknown);
            }
            Instr::ArrayNewData(type_idx, _) => {
                let _array = array_type(*type_idx, array_types)?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_expect(&mut stack, ValType::I32)?;
                stack.push(ValType::Unknown);
            }
            Instr::ArrayNewElem(type_idx, _) => {
                let _array = array_type(*type_idx, array_types)?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_expect(&mut stack, ValType::I32)?;
                stack.push(ValType::Unknown);
            }
            Instr::ArrayGet(type_idx) => {
                let array = array_type(*type_idx, array_types)?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_ref(&mut stack, "array.get")?;
                stack.push(array.element);
            }
            Instr::ArrayGetS(type_idx) | Instr::ArrayGetU(type_idx) => {
                let _array = array_type(*type_idx, array_types)?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_ref(&mut stack, "array.get_s/u")?;
                stack.push(ValType::I32);
            }
            Instr::ArraySet(type_idx) => {
                let array = array_type(*type_idx, array_types)?;
                if !array.mutable {
                    return Err(format!("array.set type {} is immutable", type_idx));
                }
                pop_expect(&mut stack, array.element)?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_ref(&mut stack, "array.set")?;
            }
            Instr::ArrayLen => {
                pop_ref(&mut stack, "array.len")?;
                stack.push(ValType::I32);
            }
            Instr::ArrayFill(type_idx) => {
                let array = array_type(*type_idx, array_types)?;
                if !array.mutable {
                    return Err(format!("array.fill type {} is immutable", type_idx));
                }
                pop_expect(&mut stack, ValType::I32)?;
                pop_expect(&mut stack, array.element)?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_ref(&mut stack, "array.fill")?;
            }
            Instr::ArrayCopy(dst_type_idx, src_type_idx) => {
                let dst = array_type(*dst_type_idx, array_types)?;
                let src = array_type(*src_type_idx, array_types)?;
                if !dst.mutable {
                    return Err(format!("array.copy type {} is immutable", dst_type_idx));
                }
                validate_array_copy_element_subtype(
                    *dst_type_idx,
                    dst,
                    *src_type_idx,
                    src,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_ref(&mut stack, "array.copy src")?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_ref(&mut stack, "array.copy dst")?;
            }
            Instr::ArrayInitData(type_idx, _) => {
                let array = array_type(*type_idx, array_types)?;
                if !array.mutable {
                    return Err(format!("array.init_data type {} is immutable", type_idx));
                }
                validate_array_init_data_storage(*type_idx, array)?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_ref(&mut stack, "array.init_data")?;
            }
            Instr::ArrayInitElem(type_idx, elem_idx) => {
                let array = array_type(*type_idx, array_types)?;
                if !array.mutable {
                    return Err(format!("array.init_elem type {} is immutable", type_idx));
                }
                validate_array_init_elem_storage(
                    *type_idx,
                    array,
                    *elem_idx,
                    elements,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_ref(&mut stack, "array.init_elem")?;
            }
            Instr::RefI31 => {
                pop_expect(&mut stack, ValType::I32)?;
                stack.push(ValType::Unknown);
            }
            Instr::I31GetS | Instr::I31GetU => {
                pop_ref(&mut stack, "i31.get")?;
                stack.push(ValType::I32);
            }
            Instr::TableGet(tableidx) => {
                let table = stack_table_type(*tableidx, table_types)?;
                pop_expect(&mut stack, table_index_type(table))?;
                let elem_ty = table.elem;
                stack.push(elem_ty);
            }
            Instr::TableSet(tableidx) => {
                let table = stack_table_type(*tableidx, table_types)?;
                let elem_ty = table.elem;
                pop_expect_typed(
                    &mut stack,
                    elem_ty,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                pop_expect(&mut stack, table_index_type(table))?;
            }
            Instr::LocalGet(idx) => {
                ensure_local_initialized(&local_initialized, locals, *idx)?;
                stack.push(
                    *locals
                        .get(*idx as usize)
                        .ok_or_else(|| format!("local index {} out of bounds", idx))?,
                );
            }
            Instr::LocalSet(idx) => {
                let ty = *locals
                    .get(*idx as usize)
                    .ok_or_else(|| format!("local index {} out of bounds", idx))?;
                pop_expect_typed(
                    &mut stack,
                    ty,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                if let Some(slot) = local_initialized.get_mut(*idx as usize) {
                    *slot = true;
                }
            }
            Instr::LocalTee(idx) => {
                let ty = *locals
                    .get(*idx as usize)
                    .ok_or_else(|| format!("local index {} out of bounds", idx))?;
                pop_expect_typed(
                    &mut stack,
                    ty,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                if let Some(slot) = local_initialized.get_mut(*idx as usize) {
                    *slot = true;
                }
                stack.push(ty);
            }
            Instr::GlobalGet(idx) => stack.push(
                *globals
                    .get(*idx as usize)
                    .ok_or_else(|| format!("global index {} out of bounds", idx))?,
            ),
            Instr::GlobalSet(idx) => {
                let ty = *globals
                    .get(*idx as usize)
                    .ok_or_else(|| format!("global index {} out of bounds", idx))?;
                if !global_mutability
                    .get(*idx as usize)
                    .copied()
                    .unwrap_or(false)
                {
                    return Err(format!("global.set {} targets immutable global", idx));
                }
                pop_expect_typed(
                    &mut stack,
                    ty,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
            }
            Instr::Drop => {
                stack
                    .pop()
                    .ok_or_else(|| "operand stack underflow".to_string())?;
            }
            Instr::Select => {
                pop_expect(&mut stack, ValType::I32)?;
                let b = stack
                    .pop()
                    .ok_or_else(|| "operand stack underflow".to_string())?;
                let a = stack
                    .pop()
                    .ok_or_else(|| "operand stack underflow".to_string())?;
                if a != b {
                    return Err(format!("select operand type mismatch {:?} vs {:?}", a, b));
                }
                if !is_untyped_select_value_type(a) {
                    return Err(format!("select without result type cannot select {:?}", a));
                }
                stack.push(a);
            }
            Instr::SelectTyped(ty) => {
                pop_expect(&mut stack, ValType::I32)?;
                pop_expect_typed(
                    &mut stack,
                    *ty,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                pop_expect_typed(
                    &mut stack,
                    *ty,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                stack.push(*ty);
            }
            Instr::Call(idx) => {
                let type_idx = *funcs
                    .get(*idx as usize)
                    .ok_or_else(|| format!("call function index {} out of bounds", idx))?
                    as usize;
                let ft = &types[type_idx];
                pop_params_typed(
                    &mut stack,
                    &ft.params,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                stack.extend(ft.results.iter().copied());
            }
            Instr::ReturnCall(idx) => {
                let type_idx = *funcs
                    .get(*idx as usize)
                    .ok_or_else(|| format!("return_call function index {} out of bounds", idx))?
                    as usize;
                let ft = &types[type_idx];
                pop_params_typed(
                    &mut stack,
                    &ft.params,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                if !result_types_compatible_in_module(
                    &ft.results,
                    results,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                ) {
                    return Err(format!(
                        "return_call result type {:?} does not match function result {:?}",
                        ft.results, results
                    ));
                }
                returned = true;
            }
            Instr::ReturnCallIndirect(type_idx, table_idx) => {
                let table = table_types.get(*table_idx as usize).ok_or_else(|| {
                    format!(
                        "return_call_indirect table index {} out of bounds",
                        table_idx
                    )
                })?;
                validate_indirect_call_table(
                    *table,
                    "return_call_indirect",
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                pop_expect(&mut stack, table_index_type(*table))?;
                let ft = types.get(*type_idx as usize).ok_or_else(|| {
                    format!("return_call_indirect type index {} out of bounds", type_idx)
                })?;
                pop_params_typed(
                    &mut stack,
                    &ft.params,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                if !result_types_compatible_in_module(
                    &ft.results,
                    results,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                ) {
                    return Err(format!(
                        "return_call_indirect result type {:?} does not match function result {:?}",
                        ft.results, results
                    ));
                }
                returned = true;
            }
            Instr::ReturnCallRef(type_idx) => {
                let ft = types.get(*type_idx as usize).ok_or_else(|| {
                    format!("return_call_ref type index {} out of bounds", type_idx)
                })?;
                let callee = stack.pop().ok_or("return_call_ref callee underflow")?;
                if !is_ref_type(callee) {
                    return Err(format!(
                        "return_call_ref callee must be reference, got {:?}",
                        callee
                    ));
                }
                if !types_compatible_in_module(
                    callee,
                    ValType::TypeRef(*type_idx),
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                ) {
                    return Err(format!(
                        "return_call_ref callee type mismatch: expected {:?}, got {:?}",
                        ValType::TypeRef(*type_idx),
                        callee
                    ));
                }
                pop_params_typed(
                    &mut stack,
                    &ft.params,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                if !result_types_compatible_in_module(
                    &ft.results,
                    results,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                ) {
                    return Err(format!(
                        "return_call_ref result type {:?} does not match function result {:?}",
                        ft.results, results
                    ));
                }
                returned = true;
            }
            Instr::TryTable(_, _) => {
                return Err("try_table requires structured validation".to_string());
            }
            Instr::Throw(tag_idx) => {
                let tag = tags
                    .get(*tag_idx as usize)
                    .ok_or_else(|| format!("throw tag index {} out of bounds", tag_idx))?;
                let ft = types.get(tag.type_idx as usize).ok_or_else(|| {
                    format!("throw tag type index {} out of bounds", tag.type_idx)
                })?;
                pop_params_typed(
                    &mut stack,
                    &ft.params,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                returned = true;
            }
            Instr::ThrowRef => {
                pop_expect(&mut stack, ValType::Unknown)?;
                returned = true;
            }
            Instr::CallIndirect(type_idx, table_idx) => {
                let table = stack_table_type(*table_idx, table_types)?;
                validate_indirect_call_table(
                    table,
                    "call_indirect",
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                pop_expect(&mut stack, table_index_type(table))?;
                let ft = &types[*type_idx as usize];
                pop_params_typed(
                    &mut stack,
                    &ft.params,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                stack.extend(ft.results.iter().copied());
            }
            Instr::CallRef(type_idx) => {
                let ft = &types[*type_idx as usize];
                let callee = pop_ref(&mut stack, "call_ref")?;
                if !types_compatible_in_module(
                    callee,
                    ValType::TypeRef(*type_idx),
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                ) {
                    return Err(format!(
                        "call_ref callee type mismatch: expected {:?}, got {:?}",
                        ValType::TypeRef(*type_idx),
                        callee
                    ));
                }
                pop_params_typed(
                    &mut stack,
                    &ft.params,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                stack.extend(ft.results.iter().copied());
            }
            Instr::Return => {
                pop_results(&mut stack, results)?;
                returned = true;
            }
            Instr::Load(op, memarg) => {
                validate_memory_memarg(*op, *memarg, memories)?;
                pop_expect(&mut stack, memory_index_ty)?;
                stack.push(load_result_type(*op)?);
            }
            Instr::Store(op, memarg) => {
                validate_memory_memarg(*op, *memarg, memories)?;
                pop_expect(&mut stack, store_value_type(*op)?)?;
                pop_expect(&mut stack, memory_index_ty)?;
            }
            Instr::AtomicLoad(sub, memarg) => {
                validate_atomic_memarg(memory_shared, *sub, *memarg, memories)?;
                pop_expect(&mut stack, ValType::I32)?;
                stack.push(atomic_load_result_type(*sub)?);
            }
            Instr::AtomicStore(sub, memarg) => {
                validate_atomic_memarg(memory_shared, *sub, *memarg, memories)?;
                pop_expect(&mut stack, atomic_store_value_type(*sub)?)?;
                pop_expect(&mut stack, memory_index_ty)?;
            }
            Instr::AtomicRmw(sub, memarg) => {
                validate_atomic_memarg(memory_shared, *sub, *memarg, memories)?;
                if atomic_rmw_is_cmpxchg(*sub) {
                    pop_expect(&mut stack, atomic_rmw_value_type(*sub)?)?;
                }
                pop_expect(&mut stack, atomic_rmw_value_type(*sub)?)?;
                pop_expect(&mut stack, ValType::I32)?;
                stack.push(atomic_rmw_result_type(*sub)?);
            }
            Instr::AtomicNotify(memarg) => {
                validate_atomic_memarg(memory_shared, 0x00, *memarg, memories)?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_expect(&mut stack, ValType::I32)?;
                stack.push(ValType::I32);
            }
            Instr::AtomicWait(sub, memarg) => {
                validate_atomic_memarg(memory_shared, *sub, *memarg, memories)?;
                pop_expect(&mut stack, ValType::I64)?;
                pop_expect(
                    &mut stack,
                    if *sub == 0x01 {
                        ValType::I32
                    } else {
                        ValType::I64
                    },
                )?;
                pop_expect(&mut stack, ValType::I32)?;
                stack.push(ValType::I32);
            }
            Instr::AtomicFence(reserved) => validate_atomic_fence(memory_shared, *reserved)?,
            Instr::MemorySize(_) => stack.push(memory_index_ty),
            Instr::MemoryGrow(_) => {
                pop_expect(&mut stack, memory_index_ty)?;
                stack.push(memory_index_ty);
            }
            Instr::Num(op) => apply_numeric_stack(*op, &mut stack)?,
            Instr::TruncSat(sub) => apply_trunc_sat_stack(*sub, &mut stack)?,
            Instr::MemoryInit(_, _) => {
                pop_expect(&mut stack, ValType::I32)?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_expect(&mut stack, memory_index_ty)?;
            }
            Instr::DataDrop(_) => {}
            Instr::MemoryCopy(_, _) => {
                pop_expect(&mut stack, memory_index_ty)?;
                pop_expect(&mut stack, memory_index_ty)?;
                pop_expect(&mut stack, memory_index_ty)?;
            }
            Instr::MemoryFill(_) => {
                pop_expect(&mut stack, memory_index_ty)?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_expect(&mut stack, memory_index_ty)?;
            }
            Instr::TableGrow(tableidx) => {
                let table = stack_table_type(*tableidx, table_types)?;
                let elem_ty = table.elem;
                let index_ty = table_index_type(table);
                pop_expect(&mut stack, index_ty)?;
                pop_expect_typed(
                    &mut stack,
                    elem_ty,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                stack.push(index_ty);
            }
            Instr::TableSize(tableidx) => {
                let table = stack_table_type(*tableidx, table_types)?;
                stack.push(table_index_type(table));
            }
            Instr::TableFill(tableidx) => {
                let table = stack_table_type(*tableidx, table_types)?;
                let elem_ty = table.elem;
                let index_ty = table_index_type(table);
                pop_expect(&mut stack, index_ty)?;
                pop_expect_typed(
                    &mut stack,
                    elem_ty,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                pop_expect(&mut stack, index_ty)?;
            }
            Instr::TableInit(_, tableidx) => {
                let table = stack_table_type(*tableidx, table_types)?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_expect(&mut stack, table_index_type(table))?;
            }
            Instr::ElemDrop(_) => {}
            Instr::TableCopy(dst, src) => {
                let dst_table = stack_table_type(*dst, table_types)?;
                let src_table = stack_table_type(*src, table_types)?;
                pop_expect(&mut stack, table_copy_len_type(dst_table, src_table))?;
                pop_expect(&mut stack, table_index_type(src_table))?;
                pop_expect(&mut stack, table_index_type(dst_table))?;
            }
            Instr::Unreachable
            | Instr::Block(_)
            | Instr::Loop(_)
            | Instr::If(_)
            | Instr::LegacyTry(_)
            | Instr::LegacyCatch(_)
            | Instr::LegacyCatchAll
            | Instr::LegacyRethrow(_)
            | Instr::LegacyDelegate(_)
            | Instr::Else
            | Instr::Br(_)
            | Instr::BrIf(_)
            | Instr::BrTable(_, _) => {}
        }
    }
    if !returned {
        pop_results(&mut stack, results)?;
        if !stack.is_empty() {
            return Err(format!(
                "function body leaves extra stack values: {:?}",
                stack
            ));
        }
    }
    Ok(())
}

struct ControlFrame {
    height: usize,
    params: Vec<ValType>,
    branch_types: Vec<ValType>,
    end_types: Vec<ValType>,
    local_initialized_at_entry: Vec<bool>,
    is_if: bool,
    is_try: bool,
    else_seen: bool,
    unreachable: bool,
    parent_unreachable: bool,
    branch_reached: bool,
}

fn validate_control_flow_stack(
    body: &[Instr],
    locals: &[ValType],
    globals: &[ValType],
    global_mutability: &[bool],
    funcs: &[u32],
    types: &[FuncType],
    table_types: &[TableType],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    type_supertypes: &[Vec<u32>],
    elements: &[ElementSegment],
    tags: &[Tag],
    memories: &[Limits],
    memory_index_ty: ValType,
    memory_shared: bool,
    func_results: &[ValType],
    param_count: usize,
) -> Result<(), String> {
    let mut stack = Vec::new();
    let mut local_initialized = initial_local_initialized(locals, param_count);
    let mut labels = vec![ControlFrame {
        height: 0,
        params: Vec::new(),
        branch_types: func_results.to_vec(),
        end_types: func_results.to_vec(),
        local_initialized_at_entry: local_initialized.clone(),
        is_if: false,
        is_try: false,
        else_seen: false,
        unreachable: false,
        parent_unreachable: false,
        branch_reached: false,
    }];

    for ins in body {
        match ins {
            Instr::End if labels.len() == 1 => {
                if !labels[0].unreachable || stack.len() > labels[0].height {
                    pop_results_typed(
                        &mut stack,
                        func_results,
                        type_supertypes,
                        array_types,
                        struct_types,
                        types,
                    )?;
                    ensure_stack_height(&stack, 0)?;
                }
                return Ok(());
            }
            Instr::End => {
                let frame = labels.pop().ok_or("end without label")?;
                if frame.is_if
                    && !frame.else_seen
                    && !frame.end_types.is_empty()
                    && !frame.parent_unreachable
                    && !frame.unreachable
                    && frame.params != frame.end_types
                {
                    return Err("if with result types requires else".to_string());
                }
                let produced_concrete_results = stack.len() > frame.height;
                let structurally_unreachable_if =
                    frame.parent_unreachable && frame.is_if && !frame.else_seen;
                let effective_unreachable = frame.unreachable || structurally_unreachable_if;
                if produced_concrete_results || !effective_unreachable {
                    pop_results_typed(
                        &mut stack,
                        &frame.end_types,
                        type_supertypes,
                        array_types,
                        struct_types,
                        types,
                    )?;
                    ensure_stack_height(&stack, frame.height)?;
                }
                stack.truncate(frame.height);
                if !effective_unreachable || produced_concrete_results || frame.branch_reached {
                    stack.extend(frame.end_types.iter().copied());
                }
                if effective_unreachable
                    && !produced_concrete_results
                    && !frame.end_types.is_empty()
                    && !frame.branch_reached
                {
                    if let Some(parent) = labels.last_mut() {
                        parent.unreachable = true;
                    }
                }
                local_initialized = frame.local_initialized_at_entry;
            }
            Instr::Block(bt) => {
                let parent_unreachable = labels.last().map(|f| f.unreachable).unwrap_or(false);
                let (params, results) = block_type_signature(*bt, types)?;
                pop_params_control(&mut stack, &labels, &params)?;
                let height = stack.len();
                if !parent_unreachable {
                    stack.extend(params.iter().copied());
                }
                labels.push(ControlFrame {
                    height,
                    params,
                    branch_types: results.clone(),
                    end_types: results,
                    local_initialized_at_entry: local_initialized.clone(),
                    is_if: false,
                    is_try: false,
                    else_seen: false,
                    unreachable: false,
                    parent_unreachable,
                    branch_reached: false,
                });
            }
            Instr::Loop(bt) => {
                let parent_unreachable = labels.last().map(|f| f.unreachable).unwrap_or(false);
                let (params, results) = block_type_signature(*bt, types)?;
                pop_params_control(&mut stack, &labels, &params)?;
                let height = stack.len();
                if !parent_unreachable {
                    stack.extend(params.iter().copied());
                }
                labels.push(ControlFrame {
                    height,
                    params: params.clone(),
                    branch_types: params,
                    end_types: results,
                    local_initialized_at_entry: local_initialized.clone(),
                    is_if: false,
                    is_try: false,
                    else_seen: false,
                    unreachable: false,
                    parent_unreachable,
                    branch_reached: false,
                });
            }
            Instr::If(bt) => {
                let parent_unreachable = labels.last().map(|f| f.unreachable).unwrap_or(false);
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                let (params, results) = block_type_signature(*bt, types)?;
                pop_params_control(&mut stack, &labels, &params)?;
                let height = stack.len();
                if !parent_unreachable {
                    stack.extend(params.iter().copied());
                }
                labels.push(ControlFrame {
                    height,
                    params,
                    branch_types: results.clone(),
                    end_types: results,
                    local_initialized_at_entry: local_initialized.clone(),
                    is_if: true,
                    is_try: false,
                    else_seen: false,
                    unreachable: false,
                    parent_unreachable,
                    branch_reached: false,
                });
            }
            Instr::TryTable(bt, handlers) => {
                let parent_unreachable = labels.last().map(|f| f.unreachable).unwrap_or(false);
                let (params, results) = block_type_signature(*bt, types)?;
                for handler in handlers {
                    let (label, branch_values) = match handler {
                        CatchKind::Catch { tag, label } => {
                            let tag_idx = *tag;
                            let tag = tags.get(tag_idx as usize).ok_or_else(|| {
                                format!("try_table catch tag index {} out of bounds", tag_idx)
                            })?;
                            let ft = types.get(tag.type_idx as usize).ok_or_else(|| {
                                format!(
                                    "try_table catch tag type index {} out of bounds",
                                    tag.type_idx
                                )
                            })?;
                            (*label, ft.params.clone())
                        }
                        CatchKind::CatchRef { tag, label } => {
                            let tag_idx = *tag;
                            let tag = tags.get(tag_idx as usize).ok_or_else(|| {
                                format!("try_table catch tag index {} out of bounds", tag_idx)
                            })?;
                            let ft = types.get(tag.type_idx as usize).ok_or_else(|| {
                                format!(
                                    "try_table catch tag type index {} out of bounds",
                                    tag.type_idx
                                )
                            })?;
                            let mut values = ft.params.clone();
                            values.push(ValType::Unknown);
                            (*label, values)
                        }
                        CatchKind::CatchAll { label } => (*label, Vec::new()),
                        CatchKind::CatchAllRef { label } => (*label, vec![ValType::Unknown]),
                    };
                    let target_idx = branch_target_index(&labels, label)?;
                    let target_types = labels[target_idx].branch_types.clone();
                    if branch_values.len() != target_types.len() {
                        return Err(format!(
                            "try_table handler branch arity {} does not match target arity {}",
                            branch_values.len(),
                            target_types.len()
                        ));
                    }
                    for (got, expected) in branch_values.iter().zip(target_types.iter()) {
                        if !types_compatible_in_module(
                            *got,
                            *expected,
                            type_supertypes,
                            array_types,
                            struct_types,
                            types,
                        ) {
                            return Err(format!(
                                "try_table handler branch type {:?} does not match target {:?}",
                                got, expected
                            ));
                        }
                    }
                    if let Some(target) = labels.get_mut(target_idx) {
                        target.branch_reached = true;
                    }
                }
                pop_params_control(&mut stack, &labels, &params)?;
                let height = stack.len();
                if !parent_unreachable {
                    stack.extend(params.iter().copied());
                }
                labels.push(ControlFrame {
                    height,
                    params,
                    branch_types: results.clone(),
                    end_types: results,
                    local_initialized_at_entry: local_initialized.clone(),
                    is_if: false,
                    is_try: false,
                    else_seen: false,
                    unreachable: false,
                    parent_unreachable,
                    branch_reached: false,
                });
            }
            Instr::LegacyTry(bt) => {
                let parent_unreachable = labels.last().map(|f| f.unreachable).unwrap_or(false);
                let (params, results) = block_type_signature(*bt, types)?;
                pop_params_control(&mut stack, &labels, &params)?;
                let height = stack.len();
                if !parent_unreachable {
                    stack.extend(params.iter().copied());
                }
                labels.push(ControlFrame {
                    height,
                    params,
                    branch_types: results.clone(),
                    end_types: results,
                    local_initialized_at_entry: local_initialized.clone(),
                    is_if: false,
                    is_try: true,
                    else_seen: false,
                    unreachable: false,
                    parent_unreachable,
                    branch_reached: false,
                });
            }
            Instr::Else => {
                let frame = labels.last_mut().ok_or("else without label")?;
                if !frame.is_if {
                    return Err("else outside if".to_string());
                }
                if frame.else_seen {
                    return Err("duplicate else".to_string());
                }
                if !frame.unreachable {
                    pop_results_typed(
                        &mut stack,
                        &frame.end_types,
                        type_supertypes,
                        array_types,
                        struct_types,
                        types,
                    )?;
                    ensure_stack_height(&stack, frame.height)?;
                }
                stack.truncate(frame.height);
                stack.extend(frame.params.iter().copied());
                local_initialized = frame.local_initialized_at_entry.clone();
                frame.else_seen = true;
                frame.unreachable = false;
            }
            Instr::LegacyCatch(tag_idx) => {
                let frame = labels.last_mut().ok_or("catch without label")?;
                if !frame.is_try {
                    return Err("catch outside try".to_string());
                }
                if !frame.unreachable {
                    pop_results_typed(
                        &mut stack,
                        &frame.end_types,
                        type_supertypes,
                        array_types,
                        struct_types,
                        types,
                    )?;
                    ensure_stack_height(&stack, frame.height)?;
                }
                stack.truncate(frame.height);
                local_initialized = frame.local_initialized_at_entry.clone();
                frame.else_seen = true;
                frame.unreachable = false;
                let tag = tags
                    .get(*tag_idx as usize)
                    .ok_or_else(|| format!("legacy catch tag index {} out of bounds", tag_idx))?;
                let ft = types.get(tag.type_idx as usize).ok_or_else(|| {
                    format!("legacy catch tag type index {} out of bounds", tag.type_idx)
                })?;
                stack.extend(ft.params.iter().copied());
            }
            Instr::LegacyCatchAll => {
                let frame = labels.last_mut().ok_or("catch_all without label")?;
                if !frame.is_try {
                    return Err("catch_all outside try".to_string());
                }
                if !frame.unreachable {
                    pop_results_typed(
                        &mut stack,
                        &frame.end_types,
                        type_supertypes,
                        array_types,
                        struct_types,
                        types,
                    )?;
                    ensure_stack_height(&stack, frame.height)?;
                }
                stack.truncate(frame.height);
                local_initialized = frame.local_initialized_at_entry.clone();
                frame.else_seen = true;
                frame.unreachable = false;
            }
            Instr::LegacyDelegate(depth) => {
                branch_target_index(&labels, *depth)?;
                let frame = labels.pop().ok_or("delegate without label")?;
                if !frame.is_try {
                    return Err("delegate outside try".to_string());
                }
                if !frame.unreachable {
                    pop_results_typed(
                        &mut stack,
                        &frame.end_types,
                        type_supertypes,
                        array_types,
                        struct_types,
                        types,
                    )?;
                    ensure_stack_height(&stack, frame.height)?;
                }
                stack.truncate(frame.height);
                if !frame.unreachable || frame.branch_reached {
                    stack.extend(frame.end_types.iter().copied());
                }
                local_initialized = frame.local_initialized_at_entry;
            }
            Instr::Br(depth) => {
                let target_idx = branch_target_index(&labels, *depth)?;
                let (target_height, branch_types) = {
                    let target = &labels[target_idx];
                    (target.height, target.branch_types.clone())
                };
                pop_branch_results_control_typed(
                    &mut stack,
                    &labels,
                    target_height,
                    &branch_types,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                if let Some(target) = labels.get_mut(target_idx) {
                    target.branch_reached = true;
                }
                if let Some(frame) = labels.last_mut() {
                    stack.truncate(frame.height);
                    frame.unreachable = true;
                }
            }
            Instr::BrIf(depth) => {
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                let current_height = labels
                    .last()
                    .map(|frame| frame.height)
                    .ok_or("br_if without current label")?;
                let (_target_height, branch_types) = {
                    let target = branch_target(&labels, *depth)?;
                    (target.height, target.branch_types.clone())
                };
                pop_br_if_branch_results_control_typed(
                    &mut stack,
                    &labels,
                    current_height,
                    &branch_types,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                stack.extend(branch_types.iter().copied());
            }
            Instr::BrOnNull(depth) => {
                let ty = pop_ref_control(&mut stack, &labels, "br_on_null")?;
                let target_idx = branch_target_index(&labels, *depth)?;
                if let Some(target) = labels.get_mut(target_idx) {
                    target.branch_reached = true;
                }
                stack.push(ty);
            }
            Instr::BrOnNonNull(depth) => {
                let ty = pop_ref_control(&mut stack, &labels, "br_on_non_null")?;
                let branch_ref_ty = non_null_heap_type(ty);
                let target_idx = branch_target_index(&labels, *depth)?;
                let branch_types = labels[target_idx].branch_types.clone();
                let current_height = labels
                    .last()
                    .map(|frame| frame.height)
                    .ok_or("br_on_non_null without current label")?;
                let (prefix_types, expected_ref_ty) = branch_types
                    .split_last()
                    .map(|(last, prefix)| (prefix, *last))
                    .ok_or_else(|| "br_on_non_null target has no reference result".to_string())?;
                if !types_compatible_in_module(
                    branch_ref_ty,
                    expected_ref_ty,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                ) {
                    return Err(format!(
                        "br_on_non_null target type mismatch: branch expects {:?}, got {:?}",
                        expected_ref_ty, branch_ref_ty
                    ));
                }
                pop_br_if_branch_results_control_typed(
                    &mut stack,
                    &labels,
                    current_height,
                    prefix_types,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                stack.extend(prefix_types.iter().copied());
                if let Some(target) = labels.get_mut(target_idx) {
                    target.branch_reached = true;
                }
            }
            Instr::BrOnCast {
                depth,
                source,
                target,
                ..
            } => {
                let operand = pop_ref_control(&mut stack, &labels, "br_on_cast")?;
                let branch_payload_ty = nullable_cast_complement_type(*target, *target);
                let fallthrough_ty = nullable_cast_complement_type(*source, *target);
                if !types_compatible_in_module(
                    *target,
                    *source,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                ) {
                    return Err(format!(
                        "br_on_cast target type {:?} is not a subtype of source {:?}",
                        target, source
                    ));
                }
                let target_idx = branch_target_index(&labels, *depth)?;
                let branch_types = labels[target_idx].branch_types.clone();
                if branch_types.len() == 1
                    && !types_compatible_in_module(
                        branch_payload_ty,
                        branch_types[0],
                        type_supertypes,
                        array_types,
                        struct_types,
                        types,
                    )
                {
                    return Err(format!(
                        "br_on_cast target type mismatch: branch expects {:?}, got {:?}",
                        branch_types[0], target
                    ));
                }
                if let Some(target_frame) = labels.get_mut(target_idx) {
                    target_frame.branch_reached = true;
                }
                stack.push(if operand == ValType::Unknown {
                    ValType::Unknown
                } else {
                    fallthrough_ty
                });
            }
            Instr::BrOnCastFail {
                depth,
                source,
                target,
                ..
            } => {
                let operand = pop_ref_control(&mut stack, &labels, "br_on_cast_fail")?;
                let branch_payload_ty = nullable_cast_complement_type(*source, *target);
                let fallthrough_ty = nullable_cast_complement_type(*target, *target);
                if !types_compatible_in_module(
                    *target,
                    *source,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                ) {
                    return Err(format!(
                        "br_on_cast_fail target type {:?} is not a subtype of source {:?}",
                        target, source
                    ));
                }
                let target_idx = branch_target_index(&labels, *depth)?;
                let branch_types = labels[target_idx].branch_types.clone();
                if branch_types.len() == 1
                    && !types_compatible_in_module(
                        if operand == ValType::Unknown {
                            ValType::Unknown
                        } else {
                            branch_payload_ty
                        },
                        branch_types[0],
                        type_supertypes,
                        array_types,
                        struct_types,
                        types,
                    )
                {
                    return Err(format!(
                        "br_on_cast_fail branch type mismatch: branch expects {:?}, got {:?}",
                        branch_types[0], operand
                    ));
                }
                if let Some(target_frame) = labels.get_mut(target_idx) {
                    target_frame.branch_reached = true;
                }
                stack.push(if operand == ValType::Unknown {
                    ValType::Unknown
                } else {
                    fallthrough_ty
                });
            }
            Instr::Return => {
                pop_return_results_control_typed(
                    &mut stack,
                    &labels,
                    func_results,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                if let Some(frame) = labels.last_mut() {
                    stack.truncate(frame.height);
                    frame.unreachable = true;
                }
            }
            Instr::Throw(tag_idx) => {
                let tag = tags
                    .get(*tag_idx as usize)
                    .ok_or_else(|| format!("throw tag index {} out of bounds", tag_idx))?;
                let ft = types.get(tag.type_idx as usize).ok_or_else(|| {
                    format!("throw tag type index {} out of bounds", tag.type_idx)
                })?;
                pop_params_control_typed(
                    &mut stack,
                    &labels,
                    &ft.params,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                if let Some(frame) = labels.last_mut() {
                    stack.truncate(frame.height);
                    frame.unreachable = true;
                }
            }
            Instr::ThrowRef => {
                let callee = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(callee) {
                    return Err(format!(
                        "throw_ref operand must be reference, got {:?}",
                        callee
                    ));
                }
                if let Some(frame) = labels.last_mut() {
                    stack.truncate(frame.height);
                    frame.unreachable = true;
                }
            }
            Instr::LegacyRethrow(depth) => {
                branch_target_index(&labels, *depth)?;
                if let Some(frame) = labels.last_mut() {
                    stack.truncate(frame.height);
                    frame.unreachable = true;
                }
            }
            Instr::I32Const(_) => stack.push(ValType::I32),
            Instr::I64Const(_) => stack.push(ValType::I64),
            Instr::F32Const(_) => stack.push(ValType::F32),
            Instr::F64Const(_) => stack.push(ValType::F64),
            Instr::V128Const(_) => stack.push(ValType::V128),
            Instr::I8x16Shuffle(lanes) => {
                for lane in lanes {
                    validate_lane(*lane, 32, "i8x16.shuffle")?;
                }
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                stack.push(ValType::V128);
            }
            Instr::I8x16Swizzle => {
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                stack.push(ValType::V128);
            }
            Instr::I8x16Splat | Instr::I16x8Splat | Instr::I32x4Splat => {
                pop_expect_control(&mut stack, &labels, memory_index_ty)?;
                stack.push(ValType::V128);
            }
            Instr::I64x2Splat => {
                pop_expect(&mut stack, ValType::I64)?;
                stack.push(ValType::V128);
            }
            Instr::F32x4Splat => {
                pop_expect(&mut stack, ValType::F32)?;
                stack.push(ValType::V128);
            }
            Instr::F64x2Splat => {
                pop_expect(&mut stack, ValType::F64)?;
                stack.push(ValType::V128);
            }
            Instr::I8x16ExtractLaneS(lane) | Instr::I8x16ExtractLaneU(lane) => {
                validate_lane(*lane, 16, "i8x16.extract_lane")?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                stack.push(ValType::I32);
            }
            Instr::I16x8ExtractLaneS(lane) | Instr::I16x8ExtractLaneU(lane) => {
                validate_lane(*lane, 8, "i16x8.extract_lane")?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                stack.push(ValType::I32);
            }
            Instr::I32x4ExtractLane(lane) => {
                validate_lane(*lane, 4, "i32x4.extract_lane")?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                stack.push(ValType::I32);
            }
            Instr::I64x2ExtractLane(lane) => {
                validate_lane(*lane, 2, "i64x2.extract_lane")?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                stack.push(ValType::I64);
            }
            Instr::F32x4ExtractLane(lane) => {
                validate_lane(*lane, 4, "f32x4.extract_lane")?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                stack.push(ValType::F32);
            }
            Instr::F64x2ExtractLane(lane) => {
                validate_lane(*lane, 2, "f64x2.extract_lane")?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                stack.push(ValType::F64);
            }
            Instr::I8x16ReplaceLane(lane) => {
                validate_replace_lane(&mut stack, *lane, 16, ValType::I32, "i8x16.replace_lane")?;
            }
            Instr::I16x8ReplaceLane(lane) => {
                validate_replace_lane(&mut stack, *lane, 8, ValType::I32, "i16x8.replace_lane")?;
            }
            Instr::I32x4ReplaceLane(lane) => {
                validate_replace_lane(&mut stack, *lane, 4, ValType::I32, "i32x4.replace_lane")?;
            }
            Instr::I64x2ReplaceLane(lane) => {
                validate_replace_lane(&mut stack, *lane, 2, ValType::I64, "i64x2.replace_lane")?;
            }
            Instr::F32x4ReplaceLane(lane) => {
                validate_replace_lane(&mut stack, *lane, 4, ValType::F32, "f32x4.replace_lane")?;
            }
            Instr::F64x2ReplaceLane(lane) => {
                validate_replace_lane(&mut stack, *lane, 2, ValType::F64, "f64x2.replace_lane")?;
            }
            Instr::I8x16Eq
            | Instr::I8x16Ne
            | Instr::I8x16LtS
            | Instr::I8x16LtU
            | Instr::I8x16GtS
            | Instr::I8x16GtU
            | Instr::I8x16LeS
            | Instr::I8x16LeU
            | Instr::I8x16GeS
            | Instr::I8x16GeU
            | Instr::I16x8Eq
            | Instr::I16x8Ne
            | Instr::I16x8LtS
            | Instr::I16x8LtU
            | Instr::I16x8GtS
            | Instr::I16x8GtU
            | Instr::I16x8LeS
            | Instr::I16x8LeU
            | Instr::I16x8GeS
            | Instr::I16x8GeU
            | Instr::I32x4Eq
            | Instr::I32x4Ne
            | Instr::I32x4LtS
            | Instr::I32x4LtU
            | Instr::I32x4GtS
            | Instr::I32x4GtU
            | Instr::I32x4LeS
            | Instr::I32x4LeU
            | Instr::I32x4GeS
            | Instr::I32x4GeU
            | Instr::I64x2Eq
            | Instr::I64x2Ne
            | Instr::I64x2LtS
            | Instr::I64x2GtS
            | Instr::I64x2LeS
            | Instr::I64x2GeS
            | Instr::F32x4Ne
            | Instr::F32x4Lt
            | Instr::F32x4Gt
            | Instr::F32x4Le
            | Instr::F32x4Ge
            | Instr::F64x2Ne
            | Instr::F64x2Lt
            | Instr::F64x2Gt
            | Instr::F64x2Le
            | Instr::F64x2Ge
            | Instr::I8x16NarrowI16x8S
            | Instr::I8x16NarrowI16x8U
            | Instr::I16x8NarrowI32x4S
            | Instr::I16x8NarrowI32x4U
            | Instr::I16x8ExtMulLowI8x16S
            | Instr::I16x8ExtMulHighI8x16S
            | Instr::I16x8ExtMulLowI8x16U
            | Instr::I16x8ExtMulHighI8x16U
            | Instr::I32x4DotI16x8S
            | Instr::I32x4ExtMulLowI16x8S
            | Instr::I32x4ExtMulHighI16x8S
            | Instr::I32x4ExtMulLowI16x8U
            | Instr::I32x4ExtMulHighI16x8U
            | Instr::I64x2ExtMulLowI32x4S
            | Instr::I64x2ExtMulHighI32x4S
            | Instr::I64x2ExtMulLowI32x4U
            | Instr::I64x2ExtMulHighI32x4U
            | Instr::I8x16Add
            | Instr::I8x16AddSatS
            | Instr::I8x16AddSatU
            | Instr::I8x16Sub
            | Instr::I8x16SubSatS
            | Instr::I8x16SubSatU
            | Instr::I8x16MinS
            | Instr::I8x16MinU
            | Instr::I8x16MaxS
            | Instr::I8x16MaxU
            | Instr::I8x16AvgrU
            | Instr::I16x8Add
            | Instr::I16x8AddSatS
            | Instr::I16x8AddSatU
            | Instr::I16x8Sub
            | Instr::I16x8SubSatS
            | Instr::I16x8SubSatU
            | Instr::I16x8Mul
            | Instr::I16x8MinS
            | Instr::I16x8MinU
            | Instr::I16x8MaxS
            | Instr::I16x8MaxU
            | Instr::I16x8AvgrU
            | Instr::I16x8Q15mulrSatS
            | Instr::I16x8RelaxedQ15mulrS
            | Instr::I16x8RelaxedDotI8x16I7x16S
            | Instr::I32x4Add
            | Instr::I32x4Sub
            | Instr::I32x4Mul
            | Instr::I32x4MinS
            | Instr::I32x4MinU
            | Instr::I32x4MaxS
            | Instr::I32x4MaxU
            | Instr::I64x2Add
            | Instr::I64x2Sub
            | Instr::I64x2Mul
            | Instr::F32x4Eq
            | Instr::F64x2Eq
            | Instr::F32x4Add
            | Instr::F32x4Sub
            | Instr::F32x4Mul
            | Instr::F32x4Div
            | Instr::F32x4Min
            | Instr::F32x4Max
            | Instr::F32x4PMin
            | Instr::F32x4PMax
            | Instr::F64x2Add
            | Instr::F64x2Sub
            | Instr::F64x2Mul
            | Instr::F64x2Div
            | Instr::F64x2Min
            | Instr::F64x2Max
            | Instr::F64x2PMin
            | Instr::F64x2PMax
            | Instr::V128And
            | Instr::V128AndNot
            | Instr::V128Or
            | Instr::V128Xor => {
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                stack.push(ValType::V128);
            }
            Instr::I8x16Shl
            | Instr::I8x16ShrS
            | Instr::I8x16ShrU
            | Instr::I16x8Shl
            | Instr::I16x8ShrS
            | Instr::I16x8ShrU
            | Instr::I32x4Shl
            | Instr::I32x4ShrS
            | Instr::I32x4ShrU
            | Instr::I64x2Shl
            | Instr::I64x2ShrS
            | Instr::I64x2ShrU => {
                pop_expect(&mut stack, ValType::I32)?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                stack.push(ValType::V128);
            }
            Instr::V128BitSelect => {
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                stack.push(ValType::V128);
            }
            Instr::I32x4RelaxedDotI8x16I7x16AddS => {
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                stack.push(ValType::V128);
            }
            Instr::F32x4RelaxedMadd
            | Instr::F32x4RelaxedNmadd
            | Instr::F64x2RelaxedMadd
            | Instr::F64x2RelaxedNmadd => {
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                stack.push(ValType::V128);
            }
            Instr::I16x8ExtAddPairwiseI8x16S
            | Instr::I16x8ExtAddPairwiseI8x16U
            | Instr::I32x4ExtAddPairwiseI16x8S
            | Instr::I32x4ExtAddPairwiseI16x8U
            | Instr::I8x16Abs
            | Instr::I8x16Neg
            | Instr::I8x16Popcnt
            | Instr::I16x8Abs
            | Instr::I16x8Neg
            | Instr::I32x4Abs
            | Instr::I32x4Neg
            | Instr::I64x2Abs
            | Instr::I64x2Neg
            | Instr::F32x4Abs
            | Instr::F32x4Neg
            | Instr::F32x4Ceil
            | Instr::F32x4Floor
            | Instr::F32x4Trunc
            | Instr::F32x4Nearest
            | Instr::F64x2Abs
            | Instr::F64x2Neg
            | Instr::F64x2Ceil
            | Instr::F64x2Floor
            | Instr::F64x2Trunc
            | Instr::F64x2Nearest
            | Instr::I16x8ExtendLowI8x16S
            | Instr::I16x8ExtendHighI8x16S
            | Instr::I16x8ExtendLowI8x16U
            | Instr::I16x8ExtendHighI8x16U
            | Instr::I32x4ExtendLowI16x8S
            | Instr::I32x4ExtendHighI16x8S
            | Instr::I32x4ExtendLowI16x8U
            | Instr::I32x4ExtendHighI16x8U
            | Instr::I64x2ExtendLowI32x4S
            | Instr::I64x2ExtendHighI32x4S
            | Instr::I64x2ExtendLowI32x4U
            | Instr::I64x2ExtendHighI32x4U
            | Instr::I32x4TruncSatF32x4S
            | Instr::I32x4TruncSatF32x4U
            | Instr::I32x4TruncSatF64x2SZero
            | Instr::I32x4TruncSatF64x2UZero
            | Instr::F32x4ConvertI32x4S
            | Instr::F32x4ConvertI32x4U
            | Instr::F32x4DemoteF64x2Zero
            | Instr::F64x2ConvertLowI32x4S
            | Instr::F64x2ConvertLowI32x4U
            | Instr::F64x2PromoteLowF32x4
            | Instr::F32x4Sqrt
            | Instr::F64x2Sqrt
            | Instr::V128Not => {
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                stack.push(ValType::V128);
            }
            Instr::V128AnyTrue => {
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                stack.push(ValType::I32);
            }
            Instr::I8x16AllTrue
            | Instr::I8x16Bitmask
            | Instr::I16x8AllTrue
            | Instr::I16x8Bitmask
            | Instr::I32x4AllTrue
            | Instr::I32x4Bitmask
            | Instr::I64x2AllTrue
            | Instr::I64x2Bitmask => {
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                stack.push(ValType::I32);
            }
            Instr::V128Load(_)
            | Instr::V128Load8Splat(_)
            | Instr::V128Load16Splat(_)
            | Instr::V128Load32Splat(_)
            | Instr::V128Load64Splat(_)
            | Instr::V128Load8x8S(_)
            | Instr::V128Load8x8U(_)
            | Instr::V128Load16x4S(_)
            | Instr::V128Load16x4U(_)
            | Instr::V128Load32x2S(_)
            | Instr::V128Load32x2U(_)
            | Instr::V128Load32Zero(_)
            | Instr::V128Load64Zero(_) => {
                validate_simd_memarg(ins, memories)?;
                pop_expect_control(&mut stack, &labels, memory_index_ty)?;
                stack.push(ValType::V128);
            }
            Instr::V128Store(_) => {
                validate_simd_memarg(ins, memories)?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                pop_expect_control(&mut stack, &labels, memory_index_ty)?;
            }
            Instr::V128Load8Lane(_, lane) => {
                validate_simd_memarg(ins, memories)?;
                validate_lane(*lane, 16, "v128.load8_lane")?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                pop_expect_control(&mut stack, &labels, memory_index_ty)?;
                stack.push(ValType::V128);
            }
            Instr::V128Load16Lane(_, lane) => {
                validate_simd_memarg(ins, memories)?;
                validate_lane(*lane, 8, "v128.load16_lane")?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                pop_expect_control(&mut stack, &labels, memory_index_ty)?;
                stack.push(ValType::V128);
            }
            Instr::V128Load32Lane(_, lane) => {
                validate_simd_memarg(ins, memories)?;
                validate_lane(*lane, 4, "v128.load32_lane")?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                pop_expect_control(&mut stack, &labels, memory_index_ty)?;
                stack.push(ValType::V128);
            }
            Instr::V128Load64Lane(_, lane) => {
                validate_simd_memarg(ins, memories)?;
                validate_lane(*lane, 2, "v128.load64_lane")?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                pop_expect_control(&mut stack, &labels, memory_index_ty)?;
                stack.push(ValType::V128);
            }
            Instr::V128Store8Lane(_, lane) => {
                validate_simd_memarg(ins, memories)?;
                validate_lane(*lane, 16, "v128.store8_lane")?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                pop_expect_control(&mut stack, &labels, memory_index_ty)?;
            }
            Instr::V128Store16Lane(_, lane) => {
                validate_simd_memarg(ins, memories)?;
                validate_lane(*lane, 8, "v128.store16_lane")?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                pop_expect_control(&mut stack, &labels, memory_index_ty)?;
            }
            Instr::V128Store32Lane(_, lane) => {
                validate_simd_memarg(ins, memories)?;
                validate_lane(*lane, 4, "v128.store32_lane")?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                pop_expect_control(&mut stack, &labels, memory_index_ty)?;
            }
            Instr::V128Store64Lane(_, lane) => {
                validate_simd_memarg(ins, memories)?;
                validate_lane(*lane, 2, "v128.store64_lane")?;
                pop_expect_control(&mut stack, &labels, ValType::V128)?;
                pop_expect_control(&mut stack, &labels, memory_index_ty)?;
            }
            Instr::RefNull(ty) => stack.push(*ty),
            Instr::RefIsNull => {
                let ty = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(ty) {
                    return Err(format!(
                        "ref.is_null operand must be reference, got {:?}",
                        ty
                    ));
                }
                stack.push(ValType::I32);
            }
            Instr::RefAsNonNull => {
                let ty = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(ty) {
                    return Err(format!(
                        "ref.as_non_null operand must be reference, got {:?}",
                        ty
                    ));
                }
                stack.push(ty);
            }
            Instr::RefTest { .. } => {
                let ty = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(ty) {
                    return Err(format!("ref.test operand must be reference, got {:?}", ty));
                }
                stack.push(ValType::I32);
            }
            Instr::RefCast { target, .. } => {
                let ty = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(ty) {
                    return Err(format!("ref.cast operand must be reference, got {:?}", ty));
                }
                stack.push(*target);
            }
            Instr::AnyConvertExtern => {
                let ty = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(ty) {
                    return Err(format!(
                        "any.convert_extern operand must be reference, got {:?}",
                        ty
                    ));
                }
                stack.push(ValType::Unknown);
            }
            Instr::ExternConvertAny => {
                let ty = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(ty) {
                    return Err(format!(
                        "extern.convert_any operand must be reference, got {:?}",
                        ty
                    ));
                }
                stack.push(ValType::ExternRef);
            }
            Instr::StructNew(type_idx) => {
                let ty = struct_type(*type_idx, struct_types)?;
                for field in ty.fields.iter().rev() {
                    pop_expect_control(&mut stack, &labels, field.ty)?;
                }
                stack.push(ValType::NonNullTypeRef(*type_idx));
            }
            Instr::StructNewDefault(type_idx) => {
                let _ty = struct_type(*type_idx, struct_types)?;
                stack.push(ValType::NonNullTypeRef(*type_idx));
            }
            Instr::StructGet(type_idx, field_idx) => {
                let ty = struct_type(*type_idx, struct_types)?;
                let field = ty
                    .fields
                    .get(*field_idx as usize)
                    .ok_or_else(|| format!("struct field index {} out of bounds", field_idx))?;
                let ref_ty = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(ref_ty) {
                    return Err(format!(
                        "struct.get operand must be reference, got {:?}",
                        ref_ty
                    ));
                }
                stack.push(field.ty);
            }
            Instr::StructGetS(type_idx, field_idx) | Instr::StructGetU(type_idx, field_idx) => {
                let ty = struct_type(*type_idx, struct_types)?;
                let field = ty
                    .fields
                    .get(*field_idx as usize)
                    .ok_or_else(|| format!("struct field index {} out of bounds", field_idx))?;
                if field.packed_bits.is_none() {
                    return Err(format!("struct.get_s/u field {} is not packed", field_idx));
                }
                let ref_ty = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(ref_ty) {
                    return Err(format!(
                        "struct.get_s/u operand must be reference, got {:?}",
                        ref_ty
                    ));
                }
                stack.push(ValType::I32);
            }
            Instr::StructSet(type_idx, field_idx) => {
                let ty = struct_type(*type_idx, struct_types)?;
                let field = ty
                    .fields
                    .get(*field_idx as usize)
                    .ok_or_else(|| format!("struct field index {} out of bounds", field_idx))?;
                if !field.mutable {
                    return Err(format!("struct.set field {} is immutable", field_idx));
                }
                pop_expect_control(&mut stack, &labels, field.ty)?;
                let ref_ty = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(ref_ty) {
                    return Err(format!(
                        "struct.set operand must be reference, got {:?}",
                        ref_ty
                    ));
                }
            }
            Instr::RefFunc(idx) => {
                let type_idx = funcs
                    .get(*idx as usize)
                    .ok_or_else(|| format!("ref.func function index {} out of bounds", idx))?;
                stack.push(ValType::NonNullTypeRef(*type_idx));
            }
            Instr::RefEq => {
                pop_expect_control_typed(
                    &mut stack,
                    &labels,
                    ValType::EqRef,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                pop_expect_control_typed(
                    &mut stack,
                    &labels,
                    ValType::EqRef,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                stack.push(ValType::I32);
            }
            Instr::ArrayNew(type_idx) => {
                let array = array_type(*type_idx, array_types)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                pop_expect_control(&mut stack, &labels, array.element)?;
                stack.push(ValType::Unknown);
            }
            Instr::ArrayNewDefault(type_idx) => {
                let _array = array_type(*type_idx, array_types)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                stack.push(ValType::Unknown);
            }
            Instr::ArrayNewFixed(type_idx, count) => {
                let array = array_type(*type_idx, array_types)?;
                for _ in 0..*count {
                    pop_expect_control(&mut stack, &labels, array.element)?;
                }
                stack.push(ValType::Unknown);
            }
            Instr::ArrayNewData(type_idx, _) => {
                let _array = array_type(*type_idx, array_types)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                stack.push(ValType::Unknown);
            }
            Instr::ArrayNewElem(type_idx, _) => {
                let _array = array_type(*type_idx, array_types)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                stack.push(ValType::Unknown);
            }
            Instr::ArrayGet(type_idx) => {
                let array = array_type(*type_idx, array_types)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                let ty = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(ty) {
                    return Err(format!("array.get operand must be reference, got {:?}", ty));
                }
                stack.push(array.element);
            }
            Instr::ArrayGetS(type_idx) | Instr::ArrayGetU(type_idx) => {
                let _array = array_type(*type_idx, array_types)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                let ty = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(ty) {
                    return Err(format!(
                        "array.get_s/u operand must be reference, got {:?}",
                        ty
                    ));
                }
                stack.push(ValType::I32);
            }
            Instr::ArraySet(type_idx) => {
                let array = array_type(*type_idx, array_types)?;
                if !array.mutable {
                    return Err(format!("array.set type {} is immutable", type_idx));
                }
                pop_expect_control(&mut stack, &labels, array.element)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                let ty = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(ty) {
                    return Err(format!("array.set operand must be reference, got {:?}", ty));
                }
            }
            Instr::ArrayLen => {
                let ty = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(ty) {
                    return Err(format!("array.len operand must be reference, got {:?}", ty));
                }
                stack.push(ValType::I32);
            }
            Instr::ArrayFill(type_idx) => {
                let array = array_type(*type_idx, array_types)?;
                if !array.mutable {
                    return Err(format!("array.fill type {} is immutable", type_idx));
                }
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                pop_expect_control(&mut stack, &labels, array.element)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                let ty = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(ty) {
                    return Err(format!(
                        "array.fill operand must be reference, got {:?}",
                        ty
                    ));
                }
            }
            Instr::ArrayCopy(dst_type_idx, src_type_idx) => {
                let dst = array_type(*dst_type_idx, array_types)?;
                let src = array_type(*src_type_idx, array_types)?;
                if !dst.mutable {
                    return Err(format!("array.copy type {} is immutable", dst_type_idx));
                }
                validate_array_copy_element_subtype(
                    *dst_type_idx,
                    dst,
                    *src_type_idx,
                    src,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                let src_ty = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(src_ty) {
                    return Err(format!(
                        "array.copy src operand must be reference, got {:?}",
                        src_ty
                    ));
                }
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                let dst_ty = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(dst_ty) {
                    return Err(format!(
                        "array.copy dst operand must be reference, got {:?}",
                        dst_ty
                    ));
                }
            }
            Instr::ArrayInitData(type_idx, _) => {
                let array = array_type(*type_idx, array_types)?;
                if !array.mutable {
                    return Err(format!("array.init_data type {} is immutable", type_idx));
                }
                validate_array_init_data_storage(*type_idx, array)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                let ty = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(ty) {
                    return Err(format!(
                        "array.init_data operand must be reference, got {:?}",
                        ty
                    ));
                }
            }
            Instr::ArrayInitElem(type_idx, elem_idx) => {
                let array = array_type(*type_idx, array_types)?;
                if !array.mutable {
                    return Err(format!("array.init_elem type {} is immutable", type_idx));
                }
                validate_array_init_elem_storage(
                    *type_idx,
                    array,
                    *elem_idx,
                    elements,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                let ty = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(ty) {
                    return Err(format!(
                        "array.init_elem operand must be reference, got {:?}",
                        ty
                    ));
                }
            }
            Instr::RefI31 => {
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                stack.push(ValType::Unknown);
            }
            Instr::I31GetS | Instr::I31GetU => {
                let ty = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(ty) {
                    return Err(format!("i31.get operand must be reference, got {:?}", ty));
                }
                stack.push(ValType::I32);
            }
            Instr::Drop => {
                pop_operand_control(&mut stack, &labels)?;
            }
            Instr::Select => {
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                let b = pop_operand_control(&mut stack, &labels)?;
                let a = pop_operand_control(&mut stack, &labels)?;
                if !types_compatible(a, b) {
                    return Err(format!("select operand type mismatch {:?} vs {:?}", a, b));
                }
                let result = if a == ValType::Unknown { b } else { a };
                if !is_untyped_select_value_type(result) {
                    return Err(format!(
                        "select without result type cannot select {:?}",
                        result
                    ));
                }
                stack.push(result);
            }
            Instr::SelectTyped(ty) => {
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                pop_expect_control_typed(
                    &mut stack,
                    &labels,
                    *ty,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                pop_expect_control_typed(
                    &mut stack,
                    &labels,
                    *ty,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                stack.push(*ty);
            }
            Instr::LocalGet(idx) => {
                ensure_local_initialized(&local_initialized, locals, *idx)?;
                stack.push(
                    *locals
                        .get(*idx as usize)
                        .ok_or_else(|| format!("local index {} out of bounds", idx))?,
                );
            }
            Instr::LocalSet(idx) => {
                let ty = *locals
                    .get(*idx as usize)
                    .ok_or_else(|| format!("local index {} out of bounds", idx))?;
                pop_expect_control_typed(
                    &mut stack,
                    &labels,
                    ty,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                if let Some(slot) = local_initialized.get_mut(*idx as usize) {
                    *slot = true;
                }
            }
            Instr::LocalTee(idx) => {
                let ty = *locals
                    .get(*idx as usize)
                    .ok_or_else(|| format!("local index {} out of bounds", idx))?;
                pop_expect_control_typed(
                    &mut stack,
                    &labels,
                    ty,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                if let Some(slot) = local_initialized.get_mut(*idx as usize) {
                    *slot = true;
                }
                stack.push(ty);
            }
            Instr::GlobalGet(idx) => stack.push(
                *globals
                    .get(*idx as usize)
                    .ok_or_else(|| format!("global index {} out of bounds", idx))?,
            ),
            Instr::GlobalSet(idx) => {
                let ty = *globals
                    .get(*idx as usize)
                    .ok_or_else(|| format!("global index {} out of bounds", idx))?;
                if !global_mutability
                    .get(*idx as usize)
                    .copied()
                    .unwrap_or(false)
                {
                    return Err(format!("global.set {} targets immutable global", idx));
                }
                pop_expect_control_typed(
                    &mut stack,
                    &labels,
                    ty,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
            }
            Instr::Call(idx) => {
                let type_idx = *funcs
                    .get(*idx as usize)
                    .ok_or_else(|| format!("call function index {} out of bounds", idx))?
                    as usize;
                let ft = types
                    .get(type_idx)
                    .ok_or_else(|| format!("call function index {} out of bounds", idx))?;
                pop_params_control_typed(
                    &mut stack,
                    &labels,
                    &ft.params,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                stack.extend(ft.results.iter().copied());
            }
            Instr::CallIndirect(type_idx, table_idx) => {
                let table = stack_table_type(*table_idx, table_types)?;
                validate_indirect_call_table(
                    table,
                    "call_indirect",
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                pop_expect_control(&mut stack, &labels, table_index_type(table))?;
                let ft = types.get(*type_idx as usize).ok_or_else(|| {
                    format!("call_indirect type index {} out of bounds", type_idx)
                })?;
                pop_params_control_typed(
                    &mut stack,
                    &labels,
                    &ft.params,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                stack.extend(ft.results.iter().copied());
            }
            Instr::ReturnCallIndirect(type_idx, table_idx) => {
                let table = stack_table_type(*table_idx, table_types)?;
                validate_indirect_call_table(
                    table,
                    "return_call_indirect",
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                pop_expect_control(&mut stack, &labels, table_index_type(table))?;
                let ft = types.get(*type_idx as usize).ok_or_else(|| {
                    format!("return_call_indirect type index {} out of bounds", type_idx)
                })?;
                pop_params_control_typed(
                    &mut stack,
                    &labels,
                    &ft.params,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                if !result_types_compatible_in_module(
                    &ft.results,
                    func_results,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                ) {
                    return Err(format!(
                        "return_call_indirect result type {:?} does not match function result {:?}",
                        ft.results, func_results
                    ));
                }
                if let Some(frame) = labels.last_mut() {
                    stack.truncate(frame.height);
                    frame.unreachable = true;
                }
            }
            Instr::CallRef(type_idx) => {
                let ft = types
                    .get(*type_idx as usize)
                    .ok_or_else(|| format!("call_ref type index {} out of bounds", type_idx))?;
                let callee = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(callee) {
                    return Err(format!(
                        "call_ref callee must be reference, got {:?}",
                        callee
                    ));
                }
                if !types_compatible_in_module(
                    callee,
                    ValType::TypeRef(*type_idx),
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                ) {
                    return Err(format!(
                        "call_ref callee type mismatch: expected {:?}, got {:?}",
                        ValType::TypeRef(*type_idx),
                        callee
                    ));
                }
                pop_params_control_typed(
                    &mut stack,
                    &labels,
                    &ft.params,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                stack.extend(ft.results.iter().copied());
            }
            Instr::ReturnCallRef(type_idx) => {
                let ft = types.get(*type_idx as usize).ok_or_else(|| {
                    format!("return_call_ref type index {} out of bounds", type_idx)
                })?;
                let callee = pop_operand_control(&mut stack, &labels)?;
                if !is_ref_type(callee) {
                    return Err(format!(
                        "return_call_ref callee must be reference, got {:?}",
                        callee
                    ));
                }
                if !types_compatible_in_module(
                    callee,
                    ValType::TypeRef(*type_idx),
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                ) {
                    return Err(format!(
                        "return_call_ref callee type mismatch: expected {:?}, got {:?}",
                        ValType::TypeRef(*type_idx),
                        callee
                    ));
                }
                pop_params_control_typed(
                    &mut stack,
                    &labels,
                    &ft.params,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                if !result_types_compatible_in_module(
                    &ft.results,
                    func_results,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                ) {
                    return Err(format!(
                        "return_call_ref result type {:?} does not match function result {:?}",
                        ft.results, func_results
                    ));
                }
                if let Some(frame) = labels.last_mut() {
                    stack.truncate(frame.height);
                    frame.unreachable = true;
                }
            }
            Instr::ReturnCall(idx) => {
                let type_idx = *funcs
                    .get(*idx as usize)
                    .ok_or_else(|| format!("return_call function index {} out of bounds", idx))?
                    as usize;
                let ft = types
                    .get(type_idx)
                    .ok_or_else(|| format!("return_call function index {} out of bounds", idx))?;
                pop_params_control_typed(
                    &mut stack,
                    &labels,
                    &ft.params,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                if !result_types_compatible_in_module(
                    &ft.results,
                    func_results,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                ) {
                    return Err(format!(
                        "return_call result type {:?} does not match function result {:?}",
                        ft.results, func_results
                    ));
                }
                if let Some(frame) = labels.last_mut() {
                    stack.truncate(frame.height);
                    frame.unreachable = true;
                }
            }
            Instr::Nop => {}
            Instr::MemorySize(_) => stack.push(memory_index_ty),
            Instr::MemoryGrow(_) => {
                pop_expect_control(&mut stack, &labels, memory_index_ty)?;
                stack.push(memory_index_ty);
            }
            Instr::Load(op, memarg) => {
                validate_memory_memarg(*op, *memarg, memories)?;
                pop_expect_control(&mut stack, &labels, memory_index_ty)?;
                stack.push(load_result_type(*op)?);
            }
            Instr::Store(op, memarg) => {
                validate_memory_memarg(*op, *memarg, memories)?;
                pop_expect_control(&mut stack, &labels, store_value_type(*op)?)?;
                pop_expect_control(&mut stack, &labels, memory_index_ty)?;
            }
            Instr::AtomicLoad(sub, memarg) => {
                validate_atomic_memarg(memory_shared, *sub, *memarg, memories)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                stack.push(atomic_load_result_type(*sub)?);
            }
            Instr::AtomicStore(sub, memarg) => {
                validate_atomic_memarg(memory_shared, *sub, *memarg, memories)?;
                pop_expect_control(&mut stack, &labels, atomic_store_value_type(*sub)?)?;
                pop_expect_control(&mut stack, &labels, memory_index_ty)?;
            }
            Instr::AtomicRmw(sub, memarg) => {
                validate_atomic_memarg(memory_shared, *sub, *memarg, memories)?;
                if atomic_rmw_is_cmpxchg(*sub) {
                    pop_expect_control(&mut stack, &labels, atomic_rmw_value_type(*sub)?)?;
                }
                pop_expect_control(&mut stack, &labels, atomic_rmw_value_type(*sub)?)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                stack.push(atomic_rmw_result_type(*sub)?);
            }
            Instr::AtomicNotify(memarg) => {
                validate_atomic_memarg(memory_shared, 0x00, *memarg, memories)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                stack.push(ValType::I32);
            }
            Instr::AtomicWait(sub, memarg) => {
                validate_atomic_memarg(memory_shared, *sub, *memarg, memories)?;
                pop_expect_control(&mut stack, &labels, ValType::I64)?;
                pop_expect_control(
                    &mut stack,
                    &labels,
                    if *sub == 0x01 {
                        ValType::I32
                    } else {
                        ValType::I64
                    },
                )?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                stack.push(ValType::I32);
            }
            Instr::AtomicFence(reserved) => validate_atomic_fence(memory_shared, *reserved)?,
            Instr::Num(op) => apply_numeric_stack_control(*op, &mut stack, &labels)?,
            Instr::TruncSat(sub) => apply_trunc_sat_stack_control(*sub, &mut stack, &labels)?,
            Instr::MemoryInit(_, _) => {
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                pop_expect_control(&mut stack, &labels, memory_index_ty)?;
            }
            Instr::DataDrop(_) => {}
            Instr::MemoryCopy(_, _) => {
                pop_expect_control(&mut stack, &labels, memory_index_ty)?;
                pop_expect_control(&mut stack, &labels, memory_index_ty)?;
                pop_expect_control(&mut stack, &labels, memory_index_ty)?;
            }
            Instr::MemoryFill(_) => {
                pop_expect_control(&mut stack, &labels, memory_index_ty)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                pop_expect_control(&mut stack, &labels, memory_index_ty)?;
            }
            Instr::TableGet(tableidx) => {
                let table = stack_table_type(*tableidx, table_types)?;
                let elem_ty = table.elem;
                pop_expect_control(&mut stack, &labels, table_index_type(table))?;
                stack.push(elem_ty);
            }
            Instr::TableSet(tableidx) => {
                let table = stack_table_type(*tableidx, table_types)?;
                let elem_ty = table.elem;
                pop_expect_control_typed(
                    &mut stack,
                    &labels,
                    elem_ty,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                pop_expect_control(&mut stack, &labels, table_index_type(table))?;
            }
            Instr::TableGrow(tableidx) => {
                let table = stack_table_type(*tableidx, table_types)?;
                let elem_ty = table.elem;
                let index_ty = table_index_type(table);
                pop_expect_control(&mut stack, &labels, index_ty)?;
                pop_expect_control_typed(
                    &mut stack,
                    &labels,
                    elem_ty,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                stack.push(index_ty);
            }
            Instr::TableSize(tableidx) => {
                let table = stack_table_type(*tableidx, table_types)?;
                stack.push(table_index_type(table));
            }
            Instr::TableFill(tableidx) => {
                let table = stack_table_type(*tableidx, table_types)?;
                let elem_ty = table.elem;
                let index_ty = table_index_type(table);
                pop_expect_control(&mut stack, &labels, index_ty)?;
                pop_expect_control_typed(
                    &mut stack,
                    &labels,
                    elem_ty,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                pop_expect_control(&mut stack, &labels, index_ty)?;
            }
            Instr::TableInit(_, tableidx) => {
                let table = stack_table_type(*tableidx, table_types)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                pop_expect_control(&mut stack, &labels, table_index_type(table))?;
            }
            Instr::ElemDrop(_) => {}
            Instr::TableCopy(dst, src) => {
                let dst_table = stack_table_type(*dst, table_types)?;
                let src_table = stack_table_type(*src, table_types)?;
                pop_expect_control(
                    &mut stack,
                    &labels,
                    table_copy_len_type(dst_table, src_table),
                )?;
                pop_expect_control(&mut stack, &labels, table_index_type(src_table))?;
                pop_expect_control(&mut stack, &labels, table_index_type(dst_table))?;
            }
            Instr::Unreachable => {
                if let Some(frame) = labels.last_mut() {
                    stack.truncate(frame.height);
                    frame.unreachable = true;
                }
            }
            Instr::BrTable(targets, default) => {
                pop_expect_control(&mut stack, &labels, ValType::I32)?;
                let stack_polymorphic = current_unreachable_height(&labels) == Some(stack.len());
                let current_height = labels
                    .last()
                    .map(|frame| frame.height)
                    .ok_or("br_table without current label")?;
                let default_target_idx = branch_target_index(&labels, *default)?;
                let default_target = &labels[default_target_idx];
                let branch_types = default_target.branch_types.clone();
                let mut target_indices = vec![default_target_idx];
                for depth in targets {
                    let target_idx = branch_target_index(&labels, *depth)?;
                    let target = &labels[target_idx];
                    if target.branch_types.len() != branch_types.len() {
                        return Err("br_table target type mismatch".to_string());
                    }
                    if !stack_polymorphic {
                        for (actual, expected) in target
                            .branch_types
                            .iter()
                            .copied()
                            .zip(branch_types.iter().copied())
                        {
                            if !types_compatible_in_module(
                                actual,
                                expected,
                                type_supertypes,
                                array_types,
                                struct_types,
                                types,
                            ) && !types_compatible_in_module(
                                expected,
                                actual,
                                type_supertypes,
                                array_types,
                                struct_types,
                                types,
                            ) {
                                return Err("br_table target type mismatch".to_string());
                            }
                        }
                    }
                    target_indices.push(target_idx);
                }
                pop_branch_results_control_typed(
                    &mut stack,
                    &labels,
                    current_height,
                    &branch_types,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )?;
                for target_idx in target_indices {
                    if let Some(target) = labels.get_mut(target_idx) {
                        target.branch_reached = true;
                    }
                }
                if let Some(frame) = labels.last_mut() {
                    stack.truncate(frame.height);
                    frame.unreachable = true;
                }
            }
        }
    }
    Ok(())
}

fn block_type_signature(
    bt: BlockType,
    types: &[FuncType],
) -> Result<(Vec<ValType>, Vec<ValType>), String> {
    match bt {
        BlockType::Empty => Ok((Vec::new(), Vec::new())),
        BlockType::Value(ty) => Ok((Vec::new(), vec![ty])),
        BlockType::TypeIndex(idx) => {
            let ft = types
                .get(idx as usize)
                .ok_or_else(|| format!("block type index {} out of bounds", idx))?;
            Ok((ft.params.clone(), ft.results.clone()))
        }
    }
}

fn branch_target(labels: &[ControlFrame], depth: u32) -> Result<&ControlFrame, String> {
    let idx = branch_target_index(labels, depth)?;
    Ok(&labels[idx])
}

fn branch_target_index(labels: &[ControlFrame], depth: u32) -> Result<usize, String> {
    let depth = depth as usize;
    if depth >= labels.len() {
        return Err(format!("branch depth {} out of bounds", depth));
    }
    Ok(labels.len() - 1 - depth)
}

fn current_unreachable_height(labels: &[ControlFrame]) -> Option<usize> {
    labels
        .last()
        .and_then(|frame| frame.unreachable.then_some(frame.height))
}

fn pop_expect_control(
    stack: &mut Vec<ValType>,
    labels: &[ControlFrame],
    expected: ValType,
) -> Result<(), String> {
    if let Some(height) = current_unreachable_height(labels) {
        if stack.len() == height {
            return Ok(());
        }
    }
    pop_expect(stack, expected)
}

fn pop_operand_control(
    stack: &mut Vec<ValType>,
    labels: &[ControlFrame],
) -> Result<ValType, String> {
    if let Some(height) = current_unreachable_height(labels) {
        if stack.len() == height {
            return Ok(ValType::Unknown);
        }
    }
    stack
        .pop()
        .ok_or_else(|| "operand stack underflow".to_string())
}

fn pop_ref_control(
    stack: &mut Vec<ValType>,
    labels: &[ControlFrame],
    op: &str,
) -> Result<ValType, String> {
    let actual = pop_operand_control(stack, labels)?;
    if !is_ref_type(actual) {
        return Err(format!(
            "{} operand must be reference, got {:?}",
            op, actual
        ));
    }
    Ok(actual)
}

fn pop_params_control(
    stack: &mut Vec<ValType>,
    labels: &[ControlFrame],
    params: &[ValType],
) -> Result<(), String> {
    for ty in params.iter().rev() {
        pop_expect_control(stack, labels, *ty)?;
    }
    Ok(())
}

fn pop_params_control_typed(
    stack: &mut Vec<ValType>,
    labels: &[ControlFrame],
    params: &[ValType],
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
) -> Result<(), String> {
    for ty in params.iter().rev() {
        pop_expect_control_typed(
            stack,
            labels,
            *ty,
            type_supertypes,
            array_types,
            struct_types,
            types,
        )?;
    }
    Ok(())
}

fn pop_expect_control_typed(
    stack: &mut Vec<ValType>,
    labels: &[ControlFrame],
    expected: ValType,
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
) -> Result<(), String> {
    if let Some(height) = current_unreachable_height(labels) {
        if stack.len() == height {
            return Ok(());
        }
    }
    pop_expect_typed(
        stack,
        expected,
        type_supertypes,
        array_types,
        struct_types,
        types,
    )
}

fn pop_results_control_typed(
    stack: &mut Vec<ValType>,
    labels: &[ControlFrame],
    results: &[ValType],
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
) -> Result<(), String> {
    for ty in results.iter().rev() {
        pop_expect_control_typed(
            stack,
            labels,
            *ty,
            type_supertypes,
            array_types,
            struct_types,
            types,
        )?;
    }
    Ok(())
}

fn pop_return_results_control_typed(
    stack: &mut Vec<ValType>,
    labels: &[ControlFrame],
    results: &[ValType],
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
) -> Result<(), String> {
    if current_unreachable_height(labels) == Some(stack.len()) {
        return pop_results_control_typed(
            stack,
            labels,
            results,
            type_supertypes,
            array_types,
            struct_types,
            types,
        );
    }
    let current_height = labels.last().map(|frame| frame.height).unwrap_or(0);
    let needed = current_height + results.len();
    if stack.len() < needed {
        return Err(format!(
            "return operand stack underflow: expected {} values above current height {}, got {}",
            results.len(),
            current_height,
            stack.len().saturating_sub(current_height)
        ));
    }
    pop_results_typed(
        stack,
        results,
        type_supertypes,
        array_types,
        struct_types,
        types,
    )
}

fn stack_table_type(tableidx: u32, table_types: &[TableType]) -> Result<TableType, String> {
    if tableidx as usize >= table_types.len() {
        return Err(format!(
            "table index {} out of bounds ({} tables)",
            tableidx,
            table_types.len()
        ));
    }
    Ok(table_types[tableidx as usize])
}

fn table_index_type(table: TableType) -> ValType {
    if table.limits.memory64 {
        ValType::I64
    } else {
        ValType::I32
    }
}

fn table_copy_len_type(dst: TableType, src: TableType) -> ValType {
    if dst.limits.memory64 && src.limits.memory64 {
        ValType::I64
    } else {
        ValType::I32
    }
}

fn validate_indirect_call_table(
    table: TableType,
    op: &str,
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
) -> Result<(), String> {
    if types_compatible_in_module(
        table.elem,
        ValType::FuncRef,
        type_supertypes,
        array_types,
        struct_types,
        types,
    ) {
        Ok(())
    } else {
        Err(format!(
            "{} target table element type {:?} is not a function reference",
            op, table.elem
        ))
    }
}

fn array_type(typeidx: u32, array_types: &[Option<ArrayType>]) -> Result<ArrayType, String> {
    array_types
        .get(typeidx as usize)
        .and_then(|ty| *ty)
        .ok_or_else(|| format!("type index {} is not an array type", typeidx))
}

fn struct_type(typeidx: u32, struct_types: &[Option<StructType>]) -> Result<&StructType, String> {
    struct_types
        .get(typeidx as usize)
        .and_then(|ty| ty.as_ref())
        .ok_or_else(|| format!("type index {} is not a struct type", typeidx))
}

fn validate_array_init_data_storage(typeidx: u32, array: ArrayType) -> Result<(), String> {
    if array.packed_bits.is_some()
        || matches!(
            array.element,
            ValType::I32 | ValType::I64 | ValType::F32 | ValType::F64 | ValType::V128
        )
    {
        Ok(())
    } else {
        Err(format!(
            "array.init_data type {} is not numeric or vector",
            typeidx
        ))
    }
}

fn pop_results(stack: &mut Vec<ValType>, results: &[ValType]) -> Result<(), String> {
    for ty in results.iter().rev() {
        pop_expect(stack, *ty)?;
    }
    Ok(())
}

fn pop_results_typed(
    stack: &mut Vec<ValType>,
    results: &[ValType],
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
) -> Result<(), String> {
    for ty in results.iter().rev() {
        pop_expect_typed(
            stack,
            *ty,
            type_supertypes,
            array_types,
            struct_types,
            types,
        )?;
    }
    Ok(())
}

fn pop_branch_results_control_typed(
    stack: &mut Vec<ValType>,
    labels: &[ControlFrame],
    target_height: usize,
    results: &[ValType],
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
) -> Result<(), String> {
    if current_unreachable_height(labels) == Some(stack.len()) {
        for ty in results.iter().rev() {
            pop_expect_control_typed(
                stack,
                labels,
                *ty,
                type_supertypes,
                array_types,
                struct_types,
                types,
            )?;
        }
        return Ok(());
    }
    let needed = target_height + results.len();
    if stack.len() < needed {
        return Err(format!(
            "branch operand stack underflow: expected {} values above target height {}, got {}",
            results.len(),
            target_height,
            stack.len().saturating_sub(target_height)
        ));
    }
    pop_results_typed(
        stack,
        results,
        type_supertypes,
        array_types,
        struct_types,
        types,
    )
}

fn pop_br_if_branch_results_control_typed(
    stack: &mut Vec<ValType>,
    labels: &[ControlFrame],
    current_height: usize,
    results: &[ValType],
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
) -> Result<(), String> {
    if current_unreachable_height(labels) == Some(stack.len()) {
        for ty in results.iter().rev() {
            pop_expect_control_typed(
                stack,
                labels,
                *ty,
                type_supertypes,
                array_types,
                struct_types,
                types,
            )?;
        }
        return Ok(());
    }
    let needed = current_height + results.len();
    if stack.len() < needed {
        return Err(format!(
            "branch operand stack underflow: expected {} values above current height {} after br_if condition, got {}",
            results.len(),
            current_height,
            stack.len().saturating_sub(current_height)
        ));
    }
    pop_results_typed(
        stack,
        results,
        type_supertypes,
        array_types,
        struct_types,
        types,
    )
}

fn ensure_stack_height(stack: &[ValType], height: usize) -> Result<(), String> {
    if stack.len() != height {
        return Err(format!(
            "control frame leaves extra stack values: expected height {}, got {}",
            height,
            stack.len()
        ));
    }
    Ok(())
}

fn pop_params_typed(
    stack: &mut Vec<ValType>,
    params: &[ValType],
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
) -> Result<(), String> {
    for ty in params.iter().rev() {
        pop_expect_typed(
            stack,
            *ty,
            type_supertypes,
            array_types,
            struct_types,
            types,
        )?;
    }
    Ok(())
}

fn pop_expect_typed(
    stack: &mut Vec<ValType>,
    expected: ValType,
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
) -> Result<(), String> {
    let actual = stack
        .pop()
        .ok_or_else(|| format!("operand stack underflow while expecting {:?}", expected))?;
    if !types_compatible_in_module(
        actual,
        expected,
        type_supertypes,
        array_types,
        struct_types,
        types,
    ) {
        return Err(format!(
            "operand type mismatch: expected {:?}, got {:?}",
            expected, actual
        ));
    }
    Ok(())
}

fn pop_expect(stack: &mut Vec<ValType>, expected: ValType) -> Result<(), String> {
    let actual = stack
        .pop()
        .ok_or_else(|| format!("operand stack underflow while expecting {:?}", expected))?;
    if !types_compatible(actual, expected) {
        return Err(format!(
            "operand type mismatch: expected {:?}, got {:?}",
            expected, actual
        ));
    }
    Ok(())
}

fn pop_ref(stack: &mut Vec<ValType>, op: &str) -> Result<ValType, String> {
    let actual = stack.pop().ok_or_else(|| {
        format!(
            "operand stack underflow while expecting reference for {}",
            op
        )
    })?;
    if !is_ref_type(actual) {
        return Err(format!(
            "{} operand must be reference, got {:?}",
            op, actual
        ));
    }
    Ok(actual)
}

fn types_compatible(actual: ValType, expected: ValType) -> bool {
    actual == expected
        || actual == ValType::Unknown
        || expected == ValType::Unknown
        || matches!(
            (actual, expected),
            (
                ValType::NullRef,
                ValType::NullRef
                    | ValType::AnyRef
                    | ValType::EqRef
                    | ValType::StructRef
                    | ValType::ArrayRef
                    | ValType::I31Ref
                    | ValType::TypeRef(_)
            ) | (
                ValType::NullFuncRef,
                ValType::NullFuncRef | ValType::FuncRef | ValType::TypeRef(_)
            ) | (
                ValType::NullExternRef,
                ValType::NullExternRef | ValType::ExternRef
            ) | (
                ValType::StructRef | ValType::ArrayRef | ValType::I31Ref | ValType::TypeRef(_),
                ValType::AnyRef | ValType::EqRef
            ) | (ValType::NonNullAnyRef, ValType::AnyRef)
                | (
                    ValType::NonNullEqRef,
                    ValType::EqRef | ValType::AnyRef | ValType::NonNullAnyRef
                )
                | (
                    ValType::NonNullFuncRef,
                    ValType::FuncRef | ValType::AnyRef | ValType::NonNullAnyRef
                )
                | (
                    ValType::NonNullExternRef,
                    ValType::ExternRef | ValType::AnyRef | ValType::NonNullAnyRef
                )
                | (
                    ValType::NonNullStructRef,
                    ValType::StructRef
                        | ValType::EqRef
                        | ValType::AnyRef
                        | ValType::NonNullEqRef
                        | ValType::NonNullAnyRef
                )
                | (
                    ValType::NonNullArrayRef,
                    ValType::ArrayRef
                        | ValType::EqRef
                        | ValType::AnyRef
                        | ValType::NonNullEqRef
                        | ValType::NonNullAnyRef
                )
                | (
                    ValType::NonNullI31Ref,
                    ValType::I31Ref
                        | ValType::EqRef
                        | ValType::AnyRef
                        | ValType::NonNullEqRef
                        | ValType::NonNullAnyRef
                )
                | (
                    ValType::NonNullTypeRef(_),
                    ValType::FuncRef
                        | ValType::AnyRef
                        | ValType::EqRef
                        | ValType::NonNullAnyRef
                        | ValType::NonNullEqRef
                )
                | (
                    ValType::NonNullTypeRef(_),
                    ValType::StructRef | ValType::NonNullStructRef
                )
                | (
                    ValType::NonNullTypeRef(_),
                    ValType::ArrayRef | ValType::NonNullArrayRef
                )
                | (ValType::TypeRef(_), ValType::StructRef)
                | (ValType::TypeRef(_), ValType::ArrayRef)
                | (ValType::ExternRef, ValType::AnyRef)
        )
}

fn types_compatible_in_module(
    actual: ValType,
    expected: ValType,
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
) -> bool {
    if types_compatible(actual, expected) {
        return true;
    }
    match (actual, expected) {
        (ValType::NullFuncRef, ValType::TypeRef(expected_idx)) => {
            let idx = expected_idx as usize;
            types.get(idx).is_some()
                && !matches!(array_types.get(idx), Some(Some(_)))
                && !matches!(struct_types.get(idx), Some(Some(_)))
        }
        (ValType::TypeRef(actual_idx), ValType::FuncRef)
        | (ValType::NonNullTypeRef(actual_idx), ValType::FuncRef)
        | (ValType::NonNullTypeRef(actual_idx), ValType::NonNullFuncRef) => {
            let idx = actual_idx as usize;
            !matches!(array_types.get(idx), Some(Some(_)))
                && !matches!(struct_types.get(idx), Some(Some(_)))
        }
        (ValType::TypeRef(actual_idx), ValType::TypeRef(expected_idx))
        | (ValType::NonNullTypeRef(actual_idx), ValType::TypeRef(expected_idx))
        | (ValType::NonNullTypeRef(actual_idx), ValType::NonNullTypeRef(expected_idx)) => {
            type_ref_matches(
                actual_idx,
                expected_idx,
                type_supertypes,
                array_types,
                struct_types,
                types,
                &mut Vec::new(),
            )
        }
        _ => false,
    }
}

fn result_types_compatible_in_module(
    actual: &[ValType],
    expected: &[ValType],
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected.iter())
            .all(|(actual, expected)| {
                types_compatible_in_module(
                    *actual,
                    *expected,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                )
            })
}

fn ref_func_initializer_matches(
    init: &[Instr],
    expected: ValType,
    funcs: &[u32],
    m: &Module,
) -> bool {
    let expected_idx = match expected {
        ValType::TypeRef(expected_idx) | ValType::NonNullTypeRef(expected_idx) => expected_idx,
        _ => return false,
    };
    let func_idx = match init {
        [Instr::RefFunc(func_idx)] | [Instr::RefFunc(func_idx), Instr::End] => *func_idx,
        _ => return false,
    };
    let Some(actual_idx) = funcs.get(func_idx as usize).copied() else {
        return false;
    };
    type_ref_is_subtype_or_rec_group_match(actual_idx, expected_idx, m, &mut Vec::new())
}

fn type_ref_is_subtype_or_rec_group_match(
    actual: u32,
    expected: u32,
    m: &Module,
    seen: &mut Vec<u32>,
) -> bool {
    if actual == expected || rec_group_type_matches(actual, expected, m, &mut Vec::new()) {
        return true;
    }
    if seen.contains(&actual) {
        return false;
    }
    seen.push(actual);
    m.type_supertypes
        .get(actual as usize)
        .map(|supers| {
            supers.iter().any(|super_idx| {
                type_ref_is_subtype_or_rec_group_match(*super_idx, expected, m, seen)
            })
        })
        .unwrap_or(false)
}

fn rec_group_type_matches(
    actual: u32,
    expected: u32,
    m: &Module,
    seen: &mut Vec<(u32, u32)>,
) -> bool {
    if actual == expected {
        return true;
    }
    if seen.contains(&(actual, expected)) {
        return true;
    }
    let actual_idx = actual as usize;
    let expected_idx = expected as usize;
    let Some(actual_group) = m.type_rec_groups.get(actual_idx).copied() else {
        return false;
    };
    let Some(expected_group) = m.type_rec_groups.get(expected_idx).copied() else {
        return false;
    };
    let actual_offset = actual.checked_sub(actual_group);
    let expected_offset = expected.checked_sub(expected_group);
    if actual_offset != expected_offset {
        return false;
    }
    seen.push((actual, expected));
    let actual_members: Vec<u32> = m
        .type_rec_groups
        .iter()
        .enumerate()
        .filter_map(|(idx, group)| (*group == actual_group).then_some(idx as u32))
        .collect();
    let expected_members: Vec<u32> = m
        .type_rec_groups
        .iter()
        .enumerate()
        .filter_map(|(idx, group)| (*group == expected_group).then_some(idx as u32))
        .collect();
    if actual_members.len() != expected_members.len()
        || !actual_members
            .iter()
            .zip(expected_members.iter())
            .all(|(a, e)| seen.contains(&(*a, *e)) || rec_group_type_matches(*a, *e, m, seen))
    {
        return false;
    }
    let actual_supers = m
        .type_supertypes
        .get(actual_idx)
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    let expected_supers = m
        .type_supertypes
        .get(expected_idx)
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    if actual_supers.len() != expected_supers.len()
        || !actual_supers
            .iter()
            .zip(expected_supers.iter())
            .all(|(a, e)| rec_group_ref_matches(*a, *e, actual_group, expected_group, m, seen))
    {
        return false;
    }
    match (
        m.type_is_func.get(actual_idx).copied().unwrap_or(false),
        m.type_is_func.get(expected_idx).copied().unwrap_or(false),
        m.struct_types.get(actual_idx),
        m.struct_types.get(expected_idx),
        m.array_types.get(actual_idx),
        m.array_types.get(expected_idx),
    ) {
        (true, true, _, _, _, _) => func_types_match_rec_group(
            &m.types[actual_idx],
            &m.types[expected_idx],
            actual_group,
            expected_group,
            m,
            seen,
        ),
        (_, _, Some(Some(actual_struct)), Some(Some(expected_struct)), _, _) => {
            actual_struct.fields.len() == expected_struct.fields.len()
                && actual_struct
                    .fields
                    .iter()
                    .zip(expected_struct.fields.iter())
                    .all(|(a, e)| {
                        a.mutable == e.mutable
                            && a.packed_bits == e.packed_bits
                            && val_types_match_rec_group(
                                a.ty,
                                e.ty,
                                actual_group,
                                expected_group,
                                m,
                                seen,
                            )
                    })
        }
        (_, _, _, _, Some(Some(actual_array)), Some(Some(expected_array))) => {
            actual_array.mutable == expected_array.mutable
                && actual_array.packed_bits == expected_array.packed_bits
                && val_types_match_rec_group(
                    actual_array.element,
                    expected_array.element,
                    actual_group,
                    expected_group,
                    m,
                    seen,
                )
        }
        _ => false,
    }
}

fn func_types_match_rec_group(
    actual: &FuncType,
    expected: &FuncType,
    actual_group: u32,
    expected_group: u32,
    m: &Module,
    seen: &mut Vec<(u32, u32)>,
) -> bool {
    actual.params.len() == expected.params.len()
        && actual.results.len() == expected.results.len()
        && actual
            .params
            .iter()
            .zip(expected.params.iter())
            .all(|(a, e)| val_types_match_rec_group(*a, *e, actual_group, expected_group, m, seen))
        && actual
            .results
            .iter()
            .zip(expected.results.iter())
            .all(|(a, e)| val_types_match_rec_group(*a, *e, actual_group, expected_group, m, seen))
}

fn val_types_match_rec_group(
    actual: ValType,
    expected: ValType,
    actual_group: u32,
    expected_group: u32,
    m: &Module,
    seen: &mut Vec<(u32, u32)>,
) -> bool {
    match (actual, expected) {
        (ValType::TypeRef(a), ValType::TypeRef(e))
        | (ValType::NonNullTypeRef(a), ValType::NonNullTypeRef(e)) => {
            rec_group_ref_matches(a, e, actual_group, expected_group, m, seen)
        }
        (ValType::NonNullTypeRef(a), ValType::TypeRef(e)) => {
            rec_group_ref_matches(a, e, actual_group, expected_group, m, seen)
        }
        _ => actual == expected,
    }
}

fn type_group_len(m: &Module, group: u32) -> u32 {
    m.type_rec_groups
        .iter()
        .filter(|candidate| **candidate == group)
        .count() as u32
}

fn type_ref_group_offset(idx: u32, group: u32, group_len: u32) -> Option<u32> {
    let offset = idx.checked_sub(group)?;
    (offset < group_len).then_some(offset)
}

fn rec_group_ref_matches(
    actual: u32,
    expected: u32,
    actual_group: u32,
    expected_group: u32,
    m: &Module,
    seen: &mut Vec<(u32, u32)>,
) -> bool {
    let actual_offset =
        type_ref_group_offset(actual, actual_group, type_group_len(m, actual_group));
    let expected_offset =
        type_ref_group_offset(expected, expected_group, type_group_len(m, expected_group));
    match (actual_offset, expected_offset) {
        (Some(a), Some(e)) => a == e && rec_group_type_matches(actual, expected, m, seen),
        (None, None) => {
            rec_group_type_matches(actual, expected, m, &mut Vec::new())
                && rec_group_type_matches(expected, actual, m, &mut Vec::new())
        }
        _ => false,
    }
}

fn type_decl_is_valid_subtype(actual_idx: u32, super_idx: u32, m: &Module) -> bool {
    let actual_usize = actual_idx as usize;
    let super_usize = super_idx as usize;
    match (
        m.struct_types.get(actual_usize),
        m.struct_types.get(super_usize),
        m.array_types.get(actual_usize),
        m.array_types.get(super_usize),
        m.type_is_func.get(actual_usize),
        m.type_is_func.get(super_usize),
    ) {
        (Some(Some(actual)), Some(Some(super_ty)), _, _, _, _) => {
            struct_decl_is_valid_subtype(actual, super_ty, m)
        }
        (_, _, Some(Some(actual)), Some(Some(super_ty)), _, _) => {
            array_decl_is_valid_subtype(*actual, *super_ty, m)
        }
        (_, _, _, _, Some(true), Some(true)) => {
            func_decl_is_valid_subtype(&m.types[actual_usize], &m.types[super_usize], m)
        }
        _ => false,
    }
}

fn func_decl_is_valid_subtype(actual: &FuncType, super_ty: &FuncType, m: &Module) -> bool {
    actual.params.len() == super_ty.params.len()
        && actual.results.len() == super_ty.results.len()
        && actual
            .params
            .iter()
            .zip(super_ty.params.iter())
            .all(|(actual_param, super_param)| val_type_is_subtype(*super_param, *actual_param, m))
        && actual.results.iter().zip(super_ty.results.iter()).all(
            |(actual_result, super_result)| val_type_is_subtype(*actual_result, *super_result, m),
        )
}

fn struct_decl_is_valid_subtype(actual: &StructType, super_ty: &StructType, m: &Module) -> bool {
    actual.fields.len() >= super_ty.fields.len()
        && actual
            .fields
            .iter()
            .zip(super_ty.fields.iter())
            .all(|(actual_field, super_field)| {
                actual_field.mutable == super_field.mutable
                    && actual_field.packed_bits == super_field.packed_bits
                    && if actual_field.mutable {
                        val_types_equivalent_for_validation(
                            actual_field.ty,
                            super_field.ty,
                            &m.type_supertypes,
                            &m.array_types,
                            &m.struct_types,
                            &m.types,
                            &mut Vec::new(),
                        )
                    } else {
                        val_type_is_subtype(actual_field.ty, super_field.ty, m)
                    }
            })
}

fn array_decl_is_valid_subtype(actual: ArrayType, super_ty: ArrayType, m: &Module) -> bool {
    actual.mutable == super_ty.mutable
        && actual.packed_bits == super_ty.packed_bits
        && if actual.mutable {
            val_types_equivalent_for_validation(
                actual.element,
                super_ty.element,
                &m.type_supertypes,
                &m.array_types,
                &m.struct_types,
                &m.types,
                &mut Vec::new(),
            )
        } else {
            val_type_is_subtype(actual.element, super_ty.element, m)
        }
}

fn validate_array_copy_element_subtype(
    dst_idx: u32,
    dst: ArrayType,
    src_idx: u32,
    src: ArrayType,
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
) -> Result<(), String> {
    if dst.packed_bits != src.packed_bits {
        return Err(format!(
            "array.copy source array {} element type is not a subtype of destination array {} element type",
            src_idx, dst_idx
        ));
    }
    if !types_compatible_in_module(
        src.element,
        dst.element,
        type_supertypes,
        array_types,
        struct_types,
        types,
    ) {
        return Err(format!(
            "array.copy source array {} element type is not a subtype of destination array {} element type",
            src_idx, dst_idx
        ));
    }
    Ok(())
}

fn validate_array_init_elem_storage(
    typeidx: u32,
    array: ArrayType,
    elemidx: u32,
    elements: &[ElementSegment],
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
) -> Result<(), String> {
    if array.packed_bits.is_some() || !is_ref_type(array.element) {
        return Err(format!(
            "array.init_elem type {} is not a reference-type array",
            typeidx
        ));
    }
    let elem_ty = elements
        .get(elemidx as usize)
        .map(|seg| seg.ty)
        .ok_or_else(|| format!("array.init_elem element index {} out of bounds", elemidx))?;
    if !types_compatible_in_module(
        elem_ty,
        array.element,
        type_supertypes,
        array_types,
        struct_types,
        types,
    ) {
        return Err(format!(
            "array.init_elem element segment {} type {:?} is not a subtype of array {} element type {:?}",
            elemidx, elem_ty, typeidx, array.element
        ));
    }
    Ok(())
}

fn val_type_is_subtype(actual: ValType, expected: ValType, m: &Module) -> bool {
    types_compatible_in_module(
        actual,
        expected,
        &m.type_supertypes,
        &m.array_types,
        &m.struct_types,
        &m.types,
    )
}

fn type_ref_matches(
    actual: u32,
    expected: u32,
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
    seen: &mut Vec<(u32, u32)>,
) -> bool {
    if actual == expected {
        return true;
    }
    if seen.contains(&(actual, expected)) {
        return true;
    }
    seen.push((actual, expected));
    if type_refs_canonically_equal(
        actual,
        expected,
        type_supertypes,
        array_types,
        struct_types,
        types,
        seen,
    ) {
        return true;
    }
    type_supertypes
        .get(actual as usize)
        .map(|supers| {
            supers.iter().any(|super_idx| {
                type_ref_matches(
                    *super_idx,
                    expected,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                    seen,
                )
            })
        })
        .unwrap_or(false)
}

fn type_refs_canonically_equal(
    left: u32,
    right: u32,
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
    seen: &mut Vec<(u32, u32)>,
) -> bool {
    if left == right {
        return true;
    }
    let left_supers = type_supertypes
        .get(left as usize)
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    let right_supers = type_supertypes
        .get(right as usize)
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    if left_supers.len() != right_supers.len()
        || !left_supers.iter().zip(right_supers.iter()).all(|(l, r)| {
            type_ref_matches(
                *l,
                *r,
                type_supertypes,
                array_types,
                struct_types,
                types,
                seen,
            ) && type_ref_matches(
                *r,
                *l,
                type_supertypes,
                array_types,
                struct_types,
                types,
                seen,
            )
        })
    {
        return false;
    }
    match (
        struct_types.get(left as usize),
        struct_types.get(right as usize),
        array_types.get(left as usize),
        array_types.get(right as usize),
        types.get(left as usize),
        types.get(right as usize),
    ) {
        (Some(Some(left_struct)), Some(Some(right_struct)), _, _, _, _) => {
            struct_types_equivalent_for_validation(
                left_struct,
                right_struct,
                type_supertypes,
                array_types,
                struct_types,
                types,
                seen,
            )
        }
        (_, _, Some(Some(left_array)), Some(Some(right_array)), _, _) => {
            array_types_equivalent_for_validation(
                *left_array,
                *right_array,
                type_supertypes,
                array_types,
                struct_types,
                types,
                seen,
            )
        }
        (_, _, _, _, Some(left_func), Some(right_func)) => func_types_equivalent_for_validation(
            left_func,
            right_func,
            type_supertypes,
            array_types,
            struct_types,
            types,
            seen,
        ),
        _ => false,
    }
}

fn func_types_equivalent_for_validation(
    left: &FuncType,
    right: &FuncType,
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
    seen: &mut Vec<(u32, u32)>,
) -> bool {
    val_type_lists_equivalent_for_validation(
        &left.params,
        &right.params,
        type_supertypes,
        array_types,
        struct_types,
        types,
        seen,
    ) && val_type_lists_equivalent_for_validation(
        &left.results,
        &right.results,
        type_supertypes,
        array_types,
        struct_types,
        types,
        seen,
    )
}

fn struct_types_equivalent_for_validation(
    left: &StructType,
    right: &StructType,
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
    seen: &mut Vec<(u32, u32)>,
) -> bool {
    left.fields.len() == right.fields.len()
        && left
            .fields
            .iter()
            .zip(right.fields.iter())
            .all(|(left_field, right_field)| {
                val_types_equivalent_for_validation(
                    left_field.ty,
                    right_field.ty,
                    type_supertypes,
                    array_types,
                    struct_types,
                    types,
                    seen,
                ) && left_field.mutable == right_field.mutable
                    && left_field.packed_bits == right_field.packed_bits
            })
}

fn array_types_equivalent_for_validation(
    left: ArrayType,
    right: ArrayType,
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
    seen: &mut Vec<(u32, u32)>,
) -> bool {
    val_types_equivalent_for_validation(
        left.element,
        right.element,
        type_supertypes,
        array_types,
        struct_types,
        types,
        seen,
    ) && left.mutable == right.mutable
        && left.packed_bits == right.packed_bits
}

fn val_type_lists_equivalent_for_validation(
    left: &[ValType],
    right: &[ValType],
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
    seen: &mut Vec<(u32, u32)>,
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right.iter()).all(|(left_ty, right_ty)| {
            val_types_equivalent_for_validation(
                *left_ty,
                *right_ty,
                type_supertypes,
                array_types,
                struct_types,
                types,
                seen,
            )
        })
}

fn val_types_equivalent_for_validation(
    left: ValType,
    right: ValType,
    type_supertypes: &[Vec<u32>],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    types: &[FuncType],
    seen: &mut Vec<(u32, u32)>,
) -> bool {
    if left == right {
        return true;
    }
    match (left, right) {
        (ValType::TypeRef(left_idx), ValType::TypeRef(right_idx))
        | (ValType::NonNullTypeRef(left_idx), ValType::NonNullTypeRef(right_idx))
        | (ValType::NonNullTypeRef(left_idx), ValType::TypeRef(right_idx)) => {
            type_ref_matches(
                left_idx,
                right_idx,
                type_supertypes,
                array_types,
                struct_types,
                types,
                seen,
            ) && type_ref_matches(
                right_idx,
                left_idx,
                type_supertypes,
                array_types,
                struct_types,
                types,
                seen,
            )
        }
        _ => false,
    }
}

fn validate_lane(lane: u8, lane_count: u8, op: &str) -> Result<(), String> {
    if lane >= lane_count {
        return Err(format!("{} lane {} out of bounds", op, lane));
    }
    Ok(())
}

fn validate_replace_lane(
    stack: &mut Vec<ValType>,
    lane: u8,
    lane_count: u8,
    value_ty: ValType,
    op: &str,
) -> Result<(), String> {
    validate_lane(lane, lane_count, op)?;
    pop_expect(stack, value_ty)?;
    pop_expect(stack, ValType::V128)?;
    stack.push(ValType::V128);
    Ok(())
}

fn load_result_type(op: u8) -> Result<ValType, String> {
    match op {
        0x28 | 0x2c..=0x2f => Ok(ValType::I32),
        0x29 | 0x30..=0x35 => Ok(ValType::I64),
        0x2a => Ok(ValType::F32),
        0x2b => Ok(ValType::F64),
        _ => Err(format!("unsupported load opcode 0x{:02x}", op)),
    }
}

fn store_value_type(op: u8) -> Result<ValType, String> {
    match op {
        0x36 | 0x3a | 0x3b => Ok(ValType::I32),
        0x37 | 0x3c..=0x3e => Ok(ValType::I64),
        0x38 => Ok(ValType::F32),
        0x39 => Ok(ValType::F64),
        _ => Err(format!("unsupported store opcode 0x{:02x}", op)),
    }
}

fn atomic_load_result_type(sub: u32) -> Result<ValType, String> {
    match sub {
        0x10 | 0x12 | 0x13 => Ok(ValType::I32),
        0x11 | 0x14 | 0x15 | 0x16 => Ok(ValType::I64),
        _ => Err(format!("unsupported atomic load subopcode 0x{:02x}", sub)),
    }
}

fn atomic_store_value_type(sub: u32) -> Result<ValType, String> {
    match sub {
        0x17 | 0x19 | 0x1a => Ok(ValType::I32),
        0x18 | 0x1b | 0x1c | 0x1d => Ok(ValType::I64),
        _ => Err(format!("unsupported atomic store subopcode 0x{:02x}", sub)),
    }
}

fn atomic_rmw_value_type(sub: u32) -> Result<ValType, String> {
    match sub {
        _ if is_i32_atomic_rmw(sub) => Ok(ValType::I32),
        _ if is_i64_atomic_rmw(sub) => Ok(ValType::I64),
        _ => Err(format!("unsupported atomic rmw subopcode 0x{:02x}", sub)),
    }
}

fn atomic_rmw_result_type(sub: u32) -> Result<ValType, String> {
    match sub {
        _ if is_i32_atomic_rmw(sub) => Ok(ValType::I32),
        _ if is_i64_atomic_rmw(sub) => Ok(ValType::I64),
        _ => Err(format!("unsupported atomic rmw subopcode 0x{:02x}", sub)),
    }
}

fn is_i64_atomic_rmw(sub: u32) -> bool {
    matches!(
        sub,
        0x1f | 0x22
            | 0x23
            | 0x24
            | 0x26
            | 0x29
            | 0x2a
            | 0x2b
            | 0x2d
            | 0x30
            | 0x31
            | 0x32
            | 0x34
            | 0x37
            | 0x38
            | 0x39
            | 0x3b
            | 0x3e
            | 0x3f
            | 0x40
            | 0x42
            | 0x45
            | 0x46
            | 0x47
            | 0x49
            | 0x4c
            | 0x4d
            | 0x4e
    )
}

fn is_i32_atomic_rmw(sub: u32) -> bool {
    matches!(
        sub,
        0x1e | 0x20
            | 0x21
            | 0x25
            | 0x27
            | 0x28
            | 0x2c
            | 0x2e
            | 0x2f
            | 0x33
            | 0x35
            | 0x36
            | 0x3a
            | 0x3c
            | 0x3d
            | 0x41
            | 0x43
            | 0x44
            | 0x48
            | 0x4a
            | 0x4b
    )
}

fn atomic_rmw_is_cmpxchg(sub: u32) -> bool {
    matches!(sub, 0x48 | 0x49 | 0x4a | 0x4b | 0x4c | 0x4d | 0x4e)
}

fn memory_expected_max_align(op: u8) -> Result<u32, String> {
    match op {
        0x28 | 0x2a | 0x34 | 0x35 | 0x36 | 0x38 | 0x3e => Ok(2),
        0x29 | 0x2b | 0x37 | 0x39 => Ok(3),
        0x2c | 0x2d | 0x30 | 0x31 | 0x3a | 0x3c => Ok(0),
        0x2e | 0x2f | 0x32 | 0x33 | 0x3b | 0x3d => Ok(1),
        _ => Err(format!("unsupported memory opcode 0x{:02x}", op)),
    }
}

fn validate_memarg_memory_offset(memarg: MemArg, memories: &[Limits]) -> Result<(), String> {
    let memory = memories.get(memarg.memory as usize).ok_or_else(|| {
        format!(
            "memory index {} out of bounds ({} memories)",
            memarg.memory,
            memories.len()
        )
    })?;
    if !memory.memory64 && memarg.offset > u32::MAX as u64 {
        return Err("memory offset outside 32-bit range".to_string());
    }
    Ok(())
}

fn validate_memory_memarg(op: u8, memarg: MemArg, memories: &[Limits]) -> Result<(), String> {
    validate_memarg_memory_offset(memarg, memories)?;
    let expected = memory_expected_max_align(op)?;
    if memarg.align > expected {
        return Err(format!(
            "invalid alignment; expected maximum alignment is {}, actual alignment is {}",
            expected, memarg.align
        ));
    }
    Ok(())
}

fn simd_memarg_expected_max_align(instr: &Instr) -> Result<u32, String> {
    match instr {
        Instr::V128Load(_) | Instr::V128Store(_) => Ok(4),
        Instr::V128Load8Splat(_) | Instr::V128Load8Lane(_, _) | Instr::V128Store8Lane(_, _) => {
            Ok(0)
        }
        Instr::V128Load16Splat(_) | Instr::V128Load16Lane(_, _) | Instr::V128Store16Lane(_, _) => {
            Ok(1)
        }
        Instr::V128Load32Splat(_)
        | Instr::V128Load32Zero(_)
        | Instr::V128Load32Lane(_, _)
        | Instr::V128Store32Lane(_, _) => Ok(2),
        Instr::V128Load64Splat(_)
        | Instr::V128Load64Zero(_)
        | Instr::V128Load64Lane(_, _)
        | Instr::V128Store64Lane(_, _) => Ok(3),
        Instr::V128Load8x8S(_)
        | Instr::V128Load8x8U(_)
        | Instr::V128Load16x4S(_)
        | Instr::V128Load16x4U(_)
        | Instr::V128Load32x2S(_)
        | Instr::V128Load32x2U(_) => Ok(3),
        _ => Err("unsupported SIMD memory instruction".to_string()),
    }
}

fn simd_memarg(instr: &Instr) -> Result<MemArg, String> {
    match instr {
        Instr::V128Load(memarg)
        | Instr::V128Load8Splat(memarg)
        | Instr::V128Load16Splat(memarg)
        | Instr::V128Load32Splat(memarg)
        | Instr::V128Load64Splat(memarg)
        | Instr::V128Load8x8S(memarg)
        | Instr::V128Load8x8U(memarg)
        | Instr::V128Load16x4S(memarg)
        | Instr::V128Load16x4U(memarg)
        | Instr::V128Load32x2S(memarg)
        | Instr::V128Load32x2U(memarg)
        | Instr::V128Load32Zero(memarg)
        | Instr::V128Load64Zero(memarg)
        | Instr::V128Store(memarg)
        | Instr::V128Load8Lane(memarg, _)
        | Instr::V128Load16Lane(memarg, _)
        | Instr::V128Load32Lane(memarg, _)
        | Instr::V128Load64Lane(memarg, _)
        | Instr::V128Store8Lane(memarg, _)
        | Instr::V128Store16Lane(memarg, _)
        | Instr::V128Store32Lane(memarg, _)
        | Instr::V128Store64Lane(memarg, _) => Ok(*memarg),
        _ => Err("unsupported SIMD memory instruction".to_string()),
    }
}

fn validate_simd_memarg(instr: &Instr, memories: &[Limits]) -> Result<(), String> {
    let memarg = simd_memarg(instr)?;
    validate_memarg_memory_offset(memarg, memories)?;
    let expected = simd_memarg_expected_max_align(instr)?;
    if memarg.align > expected {
        return Err(format!(
            "invalid alignment; expected maximum alignment is {}, actual alignment is {}",
            expected, memarg.align
        ));
    }
    Ok(())
}

fn atomic_expected_align(sub: u32) -> Result<u32, String> {
    match sub {
        0x00 | 0x01 => Ok(2),
        0x02 => Ok(3),
        0x12 | 0x14 | 0x19 | 0x1b | 0x20 | 0x22 | 0x27 | 0x29 | 0x2e | 0x30 | 0x35 | 0x37
        | 0x3c | 0x3e | 0x43 | 0x45 | 0x4a | 0x4c => Ok(0),
        0x13 | 0x15 | 0x1a | 0x1c | 0x21 | 0x23 | 0x28 | 0x2a | 0x2f | 0x31 | 0x36 | 0x38
        | 0x3d | 0x3f | 0x44 | 0x46 | 0x4b | 0x4d => Ok(1),
        0x10 | 0x16 | 0x17 | 0x1d | 0x24 | 0x2b | 0x32 | 0x39 | 0x40 | 0x47 | 0x4e => Ok(2),
        0x11 | 0x18 => Ok(3),
        _ if is_i64_atomic_rmw(sub) => Ok(3),
        _ if is_i32_atomic_rmw(sub) => Ok(2),
        _ => Err(format!("unsupported atomic subopcode 0x{:02x}", sub)),
    }
}

fn validate_atomic_memarg(
    memory_shared: bool,
    sub: u32,
    memarg: MemArg,
    memories: &[Limits],
) -> Result<(), String> {
    validate_memarg_memory_offset(memarg, memories)?;
    if !memory_shared {
        return Err("atomic memory operation requires shared memory".to_string());
    }
    let expected = atomic_expected_align(sub)?;
    if memarg.align != expected {
        if sub == 0x20 {
            return Err(format!(
                "i32.atomic.rmw.add8_u: invalid alignment; expected maximum alignment is {}, actual alignment is {}",
                expected, memarg.align
            ));
        }
        if sub == 0x21 {
            return Err(format!(
                "i32.atomic.rmw.add16_u: invalid alignment for atomic operation; expected alignment is {}, actual alignment is {}",
                expected, memarg.align
            ));
        }
        if expected == 0 && memarg.align > expected {
            return Err(format!(
                "invalid alignment; expected maximum alignment is {}, actual alignment is {}",
                expected, memarg.align
            ));
        }
        if sub == 0x1e {
            return Err(format!(
                "i32.atomic.rmw.add: invalid alignment for atomic operation; expected alignment is {}, actual alignment is {}",
                expected, memarg.align
            ));
        }
        return Err(format!(
            "invalid alignment for atomic operation; expected alignment is {}, actual alignment is {}",
            expected, memarg.align
        ));
    }
    Ok(())
}

fn validate_atomic_fence(memory_shared: bool, reserved: u8) -> Result<(), String> {
    if !memory_shared {
        return Err("atomic memory operation requires shared memory".to_string());
    }
    if reserved != 0 {
        return Err("invalid atomic operand".to_string());
    }
    Ok(())
}

fn apply_numeric_stack(op: u8, stack: &mut Vec<ValType>) -> Result<(), String> {
    match op {
        0x45 => unary(stack, ValType::I32, ValType::I32),
        0x46..=0x4f => binary(stack, ValType::I32, ValType::I32),
        0x50 => unary(stack, ValType::I64, ValType::I32),
        0x51..=0x5a => binary(stack, ValType::I64, ValType::I32),
        0x5b..=0x60 => binary(stack, ValType::F32, ValType::I32),
        0x61..=0x66 => binary(stack, ValType::F64, ValType::I32),
        0x67..=0x69 | 0xc0 | 0xc1 => unary(stack, ValType::I32, ValType::I32),
        0x6a..=0x78 => binary(stack, ValType::I32, ValType::I32),
        0x79..=0x7b | 0xc2..=0xc4 => unary(stack, ValType::I64, ValType::I64),
        0x7c..=0x8a => binary(stack, ValType::I64, ValType::I64),
        0x8b..=0x91 => unary(stack, ValType::F32, ValType::F32),
        0x92..=0x98 => binary(stack, ValType::F32, ValType::F32),
        0x99..=0x9f => unary(stack, ValType::F64, ValType::F64),
        0xa0..=0xa6 => binary(stack, ValType::F64, ValType::F64),
        0xa7 => unary(stack, ValType::I64, ValType::I32),
        0xa8 | 0xa9 => unary(stack, ValType::F32, ValType::I32),
        0xaa | 0xab => unary(stack, ValType::F64, ValType::I32),
        0xac | 0xad => unary(stack, ValType::I32, ValType::I64),
        0xae | 0xaf => unary(stack, ValType::F32, ValType::I64),
        0xb0 | 0xb1 => unary(stack, ValType::F64, ValType::I64),
        0xb2 | 0xb3 => unary(stack, ValType::I32, ValType::F32),
        0xb4 | 0xb5 => unary(stack, ValType::I64, ValType::F32),
        0xb6 => unary(stack, ValType::F64, ValType::F32),
        0xb7 | 0xb8 => unary(stack, ValType::I32, ValType::F64),
        0xb9 | 0xba => unary(stack, ValType::I64, ValType::F64),
        0xbb => unary(stack, ValType::F32, ValType::F64),
        0xbc => unary(stack, ValType::F32, ValType::I32),
        0xbd => unary(stack, ValType::F64, ValType::I64),
        0xbe => unary(stack, ValType::I32, ValType::F32),
        0xbf => unary(stack, ValType::I64, ValType::F64),
        _ => Err(format!(
            "unsupported numeric validation opcode 0x{:02x}",
            op
        )),
    }
}

fn is_extended_const_numeric_op(op: u8) -> bool {
    matches!(op, 0x6a..=0x6c | 0x7c..=0x7e)
}

fn is_untyped_select_value_type(ty: ValType) -> bool {
    matches!(
        ty,
        ValType::I32
            | ValType::I64
            | ValType::F32
            | ValType::F64
            | ValType::V128
            | ValType::Unknown
    )
}

fn is_ref_type(ty: ValType) -> bool {
    matches!(
        ty,
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
            | ValType::Unknown
    )
}

fn apply_trunc_sat_stack(sub: u32, stack: &mut Vec<ValType>) -> Result<(), String> {
    match sub {
        0 | 1 => unary(stack, ValType::F32, ValType::I32),
        2 | 3 => unary(stack, ValType::F64, ValType::I32),
        4 | 5 => unary(stack, ValType::F32, ValType::I64),
        6 | 7 => unary(stack, ValType::F64, ValType::I64),
        _ => Err(format!(
            "unsupported trunc_sat validation subopcode {}",
            sub
        )),
    }
}

fn apply_numeric_stack_control(
    op: u8,
    stack: &mut Vec<ValType>,
    labels: &[ControlFrame],
) -> Result<(), String> {
    match op {
        0x45 => unary_control(stack, labels, ValType::I32, ValType::I32),
        0x46..=0x4f => binary_control(stack, labels, ValType::I32, ValType::I32),
        0x50 => unary_control(stack, labels, ValType::I64, ValType::I32),
        0x51..=0x5a => binary_control(stack, labels, ValType::I64, ValType::I32),
        0x5b..=0x60 => binary_control(stack, labels, ValType::F32, ValType::I32),
        0x61..=0x66 => binary_control(stack, labels, ValType::F64, ValType::I32),
        0x67..=0x69 | 0xc0 | 0xc1 => unary_control(stack, labels, ValType::I32, ValType::I32),
        0x6a..=0x78 => binary_control(stack, labels, ValType::I32, ValType::I32),
        0x79..=0x7b | 0xc2..=0xc4 => unary_control(stack, labels, ValType::I64, ValType::I64),
        0x7c..=0x8a => binary_control(stack, labels, ValType::I64, ValType::I64),
        0x8b..=0x91 => unary_control(stack, labels, ValType::F32, ValType::F32),
        0x92..=0x98 => binary_control(stack, labels, ValType::F32, ValType::F32),
        0x99..=0x9f => unary_control(stack, labels, ValType::F64, ValType::F64),
        0xa0..=0xa6 => binary_control(stack, labels, ValType::F64, ValType::F64),
        0xa7 => unary_control(stack, labels, ValType::I64, ValType::I32),
        0xa8 | 0xa9 => unary_control(stack, labels, ValType::F32, ValType::I32),
        0xaa | 0xab => unary_control(stack, labels, ValType::F64, ValType::I32),
        0xac | 0xad => unary_control(stack, labels, ValType::I32, ValType::I64),
        0xae | 0xaf => unary_control(stack, labels, ValType::F32, ValType::I64),
        0xb0 | 0xb1 => unary_control(stack, labels, ValType::F64, ValType::I64),
        0xb2 | 0xb3 => unary_control(stack, labels, ValType::I32, ValType::F32),
        0xb4 | 0xb5 => unary_control(stack, labels, ValType::I64, ValType::F32),
        0xb6 => unary_control(stack, labels, ValType::F64, ValType::F32),
        0xb7 | 0xb8 => unary_control(stack, labels, ValType::I32, ValType::F64),
        0xb9 | 0xba => unary_control(stack, labels, ValType::I64, ValType::F64),
        0xbb => unary_control(stack, labels, ValType::F32, ValType::F64),
        0xbc => unary_control(stack, labels, ValType::F32, ValType::I32),
        0xbd => unary_control(stack, labels, ValType::F64, ValType::I64),
        0xbe => unary_control(stack, labels, ValType::I32, ValType::F32),
        0xbf => unary_control(stack, labels, ValType::I64, ValType::F64),
        _ => Err(format!(
            "unsupported numeric validation opcode 0x{:02x}",
            op
        )),
    }
}

fn apply_trunc_sat_stack_control(
    sub: u32,
    stack: &mut Vec<ValType>,
    labels: &[ControlFrame],
) -> Result<(), String> {
    match sub {
        0 | 1 => unary_control(stack, labels, ValType::F32, ValType::I32),
        2 | 3 => unary_control(stack, labels, ValType::F64, ValType::I32),
        4 | 5 => unary_control(stack, labels, ValType::F32, ValType::I64),
        6 | 7 => unary_control(stack, labels, ValType::F64, ValType::I64),
        _ => Err(format!(
            "unsupported trunc_sat validation subopcode {}",
            sub
        )),
    }
}

fn unary(stack: &mut Vec<ValType>, input: ValType, output: ValType) -> Result<(), String> {
    pop_expect(stack, input)?;
    stack.push(output);
    Ok(())
}

fn binary(stack: &mut Vec<ValType>, input: ValType, output: ValType) -> Result<(), String> {
    pop_expect(stack, input)?;
    pop_expect(stack, input)?;
    stack.push(output);
    Ok(())
}

fn unary_control(
    stack: &mut Vec<ValType>,
    labels: &[ControlFrame],
    input: ValType,
    output: ValType,
) -> Result<(), String> {
    pop_expect_control(stack, labels, input)?;
    stack.push(output);
    Ok(())
}

fn binary_control(
    stack: &mut Vec<ValType>,
    labels: &[ControlFrame],
    input: ValType,
    output: ValType,
) -> Result<(), String> {
    pop_expect_control(stack, labels, input)?;
    pop_expect_control(stack, labels, input)?;
    stack.push(output);
    Ok(())
}

fn const_expr_type(
    expr: &[Instr],
    globals: &[(ValType, bool)],
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
) -> Result<ValType, String> {
    let mut stack: Vec<ValType> = Vec::new();
    for ins in expr {
        match ins {
            Instr::I32Const(_) => stack.push(ValType::I32),
            Instr::I64Const(_) => stack.push(ValType::I64),
            Instr::F32Const(_) => stack.push(ValType::F32),
            Instr::F64Const(_) => stack.push(ValType::F64),
            Instr::V128Const(_) => stack.push(ValType::V128),
            Instr::RefNull(ref_ty) => stack.push(*ref_ty),
            Instr::RefFunc(_) => stack.push(ValType::NonNullFuncRef),
            Instr::GlobalGet(idx) => {
                let (global_ty, mutable) =
                    globals.get(*idx as usize).copied().ok_or_else(|| {
                        format!(
                            "constant expression global index {} not available ({} globals)",
                            idx,
                            globals.len()
                        )
                    })?;
                if mutable {
                    return Err(format!(
                        "constant expression global.get {} references mutable global",
                        idx
                    ));
                }
                stack.push(global_ty);
            }
            Instr::ArrayNew(type_idx) => {
                let array = array_type(*type_idx, array_types)?;
                pop_expect(&mut stack, ValType::I32)?;
                pop_expect(&mut stack, array.element)?;
                stack.push(ValType::Unknown);
            }
            Instr::ArrayNewDefault(type_idx) => {
                let _array = array_type(*type_idx, array_types)?;
                pop_expect(&mut stack, ValType::I32)?;
                stack.push(ValType::Unknown);
            }
            Instr::ArrayNewFixed(type_idx, count) => {
                let array = array_type(*type_idx, array_types)?;
                for _ in 0..*count {
                    pop_expect(&mut stack, array.element)?;
                }
                stack.push(ValType::Unknown);
            }
            Instr::RefI31 => {
                pop_expect(&mut stack, ValType::I32)?;
                stack.push(ValType::Unknown);
            }
            Instr::StructNew(type_idx) => {
                let ty = struct_type(*type_idx, struct_types)?;
                for field in ty.fields.iter().rev() {
                    pop_expect(&mut stack, field.ty)?;
                }
                stack.push(ValType::NonNullTypeRef(*type_idx));
            }
            Instr::StructNewDefault(type_idx) => {
                let _ty = struct_type(*type_idx, struct_types)?;
                stack.push(ValType::NonNullTypeRef(*type_idx));
            }
            Instr::AnyConvertExtern => {
                let ty = stack
                    .pop()
                    .ok_or_else(|| "any.convert_extern operand underflow".to_string())?;
                if !is_ref_type(ty) {
                    return Err(format!(
                        "any.convert_extern operand must be reference, got {:?}",
                        ty
                    ));
                }
                stack.push(ValType::Unknown);
            }
            Instr::ExternConvertAny => {
                let ty = stack
                    .pop()
                    .ok_or_else(|| "extern.convert_any operand underflow".to_string())?;
                if !is_ref_type(ty) {
                    return Err(format!(
                        "extern.convert_any operand must be reference, got {:?}",
                        ty
                    ));
                }
                stack.push(ValType::ExternRef);
            }
            Instr::Num(op) if is_extended_const_numeric_op(*op) => {
                apply_numeric_stack(*op, &mut stack)?
            }
            Instr::Num(op) => {
                return Err(format!(
                    "non-constant initializer numeric opcode 0x{:02x}",
                    op
                ))
            }
            Instr::End => break,
            other => return Err(format!("non-constant initializer instruction: {:?}", other)),
        }
    }
    match stack.len() {
        0 => Err("empty constant expression".to_string()),
        1 => Ok(stack[0]),
        _ => Err("constant expression produces multiple values".to_string()),
    }
}

fn validate_instr_indices(
    body: &[Instr],
    local_count: usize,
    func_count: usize,
    global_count: usize,
    table_types: &[TableType],
    memories: &[Limits],
    element_count: usize,
    data_count: usize,
    type_count: usize,
    array_types: &[Option<ArrayType>],
    struct_types: &[Option<StructType>],
    tags: &[Tag],
    declared_funcs: &[bool],
) -> Result<(), String> {
    let table_count = table_types.len();
    let memory_count = memories.len();
    for ins in body {
        match ins {
            Instr::Call(idx) | Instr::ReturnCall(idx) => {
                if *idx as usize >= func_count {
                    return Err(format!(
                        "call function index {} out of bounds ({} funcs)",
                        idx, func_count
                    ));
                }
            }
            Instr::CallIndirect(type_idx, table_idx) => {
                if *type_idx as usize >= type_count {
                    return Err(format!(
                        "call_indirect type index {} out of bounds ({} types)",
                        type_idx, type_count
                    ));
                }
                if *table_idx as usize >= table_count {
                    return Err(format!(
                        "call_indirect table index {} out of bounds ({} tables)",
                        table_idx, table_count
                    ));
                }
            }
            Instr::ReturnCallIndirect(type_idx, table_idx) => {
                if *type_idx as usize >= type_count {
                    return Err(format!(
                        "return_call_indirect type index {} out of bounds ({} types)",
                        type_idx, type_count
                    ));
                }
                if *table_idx as usize >= table_count {
                    return Err(format!(
                        "return_call_indirect table index {} out of bounds ({} tables)",
                        table_idx, table_count
                    ));
                }
            }
            Instr::CallRef(type_idx) | Instr::ReturnCallRef(type_idx) => {
                if *type_idx as usize >= type_count {
                    return Err(format!(
                        "call_ref/return_call_ref type index {} out of bounds ({} types)",
                        type_idx, type_count
                    ));
                }
            }
            Instr::StructNew(type_idx) | Instr::StructNewDefault(type_idx) => {
                if *type_idx as usize >= type_count {
                    return Err(format!(
                        "struct type index {} out of bounds ({} types)",
                        type_idx, type_count
                    ));
                }
                struct_type(*type_idx, struct_types)?;
            }
            Instr::StructGet(type_idx, field_idx)
            | Instr::StructGetS(type_idx, field_idx)
            | Instr::StructGetU(type_idx, field_idx)
            | Instr::StructSet(type_idx, field_idx) => {
                if *type_idx as usize >= type_count {
                    return Err(format!(
                        "struct type index {} out of bounds ({} types)",
                        type_idx, type_count
                    ));
                }
                let ty = struct_type(*type_idx, struct_types)?;
                if *field_idx as usize >= ty.fields.len() {
                    return Err(format!(
                        "struct field index {} out of bounds ({} fields)",
                        field_idx,
                        ty.fields.len()
                    ));
                }
            }
            Instr::ArrayNew(type_idx)
            | Instr::ArrayNewDefault(type_idx)
            | Instr::ArrayNewFixed(type_idx, _)
            | Instr::ArrayGet(type_idx)
            | Instr::ArrayGetS(type_idx)
            | Instr::ArrayGetU(type_idx)
            | Instr::ArraySet(type_idx) => {
                if *type_idx as usize >= type_count {
                    return Err(format!(
                        "array type index {} out of bounds ({} types)",
                        type_idx, type_count
                    ));
                }
                if !matches!(array_types.get(*type_idx as usize), Some(Some(_))) {
                    return Err(format!("type index {} is not an array type", type_idx));
                }
            }
            Instr::ArrayNewData(type_idx, data_idx) => {
                if *type_idx as usize >= type_count {
                    return Err(format!(
                        "array type index {} out of bounds ({} types)",
                        type_idx, type_count
                    ));
                }
                if !matches!(array_types.get(*type_idx as usize), Some(Some(_))) {
                    return Err(format!("type index {} is not an array type", type_idx));
                }
                if *data_idx as usize >= data_count {
                    return Err(format!(
                        "array.new_data data index {} out of bounds ({} data segments)",
                        data_idx, data_count
                    ));
                }
            }
            Instr::ArrayNewElem(type_idx, elem_idx) => {
                if *type_idx as usize >= type_count {
                    return Err(format!(
                        "array type index {} out of bounds ({} types)",
                        type_idx, type_count
                    ));
                }
                if !matches!(array_types.get(*type_idx as usize), Some(Some(_))) {
                    return Err(format!("type index {} is not an array type", type_idx));
                }
                if *elem_idx as usize >= element_count {
                    return Err(format!(
                        "array.new_elem element index {} out of bounds ({} element segments)",
                        elem_idx, element_count
                    ));
                }
            }
            Instr::ArrayFill(type_idx) => {
                if *type_idx as usize >= type_count {
                    return Err(format!(
                        "array type index {} out of bounds ({} types)",
                        type_idx, type_count
                    ));
                }
                if !matches!(array_types.get(*type_idx as usize), Some(Some(_))) {
                    return Err(format!("type index {} is not an array type", type_idx));
                }
            }
            Instr::ArrayCopy(dst_type_idx, src_type_idx) => {
                for type_idx in [dst_type_idx, src_type_idx] {
                    if *type_idx as usize >= type_count {
                        return Err(format!(
                            "array type index {} out of bounds ({} types)",
                            type_idx, type_count
                        ));
                    }
                    if !matches!(array_types.get(*type_idx as usize), Some(Some(_))) {
                        return Err(format!("type index {} is not an array type", type_idx));
                    }
                }
            }
            Instr::ArrayInitData(type_idx, data_idx) => {
                if *type_idx as usize >= type_count {
                    return Err(format!(
                        "array type index {} out of bounds ({} types)",
                        type_idx, type_count
                    ));
                }
                if !matches!(array_types.get(*type_idx as usize), Some(Some(_))) {
                    return Err(format!("type index {} is not an array type", type_idx));
                }
                if *data_idx as usize >= data_count {
                    return Err(format!(
                        "array.init_data data index {} out of bounds ({} data segments)",
                        data_idx, data_count
                    ));
                }
            }
            Instr::ArrayInitElem(type_idx, elem_idx) => {
                if *type_idx as usize >= type_count {
                    return Err(format!(
                        "array type index {} out of bounds ({} types)",
                        type_idx, type_count
                    ));
                }
                if !matches!(array_types.get(*type_idx as usize), Some(Some(_))) {
                    return Err(format!("type index {} is not an array type", type_idx));
                }
                if *elem_idx as usize >= element_count {
                    return Err(format!(
                        "array.init_elem element index {} out of bounds ({} element segments)",
                        elem_idx, element_count
                    ));
                }
            }
            Instr::Throw(tag_idx) => {
                if *tag_idx as usize >= tags.len() {
                    return Err(format!(
                        "throw tag index {} out of bounds ({} tags)",
                        tag_idx,
                        tags.len()
                    ));
                }
            }
            Instr::LegacyCatch(tag_idx) => {
                if *tag_idx as usize >= tags.len() {
                    return Err(format!(
                        "legacy catch tag index {} out of bounds ({} tags)",
                        tag_idx,
                        tags.len()
                    ));
                }
            }
            Instr::TryTable(block_type, handlers) => {
                validate_block_type_ref(*block_type, type_count)
                    .map_err(|err| format!("try_table block type: {}", err))?;
                if let BlockType::TypeIndex(type_idx) = block_type {
                    if *type_idx as usize >= type_count {
                        return Err(format!(
                            "block type index {} out of bounds ({} types)",
                            type_idx, type_count
                        ));
                    }
                }
                for handler in handlers {
                    match handler {
                        CatchKind::Catch { tag, .. } | CatchKind::CatchRef { tag, .. } => {
                            if *tag as usize >= tags.len() {
                                return Err(format!(
                                    "try_table catch tag index {} out of bounds ({} tags)",
                                    tag,
                                    tags.len()
                                ));
                            }
                        }
                        CatchKind::CatchAll { .. } | CatchKind::CatchAllRef { .. } => {}
                    }
                }
            }
            Instr::Block(BlockType::TypeIndex(type_idx))
            | Instr::Loop(BlockType::TypeIndex(type_idx))
            | Instr::If(BlockType::TypeIndex(type_idx))
            | Instr::LegacyTry(BlockType::TypeIndex(type_idx)) => {
                if *type_idx as usize >= type_count {
                    return Err(format!(
                        "block type index {} out of bounds ({} types)",
                        type_idx, type_count
                    ));
                }
            }
            Instr::Block(block_type)
            | Instr::Loop(block_type)
            | Instr::If(block_type)
            | Instr::LegacyTry(block_type) => {
                validate_block_type_ref(*block_type, type_count)
                    .map_err(|err| format!("block type: {}", err))?;
            }
            Instr::LegacyCatchAll | Instr::LegacyRethrow(_) | Instr::LegacyDelegate(_) => {}
            Instr::TableGet(idx)
            | Instr::TableSet(idx)
            | Instr::TableGrow(idx)
            | Instr::TableSize(idx)
            | Instr::TableFill(idx) => {
                if *idx as usize >= table_count {
                    return Err(format!(
                        "table instruction index {} out of bounds ({} tables)",
                        idx, table_count
                    ));
                }
            }
            Instr::TableInit(elemidx, tableidx) => {
                if *elemidx as usize >= element_count {
                    return Err(format!(
                        "table.init element index {} out of bounds ({} elements)",
                        elemidx, element_count
                    ));
                }
                if *tableidx as usize >= table_count {
                    return Err(format!(
                        "table.init table index {} out of bounds ({} tables)",
                        tableidx, table_count
                    ));
                }
            }
            Instr::ElemDrop(elemidx) => {
                if *elemidx as usize >= element_count {
                    return Err(format!(
                        "elem.drop element index {} out of bounds ({} elements)",
                        elemidx, element_count
                    ));
                }
            }
            Instr::TableCopy(dst, src) => {
                if *dst as usize >= table_count || *src as usize >= table_count {
                    return Err(format!(
                        "table.copy table index out of bounds (dst {}, src {}, {} tables)",
                        dst, src, table_count
                    ));
                }
            }
            Instr::Load(_, memarg)
            | Instr::Store(_, memarg)
            | Instr::AtomicLoad(_, memarg)
            | Instr::AtomicStore(_, memarg)
            | Instr::AtomicRmw(_, memarg)
            | Instr::AtomicNotify(memarg)
            | Instr::AtomicWait(_, memarg) => {
                if memarg.memory as usize >= memory_count {
                    return Err(format!(
                        "memory instruction index {} out of bounds ({} memories)",
                        memarg.memory, memory_count
                    ));
                }
            }
            Instr::MemorySize(memoryidx) | Instr::MemoryGrow(memoryidx) => {
                if *memoryidx as usize >= memory_count {
                    return Err(format!(
                        "memory instruction index {} out of bounds ({} memories)",
                        memoryidx, memory_count
                    ));
                }
            }
            Instr::MemoryInit(_, memoryidx) | Instr::MemoryFill(memoryidx) => {
                if *memoryidx as usize >= memory_count {
                    return Err(format!(
                        "memory instruction index {} out of bounds ({} memories)",
                        memoryidx, memory_count
                    ));
                }
            }
            Instr::MemoryCopy(dst, src) => {
                if *dst as usize >= memory_count || *src as usize >= memory_count {
                    return Err(format!(
                        "memory.copy memory index out of bounds (dst {}, src {}, {} memories)",
                        dst, src, memory_count
                    ));
                }
            }
            Instr::RefFunc(idx) => {
                if *idx as usize >= func_count {
                    return Err(format!(
                        "ref.func function index {} out of bounds ({} funcs)",
                        idx, func_count
                    ));
                }
                if !declared_funcs.get(*idx as usize).copied().unwrap_or(false) {
                    return Err(format!("undeclared ref.func function index {}", idx));
                }
            }
            Instr::RefNull(ty) | Instr::SelectTyped(ty) => {
                validate_val_type_ref(*ty, type_count)?;
            }
            Instr::RefTest { target, .. } | Instr::RefCast { target, .. } => {
                validate_val_type_ref(*target, type_count)?;
            }
            Instr::BrOnCast { source, target, .. } | Instr::BrOnCastFail { source, target, .. } => {
                validate_val_type_ref(*source, type_count)?;
                validate_val_type_ref(*target, type_count)?;
            }
            Instr::RefEq => {}
            Instr::LocalGet(idx) | Instr::LocalSet(idx) | Instr::LocalTee(idx) => {
                if *idx as usize >= local_count {
                    return Err(format!(
                        "local index {} out of bounds ({} locals)",
                        idx, local_count
                    ));
                }
            }
            Instr::GlobalGet(idx) | Instr::GlobalSet(idx) => {
                if *idx as usize >= global_count {
                    return Err(format!(
                        "global index {} out of bounds ({} globals)",
                        idx, global_count
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn section_order(id: u8) -> Result<i32, String> {
    match id {
        1..=5 => Ok(id as i32),
        13 => Ok(6),
        6 => Ok(7),
        7..=9 => Ok(id as i32 + 1),
        12 => Ok(11),
        10 => Ok(12),
        11 => Ok(13),
        other => Err(format!("unknown section id {}", other)),
    }
}

fn parse_type_section(r: &mut Reader, m: &mut Module) -> Result<(), String> {
    let n = r.u32()?;
    for _ in 0..n {
        parse_type_declaration(r, m)?;
    }
    Ok(())
}

fn parse_type_declaration(r: &mut Reader, m: &mut Module) -> Result<(), String> {
    let visible_type_count = m.types.len() + 1;
    parse_type_declaration_in_group(r, m, None, true, visible_type_count)
}

fn push_type_group(m: &mut Module, group_id: Option<u32>) {
    let idx = m.types.len() as u32;
    m.type_rec_groups.push(group_id.unwrap_or(idx));
}

fn parse_type_declaration_in_group(
    r: &mut Reader,
    m: &mut Module,
    group_id: Option<u32>,
    default_final: bool,
    visible_type_count: usize,
) -> Result<(), String> {
    let form = r.byte()?;
    match form {
        0x60 => {
            let func_type = read_func_type_after_form(r)?;
            validate_func_type_ref_limit(&func_type, visible_type_count)?;
            push_type_group(m, group_id);
            m.types.push(func_type);
            m.type_is_func.push(true);
            m.type_supertypes.push(Vec::new());
            m.type_is_final.push(default_final);
            m.array_types.push(None);
            m.struct_types.push(None);
        }
        0x4e => {
            let n = r.u32()?;
            let group_id = m.types.len() as u32;
            let visible_type_count = m.types.len() + n as usize;
            for _ in 0..n {
                parse_type_declaration_in_group(r, m, Some(group_id), true, visible_type_count)?;
            }
        }
        0x4f | 0x50 => {
            let is_final = form == 0x4f;
            let supertype_count = r.u32()?;
            let mut supertypes = Vec::with_capacity(supertype_count as usize);
            for _ in 0..supertype_count {
                supertypes.push(r.u32()?);
            }
            let start = m.types.len();
            parse_type_declaration_in_group(r, m, group_id, true, visible_type_count)?;
            if m.types.len() != start + 1 {
                return Err("subtype declaration must wrap one type".to_string());
            }
            if let Some(slot) = m.type_supertypes.get_mut(start) {
                *slot = supertypes;
            }
            if let Some(slot) = m.type_is_final.get_mut(start) {
                *slot = is_final;
            }
        }
        0x5f => {
            let struct_type = read_struct_type_after_form(r)?;
            validate_struct_type_ref_limit(&struct_type, visible_type_count)?;
            push_type_group(m, group_id);
            m.types.push(FuncType {
                params: Vec::new(),
                results: Vec::new(),
            });
            m.type_is_func.push(false);
            m.type_supertypes.push(Vec::new());
            m.type_is_final.push(default_final);
            m.array_types.push(None);
            m.struct_types.push(Some(struct_type));
        }
        0x5e => {
            let array_type = read_array_type_after_form(r)?;
            validate_val_type_ref(array_type.element, visible_type_count)?;
            push_type_group(m, group_id);
            m.types.push(FuncType {
                params: Vec::new(),
                results: Vec::new(),
            });
            m.type_is_func.push(false);
            m.type_supertypes.push(Vec::new());
            m.type_is_final.push(default_final);
            m.array_types.push(Some(array_type));
            m.struct_types.push(None);
        }
        other => return Err(format!("expected functype form 0x60, got 0x{:02x}", other)),
    }
    Ok(())
}

fn validate_func_type_ref_limit(ft: &FuncType, visible_type_count: usize) -> Result<(), String> {
    for ty in ft.params.iter().chain(ft.results.iter()) {
        validate_val_type_ref(*ty, visible_type_count)?;
    }
    Ok(())
}

fn validate_struct_type_ref_limit(
    strukt: &StructType,
    visible_type_count: usize,
) -> Result<(), String> {
    for field in &strukt.fields {
        validate_val_type_ref(field.ty, visible_type_count)?;
    }
    Ok(())
}

fn read_struct_type_after_form(r: &mut Reader) -> Result<StructType, String> {
    let fields = r.u32()?;
    let mut out = Vec::with_capacity(fields as usize);
    for _ in 0..fields {
        let (ty, packed_bits) = read_storage_type(r)?;
        let mutability = r.byte()?;
        if mutability > 1 {
            return Err(format!("bad struct field mutability 0x{:02x}", mutability));
        }
        out.push(StructField {
            ty,
            mutable: mutability != 0,
            packed_bits,
        });
    }
    Ok(StructType { fields: out })
}

fn read_array_type_after_form(r: &mut Reader) -> Result<ArrayType, String> {
    let (element, packed_bits) = read_storage_type(r)?;
    let mutability = r.byte()?;
    if mutability > 1 {
        return Err(format!("bad array field mutability 0x{:02x}", mutability));
    }
    Ok(ArrayType {
        element,
        mutable: mutability != 0,
        packed_bits,
    })
}

fn read_storage_type(r: &mut Reader) -> Result<(ValType, Option<u8>), String> {
    match r.byte()? {
        0x78 => Ok((ValType::I32, Some(8))),
        0x77 => Ok((ValType::I32, Some(16))),
        b @ (0x64 | 0x63) => read_val_type_from_first(b, r).map(|ty| (ty, None)),
        b => ValType::from_byte(b).map(|ty| (ty, None)),
    }
}

fn read_func_type_after_form(r: &mut Reader) -> Result<FuncType, String> {
    let np = r.u32()? as usize;
    let mut params = Vec::with_capacity(np);
    for _ in 0..np {
        params.push(read_val_type(r)?);
    }
    let nr = r.u32()? as usize;
    let mut results = Vec::with_capacity(nr);
    for _ in 0..nr {
        results.push(read_val_type(r)?);
    }
    Ok(FuncType { params, results })
}

fn parse_import_section(r: &mut Reader, m: &mut Module) -> Result<(), String> {
    let n = r.u32()?;
    for _ in 0..n {
        let module = r.name()?;
        let name = r.name()?;
        let kind_byte = r.byte()?;
        let kind = match kind_byte {
            0x00 => {
                let t = r.u32()?;
                m.imported_func_count += 1;
                ImportKind::Func(t)
            }
            0x01 => {
                let elem = read_ref_type(r)?;
                let limits = read_limits(r, LimitContext::Table, true)?;
                let ty = TableType { elem, limits };
                m.tables.push(ty);
                ImportKind::Table(ty)
            }
            0x02 => {
                let lim = read_limits(r, LimitContext::Memory, true)?;
                m.memories.push(lim);
                ImportKind::Memory(lim)
            }
            0x03 => {
                let ty = read_val_type(r)?;
                let mutb = r.byte()?;
                if mutb > 1 {
                    return Err(format!("bad global mutability 0x{:02x}", mutb));
                }
                ImportKind::Global {
                    ty,
                    mutable: mutb == 1,
                }
            }
            0x04 => {
                let attribute = r.u32()?;
                let type_idx = r.u32()?;
                m.tags.push(Tag {
                    attribute,
                    type_idx,
                });
                ImportKind::Tag(type_idx)
            }
            other => return Err(format!("bad import kind 0x{:02x}", other)),
        };
        m.imports.push(Import { module, name, kind });
    }
    Ok(())
}

fn parse_function_section(r: &mut Reader, m: &mut Module) -> Result<(), String> {
    let n = r.u32()?;
    for _ in 0..n {
        m.func_types.push(r.u32()?);
    }
    Ok(())
}

fn parse_table_section(r: &mut Reader, m: &mut Module) -> Result<(), String> {
    let n = r.u32()?;
    for _ in 0..n {
        let first = r.byte()?;
        let has_init_expr = first == 0x40;
        let elem = if has_init_expr {
            let reserved = r.byte()?;
            if reserved != 0 {
                return Err(format!("bad table init reserved byte 0x{:02x}", reserved));
            }
            read_ref_type(r)?
        } else {
            read_ref_type_from_first(first, r)?
        };
        let limits = read_limits(r, LimitContext::Table, true)?;
        let init = if has_init_expr {
            Some(decode_expr(r)?)
        } else {
            None
        };
        m.tables.push(TableType { elem, limits });
        m.table_inits.push(init);
    }
    Ok(())
}

fn parse_memory_section(r: &mut Reader, m: &mut Module) -> Result<(), String> {
    let n = r.u32()?;
    for _ in 0..n {
        m.memories.push(read_limits(r, LimitContext::Memory, true)?);
    }
    Ok(())
}

fn parse_global_section(r: &mut Reader, m: &mut Module) -> Result<(), String> {
    let n = r.u32()?;
    for _ in 0..n {
        let ty = read_val_type(r)?;
        let mutb = r.byte()?;
        if mutb > 1 {
            return Err(format!("bad global mutability 0x{:02x}", mutb));
        }
        let init = decode_expr(r)?;
        m.globals.push(Global {
            ty,
            mutable: mutb == 1,
            init,
        });
    }
    Ok(())
}

fn parse_tag_section(r: &mut Reader, m: &mut Module) -> Result<(), String> {
    let n = r.u32()?;
    for _ in 0..n {
        let attribute = r.u32()?;
        let type_idx = r.u32()?;
        m.tags.push(Tag {
            attribute,
            type_idx,
        });
    }
    Ok(())
}

fn parse_export_section(r: &mut Reader, m: &mut Module) -> Result<(), String> {
    let n = r.u32()?;
    for _ in 0..n {
        let name = r.name()?;
        let kind_byte = r.byte()?;
        let kind = match kind_byte {
            0x00 => ExportKind::Func,
            0x01 => ExportKind::Table,
            0x02 => ExportKind::Memory,
            0x03 => ExportKind::Global,
            0x04 => ExportKind::Tag,
            other => return Err(format!("bad export kind 0x{:02x}", other)),
        };
        let index = r.u32()?;
        m.exports.push(Export { name, kind, index });
    }
    Ok(())
}

fn parse_element_section(r: &mut Reader, m: &mut Module) -> Result<(), String> {
    let n = r.u32()?;
    for _ in 0..n {
        let flags = r.u32()?;
        let seg = match flags {
            0 => {
                let offset = decode_expr(r)?;
                ElementSegment {
                    table: 0,
                    offset: Some(offset),
                    items: read_elem_func_indices(r)?,
                    mode: ElementMode::Active,
                    ty: ValType::FuncRef,
                }
            }
            1 => {
                read_elem_kind(r)?;
                ElementSegment {
                    table: 0,
                    offset: None,
                    items: read_elem_func_indices(r)?,
                    mode: ElementMode::Passive,
                    ty: ValType::FuncRef,
                }
            }
            2 => {
                let table = r.u32()?;
                let offset = decode_expr(r)?;
                read_elem_kind(r)?;
                ElementSegment {
                    table,
                    offset: Some(offset),
                    items: read_elem_func_indices(r)?,
                    mode: ElementMode::Active,
                    ty: ValType::FuncRef,
                }
            }
            3 => {
                read_elem_kind(r)?;
                ElementSegment {
                    table: 0,
                    offset: None,
                    items: read_elem_func_indices(r)?,
                    mode: ElementMode::Declarative,
                    ty: ValType::FuncRef,
                }
            }
            4 => {
                let offset = decode_expr(r)?;
                ElementSegment {
                    table: 0,
                    offset: Some(offset),
                    items: read_elem_exprs(r)?,
                    mode: ElementMode::Active,
                    ty: ValType::FuncRef,
                }
            }
            5 => {
                let ty = read_ref_type(r)?;
                ElementSegment {
                    table: 0,
                    offset: None,
                    items: read_elem_exprs(r)?,
                    mode: ElementMode::Passive,
                    ty,
                }
            }
            6 => {
                let table = r.u32()?;
                let offset = decode_expr(r)?;
                let ty = read_ref_type(r)?;
                ElementSegment {
                    table,
                    offset: Some(offset),
                    items: read_elem_exprs(r)?,
                    mode: ElementMode::Active,
                    ty,
                }
            }
            7 => {
                let ty = read_ref_type(r)?;
                ElementSegment {
                    table: 0,
                    offset: None,
                    items: read_elem_exprs(r)?,
                    mode: ElementMode::Declarative,
                    ty,
                }
            }
            other => return Err(format!("unsupported element segment flags {}", other)),
        };
        m.elements.push(seg);
    }
    Ok(())
}

fn read_elem_kind(r: &mut Reader) -> Result<(), String> {
    let kind = r.byte()?;
    if kind == 0x00 {
        Ok(())
    } else {
        Err(format!("unsupported element kind 0x{:02x}", kind))
    }
}

fn read_elem_func_indices(r: &mut Reader) -> Result<Vec<ElementItem>, String> {
    let cnt = r.u32()? as usize;
    let mut items = Vec::with_capacity(cnt);
    for _ in 0..cnt {
        items.push(ElementItem::Func(r.u32()?));
    }
    Ok(items)
}

fn read_elem_exprs(r: &mut Reader) -> Result<Vec<ElementItem>, String> {
    let cnt = r.u32()? as usize;
    let mut items = Vec::with_capacity(cnt);
    for _ in 0..cnt {
        items.push(ElementItem::Expr(decode_expr(r)?));
    }
    Ok(items)
}

fn parse_code_section(r: &mut Reader, m: &mut Module) -> Result<(), String> {
    let n = r.u32()?;
    for _ in 0..n {
        let body_size = r.u32()? as usize;
        let body_bytes = r.bytes_n(body_size)?;
        let mut br = Reader::new(body_bytes);
        let local_decl_count = br.u32()?;
        let mut locals = Vec::new();
        for _ in 0..local_decl_count {
            let count = br.u32()? as usize;
            let ty = read_val_type(&mut br)?;
            for _ in 0..count {
                locals.push(ty);
            }
        }
        let body = decode_expr(&mut br)?;
        m.code.push(Code { locals, body });
    }
    Ok(())
}

fn parse_data_section(r: &mut Reader, m: &mut Module) -> Result<(), String> {
    let n = r.u32()?;
    for _ in 0..n {

        let mode = r.u32()?;
        match mode {
            0 => {
                let offset = decode_expr(r)?;
                let len = r.u32()? as usize;
                let bytes = r.bytes_n(len)?.to_vec();
                m.data.push(DataSegment {
                    memory: 0,
                    offset: Some(offset),
                    bytes,
                    passive: false,
                });
            }
            1 => {
                let len = r.u32()? as usize;
                let bytes = r.bytes_n(len)?.to_vec();
                m.data.push(DataSegment {
                    memory: 0,
                    offset: None,
                    bytes,
                    passive: true,
                });
            }
            2 => {
                let memory = r.u32()?;
                let offset = decode_expr(r)?;
                let len = r.u32()? as usize;
                let bytes = r.bytes_n(len)?.to_vec();
                m.data.push(DataSegment {
                    memory,
                    offset: Some(offset),
                    bytes,
                    passive: false,
                });
            }
            other => return Err(format!("unsupported data segment mode {}", other)),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{instantiate, memory_imports, Imports, WasmValue};

    use super::parse_module;

    fn wasm_with_sections(sections: &[&[u8]]) -> Vec<u8> {
        let mut bytes = b"\0asm\x01\0\0\0".to_vec();
        for section in sections {
            bytes.extend_from_slice(section);
        }
        bytes
    }

    fn bulk_body(instr: &[u8]) -> Vec<u8> {
        let mut body = Vec::with_capacity(instr.len() + 2);
        body.push(0);
        body.extend_from_slice(instr);
        body.push(0x0b);

        let mut code = Vec::with_capacity(body.len() + 4);
        code.push(10);
        code.push((body.len() + 2) as u8);
        code.push(1);
        code.push(body.len() as u8);
        code.extend_from_slice(&body);
        code
    }

    fn bulk_module(instr: &[u8], data_count: Option<u8>, data: Option<&[u8]>) -> Vec<u8> {
        let type_sec = [1, 4, 1, 0x60, 0, 0];
        let func_sec = [3, 2, 1, 0];
        let memory_sec = [5, 3, 1, 0, 1];
        let code_sec = bulk_body(instr);
        let mut sections: Vec<&[u8]> = vec![&type_sec, &func_sec, &memory_sec];
        let data_count_sec;
        if let Some(count) = data_count {
            data_count_sec = [12, 1, count];
            sections.push(&data_count_sec);
        }
        sections.push(&code_sec);
        if let Some(data) = data {
            sections.push(data);
        }
        wasm_with_sections(&sections)
    }

    #[test]
    fn datacount_section_orders_before_code_despite_numeric_id() {
        let bytes = wasm_with_sections(&[
            &[12, 1, 0],
            &[10, 1, 0],
        ]);
        let module = parse_module(&bytes).expect("datacount before code parses");
        assert_eq!(module.data_count, Some(0));
        assert!(module.code.is_empty());
    }

    #[test]
    fn datacount_section_matches_data_segment_count() {
        let bytes = wasm_with_sections(&[
            &[12, 1, 1],
            &[11, 3, 1, 1, 0],
        ]);
        let module = parse_module(&bytes).expect("matching datacount parses");
        assert_eq!(module.data_count, Some(1));
        assert_eq!(module.data.len(), 1);
        assert!(module.data[0].passive);
    }

    #[test]
    fn datacount_section_rejects_data_segment_mismatch() {
        let bytes = wasm_with_sections(&[
            &[12, 1, 2],
            &[11, 3, 1, 1, 0],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("mismatch must reject"),
            Err(err) => err,
        };
        assert!(err.contains("data count mismatch"), "{err}");
    }

    #[test]
    fn validation_accepts_typed_ref_function_signature_forms() {
        let bytes = wasm_with_sections(&[
            &[
                0x01, 0x1b,
                0x02,
                0x60, 0x00, 0x00,
                0x60, 0x0c,
                0x70, 0x6f,
                0x64, 0x70,
                0x64, 0x6f,
                0x64, 0x00,
                0x64, 0x00,
                0x64, 0x00,
                0x64, 0x00,
                0x70, 0x6f,
                0x63, 0x00,
                0x63, 0x00,
                0x00,
            ],
            &[0x03, 0x02, 0x01, 0x01],
            &[0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b],
        ]);
        let module = parse_module(&bytes).expect("typed-ref function signature parses");
        assert_eq!(module.types.len(), 2);
        assert_eq!(module.types[1].params.len(), 12);
        assert_eq!(module.types[1].params[2], super::ValType::NonNullFuncRef);
        assert_eq!(module.types[1].params[3], super::ValType::NonNullExternRef);
        assert_eq!(module.types[1].params[4], super::ValType::NonNullTypeRef(0));
    }

    #[test]
    fn validation_rejects_unknown_typed_ref_function_signature() {
        let bytes = wasm_with_sections(&[
            &[0x01, 0x06, 0x01, 0x60, 0x01, 0x64, 0x01, 0x00],
            &[0x03, 0x02, 0x01, 0x00],
            &[0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("unknown typed-ref index must reject"),
            Err(err) => err,
        };
        assert!(err.contains("type index 1 out of bounds"), "{err}");
    }

    #[test]
    fn validation_rejects_plain_type_forward_reference_to_later_sibling() {
        let bytes = wasm_with_sections(&[
            &[
                0x01, 0x0a,
                0x02,
                0x60, 0x01, 0x64, 0x01, 0x00,
                0x60, 0x00, 0x00,
            ],
            &[0x03, 0x02, 0x01, 0x00],
            &[0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("plain type forward reference must reject"),
            Err(err) => err,
        };
        assert!(err.contains("type index 1 out of bounds"), "{err}");
    }

    #[test]
    fn validation_accepts_typed_ref_element_segment_item_compatibility() {
        let bytes = wasm_with_sections(&[
            &[0x01, 0x04, 0x01, 0x60, 0x00, 0x00],
            &[0x03, 0x02, 0x01, 0x00],
            &[0x04, 0x05, 0x01, 0x63, 0x00, 0x00, 0x02],
            &[
                0x09, 0x0c,
                0x01,
                0x06,
                0x00,
                0x41, 0x00, 0x0b,
                0x63, 0x00,
                0x01,
                0xd2, 0x00, 0x0b,
            ],
            &[0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b],
        ]);
        parse_module(&bytes).expect("typed-ref element segment item parses");
    }

    #[test]
    fn validation_accepts_i31ref_element_segment_for_anyref_table() {
        let bytes = wasm_with_sections(&[
            &[0x04, 0x04, 0x01, 0x6e, 0x00, 0x03],
            &[
                0x09, 0x0d,
                0x01,
                0x06,
                0x00,
                0x41, 0x00, 0x0b,
                0x6c,
                0x01,
                0x41, 0x07, 0xfb, 0x1c, 0x0b,
            ],
        ]);
        parse_module(&bytes).expect("i31ref element segment is compatible with anyref table");
    }

    #[test]
    fn validation_accepts_recursive_function_type_groups() {
        let bytes = wasm_with_sections(&[
            &[
                0x01, 0x10,
                0x02,
                0x60, 0x00, 0x00,
                0x4e, 0x02,
                0x60, 0x01, 0x64, 0x02, 0x00,
                0x60, 0x00, 0x01, 0x64, 0x01,
            ],
            &[0x03, 0x02, 0x01, 0x01],
            &[0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b],
        ]);
        let module = parse_module(&bytes).expect("recursive function group parses");
        assert_eq!(module.types.len(), 3);
        assert_eq!(module.type_is_func, vec![true, true, true]);
        assert_eq!(
            module.types[1].params,
            vec![super::ValType::NonNullTypeRef(2)]
        );
        assert_eq!(
            module.types[2].results,
            vec![super::ValType::NonNullTypeRef(1)]
        );
    }

    #[test]
    fn validation_accepts_recursive_struct_type_placeholders() {
        let bytes = wasm_with_sections(&[
            &[
                0x01, 0x08,
                0x01,
                0x4e, 0x02,
                0x60, 0x00, 0x00,
                0x5f, 0x00,
            ],
            &[0x03, 0x02, 0x01, 0x00],
            &[0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b],
        ]);
        let module = parse_module(&bytes).expect("recursive struct placeholder parses");
        assert_eq!(module.types.len(), 2);
        assert_eq!(module.type_is_func, vec![true, false]);
    }

    #[test]
    fn validation_accepts_gc_subtype_declaration_wrappers() {
        let bytes = wasm_with_sections(&[&[
            0x01, 0x2f,
            0x09,
            0x50, 0x00, 0x5f, 0x00,
            0x50, 0x00, 0x5f, 0x01, 0x7f, 0x00,
            0x5e, 0x78, 0x00,
            0x50, 0x01, 0x00, 0x5f, 0x00,
            0x50, 0x01, 0x00, 0x5f, 0x00,
            0x50, 0x01, 0x01, 0x5f, 0x01, 0x7f, 0x00,
            0x50, 0x01, 0x01, 0x5f, 0x01, 0x7f, 0x00,
            0x60, 0x00, 0x00,
            0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f,
        ]]);
        let module = parse_module(&bytes).expect("gc subtype wrappers parse");
        assert_eq!(module.types.len(), 9);
        assert_eq!(
            module.type_is_func,
            vec![false, false, false, false, false, false, false, true, true]
        );
        assert_eq!(
            module.type_is_final,
            vec![false, false, true, false, false, false, false, true, true]
        );
        assert_eq!(module.type_supertypes[3], vec![0]);
        assert_eq!(module.type_supertypes[5], vec![1]);
    }

    #[test]
    fn validation_rejects_plain_struct_final_supertype() {
        let bytes = wasm_with_sections(&[&[
            0x01, 0x08,
            0x02,
            0x5f, 0x00,
            0x50, 0x01, 0x00, 0x5f, 0x00,
        ]]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("extension of a bare struct type must reject"),
            Err(err) => err,
        };
        assert!(err.contains("final explicit supertype"), "{err}");
    }

    #[test]
    fn validation_preserves_final_subtype_declaration() {
        let bytes = wasm_with_sections(&[&[
            0x01, 0x11,
            0x03,
            0x50, 0x00, 0x60, 0x00, 0x00,
            0x50, 0x01, 0x00, 0x60, 0x00, 0x00,
            0x4f, 0x00, 0x60, 0x00, 0x00,
        ]]);
        let module = parse_module(&bytes).expect("final subtype wrapper parses");
        assert_eq!(module.type_is_func, vec![true, true, true]);
        assert_eq!(module.type_supertypes, vec![vec![], vec![0], vec![]]);
        assert_eq!(module.type_is_final, vec![false, false, true]);
    }

    #[test]
    fn validation_accepts_array_type_placeholders() {
        let bytes = wasm_with_sections(&[&[
            0x01, 0x0f,
            0x04,
            0x5e, 0x78, 0x00,
            0x5e, 0x7f, 0x01,
            0x5e, 0x64, 0x6b, 0x00,
            0x5e, 0x63, 0x00, 0x01,
        ]]);
        let module = parse_module(&bytes).expect("array type placeholders parse");
        assert_eq!(module.types.len(), 4);
        assert_eq!(module.type_is_func, vec![false, false, false, false]);
    }

    #[test]
    fn validation_rejects_function_declared_with_struct_type() {
        let bytes = wasm_with_sections(&[
            &[0x01, 0x03, 0x01, 0x5f, 0x00],
            &[0x03, 0x02, 0x01, 0x00],
            &[0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("function over struct type must reject"),
            Err(err) => err,
        };
        assert!(err.contains("not a function type"), "{err}");
    }

    #[test]
    fn validation_accepts_typed_ref_blocktype() {
        let bytes = wasm_with_sections(&[
            &[0x01, 0x04, 0x01, 0x60, 0x00, 0x00],
            &[0x03, 0x02, 0x01, 0x00],
            &[
                0x0a, 0x0b,
                0x01,
                0x09,
                0x00,
                0x02, 0x63, 0x00,
                0xd0, 0x00,
                0x0b,
                0x1a,
                0x0b,
            ],
        ]);
        parse_module(&bytes).expect("typed-ref blocktype parses");
    }

    #[test]
    fn validation_accepts_table_copy_between_typed_func_refs() {
        let bytes = wasm_with_sections(&[
            &[0x01, 0x04, 0x01, 0x60, 0x00, 0x00],
            &[0x03, 0x02, 0x01, 0x00],
            &[
                0x04, 0x09,
                0x02,
                0x63, 0x70, 0x00, 0x01,
                0x63, 0x00, 0x00, 0x01,
            ],
            &[
                0x0a, 0x0e,
                0x01,
                0x0c,
                0x00,
                0x41, 0x00,
                0x41, 0x00,
                0x41, 0x00,
                0xfc, 0x0e, 0x00, 0x01,
                0x0b,
            ],
        ]);
        parse_module(&bytes).expect("typed table.copy parses");
    }

    #[test]
    fn validation_rejects_table_copy_between_func_and_extern_refs() {
        let bytes = wasm_with_sections(&[
            &[0x01, 0x04, 0x01, 0x60, 0x00, 0x00],
            &[0x03, 0x02, 0x01, 0x00],
            &[
                0x04, 0x07,
                0x02,
                0x70, 0x00, 0x01,
                0x6f, 0x00, 0x01,
            ],
            &[
                0x0a, 0x0e,
                0x01,
                0x0c,
                0x00,
                0x41, 0x00, 0x41, 0x00, 0x41, 0x00, 0xfc, 0x0e, 0x00, 0x01,
                0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("incompatible table.copy must reject"),
            Err(err) => err,
        };
        assert!(err.contains("table.copy"), "{err}");
    }

    #[test]
    fn validation_accepts_ref_as_non_null_call_ref_module() {
        let bytes = wasm_with_sections(&[
            &[
                0x01, 0x0b,
                0x02,
                0x60, 0x00, 0x01, 0x7f,
                0x60, 0x01, 0x63, 0x00, 0x01, 0x7f,
            ],
            &[0x03, 0x03, 0x02, 0x00, 0x01],
            &[
                0x0a, 0x0e,
                0x02,
                0x04, 0x00, 0x41, 0x07, 0x0b,
                0x07, 0x00, 0x20, 0x00, 0xd4, 0x14, 0x00, 0x0b,
            ],
        ]);
        let module = parse_module(&bytes).expect("ref.as_non_null/call_ref module parses");
        assert!(matches!(module.code[1].body[1], super::Instr::RefAsNonNull));
        assert!(matches!(module.code[1].body[2], super::Instr::CallRef(0)));
    }

    #[test]
    fn validation_rejects_call_ref_incompatible_callee_ref_type() {
        let bytes = wasm_with_sections(&[
            &[
                0x01, 0x0a,
                0x02,
                0x60, 0x00, 0x01, 0x7f,
                0x60, 0x01, 0x6f, 0x01, 0x7f,
            ],
            &[0x03, 0x03, 0x02, 0x00, 0x01],
            &[
                0x0a, 0x0d,
                0x02,
                0x04, 0x00, 0x41, 0x07, 0x0b,
                0x06, 0x00, 0x20, 0x00, 0x14, 0x00, 0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("call_ref incompatible callee must reject"),
            Err(err) => err,
        };
        assert!(err.contains("call_ref callee type mismatch"), "{err}");
    }

    #[test]
    fn validation_rejects_return_call_indirect_unreachable_suffix_result() {
        let bytes = [
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x60, 0x00, 0x00,
            0x03, 0x02, 0x01, 0x00, 0x04, 0x04, 0x01, 0x70, 0x00, 0x00, 0x0a, 0x0a, 0x01, 0x08,
            0x00, 0x41, 0x00, 0x13, 0x00, 0x00, 0x45, 0x0b, 0x00, 0x1a, 0x04, 0x6e, 0x61, 0x6d,
            0x65, 0x01, 0x13, 0x01, 0x00, 0x10, 0x74, 0x79, 0x70, 0x65, 0x2d, 0x76, 0x6f, 0x69,
            0x64, 0x2d, 0x76, 0x73, 0x2d, 0x6e, 0x75, 0x6d,
        ];
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("tail-call unreachable suffix result must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("extra stack values") || err.contains("fallthrough"),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_ref_func_initializer_nominal_subtype_mismatch() {
        let bytes = [
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x2d, 0x04, 0x4e, 0x02, 0x50,
            0x00, 0x60, 0x00, 0x00, 0x5f, 0x01, 0x64, 0x00, 0x00, 0x4e, 0x02, 0x50, 0x00, 0x60,
            0x00, 0x00, 0x5f, 0x01, 0x64, 0x00, 0x00, 0x4e, 0x02, 0x50, 0x01, 0x00, 0x60, 0x00,
            0x00, 0x5f, 0x00, 0x4e, 0x02, 0x50, 0x01, 0x02, 0x60, 0x00, 0x00, 0x5f, 0x00, 0x03,
            0x02, 0x01, 0x06, 0x06, 0x07, 0x01, 0x64, 0x04, 0x00, 0xd2, 0x00, 0x0b, 0x0a, 0x04,
            0x01, 0x02, 0x00, 0x0b, 0x00, 0x1e, 0x04, 0x6e, 0x61, 0x6d, 0x65, 0x01, 0x04, 0x01,
            0x00, 0x01, 0x67, 0x04, 0x11, 0x04, 0x00, 0x02, 0x66, 0x31, 0x02, 0x02, 0x66, 0x32,
            0x04, 0x02, 0x67, 0x31, 0x06, 0x02, 0x67, 0x32,
        ];
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("nominal subtype mismatch must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("initializer type") || err.contains("final explicit supertype"),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_cross_family_null_bottom_result() {
        let bytes = [
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
            0x01, 0x08, 0x01,
            0x60, 0x01, 0x63, 0x73, 0x01, 0x63, 0x71,
            0x03, 0x02, 0x01, 0x00,
            0x0a, 0x06, 0x01, 0x04, 0x00, 0x20, 0x00, 0x0b,
        ];
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("nofunc must not validate as none"),
            Err(err) => err,
        };
        assert!(err.contains("operand type mismatch"), "{err}");
    }

    #[test]
    fn validation_accepts_same_family_null_bottom_results() {
        let nofunc_to_func = [
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
            0x01, 0x08, 0x01,
            0x60, 0x01, 0x63, 0x73, 0x01, 0x63, 0x70,
            0x03, 0x02, 0x01, 0x00,
            0x0a, 0x06, 0x01, 0x04, 0x00, 0x20, 0x00, 0x0b,
        ];
        parse_module(&nofunc_to_func).expect("nofunc is valid as nullable func");

        let noextern_to_extern = [
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
            0x01, 0x08, 0x01,
            0x60, 0x01, 0x63, 0x72, 0x01, 0x63, 0x6f,
            0x03, 0x02, 0x01, 0x00,
            0x0a, 0x06, 0x01, 0x04, 0x00, 0x20, 0x00, 0x0b,
        ];
        parse_module(&noextern_to_extern).expect("noextern is valid as nullable extern");
    }

    #[test]
    fn validation_accepts_ref_func_initializer_recursive_type_equivalence() {
        let bytes = [
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x2d, 0x04, 0x4e, 0x02, 0x50,
            0x00, 0x60, 0x00, 0x00, 0x5f, 0x01, 0x64, 0x00, 0x00, 0x4e, 0x02, 0x50, 0x00, 0x60,
            0x00, 0x00, 0x5f, 0x01, 0x64, 0x02, 0x00, 0x4e, 0x02, 0x50, 0x01, 0x00, 0x60, 0x00,
            0x00, 0x5f, 0x00, 0x4e, 0x02, 0x50, 0x01, 0x02, 0x60, 0x00, 0x00, 0x5f, 0x00, 0x03,
            0x02, 0x01, 0x06, 0x06, 0x07, 0x01, 0x64, 0x04, 0x00, 0xd2, 0x00, 0x0b, 0x0a, 0x04,
            0x01, 0x02, 0x00, 0x0b, 0x00, 0x1e, 0x04, 0x6e, 0x61, 0x6d, 0x65, 0x01, 0x04, 0x01,
            0x00, 0x01, 0x67, 0x04, 0x11, 0x04, 0x00, 0x02, 0x66, 0x31, 0x02, 0x02, 0x66, 0x32,
            0x04, 0x02, 0x67, 0x31, 0x06, 0x02, 0x67, 0x32,
        ];
        parse_module(&bytes)
            .expect("recursive type-equivalent ref.func initializer should validate");
    }

    #[test]
    fn validation_accepts_ref_func_initializer_transitive_recursive_supertype() {
        let bytes = [
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x5f, 0x05, 0x4e, 0x02, 0x50,
            0x00, 0x60, 0x00, 0x00, 0x50, 0x00, 0x5f, 0x01, 0x64, 0x00, 0x00, 0x4e, 0x02, 0x50,
            0x00, 0x60, 0x00, 0x00, 0x50, 0x00, 0x5f, 0x01, 0x64, 0x02, 0x00, 0x4e, 0x02, 0x50,
            0x01, 0x00, 0x60, 0x00, 0x00, 0x50, 0x01, 0x01, 0x5f, 0x05, 0x64, 0x00, 0x00, 0x64,
            0x00, 0x00, 0x64, 0x02, 0x00, 0x64, 0x02, 0x00, 0x64, 0x04, 0x00, 0x4e, 0x02, 0x50,
            0x01, 0x02, 0x60, 0x00, 0x00, 0x50, 0x01, 0x03, 0x5f, 0x05, 0x64, 0x00, 0x00, 0x64,
            0x02, 0x00, 0x64, 0x00, 0x00, 0x64, 0x02, 0x00, 0x64, 0x06, 0x00, 0x4e, 0x02, 0x50,
            0x01, 0x06, 0x60, 0x00, 0x00, 0x5f, 0x00, 0x03, 0x02, 0x01, 0x08, 0x06, 0x0d, 0x02,
            0x64, 0x00, 0x00, 0xd2, 0x00, 0x0b, 0x64, 0x04, 0x00, 0xd2, 0x00, 0x0b, 0x0a, 0x04,
            0x01, 0x02, 0x00, 0x0b, 0x00, 0x29, 0x04, 0x6e, 0x61, 0x6d, 0x65, 0x01, 0x04, 0x01,
            0x00, 0x01, 0x68, 0x04, 0x1c, 0x07, 0x00, 0x02, 0x66, 0x31, 0x01, 0x02, 0x73, 0x31,
            0x02, 0x02, 0x66, 0x32, 0x03, 0x02, 0x73, 0x32, 0x04, 0x02, 0x67, 0x31, 0x06, 0x02,
            0x67, 0x32, 0x08, 0x01, 0x68,
        ];
        parse_module(&bytes).expect("transitive recursive ref.func initializer should validate");
    }

    #[test]
    fn validation_accepts_ref_as_non_null_return_call_ref_module() {
        let bytes = wasm_with_sections(&[
            &[
                0x01, 0x0b,
                0x02,
                0x60, 0x00, 0x01, 0x7f,
                0x60, 0x01, 0x63, 0x00, 0x01, 0x7f,
            ],
            &[0x03, 0x03, 0x02, 0x00, 0x01],
            &[
                0x0a, 0x0e,
                0x02,
                0x04, 0x00, 0x41, 0x07, 0x0b,
                0x07, 0x00, 0x20, 0x00, 0xd4, 0x15, 0x00, 0x0b,
            ],
        ]);
        let module = parse_module(&bytes).expect("return_call_ref module parses");
        assert!(matches!(module.code[1].body[1], super::Instr::RefAsNonNull));
        assert!(matches!(
            module.code[1].body[2],
            super::Instr::ReturnCallRef(0)
        ));
    }

    #[test]
    fn validation_accepts_return_call_ref_result_subtype_to_funcref() {
        let bytes = wasm_with_sections(&[
            &[
                0x01, 0x0d,
                0x03,
                0x60, 0x00, 0x00,
                0x60, 0x00, 0x01, 0x64, 0x00,
                0x60, 0x00, 0x01, 0x70,
            ],
            &[0x03, 0x04, 0x03, 0x00, 0x01, 0x02],
            &[0x09, 0x06, 0x01, 0x03, 0x00, 0x02, 0x00, 0x01],
            &[
                0x0a, 0x10,
                0x03,
                0x02, 0x00, 0x0b,
                0x04, 0x00, 0xd2, 0x00, 0x0b,
                0x06, 0x00, 0xd2, 0x01, 0x15, 0x01, 0x0b,
            ],
        ]);
        parse_module(&bytes).expect("return_call_ref result subtype validates");
    }

    #[test]
    fn validation_accepts_br_on_null_and_br_on_cast() {
        let br_on_null = wasm_with_sections(&[
            &[0x01, 0x05, 0x01, 0x60, 0x01, 0x6e, 0x00],
            &[0x03, 0x02, 0x01, 0x00],
            &[
                0x0a, 0x0c,
                0x01,
                0x0a,
                0x00,
                0x02, 0x40,
                0x20, 0x00,
                0xd5, 0x00,
                0x1a,
                0x0b,
                0x0b,
            ],
        ]);
        let module = parse_module(&br_on_null).expect("br_on_null module parses");
        assert!(matches!(module.code[0].body[2], super::Instr::BrOnNull(0)));

        let br_on_non_null = wasm_with_sections(&[
            &[0x01, 0x05, 0x01, 0x60, 0x01, 0x6e, 0x00],
            &[0x03, 0x02, 0x01, 0x00],
            &[
                0x0a, 0x0e,
                0x01,
                0x0c,
                0x00,
                0x02, 0x6e,
                0x20, 0x00,
                0xd6, 0x00,
                0xd0, 0x6e,
                0x0b,
                0x1a,
                0x0b,
            ],
        ]);
        let module = parse_module(&br_on_non_null).expect("br_on_non_null module parses");
        assert!(matches!(
            module.code[0].body[2],
            super::Instr::BrOnNonNull(0)
        ));

        let br_on_cast = wasm_with_sections(&[
            &[
                0x01, 0x16,
                0x04,
                0x5f, 0x00,
                0x60, 0x01, 0x64, 0x6e, 0x01, 0x64, 0x00,
                0x60, 0x01, 0x6e, 0x01, 0x64, 0x00,
                0x60, 0x01, 0x6e, 0x01, 0x63, 0x00,
            ],
            &[0x03, 0x04, 0x03, 0x01, 0x02, 0x03],
            &[
                0x0a, 0x2f,
                0x03,
                0x0f, 0x00, 0x02, 0x64, 0x6e, 0x20, 0x00, 0xfb, 0x18, 0x00, 0x01, 0x6e, 0x00, 0x0b,
                0x00, 0x0b, 0x0e, 0x00, 0x02, 0x6e, 0x20, 0x00, 0xfb, 0x18, 0x01, 0x01, 0x6e, 0x00,
                0x0b, 0x00, 0x0b, 0x0e, 0x00, 0x02, 0x6e, 0x20, 0x00, 0xfb, 0x18, 0x03, 0x01, 0x6e,
                0x00, 0x0b, 0x00, 0x0b,
            ],
        ]);
        let module = parse_module(&br_on_cast).expect("br_on_cast module parses");
        assert!(matches!(
            module.code[0].body[2],
            super::Instr::BrOnCast { depth: 1, .. }
        ));

        let br_on_cast_fail = wasm_with_sections(&[
            &[
                0x01, 0x0f,
                0x03,
                0x5f, 0x00,
                0x60, 0x01, 0x64, 0x6e, 0x01, 0x64, 0x6e,
                0x60, 0x01, 0x6e, 0x01, 0x6e,
            ],
            &[0x03, 0x03, 0x02, 0x01, 0x02],
            &[
                0x0a, 0x1e,
                0x02,
                0x0e, 0x00, 0x02, 0x64, 0x00, 0x20, 0x00, 0xfb, 0x19, 0x00, 0x01, 0x6e, 0x00, 0x0b,
                0x0b, 0x0d, 0x00, 0x02, 0x6e, 0x20, 0x00, 0xfb, 0x19, 0x01, 0x01, 0x6e, 0x00, 0x0b,
                0x0b,
            ],
        ]);
        let module = parse_module(&br_on_cast_fail).expect("br_on_cast_fail module parses");
        assert!(matches!(
            module.code[0].body[2],
            super::Instr::BrOnCastFail { depth: 1, .. }
        ));
    }

    #[test]
    fn validation_accepts_nullable_br_on_cast_complement_as_non_null() {
        let bytes = wasm_with_sections(&[
            &[
                0x01, 0x07,
                0x02,
                0x5f, 0x00,
                0x60, 0x01, 0x6e, 0x00,
            ],
            &[0x03, 0x03, 0x02, 0x01, 0x01],
            &[
                0x0a, 0x21,
                0x02,
                0x0f,
                0x00,
                0x02, 0x64, 0x6e,
                0x20, 0x00,
                0xfb, 0x18, 0x03, 0x00, 0x6e,
                0x6b,
                0x0b,
                0x1a,
                0x0b,
                0x0f,
                0x00,
                0x02, 0x64, 0x6e,
                0x20, 0x00,
                0xfb, 0x19, 0x03, 0x00, 0x6e,
                0x6b,
                0x0b,
                0x1a,
                0x0b,
            ],
        ]);
        parse_module(&bytes).expect("nullable cast complements are non-null");
    }

    #[test]
    fn tag_section_accepts_exception_tag_after_type_section() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[13, 3, 1, 0, 0],
        ]);
        let module = parse_module(&bytes).expect("exception tag section parses");
        assert_eq!(module.tags.len(), 1);
        assert_eq!(module.tags[0].attribute, 0);
        assert_eq!(module.tags[0].type_idx, 0);
    }

    #[test]
    fn tag_section_orders_after_memory_before_global() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[5, 3, 1, 0, 1],
            &[13, 3, 1, 0, 0],
            &[6, 6, 1, 0x7f, 0, 0x41, 0, 0x0b],
        ]);
        let module = parse_module(&bytes).expect("tag before global parses");
        assert_eq!(module.memories.len(), 1);
        assert_eq!(module.tags.len(), 1);
        assert_eq!(module.globals.len(), 1);
    }

    #[test]
    fn global_before_tag_section_is_out_of_order() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[6, 6, 1, 0x7f, 0, 0x41, 0, 0x0b],
            &[13, 3, 1, 0, 0],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("global before tag must reject"),
            Err(err) => err,
        };
        assert!(err.contains("section 13 out of order"), "{err}");
    }

    #[test]
    fn tag_section_rejects_type_index_out_of_bounds() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[13, 3, 1, 0, 1],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("missing tag type must reject"),
            Err(err) => err,
        };
        assert!(err.contains("tag 0 type index 1 out of bounds"), "{err}");
    }

    #[test]
    fn tag_section_rejects_unsupported_attribute() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[13, 3, 1, 1, 0],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("unsupported tag attribute must reject"),
            Err(err) => err,
        };
        assert!(err.contains("tag 0 has unsupported attribute 1"), "{err}");
    }

    #[test]
    fn tag_export_kind_accepts_exception_tag() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[13, 3, 1, 0, 0],
            &[7, 5, 1, 1, b'e', 0x04, 0],
        ]);
        let module = parse_module(&bytes).expect("tag export parses");
        assert_eq!(module.exports.len(), 1);
        assert_eq!(module.exports[0].kind, super::ExportKind::Tag);
    }

    #[test]
    fn tag_export_kind_rejects_out_of_bounds_tag() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[7, 5, 1, 1, b'e', 0x04, 0],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("missing tag export must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("tag export 'e' index 0 out of bounds"),
            "{err}"
        );
    }

    #[test]
    fn tag_import_kind_accepts_exception_tag() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[
                2, 8,
                1,
                1, b'm',
                1, b't',
                0x04, 0, 0,
            ],
        ]);
        let module = parse_module(&bytes).expect("tag import parses");
        assert_eq!(module.tags.len(), 1);
        assert!(matches!(module.imports[0].kind, super::ImportKind::Tag(0)));
    }

    #[test]
    fn tag_import_kind_rejects_type_index_out_of_bounds() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[2, 8, 1, 1, b'm', 1, b't', 0x04, 0, 1],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("missing tag import type must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("tag import type index 1 out of bounds"),
            "{err}"
        );
    }

    #[test]
    fn tag_import_kind_rejects_unsupported_attribute() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[2, 8, 1, 1, b'm', 1, b't', 0x04, 1, 0],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("unsupported tag import attribute must reject"),
            Err(err) => err,
        };
        assert!(err.contains("tag 0 has unsupported attribute 1"), "{err}");
    }

    #[test]
    fn tag_kind_rejects_non_empty_result_type() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[13, 3, 1, 0, 0],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("non-empty tag result must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("tag 0 type index 0 has non-empty result type"),
            "{err}"
        );
    }

    #[test]
    fn exception_throw_instruction_accepts_tag_operand() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[13, 3, 1, 0, 0],
            &[10, 6, 1, 4, 0, 0x08, 0, 0x0b],
        ]);
        let module = parse_module(&bytes).expect("throw instruction parses");
        assert!(matches!(module.code[0].body[0], super::Instr::Throw(0)));
    }

    #[test]
    fn exception_try_table_catch_accepts_handler_branch() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[13, 3, 1, 0, 0],
            &[
                10, 16,
                1, 14,
                0,
                0x02, 0x40,
                0x1f, 0x40, 1, 0, 0, 0,
                0x08, 0,
                0x0b, 0x0b, 0x0b,
            ],
        ]);
        let module = parse_module(&bytes).expect("try_table catch parses");
        assert!(matches!(
            module.code[0].body[1],
            super::Instr::TryTable(_, _)
        ));
    }

    #[test]
    fn legacy_exception_try_catch_all_accepts_structured_region() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[
                10, 10,
                1, 8,
                0,
                0x06, 0x40,
                0x01,
                0x19,
                0x01,
                0x0b,
                0x0b,
            ],
        ]);
        let module = parse_module(&bytes).expect("legacy try/catch_all module parses");
        assert!(matches!(module.code[0].body[0], super::Instr::LegacyTry(_)));
        assert!(module.code[0]
            .body
            .iter()
            .any(|ins| matches!(ins, super::Instr::LegacyCatchAll)));
    }

    #[test]
    fn exception_throw_ref_accepts_catch_ref_exnref_branch() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[13, 3, 1, 0, 0],
            &[
                10, 18,
                1, 16,
                0,
                0x02, 0x69,
                0x1f, 0x40, 1, 1, 0, 0,
                0x08, 0,
                0x0b,
                0x00,
                0x0b,
                0x0a,
                0x0b,
            ],
        ]);
        let module = parse_module(&bytes).expect("throw_ref module parses");
        assert!(module.code[0]
            .body
            .iter()
            .any(|ins| matches!(ins, super::Instr::ThrowRef)));
    }

    #[test]
    fn validation_rejects_function_type_index_out_of_bounds() {
        let bytes = wasm_with_sections(&[
            &[3, 2, 1, 0],
            &[10, 4, 1, 2, 0, 11],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("missing type must reject"),
            Err(err) => err,
        };
        assert!(err.contains("type index 0 out of bounds"), "{err}");
    }

    #[test]
    fn validation_rejects_local_index_out_of_bounds() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[10, 6, 1, 4, 0, 0x20, 0, 11],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("local oob must reject"),
            Err(err) => err,
        };
        assert!(err.contains("local index 0 out of bounds"), "{err}");
    }

    #[test]
    fn validation_rejects_call_index_out_of_bounds() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[10, 6, 1, 4, 0, 0x10, 1, 11],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("call oob must reject"),
            Err(err) => err,
        };
        assert!(err.contains("call function index 1 out of bounds"), "{err}");
    }

    #[test]
    fn validation_accepts_type_index_block_signature_and_execution_preserves_results() {
        let bytes = wasm_with_sections(&[
            &[
                1, 10, 2,
                0x60, 0, 2, 0x7f, 0x7f,
                0x60, 0, 1, 0x7f,
            ],
            &[3, 2, 1, 1],
            &[7, 7, 1, 3, b'r', b'u', b'n', 0, 0],
            &[
                10, 12, 1, 10, 0,
                0x02, 0x00,
                0x41, 0x07,
                0x41, 0x09,
                0x0b,
                0x1a,
                0x0b,
            ],
        ]);
        let module = parse_module(&bytes).expect("type-index block signature parses");
        let mut instance =
            instantiate(&module, Imports::default()).expect("type-index block instantiates");
        let results = instance
            .call("run", &[])
            .expect("type-index block executes");
        assert_eq!(results, vec![WasmValue::I32(7)]);
    }

    #[test]
    fn validation_rejects_type_index_block_signature_out_of_bounds() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[
                10, 7, 1, 5, 0,
                0x02, 0x01,
                0x0b,
                0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("out-of-bounds block type index must reject"),
            Err(err) => err,
        };
        assert!(err.contains("block type index 1 out of bounds"), "{err}");
    }

    #[test]
    fn validation_rejects_block_fallthrough_extra_values() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[
                10, 9, 1, 7, 0,
                0x02, 0x00,
                0x41, 0x00,
                0x0b,
                0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("extra block fallthrough values must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("control frame leaves extra stack values"),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_branch_operands_below_target_height() {
        let bytes = wasm_with_sections(&[
            &[
                1, 9, 2,
                0x60, 0, 2, 0x7f, 0x7f,
                0x60, 0, 0,
            ],
            &[3, 2, 1, 0],
            &[
                10, 13, 1, 11, 0,
                0x41, 0x01,
                0x02, 0x00,
                0x41, 0x00,
                0x0c, 0x00,
                0x0b,
                0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("branch operands below target height must reject"),
            Err(err) => err,
        };
        assert!(err.contains("branch operand stack underflow"), "{err}");
    }

    #[test]
    fn validation_rejects_br_if_using_value_below_current_frame() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[
                10, 16, 1, 14, 0,
                0x02, 0x7f,
                0x41, 0x00,
                0x02, 0x40,
                0x41, 0x01,
                0x0d, 0x01,
                0x0b,
                0x0b,
                0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("br_if must not use value below current frame"),
            Err(err) => err,
        };
        assert!(err.contains("branch operand stack underflow"), "{err}");
    }

    #[test]
    fn validation_accepts_unreachable_if_else_result_join() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[7, 7, 1, 3, b'r', b'u', b'n', 0, 0],
            &[
                10, 13, 1, 11, 0,
                0x41, 0x00,
                0x04, 0x7f,
                0x00,
                0x05,
                0x41, 0x07,
                0x0b,
                0x0b,
            ],
        ]);
        let module = parse_module(&bytes).expect("unreachable if/else result join validates");
        let mut instance =
            instantiate(&module, Imports::default()).expect("unreachable if/else instantiates");
        let results = instance
            .call("run", &[])
            .expect("unreachable if/else executes");
        assert_eq!(results, vec![WasmValue::I32(7)]);
    }

    #[test]
    fn validation_accepts_unreachable_result_if_without_else() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[
                10, 8, 1, 6, 0,
                0x00,
                0x04, 0x7f,
                0x0b,
                0x0b,
            ],
        ]);
        parse_module(&bytes).expect("unreachable result if without else validates");
    }

    #[test]
    fn validation_rejects_concrete_value_after_unreachable() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[
                10, 7, 1, 5, 0,
                0x00,
                0x41, 0x00,
                0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("concrete value after unreachable must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("control frame leaves extra stack values"),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_concrete_type_mismatch_after_return() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[
                10, 12, 1, 10, 0,
                0x0f,
                0x43, 0x00, 0x00, 0x00, 0x00,
                0x45,
                0x1a,
                0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("concrete type mismatch after return must reject"),
            Err(err) => err,
        };
        assert!(err.contains("operand type mismatch"), "{err}");
    }

    #[test]
    fn validation_accepts_extra_stack_values_before_void_return() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[
                10, 9, 1, 7, 0,
                0x41, 0x01,
                0x41, 0x02,
                0x0f,
                0x0b,
            ],
        ]);
        parse_module(&bytes).expect("extra values below void return are unreachable");
    }

    #[test]
    fn validation_rejects_bare_return_missing_function_result() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[
                10, 5, 1, 3, 0,
                0x0f,
                0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("bare return without function result must reject"),
            Err(err) => err,
        };
        assert!(err.contains("return operand stack underflow"), "{err}");
    }

    #[test]
    fn validation_rejects_return_consuming_value_below_block_height() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[
                10, 10, 1, 8, 0,
                0x41, 0x00,
                0x02, 0x40,
                0x0f,
                0x0b,
                0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("return must not consume operands below block height"),
            Err(err) => err,
        };
        assert!(err.contains("return operand stack underflow"), "{err}");
    }

    #[test]
    fn validation_rejects_return_without_function_result_as_br_operand() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[
                10, 12, 1, 10, 0,
                0x41, 0x00,
                0x02, 0x40,
                0x0f,
                0x0c, 0x00,
                0x0b,
                0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("return without result as br operand must reject"),
            Err(err) => err,
        };
        assert!(err.contains("operand stack underflow"), "{err}");
    }

    #[test]
    fn validation_rejects_return_without_function_result_as_br_if_operand() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[
                10, 14, 1, 12, 0,
                0x41, 0x00,
                0x02, 0x40,
                0x0f,
                0x41, 0x01,
                0x0d, 0x00,
                0x0b,
                0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("return without result as br_if operand must reject"),
            Err(err) => err,
        };
        assert!(err.contains("operand stack underflow"), "{err}");
    }

    #[test]
    fn validation_accepts_result_if_without_else_when_params_are_results() {
        let bytes = wasm_with_sections(&[
            &[
                1, 10, 2,
                0x60, 0, 1, 0x7f,
                0x60, 1, 0x7f, 1, 0x7f,
            ],
            &[3, 2, 1, 0],
            &[
                10, 11, 1, 9, 0,
                0x41, 0x01,
                0x41, 0x00,
                0x04, 0x01,
                0x0b,
                0x0b,
            ],
        ]);
        parse_module(&bytes).expect("if without else can pass params through as results");
    }

    #[test]
    fn validation_accepts_i64_extend_i32_inside_control_flow() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7e],
            &[3, 2, 1, 0],
            &[7, 7, 1, 3, b'r', b'u', b'n', 0, 0],
            &[
                10, 16, 1, 14, 0,
                0x41, 0x01,
                0x04, 0x7e,
                0x41, 0x07,
                0xad,
                0x05,
                0x41, 0x07,
                0xad,
                0x0b,
                0x0b,
            ],
        ]);
        let module = parse_module(&bytes).expect("control-flow i64.extend_i32_u validates");
        let mut instance =
            instantiate(&module, Imports::default()).expect("control-flow extend instantiates");
        let results = instance
            .call("run", &[])
            .expect("control-flow extend executes");
        assert_eq!(results, vec![WasmValue::I64(7)]);
    }

    #[test]
    fn validation_accepts_branch_to_outer_result_from_nested_block() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[7, 7, 1, 3, b'r', b'u', b'n', 0, 0],
            &[
                10, 15, 1, 13, 0,
                0x02, 0x7f,
                0x02, 0x40,
                0x41, 0x07,
                0x0c, 0x01,
                0x0b,
                0x00,
                0x0b,
                0x0b,
            ],
        ]);
        let module = parse_module(&bytes).expect("outer result branch validates");
        let mut instance =
            instantiate(&module, Imports::default()).expect("outer result branch instantiates");
        let results = instance
            .call("run", &[])
            .expect("outer result branch executes");
        assert_eq!(results, vec![WasmValue::I32(7)]);
    }

    #[test]
    fn validation_tracks_nested_if_else_inside_unreachable_branch_tail() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[7, 7, 1, 3, b'r', b'u', b'n', 0, 0],
            &[
                10, 17, 1, 15, 0,
                0x02, 0x7f,
                0x41, 0x07,
                0x0c, 0x00,
                0x41, 0x00,
                0x04, 0x40,
                0x05,
                0x0b,
                0x0b,
                0x0b,
            ],
        ]);
        let module = parse_module(&bytes).expect("unreachable nested if/else validates");
        let mut instance =
            instantiate(&module, Imports::default()).expect("unreachable nested if instantiates");
        let results = instance
            .call("run", &[])
            .expect("unreachable nested if executes");
        assert_eq!(results, vec![WasmValue::I32(7)]);
    }

    #[test]
    fn validation_accepts_multivalue_branch_target_arity() {
        let bytes = wasm_with_sections(&[
            &[
                1, 10, 2,
                0x60, 0, 2, 0x7f, 0x7f,
                0x60, 0, 1, 0x7f,
            ],
            &[3, 2, 1, 1],
            &[7, 7, 1, 3, b'r', b'u', b'n', 0, 0],
            &[
                10, 14, 1, 12, 0,
                0x02, 0x00,
                0x41, 0x07,
                0x41, 0x09,
                0x0c, 0x00,
                0x0b,
                0x1a,
                0x0b,
            ],
        ]);
        let module = parse_module(&bytes).expect("multivalue br target arity validates");
        let mut instance =
            instantiate(&module, Imports::default()).expect("multivalue br instantiates");
        let results = instance.call("run", &[]).expect("multivalue br executes");
        assert_eq!(results, vec![WasmValue::I32(7)]);
    }

    #[test]
    fn validation_rejects_multivalue_branch_missing_target_result() {
        let bytes = wasm_with_sections(&[
            &[
                1, 10, 2,
                0x60, 0, 2, 0x7f, 0x7f,
                0x60, 0, 1, 0x7f,
            ],
            &[3, 2, 1, 1],
            &[7, 7, 1, 3, b'r', b'u', b'n', 0, 0],
            &[
                10, 12, 1, 10, 0,
                0x02, 0x00,
                0x41, 0x07,
                0x0c, 0x00,
                0x0b,
                0x1a,
                0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("missing branch target result must reject"),
            Err(err) => err,
        };
        assert!(err.contains("branch operand stack underflow"), "{err}");
    }

    #[test]
    fn validation_accepts_multivalue_if_else_join_arity() {
        let bytes = wasm_with_sections(&[
            &[
                1, 10, 2,
                0x60, 0, 2, 0x7f, 0x7f,
                0x60, 0, 1, 0x7f,
            ],
            &[3, 2, 1, 1],
            &[7, 7, 1, 3, b'r', b'u', b'n', 0, 0],
            &[
                10, 19, 1, 17, 0,
                0x41, 0x01,
                0x04, 0x00,
                0x41, 0x07,
                0x41, 0x09,
                0x05,
                0x41, 0x0b,
                0x41, 0x0d,
                0x0b,
                0x1a,
                0x0b,
            ],
        ]);
        let module = parse_module(&bytes).expect("multivalue if/else validates");
        let mut instance =
            instantiate(&module, Imports::default()).expect("multivalue if/else instantiates");
        let results = instance
            .call("run", &[])
            .expect("multivalue if/else executes");
        assert_eq!(results, vec![WasmValue::I32(7)]);
    }

    #[test]
    fn validation_rejects_multivalue_if_else_missing_result() {
        let bytes = wasm_with_sections(&[
            &[
                1, 10, 2,
                0x60, 0, 2, 0x7f, 0x7f,
                0x60, 0, 1, 0x7f,
            ],
            &[3, 2, 1, 1],
            &[7, 7, 1, 3, b'r', b'u', b'n', 0, 0],
            &[
                10, 17, 1, 15, 0,
                0x41, 0x01,
                0x04, 0x00,
                0x41, 0x07,
                0x41, 0x09,
                0x05,
                0x41, 0x0b,
                0x0b,
                0x1a,
                0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("missing if/else result must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("operand stack underflow while expecting I32"),
            "{err}"
        );
    }

    #[test]
    fn validation_accepts_multivalue_br_table_target_arity() {
        let bytes = wasm_with_sections(&[
            &[
                1, 10, 2,
                0x60, 0, 2, 0x7f, 0x7f,
                0x60, 0, 1, 0x7f,
            ],
            &[3, 2, 1, 1],
            &[7, 7, 1, 3, b'r', b'u', b'n', 0, 0],
            &[
                10, 18, 1, 16, 0,
                0x02, 0x00,
                0x41, 0x07,
                0x41, 0x09,
                0x41, 0x00,
                0x0e, 0x01, 0x00, 0x00,
                0x0b,
                0x1a,
                0x0b,
            ],
        ]);
        let module = parse_module(&bytes).expect("multivalue br_table validates");
        let mut instance =
            instantiate(&module, Imports::default()).expect("multivalue br_table instantiates");
        let results = instance
            .call("run", &[])
            .expect("multivalue br_table executes");
        assert_eq!(results, vec![WasmValue::I32(7)]);
    }

    #[test]
    fn validation_accepts_unreachable_br_table_target_type_mismatch() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[7, 7, 1, 3, b'r', b'u', b'n', 0, 0],
            &[
                10, 29, 1, 27, 0,
                0x02, 0x7c,
                0x02, 0x7d,
                0x00,
                0x41, 0x01,
                0x0e, 0x02, 0x00, 0x01, 0x01,
                0x0b,
                0x1a,
                0x44, 0, 0, 0, 0, 0, 0, 0, 0,
                0x0b,
                0x1a,
                0x0b,
            ],
        ]);
        let module = parse_module(&bytes).expect("unreachable br_table validates");
        let mut instance =
            instantiate(&module, Imports::default()).expect("unreachable br_table instantiates");
        let err = match instance.call("run", &[]) {
            Ok(_) => panic!("unreachable br_table body must trap"),
            Err(err) => err,
        };
        assert!(err.contains("unreachable"), "{err}");
    }

    #[test]
    fn validation_rejects_nested_void_operand_after_unreachable_branch() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[
                10, 15, 1, 13, 0,
                0x02, 0x40,
                0x0c, 0x00,
                0x02, 0x40,
                0x01,
                0x67,
                0x1a,
                0x0b,
                0x0b,
                0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("nested void operand after branch must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("operand stack underflow while expecting I32"),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_unreachable_br_table_target_arity_mismatch() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[
                10, 19, 1, 17, 0,
                0x02, 0x40,
                0x02, 0x7d,
                0x00,
                0x41, 0x01,
                0x0e, 0x02, 0x00, 0x01, 0x00,
                0x0b,
                0x1a,
                0x0b,
                0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("br_table target arity mismatch must reject"),
            Err(err) => err,
        };
        assert!(err.contains("br_table target type mismatch"), "{err}");
    }

    #[test]
    fn validation_rejects_nested_void_fallthrough_despite_outer_branch() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[
                10, 14, 1, 12, 0,
                0x02, 0x7f,
                0x02, 0x40,
                0x41, 0x00,
                0x0c, 0x01,
                0x0b,
                0x0b,
                0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("reachable outer fallthrough without result must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("operand stack underflow while expecting I32"),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_multivalue_br_table_missing_target_result() {
        let bytes = wasm_with_sections(&[
            &[
                1, 10, 2,
                0x60, 0, 2, 0x7f, 0x7f,
                0x60, 0, 1, 0x7f,
            ],
            &[3, 2, 1, 1],
            &[7, 7, 1, 3, b'r', b'u', b'n', 0, 0],
            &[
                10, 16, 1, 14, 0,
                0x02, 0x00,
                0x41, 0x07,
                0x41, 0x00,
                0x0e, 0x01, 0x00, 0x00,
                0x0b,
                0x1a,
                0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("missing br_table target result must reject"),
            Err(err) => err,
        };
        assert!(err.contains("branch operand stack underflow"), "{err}");
    }

    #[test]
    fn validation_rejects_br_table_operand_below_current_frame() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[
                10, 17, 1, 15, 0,
                0x02, 0x7f,
                0x41, 0x00,
                0x02, 0x40,
                0x41, 0x00,
                0x0e, 0x00, 0x01,
                0x0b,
                0x0b,
                0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("br_table must not consume operands below the current frame"),
            Err(err) => err,
        };
        assert!(err.contains("branch operand stack underflow"), "{err}");
    }

    #[test]
    fn validation_rejects_global_index_out_of_bounds() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[10, 6, 1, 4, 0, 0x23, 0, 11],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("global oob must reject"),
            Err(err) => err,
        };
        assert!(err.contains("global index 0 out of bounds"), "{err}");
    }

    #[test]
    fn validation_rejects_duplicate_export_name() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[7, 9, 2, 1, b'x', 0, 0, 1, b'x', 0, 0],
            &[10, 4, 1, 2, 0, 11],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("duplicate export must reject"),
            Err(err) => err,
        };
        assert!(err.contains("duplicate export name"), "{err}");
    }

    #[test]
    fn validation_rejects_export_index_out_of_bounds() {
        let bytes = wasm_with_sections(&[
            &[7, 5, 1, 1, b'f', 0, 0],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("export oob must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("function export 'f' index 0 out of bounds"),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_start_signature_with_params() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 1, 0x7f, 0],
            &[3, 2, 1, 0],
            &[8, 1, 0],
            &[10, 4, 1, 2, 0, 11],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("start with params must reject"),
            Err(err) => err,
        };
        assert!(err.contains("start function 0 must have empty"), "{err}");
    }

    #[test]
    fn validation_rejects_import_global_bad_mutability() {
        let bytes = wasm_with_sections(&[&[2, 10, 1, 3, b'e', b'n', b'v', 1, b'g', 3, 0x7f, 2]]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("bad mutability must reject"),
            Err(err) => err,
        };
        assert!(err.contains("bad global mutability"), "{err}");
    }

    #[test]
    fn validation_rejects_global_initializer_type_mismatch() {
        let bytes = wasm_with_sections(&[
            &[6, 6, 1, 0x7e, 0, 0x41, 0, 11],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("initializer type mismatch must reject"),
            Err(err) => err,
        };
        assert!(err.contains("initializer type I32 does not match"), "{err}");
    }

    #[test]
    fn validation_rejects_non_constant_global_initializer() {
        let bytes = wasm_with_sections(&[
            &[6, 6, 1, 0x7f, 0, 0x20, 0, 11],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("non-constant initializer must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("non-constant initializer instruction"),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_mutable_import_global_initializer_reference() {
        let bytes = wasm_with_sections(&[
            &[2, 10, 1, 3, b'e', b'n', b'v', 1, b'g', 3, 0x7f, 1],
            &[6, 6, 1, 0x7f, 0, 0x23, 0, 11],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("mutable global.get initializer must reject"),
            Err(err) => err,
        };
        assert!(err.contains("references mutable global"), "{err}");
    }

    #[test]
    fn validation_accepts_prior_defined_global_initializer_reference() {
        let bytes = wasm_with_sections(&[&[
            6, 11, 2,
            0x7f, 0, 0x41, 0, 11,
            0x7f, 0, 0x23, 0, 11,
        ]]);
        parse_module(&bytes).expect("prior immutable defined global is accepted");
    }

    #[test]
    fn validation_rejects_undeclared_ref_func_in_code() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[10, 7, 1, 5, 0, 0xd2, 0, 0x1a, 0x0b],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("undeclared ref.func must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("undeclared ref.func function index 0"),
            "{err}"
        );
    }

    #[test]
    fn validation_accepts_ref_func_declared_by_global_initializer() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[6, 6, 1, 0x70, 0, 0xd2, 0, 0x0b],
            &[10, 7, 1, 5, 0, 0xd2, 0, 0x1a, 0x0b],
        ]);
        parse_module(&bytes).expect("global initializer declares ref.func target");
    }

    #[test]
    fn validation_accepts_extended_const_i32_add_global_initializer() {
        let bytes = wasm_with_sections(&[&[
            6, 9, 1,
            0x7f, 0,
            0x41, 1,
            0x41, 2,
            0x6a,
            11,
        ]]);
        parse_module(&bytes).expect("extended const i32.add initializer is accepted");
    }

    #[test]
    fn validation_rejects_global_set_to_immutable_global() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[6, 6, 1, 0x7f, 0, 0x41, 0, 11],
            &[10, 8, 1, 6, 0, 0x41, 1, 0x24, 0, 11],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("global.set to immutable global must reject"),
            Err(err) => err,
        };
        assert!(err.contains("targets immutable global"), "{err}");
    }

    #[test]
    fn validation_rejects_overaligned_ordinary_memory_load() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[5, 3, 1, 0, 1],
            &[10, 9, 1, 7, 0, 0x41, 0, 0x2d, 1, 0, 0x0b],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("overaligned i32.load8_u must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains(
                "invalid alignment; expected maximum alignment is 0, actual alignment is 1"
            ),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_overaligned_ordinary_memory_store() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[5, 3, 1, 0, 1],
            &[10, 11, 1, 9, 0, 0x41, 0, 0x41, 42, 0x3b, 2, 0, 0x0b],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("overaligned i32.store16 must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains(
                "invalid alignment; expected maximum alignment is 1, actual alignment is 2"
            ),
            "{err}"
        );
    }

    #[test]
    fn validation_accepts_underaligned_ordinary_memory_load() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[5, 3, 1, 0, 1],
            &[10, 9, 1, 7, 0, 0x41, 0, 0x28, 0, 0, 0x0b],
        ]);
        parse_module(&bytes).expect("underaligned ordinary i32.load validates");
    }

    #[test]
    fn validation_accepts_i64_load32_u_natural_alignment() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7e],
            &[3, 2, 1, 0],
            &[5, 3, 1, 0, 1],
            &[10, 9, 1, 7, 0, 0x41, 0, 0x35, 2, 0, 0x0b],
        ]);
        parse_module(&bytes).expect("i64.load32_u validates at natural alignment");
    }

    #[test]
    fn validation_rejects_overaligned_simd_memory_loads() {
        let v128_load = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7b],
            &[3, 2, 1, 0],
            &[5, 3, 1, 0, 1],
            &[10, 10, 1, 8, 0, 0x41, 0, 0xfd, 0, 5, 0, 0x0b],
        ]);
        let err = match parse_module(&v128_load) {
            Ok(_) => panic!("overaligned v128.load must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains(
                "invalid alignment; expected maximum alignment is 4, actual alignment is 5"
            ),
            "{err}"
        );

        let v128_load8_splat = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7b],
            &[3, 2, 1, 0],
            &[5, 3, 1, 0, 1],
            &[10, 10, 1, 8, 0, 0x41, 0, 0xfd, 7, 1, 0, 0x0b],
        ]);
        let err = match parse_module(&v128_load8_splat) {
            Ok(_) => panic!("overaligned v128.load8_splat must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains(
                "invalid alignment; expected maximum alignment is 0, actual alignment is 1"
            ),
            "{err}"
        );
    }

    #[test]
    fn validation_accepts_underaligned_simd_memory_load() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7b],
            &[3, 2, 1, 0],
            &[5, 3, 1, 0, 1],
            &[10, 10, 1, 8, 0, 0x41, 0, 0xfd, 0, 0, 0, 0x0b],
        ]);
        parse_module(&bytes).expect("underaligned v128.load validates");
    }

    #[test]
    fn validation_rejects_32bit_simd_memory_offset_above_u32() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[5, 3, 1, 0, 1],
            &[
                10, 15, 1,
                13, 0,
                0x41, 0,
                0xfd, 0,
                4,
                0x80, 0x80, 0x80, 0x80, 0x10,
                0x1a,
                0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("32-bit SIMD memarg offset above u32 must reject"),
            Err(err) => err,
        };
        assert!(err.contains("memory offset outside 32-bit range"), "{err}");
    }

    #[test]
    fn validation_accepts_memory64_simd_memory_offset_above_u32() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[5, 4, 1, 4, 1, 0],
            &[
                10, 15, 1,
                13, 0,
                0x42, 0,
                0xfd, 0,
                4,
                0x80, 0x80, 0x80, 0x80, 0x10,
                0x1a,
                0x0b,
            ],
        ]);
        parse_module(&bytes).expect("memory64 SIMD memarg offset above u32 is accepted");
    }

    #[test]
    fn validation_rejects_f32_neg_global_initializer() {
        let bytes = wasm_with_sections(&[&[
            6, 10, 1,
            0x7d, 0,
            0x43, 0, 0, 0, 0,
            0x8c,
            11,
        ]]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("f32.neg initializer must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("non-constant initializer numeric opcode 0x8c"),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_i32_ctz_global_initializer() {
        let bytes = wasm_with_sections(&[&[
            6, 7, 1,
            0x7f, 0,
            0x41, 0,
            0x68,
            11,
        ]]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("i32.ctz initializer must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("non-constant initializer numeric opcode 0x68"),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_untyped_select_with_funcref_operands() {
        let bytes = wasm_with_sections(&[
            &[1, 8, 1, 0x60, 3, 0x70, 0x70, 0x7f, 1, 0x70],
            &[3, 2, 1, 0],
            &[
                10, 11, 1,
                9, 0,
                0x20, 0,
                0x20, 1,
                0x20, 2,
                0x1b,
                11,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("untyped select over funcref operands must reject"),
            Err(err) => err,
        };
        assert!(err.contains("select without result type"), "{err}");
    }

    #[test]
    fn validation_accepts_typed_select_with_funcref_operands() {
        let bytes = wasm_with_sections(&[
            &[1, 8, 1, 0x60, 3, 0x70, 0x70, 0x7f, 1, 0x70],
            &[3, 2, 1, 0],
            &[
                10, 13, 1,
                11, 0,
                0x20, 0,
                0x20, 1,
                0x20, 2,
                0x1c, 1, 0x70,
                11,
            ],
        ]);
        parse_module(&bytes).expect("typed select over funcref operands is accepted");
    }

    #[test]
    fn validation_accepts_typed_select_with_typed_funcref_operands() {
        let bytes = wasm_with_sections(&[
            &[
                1, 13, 2,
                0x60, 0, 0,
                0x60, 3, 0x64, 0x00, 0x64, 0x00, 0x7f, 1,
                0x70,
            ],
            &[3, 2, 1, 1],
            &[
                10, 13, 1,
                11, 0,
                0x20, 0,
                0x20, 1,
                0x20, 2,
                0x1c, 1, 0x70,
                11,
            ],
        ]);
        parse_module(&bytes).expect("typed select accepts typed function references as funcref");
    }

    #[test]
    fn validation_accepts_typed_select_joining_ref_func_and_null_func() {
        let bytes = wasm_with_sections(&[
            &[
                1, 9, 2,
                0x60, 0, 0,
                0x60, 1, 0x7f, 1, 0x70,
            ],
            &[3, 3, 2, 0, 1],
            &[6, 6, 1, 0x70, 0, 0xd2, 0, 0x0b],
            &[
                10, 16, 2,
                2, 0, 0x0b,
                11, 0,
                0xd2, 0x00,
                0xd0, 0x70,
                0x20, 0x00,
                0x1c, 1, 0x70,
                0x0b,
            ],
        ]);
        parse_module(&bytes).expect("typed select joins ref.func and ref.null func as funcref");
    }

    #[test]
    fn validation_accepts_unreachable_untyped_select_as_reference_operand() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[
                10, 10, 1, 8, 0,
                0x00,
                0x41, 0x01,
                0x1b,
                0xd1,
                0x1a,
                0x0b,
            ],
        ]);
        parse_module(&bytes).expect("unreachable select result can satisfy ref.is_null");
    }

    #[test]
    fn validation_accepts_unreachable_select_as_i32_result() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[
                10, 7, 1, 5, 0,
                0x00,
                0x1b,
                0x1b,
                0x0b,
            ],
        ]);
        parse_module(&bytes).expect("unreachable select can satisfy i32 result");
    }

    #[test]
    fn validation_accepts_branch_polymorphic_numeric_operand() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[
                10, 11, 1, 9, 0,
                0x02, 0x40,
                0x0c, 0x00,
                0x67,
                0x1a,
                0x0b,
                0x0b,
            ],
        ]);
        parse_module(&bytes).expect("branch-polymorphic numeric operand validates");
    }

    #[test]
    fn validation_accepts_branch_supplied_enclosing_block_result() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[
                10, 14, 1, 12, 0,
                0x02, 0x7f,
                0x03, 0x7f,
                0x41, 0x03,
                0x0c, 0x01,
                0x0b,
                0x0b,
                0x0b,
            ],
        ]);
        parse_module(&bytes).expect("branch-supplied enclosing block result validates");
    }

    #[test]
    fn validation_accepts_ref_is_null_on_externref() {
        let bytes = wasm_with_sections(&[
            &[1, 6, 1, 0x60, 1, 0x6f, 1, 0x7f],
            &[3, 2, 1, 0],
            &[
                10, 7, 1,
                5, 0,
                0x20, 0,
                0xd1,
                11,
            ],
        ]);
        parse_module(&bytes).expect("ref.is_null over externref is accepted");
    }

    #[test]
    fn validation_rejects_extended_const_type_mismatch() {
        let bytes = wasm_with_sections(&[&[
            6, 9, 1,
            0x7f, 0,
            0x41, 1,
            0x42, 2,
            0x6a,
            11,
        ]]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("typed extended const mismatch must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("expected I32") || err.contains("operand stack"),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_data_offset_type_mismatch() {
        let bytes = wasm_with_sections(&[
            &[5, 3, 1, 0, 1],
            &[11, 6, 1, 0, 0x42, 0, 11, 0],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("data offset type mismatch must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("data segment 0 offset initializer type I64 does not match I32"),
            "{err}"
        );
    }

    #[test]
    fn validation_accepts_memory64_data_offset_i64() {
        let bytes = wasm_with_sections(&[
            &[5, 3, 1, 0x04, 1],
            &[11, 6, 1, 0, 0x42, 0, 11, 0],
        ]);
        parse_module(&bytes).expect("memory64 active data offset may be i64");
    }

    #[test]
    fn validation_accepts_memory64_load_address_i64() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[5, 3, 1, 0x04, 1],
            &[
                10, 9, 1, 7, 0,
                0x42, 0,
                0x28, 2, 0,
                0x0b,
            ],
        ]);
        parse_module(&bytes).expect("memory64 load address may be i64");
    }

    #[test]
    fn validation_rejects_memory_limit_max_less_than_min() {
        let bytes = wasm_with_sections(&[
            &[5, 4, 1, 1, 2, 1],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("max less than min must reject"),
            Err(err) => err,
        };
        assert!(err.contains("limits max 1 is less than min 2"), "{err}");
    }

    #[test]
    fn validation_rejects_memory_instruction_without_memory() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[
                10, 10, 1, 8, 0,
                0x41, 0x00,
                0x2a, 0x02, 0x00,
                0x1a,
                0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("memory instruction without memory must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("memory instruction index 0 out of bounds"),
            "{err}"
        );
    }

    #[test]
    fn validation_accepts_explicit_memory_index_memarg() {
        let bytes = wasm_with_sections(&[
            &[1, 6, 1, 0x60, 1, 0x7f, 1, 0x7f],
            &[3, 2, 1, 0],
            &[5, 5, 2, 0, 0, 0, 1],
            &[
                10, 10, 1, 8, 0,
                0x20, 0,
                0x2d, 0x40, 1, 0,
                0x0b,
            ],
        ]);
        parse_module(&bytes).expect("explicit memory-index memarg validates");
    }

    #[test]
    fn validation_accepts_explicit_memory_indices_for_bulk_memory_ops() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[5, 5, 2, 0, 0, 0, 1],
            &[12, 1, 1],
            &[
                10, 33, 1, 31, 0,
                0x41, 0, 0x41, 0, 0x41, 0, 0xfc, 11, 1,
                0x41, 0, 0x41, 0, 0x41, 0, 0xfc, 10, 1, 1,
                0x41, 0, 0x41, 0, 0x41, 0, 0xfc, 8, 0, 1,
                0x0b,
            ],
            &[11, 3, 1, 1, 0],
        ]);
        parse_module(&bytes).expect("explicit bulk-memory indices validate");
    }

    #[test]
    fn validation_rejects_explicit_memory_index_out_of_bounds() {
        let bytes = wasm_with_sections(&[
            &[1, 6, 1, 0x60, 1, 0x7f, 1, 0x7f],
            &[3, 2, 1, 0],
            &[5, 3, 1, 0, 1],
            &[
                10, 10, 1, 8, 0,
                0x20, 0,
                0x2d, 0x40, 1, 0,
                0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("out-of-bounds explicit memory index must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("memory instruction index 1 out of bounds (1 memories)"),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_mvp_memory_limit_above_implementation_max() {
        let min_too_large = wasm_with_sections(&[
            &[5, 5, 1, 0, 0x81, 0x80, 0x04],
        ]);
        let err = match parse_module(&min_too_large) {
            Ok(_) => panic!("memory min above limit must reject"),
            Err(err) => err,
        };
        assert!(err.contains("memory min 65537 exceeds"), "{err}");

        let max_too_large = wasm_with_sections(&[
            &[5, 6, 1, 1, 0, 0x81, 0x80, 0x04],
        ]);
        let err = match parse_module(&max_too_large) {
            Ok(_) => panic!("memory max above limit must reject"),
            Err(err) => err,
        };
        assert!(err.contains("memory max 65537 exceeds"), "{err}");
    }

    #[test]
    fn validation_accepts_table_limit_above_memory_page_cap() {
        let bytes = wasm_with_sections(&[&[
            4, 9, 1,
            0x70,
            0x01,
            0x00,
            0xff, 0xff, 0xff, 0xff, 0x0f,
        ]]);
        let module = parse_module(&bytes).expect("table max u32::MAX is accepted");
        assert_eq!(module.tables.len(), 1);
        assert_eq!(module.tables[0].limits.max, Some(u32::MAX as u64));
    }

    #[test]
    fn validation_rejects_defined_table_initial_size_above_implementation_limit() {
        let bytes = wasm_with_sections(&[&[
            4, 8, 1,
            0x70,
            0x00,
            0xff, 0xff, 0xff, 0xff, 0x0f,
        ]]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("oversized defined table must reject before allocation"),
            Err(err) => err,
        };
        assert!(
            err.contains("table min 4294967295 exceeds implementation limit 10000000"),
            "{err}"
        );
    }

    #[test]
    fn validation_accepts_table_init_expr_encoding() {
        let bytes = wasm_with_sections(&[&[
            4, 9, 1,
            0x40,
            0x00,
            0x70,
            0x00,
            0x01,
            0xd0, 0x70, 0x0b,
        ]]);
        let module = parse_module(&bytes).expect("table init expression is accepted");
        assert_eq!(module.tables.len(), 1);
        assert_eq!(module.tables[0].elem, super::ValType::FuncRef);
    }

    #[test]
    fn validation_rejects_table_init_expr_type_mismatch() {
        let bytes = wasm_with_sections(&[&[
            4, 9, 1,
            0x40,
            0x00,
            0x70,
            0x00,
            0x01,
            0x41, 0x00, 0x0b,
        ]]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("table initializer type mismatch must reject"),
            Err(err) => err,
        };
        assert!(err.contains("table 0 initializer type I32"), "{err}");
    }

    #[test]
    fn validation_rejects_nondefaultable_table_without_initializer() {
        let bytes = wasm_with_sections(&[&[
            4, 5, 1,
            0x64, 0x70,
            0x00,
            0x00,
        ]]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("non-defaultable table without initializer must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("not defaultable and has no initializer"),
            "{err}"
        );
    }

    #[test]
    fn validation_accepts_memory64_min_only() {
        let bytes = wasm_with_sections(&[
            &[5, 3, 1, 0x04, 1],
        ]);
        parse_module(&bytes).expect("memory64 min-only limits are accepted");
    }

    #[test]
    fn validation_rejects_memory64_limits_above_implementation_cap() {
        let max_too_large = wasm_with_sections(&[

            &[5, 6, 1, 0x05, 0, 0x81, 0x80, 0x10],
        ]);
        let err = match parse_module(&max_too_large) {
            Ok(_) => panic!("memory64 max above implementation cap must reject"),
            Err(err) => err,
        };
        assert!(err.contains("memory max 262145 exceeds"), "{err}");

        let min_too_large = wasm_with_sections(&[

            &[5, 5, 1, 0x04, 0x81, 0x80, 0x10],
        ]);
        let err = match parse_module(&min_too_large) {
            Ok(_) => panic!("memory64 min above implementation cap must reject"),
            Err(err) => err,
        };
        assert!(err.contains("memory min 262145 exceeds"), "{err}");
    }

    #[test]
    fn validation_accepts_shared_memory_with_max() {
        let bytes = wasm_with_sections(&[
            &[5, 4, 1, 0x03, 1, 1],
        ]);
        let module = parse_module(&bytes).expect("shared memory with max is accepted");
        let lim = module.memories.first().expect("memory");
        assert!(lim.shared);
        assert_eq!(lim.min, 1);
        assert_eq!(lim.max, Some(1));
    }

    #[test]
    fn validation_rejects_shared_memory_without_max() {
        let bytes = wasm_with_sections(&[
            &[5, 3, 1, 0x02, 1],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("shared memory without max must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("shared memory must have a maximum defined"),
            "{err}"
        );
    }

    #[test]
    fn validation_records_imported_shared_memory_limits() {
        let bytes = wasm_with_sections(&[&[
            2, 13, 1,
            3, b'e', b'n', b'v',
            3, b'm', b'e', b'm',
            0x02,
            0x03, 1, 1,
        ]]);
        let module = parse_module(&bytes).expect("shared memory import is accepted");
        let imports = memory_imports(&module);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].module, "env");
        assert_eq!(imports[0].name, "mem");
        assert!(imports[0].shared);
        assert_eq!(imports[0].min, 1);
        assert_eq!(imports[0].max, Some(1));
    }

    #[test]
    fn validation_accepts_i32_atomic_load_store_on_shared_memory() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[5, 4, 1, 0x03, 1, 1],
            &[10, 10, 1, 8, 0, 0x41, 0, 0xfe, 0x10, 2, 0, 0x0b],
        ]);
        parse_module(&bytes).expect("i32.atomic.load on shared memory validates");

        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[5, 4, 1, 0x03, 1, 1],
            &[10, 12, 1, 10, 0, 0x41, 0, 0x41, 42, 0xfe, 0x17, 2, 0, 0x0b],
        ]);
        parse_module(&bytes).expect("i32.atomic.store on shared memory validates");

        for (subopcode, align) in [(0x12, 0), (0x13, 1)] {
            let bytes = wasm_with_sections(&[
                &[1, 5, 1, 0x60, 0, 1, 0x7f],
                &[3, 2, 1, 0],
                &[5, 4, 1, 0x03, 1, 1],
                &[10, 10, 1, 8, 0, 0x41, 0, 0xfe, subopcode, align, 0, 0x0b],
            ]);
            parse_module(&bytes).expect("i32 narrow atomic load on shared memory validates");
        }

        for (subopcode, align) in [(0x19, 0), (0x1a, 1)] {
            let bytes = wasm_with_sections(&[
                &[1, 4, 1, 0x60, 0, 0],
                &[3, 2, 1, 0],
                &[5, 4, 1, 0x03, 1, 1],
                &[
                    10, 12, 1, 10, 0, 0x41, 0, 0x41, 42, 0xfe, subopcode, align, 0, 0x0b,
                ],
            ]);
            parse_module(&bytes).expect("i32 narrow atomic store on shared memory validates");
        }

        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[5, 4, 1, 0x03, 1, 1],
            &[
                10, 20, 1, 18, 0, 0x41, 0, 0x41, 40, 0xfe, 0x17, 2, 0, 0x41, 0, 0x41, 2, 0xfe,
                0x1e, 2, 0, 0x0b,
            ],
        ]);
        parse_module(&bytes).expect("i32.atomic.rmw.add on shared memory validates");

        for (subopcode, align) in [(0x20, 0), (0x21, 1)] {
            let bytes = wasm_with_sections(&[
                &[1, 5, 1, 0x60, 0, 1, 0x7f],
                &[3, 2, 1, 0],
                &[5, 4, 1, 0x03, 1, 1],
                &[
                    10, 12, 1, 10, 0, 0x41, 0, 0x41, 5, 0xfe, subopcode, align, 0, 0x0b,
                ],
            ]);
            parse_module(&bytes).expect("i32 narrow atomic rmw.add on shared memory validates");
        }

        for subopcode in [0x25, 0x2c, 0x33, 0x3a, 0x41] {
            let bytes = wasm_with_sections(&[
                &[1, 5, 1, 0x60, 0, 1, 0x7f],
                &[3, 2, 1, 0],
                &[5, 4, 1, 0x03, 1, 1],
                &[
                    10, 20, 1, 18, 0, 0x41, 0, 0x41, 40, 0xfe, 0x17, 2, 0, 0x41, 0, 0x41, 2, 0xfe,
                    subopcode, 2, 0, 0x0b,
                ],
            ]);
            parse_module(&bytes).expect("i32.atomic.rmw variant on shared memory validates");
        }

        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[5, 4, 1, 0x03, 1, 1],
            &[
                10, 22, 1, 20, 0, 0x41, 0, 0x41, 40, 0xfe, 0x17, 2, 0, 0x41, 0, 0x41, 40, 0x41, 7,
                0xfe, 0x48, 2, 0, 0x0b,
            ],
        ]);
        parse_module(&bytes).expect("i32.atomic.rmw.cmpxchg on shared memory validates");

        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[5, 4, 1, 0x03, 1, 1],
            &[10, 12, 1, 10, 0, 0x41, 0, 0x41, 1, 0xfe, 0x00, 2, 0, 0x0b],
        ]);
        parse_module(&bytes).expect("memory.atomic.notify on shared memory validates");

        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[5, 4, 1, 0x03, 1, 1],
            &[
                10, 14, 1, 12, 0, 0x41, 0, 0x41, 42, 0x42, 0, 0xfe, 0x01, 2, 0, 0x0b,
            ],
        ]);
        parse_module(&bytes).expect("memory.atomic.wait32 on shared memory validates");

        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[5, 4, 1, 0x03, 1, 1],
            &[
                10, 14, 1, 12, 0, 0x41, 0, 0x42, 42, 0x42, 0, 0xfe, 0x02, 3, 0, 0x0b,
            ],
        ]);
        parse_module(&bytes).expect("memory.atomic.wait64 on shared memory validates");
    }

    #[test]
    fn validation_rejects_i32_atomic_load_underaligned_memarg() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[5, 4, 1, 0x03, 1, 1],
            &[10, 10, 1, 8, 0, 0x41, 0, 0xfe, 0x10, 1, 0, 0x0b],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("underaligned atomic load must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains(
                "invalid alignment for atomic operation; expected alignment is 2, actual alignment is 1"
            ),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_i32_narrow_atomic_bad_alignments() {
        let load8_overaligned = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[5, 4, 1, 0x03, 1, 1],
            &[10, 10, 1, 8, 0, 0x41, 0, 0xfe, 0x12, 1, 0, 0x0b],
        ]);
        let err = match parse_module(&load8_overaligned) {
            Ok(_) => panic!("load8 overalign rejects"),
            Err(err) => err,
        };
        assert!(
            err.contains(
                "invalid alignment; expected maximum alignment is 0, actual alignment is 1"
            ),
            "{err}"
        );

        let load16_underaligned = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[5, 4, 1, 0x03, 1, 1],
            &[10, 10, 1, 8, 0, 0x41, 0, 0xfe, 0x13, 0, 0, 0x0b],
        ]);
        let err = match parse_module(&load16_underaligned) {
            Ok(_) => panic!("load16 underalign rejects"),
            Err(err) => err,
        };
        assert!(
            err.contains(
                "invalid alignment for atomic operation; expected alignment is 1, actual alignment is 0"
            ),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_i32_narrow_rmw_add_bad_alignments() {
        let add8_overaligned = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[5, 4, 1, 0x03, 1, 1],
            &[10, 12, 1, 10, 0, 0x41, 0, 0x41, 5, 0xfe, 0x20, 1, 0, 0x0b],
        ]);
        let err = match parse_module(&add8_overaligned) {
            Ok(_) => panic!("add8 overalign rejects"),
            Err(err) => err,
        };
        assert!(
            err.contains(
                "i32.atomic.rmw.add8_u: invalid alignment; expected maximum alignment is 0, actual alignment is 1"
            ),
            "{err}"
        );

        let add16_underaligned = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[5, 4, 1, 0x03, 1, 1],
            &[10, 12, 1, 10, 0, 0x41, 0, 0x41, 5, 0xfe, 0x21, 0, 0, 0x0b],
        ]);
        let err = match parse_module(&add16_underaligned) {
            Ok(_) => panic!("add16 underalign rejects"),
            Err(err) => err,
        };
        assert!(
            err.contains(
                "i32.atomic.rmw.add16_u: invalid alignment for atomic operation; expected alignment is 1, actual alignment is 0"
            ),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_i32_atomic_rmw_add_underaligned_memarg() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[5, 4, 1, 0x03, 1, 1],
            &[
                10, 20, 1, 18, 0, 0x41, 0, 0x41, 40, 0xfe, 0x17, 2, 0, 0x41, 0, 0x41, 2, 0xfe,
                0x1e, 1, 0, 0x0b,
            ],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("underaligned atomic rmw.add must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains(
                "i32.atomic.rmw.add: invalid alignment for atomic operation; expected alignment is 2, actual alignment is 1"
            ),
            "{err}"
        );
    }

    #[test]
    fn validation_accepts_memory64_flag_on_table_limits() {
        let bytes = wasm_with_sections(&[
            &[4, 4, 1, 0x70, 0x04, 1],
        ]);
        parse_module(&bytes).expect("memory64 table limits validate");
    }

    #[test]
    fn validation_accepts_table64_size_i64_result() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7e],
            &[3, 2, 1, 0],
            &[4, 4, 1, 0x70, 0x04, 0],
            &[10, 7, 1, 5, 0, 0xfc, 0x10, 0, 0x0b],
        ]);
        parse_module(&bytes).expect("table64 table.size has i64 type");
    }

    #[test]
    fn validation_accepts_table64_active_elem_i64_offset() {
        let bytes = wasm_with_sections(&[
            &[4, 4, 1, 0x70, 0x04, 1],
            &[9, 6, 1, 0, 0x42, 0, 0x0b, 0],
        ]);
        parse_module(&bytes).expect("table64 active element offset accepts i64");
    }

    #[test]
    fn validation_accepts_table_copy_mixed_table64_length_i32() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[4, 7, 2, 0x70, 0x04, 30, 0x70, 0x00, 30],
            &[
                10, 14, 1, 12, 0, 0x42, 13, 0x41, 2, 0x41, 3, 0xfc, 14, 0, 1, 0x0b,
            ],
        ]);
        parse_module(&bytes).expect("table.copy mixed table64 uses i32 length");
    }

    #[test]
    fn validation_accepts_table_copy_table64_length_i64() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[4, 7, 2, 0x70, 0x04, 30, 0x70, 0x04, 30],
            &[
                10, 14, 1, 12, 0, 0x42, 13, 0x42, 2, 0x42, 3, 0xfc, 14, 0, 1, 0x0b,
            ],
        ]);
        parse_module(&bytes).expect("table.copy table64-table64 uses i64 length");
    }

    #[test]
    fn validation_accepts_call_indirect_table64_i64_index() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[4, 4, 1, 0x70, 0x04, 1],
            &[10, 9, 1, 7, 0, 0x42, 0, 0x11, 0, 0, 0x0b],
        ]);
        parse_module(&bytes).expect("call_indirect table64 uses i64 table index");
    }

    #[test]
    fn validation_rejects_call_indirect_non_function_table() {
        let bytes = wasm_with_sections(&[
            &[1, 4, 1, 0x60, 0, 0],
            &[3, 2, 1, 0],
            &[4, 4, 1, 0x6f, 0x00, 1],
            &[10, 9, 1, 7, 0, 0x41, 0, 0x11, 0, 0, 0x0b],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("call_indirect through externref table must reject"),
            Err(err) => err,
        };
        assert!(err.contains("not a function reference"), "{err}");
    }

    #[test]
    fn validation_accepts_table_grow_typed_func_ref_into_funcref_table() {
        let bytes = wasm_with_sections(&[
            &[
                1, 8, 2,
                0x60, 0, 0,
                0x60, 0, 1, 0x7f,
            ],
            &[3, 3, 2, 0, 1],
            &[4, 4, 1, 0x70, 0, 1],
            &[9, 5, 1, 3, 0, 1, 0],
            &[
                10, 14, 2,
                2, 0, 0x0b,
                9, 0, 0xd2, 0, 0x41, 1, 0xfc, 15, 0, 0x0b,
            ],
        ]);
        parse_module(&bytes).expect("table.grow accepts typed function reference for funcref");
    }

    #[test]
    fn validation_accepts_local_set_typed_func_ref_into_funcref_local() {
        let bytes = wasm_with_sections(&[
            &[
                1, 8, 2,
                0x60, 0, 0,
                0x60, 0, 1, 0x70,
            ],
            &[3, 3, 2, 0, 1],
            &[9, 5, 1, 3, 0, 1, 0],
            &[
                10, 15, 2,
                2, 0, 0x0b,
                10, 1, 1, 0x70,
                0xd2, 0,
                0x21, 0,
                0x20, 0,
                0x0b,
            ],
        ]);
        parse_module(&bytes).expect("local.set accepts typed function reference for funcref");
    }

    #[test]
    fn validation_rejects_memory64_size_as_i32_result() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[5, 3, 1, 0x04, 1],
            &[10, 6, 1, 4, 0, 0x3f, 0, 11],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("memory64 memory.size has i64 type"),
            Err(err) => err,
        };
        assert!(
            err.contains("expected I32") || err.contains("got I64"),
            "{err}"
        );
    }

    #[test]
    fn validation_accepts_return_call() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 3, 2, 0, 0],
            &[10, 11, 2, 4, 0, 0x41, 7, 11, 4, 0, 0x12, 0, 11],
        ]);
        parse_module(&bytes).expect("return_call is accepted");
    }

    #[test]
    fn validation_rejects_return_call_result_mismatch() {
        let bytes = wasm_with_sections(&[
            &[
                1, 9, 2,
                0x60, 0, 1, 0x7f,
                0x60, 0, 1, 0x7e,
            ],
            &[3, 3, 2, 0, 1],
            &[10, 11, 2, 4, 0, 0x41, 7, 11, 4, 0, 0x12, 0, 11],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("return_call result mismatch must reject"),
            Err(err) => err,
        };
        assert!(err.contains("return_call result type"), "{err}");
    }

    #[test]
    fn validation_rejects_active_data_without_memory() {
        let bytes = wasm_with_sections(&[
            &[11, 7, 1, 0, 0x41, 0, 11, 1, 0xaa],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("active data without memory must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("data segment 0 memory index 0 out of bounds (0 memories)"),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_active_data_explicit_memory_out_of_bounds() {
        let bytes = wasm_with_sections(&[
            &[5, 3, 1, 0, 1],
            &[11, 7, 1, 2, 1, 0x41, 0, 11, 0],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("active data memory oob must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("data segment 0 memory index 1 out of bounds (1 memories)"),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_memory_init_without_datacount() {
        let passive_data = [11, 4, 1, 1, 1, 0xaa];
        let bytes = bulk_module(
            &[0x41, 0, 0x41, 0, 0x41, 0, 0xfc, 0x08, 0, 0],
            None,
            Some(&passive_data),
        );
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("memory.init without datacount must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("memory.init requires DataCount section"),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_memory_init_data_index_out_of_bounds() {
        let passive_data = [11, 4, 1, 1, 1, 0xaa];
        let bytes = bulk_module(
            &[0x41, 0, 0x41, 0, 0x41, 0, 0xfc, 0x08, 1, 0],
            Some(1),
            Some(&passive_data),
        );
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("memory.init data oob must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("memory.init data index 1 out of bounds (1 data segments)"),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_data_drop_without_datacount() {
        let passive_data = [11, 4, 1, 1, 1, 0xaa];
        let bytes = bulk_module(&[0xfc, 0x09, 0], None, Some(&passive_data));
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("data.drop without datacount must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("data.drop requires DataCount section"),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_data_drop_index_out_of_bounds() {
        let passive_data = [11, 4, 1, 1, 1, 0xaa];
        let bytes = bulk_module(&[0xfc, 0x09, 1], Some(1), Some(&passive_data));
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("data.drop data oob must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("data.drop data index 1 out of bounds (1 data segments)"),
            "{err}"
        );
    }

    #[test]
    fn validation_accepts_bulk_memory_active_data_target() {
        let active_data = [11, 7, 1, 0, 0x41, 0, 0x0b, 1, 0xaa];
        let bytes = bulk_module(&[0xfc, 0x09, 0], Some(1), Some(&active_data));
        parse_module(&bytes).expect("Node/V8 accepts data.drop targeting an active data segment");
    }

    #[test]
    fn validation_accepts_externref_param_drop() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 1, 0x6f, 0],
            &[3, 2, 1, 0],
            &[10, 7, 1, 5, 0, 0x20, 0, 0x1a, 11],
        ]);
        parse_module(&bytes).expect("externref param is a valid reference type");
    }

    #[test]
    fn validation_accepts_funcref_ref_null_result() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x70],
            &[3, 2, 1, 0],
            &[10, 6, 1, 4, 0, 0xd0, 0x70, 11],
        ]);
        parse_module(&bytes).expect("ref.null func produces funcref");
    }

    #[test]
    fn validation_accepts_structref_ref_null_result() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x6b],
            &[3, 2, 1, 0],
            &[10, 6, 1, 4, 0, 0xd0, 0x6b, 11],
        ]);
        parse_module(&bytes).expect("ref.null struct produces structref");
    }

    #[test]
    fn validation_rejects_structref_result_from_extern_null() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x6b],
            &[3, 2, 1, 0],
            &[10, 6, 1, 4, 0, 0xd0, 0x6f, 11],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("extern null must not satisfy structref result"),
            Err(err) => err,
        };
        assert!(err.contains("type mismatch"), "{err}");
    }

    #[test]
    fn validation_accepts_externref_global_null_initializer() {
        let bytes = wasm_with_sections(&[
            &[6, 6, 1, 0x6f, 0, 0xd0, 0x6f, 11],
        ]);
        parse_module(&bytes).expect("ref.null extern is a valid global initializer");
    }

    #[test]
    fn validation_accepts_noexn_null_reference_type() {
        let bytes = wasm_with_sections(&[
            &[6, 6, 1, 0x74, 0, 0xd0, 0x74, 11],
        ]);
        parse_module(&bytes).expect("noexn null reference type is accepted");
    }

    #[test]
    fn validation_rejects_ref_null_non_reference_type() {
        let bytes = wasm_with_sections(&[
            &[1, 5, 1, 0x60, 0, 1, 0x7f],
            &[3, 2, 1, 0],
            &[10, 6, 1, 4, 0, 0xd0, 0x7f, 11],
        ]);
        let err = match parse_module(&bytes) {
            Ok(_) => panic!("ref.null i32 must reject"),
            Err(err) => err,
        };
        assert!(
            err.contains("unsupported reference heap type 0x7f"),
            "{err}"
        );
    }
}
