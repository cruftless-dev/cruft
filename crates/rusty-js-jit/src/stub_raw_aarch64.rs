
pub use crate::stub::{ICEntry, ICSiteId, ICState, ICStubCache, IC_STUB_CACHE, MISS_THRESHOLD};
pub use crate::stub_cranelift::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawPackedArrayReadThunkUnavailable {
    UnsupportedArchitecture,
    UnverifiedLayout,
    EmissionDeferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawPackedArrayReadThunkPlan {
    pub slot_stride: i32,
    pub slot_payload_off: i32,
    pub object_array_packed_off: i32,
    pub object_dense_doubles_off: i32,
    pub vec_len_off: i32,
    pub vec_ptr_off: i32,
    pub fallback: *const u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawPackedArrayReadThunkCode {
    pub bytes: Vec<u8>,
    pub heap_base_literal_offset: Option<usize>,
    pub fallback_literal_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacosArm64ExecProbeCode {
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacosArm64IndirectProbeCode {
    pub bytes: Vec<u8>,
    pub fallback_literal_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacosArm64InstalledProbe {
    ptr: *const u8,
    len: usize,
}

impl MacosArm64InstalledProbe {
    pub fn ptr(&self) -> *const u8 {
        self.ptr
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawPackedArrayReadThunk {
    ptr: *const u8,
    len: usize,
}

impl RawPackedArrayReadThunk {
    pub fn ptr(&self) -> *const u8 {
        self.ptr
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[cfg(test)]
    fn synthetic_for_test(ptr: *const u8, len: usize) -> Self {
        Self { ptr, len }
    }
}

pub fn raw_packed_array_read_supported() -> bool {
    cfg!(target_arch = "aarch64")
}

static RAW_PACKED_READ_PROBE_TARGET: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
static RAW_PACKED_READ_PROBE_CALLS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

pub fn install_raw_packed_read_probe_target(thunk: RawPackedArrayReadThunk) {
    RAW_PACKED_READ_PROBE_TARGET.store(thunk.ptr() as usize, std::sync::atomic::Ordering::Relaxed);
    RAW_PACKED_READ_PROBE_CALLS.store(0, std::sync::atomic::Ordering::Relaxed);
}

pub fn raw_packed_read_probe_call_count() -> u64 {
    RAW_PACKED_READ_PROBE_CALLS.load(std::sync::atomic::Ordering::Relaxed)
}

#[no_mangle]
pub extern "C" fn jit_packed_read_raw_probe_thunk(receiver: i64, index_i64: i64) -> f64 {
    let previous = RAW_PACKED_READ_PROBE_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if previous == 0 {
        eprintln!("[rpar-raw-probe] first-call receiver={receiver} index={index_i64}");
    }
    let target = RAW_PACKED_READ_PROBE_TARGET.load(std::sync::atomic::Ordering::Relaxed);
    if target == 0 {
        eprintln!("[rpar-raw-probe] missing target");
        return f64::NAN;
    }
    let f: extern "C" fn(i64, i64) -> f64 = unsafe { std::mem::transmute(target) };
    f(receiver, index_i64)
}

pub fn build_packed_array_read_raw_thunk_plan(
    layout: &crate::deopt::InlineIcLayout,
) -> Result<RawPackedArrayReadThunkPlan, RawPackedArrayReadThunkUnavailable> {
    if !raw_packed_array_read_supported() {
        return Err(RawPackedArrayReadThunkUnavailable::UnsupportedArchitecture);
    }
    if !layout.array_verified {
        return Err(RawPackedArrayReadThunkUnavailable::UnverifiedLayout);
    }
    Ok(RawPackedArrayReadThunkPlan {
        slot_stride: layout.slot_stride,
        slot_payload_off: layout.slot_payload_off,
        object_array_packed_off: layout.object_array_packed_off,
        object_dense_doubles_off: layout.object_dense_doubles_off,
        vec_len_off: layout.vec_len_off,
        vec_ptr_off: layout.vec_ptr_off,
        fallback: crate::deopt::jit_getindex_on_object as *const u8,
    })
}

pub fn encode_packed_array_read_raw_slow_thunk(
    plan: &RawPackedArrayReadThunkPlan,
) -> Result<RawPackedArrayReadThunkCode, RawPackedArrayReadThunkUnavailable> {
    if !raw_packed_array_read_supported() {
        return Err(RawPackedArrayReadThunkUnavailable::UnsupportedArchitecture);
    }

    let mut bytes = Vec::with_capacity(40);
    push_u32_le(&mut bytes, 0xA9BF_7BFD);
    push_u32_le(&mut bytes, 0x9100_03FD);
    push_u32_le(&mut bytes, 0x5800_00D0);
    push_u32_le(&mut bytes, 0xD63F_0200);
    push_u32_le(&mut bytes, 0xA8C1_7BFD);
    push_u32_le(&mut bytes, 0x9E62_0000);
    push_u32_le(&mut bytes, 0xD65F_03C0);
    push_u32_le(&mut bytes, 0xD503_201F);
    let fallback_literal_offset = bytes.len();
    push_u64_le(&mut bytes, plan.fallback as u64);

    Ok(RawPackedArrayReadThunkCode {
        bytes,
        heap_base_literal_offset: None,
        fallback_literal_offset,
    })
}

pub fn encode_packed_array_read_raw_fast_thunk(
    plan: &RawPackedArrayReadThunkPlan,
) -> Result<RawPackedArrayReadThunkCode, RawPackedArrayReadThunkUnavailable> {
    if !raw_packed_array_read_supported() {
        return Err(RawPackedArrayReadThunkUnavailable::UnsupportedArchitecture);
    }

    let slot_stride = u16_imm(plan.slot_stride)?;
    let slot_payload_off = u12_imm(plan.slot_payload_off)?;
    let packed_off = u12_imm(plan.object_array_packed_off)?;
    let len_off = u12_scaled_8(plan.object_dense_doubles_off + plan.vec_len_off)?;
    let ptr_off = u12_scaled_8(plan.object_dense_doubles_off + plan.vec_ptr_off)?;

    let mut bytes = Vec::with_capacity(108);
    push_u32_le(&mut bytes, 0xD340_5409);
    push_u32_le(&mut bytes, 0xD280_000B | ((slot_stride as u32) << 5));
    push_u32_le(&mut bytes, 0x9B0B_7D29);
    push_u32_le(&mut bytes, 0x5800_028A);
    push_u32_le(&mut bytes, 0xF940_014A);
    push_u32_le(&mut bytes, 0x8B09_0149);
    push_u32_le(&mut bytes, 0x9100_0129 | ((slot_payload_off as u32) << 10));
    push_u32_le(&mut bytes, 0x3940_012C | ((packed_off as u32) << 10));
    push_u32_le(&mut bytes, 0x3400_00EC);
    push_u32_le(&mut bytes, 0xF940_012D | ((len_off as u32 / 8) << 10));
    push_u32_le(&mut bytes, 0xEB0D_003F);
    push_u32_le(&mut bytes, 0x5400_0082);
    push_u32_le(&mut bytes, 0xF940_012E | ((ptr_off as u32 / 8) << 10));
    push_u32_le(&mut bytes, 0xFC61_79C0);
    push_u32_le(&mut bytes, 0xD65F_03C0);

    push_u32_le(&mut bytes, 0xA9BF_7BFD);
    push_u32_le(&mut bytes, 0x9100_03FD);
    push_u32_le(&mut bytes, 0x5800_0110);
    push_u32_le(&mut bytes, 0xD63F_0200);
    push_u32_le(&mut bytes, 0xA8C1_7BFD);
    push_u32_le(&mut bytes, 0x9E62_0000);
    push_u32_le(&mut bytes, 0xD65F_03C0);
    push_u32_le(&mut bytes, 0xD503_201F);

    let heap_base_literal_offset = bytes.len();
    push_u64_le(&mut bytes, crate::deopt::inline_ic_heap_base_addr() as u64);
    let fallback_literal_offset = bytes.len();
    push_u64_le(&mut bytes, plan.fallback as u64);

    Ok(RawPackedArrayReadThunkCode {
        bytes,
        heap_base_literal_offset: Some(heap_base_literal_offset),
        fallback_literal_offset,
    })
}

pub fn encode_macos_arm64_const_return_probe(imm16: u16) -> MacosArm64ExecProbeCode {
    let mut bytes = Vec::with_capacity(8);
    let mov_x0_imm16 = 0xD280_0000_u32 | ((imm16 as u32) << 5);
    push_u32_le(&mut bytes, mov_x0_imm16);
    push_u32_le(&mut bytes, 0xD65F_03C0);
    MacosArm64ExecProbeCode { bytes }
}

pub fn encode_macos_arm64_add2_probe() -> MacosArm64ExecProbeCode {
    let mut bytes = Vec::with_capacity(8);
    push_u32_le(&mut bytes, 0x8B01_0000);
    push_u32_le(&mut bytes, 0xD65F_03C0);
    MacosArm64ExecProbeCode { bytes }
}

pub fn encode_macos_arm64_indirect_blr_probe(
    fallback: extern "C" fn(i64, i64) -> i64,
) -> MacosArm64IndirectProbeCode {
    let mut bytes = Vec::with_capacity(24);
    push_u32_le(&mut bytes, 0x5800_0090);
    push_u32_le(&mut bytes, 0xD63F_0200);
    push_u32_le(&mut bytes, 0xD65F_03C0);
    push_u32_le(&mut bytes, 0xD503_201F);
    let fallback_literal_offset = bytes.len();
    push_u64_le(&mut bytes, fallback as *const u8 as u64);
    MacosArm64IndirectProbeCode {
        bytes,
        fallback_literal_offset,
    }
}

pub fn encode_macos_arm64_framed_indirect_blr_probe(
    fallback: extern "C" fn(i64, i64) -> i64,
) -> MacosArm64IndirectProbeCode {
    let mut bytes = Vec::with_capacity(40);
    push_u32_le(&mut bytes, 0xA9BF_7BFD);
    push_u32_le(&mut bytes, 0x9100_03FD);
    push_u32_le(&mut bytes, 0x5800_00D0);
    push_u32_le(&mut bytes, 0xD63F_0200);
    push_u32_le(&mut bytes, 0xA8C1_7BFD);
    push_u32_le(&mut bytes, 0xD65F_03C0);
    push_u32_le(&mut bytes, 0xD503_201F);
    push_u32_le(&mut bytes, 0xD503_201F);
    let fallback_literal_offset = bytes.len();
    push_u64_le(&mut bytes, fallback as *const u8 as u64);
    MacosArm64IndirectProbeCode {
        bytes,
        fallback_literal_offset,
    }
}

pub fn install_macos_arm64_exec_probe(
    code: &MacosArm64ExecProbeCode,
) -> Result<MacosArm64InstalledProbe, RawPackedArrayReadThunkUnavailable> {
    install_macos_arm64_exec_probe_bytes(&code.bytes)
}

pub fn install_macos_arm64_indirect_probe(
    code: &MacosArm64IndirectProbeCode,
) -> Result<MacosArm64InstalledProbe, RawPackedArrayReadThunkUnavailable> {
    install_macos_arm64_exec_probe_bytes(&code.bytes)
}

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
fn install_macos_arm64_exec_probe_bytes(
    bytes: &[u8],
) -> Result<MacosArm64InstalledProbe, RawPackedArrayReadThunkUnavailable> {
    use std::ffi::c_void;

    const PROT_READ: i32 = 0x1;
    const PROT_WRITE: i32 = 0x2;
    const PROT_EXEC: i32 = 0x4;
    const MAP_PRIVATE: i32 = 0x0002;
    const MAP_ANON: i32 = 0x1000;
    const MAP_FAILED: *mut c_void = !0usize as *mut c_void;

    extern "C" {
        fn mmap(
            addr: *mut c_void,
            len: usize,
            prot: i32,
            flags: i32,
            fd: i32,
            offset: isize,
        ) -> *mut c_void;
        fn mprotect(addr: *mut c_void, len: usize, prot: i32) -> i32;
        fn sys_icache_invalidate(start: *mut c_void, len: usize);
    }

    if bytes.is_empty() {
        return Err(RawPackedArrayReadThunkUnavailable::EmissionDeferred);
    }

    let ptr = unsafe {
        mmap(
            std::ptr::null_mut(),
            bytes.len(),
            PROT_READ | PROT_WRITE,
            MAP_PRIVATE | MAP_ANON,
            -1,
            0,
        )
    };
    if ptr == MAP_FAILED || ptr.is_null() {
        return Err(RawPackedArrayReadThunkUnavailable::EmissionDeferred);
    }

    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr as *mut u8, bytes.len());
        if mprotect(ptr, bytes.len(), PROT_READ | PROT_EXEC) != 0 {
            return Err(RawPackedArrayReadThunkUnavailable::EmissionDeferred);
        }
        sys_icache_invalidate(ptr, bytes.len());
    }

    Ok(MacosArm64InstalledProbe {
        ptr: ptr as *const u8,
        len: bytes.len(),
    })
}

#[cfg(not(all(target_arch = "aarch64", target_os = "macos")))]
fn install_macos_arm64_exec_probe_bytes(
    _bytes: &[u8],
) -> Result<MacosArm64InstalledProbe, RawPackedArrayReadThunkUnavailable> {
    Err(RawPackedArrayReadThunkUnavailable::UnsupportedArchitecture)
}

fn push_u32_le(out: &mut Vec<u8>, word: u32) {
    out.extend_from_slice(&word.to_le_bytes());
}

fn push_u64_le(out: &mut Vec<u8>, word: u64) {
    out.extend_from_slice(&word.to_le_bytes());
}

fn u16_imm(value: i32) -> Result<u16, RawPackedArrayReadThunkUnavailable> {
    u16::try_from(value).map_err(|_| RawPackedArrayReadThunkUnavailable::EmissionDeferred)
}

fn u12_imm(value: i32) -> Result<u16, RawPackedArrayReadThunkUnavailable> {
    let value =
        u16::try_from(value).map_err(|_| RawPackedArrayReadThunkUnavailable::EmissionDeferred)?;
    if value <= 4095 {
        Ok(value)
    } else {
        Err(RawPackedArrayReadThunkUnavailable::EmissionDeferred)
    }
}

fn u12_scaled_8(value: i32) -> Result<u16, RawPackedArrayReadThunkUnavailable> {
    let value = u12_imm(value)?;
    if value % 8 == 0 {
        Ok(value)
    } else {
        Err(RawPackedArrayReadThunkUnavailable::EmissionDeferred)
    }
}

pub fn build_packed_array_read_raw_thunk(
    layout: &crate::deopt::InlineIcLayout,
) -> Result<RawPackedArrayReadThunk, RawPackedArrayReadThunkUnavailable> {
    let plan = build_packed_array_read_raw_thunk_plan(layout)?;
    let code = encode_packed_array_read_raw_fast_thunk(&plan)?;
    install_packed_array_read_raw_thunk(&code)
}

pub fn install_packed_array_read_raw_thunk(
    code: &RawPackedArrayReadThunkCode,
) -> Result<RawPackedArrayReadThunk, RawPackedArrayReadThunkUnavailable> {
    let installed = install_macos_arm64_exec_probe_bytes(&code.bytes)?;
    Ok(RawPackedArrayReadThunk {
        ptr: installed.ptr(),
        len: installed.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    extern "C" fn maec_ext4_fallback_add(a: i64, b: i64) -> i64 {
        a + b
    }

    extern "C" fn rpar_ext11_probe_target(receiver: i64, index: i64) -> f64 {
        (receiver + index) as f64 + 0.5
    }

    fn make_shape() -> std::rc::Rc<rusty_js_shapes::Shape> {
        rusty_js_shapes::Shape::root().transition_to("x")
    }

    fn write_u64_at(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn cold_entry_starts_null() {
        let e = ICEntry::new_cold();
        assert_eq!(e.state(), ICState::Cold);
        assert!(e.cached_shape.is_null());
        assert_eq!(e.cached_slot, 0);
        assert_eq!(e.miss_count, 0);
        assert!(!e.degraded);
    }

    #[test]
    fn cold_to_warm_on_first_observe() {
        let mut e = ICEntry::new_cold();
        let s = make_shape();
        e.observe(s.clone(), 0);
        assert_eq!(e.state(), ICState::WarmMono);
        assert_eq!(e.cached_shape, std::rc::Rc::as_ptr(&s));
        assert_eq!(e.cached_slot, 0);
        assert_eq!(e.miss_count, 0);
    }

    #[test]
    fn warm_to_cold_after_miss_on_shape_change() {
        let mut e = ICEntry::new_cold();
        let s1 = rusty_js_shapes::Shape::root().transition_to("x");
        let s2 = rusty_js_shapes::Shape::root().transition_to("y");
        assert!(!std::rc::Rc::ptr_eq(&s1, &s2));
        e.observe(s1.clone(), 0);
        assert_eq!(e.state(), ICState::WarmMono);
        e.observe(s2.clone(), 0);

        assert_eq!(e.cached_shape, std::rc::Rc::as_ptr(&s2));
        assert_eq!(e.miss_count, 1);
        assert_eq!(e.state(), ICState::ColdAfterMiss);
    }

    #[test]
    fn degrades_past_miss_threshold() {
        let mut e = ICEntry::new_cold();
        let s0 = make_shape();
        e.observe(s0, 0);

        for i in 1..=(MISS_THRESHOLD + 1) {
            let s = rusty_js_shapes::Shape::root().transition_to(&format!("p{}", i));
            e.observe(s, 0);
            if i <= MISS_THRESHOLD {
                assert!(
                    !e.degraded,
                    "should not degrade until miss_count > {}",
                    MISS_THRESHOLD
                );
            }
        }
        assert!(e.degraded, "should degrade past MISS_THRESHOLD");
        assert_eq!(e.state(), ICState::Degraded);
        assert!(e.cached_shape.is_null(), "degraded entry clears its cache");
        assert!(e.pinned_shape_holder.is_none());
    }

    #[test]
    fn degraded_entry_stops_observing() {
        let mut e = ICEntry::new_cold();
        e.degraded = true;
        let s = make_shape();
        let pre_count = e.miss_count;
        e.observe(s, 0);
        assert!(e.degraded);
        assert!(e.cached_shape.is_null());
        assert_eq!(e.miss_count, pre_count);
    }

    #[test]
    fn observe_miss_no_shape_increments_count() {
        let mut e = ICEntry::new_cold();
        let s = make_shape();
        e.observe(s, 0);
        let initial = e.miss_count;
        e.observe_miss_no_shape();
        assert_eq!(e.miss_count, initial + 1);
        assert_eq!(e.state(), ICState::ColdAfterMiss);
    }

    #[test]
    fn observe_miss_no_shape_on_cold_is_noop() {
        let mut e = ICEntry::new_cold();
        e.observe_miss_no_shape();
        assert_eq!(e.state(), ICState::Cold);
        assert_eq!(e.miss_count, 0);
    }

    #[test]
    fn icstubcache_alloc_assigns_sequential_ids() {
        let mut c = ICStubCache::new();
        assert_eq!(c.alloc_site(), 0);
        assert_eq!(c.alloc_site(), 1);
        assert_eq!(c.alloc_site(), 2);
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn icstubcache_histogram_classifies_state() {
        let mut c = ICStubCache::new();
        let s = make_shape();
        let id0 = c.alloc_site();
        let id1 = c.alloc_site();
        let id2 = c.alloc_site();
        let id3 = c.alloc_site();
        c.entry_mut(id1).observe(s.clone(), 0);
        c.entry_mut(id2).observe(s.clone(), 0);
        c.entry_mut(id2)
            .observe(rusty_js_shapes::Shape::root().transition_to("y"), 0);
        c.entry_mut(id3).degraded = true;
        let (cold, warm, cam, deg) = c.state_histogram();
        assert_eq!((cold, warm, cam, deg), (1, 1, 1, 1));
        let _ = id0;
    }

    #[test]
    fn stub_pattern_cache_hit_returns_slot_value() {
        let f = build_stub_pattern_module().expect("build module");

        let synthetic_shape = 0xDEAD_BEEF_i64;

        let values: Vec<i64> = vec![10, 20, 30, 40, 50];
        let base = values.as_ptr() as i64;

        let result = f(synthetic_shape, synthetic_shape, 2, base,   999);
        assert_eq!(result, 30, "cache hit should load values_base[slot*8]");

        let result = f(synthetic_shape, synthetic_shape, 0, base,   999);
        assert_eq!(result, 10);

        drop(values);
    }

    #[test]
    fn stub_pattern_cache_miss_returns_slow_path() {
        let f = build_stub_pattern_module().expect("build module");
        let recv_shape = 0xCAFE_BABE_i64;
        let cached_shape = 0xDEAD_BEEF_i64;
        let values: Vec<i64> = vec![10, 20, 30];
        let base = values.as_ptr() as i64;
        let slow_path = 0x12345_i64;

        let result = f(recv_shape, cached_shape, 0, base, slow_path);
        assert_eq!(result, slow_path);
        drop(values);
    }

    #[test]
    fn doc738_convention_smoke_test() {

        let _: ICSiteId = 0;
        let _: ICState = ICState::Cold;
        let _: ICStubCache = ICStubCache::new();
        let _ = MISS_THRESHOLD;

    }

    #[test]
    fn rpar_ext2_raw_thunk_rejects_unverified_layout() {
        let err = build_packed_array_read_raw_thunk(&crate::deopt::InlineIcLayout::default())
            .expect_err("default layout is not verified for raw packed reads");
        let expected = if raw_packed_array_read_supported() {
            RawPackedArrayReadThunkUnavailable::UnverifiedLayout
        } else {
            RawPackedArrayReadThunkUnavailable::UnsupportedArchitecture
        };
        assert_eq!(err, expected);
    }

    #[test]
    fn rpar_ext2_raw_thunk_descriptor_holds_pointer_and_len() {
        static BYTES: [u8; 4] = [0, 1, 2, 3];
        let thunk = RawPackedArrayReadThunk::synthetic_for_test(BYTES.as_ptr(), BYTES.len());
        assert_eq!(thunk.ptr(), BYTES.as_ptr());
        assert_eq!(thunk.len(), BYTES.len());
        assert!(!thunk.is_empty());
    }

    #[test]
    fn rpar_ext3_raw_thunk_plan_freezes_verified_layout() {
        if !raw_packed_array_read_supported() {
            return;
        }
        let layout = crate::deopt::InlineIcLayout {
            slot_stride: 64,
            slot_payload_off: 8,
            object_shape_off: 0,
            object_shape_values_off: 0,
            vec_ptr_off: 0,
            value_size: 16,
            value_number_payload_off: 8,
            value_payload_off: 8,
            value_tag_number: 1,
            value_tag_string: 4,
            value_tag_object: 7,
            verified: true,
            object_array_dense_off: 16,
            object_dense_elements_off: 24,
            vec_len_off: 8,
            vec_cap_off: 16,
            array_verified: true,
            object_array_packed_all_safe_i64_off: 31,
            object_array_packed_off: 32,
            object_dense_doubles_off: 40,
            object_dense_i64_sidecar_valid_off: 33,
        };
        let plan = build_packed_array_read_raw_thunk_plan(&layout).expect("verified plan");
        assert_eq!(plan.slot_stride, 64);
        assert_eq!(plan.slot_payload_off, 8);
        assert_eq!(plan.object_array_packed_off, 32);
        assert_eq!(plan.object_dense_doubles_off, 40);
        assert_eq!(plan.vec_len_off, 8);
        assert_eq!(plan.vec_ptr_off, 0);
        assert_eq!(
            plan.fallback,
            crate::deopt::jit_getindex_on_object as *const u8
        );
    }

    #[test]
    fn rpar_ext4_slow_thunk_encoder_emits_fallback_literal_template() {
        if !raw_packed_array_read_supported() {
            return;
        }
        let plan = RawPackedArrayReadThunkPlan {
            slot_stride: 64,
            slot_payload_off: 8,
            object_array_packed_off: 32,
            object_dense_doubles_off: 40,
            vec_len_off: 8,
            vec_ptr_off: 0,
            fallback: crate::deopt::jit_getindex_on_object as *const u8,
        };
        let code = encode_packed_array_read_raw_slow_thunk(&plan).expect("slow thunk bytes");
        assert_eq!(code.bytes.len(), 40);
        assert_eq!(code.heap_base_literal_offset, None);
        assert_eq!(code.fallback_literal_offset, 32);
        assert_eq!(
            &code.bytes[..32],
            &[
                0xFD, 0x7B, 0xBF, 0xA9,
                0xFD, 0x03, 0x00, 0x91,
                0xD0, 0x00, 0x00, 0x58,
                0x00, 0x02, 0x3F, 0xD6,
                0xFD, 0x7B, 0xC1, 0xA8,
                0x00, 0x00, 0x62, 0x9E,
                0xC0, 0x03, 0x5F, 0xD6,
                0x1F, 0x20, 0x03, 0xD5,
            ]
        );
        let mut literal = [0u8; 8];
        literal.copy_from_slice(&code.bytes[code.fallback_literal_offset..]);
        assert_eq!(u64::from_le_bytes(literal), plan.fallback as u64);
    }

    #[test]
    fn rpar_ext8_fast_thunk_encoder_emits_packed_guard_template() {
        if !raw_packed_array_read_supported() {
            return;
        }
        let plan = RawPackedArrayReadThunkPlan {
            slot_stride: 64,
            slot_payload_off: 8,
            object_array_packed_off: 32,
            object_dense_doubles_off: 40,
            vec_len_off: 8,
            vec_ptr_off: 0,
            fallback: crate::deopt::jit_getindex_on_object as *const u8,
        };
        let code = encode_packed_array_read_raw_fast_thunk(&plan).expect("fast thunk bytes");
        assert_eq!(code.bytes.len(), 108);
        assert_eq!(code.heap_base_literal_offset, Some(92));
        assert_eq!(code.fallback_literal_offset, 100);
        assert_eq!(
            &code.bytes[..92],
            &[
                0x09, 0x54, 0x40, 0xD3,
                0x0B, 0x08, 0x80, 0xD2,
                0x29, 0x7D, 0x0B, 0x9B,
                0x8A, 0x02, 0x00, 0x58,
                0x4A, 0x01, 0x40, 0xF9,
                0x49, 0x01, 0x09, 0x8B,
                0x29, 0x21, 0x00, 0x91,
                0x2C, 0x81, 0x40, 0x39,
                0xEC, 0x00, 0x00, 0x34,
                0x2D, 0x19, 0x40, 0xF9,
                0x3F, 0x00, 0x0D, 0xEB,
                0x82, 0x00, 0x00, 0x54,
                0x2E, 0x15, 0x40, 0xF9,
                0xC0, 0x79, 0x61, 0xFC,
                0xC0, 0x03, 0x5F, 0xD6,
                0xFD, 0x7B, 0xBF, 0xA9,
                0xFD, 0x03, 0x00, 0x91,
                0x10, 0x01, 0x00, 0x58,
                0x00, 0x02, 0x3F, 0xD6,
                0xFD, 0x7B, 0xC1, 0xA8,
                0x00, 0x00, 0x62, 0x9E,
                0xC0, 0x03, 0x5F, 0xD6,
                0x1F, 0x20, 0x03, 0xD5,
            ]
        );
        let mut heap_literal = [0u8; 8];
        heap_literal.copy_from_slice(&code.bytes[code.heap_base_literal_offset.unwrap()..100]);
        assert_eq!(
            u64::from_le_bytes(heap_literal),
            crate::deopt::inline_ic_heap_base_addr() as u64
        );
        let mut fallback_literal = [0u8; 8];
        fallback_literal.copy_from_slice(&code.bytes[code.fallback_literal_offset..]);
        assert_eq!(u64::from_le_bytes(fallback_literal), plan.fallback as u64);
    }

    #[test]
    fn rpar_ext8_installed_fast_thunk_parent_timeout_smoke() {
        if !cfg!(all(target_arch = "aarch64", target_os = "macos")) {
            return;
        }
        if std::env::var("RPAR_EXT8_CHILD").ok().as_deref() == Some("1") {
            return;
        }
        let exe = std::env::current_exe().expect("current test executable");
        let mut child = std::process::Command::new(exe)
            .arg("rpar_ext8_installed_fast_thunk_child")
            .arg("--exact")
            .arg("--nocapture")
            .env("RPAR_EXT8_CHILD", "1")
            .spawn()
            .expect("spawn RPAR EXT 8 child test");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Some(status) = child.try_wait().expect("poll RPAR EXT 8 child") {
                assert!(status.success(), "RPAR EXT 8 child failed with {status}");
                return;
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("RPAR EXT 8 installed fast thunk smoke timed out");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn rpar_ext8_installed_fast_thunk_child() {
        if std::env::var("RPAR_EXT8_CHILD").ok().as_deref() != Some("1") {
            return;
        }
        let layout = crate::deopt::InlineIcLayout {
            slot_stride: 64,
            slot_payload_off: 8,
            object_shape_off: 0,
            object_shape_values_off: 0,
            vec_ptr_off: 0,
            value_size: 16,
            value_number_payload_off: 8,
            value_payload_off: 8,
            value_tag_number: 1,
            value_tag_string: 4,
            value_tag_object: 7,
            verified: true,
            object_array_dense_off: 16,
            object_dense_elements_off: 24,
            vec_len_off: 8,
            vec_cap_off: 16,
            array_verified: true,
            object_array_packed_all_safe_i64_off: 31,
            object_array_packed_off: 32,
            object_dense_doubles_off: 40,
            object_dense_i64_sidecar_valid_off: 33,
        };
        let values = [10.5_f64, 20.25, 30.75];
        let mut heap = vec![0_u8; 64 * 2];
        let receiver_id = 1_usize;
        let obj_addr = receiver_id * layout.slot_stride as usize + layout.slot_payload_off as usize;
        heap[obj_addr + layout.object_array_packed_off as usize] = 1;
        write_u64_at(
            &mut heap,
            obj_addr + layout.object_dense_doubles_off as usize + layout.vec_ptr_off as usize,
            values.as_ptr() as u64,
        );
        write_u64_at(
            &mut heap,
            obj_addr + layout.object_dense_doubles_off as usize + layout.vec_len_off as usize,
            values.len() as u64,
        );
        crate::deopt::set_inline_ic_heap_base(heap.as_ptr() as usize);

        let thunk = build_packed_array_read_raw_thunk(&layout).expect("installed raw fast thunk");
        assert!(!thunk.is_empty());
        let f: extern "C" fn(i64, i64) -> f64 = unsafe { std::mem::transmute(thunk.ptr()) };
        assert_eq!(f(receiver_id as i64, 0), 10.5);
        assert_eq!(f(receiver_id as i64, 2), 30.75);
        assert_eq!(
            f(receiver_id as i64, 3),
            (((receiver_id as i64) << 8) ^ 3) as f64
        );

        heap[obj_addr + layout.object_array_packed_off as usize] = 0;
        assert_eq!(
            f(receiver_id as i64, 1),
            (((receiver_id as i64) << 8) ^ 1) as f64
        );
    }

    #[test]
    fn rpar_ext11_raw_probe_wrapper_counts_and_delegates() {
        let thunk = RawPackedArrayReadThunk::synthetic_for_test(
            rpar_ext11_probe_target as *const u8,
            std::mem::size_of::<usize>(),
        );
        install_raw_packed_read_probe_target(thunk);
        assert_eq!(raw_packed_read_probe_call_count(), 0);
        assert_eq!(jit_packed_read_raw_probe_thunk(7, 3), 10.5);
        assert_eq!(raw_packed_read_probe_call_count(), 1);
        assert_eq!(jit_packed_read_raw_probe_thunk(11, 4), 15.5);
        assert_eq!(raw_packed_read_probe_call_count(), 2);
    }

    #[test]
    fn rpar_ext7_installed_framed_slow_thunk_parent_timeout_smoke() {
        if !cfg!(all(target_arch = "aarch64", target_os = "macos")) {
            return;
        }
        if std::env::var("RPAR_EXT7_CHILD").ok().as_deref() == Some("1") {
            return;
        }
        let exe = std::env::current_exe().expect("current test executable");
        let mut child = std::process::Command::new(exe)
            .arg("rpar_ext7_installed_framed_slow_thunk_child")
            .arg("--exact")
            .arg("--nocapture")
            .env("RPAR_EXT7_CHILD", "1")
            .spawn()
            .expect("spawn RPAR EXT 7 child test");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Some(status) = child.try_wait().expect("poll RPAR EXT 7 child") {
                assert!(status.success(), "RPAR EXT 7 child failed with {status}");
                return;
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("RPAR EXT 7 installed slow thunk smoke timed out");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn rpar_ext7_installed_framed_slow_thunk_child() {
        if std::env::var("RPAR_EXT7_CHILD").ok().as_deref() != Some("1") {
            return;
        }
        let layout = crate::deopt::InlineIcLayout {
            slot_stride: 64,
            slot_payload_off: 8,
            object_shape_off: 0,
            object_shape_values_off: 0,
            vec_ptr_off: 0,
            value_size: 16,
            value_number_payload_off: 8,
            value_payload_off: 8,
            value_tag_number: 1,
            value_tag_string: 4,
            value_tag_object: 7,
            verified: true,
            object_array_dense_off: 16,
            object_dense_elements_off: 24,
            vec_len_off: 8,
            vec_cap_off: 16,
            array_verified: true,
            object_array_packed_all_safe_i64_off: 31,
            object_array_packed_off: 32,
            object_dense_doubles_off: 40,
            object_dense_i64_sidecar_valid_off: 33,
        };
        let plan = build_packed_array_read_raw_thunk_plan(&layout).expect("raw slow plan");
        let code = encode_packed_array_read_raw_slow_thunk(&plan).expect("raw slow code");
        let thunk = install_packed_array_read_raw_thunk(&code).expect("installed raw slow thunk");
        assert!(!thunk.is_empty());
        let f: extern "C" fn(i64, i64) -> f64 = unsafe { std::mem::transmute(thunk.ptr()) };
        assert_eq!(f(7, 3), ((7_i64 << 8) ^ 3) as f64);
    }

    #[test]
    fn maec_ext1_const_return_probe_emits_assembler_checked_bytes() {
        let code = encode_macos_arm64_const_return_probe(42);
        assert_eq!(code.bytes.len(), 8);
        assert_eq!(
            code.bytes,
            vec![
                0x40, 0x05, 0x80, 0xD2,
                0xC0, 0x03, 0x5F, 0xD6,
            ]
        );
    }

    #[test]
    fn maec_ext2_const_return_installed_parent_timeout_smoke() {
        if !raw_packed_array_read_supported() {
            return;
        }

        let exe = std::env::current_exe().expect("current test exe");
        let mut child = std::process::Command::new(exe)
            .arg("--exact")
            .arg("stub_raw_aarch64::tests::maec_ext2_const_return_installed_child")
            .arg("--nocapture")
            .env("MAEC_EXT2_CHILD", "1")
            .spawn()
            .expect("spawn MAEC child test");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Some(status) = child.try_wait().expect("poll MAEC child") {
                assert!(status.success(), "MAEC child failed with {status}");
                return;
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("MAEC child executable-call smoke timed out");
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    }

    #[test]
    fn maec_ext2_const_return_installed_child() {
        if std::env::var("MAEC_EXT2_CHILD").ok().as_deref() != Some("1") {
            return;
        }
        let code = encode_macos_arm64_const_return_probe(42);
        let installed = install_macos_arm64_exec_probe(&code).expect("install constant probe");
        assert_eq!(installed.len(), 8);
        let f: extern "C" fn() -> i64 = unsafe { std::mem::transmute(installed.ptr()) };
        assert_eq!(f(), 42);
    }

    #[test]
    fn maec_ext3_add2_probe_emits_assembler_checked_bytes() {
        let code = encode_macos_arm64_add2_probe();
        assert_eq!(code.bytes.len(), 8);
        assert_eq!(
            code.bytes,
            vec![
                0x00, 0x00, 0x01, 0x8B,
                0xC0, 0x03, 0x5F, 0xD6,
            ]
        );
    }

    #[test]
    fn maec_ext3_add2_installed_parent_timeout_smoke() {
        if !raw_packed_array_read_supported() {
            return;
        }

        let exe = std::env::current_exe().expect("current test exe");
        let mut child = std::process::Command::new(exe)
            .arg("--exact")
            .arg("stub_raw_aarch64::tests::maec_ext3_add2_installed_child")
            .arg("--nocapture")
            .env("MAEC_EXT3_CHILD", "1")
            .spawn()
            .expect("spawn MAEC EXT 3 child test");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Some(status) = child.try_wait().expect("poll MAEC EXT 3 child") {
                assert!(status.success(), "MAEC EXT 3 child failed with {status}");
                return;
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("MAEC EXT 3 child executable-call smoke timed out");
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    }

    #[test]
    fn maec_ext3_add2_installed_child() {
        if std::env::var("MAEC_EXT3_CHILD").ok().as_deref() != Some("1") {
            return;
        }
        let code = encode_macos_arm64_add2_probe();
        let installed = install_macos_arm64_exec_probe(&code).expect("install add2 probe");
        assert_eq!(installed.len(), 8);
        let f: extern "C" fn(i64, i64) -> i64 = unsafe { std::mem::transmute(installed.ptr()) };
        assert_eq!(f(17, 25), 42);
    }

    #[test]
    fn maec_ext4_indirect_blr_probe_emits_assembler_checked_bytes() {
        let code = encode_macos_arm64_indirect_blr_probe(maec_ext4_fallback_add);
        assert_eq!(code.bytes.len(), 24);
        assert_eq!(code.fallback_literal_offset, 16);
        assert_eq!(
            &code.bytes[..16],
            &[
                0x90, 0x00, 0x00, 0x58,
                0x00, 0x02, 0x3F, 0xD6,
                0xC0, 0x03, 0x5F, 0xD6,
                0x1F, 0x20, 0x03, 0xD5,
            ]
        );
        let mut literal = [0u8; 8];
        literal.copy_from_slice(&code.bytes[code.fallback_literal_offset..]);
        assert_eq!(
            u64::from_le_bytes(literal),
            maec_ext4_fallback_add as *const u8 as u64
        );
    }

    #[test]
    fn maec_ext4_indirect_blr_parent_timeout_smoke() {
        if !raw_packed_array_read_supported() {
            return;
        }

        let exe = std::env::current_exe().expect("current test exe");
        let mut child = std::process::Command::new(exe)
            .arg("--exact")
            .arg("stub_raw_aarch64::tests::maec_ext4_indirect_blr_child")
            .arg("--nocapture")
            .env("MAEC_EXT4_CHILD", "1")
            .spawn()
            .expect("spawn MAEC EXT 4 child test");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Some(status) = child.try_wait().expect("poll MAEC EXT 4 child") {
                panic!("MAEC EXT 4 indirect blr unexpectedly exited with {status}");
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    }

    #[test]
    fn maec_ext4_indirect_blr_child() {
        if std::env::var("MAEC_EXT4_CHILD").ok().as_deref() != Some("1") {
            return;
        }
        let code = encode_macos_arm64_indirect_blr_probe(maec_ext4_fallback_add);
        let installed =
            install_macos_arm64_indirect_probe(&code).expect("install indirect blr probe");
        assert_eq!(installed.len(), 24);
        let f: extern "C" fn(i64, i64) -> i64 = unsafe { std::mem::transmute(installed.ptr()) };
        assert_eq!(f(17, 25), 42);
    }

    #[test]
    fn maec_ext5_framed_indirect_blr_probe_emits_assembler_checked_bytes() {
        let code = encode_macos_arm64_framed_indirect_blr_probe(maec_ext4_fallback_add);
        assert_eq!(code.bytes.len(), 40);
        assert_eq!(code.fallback_literal_offset, 32);
        assert_eq!(
            &code.bytes[..32],
            &[
                0xFD, 0x7B, 0xBF, 0xA9,
                0xFD, 0x03, 0x00, 0x91,
                0xD0, 0x00, 0x00, 0x58,
                0x00, 0x02, 0x3F, 0xD6,
                0xFD, 0x7B, 0xC1, 0xA8,
                0xC0, 0x03, 0x5F, 0xD6,
                0x1F, 0x20, 0x03, 0xD5,
                0x1F, 0x20, 0x03, 0xD5,
            ]
        );
        let mut literal = [0u8; 8];
        literal.copy_from_slice(&code.bytes[code.fallback_literal_offset..]);
        assert_eq!(
            u64::from_le_bytes(literal),
            maec_ext4_fallback_add as *const u8 as u64
        );
    }

    #[test]
    fn maec_ext5_framed_indirect_blr_parent_timeout_smoke() {
        if !raw_packed_array_read_supported() {
            return;
        }

        let exe = std::env::current_exe().expect("current test exe");
        let mut child = std::process::Command::new(exe)
            .arg("--exact")
            .arg("stub_raw_aarch64::tests::maec_ext5_framed_indirect_blr_child")
            .arg("--nocapture")
            .env("MAEC_EXT5_CHILD", "1")
            .spawn()
            .expect("spawn MAEC EXT 5 child test");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Some(status) = child.try_wait().expect("poll MAEC EXT 5 child") {
                assert!(status.success(), "MAEC EXT 5 child failed with {status}");
                return;
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("MAEC EXT 5 child executable-call smoke timed out");
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    }

    #[test]
    fn maec_ext5_framed_indirect_blr_child() {
        if std::env::var("MAEC_EXT5_CHILD").ok().as_deref() != Some("1") {
            return;
        }
        let code = encode_macos_arm64_framed_indirect_blr_probe(maec_ext4_fallback_add);
        let installed =
            install_macos_arm64_indirect_probe(&code).expect("install framed indirect blr probe");
        assert_eq!(installed.len(), 40);
        let f: extern "C" fn(i64, i64) -> i64 = unsafe { std::mem::transmute(installed.ptr()) };
        assert_eq!(f(17, 25), 42);
    }
}
