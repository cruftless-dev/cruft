
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {

    PushNull = 0x01,
    PushUndef = 0x02,
    PushTrue = 0x03,
    PushFalse = 0x04,

    PushTDZ = 0x0a,

    InitLocal = 0x0b,

    SetThisTDZ = 0x0c,

    PushThisRaw = 0x0d,

    PushI32 = 0x05,

    PushConst = 0x06,
    Pop = 0x07,
    Dup = 0x08,
    Swap = 0x09,

    LoadLocal = 0x10,

    StoreLocal = 0x11,

    StoreLocalHoistedFunc = 0x2a,

    LoadArg = 0x12,

    StoreArg = 0x13,

    LoadGlobal = 0x14,

    StoreGlobal = 0x15,

    LoadUpvalue = 0x16,

    StoreUpvalue = 0x17,

    DefineLocal = 0x18,

    ResetLocalCell = 0x19,

    LoadWithName = 0x1A,

    StoreWithName = 0x1B,

    EnterWith = 0x1C,

    ExitWith = 0x1D,

    ResolveWithName = 0x1E,

    LoadWithNameRef = 0x1F,

    Add = 0x20,
    Sub = 0x21,
    Mul = 0x22,
    Div = 0x23,
    Mod = 0x24,
    Pow = 0x25,
    Neg = 0x26,
    Pos = 0x27,
    Inc = 0x28,
    Dec = 0x29,

    Lt = 0x30,
    Gt = 0x31,
    Le = 0x32,
    Ge = 0x33,
    Eq = 0x34,
    Ne = 0x35,
    StrictEq = 0x36,
    StrictNe = 0x37,
    In = 0x38,
    Instanceof = 0x39,

    BitAnd = 0x40,
    BitOr = 0x41,
    BitXor = 0x42,
    BitNot = 0x43,
    Shl = 0x44,
    Shr = 0x45,
    UShr = 0x46,

    Not = 0x50,

    Jump = 0x60,

    JumpIfTrue = 0x61,

    JumpIfFalse = 0x62,

    JumpIfTrueKeep = 0x63,

    JumpIfFalseKeep = 0x64,

    JumpIfNullish = 0x65,

    ModuleAwait = 0x66,

    Await = 0x67,

    Call = 0x70,

    New = 0x71,
    Return = 0x72,
    ReturnUndef = 0x73,

    CallMethod = 0x74,

    TailCall = 0x7E,

    TailCallMethod = 0x7F,

    PushThis = 0x75,

    PushImportMeta = 0x76,

    PushNewTarget = 0x77,

    SetThis = 0x78,

    PropagateNewTarget = 0x79,

    DirectEval = 0x7A,

    DirectEvalApply = 0x68,

    Yield = 0x7B,

    YieldDelegate = 0x7C,

    AsyncYieldDelegate = 0x7D,

    GetProp = 0x80,

    SetProp = 0x81,
    GetIndex = 0x82,
    SetIndex = 0x83,

    SetPropStrict = 0x87,

    SetIndexStrict = 0x88,

    EnterPrivateHomeLocal = 0x89,

    ExitPrivateHome = 0x8A,

    GetSuperConstructor = 0x8B,

    CheckConstructor = 0x8C,

    SetPrototype = 0x84,

    SetClassPrototype = 0x85,

    SetClassConstructorParent = 0x86,

    CheckSuperclassConstructor = 0x0E,

    SuperclassPrototype = 0x0F,

    NewObject = 0x90,

    NewArray = 0x91,

    InitProp = 0x92,

    InitIndex = 0x93,

    InstallBoundaryWrapper = 0x94,

    NewObjectWithCapacity = 0x95,

    InitPropStaticSlot = 0x96,

    Typeof = 0xA0,
    Void = 0xA1,
    Delete = 0xA2,

    DeleteProp = 0xA3,

    DeleteIndex = 0xA4,

    StoreWithNameRef = 0xA5,

    ToString = 0xA6,

    DeleteLocal = 0xA7,

    DeleteUpvalue = 0xA8,

    DeleteWithName = 0xA9,

    RequireObjectCoercible = 0xAA,

    ToPropertyKey = 0xAB,

    StoreUpvalueRaw = 0xAC,

    ToNumeric = 0xAD,

    LoadWithNameOrLocal = 0xAE,

    ToNumberIndexCall = 0xAF,

    MakeClosure = 0xB0,

    MakeArrow = 0xB1,

    CaptureLocal = 0xB2,

    CaptureUpvalue = 0xB3,

    CallMethodThenInlineArrow = 0xB4,

    Throw = 0xC0,

    TryEnter = 0xC1,
    TryExit = 0xC2,

    IterInit = 0xD0,
    IterNext = 0xD1,
    IterClose = 0xD2,

    Nop = 0xE0,
    Debugger = 0xE1,

    PushConst32 = 0xE2,

    PushLiteral = 0xE4,

    PushLiteral32 = 0xE5,

    GetArrayLengthDirect = 0xE3,

    AddI64 = 0xF0,
    SubI64 = 0xF1,
    MulI64 = 0xF2,
    IncI64 = 0xF3,
    DecI64 = 0xF4,
    LtI64 = 0xF5,
    LeI64 = 0xF6,
    GtI64 = 0xF7,
    GeI64 = 0xF8,
    EqI64 = 0xF9,
    NeI64 = 0xFA,

    GetPropOnObject = 0xFB,

    CallMethodIcCached = 0xFC,

    GetPropSkipForMethod = 0xFD,

    ForOfFastNext = 0xFE,

    LoadGlobalOrUndef = 0xFF,
}

impl Op {

    pub fn operand_size(self) -> usize {
        use Op::*;
        match self {
            PushNull
            | PushUndef
            | PushTrue
            | PushFalse
            | PushTDZ
            | SetThisTDZ
            | PushThisRaw
            | Pop
            | Dup
            | Swap
            | Add
            | Sub
            | Mul
            | Div
            | Mod
            | Pow
            | Neg
            | Pos
            | Inc
            | Dec
            | Lt
            | Gt
            | Le
            | Ge
            | Eq
            | Ne
            | StrictEq
            | StrictNe
            | In
            | Instanceof
            | BitAnd
            | BitOr
            | BitXor
            | BitNot
            | Shl
            | Shr
            | UShr
            | Not
            | Return
            | ReturnUndef
            | GetIndex
            | SetIndex
            | SetIndexStrict
            | ExitPrivateHome
            | GetSuperConstructor
            | CheckConstructor
            | SetPrototype
            | SetClassPrototype
            | SetClassConstructorParent
            | NewObject
            | Typeof
            | Void
            | Delete
            | DeleteIndex
            | ToString
            | RequireObjectCoercible
            | ToPropertyKey
            | ToNumeric
            | ToNumberIndexCall
            | Throw
            | TryExit
            | IterInit
            | IterNext
            | IterClose
            | Nop
            | Debugger
            | PushThis
            | PushImportMeta
            | PushNewTarget
            | SetThis
            | PropagateNewTarget
            | EnterWith
            | ExitWith
            | AddI64
            | SubI64
            | MulI64
            | IncI64
            | DecI64
            | LtI64
            | LeI64
            | GtI64
            | GeI64
            | EqI64
            | NeI64
            | Yield
            | YieldDelegate
            | AsyncYieldDelegate
            | DirectEvalApply
            | ModuleAwait
            | Await
            | CheckSuperclassConstructor
            | CallMethodThenInlineArrow
            | SuperclassPrototype => 0,
            Call | New | CallMethod | DirectEval | CallMethodIcCached | TailCall
            | TailCallMethod => 1,
            PushConst
            | PushLiteral
            | LoadLocal
            | StoreLocal
            | StoreLocalHoistedFunc
            | InitLocal
            | LoadArg
            | StoreArg
            | LoadGlobal
            | LoadGlobalOrUndef
            | StoreGlobal
            | LoadUpvalue
            | StoreUpvalue
            | DefineLocal
            | ResetLocalCell
            | LoadWithName
            | StoreWithName
            | ResolveWithName
            | LoadWithNameRef
            | GetProp
            | GetPropOnObject
            | GetPropSkipForMethod
            | GetArrayLengthDirect
            | SetProp
            | NewArray
            | InitProp
            | NewObjectWithCapacity
            | SetPropStrict
            | EnterPrivateHomeLocal
            | MakeClosure
            | MakeArrow
            | CaptureLocal
            | CaptureUpvalue
            | DeleteProp
            | StoreWithNameRef
            | DeleteLocal
            | DeleteUpvalue
            | DeleteWithName
            | StoreUpvalueRaw => 2,
            LoadWithNameOrLocal | InitPropStaticSlot => 4,
            PushI32
            | PushConst32
            | PushLiteral32
            | Jump
            | JumpIfTrue
            | JumpIfFalse
            | JumpIfTrueKeep
            | JumpIfFalseKeep
            | JumpIfNullish
            | InitIndex
            | TryEnter
            | InstallBoundaryWrapper => 4,
            ForOfFastNext => 10,
        }
    }
}

pub fn encode_op(buf: &mut Vec<u8>, op: Op) -> usize {
    buf.push(op as u8);
    buf.len()
}

pub fn encode_u8(buf: &mut Vec<u8>, v: u8) {
    buf.push(v);
}

pub fn encode_u16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_le_bytes());
}

pub fn encode_i32(buf: &mut Vec<u8>, v: i32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

pub fn encode_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

pub fn decode_u16(bc: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([bc[off], bc[off + 1]])
}

pub fn decode_i32(bc: &[u8], off: usize) -> i32 {
    i32::from_le_bytes([bc[off], bc[off + 1], bc[off + 2], bc[off + 3]])
}

pub fn decode_u32(bc: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([bc[off], bc[off + 1], bc[off + 2], bc[off + 3]])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LazyArrowCaptureSource {
    Local,
    Upvalue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LazyArrowCaptureDescriptor {
    pub source: LazyArrowCaptureSource,
    pub slot: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallMethodThenInlineArrowPayload {
    pub proto_idx: u16,
    pub captures: Vec<LazyArrowCaptureDescriptor>,
}

pub fn encode_call_method_then_inline_arrow(
    buf: &mut Vec<u8>,
    payload: &CallMethodThenInlineArrowPayload,
) {
    encode_op(buf, Op::CallMethodThenInlineArrow);
    encode_u16(buf, payload.proto_idx);
    encode_u16(buf, payload.captures.len() as u16);
    for capture in &payload.captures {
        encode_u8(
            buf,
            match capture.source {
                LazyArrowCaptureSource::Local => 0,
                LazyArrowCaptureSource::Upvalue => 1,
            },
        );
        encode_u16(buf, capture.slot);
    }
}

pub fn decode_call_method_then_inline_arrow_payload(
    bc: &[u8],
    off: usize,
) -> Option<(CallMethodThenInlineArrowPayload, usize)> {
    if off + 4 > bc.len() {
        return None;
    }
    let proto_idx = decode_u16(bc, off);
    let capture_count = decode_u16(bc, off + 2) as usize;
    let mut cursor = off + 4;
    let mut captures = Vec::with_capacity(capture_count);
    for _ in 0..capture_count {
        if cursor + 3 > bc.len() {
            return None;
        }
        let source = match bc[cursor] {
            0 => LazyArrowCaptureSource::Local,
            1 => LazyArrowCaptureSource::Upvalue,
            _ => return None,
        };
        let slot = decode_u16(bc, cursor + 1);
        captures.push(LazyArrowCaptureDescriptor { source, slot });
        cursor += 3;
    }
    Some((
        CallMethodThenInlineArrowPayload {
            proto_idx,
            captures,
        },
        cursor,
    ))
}

pub fn instruction_len_at(bc: &[u8], off: usize) -> Option<usize> {
    let op = op_from_byte(*bc.get(off)?)?;
    match op {
        Op::CallMethodThenInlineArrow => {
            let (_, next) = decode_call_method_then_inline_arrow_payload(bc, off + 1)?;
            Some(next - off)
        }
        _ => Some(1 + op.operand_size()),
    }
}

pub fn op_from_byte(b: u8) -> Option<Op> {
    use Op::*;
    Some(match b {
        0x01 => PushNull,
        0x02 => PushUndef,
        0x03 => PushTrue,
        0x04 => PushFalse,
        0x0a => PushTDZ,
        0x0b => InitLocal,
        0x0c => SetThisTDZ,
        0x0d => PushThisRaw,
        0x05 => PushI32,
        0x06 => PushConst,
        0x07 => Pop,
        0x08 => Dup,
        0x09 => Swap,
        0x10 => LoadLocal,
        0x11 => StoreLocal,
        0x2a => StoreLocalHoistedFunc,
        0x12 => LoadArg,
        0x13 => StoreArg,
        0x14 => LoadGlobal,
        0x15 => StoreGlobal,
        0x16 => LoadUpvalue,
        0x17 => StoreUpvalue,
        0x18 => DefineLocal,
        0x19 => ResetLocalCell,
        0x1A => LoadWithName,
        0x1B => StoreWithName,
        0x1C => EnterWith,
        0x1D => ExitWith,
        0x1E => ResolveWithName,
        0x1F => LoadWithNameRef,
        0x20 => Add,
        0x21 => Sub,
        0x22 => Mul,
        0x23 => Div,
        0x24 => Mod,
        0x25 => Pow,
        0x26 => Neg,
        0x27 => Pos,
        0x28 => Inc,
        0x29 => Dec,
        0x30 => Lt,
        0x31 => Gt,
        0x32 => Le,
        0x33 => Ge,
        0x34 => Eq,
        0x35 => Ne,
        0x36 => StrictEq,
        0x37 => StrictNe,
        0x38 => In,
        0x39 => Instanceof,
        0x40 => BitAnd,
        0x41 => BitOr,
        0x42 => BitXor,
        0x43 => BitNot,
        0x44 => Shl,
        0x45 => Shr,
        0x46 => UShr,
        0x50 => Not,
        0x60 => Jump,
        0x61 => JumpIfTrue,
        0x62 => JumpIfFalse,
        0x63 => JumpIfTrueKeep,
        0x64 => JumpIfFalseKeep,
        0x65 => JumpIfNullish,
        0x66 => ModuleAwait,
        0x67 => Await,
        0x68 => DirectEvalApply,
        0x70 => Call,
        0x71 => New,
        0x72 => Return,
        0x73 => ReturnUndef,
        0x74 => CallMethod,
        0x7E => TailCall,
        0x7F => TailCallMethod,
        0x75 => PushThis,
        0x76 => PushImportMeta,
        0x77 => PushNewTarget,
        0x78 => SetThis,
        0x79 => PropagateNewTarget,
        0x7A => DirectEval,
        0x7B => Yield,
        0x7C => YieldDelegate,
        0x7D => AsyncYieldDelegate,
        0x80 => GetProp,
        0x81 => SetProp,
        0x82 => GetIndex,
        0x83 => SetIndex,
        0x87 => SetPropStrict,
        0x88 => SetIndexStrict,
        0x89 => EnterPrivateHomeLocal,
        0x8A => ExitPrivateHome,
        0x8B => GetSuperConstructor,
        0x8C => CheckConstructor,
        0x84 => SetPrototype,
        0x85 => SetClassPrototype,
        0x86 => SetClassConstructorParent,
        0x0E => CheckSuperclassConstructor,
        0x0F => SuperclassPrototype,
        0x90 => NewObject,
        0x91 => NewArray,
        0x92 => InitProp,
        0x93 => InitIndex,
        0x94 => InstallBoundaryWrapper,
        0x95 => NewObjectWithCapacity,
        0x96 => InitPropStaticSlot,
        0xA0 => Typeof,
        0xA1 => Void,
        0xA2 => Delete,
        0xA3 => DeleteProp,
        0xA4 => DeleteIndex,
        0xA5 => StoreWithNameRef,
        0xA6 => ToString,
        0xA7 => DeleteLocal,
        0xA8 => DeleteUpvalue,
        0xA9 => DeleteWithName,
        0xAA => RequireObjectCoercible,
        0xAB => ToPropertyKey,
        0xAC => StoreUpvalueRaw,
        0xAD => ToNumeric,
        0xAE => LoadWithNameOrLocal,
        0xAF => ToNumberIndexCall,
        0xB0 => MakeClosure,
        0xB1 => MakeArrow,
        0xB2 => CaptureLocal,
        0xB3 => CaptureUpvalue,
        0xB4 => CallMethodThenInlineArrow,
        0xC0 => Throw,
        0xC1 => TryEnter,
        0xC2 => TryExit,
        0xD0 => IterInit,
        0xD1 => IterNext,
        0xD2 => IterClose,
        0xE0 => Nop,
        0xE1 => Debugger,
        0xE2 => PushConst32,
        0xE4 => PushLiteral,
        0xE5 => PushLiteral32,
        0xE3 => GetArrayLengthDirect,
        0xF0 => AddI64,
        0xF1 => SubI64,
        0xF2 => MulI64,
        0xF3 => IncI64,
        0xF4 => DecI64,
        0xF5 => LtI64,
        0xF6 => LeI64,
        0xF7 => GtI64,
        0xF8 => GeI64,
        0xF9 => EqI64,
        0xFA => NeI64,
        0xFB => GetPropOnObject,
        0xFC => CallMethodIcCached,
        0xFD => GetPropSkipForMethod,
        0xFE => ForOfFastNext,
        0xFF => LoadGlobalOrUndef,
        _ => return None,
    })
}
