
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeoptReason {

    IntegerOverflow {
        op_pc: u32,
    },

    BoundaryArgMismatch,

    ICShapeMismatch {
        ic_id: u32,
    },

    ICCallTargetChanged {
        ic_id: u32,
    },

    TypeWidening {
        local_slot: u32,
    },

    SideEffectCallError {
        call_pc: u32,
    },

    Site135HolePrototypeMiss {
        read_pc: u32,
    },
    RuntimeHelperBailout {
        helper_id: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JitLocation {

    Register(u8),

    StackSlot(i32),

    Constant(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeoptLiveLocal {

    pub interp_slot: u16,
    pub jit_location: JitLocation,
}

#[derive(Debug, Clone)]
pub struct DeoptSite {

    pub reason: DeoptReason,

    pub resume_pc: u32,

    pub live_locals: Vec<DeoptLiveLocal>,

    pub stack_depth: u8,

    pub stack_slots: Vec<DeoptLiveLocal>,
}

#[derive(Debug, Clone)]
pub struct DeoptRecoveredState {
    pub reason: DeoptReason,
    pub resume_pc: u32,

    pub local_values: Vec<(u16, i64)>,

    pub stack_values: Vec<(u16, i64)>,
}

pub const DEOPT_TRIP_FIXED_REGISTER_ARG_CAPACITY: usize = 4;
pub const DEOPT_CALL_FRAME_REGISTER_CAPACITY: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct DeoptCallFrame {
    pub site_id: u32,
    pub regs: [i64; DEOPT_CALL_FRAME_REGISTER_CAPACITY],

    pub frame_base: i64,
}

pub fn reconstruct_state(
    sites: &[DeoptSite],
    frame: &DeoptCallFrame,
) -> Option<DeoptRecoveredState> {
    let site = sites.get(frame.site_id as usize)?;
    let local_values = site
        .live_locals
        .iter()
        .map(|live| {
            let v = read_location(&live.jit_location, frame);
            (live.interp_slot, v)
        })
        .collect();
    let stack_values = site
        .stack_slots
        .iter()
        .map(|live| {
            let v = read_location(&live.jit_location, frame);
            (live.interp_slot, v)
        })
        .collect();
    Some(DeoptRecoveredState {
        reason: site.reason,
        resume_pc: site.resume_pc,
        local_values,
        stack_values,
    })
}

fn read_location(loc: &JitLocation, frame: &DeoptCallFrame) -> i64 {
    match loc {
        JitLocation::Register(idx) => frame.regs.get(*idx as usize).copied().unwrap_or(0),
        JitLocation::StackSlot(offset) => read_stack_slot(frame.frame_base, *offset),
        JitLocation::Constant(c) => *c,
    }
}

fn read_stack_slot(frame_base: i64, offset: i32) -> i64 {
    let Some(addr) = (frame_base as usize).checked_add(offset.max(0) as usize) else {
        return 0;
    };
    if addr == 0 || addr % std::mem::align_of::<i64>() != 0 {
        return 0;
    }

    unsafe { std::ptr::read(addr as *const i64) }
}

pub fn jit_deopt_thunk(sites: &[DeoptSite], frame: DeoptCallFrame) -> Option<DeoptRecoveredState> {
    reconstruct_state(sites, &frame)
}

#[derive(Debug, Clone, Copy)]
pub enum JitCallOutcome {

    Returned(i64),

    Deopted(u32  ),
}

pub type DeoptSiteTable = Vec<DeoptSite>;

thread_local! {

    pub static CURRENT_DEOPT_SITES: std::cell::Cell<*const DeoptSiteTable> = const { std::cell::Cell::new(std::ptr::null()) };

    pub static LAST_DEOPT_FRAME: std::cell::RefCell<Option<DeoptRecoveredState>> = const { std::cell::RefCell::new(None) };
}

pub static JIT_FORCE_SHAPE_TRIP: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub fn set_force_shape_trip(v: bool) {
    JIT_FORCE_SHAPE_TRIP.store(v, std::sync::atomic::Ordering::Relaxed);
}

pub fn get_force_shape_trip_addr() -> usize {
    &JIT_FORCE_SHAPE_TRIP as *const _ as usize
}

pub static OSR_DEOPT_FLAG: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn set_osr_deopt_flag() {
    OSR_DEOPT_FLAG.store(1, std::sync::atomic::Ordering::Relaxed);
}

pub fn clear_osr_deopt_flag() {
    OSR_DEOPT_FLAG.store(0, std::sync::atomic::Ordering::Relaxed);
}

pub fn get_osr_deopt_flag_addr() -> usize {
    &OSR_DEOPT_FLAG as *const _ as usize
}

pub const INLINE_IC_MAX_SITES: usize = 8192;

pub static INLINE_IC_SHAPE: [std::sync::atomic::AtomicUsize; INLINE_IC_MAX_SITES] = {
    const Z: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    [Z; INLINE_IC_MAX_SITES]
};
pub static INLINE_IC_SLOT: [std::sync::atomic::AtomicU32; INLINE_IC_MAX_SITES] = {
    const Z: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    [Z; INLINE_IC_MAX_SITES]
};

pub static INLINE_IC_HEAP_BASE: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

pub fn set_inline_ic_heap_base(base: usize) {
    INLINE_IC_HEAP_BASE.store(base, std::sync::atomic::Ordering::Relaxed);
}
pub fn inline_ic_shape_base_addr() -> usize {
    INLINE_IC_SHAPE.as_ptr() as usize
}
pub fn inline_ic_slot_base_addr() -> usize {
    INLINE_IC_SLOT.as_ptr() as usize
}
pub fn inline_ic_heap_base_addr() -> usize {
    &INLINE_IC_HEAP_BASE as *const _ as usize
}

pub const POLY_INLINE_IC_MAX_SHAPES: usize = 4;
const POLY_INLINE_IC_MAX_CELLS: usize = INLINE_IC_MAX_SITES * POLY_INLINE_IC_MAX_SHAPES;
pub const POLY_INDEXED_SLOT_MAX_LEN: usize = 256;

static POLY_INLINE_IC_SHAPES: [std::sync::atomic::AtomicUsize; POLY_INLINE_IC_MAX_CELLS] =
    [const { std::sync::atomic::AtomicUsize::new(0) }; POLY_INLINE_IC_MAX_CELLS];
static POLY_INLINE_IC_SLOTS: [std::sync::atomic::AtomicU32; POLY_INLINE_IC_MAX_CELLS] =
    [const { std::sync::atomic::AtomicU32::new(0) }; POLY_INLINE_IC_MAX_CELLS];
static POLY_INLINE_IC_COUNTS: [std::sync::atomic::AtomicU32; INLINE_IC_MAX_SITES] =
    [const { std::sync::atomic::AtomicU32::new(0) }; INLINE_IC_MAX_SITES];
static POLY_INDEXED_SLOT_LENS: [std::sync::atomic::AtomicU32; INLINE_IC_MAX_SITES] =
    [const { std::sync::atomic::AtomicU32::new(0) }; INLINE_IC_MAX_SITES];
static POLY_INDEXED_SLOT_PTRS: [std::sync::atomic::AtomicUsize; INLINE_IC_MAX_SITES] =
    [const { std::sync::atomic::AtomicUsize::new(0) }; INLINE_IC_MAX_SITES];
static POLY_INDEXED_VALUE_LENS: [std::sync::atomic::AtomicU32; INLINE_IC_MAX_SITES] =
    [const { std::sync::atomic::AtomicU32::new(0) }; INLINE_IC_MAX_SITES];
static POLY_INDEXED_VALUE_PTRS: [std::sync::atomic::AtomicUsize; INLINE_IC_MAX_SITES] =
    [const { std::sync::atomic::AtomicUsize::new(0) }; INLINE_IC_MAX_SITES];

static POLY_INDEXED_SLOT_PAYLOADS: std::sync::OnceLock<
    std::sync::Mutex<Vec<Box<[std::sync::atomic::AtomicU32]>>>,
> = std::sync::OnceLock::new();
static POLY_INDEXED_VALUE_PAYLOADS: std::sync::OnceLock<
    std::sync::Mutex<Vec<Box<[std::sync::atomic::AtomicU64]>>>,
> = std::sync::OnceLock::new();

pub fn poly_inline_ic_shape_base_addr() -> usize {
    POLY_INLINE_IC_SHAPES.as_ptr() as usize
}

pub fn poly_inline_ic_slot_base_addr() -> usize {
    POLY_INLINE_IC_SLOTS.as_ptr() as usize
}

pub fn poly_inline_ic_count_base_addr() -> usize {
    POLY_INLINE_IC_COUNTS.as_ptr() as usize
}

pub fn poly_indexed_slot_ptr_base_addr() -> usize {
    POLY_INDEXED_SLOT_PTRS.as_ptr() as usize
}

pub fn poly_indexed_slot_len_base_addr() -> usize {
    POLY_INDEXED_SLOT_LENS.as_ptr() as usize
}

pub fn poly_indexed_value_ptr_base_addr() -> usize {
    POLY_INDEXED_VALUE_PTRS.as_ptr() as usize
}

pub fn poly_indexed_value_len_base_addr() -> usize {
    POLY_INDEXED_VALUE_LENS.as_ptr() as usize
}

pub fn poly_inline_ic_clear(site_id: u32) {
    let site = site_id as usize;
    if site >= INLINE_IC_MAX_SITES {
        return;
    }
    POLY_INLINE_IC_COUNTS[site].store(0, std::sync::atomic::Ordering::Relaxed);
    let base = site * POLY_INLINE_IC_MAX_SHAPES;
    for lane in 0..POLY_INLINE_IC_MAX_SHAPES {
        POLY_INLINE_IC_SHAPES[base + lane].store(0, std::sync::atomic::Ordering::Relaxed);
        POLY_INLINE_IC_SLOTS[base + lane].store(0, std::sync::atomic::Ordering::Relaxed);
    }
    POLY_INDEXED_SLOT_LENS[site].store(0, std::sync::atomic::Ordering::Relaxed);
    POLY_INDEXED_VALUE_LENS[site].store(0, std::sync::atomic::Ordering::Relaxed);
    POLY_INDEXED_SLOT_PTRS[site].store(0, std::sync::atomic::Ordering::Relaxed);
    POLY_INDEXED_VALUE_PTRS[site].store(0, std::sync::atomic::Ordering::Relaxed);
}

pub fn poly_inline_ic_publish(site_id: u32, entries: &[(usize, u32)]) -> bool {
    let site = site_id as usize;
    if site >= INLINE_IC_MAX_SITES
        || entries.is_empty()
        || entries.len() > POLY_INLINE_IC_MAX_SHAPES
    {
        return false;
    }
    poly_inline_ic_clear(site_id);
    let base = site * POLY_INLINE_IC_MAX_SHAPES;
    for (lane, &(shape_bits, slot)) in entries.iter().enumerate() {
        if shape_bits == 0 {
            poly_inline_ic_clear(site_id);
            return false;
        }
        POLY_INLINE_IC_SHAPES[base + lane].store(shape_bits, std::sync::atomic::Ordering::Relaxed);
        POLY_INLINE_IC_SLOTS[base + lane].store(slot, std::sync::atomic::Ordering::Relaxed);
    }
    POLY_INLINE_IC_COUNTS[site].store(entries.len() as u32, std::sync::atomic::Ordering::Relaxed);
    true
}

pub fn poly_indexed_slots_publish(site_id: u32, slots: &[u32]) -> bool {
    let site = site_id as usize;
    if site >= INLINE_IC_MAX_SITES
        || slots.is_empty()
        || slots.len() > POLY_INDEXED_SLOT_MAX_LEN
        || slots.contains(&u32::MAX)
    {
        return false;
    }
    POLY_INDEXED_SLOT_LENS[site].store(0, std::sync::atomic::Ordering::Release);
    let payload: Box<[std::sync::atomic::AtomicU32]> = slots
        .iter()
        .copied()
        .map(std::sync::atomic::AtomicU32::new)
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let ptr = payload.as_ptr() as usize;
    POLY_INDEXED_SLOT_PTRS[site].store(ptr, std::sync::atomic::Ordering::Release);
    POLY_INDEXED_SLOT_PAYLOADS
        .get_or_init(|| std::sync::Mutex::new(Vec::new()))
        .lock()
        .expect("poly indexed slot payload lock poisoned")
        .push(payload);
    POLY_INDEXED_SLOT_LENS[site].store(slots.len() as u32, std::sync::atomic::Ordering::Release);
    true
}

pub fn poly_indexed_values_publish(site_id: u32, values: &[f64]) -> bool {
    let site = site_id as usize;
    if site >= INLINE_IC_MAX_SITES
        || values.is_empty()
        || values.len() > POLY_INDEXED_SLOT_MAX_LEN
        || values.iter().any(|value| !value.is_finite())
    {
        return false;
    }
    POLY_INDEXED_VALUE_LENS[site].store(0, std::sync::atomic::Ordering::Release);
    let payload: Box<[std::sync::atomic::AtomicU64]> = values
        .iter()
        .map(|value| std::sync::atomic::AtomicU64::new(value.to_bits()))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let ptr = payload.as_ptr() as usize;
    POLY_INDEXED_VALUE_PTRS[site].store(ptr, std::sync::atomic::Ordering::Release);
    POLY_INDEXED_VALUE_PAYLOADS
        .get_or_init(|| std::sync::Mutex::new(Vec::new()))
        .lock()
        .expect("poly indexed value payload lock poisoned")
        .push(payload);
    POLY_INDEXED_VALUE_LENS[site].store(values.len() as u32, std::sync::atomic::Ordering::Release);
    true
}

pub fn poly_inline_ic_fact_for_test(site_id: u32) -> Vec<(usize, u32)> {
    let site = site_id as usize;
    if site >= INLINE_IC_MAX_SITES {
        return Vec::new();
    }
    let count = POLY_INLINE_IC_COUNTS[site].load(std::sync::atomic::Ordering::Relaxed) as usize;
    let base = site * POLY_INLINE_IC_MAX_SHAPES;
    let mut out = Vec::with_capacity(count.min(POLY_INLINE_IC_MAX_SHAPES));
    for lane in 0..count.min(POLY_INLINE_IC_MAX_SHAPES) {
        out.push((
            POLY_INLINE_IC_SHAPES[base + lane].load(std::sync::atomic::Ordering::Relaxed),
            POLY_INLINE_IC_SLOTS[base + lane].load(std::sync::atomic::Ordering::Relaxed),
        ));
    }
    out
}

pub fn poly_indexed_slot_fact_for_test(site_id: u32) -> Vec<u32> {
    let site = site_id as usize;
    if site >= INLINE_IC_MAX_SITES {
        return Vec::new();
    }
    let len = POLY_INDEXED_SLOT_LENS[site].load(std::sync::atomic::Ordering::Relaxed) as usize;
    let ptr = POLY_INDEXED_SLOT_PTRS[site].load(std::sync::atomic::Ordering::Acquire);
    if ptr == 0 || len == 0 {
        return Vec::new();
    }
    let len = len.min(POLY_INDEXED_SLOT_MAX_LEN);
    let slice =
        unsafe { std::slice::from_raw_parts(ptr as *const std::sync::atomic::AtomicU32, len) };
    slice
        .iter()
        .map(|cell| cell.load(std::sync::atomic::Ordering::Relaxed))
        .collect()
}

pub fn poly_indexed_value_fact_for_test(site_id: u32) -> Vec<f64> {
    let site = site_id as usize;
    if site >= INLINE_IC_MAX_SITES {
        return Vec::new();
    }
    let len = POLY_INDEXED_VALUE_LENS[site].load(std::sync::atomic::Ordering::Relaxed) as usize;
    let ptr = POLY_INDEXED_VALUE_PTRS[site].load(std::sync::atomic::Ordering::Acquire);
    if ptr == 0 || len == 0 {
        return Vec::new();
    }
    let len = len.min(POLY_INDEXED_SLOT_MAX_LEN);
    let slice =
        unsafe { std::slice::from_raw_parts(ptr as *const std::sync::atomic::AtomicU64, len) };
    slice
        .iter()
        .map(|cell| f64::from_bits(cell.load(std::sync::atomic::Ordering::Relaxed)))
        .collect()
}

const MWOR_VALUE_PTR_MAX_PROPS: usize = 65_536;
static MWOR_VALUE_PTRS: [std::sync::atomic::AtomicUsize; MWOR_VALUE_PTR_MAX_PROPS] =
    [const { std::sync::atomic::AtomicUsize::new(0) }; MWOR_VALUE_PTR_MAX_PROPS];

pub fn mwor_value_ptr_base_addr() -> usize {
    MWOR_VALUE_PTRS.as_ptr() as usize
}

pub fn mwor_clear_value_ptrs(prop_indices: &[u16]) {
    for &idx in prop_indices {
        MWOR_VALUE_PTRS[idx as usize].store(0, std::sync::atomic::Ordering::Relaxed);
    }
}

pub fn mwor_set_value_ptr(prop_idx: u16, ptr: usize) {
    MWOR_VALUE_PTRS[prop_idx as usize].store(ptr, std::sync::atomic::Ordering::Relaxed);
}

pub fn mwor_value_ptr_for_test(prop_idx: u16) -> usize {
    MWOR_VALUE_PTRS[prop_idx as usize].load(std::sync::atomic::Ordering::Relaxed)
}

pub const ARIA_SHAPE_CELL_LHS_NAME: usize = 0;
pub const ARIA_SHAPE_CELL_RHS_NAME: usize = 1;
pub const ARIA_SHAPE_CELL_LHS_CONSTRAINTS: usize = 2;
pub const ARIA_SHAPE_CELL_RHS_CONSTRAINTS: usize = 3;
pub const ARIA_SHAPE_CELL_LHS_ATTRIBUTES: usize = 4;
pub const ARIA_SHAPE_CELL_RHS_ATTRIBUTES: usize = 5;
pub const ARIA_SHAPE_CELL_COUNT: usize = 6;

static ARIA_SHAPE_VALUE_CELL_PTRS: [std::sync::atomic::AtomicUsize; ARIA_SHAPE_CELL_COUNT] =
    [const { std::sync::atomic::AtomicUsize::new(0) }; ARIA_SHAPE_CELL_COUNT];

pub fn aria_shape_value_cell_base_addr() -> usize {
    ARIA_SHAPE_VALUE_CELL_PTRS.as_ptr() as usize
}

pub fn aria_shape_value_cell_clear() {
    for cell in &ARIA_SHAPE_VALUE_CELL_PTRS {
        cell.store(0, std::sync::atomic::Ordering::Relaxed);
    }
}

pub fn aria_shape_value_cell_set(cell: usize, ptr: usize) {
    if let Some(slot) = ARIA_SHAPE_VALUE_CELL_PTRS.get(cell) {
        slot.store(ptr, std::sync::atomic::Ordering::Relaxed);
    }
}

pub fn aria_shape_value_cell_for_test(cell: usize) -> usize {
    ARIA_SHAPE_VALUE_CELL_PTRS
        .get(cell)
        .map(|slot| slot.load(std::sync::atomic::Ordering::Relaxed))
        .unwrap_or(0)
}

static ARIA_DICTIONARY_VALUE_CELL_PTRS: [std::sync::atomic::AtomicUsize; ARIA_SHAPE_CELL_COUNT] =
    [const { std::sync::atomic::AtomicUsize::new(0) }; ARIA_SHAPE_CELL_COUNT];

pub fn aria_dictionary_value_cell_base_addr() -> usize {
    ARIA_DICTIONARY_VALUE_CELL_PTRS.as_ptr() as usize
}

pub fn aria_dictionary_value_cell_clear() {
    for cell in &ARIA_DICTIONARY_VALUE_CELL_PTRS {
        cell.store(0, std::sync::atomic::Ordering::Relaxed);
    }
}

pub fn aria_dictionary_value_cell_set(cell: usize, ptr: usize) {
    if let Some(slot) = ARIA_DICTIONARY_VALUE_CELL_PTRS.get(cell) {
        slot.store(ptr, std::sync::atomic::Ordering::Relaxed);
    }
}

pub fn aria_dictionary_value_cell_for_test(cell: usize) -> usize {
    ARIA_DICTIONARY_VALUE_CELL_PTRS
        .get(cell)
        .map(|slot| slot.load(std::sync::atomic::Ordering::Relaxed))
        .unwrap_or(0)
}

const PACKED_LEAF_FACT_MAX_SLOTS: usize = 256;
static PACKED_LEAF_DENSE_PTRS: [std::sync::atomic::AtomicUsize; PACKED_LEAF_FACT_MAX_SLOTS] =
    [const { std::sync::atomic::AtomicUsize::new(0) }; PACKED_LEAF_FACT_MAX_SLOTS];
static PACKED_LEAF_DENSE_LENS: [std::sync::atomic::AtomicUsize; PACKED_LEAF_FACT_MAX_SLOTS] =
    [const { std::sync::atomic::AtomicUsize::new(0) }; PACKED_LEAF_FACT_MAX_SLOTS];

pub fn packed_leaf_dense_ptr_base_addr() -> usize {
    PACKED_LEAF_DENSE_PTRS.as_ptr() as usize
}

pub fn packed_leaf_dense_len_base_addr() -> usize {
    PACKED_LEAF_DENSE_LENS.as_ptr() as usize
}

pub fn packed_leaf_clear_facts(slots: &[u16]) {
    for &slot in slots {
        let idx = slot as usize;
        if idx < PACKED_LEAF_FACT_MAX_SLOTS {
            PACKED_LEAF_DENSE_PTRS[idx].store(0, std::sync::atomic::Ordering::Relaxed);
            PACKED_LEAF_DENSE_LENS[idx].store(0, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

pub fn packed_leaf_set_fact(slot: u16, ptr: usize, len: usize) {
    let idx = slot as usize;
    if idx < PACKED_LEAF_FACT_MAX_SLOTS {
        PACKED_LEAF_DENSE_PTRS[idx].store(ptr, std::sync::atomic::Ordering::Relaxed);
        PACKED_LEAF_DENSE_LENS[idx].store(len, std::sync::atomic::Ordering::Relaxed);
    }
}

pub fn packed_leaf_fact_for_test(slot: u16) -> (usize, usize) {
    let idx = slot as usize;
    if idx < PACKED_LEAF_FACT_MAX_SLOTS {
        (
            PACKED_LEAF_DENSE_PTRS[idx].load(std::sync::atomic::Ordering::Relaxed),
            PACKED_LEAF_DENSE_LENS[idx].load(std::sync::atomic::Ordering::Relaxed),
        )
    } else {
        (0, 0)
    }
}

const STRING_DIRECT_FACT_MAX_SLOTS: usize = 256;
static STRING_DIRECT_DATA_PTRS: [std::sync::atomic::AtomicUsize; STRING_DIRECT_FACT_MAX_SLOTS] =
    [const { std::sync::atomic::AtomicUsize::new(0) }; STRING_DIRECT_FACT_MAX_SLOTS];
static STRING_DIRECT_LENS: [std::sync::atomic::AtomicUsize; STRING_DIRECT_FACT_MAX_SLOTS] =
    [const { std::sync::atomic::AtomicUsize::new(0) }; STRING_DIRECT_FACT_MAX_SLOTS];

pub fn string_direct_data_ptr_base_addr() -> usize {
    STRING_DIRECT_DATA_PTRS.as_ptr() as usize
}

pub fn string_direct_len_base_addr() -> usize {
    STRING_DIRECT_LENS.as_ptr() as usize
}

pub fn string_direct_clear_facts(slots: &[u16]) {
    for &slot in slots {
        let idx = slot as usize;
        if idx < STRING_DIRECT_FACT_MAX_SLOTS {
            STRING_DIRECT_DATA_PTRS[idx].store(0, std::sync::atomic::Ordering::Relaxed);
            STRING_DIRECT_LENS[idx].store(0, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

pub fn string_direct_set_fact(slot: u16, ptr: usize, len: usize) {
    let idx = slot as usize;
    if idx < STRING_DIRECT_FACT_MAX_SLOTS {
        STRING_DIRECT_DATA_PTRS[idx].store(ptr, std::sync::atomic::Ordering::Relaxed);
        STRING_DIRECT_LENS[idx].store(len, std::sync::atomic::Ordering::Relaxed);
    }
}

const TYPED_ARRAY_DIRECT_FACT_MAX_SLOTS: usize = 256;
static TYPED_ARRAY_DIRECT_DATA_PTRS: [std::sync::atomic::AtomicUsize;
    TYPED_ARRAY_DIRECT_FACT_MAX_SLOTS] =
    [const { std::sync::atomic::AtomicUsize::new(0) }; TYPED_ARRAY_DIRECT_FACT_MAX_SLOTS];
static TYPED_ARRAY_DIRECT_LENS: [std::sync::atomic::AtomicUsize;
    TYPED_ARRAY_DIRECT_FACT_MAX_SLOTS] =
    [const { std::sync::atomic::AtomicUsize::new(0) }; TYPED_ARRAY_DIRECT_FACT_MAX_SLOTS];
static TYPED_ARRAY_DIRECT_KINDS: [std::sync::atomic::AtomicUsize;
    TYPED_ARRAY_DIRECT_FACT_MAX_SLOTS] =
    [const { std::sync::atomic::AtomicUsize::new(0) }; TYPED_ARRAY_DIRECT_FACT_MAX_SLOTS];

pub fn typed_array_direct_data_ptr_base_addr() -> usize {
    TYPED_ARRAY_DIRECT_DATA_PTRS.as_ptr() as usize
}

pub fn typed_array_direct_len_base_addr() -> usize {
    TYPED_ARRAY_DIRECT_LENS.as_ptr() as usize
}

pub fn typed_array_direct_kind_base_addr() -> usize {
    TYPED_ARRAY_DIRECT_KINDS.as_ptr() as usize
}

pub fn typed_array_direct_clear_facts(slots: &[u16]) {
    for &slot in slots {
        let idx = slot as usize;
        if idx < TYPED_ARRAY_DIRECT_FACT_MAX_SLOTS {
            TYPED_ARRAY_DIRECT_DATA_PTRS[idx].store(0, std::sync::atomic::Ordering::Relaxed);
            TYPED_ARRAY_DIRECT_LENS[idx].store(0, std::sync::atomic::Ordering::Relaxed);
            TYPED_ARRAY_DIRECT_KINDS[idx].store(0, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

pub fn typed_array_direct_set_fact(slot: u16, ptr: usize, len: usize, kind: i64) {
    let idx = slot as usize;
    if idx < TYPED_ARRAY_DIRECT_FACT_MAX_SLOTS {
        TYPED_ARRAY_DIRECT_DATA_PTRS[idx].store(ptr, std::sync::atomic::Ordering::Relaxed);
        TYPED_ARRAY_DIRECT_LENS[idx].store(len, std::sync::atomic::Ordering::Relaxed);
        TYPED_ARRAY_DIRECT_KINDS[idx].store(kind as usize, std::sync::atomic::Ordering::Relaxed);
    }
}

pub fn typed_array_direct_fact_for_test(slot: u16) -> (usize, usize, usize) {
    let idx = slot as usize;
    if idx < TYPED_ARRAY_DIRECT_FACT_MAX_SLOTS {
        (
            TYPED_ARRAY_DIRECT_DATA_PTRS[idx].load(std::sync::atomic::Ordering::Relaxed),
            TYPED_ARRAY_DIRECT_LENS[idx].load(std::sync::atomic::Ordering::Relaxed),
            TYPED_ARRAY_DIRECT_KINDS[idx].load(std::sync::atomic::Ordering::Relaxed),
        )
    } else {
        (0, 0, 0)
    }
}

#[cfg(test)]
mod typed_array_direct_fact_tests {
    #[test]
    fn typed_array_direct_fact_table_round_trips_and_clears() {
        super::typed_array_direct_clear_facts(&[7]);
        assert_eq!(super::typed_array_direct_fact_for_test(7), (0, 0, 0));

        super::typed_array_direct_set_fact(7, 0xabc0, 4096, 4);
        assert_eq!(
            super::typed_array_direct_fact_for_test(7),
            (0xabc0, 4096, 4)
        );

        super::typed_array_direct_clear_facts(&[7]);
        assert_eq!(super::typed_array_direct_fact_for_test(7), (0, 0, 0));
    }
}

pub fn string_direct_fact_for_test(slot: u16) -> (usize, usize) {
    let idx = slot as usize;
    if idx < STRING_DIRECT_FACT_MAX_SLOTS {
        (
            STRING_DIRECT_DATA_PTRS[idx].load(std::sync::atomic::Ordering::Relaxed),
            STRING_DIRECT_LENS[idx].load(std::sync::atomic::Ordering::Relaxed),
        )
    } else {
        (0, 0)
    }
}

pub fn inline_ic_populate(site_id: u32, shape_bits: usize, slot: u32) {
    let i = site_id as usize;
    if i < INLINE_IC_MAX_SITES {
        INLINE_IC_SHAPE[i].store(shape_bits, std::sync::atomic::Ordering::Relaxed);
        INLINE_IC_SLOT[i].store(slot, std::sync::atomic::Ordering::Relaxed);
    }
}

pub fn inline_ic_clear(site_id: u32) {
    inline_ic_populate(site_id, 0, 0);
}

pub fn inline_ic_fact_for_test(site_id: u32) -> (usize, u32) {
    let i = site_id as usize;
    if i < INLINE_IC_MAX_SITES {
        (
            INLINE_IC_SHAPE[i].load(std::sync::atomic::Ordering::Relaxed),
            INLINE_IC_SLOT[i].load(std::sync::atomic::Ordering::Relaxed),
        )
    } else {
        (0, 0)
    }
}

#[cfg(test)]
mod poly_inline_ic_tests {
    #[test]
    fn poly_inline_ic_fact_table_round_trips_clears_and_bounds() {
        super::poly_inline_ic_clear(17);
        assert!(super::poly_inline_ic_fact_for_test(17).is_empty());

        assert!(super::poly_inline_ic_publish(
            17,
            &[(0xaaa0, 1), (0xbbb0, 2)]
        ));
        assert_eq!(
            super::poly_inline_ic_fact_for_test(17),
            vec![(0xaaa0, 1), (0xbbb0, 2)]
        );

        assert!(!super::poly_inline_ic_publish(
            17,
            &[(0x1, 1), (0x2, 2), (0x3, 3), (0x4, 4), (0x5, 5)]
        ));
        assert_eq!(
            super::poly_inline_ic_fact_for_test(17),
            vec![(0xaaa0, 1), (0xbbb0, 2)],
            "rejected publish must not replace a valid prior fact"
        );

        assert!(!super::poly_inline_ic_publish(17, &[(0, 1)]));
        assert!(
            super::poly_inline_ic_fact_for_test(17).is_empty(),
            "zero shape is an invalid partial publish and clears the site"
        );
    }

    #[test]
    fn poly_inline_ic_codegen_bases_are_exposed() {
        assert_ne!(super::poly_inline_ic_shape_base_addr(), 0);
        assert_ne!(super::poly_inline_ic_slot_base_addr(), 0);
        assert_ne!(super::poly_inline_ic_count_base_addr(), 0);
        assert_ne!(super::poly_indexed_slot_ptr_base_addr(), 0);
        assert_ne!(super::poly_indexed_slot_len_base_addr(), 0);
        assert_ne!(super::poly_indexed_value_ptr_base_addr(), 0);
        assert_ne!(super::poly_indexed_value_len_base_addr(), 0);
    }

    #[test]
    fn poly_indexed_slot_table_round_trips_and_clears() {
        super::poly_inline_ic_clear(29);
        assert!(super::poly_indexed_slot_fact_for_test(29).is_empty());

        assert!(super::poly_indexed_slots_publish(29, &[1, 2, 2, 1]));
        assert_eq!(super::poly_indexed_slot_fact_for_test(29), vec![1, 2, 2, 1]);

        assert!(!super::poly_indexed_slots_publish(
            29,
            &vec![0; super::POLY_INDEXED_SLOT_MAX_LEN + 1]
        ));
        assert_eq!(super::poly_indexed_slot_fact_for_test(29), vec![1, 2, 2, 1]);

        super::poly_inline_ic_clear(29);
        assert!(super::poly_indexed_slot_fact_for_test(29).is_empty());
    }

    #[test]
    fn poly_indexed_value_table_round_trips_and_clears() {
        super::poly_inline_ic_clear(31);
        assert!(super::poly_indexed_value_fact_for_test(31).is_empty());

        assert!(super::poly_indexed_values_publish(31, &[1.25, -2.5, 3.75]));
        assert_eq!(
            super::poly_indexed_value_fact_for_test(31),
            vec![1.25, -2.5, 3.75]
        );

        assert!(!super::poly_indexed_values_publish(31, &[f64::NAN]));
        assert_eq!(
            super::poly_indexed_value_fact_for_test(31),
            vec![1.25, -2.5, 3.75]
        );

        assert!(!super::poly_indexed_values_publish(
            31,
            &vec![0.0; super::POLY_INDEXED_SLOT_MAX_LEN + 1]
        ));
        assert_eq!(
            super::poly_indexed_value_fact_for_test(31),
            vec![1.25, -2.5, 3.75]
        );

        super::poly_inline_ic_clear(31);
        assert!(super::poly_indexed_value_fact_for_test(31).is_empty());
    }

    #[test]
    fn poly_indexed_sparse_payload_replacement_and_clear_deny_stale_reads() {
        super::poly_inline_ic_clear(37);
        assert!(super::poly_indexed_slots_publish(37, &[4, 5, 6]));
        assert!(super::poly_indexed_values_publish(37, &[4.0, 5.0, 6.0]));
        assert_eq!(super::poly_indexed_slot_fact_for_test(37), vec![4, 5, 6]);
        assert_eq!(
            super::poly_indexed_value_fact_for_test(37),
            vec![4.0, 5.0, 6.0]
        );

        assert!(super::poly_indexed_slots_publish(37, &[8, 9]));
        assert!(super::poly_indexed_values_publish(37, &[8.0, 9.0]));
        assert_eq!(super::poly_indexed_slot_fact_for_test(37), vec![8, 9]);
        assert_eq!(super::poly_indexed_value_fact_for_test(37), vec![8.0, 9.0]);

        super::poly_inline_ic_clear(37);
        assert!(super::poly_indexed_slot_fact_for_test(37).is_empty());
        assert!(super::poly_indexed_value_fact_for_test(37).is_empty());
    }

    #[test]
    fn inline_ic_clear_empties_monomorphic_site_fact() {
        super::inline_ic_populate(23, 0x1111, 9);
        assert_eq!(super::inline_ic_fact_for_test(23), (0x1111, 9));

        super::inline_ic_clear(23);
        assert_eq!(super::inline_ic_fact_for_test(23), (0, 0));
    }
}

#[derive(Clone, Copy, Default)]
pub struct InlineIcLayout {
    pub slot_stride: i32,
    pub slot_payload_off: i32,
    pub object_shape_off: i32,
    pub object_shape_values_off: i32,
    pub vec_ptr_off: i32,
    pub value_size: i32,
    pub value_number_payload_off: i32,

    pub value_payload_off: i32,
    pub value_tag_number: i32,
    pub value_tag_string: i32,
    pub value_tag_object: i32,

    pub verified: bool,

    pub object_array_dense_off: i32,
    pub object_dense_elements_off: i32,
    pub vec_len_off: i32,
    pub vec_cap_off: i32,

    pub array_verified: bool,

    pub object_array_packed_all_safe_i64_off: i32,
    pub object_array_packed_off: i32,
    pub object_dense_doubles_off: i32,
    pub object_dense_i64_sidecar_valid_off: i32,
}

static INLINE_IC_LAYOUT: std::sync::RwLock<InlineIcLayout> =
    std::sync::RwLock::new(InlineIcLayout {
        slot_stride: 0,
        slot_payload_off: 0,
        object_shape_off: 0,
        object_shape_values_off: 0,
        vec_ptr_off: 0,
        value_size: 0,
        value_number_payload_off: 0,
        value_payload_off: 0,
        value_tag_number: 0,
        value_tag_string: 0,
        value_tag_object: 0,
        verified: false,
        object_array_dense_off: 0,
        object_dense_elements_off: 0,
        vec_len_off: 0,
        vec_cap_off: 0,
        array_verified: false,
        object_array_packed_all_safe_i64_off: 0,
        object_array_packed_off: 0,
        object_dense_doubles_off: 0,
        object_dense_i64_sidecar_valid_off: 0,
    });

pub fn set_inline_ic_layout(l: InlineIcLayout) {
    *INLINE_IC_LAYOUT.write().unwrap() = l;
}
pub fn inline_ic_layout() -> InlineIcLayout {
    *INLINE_IC_LAYOUT.read().unwrap()
}
pub fn inline_ic_enabled() -> bool {

    std::env::var("CRUFT_IC_INLINE")
        .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
        .unwrap_or(true)
        && inline_ic_layout().verified
}

pub fn inline_array_ic_enabled() -> bool {
    std::env::var("CRUFT_IC_INLINE")
        .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
        .unwrap_or(true)
        && inline_ic_layout().array_verified
}

pub type GetPropFn = extern "C" fn(i64, i64) -> i64;
pub type GetPropObjectFn = extern "C" fn(i64, i64) -> i64;
pub type GetPropObjectOrNullFn = extern "C" fn(i64, i64) -> i64;
pub type GetPropTruthyFn = extern "C" fn(i64, i64) -> f64;
pub type MworGetPropFn = extern "C" fn(i64, i64) -> i64;
pub type StringPropStrictEqFn = extern "C" fn(i64, i64, i64) -> f64;
pub type AriaStringContentEqFn = extern "C" fn(i64, i64) -> f64;
pub type CallDirect2PropPredicateFn = extern "C" fn(i64, i64, i64, i64) -> i64;
pub type AriaParentPredicateFusionFn = extern "C" fn(i64, i64, i64, i64, i64, i64, i64) -> i64;

thread_local! {
    static ACTIVE_GETPROP_FN: std::cell::Cell<Option<GetPropFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_GETPROP_OBJECT_FN: std::cell::Cell<Option<GetPropObjectFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_GETPROP_OBJECT_OR_NULL_FN: std::cell::Cell<Option<GetPropObjectOrNullFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_GETPROP_TRUTHY_FN: std::cell::Cell<Option<GetPropTruthyFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_MWOR_GETPROP_FN: std::cell::Cell<Option<MworGetPropFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_STRING_PROP_STRICT_EQ_FN: std::cell::Cell<Option<StringPropStrictEqFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_ARIA_STRING_CONTENT_EQ_FN: std::cell::Cell<Option<AriaStringContentEqFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_CALL_DIRECT2_PROP_PREDICATE_FN: std::cell::Cell<Option<CallDirect2PropPredicateFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_ARIA_PARENT_PREDICATE_FUSION_FN: std::cell::Cell<Option<AriaParentPredicateFusionFn>> =
        const { std::cell::Cell::new(None) };
    static ARIA_PARENT_PREDICATE_FUSION_BOUNDARY_TIMING:
        std::cell::RefCell<AriaParentPredicateFusionBoundaryTiming> =
            const { std::cell::RefCell::new(AriaParentPredicateFusionBoundaryTiming::new()) };
}

#[derive(Clone, Copy)]
struct AriaParentPredicateFusionBoundaryTiming {
    calls: u64,
    deopts: u64,
    false_rows: u64,
    true_rows: u64,
    total_ns: u128,
}

impl AriaParentPredicateFusionBoundaryTiming {
    const fn new() -> Self {
        Self {
            calls: 0,
            deopts: 0,
            false_rows: 0,
            true_rows: 0,
            total_ns: 0,
        }
    }
}

fn aria_parent_predicate_fusion_boundary_timing_enabled() -> bool {
    std::env::var_os("CRUFT_LEJIT_ARIA_PARENT_FUSION_BOUNDARY_TIMING").is_some()
}

fn aria_parent_predicate_fusion_stub_true_enabled() -> bool {
    std::env::var_os("CRUFT_LEJIT_ARIA_PARENT_FUSION_STUB_TRUE").is_some()
}

fn record_aria_parent_predicate_fusion_boundary_timing(result: i64, ns: u128) {
    if !aria_parent_predicate_fusion_boundary_timing_enabled() {
        return;
    }
    ARIA_PARENT_PREDICATE_FUSION_BOUNDARY_TIMING.with(|timing| {
        let mut t = timing.borrow_mut();
        t.calls = t.calls.saturating_add(1);
        match result {
            -1 => t.deopts = t.deopts.saturating_add(1),
            0 => t.false_rows = t.false_rows.saturating_add(1),
            _ => t.true_rows = t.true_rows.saturating_add(1),
        }
        t.total_ns = t.total_ns.saturating_add(ns);
        if t.calls <= 8 || t.calls.is_power_of_two() {
            let avg_total_ns = t.total_ns / t.calls as u128;
            eprintln!(
                "[aria-parent-fusion-boundary-timing] calls={} deopts={} false={} true={} avg_total_ns={}",
                t.calls, t.deopts, t.false_rows, t.true_rows, avg_total_ns
            );
        }
    });
}

pub fn set_active_getprop_fn(f: GetPropFn) {
    ACTIVE_GETPROP_FN.with(|c| c.set(Some(f)));
}

pub fn clear_active_getprop_fn() {
    ACTIVE_GETPROP_FN.with(|c| c.set(None));
}
pub fn set_active_getprop_object_fn(f: GetPropObjectFn) {
    ACTIVE_GETPROP_OBJECT_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_getprop_object_fn() {
    ACTIVE_GETPROP_OBJECT_FN.with(|c| c.set(None));
}
pub fn set_active_getprop_object_or_null_fn(f: GetPropObjectOrNullFn) {
    ACTIVE_GETPROP_OBJECT_OR_NULL_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_getprop_object_or_null_fn() {
    ACTIVE_GETPROP_OBJECT_OR_NULL_FN.with(|c| c.set(None));
}
pub fn set_active_getprop_truthy_fn(f: GetPropTruthyFn) {
    ACTIVE_GETPROP_TRUTHY_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_getprop_truthy_fn() {
    ACTIVE_GETPROP_TRUTHY_FN.with(|c| c.set(None));
}
pub fn set_active_mwor_getprop_fn(f: MworGetPropFn) {
    ACTIVE_MWOR_GETPROP_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_mwor_getprop_fn() {
    ACTIVE_MWOR_GETPROP_FN.with(|c| c.set(None));
}
pub fn set_active_string_prop_strict_eq_fn(f: StringPropStrictEqFn) {
    ACTIVE_STRING_PROP_STRICT_EQ_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_string_prop_strict_eq_fn() {
    ACTIVE_STRING_PROP_STRICT_EQ_FN.with(|c| c.set(None));
}
pub fn set_active_aria_string_content_eq_fn(f: AriaStringContentEqFn) {
    ACTIVE_ARIA_STRING_CONTENT_EQ_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_aria_string_content_eq_fn() {
    ACTIVE_ARIA_STRING_CONTENT_EQ_FN.with(|c| c.set(None));
}
pub fn set_active_call_direct2_prop_predicate_fn(f: CallDirect2PropPredicateFn) {
    ACTIVE_CALL_DIRECT2_PROP_PREDICATE_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_call_direct2_prop_predicate_fn() {
    ACTIVE_CALL_DIRECT2_PROP_PREDICATE_FN.with(|c| c.set(None));
}
pub fn set_active_aria_parent_predicate_fusion_fn(f: AriaParentPredicateFusionFn) {
    ACTIVE_ARIA_PARENT_PREDICATE_FUSION_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_aria_parent_predicate_fusion_fn() {
    ACTIVE_ARIA_PARENT_PREDICATE_FUSION_FN.with(|c| c.set(None));
}

pub type NewObjectFn = extern "C" fn(i64) -> i64;
pub type NewArrayFn = extern "C" fn() -> i64;
pub type InitIndexFn = extern "C" fn(i64, i64, f64);
pub type InitPropObjectFn = extern "C" fn(i64, i64, i64);
pub type InitPropNullFn = extern "C" fn(i64, i64);
thread_local! {
    static ACTIVE_NEWOBJECT_FN: std::cell::Cell<Option<NewObjectFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_NEWARRAY_FN: std::cell::Cell<Option<NewArrayFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_INITINDEX_FN: std::cell::Cell<Option<InitIndexFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_INITPROP_OBJECT_FN: std::cell::Cell<Option<InitPropObjectFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_INITPROP_NULL_FN: std::cell::Cell<Option<InitPropNullFn>> =
        const { std::cell::Cell::new(None) };
}
pub fn set_active_newobject_fn(f: NewObjectFn) {
    ACTIVE_NEWOBJECT_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_newarray_fn(f: NewArrayFn) {
    ACTIVE_NEWARRAY_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_initindex_fn(f: InitIndexFn) {
    ACTIVE_INITINDEX_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_initprop_object_fn(f: InitPropObjectFn) {
    ACTIVE_INITPROP_OBJECT_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_initprop_null_fn(f: InitPropNullFn) {
    ACTIVE_INITPROP_NULL_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_newobject_fn() {
    ACTIVE_NEWOBJECT_FN.with(|c| c.set(None));
}
pub fn clear_active_newarray_fn() {
    ACTIVE_NEWARRAY_FN.with(|c| c.set(None));
}
pub fn clear_active_initindex_fn() {
    ACTIVE_INITINDEX_FN.with(|c| c.set(None));
}
pub fn clear_active_initprop_object_fn() {
    ACTIVE_INITPROP_OBJECT_FN.with(|c| c.set(None));
}
pub fn clear_active_initprop_null_fn() {
    ACTIVE_INITPROP_NULL_FN.with(|c| c.set(None));
}
#[no_mangle]
pub extern "C" fn jit_new_object(capacity: i64) -> i64 {
    if let Some(f) = ACTIVE_NEWOBJECT_FN.with(|c| c.get()) {
        f(capacity)
    } else {
        0
    }
}
#[no_mangle]
pub extern "C" fn jit_new_array() -> i64 {
    if let Some(f) = ACTIVE_NEWARRAY_FN.with(|c| c.get()) {
        f()
    } else {
        0
    }
}
#[no_mangle]
pub extern "C" fn jit_initindex_on_array(receiver_idx: i64, index_i64: i64, value: f64) {
    if let Some(f) = ACTIVE_INITINDEX_FN.with(|c| c.get()) {
        f(receiver_idx, index_i64, value)
    }
}
#[no_mangle]
pub extern "C" fn jit_initprop_object_on_object(
    receiver_idx: i64,
    prop_name_idx: i64,
    value_obj_idx: i64,
) {
    if let Some(f) = ACTIVE_INITPROP_OBJECT_FN.with(|c| c.get()) {
        f(receiver_idx, prop_name_idx, value_obj_idx)
    }
}
#[no_mangle]
pub extern "C" fn jit_initprop_null_on_object(receiver_idx: i64, prop_name_idx: i64) {
    if let Some(f) = ACTIVE_INITPROP_NULL_FN.with(|c| c.get()) {
        f(receiver_idx, prop_name_idx)
    }
}

thread_local! {
    pub static CURRENT_RUNTIME: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub static CURRENT_PROTO: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub fn set_current_runtime(rt_ptr: usize) {
    CURRENT_RUNTIME.with(|c| c.set(rt_ptr));
}
pub fn clear_current_runtime() {
    CURRENT_RUNTIME.with(|c| c.set(0));
}
pub fn get_current_runtime() -> usize {
    CURRENT_RUNTIME.with(|c| c.get())
}

pub fn set_current_proto(proto_ptr: usize) {
    CURRENT_PROTO.with(|c| c.set(proto_ptr));
}
pub fn clear_current_proto() {
    CURRENT_PROTO.with(|c| c.set(0));
}
pub fn get_current_proto() -> usize {
    CURRENT_PROTO.with(|c| c.get())
}

#[no_mangle]
pub extern "C" fn jit_getprop_on_object(receiver_idx: i64, prop_name_idx: i64) -> i64 {
    if let Some(f) = ACTIVE_GETPROP_FN.with(|c| c.get()) {
        f(receiver_idx, prop_name_idx)
    } else {

        (receiver_idx << 8) ^ prop_name_idx
    }
}

#[no_mangle]
pub extern "C" fn jit_getprop_object_on_object(receiver_idx: i64, prop_name_idx: i64) -> i64 {
    if let Some(f) = ACTIVE_GETPROP_OBJECT_FN.with(|c| c.get()) {
        f(receiver_idx, prop_name_idx)
    } else {

        (receiver_idx << 8) ^ prop_name_idx
    }
}

#[no_mangle]
pub extern "C" fn jit_getprop_truthy_on_object(receiver_idx: i64, prop_name_idx: i64) -> f64 {
    if let Some(f) = ACTIVE_GETPROP_TRUTHY_FN.with(|c| c.get()) {
        f(receiver_idx, prop_name_idx)
    } else if ((receiver_idx << 8) ^ prop_name_idx) != 0 {
        1.0
    } else {
        0.0
    }
}

#[no_mangle]
pub extern "C" fn jit_mwor_getprop_on_object(receiver_idx: i64, prop_name_idx: i64) -> i64 {
    if let Some(f) = ACTIVE_MWOR_GETPROP_FN.with(|c| c.get()) {
        f(receiver_idx, prop_name_idx)
    } else {
        (receiver_idx << 8) ^ prop_name_idx
    }
}

#[no_mangle]
pub extern "C" fn jit_string_prop_strict_eq(lhs_idx: i64, rhs_idx: i64, prop_name_idx: i64) -> f64 {
    ACTIVE_STRING_PROP_STRICT_EQ_FN
        .with(|c| c.get())
        .map(|f| f(lhs_idx, rhs_idx, prop_name_idx))
        .unwrap_or(0.0)
}

#[no_mangle]
pub extern "C" fn jit_aria_string_content_eq(lhs_payload: i64, rhs_payload: i64) -> f64 {
    ACTIVE_ARIA_STRING_CONTENT_EQ_FN
        .with(|c| c.get())
        .map(|f| f(lhs_payload, rhs_payload))
        .unwrap_or(0.0)
}

pub type GetIndexFn = extern "C" fn(i64, i64) -> f64;
pub type GetIndexObjFn = extern "C" fn(i64, i64) -> i64;
pub type IndexedPrototypeEpochFn = extern "C" fn() -> i64;

thread_local! {
    static ACTIVE_GETINDEX_FN: std::cell::Cell<Option<GetIndexFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_GETINDEX_OBJ_FN: std::cell::Cell<Option<GetIndexObjFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_INDEXED_PROTOTYPE_EPOCH_FN: std::cell::Cell<Option<IndexedPrototypeEpochFn>> =
        const { std::cell::Cell::new(None) };
}

static GETINDEX_HELPER_CALL_COUNT: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

fn deopt_env_truthy(name: &str) -> bool {
    std::env::var(name)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

pub fn set_active_getindex_fn(f: GetIndexFn) {
    ACTIVE_GETINDEX_FN.with(|c| c.set(Some(f)));
}

pub fn set_active_getindex_obj_fn(f: GetIndexObjFn) {
    ACTIVE_GETINDEX_OBJ_FN.with(|c| c.set(Some(f)));
}

pub fn set_active_indexed_prototype_epoch_fn(f: IndexedPrototypeEpochFn) {
    ACTIVE_INDEXED_PROTOTYPE_EPOCH_FN.with(|c| c.set(Some(f)));
}

pub fn clear_active_getindex_fn() {
    ACTIVE_GETINDEX_FN.with(|c| c.set(None));
}

pub fn clear_active_getindex_obj_fn() {
    ACTIVE_GETINDEX_OBJ_FN.with(|c| c.set(None));
}

#[no_mangle]
pub extern "C" fn jit_getindex_on_object(receiver_idx: i64, index_i64: i64) -> f64 {
    if deopt_env_truthy("CRUFT_LEJIT_GETINDEX_HELPER_COUNT") {
        let n = GETINDEX_HELPER_CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if n == 1 || n % 1_000_000 == 0 {
            eprintln!("[getindex-helper-count] calls={n}");
        }
    }
    if let Some(f) = ACTIVE_GETINDEX_FN.with(|c| c.get()) {
        f(receiver_idx, index_i64)
    } else {

        ((receiver_idx << 8) ^ index_i64) as f64
    }
}

#[no_mangle]
pub extern "C" fn jit_getindex_obj_on_object(receiver_idx: i64, index_i64: i64) -> i64 {
    if let Some(f) = ACTIVE_GETINDEX_OBJ_FN.with(|c| c.get()) {
        f(receiver_idx, index_i64)
    } else {

        (receiver_idx << 8) ^ index_i64
    }
}

#[no_mangle]
pub extern "C" fn jit_indexed_prototype_epoch() -> i64 {
    ACTIVE_INDEXED_PROTOTYPE_EPOCH_FN
        .with(|c| c.get())
        .map(|f| f())
        .unwrap_or(1)
}

pub type ObjectLengthFn = extern "C" fn(i64) -> f64;

thread_local! {
    static ACTIVE_OBJECT_LENGTH_FN: std::cell::Cell<Option<ObjectLengthFn>> =
        const { std::cell::Cell::new(None) };
}

static OBJECT_LENGTH_HELPER_CALL_COUNT: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

pub fn set_active_object_length_fn(f: ObjectLengthFn) {
    ACTIVE_OBJECT_LENGTH_FN.with(|c| c.set(Some(f)));
}

pub fn clear_active_object_length_fn() {
    ACTIVE_OBJECT_LENGTH_FN.with(|c| c.set(None));
}

pub fn call_object_length(obj_id: i64) -> f64 {
    if deopt_env_truthy("CRUFT_LEJIT_OBJECT_LENGTH_HELPER_COUNT") {
        let n =
            OBJECT_LENGTH_HELPER_CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if n == 1 || n % 1_000_000 == 0 {
            eprintln!("[object-length-helper-count] calls={n}");
        }
    }
    if let Some(f) = ACTIVE_OBJECT_LENGTH_FN.with(|c| c.get()) {
        f(obj_id)
    } else {
        f64::NAN
    }
}

pub type StringLenFn = extern "C" fn(i64) -> f64;
pub type StringCharCodeAtFn = extern "C" fn(i64, i64) -> f64;
pub type StringIndexOfFn = extern "C" fn(i64, i64, i64) -> f64;
pub type StringFromCharCodeFn = extern "C" fn(f64) -> f64;
pub type StringCtorFn = extern "C" fn(f64) -> f64;
pub type StringLiteralFn = extern "C" fn(i64, i64) -> f64;
pub type StringLocalConstStrictEqFn = extern "C" fn(f64, i64, i64) -> f64;
pub type OwnedStringResultLenFn = extern "C" fn(f64) -> f64;
pub type OwnedStringResultCharCodeAtFn = extern "C" fn(f64, i64) -> f64;
pub type Site874TokenFinalizationStringConsumersFn = extern "C" fn(f64) -> i64;
pub type StringConcatOwnedResultFn = extern "C" fn(f64, f64) -> f64;

pub type StringConcatOwnedValueFn = extern "C" fn(f64, f64) -> f64;

pub type GetIndexStringFn = extern "C" fn(i64, i64) -> f64;

pub type RegexpExecIcFn = extern "C" fn(i64, f64) -> i64;

pub type RegexpExecGlobalObjectOrNullIcFn = extern "C" fn(i64, f64) -> i64;
pub const REGEXP_EXEC_DEOPT_SENTINEL: i64 = -1;
pub const REGEXP_EXEC_NORMAL_NULL_SENTINEL: i64 = -2;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegexpExecIcResultKind {
    ObjectId,
    NormalNull,
    Deopt,
}
pub fn regexp_exec_ic_result_kind(result: i64) -> RegexpExecIcResultKind {
    match result {
        REGEXP_EXEC_DEOPT_SENTINEL => RegexpExecIcResultKind::Deopt,
        REGEXP_EXEC_NORMAL_NULL_SENTINEL => RegexpExecIcResultKind::NormalNull,
        _ if result >= 0 => RegexpExecIcResultKind::ObjectId,
        _ => RegexpExecIcResultKind::Deopt,
    }
}

pub type ToNumberIndexFn = extern "C" fn(i64, i64) -> f64;
pub type StringCommaIfTruthyFn = extern "C" fn(f64) -> f64;
pub type ArrayJoinOwnedResultFn = extern "C" fn(i64, f64) -> f64;
pub type StringConcatOwnedValuePreserveRightFn = extern "C" fn(f64, f64) -> f64;
pub type StringDirectKindFn = extern "C" fn(i64) -> i64;
pub type StringDirectDataPtrFn = extern "C" fn(i64) -> i64;
pub type StringDirectLenFn = extern "C" fn(i64) -> i64;
pub type CssSite813HashUnitTraceFn = extern "C" fn(i64, i64, i64);
pub type CssSite813HashStoreTraceFn = extern "C" fn(i64, i64, i64);
pub type CssSite813CharcodeGateTraceFn = extern "C" fn(i64, i64, i64, i64, i64);
pub type PrimitiveStringIndexOwnedFn = extern "C" fn(i64, i64) -> f64;
pub type CwcStringPropLenFn = extern "C" fn(i64, i64) -> f64;
pub type CwcBooleanPropAsNumberFn = extern "C" fn(i64, i64) -> f64;
pub type ArrayPush1Fn = extern "C" fn(i64, f64) -> f64;
pub type ArrayPush1ObjFn = extern "C" fn(i64, i64) -> f64;
pub type ArrayPopFn = extern "C" fn(i64) -> f64;
pub type ArrayIsArrayFn = extern "C" fn(i64) -> f64;
pub type TypedArrayDataPtrFn = extern "C" fn(i64) -> i64;
pub type TypedArrayLenFn = extern "C" fn(i64) -> i64;
pub type TypedArrayKindFn = extern "C" fn(i64) -> i64;

thread_local! {
    static ACTIVE_STRING_LEN_FN: std::cell::Cell<Option<StringLenFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_STRING_CCA_FN: std::cell::Cell<Option<StringCharCodeAtFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_STRING_INDEXOF_FN: std::cell::Cell<Option<StringIndexOfFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_STRING_FROM_CHAR_CODE_FN: std::cell::Cell<Option<StringFromCharCodeFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_STRING_CTOR_FN: std::cell::Cell<Option<StringCtorFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_STRING_LITERAL_FN: std::cell::Cell<Option<StringLiteralFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_STRING_LOCAL_CONST_STRICT_EQ_FN: std::cell::Cell<Option<StringLocalConstStrictEqFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_OWNED_STRING_RESULT_LEN_FN: std::cell::Cell<Option<OwnedStringResultLenFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_OWNED_STRING_RESULT_CCA_FN: std::cell::Cell<Option<OwnedStringResultCharCodeAtFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_SITE874_TOKEN_FINALIZATION_STRING_CONSUMERS_FN: std::cell::Cell<Option<Site874TokenFinalizationStringConsumersFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_STRING_CONCAT_OWNED_RESULT_FN: std::cell::Cell<Option<StringConcatOwnedResultFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_STRING_CONCAT_OWNED_VALUE_FN: std::cell::Cell<Option<StringConcatOwnedValueFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_STRING_CONCAT_OWNED_VALUE_PRESERVE_RIGHT_FN: std::cell::Cell<Option<StringConcatOwnedValuePreserveRightFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_GETINDEX_STRING_FN: std::cell::Cell<Option<GetIndexStringFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_REGEXP_EXEC_IC_FN: std::cell::Cell<Option<RegexpExecIcFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_REGEXP_EXEC_GLOBAL_OBJECT_OR_NULL_IC_FN: std::cell::Cell<Option<RegexpExecGlobalObjectOrNullIcFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_TO_NUMBER_INDEX_FN: std::cell::Cell<Option<ToNumberIndexFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_STRING_COMMA_IF_TRUTHY_FN: std::cell::Cell<Option<StringCommaIfTruthyFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_ARRAY_JOIN_OWNED_RESULT_FN: std::cell::Cell<Option<ArrayJoinOwnedResultFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_STRING_DIRECT_KIND_FN: std::cell::Cell<Option<StringDirectKindFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_STRING_DIRECT_DATA_PTR_FN: std::cell::Cell<Option<StringDirectDataPtrFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_STRING_DIRECT_LEN_FN: std::cell::Cell<Option<StringDirectLenFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_PRIMITIVE_STRING_INDEX_OWNED_FN: std::cell::Cell<Option<PrimitiveStringIndexOwnedFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_CWC_STRING_PROP_LEN_FN: std::cell::Cell<Option<CwcStringPropLenFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_CWC_BOOLEAN_PROP_AS_NUMBER_FN: std::cell::Cell<Option<CwcBooleanPropAsNumberFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_ARRAY_PUSH1_FN: std::cell::Cell<Option<ArrayPush1Fn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_ARRAY_PUSH1_OBJ_FN: std::cell::Cell<Option<ArrayPush1ObjFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_ARRAY_POP_FN: std::cell::Cell<Option<ArrayPopFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_ARRAY_IS_ARRAY_FN: std::cell::Cell<Option<ArrayIsArrayFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_TYPED_ARRAY_DATA_PTR_FN: std::cell::Cell<Option<TypedArrayDataPtrFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_TYPED_ARRAY_LEN_FN: std::cell::Cell<Option<TypedArrayLenFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_TYPED_ARRAY_KIND_FN: std::cell::Cell<Option<TypedArrayKindFn>> =
        const { std::cell::Cell::new(None) };
}

pub fn set_active_string_len_fn(f: StringLenFn) {
    ACTIVE_STRING_LEN_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_string_char_code_at_fn(f: StringCharCodeAtFn) {
    ACTIVE_STRING_CCA_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_string_index_of_fn(f: StringIndexOfFn) {
    ACTIVE_STRING_INDEXOF_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_string_from_char_code_fn(f: StringFromCharCodeFn) {
    ACTIVE_STRING_FROM_CHAR_CODE_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_string_ctor_fn(f: StringCtorFn) {
    ACTIVE_STRING_CTOR_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_string_literal_fn(f: StringLiteralFn) {
    ACTIVE_STRING_LITERAL_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_string_local_const_strict_eq_fn(f: StringLocalConstStrictEqFn) {
    ACTIVE_STRING_LOCAL_CONST_STRICT_EQ_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_owned_string_result_len_fn(f: OwnedStringResultLenFn) {
    ACTIVE_OWNED_STRING_RESULT_LEN_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_owned_string_result_char_code_at_fn(f: OwnedStringResultCharCodeAtFn) {
    ACTIVE_OWNED_STRING_RESULT_CCA_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_site874_token_finalization_string_consumers_fn(
    f: Site874TokenFinalizationStringConsumersFn,
) {
    ACTIVE_SITE874_TOKEN_FINALIZATION_STRING_CONSUMERS_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_string_concat_owned_result_fn(f: StringConcatOwnedResultFn) {
    ACTIVE_STRING_CONCAT_OWNED_RESULT_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_string_concat_owned_value_fn(f: StringConcatOwnedValueFn) {
    ACTIVE_STRING_CONCAT_OWNED_VALUE_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_string_concat_owned_value_preserve_right_fn(
    f: StringConcatOwnedValuePreserveRightFn,
) {
    ACTIVE_STRING_CONCAT_OWNED_VALUE_PRESERVE_RIGHT_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_getindex_string_fn(f: GetIndexStringFn) {
    ACTIVE_GETINDEX_STRING_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_primitive_string_index_owned_fn(f: PrimitiveStringIndexOwnedFn) {
    ACTIVE_PRIMITIVE_STRING_INDEX_OWNED_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_regexp_exec_ic_fn(f: RegexpExecIcFn) {
    ACTIVE_REGEXP_EXEC_IC_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_regexp_exec_global_object_or_null_ic_fn(f: RegexpExecGlobalObjectOrNullIcFn) {
    ACTIVE_REGEXP_EXEC_GLOBAL_OBJECT_OR_NULL_IC_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_to_number_index_fn(f: ToNumberIndexFn) {
    ACTIVE_TO_NUMBER_INDEX_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_string_comma_if_truthy_fn(f: StringCommaIfTruthyFn) {
    ACTIVE_STRING_COMMA_IF_TRUTHY_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_array_join_owned_result_fn(f: ArrayJoinOwnedResultFn) {
    ACTIVE_ARRAY_JOIN_OWNED_RESULT_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_string_direct_kind_fn(f: StringDirectKindFn) {
    ACTIVE_STRING_DIRECT_KIND_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_string_direct_data_ptr_fn(f: StringDirectDataPtrFn) {
    ACTIVE_STRING_DIRECT_DATA_PTR_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_string_direct_len_fn(f: StringDirectLenFn) {
    ACTIVE_STRING_DIRECT_LEN_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_cwc_string_prop_len_fn(f: CwcStringPropLenFn) {
    ACTIVE_CWC_STRING_PROP_LEN_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_cwc_boolean_prop_as_number_fn(f: CwcBooleanPropAsNumberFn) {
    ACTIVE_CWC_BOOLEAN_PROP_AS_NUMBER_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_array_push1_fn(f: ArrayPush1Fn) {
    ACTIVE_ARRAY_PUSH1_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_array_push1_obj_fn(f: ArrayPush1ObjFn) {
    ACTIVE_ARRAY_PUSH1_OBJ_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_array_pop_fn(f: ArrayPopFn) {
    ACTIVE_ARRAY_POP_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_array_is_array_fn(f: ArrayIsArrayFn) {
    ACTIVE_ARRAY_IS_ARRAY_FN.with(|c| c.set(Some(f)));
}
pub fn set_active_typed_array_data_ptr_fn(f: TypedArrayDataPtrFn) {
    ACTIVE_TYPED_ARRAY_DATA_PTR_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_typed_array_data_ptr_fn() {
    ACTIVE_TYPED_ARRAY_DATA_PTR_FN.with(|c| c.set(None));
}
pub fn set_active_typed_array_len_fn(f: TypedArrayLenFn) {
    ACTIVE_TYPED_ARRAY_LEN_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_typed_array_len_fn() {
    ACTIVE_TYPED_ARRAY_LEN_FN.with(|c| c.set(None));
}
pub fn set_active_typed_array_kind_fn(f: TypedArrayKindFn) {
    ACTIVE_TYPED_ARRAY_KIND_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_typed_array_kind_fn() {
    ACTIVE_TYPED_ARRAY_KIND_FN.with(|c| c.set(None));
}

pub fn call_string_len(payload: i64) -> f64 {
    ACTIVE_STRING_LEN_FN
        .with(|c| c.get())
        .map(|f| f(payload))
        .unwrap_or(f64::NAN)
}

#[no_mangle]
pub extern "C" fn jit_cwc_string_prop_len(receiver_idx: i64, prop_name_idx: i64) -> f64 {
    ACTIVE_CWC_STRING_PROP_LEN_FN
        .with(|c| c.get())
        .map(|f| f(receiver_idx, prop_name_idx))
        .unwrap_or(f64::NAN)
}

#[no_mangle]
pub extern "C" fn jit_cwc_boolean_prop_as_number(receiver_idx: i64, prop_name_idx: i64) -> f64 {
    ACTIVE_CWC_BOOLEAN_PROP_AS_NUMBER_FN
        .with(|c| c.get())
        .map(|f| f(receiver_idx, prop_name_idx))
        .unwrap_or(0.0)
}
pub fn call_string_char_code_at(payload: i64, i: i64) -> f64 {
    ACTIVE_STRING_CCA_FN
        .with(|c| c.get())
        .map(|f| f(payload, i))
        .unwrap_or(f64::NAN)
}
pub fn call_string_index_of(haystack: i64, needle: i64, from: i64) -> f64 {
    ACTIVE_STRING_INDEXOF_FN
        .with(|c| c.get())
        .map(|f| f(haystack, needle, from))
        .unwrap_or(f64::NAN)
}
pub fn call_string_from_char_code(code_unit: f64) -> f64 {
    ACTIVE_STRING_FROM_CHAR_CODE_FN
        .with(|c| c.get())
        .map(|f| f(code_unit))
        .unwrap_or(f64::NAN)
}
pub fn call_string_ctor(value: f64) -> f64 {
    ACTIVE_STRING_CTOR_FN
        .with(|c| c.get())
        .map(|f| f(value))
        .unwrap_or(f64::NAN)
}
pub fn call_string_literal(ptr: i64, len: i64) -> f64 {
    ACTIVE_STRING_LITERAL_FN
        .with(|c| c.get())
        .map(|f| f(ptr, len))
        .unwrap_or(f64::NAN)
}
pub fn call_string_local_const_strict_eq(value: f64, ptr: i64, len: i64) -> f64 {
    ACTIVE_STRING_LOCAL_CONST_STRICT_EQ_FN
        .with(|c| c.get())
        .map(|f| f(value, ptr, len))
        .unwrap_or(0.0)
}
pub fn call_owned_string_result_len(handle: f64) -> f64 {
    ACTIVE_OWNED_STRING_RESULT_LEN_FN
        .with(|c| c.get())
        .map(|f| f(handle))
        .unwrap_or(f64::NAN)
}
pub fn call_owned_string_result_char_code_at(handle: f64, i: i64) -> f64 {
    ACTIVE_OWNED_STRING_RESULT_CCA_FN
        .with(|c| c.get())
        .map(|f| f(handle, i))
        .unwrap_or(f64::NAN)
}
pub fn call_site874_token_finalization_string_consumers(token: f64) -> i64 {
    ACTIVE_SITE874_TOKEN_FINALIZATION_STRING_CONSUMERS_FN
        .with(|c| c.get())
        .map(|f| f(token))
        .unwrap_or(-1)
}
pub fn call_string_concat_owned_result(left: f64, right: f64) -> f64 {
    ACTIVE_STRING_CONCAT_OWNED_RESULT_FN
        .with(|c| c.get())
        .map(|f| f(left, right))
        .unwrap_or(f64::NAN)
}
pub fn call_string_concat_owned_value(left: f64, right: f64) -> f64 {
    ACTIVE_STRING_CONCAT_OWNED_VALUE_FN
        .with(|c| c.get())
        .map(|f| f(left, right))
        .unwrap_or(f64::NAN)
}
pub fn call_string_concat_owned_value_preserve_right(left: f64, right: f64) -> f64 {
    ACTIVE_STRING_CONCAT_OWNED_VALUE_PRESERVE_RIGHT_FN
        .with(|c| c.get())
        .map(|f| f(left, right))
        .unwrap_or(f64::NAN)
}
pub fn call_getindex_string_on_object(receiver_idx: i64, index_i64: i64) -> f64 {
    ACTIVE_GETINDEX_STRING_FN
        .with(|c| c.get())
        .map(|f| f(receiver_idx, index_i64))
        .unwrap_or(f64::NAN)
}
pub fn call_primitive_string_index_owned(payload: i64, index_i64: i64) -> f64 {
    ACTIVE_PRIMITIVE_STRING_INDEX_OWNED_FN
        .with(|c| c.get())
        .map(|f| f(payload, index_i64))
        .unwrap_or(f64::NAN)
}
pub fn call_regexp_exec_ic(receiver_id: i64, arg: f64) -> i64 {
    ACTIVE_REGEXP_EXEC_IC_FN
        .with(|c| c.get())
        .map(|f| f(receiver_id, arg))
        .unwrap_or(REGEXP_EXEC_DEOPT_SENTINEL)
}
pub fn call_regexp_exec_global_object_or_null_ic(receiver_id: i64, arg: f64) -> i64 {
    ACTIVE_REGEXP_EXEC_GLOBAL_OBJECT_OR_NULL_IC_FN
        .with(|c| c.get())
        .map(|f| f(receiver_id, arg))
        .unwrap_or(REGEXP_EXEC_DEOPT_SENTINEL)
}
pub fn call_to_number_index(obj_id: i64, index: i64) -> f64 {
    ACTIVE_TO_NUMBER_INDEX_FN
        .with(|c| c.get())
        .map(|f| f(obj_id, index))
        .unwrap_or(f64::NAN)
}
pub fn call_string_comma_if_truthy(value: f64) -> f64 {
    ACTIVE_STRING_COMMA_IF_TRUTHY_FN
        .with(|c| c.get())
        .map(|f| f(value))
        .unwrap_or(f64::NAN)
}
pub fn call_array_join_owned_result(receiver_idx: i64, sep: f64) -> f64 {
    ACTIVE_ARRAY_JOIN_OWNED_RESULT_FN
        .with(|c| c.get())
        .map(|f| f(receiver_idx, sep))
        .unwrap_or(f64::NAN)
}
pub fn call_string_direct_kind(payload: i64) -> i64 {
    ACTIVE_STRING_DIRECT_KIND_FN
        .with(|c| c.get())
        .map(|f| f(payload))
        .unwrap_or(0)
}
pub fn call_string_direct_data_ptr(payload: i64) -> i64 {
    ACTIVE_STRING_DIRECT_DATA_PTR_FN
        .with(|c| c.get())
        .map(|f| f(payload))
        .unwrap_or(0)
}
pub fn call_string_direct_len(payload: i64) -> i64 {
    ACTIVE_STRING_DIRECT_LEN_FN
        .with(|c| c.get())
        .map(|f| f(payload))
        .unwrap_or(0)
}
pub fn call_array_push1(receiver_payload: i64, value: f64) -> f64 {
    ACTIVE_ARRAY_PUSH1_FN
        .with(|c| c.get())
        .map(|f| f(receiver_payload, value))
        .unwrap_or(f64::NAN)
}
pub fn call_array_push1_obj(receiver_payload: i64, value_payload: i64) -> f64 {
    ACTIVE_ARRAY_PUSH1_OBJ_FN
        .with(|c| c.get())
        .map(|f| f(receiver_payload, value_payload))
        .unwrap_or(f64::NAN)
}
pub fn call_array_pop(receiver_payload: i64) -> f64 {
    ACTIVE_ARRAY_POP_FN
        .with(|c| c.get())
        .map(|f| f(receiver_payload))
        .unwrap_or(f64::NAN)
}
pub fn call_array_is_array(receiver_payload: i64) -> f64 {
    ACTIVE_ARRAY_IS_ARRAY_FN
        .with(|c| c.get())
        .map(|f| f(receiver_payload))
        .unwrap_or(f64::NAN)
}

#[no_mangle]
pub extern "C" fn jit_array_push1_obj(receiver_payload: i64, value_payload: i64) -> f64 {
    call_array_push1_obj(receiver_payload, value_payload)
}

#[no_mangle]
pub extern "C" fn jit_array_is_array(receiver_payload: i64) -> f64 {
    call_array_is_array(receiver_payload)
}

#[no_mangle]
pub extern "C" fn jit_typed_array_data_ptr(receiver_payload: i64) -> i64 {
    ACTIVE_TYPED_ARRAY_DATA_PTR_FN
        .with(|c| c.get())
        .map(|f| f(receiver_payload))
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn jit_typed_array_len(receiver_payload: i64) -> i64 {
    ACTIVE_TYPED_ARRAY_LEN_FN
        .with(|c| c.get())
        .map(|f| f(receiver_payload))
        .unwrap_or(-1)
}

#[no_mangle]
pub extern "C" fn jit_typed_array_kind(receiver_payload: i64) -> i64 {
    ACTIVE_TYPED_ARRAY_KIND_FN
        .with(|c| c.get())
        .map(|f| f(receiver_payload))
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn jit_buffer_backing_epoch(buffer_payload: i64) -> i64 {
    ACTIVE_BUFFER_BACKING_EPOCH_FN
        .with(|c| c.get())
        .map(|f| f(buffer_payload))
        .unwrap_or(-1)
}

#[no_mangle]
pub extern "C" fn jit_string_direct_kind(payload: i64) -> i64 {
    ACTIVE_STRING_DIRECT_KIND_FN
        .with(|c| c.get())
        .map(|f| f(payload))
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn jit_string_direct_data_ptr(payload: i64) -> i64 {
    ACTIVE_STRING_DIRECT_DATA_PTR_FN
        .with(|c| c.get())
        .map(|f| f(payload))
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn jit_string_direct_len(payload: i64) -> i64 {
    ACTIVE_STRING_DIRECT_LEN_FN
        .with(|c| c.get())
        .map(|f| f(payload))
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn jit_css_site813_hash_unit_trace(pc: i64, index: i64, unit: i64) {
    eprintln!("[css-site813-generated-hash-unit] pc={pc} index={index} unit={unit}");
}

#[no_mangle]
pub extern "C" fn jit_css_site813_hash_store_trace(pc: i64, slot: i64, hash: i64) {
    eprintln!("[css-site813-generated-hash-store] pc={pc} slot={slot} hash={hash}");
}

#[no_mangle]
pub extern "C" fn jit_css_site813_charcode_gate_trace(
    pc: i64,
    index: i64,
    payload: i64,
    len: i64,
    outcome: i64,
) {
    eprintln!(
        "[css-site813-charcode-gate] pc={pc} index={index} payload={payload} len={len} outcome={outcome}"
    );
}

#[no_mangle]
pub extern "C" fn jit_string_from_char_code(code_unit: f64) -> f64 {
    call_string_from_char_code(code_unit)
}

#[no_mangle]
pub extern "C" fn jit_string_ctor(value: f64) -> f64 {
    call_string_ctor(value)
}

#[no_mangle]
pub extern "C" fn jit_string_literal(ptr: i64, len: i64) -> f64 {
    call_string_literal(ptr, len)
}

#[no_mangle]
pub extern "C" fn jit_string_local_const_strict_eq(value: f64, ptr: i64, len: i64) -> f64 {
    call_string_local_const_strict_eq(value, ptr, len)
}

#[no_mangle]
pub extern "C" fn jit_owned_string_result_len(handle: f64) -> f64 {
    call_owned_string_result_len(handle)
}

#[no_mangle]
pub extern "C" fn jit_owned_string_result_char_code_at(handle: f64, i: i64) -> f64 {
    call_owned_string_result_char_code_at(handle, i)
}

#[no_mangle]
pub extern "C" fn jit_site874_token_finalization_string_consumers(token: f64) -> i64 {
    call_site874_token_finalization_string_consumers(token)
}

#[no_mangle]
pub extern "C" fn jit_string_concat_owned_result(left: f64, right: f64) -> f64 {
    call_string_concat_owned_result(left, right)
}

#[no_mangle]
pub extern "C" fn jit_string_concat_owned_value(left: f64, right: f64) -> f64 {
    call_string_concat_owned_value(left, right)
}

#[no_mangle]
pub extern "C" fn jit_string_concat_owned_value_preserve_right(left: f64, right: f64) -> f64 {
    call_string_concat_owned_value_preserve_right(left, right)
}

#[no_mangle]
pub extern "C" fn jit_getindex_string_on_object(receiver_idx: i64, index_i64: i64) -> f64 {
    call_getindex_string_on_object(receiver_idx, index_i64)
}

#[no_mangle]
pub extern "C" fn jit_primitive_string_index_owned(payload: i64, index_i64: i64) -> f64 {
    call_primitive_string_index_owned(payload, index_i64)
}

#[no_mangle]
pub extern "C" fn jit_regexp_exec_ic(receiver_id: i64, arg: f64) -> i64 {
    call_regexp_exec_ic(receiver_id, arg)
}

#[no_mangle]
pub extern "C" fn jit_regexp_exec_global_object_or_null_ic(receiver_id: i64, arg: f64) -> i64 {
    call_regexp_exec_global_object_or_null_ic(receiver_id, arg)
}

#[no_mangle]
pub extern "C" fn jit_to_number_index(obj_id: i64, index: i64) -> f64 {
    call_to_number_index(obj_id, index)
}

#[no_mangle]
pub extern "C" fn jit_string_comma_if_truthy(value: f64) -> f64 {
    call_string_comma_if_truthy(value)
}

#[no_mangle]
pub extern "C" fn jit_array_join_owned_result(receiver_idx: i64, sep: f64) -> f64 {
    call_array_join_owned_result(receiver_idx, sep)
}

pub type SetPropFn = extern "C" fn(i64, i64, f64);
pub type SetPropFreshDataAddFn = extern "C" fn(i64, i64, f64);
pub type MworSetPropFn = extern "C" fn(i64, i64, f64);

pub type SetIndexFn = extern "C" fn(i64, i64, f64);

pub type SetGlobalVarFn = extern "C" fn(i64, f64);

pub type CallDirect1Fn = extern "C" fn(i64, f64) -> f64;

pub type CallIndexedDirect1Fn = extern "C" fn(i64, i64, f64) -> f64;
pub type CallDirect1VoidFn = extern "C" fn(i64, f64) -> f64;

pub type CallDirect0Fn = extern "C" fn(i64) -> f64;

pub type CallDirect0ObjRetFn = extern "C" fn(i64) -> i64;

pub type CallDirect1ObjRetFn = extern "C" fn(i64, f64) -> i64;

pub type CallDirect1ObjRetNonNullGuardFn = extern "C" fn(i64, f64) -> i64;

pub type HostMethod1ObjRetFn = extern "C" fn(i64, i64, f64) -> i64;

pub type HostMethod2ObjRetFn = extern "C" fn(i64, i64, f64, f64) -> i64;

pub type HostMethod0NumRetFn = extern "C" fn(i64) -> f64;

pub type HostMethod1NumRetFn = extern "C" fn(i64, i64, f64) -> f64;

pub type HostMethod1StringFn = extern "C" fn(i64, i64, f64) -> f64;

pub type HostMethod2StringFn = extern "C" fn(i64, i64, f64, f64) -> f64;

pub type HostMethod4StringFn = extern "C" fn(i64, i64, f64, f64, f64, f64) -> f64;

pub type HostGlobalObjectFn = extern "C" fn(i64) -> i64;

pub type HostObjectPropStringFn = extern "C" fn(i64, i64) -> f64;

pub type HostObjectPropNumRetFn = extern "C" fn(i64, i64) -> f64;

pub type HostObjectMethod0StringFn = extern "C" fn(i64, i64) -> f64;

pub type HostObjectMethod1StringFn = extern "C" fn(i64, i64, f64) -> f64;

pub type BufferWriteU32BeFn = extern "C" fn(i64, f64, f64) -> f64;

pub type BufferWriteU32BeU32OffsetFn = extern "C" fn(i64, i64, i64) -> f64;

pub type BufferWriteU32LeU32OffsetFn = extern "C" fn(i64, i64, i64) -> f64;

pub type BufferWriteU8U32OffsetFn = extern "C" fn(i64, i64, i64) -> f64;

pub type BufferWriteU16BeU32OffsetFn = extern "C" fn(i64, i64, i64) -> f64;

pub type BufferWriteU16LeU32OffsetFn = extern "C" fn(i64, i64, i64) -> f64;

pub type BufferBackingEpochFn = extern "C" fn(i64) -> i64;

pub type BufferReadU32BeFn = extern "C" fn(i64, f64) -> f64;

pub type BufferReadIntegerOffsetFn = extern "C" fn(i64, i64) -> f64;

pub type HostConstruct1ObjRetFn = extern "C" fn(i64, f64) -> i64;

pub type CallDirect1ObjFn = extern "C" fn(i64, i64) -> f64;

pub type CallDirect1ObjObjRetFn = extern "C" fn(i64, i64) -> i64;

pub type CallDirect2ObjNumFn = extern "C" fn(i64, i64, f64) -> f64;

pub type CallDirect2ObjNumVoidFn = extern "C" fn(i64, i64, f64) -> i64;

pub type CallDirect2ObjNumObjRetFn = extern "C" fn(i64, i64, f64) -> i64;

pub type CallDirect2NumNumFn = extern "C" fn(i64, f64, f64) -> f64;

pub type CallDirect2ObjObjPredicateFn = extern "C" fn(i64, i64, i64) -> i64;

pub type CallCapturedAddArgStoreFn = extern "C" fn(i64, f64) -> f64;

pub type CallDirect0ValueStoreDeadFn = extern "C" fn(i64, i64) -> i64;

pub type CallDirect4ValueStoreDeadFn = extern "C" fn(i64, i64, i64, i64, i64, i64) -> i64;
pub type CallDirect4ValueStoreDeadLanesFn =
    extern "C" fn(i64, i64, i64, f64, i64, f64, i64, f64, i64, f64) -> i64;

thread_local! {
    static ACTIVE_SETPROP_FN: std::cell::Cell<Option<SetPropFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_SETPROP_FRESH_DATA_ADD_FN: std::cell::Cell<Option<SetPropFreshDataAddFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_MWOR_SETPROP_FN: std::cell::Cell<Option<MworSetPropFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_SETINDEX_FN: std::cell::Cell<Option<SetIndexFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_SETGLOBAL_VAR_FN: std::cell::Cell<Option<SetGlobalVarFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_CALL_DIRECT1_FN: std::cell::Cell<Option<CallDirect1Fn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_CALL_INDEXED_DIRECT1_FN: std::cell::Cell<Option<CallIndexedDirect1Fn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_CALL_DIRECT1_VOID_FN: std::cell::Cell<Option<CallDirect1VoidFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_CALL_DIRECT0_FN: std::cell::Cell<Option<CallDirect0Fn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_CALL_DIRECT0_OBJRET_FN: std::cell::Cell<Option<CallDirect0ObjRetFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_CALL_DIRECT1_OBJRET_FN: std::cell::Cell<Option<CallDirect1ObjRetFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_CALL_DIRECT1_OBJRET_NONNULL_GUARD_FN: std::cell::Cell<Option<CallDirect1ObjRetNonNullGuardFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_HOST_METHOD1_OBJRET_FN: std::cell::Cell<Option<HostMethod1ObjRetFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_HOST_METHOD2_OBJRET_FN: std::cell::Cell<Option<HostMethod2ObjRetFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_HOST_METHOD0_NUMRET_FN: std::cell::Cell<Option<HostMethod0NumRetFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_HOST_METHOD1_NUMRET_FN: std::cell::Cell<Option<HostMethod1NumRetFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_HOST_METHOD1_STRING_FN: std::cell::Cell<Option<HostMethod1StringFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_HOST_METHOD2_STRING_FN: std::cell::Cell<Option<HostMethod2StringFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_HOST_METHOD4_STRING_FN: std::cell::Cell<Option<HostMethod4StringFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_HOST_GLOBAL_OBJECT_FN: std::cell::Cell<Option<HostGlobalObjectFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_HOST_OBJECT_PROP_STRING_FN: std::cell::Cell<Option<HostObjectPropStringFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_HOST_OBJECT_PROP_NUMRET_FN: std::cell::Cell<Option<HostObjectPropNumRetFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_HOST_OBJECT_METHOD0_STRING_FN: std::cell::Cell<Option<HostObjectMethod0StringFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_HOST_OBJECT_METHOD1_STRING_FN: std::cell::Cell<Option<HostObjectMethod1StringFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_BUFFER_WRITE_U32BE_FN: std::cell::Cell<Option<BufferWriteU32BeFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_BUFFER_WRITE_U32BE_U32_OFFSET_FN: std::cell::Cell<Option<BufferWriteU32BeU32OffsetFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_BUFFER_WRITE_U32LE_U32_OFFSET_FN: std::cell::Cell<Option<BufferWriteU32LeU32OffsetFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_BUFFER_WRITE_U8_U32_OFFSET_FN: std::cell::Cell<Option<BufferWriteU8U32OffsetFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_BUFFER_WRITE_U16BE_U32_OFFSET_FN: std::cell::Cell<Option<BufferWriteU16BeU32OffsetFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_BUFFER_WRITE_U16LE_U32_OFFSET_FN: std::cell::Cell<Option<BufferWriteU16LeU32OffsetFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_BUFFER_BACKING_EPOCH_FN: std::cell::Cell<Option<BufferBackingEpochFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_BUFFER_READ_U32BE_FN: std::cell::Cell<Option<BufferReadU32BeFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_BUFFER_READ_U8_INTEGER_OFFSET_FN: std::cell::Cell<Option<BufferReadIntegerOffsetFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_BUFFER_READ_U16BE_INTEGER_OFFSET_FN: std::cell::Cell<Option<BufferReadIntegerOffsetFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_BUFFER_READ_U16LE_INTEGER_OFFSET_FN: std::cell::Cell<Option<BufferReadIntegerOffsetFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_BUFFER_READ_U32BE_INTEGER_OFFSET_FN: std::cell::Cell<Option<BufferReadIntegerOffsetFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_BUFFER_READ_U32LE_INTEGER_OFFSET_FN: std::cell::Cell<Option<BufferReadIntegerOffsetFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_BUFFER_READ_I32BE_INTEGER_OFFSET_FN: std::cell::Cell<Option<BufferReadIntegerOffsetFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_HOST_CONSTRUCT1_OBJRET_FN: std::cell::Cell<Option<HostConstruct1ObjRetFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_CALL_DIRECT1_OBJ_FN: std::cell::Cell<Option<CallDirect1ObjFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_CALL_DIRECT1_OBJ_OBJRET_FN: std::cell::Cell<Option<CallDirect1ObjObjRetFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_CALL_DIRECT2_OBJ_NUM_FN: std::cell::Cell<Option<CallDirect2ObjNumFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_CALL_DIRECT2_OBJ_NUM_VOID_FN: std::cell::Cell<Option<CallDirect2ObjNumVoidFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_CALL_DIRECT2_OBJ_NUM_OBJRET_FN: std::cell::Cell<Option<CallDirect2ObjNumObjRetFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_CALL_DIRECT2_NUM_NUM_FN: std::cell::Cell<Option<CallDirect2NumNumFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_CALL_DIRECT2_OBJ_OBJ_PREDICATE_FN: std::cell::Cell<Option<CallDirect2ObjObjPredicateFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_CALL_CAPTURED_ADD_ARG_STORE_FN: std::cell::Cell<Option<CallCapturedAddArgStoreFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_CALL_DIRECT0_VALUE_STORE_DEAD_FN: std::cell::Cell<Option<CallDirect0ValueStoreDeadFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_CALL_DIRECT4_VALUE_STORE_DEAD_FN: std::cell::Cell<Option<CallDirect4ValueStoreDeadFn>> =
        const { std::cell::Cell::new(None) };
    static ACTIVE_CALL_DIRECT4_VALUE_STORE_DEAD_LANES_FN: std::cell::Cell<Option<CallDirect4ValueStoreDeadLanesFn>> =
        const { std::cell::Cell::new(None) };
    static CURRENT_VALUE_WRITEBACK_CELLS: std::cell::RefCell<Vec<(i64, i64)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

pub fn set_active_setprop_fn(f: SetPropFn) {
    ACTIVE_SETPROP_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_setprop_fn() {
    ACTIVE_SETPROP_FN.with(|c| c.set(None));
}
pub fn set_active_setprop_fresh_data_add_fn(f: SetPropFreshDataAddFn) {
    ACTIVE_SETPROP_FRESH_DATA_ADD_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_setprop_fresh_data_add_fn() {
    ACTIVE_SETPROP_FRESH_DATA_ADD_FN.with(|c| c.set(None));
}
pub fn set_active_mwor_setprop_fn(f: MworSetPropFn) {
    ACTIVE_MWOR_SETPROP_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_mwor_setprop_fn() {
    ACTIVE_MWOR_SETPROP_FN.with(|c| c.set(None));
}
pub fn set_active_setindex_fn(f: SetIndexFn) {
    ACTIVE_SETINDEX_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_setindex_fn() {
    ACTIVE_SETINDEX_FN.with(|c| c.set(None));
}
pub fn set_active_setglobal_var_fn(f: SetGlobalVarFn) {
    ACTIVE_SETGLOBAL_VAR_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_setglobal_var_fn() {
    ACTIVE_SETGLOBAL_VAR_FN.with(|c| c.set(None));
}
pub fn set_active_call_direct1_fn(f: CallDirect1Fn) {
    ACTIVE_CALL_DIRECT1_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_call_direct1_fn() {
    ACTIVE_CALL_DIRECT1_FN.with(|c| c.set(None));
}
pub fn set_active_call_indexed_direct1_fn(f: CallIndexedDirect1Fn) {
    ACTIVE_CALL_INDEXED_DIRECT1_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_call_indexed_direct1_fn() {
    ACTIVE_CALL_INDEXED_DIRECT1_FN.with(|c| c.set(None));
}
pub fn set_active_call_direct1_void_fn(f: CallDirect1VoidFn) {
    ACTIVE_CALL_DIRECT1_VOID_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_call_direct1_void_fn() {
    ACTIVE_CALL_DIRECT1_VOID_FN.with(|c| c.set(None));
}
pub fn set_active_call_direct0_fn(f: CallDirect0Fn) {
    ACTIVE_CALL_DIRECT0_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_call_direct0_fn() {
    ACTIVE_CALL_DIRECT0_FN.with(|c| c.set(None));
}
pub fn set_active_call_direct0_objret_fn(f: CallDirect0ObjRetFn) {
    ACTIVE_CALL_DIRECT0_OBJRET_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_call_direct0_objret_fn() {
    ACTIVE_CALL_DIRECT0_OBJRET_FN.with(|c| c.set(None));
}
pub fn set_active_call_direct1_objret_fn(f: CallDirect1ObjRetFn) {
    ACTIVE_CALL_DIRECT1_OBJRET_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_call_direct1_objret_fn() {
    ACTIVE_CALL_DIRECT1_OBJRET_FN.with(|c| c.set(None));
}
pub fn set_active_call_direct1_objret_nonnull_guard_fn(f: CallDirect1ObjRetNonNullGuardFn) {
    ACTIVE_CALL_DIRECT1_OBJRET_NONNULL_GUARD_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_call_direct1_objret_nonnull_guard_fn() {
    ACTIVE_CALL_DIRECT1_OBJRET_NONNULL_GUARD_FN.with(|c| c.set(None));
}
pub fn set_active_host_method1_objret_fn(f: HostMethod1ObjRetFn) {
    ACTIVE_HOST_METHOD1_OBJRET_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_host_method1_objret_fn() {
    ACTIVE_HOST_METHOD1_OBJRET_FN.with(|c| c.set(None));
}
pub fn set_active_host_method2_objret_fn(f: HostMethod2ObjRetFn) {
    ACTIVE_HOST_METHOD2_OBJRET_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_host_method2_objret_fn() {
    ACTIVE_HOST_METHOD2_OBJRET_FN.with(|c| c.set(None));
}
pub fn set_active_host_method0_numret_fn(f: HostMethod0NumRetFn) {
    ACTIVE_HOST_METHOD0_NUMRET_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_host_method0_numret_fn() {
    ACTIVE_HOST_METHOD0_NUMRET_FN.with(|c| c.set(None));
}
pub fn set_active_host_method1_numret_fn(f: HostMethod1NumRetFn) {
    ACTIVE_HOST_METHOD1_NUMRET_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_host_method1_numret_fn() {
    ACTIVE_HOST_METHOD1_NUMRET_FN.with(|c| c.set(None));
}
pub fn set_active_host_method1_string_fn(f: HostMethod1StringFn) {
    ACTIVE_HOST_METHOD1_STRING_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_host_method1_string_fn() {
    ACTIVE_HOST_METHOD1_STRING_FN.with(|c| c.set(None));
}
pub fn set_active_host_method2_string_fn(f: HostMethod2StringFn) {
    ACTIVE_HOST_METHOD2_STRING_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_host_method2_string_fn() {
    ACTIVE_HOST_METHOD2_STRING_FN.with(|c| c.set(None));
}
pub fn set_active_host_method4_string_fn(f: HostMethod4StringFn) {
    ACTIVE_HOST_METHOD4_STRING_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_host_method4_string_fn() {
    ACTIVE_HOST_METHOD4_STRING_FN.with(|c| c.set(None));
}
pub fn set_active_host_global_object_fn(f: HostGlobalObjectFn) {
    ACTIVE_HOST_GLOBAL_OBJECT_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_host_global_object_fn() {
    ACTIVE_HOST_GLOBAL_OBJECT_FN.with(|c| c.set(None));
}
pub fn set_active_host_object_prop_string_fn(f: HostObjectPropStringFn) {
    ACTIVE_HOST_OBJECT_PROP_STRING_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_host_object_prop_string_fn() {
    ACTIVE_HOST_OBJECT_PROP_STRING_FN.with(|c| c.set(None));
}
pub fn set_active_host_object_prop_numret_fn(f: HostObjectPropNumRetFn) {
    ACTIVE_HOST_OBJECT_PROP_NUMRET_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_host_object_prop_numret_fn() {
    ACTIVE_HOST_OBJECT_PROP_NUMRET_FN.with(|c| c.set(None));
}
pub fn set_active_host_object_method0_string_fn(f: HostObjectMethod0StringFn) {
    ACTIVE_HOST_OBJECT_METHOD0_STRING_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_host_object_method0_string_fn() {
    ACTIVE_HOST_OBJECT_METHOD0_STRING_FN.with(|c| c.set(None));
}
pub fn set_active_host_object_method1_string_fn(f: HostObjectMethod1StringFn) {
    ACTIVE_HOST_OBJECT_METHOD1_STRING_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_host_object_method1_string_fn() {
    ACTIVE_HOST_OBJECT_METHOD1_STRING_FN.with(|c| c.set(None));
}
pub fn set_active_buffer_write_u32be_fn(f: BufferWriteU32BeFn) {
    ACTIVE_BUFFER_WRITE_U32BE_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_buffer_write_u32be_fn() {
    ACTIVE_BUFFER_WRITE_U32BE_FN.with(|c| c.set(None));
}
pub fn set_active_buffer_write_u32be_u32_offset_fn(f: BufferWriteU32BeU32OffsetFn) {
    ACTIVE_BUFFER_WRITE_U32BE_U32_OFFSET_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_buffer_write_u32be_u32_offset_fn() {
    ACTIVE_BUFFER_WRITE_U32BE_U32_OFFSET_FN.with(|c| c.set(None));
}
pub fn set_active_buffer_write_u32le_u32_offset_fn(f: BufferWriteU32LeU32OffsetFn) {
    ACTIVE_BUFFER_WRITE_U32LE_U32_OFFSET_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_buffer_write_u32le_u32_offset_fn() {
    ACTIVE_BUFFER_WRITE_U32LE_U32_OFFSET_FN.with(|c| c.set(None));
}
pub fn set_active_buffer_write_u8_u32_offset_fn(f: BufferWriteU8U32OffsetFn) {
    ACTIVE_BUFFER_WRITE_U8_U32_OFFSET_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_buffer_write_u8_u32_offset_fn() {
    ACTIVE_BUFFER_WRITE_U8_U32_OFFSET_FN.with(|c| c.set(None));
}
pub fn set_active_buffer_write_u16be_u32_offset_fn(f: BufferWriteU16BeU32OffsetFn) {
    ACTIVE_BUFFER_WRITE_U16BE_U32_OFFSET_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_buffer_write_u16be_u32_offset_fn() {
    ACTIVE_BUFFER_WRITE_U16BE_U32_OFFSET_FN.with(|c| c.set(None));
}
pub fn set_active_buffer_write_u16le_u32_offset_fn(f: BufferWriteU16LeU32OffsetFn) {
    ACTIVE_BUFFER_WRITE_U16LE_U32_OFFSET_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_buffer_write_u16le_u32_offset_fn() {
    ACTIVE_BUFFER_WRITE_U16LE_U32_OFFSET_FN.with(|c| c.set(None));
}
pub fn set_active_buffer_backing_epoch_fn(f: BufferBackingEpochFn) {
    ACTIVE_BUFFER_BACKING_EPOCH_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_buffer_backing_epoch_fn() {
    ACTIVE_BUFFER_BACKING_EPOCH_FN.with(|c| c.set(None));
}
pub fn set_active_buffer_read_u32be_fn(f: BufferReadU32BeFn) {
    ACTIVE_BUFFER_READ_U32BE_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_buffer_read_u32be_fn() {
    ACTIVE_BUFFER_READ_U32BE_FN.with(|c| c.set(None));
}
pub fn set_active_buffer_read_u8_integer_offset_fn(f: BufferReadIntegerOffsetFn) {
    ACTIVE_BUFFER_READ_U8_INTEGER_OFFSET_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_buffer_read_u8_integer_offset_fn() {
    ACTIVE_BUFFER_READ_U8_INTEGER_OFFSET_FN.with(|c| c.set(None));
}
pub fn set_active_buffer_read_u16be_integer_offset_fn(f: BufferReadIntegerOffsetFn) {
    ACTIVE_BUFFER_READ_U16BE_INTEGER_OFFSET_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_buffer_read_u16be_integer_offset_fn() {
    ACTIVE_BUFFER_READ_U16BE_INTEGER_OFFSET_FN.with(|c| c.set(None));
}
pub fn set_active_buffer_read_u16le_integer_offset_fn(f: BufferReadIntegerOffsetFn) {
    ACTIVE_BUFFER_READ_U16LE_INTEGER_OFFSET_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_buffer_read_u16le_integer_offset_fn() {
    ACTIVE_BUFFER_READ_U16LE_INTEGER_OFFSET_FN.with(|c| c.set(None));
}
pub fn set_active_buffer_read_u32be_integer_offset_fn(f: BufferReadIntegerOffsetFn) {
    ACTIVE_BUFFER_READ_U32BE_INTEGER_OFFSET_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_buffer_read_u32be_integer_offset_fn() {
    ACTIVE_BUFFER_READ_U32BE_INTEGER_OFFSET_FN.with(|c| c.set(None));
}
pub fn set_active_buffer_read_u32le_integer_offset_fn(f: BufferReadIntegerOffsetFn) {
    ACTIVE_BUFFER_READ_U32LE_INTEGER_OFFSET_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_buffer_read_u32le_integer_offset_fn() {
    ACTIVE_BUFFER_READ_U32LE_INTEGER_OFFSET_FN.with(|c| c.set(None));
}
pub fn set_active_buffer_read_i32be_integer_offset_fn(f: BufferReadIntegerOffsetFn) {
    ACTIVE_BUFFER_READ_I32BE_INTEGER_OFFSET_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_buffer_read_i32be_integer_offset_fn() {
    ACTIVE_BUFFER_READ_I32BE_INTEGER_OFFSET_FN.with(|c| c.set(None));
}
pub fn set_active_host_construct1_objret_fn(f: HostConstruct1ObjRetFn) {
    ACTIVE_HOST_CONSTRUCT1_OBJRET_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_host_construct1_objret_fn() {
    ACTIVE_HOST_CONSTRUCT1_OBJRET_FN.with(|c| c.set(None));
}
pub fn set_active_call_direct1_obj_fn(f: CallDirect1ObjFn) {
    ACTIVE_CALL_DIRECT1_OBJ_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_call_direct1_obj_fn() {
    ACTIVE_CALL_DIRECT1_OBJ_FN.with(|c| c.set(None));
}
pub fn set_active_call_direct1_obj_objret_fn(f: CallDirect1ObjObjRetFn) {
    ACTIVE_CALL_DIRECT1_OBJ_OBJRET_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_call_direct1_obj_objret_fn() {
    ACTIVE_CALL_DIRECT1_OBJ_OBJRET_FN.with(|c| c.set(None));
}
pub fn set_active_call_direct2_obj_num_fn(f: CallDirect2ObjNumFn) {
    ACTIVE_CALL_DIRECT2_OBJ_NUM_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_call_direct2_obj_num_fn() {
    ACTIVE_CALL_DIRECT2_OBJ_NUM_FN.with(|c| c.set(None));
}
pub fn set_active_call_direct2_obj_num_void_fn(f: CallDirect2ObjNumVoidFn) {
    ACTIVE_CALL_DIRECT2_OBJ_NUM_VOID_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_call_direct2_obj_num_void_fn() {
    ACTIVE_CALL_DIRECT2_OBJ_NUM_VOID_FN.with(|c| c.set(None));
}
pub fn set_active_call_direct2_obj_num_objret_fn(f: CallDirect2ObjNumObjRetFn) {
    ACTIVE_CALL_DIRECT2_OBJ_NUM_OBJRET_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_call_direct2_obj_num_objret_fn() {
    ACTIVE_CALL_DIRECT2_OBJ_NUM_OBJRET_FN.with(|c| c.set(None));
}
pub fn set_active_call_direct2_num_num_fn(f: CallDirect2NumNumFn) {
    ACTIVE_CALL_DIRECT2_NUM_NUM_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_call_direct2_num_num_fn() {
    ACTIVE_CALL_DIRECT2_NUM_NUM_FN.with(|c| c.set(None));
}
pub fn set_active_call_direct2_obj_obj_predicate_fn(f: CallDirect2ObjObjPredicateFn) {
    ACTIVE_CALL_DIRECT2_OBJ_OBJ_PREDICATE_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_call_direct2_obj_obj_predicate_fn() {
    ACTIVE_CALL_DIRECT2_OBJ_OBJ_PREDICATE_FN.with(|c| c.set(None));
}
pub fn set_active_call_captured_add_arg_store_fn(f: CallCapturedAddArgStoreFn) {
    ACTIVE_CALL_CAPTURED_ADD_ARG_STORE_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_call_captured_add_arg_store_fn() {
    ACTIVE_CALL_CAPTURED_ADD_ARG_STORE_FN.with(|c| c.set(None));
}
pub fn set_active_call_direct0_value_store_dead_fn(f: CallDirect0ValueStoreDeadFn) {
    ACTIVE_CALL_DIRECT0_VALUE_STORE_DEAD_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_call_direct0_value_store_dead_fn() {
    ACTIVE_CALL_DIRECT0_VALUE_STORE_DEAD_FN.with(|c| c.set(None));
}
pub fn set_active_call_direct4_value_store_dead_fn(f: CallDirect4ValueStoreDeadFn) {
    ACTIVE_CALL_DIRECT4_VALUE_STORE_DEAD_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_call_direct4_value_store_dead_fn() {
    ACTIVE_CALL_DIRECT4_VALUE_STORE_DEAD_FN.with(|c| c.set(None));
}
pub fn set_active_call_direct4_value_store_dead_lanes_fn(f: CallDirect4ValueStoreDeadLanesFn) {
    ACTIVE_CALL_DIRECT4_VALUE_STORE_DEAD_LANES_FN.with(|c| c.set(Some(f)));
}
pub fn clear_active_call_direct4_value_store_dead_lanes_fn() {
    ACTIVE_CALL_DIRECT4_VALUE_STORE_DEAD_LANES_FN.with(|c| c.set(None));
}

pub fn set_current_value_writeback_cells(cells: &[(i64, i64)]) {
    CURRENT_VALUE_WRITEBACK_CELLS.with(|slot_cells| {
        let mut slot_cells = slot_cells.borrow_mut();
        slot_cells.clear();
        slot_cells.extend_from_slice(cells);
    });
}

pub fn clear_current_value_writeback_cells() {
    CURRENT_VALUE_WRITEBACK_CELLS.with(|slot_cells| slot_cells.borrow_mut().clear());
}

pub fn current_value_writeback_cell_ptr(upvalue_slot: i64) -> Option<i64> {
    CURRENT_VALUE_WRITEBACK_CELLS.with(|slot_cells| {
        slot_cells
            .borrow()
            .iter()
            .find_map(|(slot, cell_ptr)| (*slot == upvalue_slot).then_some(*cell_ptr))
    })
}

#[no_mangle]
pub extern "C" fn jit_setprop_on_object(receiver_idx: i64, prop_name_idx: i64, value: f64) {
    if let Some(f) = ACTIVE_SETPROP_FN.with(|c| c.get()) {
        f(receiver_idx, prop_name_idx, value);
    }

}

#[no_mangle]
pub extern "C" fn jit_setprop_fresh_data_add_on_object(
    receiver_idx: i64,
    prop_name_idx: i64,
    value: f64,
) {
    if let Some(f) = ACTIVE_SETPROP_FRESH_DATA_ADD_FN.with(|c| c.get()) {
        f(receiver_idx, prop_name_idx, value);
    }

}

#[no_mangle]
pub extern "C" fn jit_mwor_setprop_on_object(receiver_idx: i64, prop_name_idx: i64, value: f64) {
    if let Some(f) = ACTIVE_MWOR_SETPROP_FN.with(|c| c.get()) {
        f(receiver_idx, prop_name_idx, value);
    }
}

#[no_mangle]
pub extern "C" fn jit_setindex_on_object(receiver_idx: i64, index_i64: i64, value: f64) {
    if let Some(f) = ACTIVE_SETINDEX_FN.with(|c| c.get()) {
        f(receiver_idx, index_i64, value);
    }
}

#[no_mangle]
pub extern "C" fn jit_setglobal_var(name_idx: i64, value: f64) {
    if let Some(f) = ACTIVE_SETGLOBAL_VAR_FN.with(|c| c.get()) {
        f(name_idx, value);
    }
}

#[no_mangle]
pub extern "C" fn jit_call_direct1(callee_payload: i64, arg0: f64) -> f64 {
    if let Some(f) = ACTIVE_CALL_DIRECT1_FN.with(|c| c.get()) {
        f(callee_payload, arg0)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_call_indexed_direct1(
    receiver_payload: i64,
    callee_payload: i64,
    arg0: f64,
) -> f64 {
    if let Some(f) = ACTIVE_CALL_INDEXED_DIRECT1_FN.with(|c| c.get()) {
        f(receiver_payload, callee_payload, arg0)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_call_direct1_void(callee_payload: i64, arg0: f64) -> f64 {
    if let Some(f) = ACTIVE_CALL_DIRECT1_VOID_FN.with(|c| c.get()) {
        f(callee_payload, arg0)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_call_direct0(callee_payload: i64) -> f64 {
    if let Some(f) = ACTIVE_CALL_DIRECT0_FN.with(|c| c.get()) {
        f(callee_payload)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_call_direct0_objret(callee_payload: i64) -> i64 {
    if let Some(f) = ACTIVE_CALL_DIRECT0_OBJRET_FN.with(|c| c.get()) {
        f(callee_payload)
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn jit_call_direct1_objret(callee_payload: i64, arg0: f64) -> i64 {
    if let Some(f) = ACTIVE_CALL_DIRECT1_OBJRET_FN.with(|c| c.get()) {
        f(callee_payload, arg0)
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn jit_call_direct1_objret_nonnull_guard(callee_payload: i64, arg0: f64) -> i64 {
    if let Some(f) = ACTIVE_CALL_DIRECT1_OBJRET_NONNULL_GUARD_FN.with(|c| c.get()) {
        f(callee_payload, arg0)
    } else if let Some(f) = ACTIVE_CALL_DIRECT1_OBJRET_FN.with(|c| c.get()) {
        f(callee_payload, arg0)
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn jit_host_method1_objret(
    receiver_payload: i64,
    prop_name_idx: i64,
    arg0: f64,
) -> i64 {
    if let Some(f) = ACTIVE_HOST_METHOD1_OBJRET_FN.with(|c| c.get()) {
        f(receiver_payload, prop_name_idx, arg0)
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn jit_host_method2_objret(
    receiver_payload: i64,
    prop_name_idx: i64,
    arg0: f64,
    arg1: f64,
) -> i64 {
    if let Some(f) = ACTIVE_HOST_METHOD2_OBJRET_FN.with(|c| c.get()) {
        f(receiver_payload, prop_name_idx, arg0, arg1)
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn jit_host_method0_numret(prop_name_idx: i64) -> f64 {
    if let Some(f) = ACTIVE_HOST_METHOD0_NUMRET_FN.with(|c| c.get()) {
        f(prop_name_idx)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_host_method1_numret(
    receiver_payload: i64,
    prop_name_idx: i64,
    arg0: f64,
) -> f64 {
    if let Some(f) = ACTIVE_HOST_METHOD1_NUMRET_FN.with(|c| c.get()) {
        f(receiver_payload, prop_name_idx, arg0)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_host_method1_string(
    receiver_payload: i64,
    prop_name_idx: i64,
    arg0: f64,
) -> f64 {
    if let Some(f) = ACTIVE_HOST_METHOD1_STRING_FN.with(|c| c.get()) {
        f(receiver_payload, prop_name_idx, arg0)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_host_method2_string(
    receiver_payload: i64,
    prop_name_idx: i64,
    arg0: f64,
    arg1: f64,
) -> f64 {
    if let Some(f) = ACTIVE_HOST_METHOD2_STRING_FN.with(|c| c.get()) {
        f(receiver_payload, prop_name_idx, arg0, arg1)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_host_method4_string(
    receiver_payload: i64,
    prop_name_idx: i64,
    arg0: f64,
    arg1: f64,
    arg2: f64,
    arg3: f64,
) -> f64 {
    if let Some(f) = ACTIVE_HOST_METHOD4_STRING_FN.with(|c| c.get()) {
        f(receiver_payload, prop_name_idx, arg0, arg1, arg2, arg3)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_host_global_object(global_name_idx: i64) -> i64 {
    if let Some(f) = ACTIVE_HOST_GLOBAL_OBJECT_FN.with(|c| c.get()) {
        f(global_name_idx)
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn jit_host_object_prop_string(receiver_payload: i64, prop_name_idx: i64) -> f64 {
    if let Some(f) = ACTIVE_HOST_OBJECT_PROP_STRING_FN.with(|c| c.get()) {
        f(receiver_payload, prop_name_idx)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_host_object_prop_numret(receiver_payload: i64, prop_name_idx: i64) -> f64 {
    if let Some(f) = ACTIVE_HOST_OBJECT_PROP_NUMRET_FN.with(|c| c.get()) {
        f(receiver_payload, prop_name_idx)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_host_object_method0_string(receiver_payload: i64, prop_name_idx: i64) -> f64 {
    if let Some(f) = ACTIVE_HOST_OBJECT_METHOD0_STRING_FN.with(|c| c.get()) {
        f(receiver_payload, prop_name_idx)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_host_object_method1_string(
    receiver_payload: i64,
    prop_name_idx: i64,
    arg0: f64,
) -> f64 {
    if let Some(f) = ACTIVE_HOST_OBJECT_METHOD1_STRING_FN.with(|c| c.get()) {
        f(receiver_payload, prop_name_idx, arg0)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_buffer_write_u32be(receiver_payload: i64, value: f64, offset: f64) -> f64 {
    if let Some(f) = ACTIVE_BUFFER_WRITE_U32BE_FN.with(|c| c.get()) {
        f(receiver_payload, value, offset)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_buffer_write_u32be_u32_offset(
    receiver_payload: i64,
    value_u32: i64,
    offset: i64,
) -> f64 {
    if let Some(f) = ACTIVE_BUFFER_WRITE_U32BE_U32_OFFSET_FN.with(|c| c.get()) {
        f(receiver_payload, value_u32, offset)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_buffer_write_u32le_u32_offset(
    receiver_payload: i64,
    value_u32: i64,
    offset: i64,
) -> f64 {
    if let Some(f) = ACTIVE_BUFFER_WRITE_U32LE_U32_OFFSET_FN.with(|c| c.get()) {
        f(receiver_payload, value_u32, offset)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_buffer_write_u8_u32_offset(
    receiver_payload: i64,
    value_u32: i64,
    offset: i64,
) -> f64 {
    if let Some(f) = ACTIVE_BUFFER_WRITE_U8_U32_OFFSET_FN.with(|c| c.get()) {
        f(receiver_payload, value_u32, offset)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_buffer_write_u16be_u32_offset(
    receiver_payload: i64,
    value_u32: i64,
    offset: i64,
) -> f64 {
    if let Some(f) = ACTIVE_BUFFER_WRITE_U16BE_U32_OFFSET_FN.with(|c| c.get()) {
        f(receiver_payload, value_u32, offset)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_buffer_write_u16le_u32_offset(
    receiver_payload: i64,
    value_u32: i64,
    offset: i64,
) -> f64 {
    if let Some(f) = ACTIVE_BUFFER_WRITE_U16LE_U32_OFFSET_FN.with(|c| c.get()) {
        f(receiver_payload, value_u32, offset)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_buffer_read_u32be(receiver_payload: i64, offset: f64) -> f64 {
    if let Some(f) = ACTIVE_BUFFER_READ_U32BE_FN.with(|c| c.get()) {
        f(receiver_payload, offset)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_buffer_read_u8_integer_offset(receiver_payload: i64, offset: i64) -> f64 {
    if let Some(f) = ACTIVE_BUFFER_READ_U8_INTEGER_OFFSET_FN.with(|c| c.get()) {
        f(receiver_payload, offset)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_buffer_read_u16be_integer_offset(receiver_payload: i64, offset: i64) -> f64 {
    if let Some(f) = ACTIVE_BUFFER_READ_U16BE_INTEGER_OFFSET_FN.with(|c| c.get()) {
        f(receiver_payload, offset)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_buffer_read_u16le_integer_offset(receiver_payload: i64, offset: i64) -> f64 {
    if let Some(f) = ACTIVE_BUFFER_READ_U16LE_INTEGER_OFFSET_FN.with(|c| c.get()) {
        f(receiver_payload, offset)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_buffer_read_u32be_integer_offset(receiver_payload: i64, offset: i64) -> f64 {
    if let Some(f) = ACTIVE_BUFFER_READ_U32BE_INTEGER_OFFSET_FN.with(|c| c.get()) {
        f(receiver_payload, offset)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_buffer_read_u32le_integer_offset(receiver_payload: i64, offset: i64) -> f64 {
    if let Some(f) = ACTIVE_BUFFER_READ_U32LE_INTEGER_OFFSET_FN.with(|c| c.get()) {
        f(receiver_payload, offset)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_buffer_read_i32be_integer_offset(receiver_payload: i64, offset: i64) -> f64 {
    if let Some(f) = ACTIVE_BUFFER_READ_I32BE_INTEGER_OFFSET_FN.with(|c| c.get()) {
        f(receiver_payload, offset)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_host_construct1_objret(callee_payload: i64, arg0: f64) -> i64 {
    if let Some(f) = ACTIVE_HOST_CONSTRUCT1_OBJRET_FN.with(|c| c.get()) {
        f(callee_payload, arg0)
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn jit_call_direct1_obj(callee_payload: i64, arg0_payload: i64) -> f64 {
    if let Some(f) = ACTIVE_CALL_DIRECT1_OBJ_FN.with(|c| c.get()) {
        f(callee_payload, arg0_payload)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_call_direct1_obj_objret(callee_payload: i64, arg0_payload: i64) -> i64 {
    if let Some(f) = ACTIVE_CALL_DIRECT1_OBJ_OBJRET_FN.with(|c| c.get()) {
        f(callee_payload, arg0_payload)
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn jit_call_direct2_obj_num(
    callee_payload: i64,
    arg0_payload: i64,
    arg1: f64,
) -> f64 {
    if let Some(f) = ACTIVE_CALL_DIRECT2_OBJ_NUM_FN.with(|c| c.get()) {
        f(callee_payload, arg0_payload, arg1)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_call_direct2_obj_num_void(
    callee_payload: i64,
    arg0_payload: i64,
    arg1: f64,
) -> i64 {
    if let Some(f) = ACTIVE_CALL_DIRECT2_OBJ_NUM_VOID_FN.with(|c| c.get()) {
        f(callee_payload, arg0_payload, arg1)
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C" fn jit_call_direct2_obj_num_objret(
    callee_payload: i64,
    arg0_payload: i64,
    arg1: f64,
) -> i64 {
    if let Some(f) = ACTIVE_CALL_DIRECT2_OBJ_NUM_OBJRET_FN.with(|c| c.get()) {
        f(callee_payload, arg0_payload, arg1)
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn jit_call_direct2_num_num(callee_payload: i64, arg0: f64, arg1: f64) -> f64 {
    if let Some(f) = ACTIVE_CALL_DIRECT2_NUM_NUM_FN.with(|c| c.get()) {
        f(callee_payload, arg0, arg1)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_call_direct2_obj_obj_predicate(
    callee_payload: i64,
    arg0_payload: i64,
    arg1_payload: i64,
) -> i64 {
    if let Some(f) = ACTIVE_CALL_DIRECT2_OBJ_OBJ_PREDICATE_FN.with(|c| c.get()) {
        f(callee_payload, arg0_payload, arg1_payload)
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C" fn jit_call_direct2_prop_predicate(
    callee_payload: i64,
    arg0_receiver_payload: i64,
    arg1_receiver_payload: i64,
    prop_name_idx: i64,
) -> i64 {
    if let Some(f) = ACTIVE_CALL_DIRECT2_PROP_PREDICATE_FN.with(|c| c.get()) {
        f(
            callee_payload,
            arg0_receiver_payload,
            arg1_receiver_payload,
            prop_name_idx,
        )
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C" fn jit_aria_parent_predicate_fusion(
    constraints_callee_payload: i64,
    attributes_callee_payload: i64,
    arg0_receiver_payload: i64,
    arg1_receiver_payload: i64,
    name_prop_idx: i64,
    constraints_prop_idx: i64,
    attributes_prop_idx: i64,
) -> i64 {
    let timing_t0 =
        aria_parent_predicate_fusion_boundary_timing_enabled().then(std::time::Instant::now);
    let result = if aria_parent_predicate_fusion_stub_true_enabled() {
        1
    } else if let Some(f) = ACTIVE_ARIA_PARENT_PREDICATE_FUSION_FN.with(|c| c.get()) {
        f(
            constraints_callee_payload,
            attributes_callee_payload,
            arg0_receiver_payload,
            arg1_receiver_payload,
            name_prop_idx,
            constraints_prop_idx,
            attributes_prop_idx,
        )
    } else {
        -1
    };
    if let Some(t0) = timing_t0 {
        record_aria_parent_predicate_fusion_boundary_timing(result, t0.elapsed().as_nanos());
    }
    result
}

#[no_mangle]
pub extern "C" fn jit_call_captured_add_arg_store(cell_ptr: i64, arg0: f64) -> f64 {
    if let Some(f) = ACTIVE_CALL_CAPTURED_ADD_ARG_STORE_FN.with(|c| c.get()) {
        f(cell_ptr, arg0)
    } else {
        f64::NAN
    }
}

#[no_mangle]
pub extern "C" fn jit_call_direct0_value_store_dead(callee_payload: i64, cell_ptr: i64) -> i64 {
    if let Some(f) = ACTIVE_CALL_DIRECT0_VALUE_STORE_DEAD_FN.with(|c| c.get()) {
        f(callee_payload, cell_ptr)
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C" fn jit_call_direct0_value_store_dead_upvalue(
    callee_payload: i64,
    upvalue_slot: i64,
) -> i64 {
    if let Some(cell_ptr) = current_value_writeback_cell_ptr(upvalue_slot) {
        jit_call_direct0_value_store_dead(callee_payload, cell_ptr)
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C" fn jit_call_direct4_value_store_dead(
    callee_payload: i64,
    cell_ptr: i64,
    arg0_ptr: i64,
    arg1_ptr: i64,
    arg2_ptr: i64,
    arg3_ptr: i64,
) -> i64 {
    if let Some(f) = ACTIVE_CALL_DIRECT4_VALUE_STORE_DEAD_FN.with(|c| c.get()) {
        f(
            callee_payload,
            cell_ptr,
            arg0_ptr,
            arg1_ptr,
            arg2_ptr,
            arg3_ptr,
        )
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C" fn jit_call_direct4_value_store_dead_upvalue(
    callee_payload: i64,
    upvalue_slot: i64,
    arg0_ptr: i64,
    arg1_ptr: i64,
    arg2_ptr: i64,
    arg3_ptr: i64,
) -> i64 {
    if let Some(cell_ptr) = current_value_writeback_cell_ptr(upvalue_slot) {
        jit_call_direct4_value_store_dead(
            callee_payload,
            cell_ptr,
            arg0_ptr,
            arg1_ptr,
            arg2_ptr,
            arg3_ptr,
        )
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C" fn jit_call_direct4_value_store_dead_lanes_upvalue(
    callee_payload: i64,
    upvalue_slot: i64,
    arg0_tag: i64,
    arg0: f64,
    arg1_tag: i64,
    arg1: f64,
    arg2_tag: i64,
    arg2: f64,
    arg3_tag: i64,
    arg3: f64,
) -> i64 {
    let Some(cell_ptr) = current_value_writeback_cell_ptr(upvalue_slot) else {
        return -1;
    };
    if let Some(f) = ACTIVE_CALL_DIRECT4_VALUE_STORE_DEAD_LANES_FN.with(|c| c.get()) {
        f(
            callee_payload,
            cell_ptr,
            arg0_tag,
            arg0,
            arg1_tag,
            arg1,
            arg2_tag,
            arg2,
            arg3_tag,
            arg3,
        )
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C" fn jit_number_mod(lhs: f64, rhs: f64) -> f64 {
    lhs % rhs
}

pub type IcObserveFn = extern "C" fn(site_id: i64, receiver_idx: i64, prop_name_idx: i64);

thread_local! {
    static ACTIVE_IC_OBSERVE_FN: std::cell::Cell<Option<IcObserveFn>> =
        const { std::cell::Cell::new(None) };
}

pub fn set_active_ic_observe_fn(f: IcObserveFn) {
    ACTIVE_IC_OBSERVE_FN.with(|c| c.set(Some(f)));
}

pub fn clear_active_ic_observe_fn() {
    ACTIVE_IC_OBSERVE_FN.with(|c| c.set(None));
}

pub type IcFastGetFn =
    extern "C" fn(receiver_idx: i64, cached_shape_ptr_usize: i64, cached_slot: i64) -> i64;

pub const IC_FAST_MISS_SENTINEL: i64 = i64::MIN;

thread_local! {
    static ACTIVE_IC_FAST_GET_FN: std::cell::Cell<Option<IcFastGetFn>> =
        const { std::cell::Cell::new(None) };
}

pub fn set_active_ic_fast_get_fn(f: IcFastGetFn) {
    ACTIVE_IC_FAST_GET_FN.with(|c| c.set(Some(f)));
}

pub fn clear_active_ic_fast_get_fn() {
    ACTIVE_IC_FAST_GET_FN.with(|c| c.set(None));
}

#[no_mangle]
pub extern "C" fn jit_getprop_with_ic(site_id: i64, receiver_idx: i64, prop_name_idx: i64) -> i64 {

    let (cached_shape_ptr, cached_slot, is_warm_mono) =
        crate::stub_aarch64::IC_STUB_CACHE.with(|cell| {
            let cache = cell.borrow();
            let e = cache.entry(site_id as u32);
            (
                e.cached_shape as i64,
                e.cached_slot as i64,
                matches!(e.state(), crate::stub_aarch64::ICState::WarmMono),
            )
        });
    if is_warm_mono && cached_shape_ptr != 0 {
        if let Some(fast_get) = ACTIVE_IC_FAST_GET_FN.with(|c| c.get()) {
            let v = fast_get(receiver_idx, cached_shape_ptr, cached_slot);
            if v != IC_FAST_MISS_SENTINEL {
                return v;
            }

        }
    }

    let result = jit_getprop_on_object(receiver_idx, prop_name_idx);
    if let Some(observe) = ACTIVE_IC_OBSERVE_FN.with(|c| c.get()) {
        observe(site_id, receiver_idx, prop_name_idx);
    }
    result
}

pub type IcFastSetFn = extern "C" fn(
    receiver_idx: i64,
    cached_shape_ptr_usize: i64,
    cached_slot: i64,
    value: f64,
) -> i64;

thread_local! {
    static ACTIVE_IC_FAST_SET_FN: std::cell::Cell<Option<IcFastSetFn>> =
        const { std::cell::Cell::new(None) };
}

pub fn set_active_ic_fast_set_fn(f: IcFastSetFn) {
    ACTIVE_IC_FAST_SET_FN.with(|c| c.set(Some(f)));
}

pub fn clear_active_ic_fast_set_fn() {
    ACTIVE_IC_FAST_SET_FN.with(|c| c.set(None));
}

#[no_mangle]
pub extern "C" fn jit_setprop_with_ic(
    site_id: i64,
    receiver_idx: i64,
    prop_name_idx: i64,
    value: f64,
) {
    let (cached_shape_ptr, cached_slot, is_warm_mono) =
        crate::stub_aarch64::IC_STUB_CACHE.with(|cell| {
            let cache = cell.borrow();
            let e = cache.entry(site_id as u32);
            (
                e.cached_shape as i64,
                e.cached_slot as i64,
                matches!(e.state(), crate::stub_aarch64::ICState::WarmMono),
            )
        });
    if is_warm_mono && cached_shape_ptr != 0 {
        if let Some(fast_set) = ACTIVE_IC_FAST_SET_FN.with(|c| c.get()) {
            if fast_set(receiver_idx, cached_shape_ptr, cached_slot, value) != IC_FAST_MISS_SENTINEL
            {
                return;
            }

        }
    }
    jit_setprop_on_object(receiver_idx, prop_name_idx, value);
    if let Some(observe) = ACTIVE_IC_OBSERVE_FN.with(|c| c.get()) {
        observe(site_id, receiver_idx, prop_name_idx);
    }
}

#[no_mangle]
pub extern "C" fn deopt_trip(site_id: i64, r0: i64, r1: i64, r2: i64, r3: i64) -> i64 {
    if deopt_trip_trace_enabled() {
        eprintln!("[deopt-trip-trace] deopt_trip site_id={site_id} regs=[{r0}, {r1}, {r2}, {r3}]");
    }
    let frame = DeoptCallFrame {
        site_id: site_id as u32,
        regs: [r0, r1, r2, r3, 0, 0, 0, 0],
        frame_base: 0,
    };
    record_deopt_frame(frame)
}

#[no_mangle]
pub extern "C" fn deopt_trip_with_frame_base(
    site_id: i64,
    r0: i64,
    r1: i64,
    r2: i64,
    r3: i64,
    r4: i64,
    r5: i64,
    r6: i64,
    r7: i64,
    frame_base: i64,
) -> i64 {
    if deopt_trip_trace_enabled() {
        eprintln!(
            "[deopt-trip-trace] deopt_trip_with_frame_base site_id={site_id} regs=[{r0}, {r1}, {r2}, {r3}, {r4}, {r5}, {r6}, {r7}] frame_base={frame_base}"
        );
    }
    let frame = DeoptCallFrame {
        site_id: site_id as u32,
        regs: [r0, r1, r2, r3, r4, r5, r6, r7],
        frame_base,
    };
    record_deopt_frame(frame)
}

fn record_deopt_frame(frame: DeoptCallFrame) -> i64 {
    let sites_ptr = CURRENT_DEOPT_SITES.with(|c| c.get());
    if sites_ptr.is_null() {
        if deopt_trip_trace_enabled() {
            eprintln!(
                "[deopt-trip-trace] record_deopt_frame site_id={} missing current site table",
                frame.site_id
            );
        }

        return 0;
    }

    let sites: &DeoptSiteTable = unsafe { &*sites_ptr };
    if let Some(state) = reconstruct_state(sites, &frame) {
        if deopt_trip_trace_enabled() {
            eprintln!(
                "[deopt-trip-trace] record_deopt_frame site_id={} recovered reason={:?} resume_pc={} locals={:?} stack_depth={}",
                frame.site_id,
                state.reason,
                state.resume_pc,
                state.local_values,
                state.stack_values.len()
            );
        }
        LAST_DEOPT_FRAME.with(|c| *c.borrow_mut() = Some(state));
    } else if deopt_trip_trace_enabled() {
        eprintln!(
            "[deopt-trip-trace] record_deopt_frame site_id={} reconstruct_state returned none",
            frame.site_id
        );
    }
    0
}

fn deopt_trip_trace_enabled() -> bool {
    std::env::var("CRUFT_LEJIT_DEOPT_TRIP_TRACE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

pub fn set_current_deopt_sites(sites: &DeoptSiteTable) {
    CURRENT_DEOPT_SITES.with(|c| c.set(sites as *const _));
}

pub fn clear_current_deopt_sites() {
    CURRENT_DEOPT_SITES.with(|c| c.set(std::ptr::null()));
}

pub fn take_last_deopt() -> Option<DeoptRecoveredState> {
    LAST_DEOPT_FRAME.with(|c| c.borrow_mut().take())
}

#[cfg(test)]
mod thunk_tests {
    use super::*;

    #[test]
    fn deopt_trip_populates_last_frame() {
        let sites = vec![DeoptSite {
            reason: DeoptReason::IntegerOverflow { op_pc: 8 },
            resume_pc: 10,
            live_locals: vec![DeoptLiveLocal {
                interp_slot: 0,
                jit_location: JitLocation::Register(0),
            }],
            stack_depth: 0,
            stack_slots: vec![],
        }];
        set_current_deopt_sites(&sites);
        let result = deopt_trip(0, 42, 0, 0, 0);
        assert_eq!(result, 0, "thunk returns sentinel 0");
        let recovered = take_last_deopt().expect("trip recorded");
        assert_eq!(recovered.resume_pc, 10);
        assert_eq!(recovered.local_values, vec![(0, 42)]);
        clear_current_deopt_sites();
    }

    #[test]
    fn deopt_trip_with_frame_base_populates_stack_slot_frame() {
        let spill = [9001_i64];
        let sites = vec![DeoptSite {
            reason: DeoptReason::BoundaryArgMismatch,
            resume_pc: 109,
            live_locals: vec![
                DeoptLiveLocal {
                    interp_slot: 11,
                    jit_location: JitLocation::Register(7),
                },
                DeoptLiveLocal {
                    interp_slot: 12,
                    jit_location: JitLocation::StackSlot(0),
                },
            ],
            stack_depth: 0,
            stack_slots: vec![],
        }];
        set_current_deopt_sites(&sites);
        let result = deopt_trip_with_frame_base(0, 0, 0, 0, 0, 0, 0, 0, 777, spill.as_ptr() as i64);
        assert_eq!(result, 0, "thunk returns sentinel 0");
        let recovered = take_last_deopt().expect("trip recorded");
        assert_eq!(recovered.resume_pc, 109);
        assert_eq!(recovered.local_values, vec![(11, 777), (12, 9001)]);
        clear_current_deopt_sites();
    }

    #[test]
    fn deopt_trip_without_table_no_panic() {
        clear_current_deopt_sites();

        let result = deopt_trip(0, 0, 0, 0, 0);
        assert_eq!(result, 0);

        assert!(take_last_deopt().is_none());
    }

    #[test]
    fn last_deopt_clears_after_take() {
        let sites = vec![DeoptSite {
            reason: DeoptReason::BoundaryArgMismatch,
            resume_pc: 0,
            live_locals: vec![],
            stack_depth: 0,
            stack_slots: vec![],
        }];
        set_current_deopt_sites(&sites);
        deopt_trip(0, 0, 0, 0, 0);
        assert!(take_last_deopt().is_some());
        assert!(take_last_deopt().is_none(), "second take returns None");
        clear_current_deopt_sites();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site_with_locals(
        reason: DeoptReason,
        resume_pc: u32,
        locals: Vec<(u16, JitLocation)>,
    ) -> DeoptSite {
        DeoptSite {
            reason,
            resume_pc,
            live_locals: locals
                .into_iter()
                .map(|(slot, loc)| DeoptLiveLocal {
                    interp_slot: slot,
                    jit_location: loc,
                })
                .collect(),
            stack_depth: 0,
            stack_slots: Vec::new(),
        }
    }

    #[test]
    fn empty_site_reconstructs_to_empty_state() {
        let site = site_with_locals(DeoptReason::IntegerOverflow { op_pc: 42 }, 16, vec![]);
        let frame = DeoptCallFrame {
            site_id: 0,
            regs: [0; 8],
            frame_base: 0,
        };
        let r = reconstruct_state(&[site], &frame).expect("site found");
        assert_eq!(r.reason, DeoptReason::IntegerOverflow { op_pc: 42 });
        assert_eq!(r.resume_pc, 16);
        assert!(r.local_values.is_empty());
        assert!(r.stack_values.is_empty());
    }

    #[test]
    fn register_locations_reconstruct() {
        let site = site_with_locals(
            DeoptReason::BoundaryArgMismatch,
            0,
            vec![
                (0, JitLocation::Register(0)),
                (1, JitLocation::Register(2)),
                (2, JitLocation::Constant(99)),
            ],
        );
        let frame = DeoptCallFrame {
            site_id: 0,
            regs: [100, 0, 200, 0, 0, 0, 0, 0],
            frame_base: 0,
        };
        let r = reconstruct_state(&[site], &frame).expect("site found");
        assert_eq!(r.local_values, vec![(0, 100), (1, 200), (2, 99)]);
    }

    #[test]
    fn missing_site_id_returns_none() {
        let frame = DeoptCallFrame {
            site_id: 5,
            regs: [0; 8],
            frame_base: 0,
        };
        let r = reconstruct_state(&[], &frame);
        assert!(r.is_none());
    }

    #[test]
    fn thunk_routes_to_reconstructor() {
        let site = site_with_locals(
            DeoptReason::ICShapeMismatch { ic_id: 7 },
            128,
            vec![(0, JitLocation::Register(0))],
        );
        let frame = DeoptCallFrame {
            site_id: 0,
            regs: [42, 0, 0, 0, 0, 0, 0, 0],
            frame_base: 0,
        };
        let r = jit_deopt_thunk(&[site], frame).expect("thunk recovered");
        assert_eq!(r.reason, DeoptReason::ICShapeMismatch { ic_id: 7 });
        assert_eq!(r.resume_pc, 128);
        assert_eq!(r.local_values, vec![(0, 42)]);
    }

    #[test]
    fn site135_hole_prototype_miss_reason_round_trips() {
        let site = DeoptSite {
            reason: DeoptReason::Site135HolePrototypeMiss { read_pc: 135 },
            resume_pc: 135,
            live_locals: vec![
                DeoptLiveLocal {
                    interp_slot: 0,
                    jit_location: JitLocation::Register(0),
                },
                DeoptLiveLocal {
                    interp_slot: 1,
                    jit_location: JitLocation::Register(1),
                },
            ],
            stack_depth: 2,
            stack_slots: vec![
                DeoptLiveLocal {
                    interp_slot: 0,
                    jit_location: JitLocation::Register(2),
                },
                DeoptLiveLocal {
                    interp_slot: 1,
                    jit_location: JitLocation::Constant(17),
                },
            ],
        };
        let frame = DeoptCallFrame {
            site_id: 0,
            regs: [44, 17, 9, 0, 0, 0, 0, 0],
            frame_base: 0,
        };
        let r = reconstruct_state(&[site], &frame).expect("site found");
        assert_eq!(
            r.reason,
            DeoptReason::Site135HolePrototypeMiss { read_pc: 135 }
        );
        assert_eq!(r.resume_pc, 135);
        assert_eq!(r.local_values, vec![(0, 44), (1, 17)]);
        assert_eq!(r.stack_values, vec![(0, 9), (1, 17)]);
    }

    #[test]
    fn stack_slot_locations_reconstruct() {
        let site = DeoptSite {
            reason: DeoptReason::IntegerOverflow { op_pc: 10 },
            resume_pc: 12,
            live_locals: vec![],
            stack_depth: 2,
            stack_slots: vec![
                DeoptLiveLocal {
                    interp_slot: 0,
                    jit_location: JitLocation::Register(0),
                },
                DeoptLiveLocal {
                    interp_slot: 1,
                    jit_location: JitLocation::Register(1),
                },
            ],
        };
        let frame = DeoptCallFrame {
            site_id: 0,
            regs: [7, 11, 0, 0, 0, 0, 0, 0],
            frame_base: 0,
        };
        let r = reconstruct_state(&[site], &frame).expect("site found");
        assert_eq!(r.stack_values, vec![(0, 7), (1, 11)]);
    }

    #[test]
    fn stack_slot_frame_base_locations_reconstruct() {
        let spill = [701_i64, 809_i64];
        let site = site_with_locals(
            DeoptReason::BoundaryArgMismatch,
            109,
            vec![
                (11, JitLocation::Register(7)),
                (12, JitLocation::StackSlot(0)),
                (13, JitLocation::StackSlot(8)),
            ],
        );
        let frame = DeoptCallFrame {
            site_id: 0,
            regs: [0, 0, 0, 0, 0, 0, 0, 611],
            frame_base: spill.as_ptr() as i64,
        };
        let r = reconstruct_state(&[site], &frame).expect("site found");
        assert_eq!(r.local_values, vec![(11, 611), (12, 701), (13, 809)]);
    }

    #[test]
    fn stack_slot_without_frame_base_reconstructs_zero() {
        let site = site_with_locals(
            DeoptReason::BoundaryArgMismatch,
            109,
            vec![(12, JitLocation::StackSlot(0))],
        );
        let frame = DeoptCallFrame {
            site_id: 0,
            regs: [0; DEOPT_CALL_FRAME_REGISTER_CAPACITY],
            frame_base: 0,
        };
        let r = reconstruct_state(&[site], &frame).expect("site found");
        assert_eq!(r.local_values, vec![(12, 0)]);
    }

    #[test]
    fn outcome_enum_discriminates() {
        let returned = JitCallOutcome::Returned(42);
        let deopted = JitCallOutcome::Deopted(3);
        match returned {
            JitCallOutcome::Returned(v) => assert_eq!(v, 42),
            _ => panic!("wrong variant"),
        }
        match deopted {
            JitCallOutcome::Deopted(id) => assert_eq!(id, 3),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn direct2_obj_obj_predicate_bridge_defaults_to_deopt_sentinel() {
        clear_active_call_direct2_obj_obj_predicate_fn();
        assert_eq!(jit_call_direct2_obj_obj_predicate(1, 2, 3), -1);
    }

    #[test]
    fn regexp_exec_result_kind_separates_normal_null_from_deopt() {
        assert_eq!(
            regexp_exec_ic_result_kind(17),
            RegexpExecIcResultKind::ObjectId
        );
        assert_eq!(
            regexp_exec_ic_result_kind(REGEXP_EXEC_NORMAL_NULL_SENTINEL),
            RegexpExecIcResultKind::NormalNull
        );
        assert_eq!(
            regexp_exec_ic_result_kind(REGEXP_EXEC_DEOPT_SENTINEL),
            RegexpExecIcResultKind::Deopt
        );
        assert_eq!(
            regexp_exec_ic_result_kind(-99),
            RegexpExecIcResultKind::Deopt
        );
        assert_ne!(REGEXP_EXEC_NORMAL_NULL_SENTINEL, REGEXP_EXEC_DEOPT_SENTINEL);
    }

    extern "C" fn test_regexp_exec_global_object_or_null(_receiver: i64, arg: f64) -> i64 {
        if arg == 1.0 {
            REGEXP_EXEC_NORMAL_NULL_SENTINEL
        } else {
            42
        }
    }

    #[test]
    fn regexp_exec_global_object_or_null_has_distinct_active_entry() {
        set_active_regexp_exec_global_object_or_null_ic_fn(test_regexp_exec_global_object_or_null);
        assert_eq!(call_regexp_exec_global_object_or_null_ic(7, 0.0), 42);
        assert_eq!(
            call_regexp_exec_global_object_or_null_ic(7, 1.0),
            REGEXP_EXEC_NORMAL_NULL_SENTINEL
        );
        assert_eq!(
            call_regexp_exec_ic(7, 1.0),
            REGEXP_EXEC_DEOPT_SENTINEL,
            "legacy exec IC must not be implicitly wired to the /g object-or-null entry"
        );
    }
}
