
const OBJECT_ID_INDEX_BITS: u32 = 25;
const OBJECT_ID_INDEX_MASK: u32 = (1 << OBJECT_ID_INDEX_BITS) - 1;

const OOM_SLOT_MARGIN: u32 = 65_536;
const OBJECT_ID_MAX_GENERATION: u16 = (u32::MAX >> OBJECT_ID_INDEX_BITS) as u16;
const SOFT_TARGET_PRESSURE_ALLOC_QUANTUM: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectId(pub u32);

impl ObjectId {
    pub fn new(index: u32, generation: u16) -> Self {
        assert!(
            index <= OBJECT_ID_INDEX_MASK,
            "ObjectId slot index exceeded packed handle capacity"
        );
        Self(((generation as u32) << OBJECT_ID_INDEX_BITS) | index)
    }

    pub fn slot_index(self) -> usize {
        (self.0 & OBJECT_ID_INDEX_MASK) as usize
    }

    pub fn slot_index_u32(self) -> u32 {
        self.0 & OBJECT_ID_INDEX_MASK
    }

    pub fn generation(self) -> u16 {
        (self.0 >> OBJECT_ID_INDEX_BITS) as u16
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    White,
    Gray,
    Black,
}

pub trait Trace {
    fn trace(&self, ids: &mut Vec<ObjectId>);

    fn trace_slice(&self, start: usize, budget: usize, ids: &mut Vec<ObjectId>) -> TraceSlice {
        if start == 0 && budget > 0 {
            self.trace(ids);
            TraceSlice {
                next_index: 0,
                complete: true,
            }
        } else {
            TraceSlice {
                next_index: start,
                complete: true,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceSlice {
    pub next_index: usize,
    pub complete: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RealmSweepState {
    pub next_index: usize,
    pub freed: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RealmMarkCursor {
    pub object: Option<(ObjectId, usize)>,
    pub next_index: usize,
}

impl Default for RealmMarkCursor {
    fn default() -> Self {
        Self {
            object: None,
            next_index: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalHandle(pub u64);

#[derive(Debug)]
pub enum Slot<T> {
    Object(T),

    External(ExternalHandle),
    Free,
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Default)]
struct DarwinTimeValue {
    seconds: i32,
    microseconds: i32,
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Default)]
struct DarwinMachTaskBasicInfo {
    virtual_size: u64,
    resident_size: u64,
    resident_size_max: u64,
    user_time: DarwinTimeValue,
    system_time: DarwinTimeValue,
    policy: i32,
    suspend_count: i32,
}

#[cfg(target_os = "macos")]
fn darwin_rss_bytes() -> usize {
    type KernReturn = i32;
    type MachPort = u32;
    type MachMsgTypeNumber = u32;
    type Integer = i32;

    const KERN_SUCCESS: KernReturn = 0;
    const MACH_TASK_BASIC_INFO: i32 = 20;

    extern "C" {
        static mach_task_self_: MachPort;
        fn task_info(
            target_task: MachPort,
            flavor: i32,
            task_info_out: *mut Integer,
            task_info_out_count: *mut MachMsgTypeNumber,
        ) -> KernReturn;
    }

    let mut info = DarwinMachTaskBasicInfo::default();
    let mut count = (std::mem::size_of::<DarwinMachTaskBasicInfo>()
        / std::mem::size_of::<Integer>()) as MachMsgTypeNumber;
    let result = unsafe {
        task_info(
            mach_task_self_,
            MACH_TASK_BASIC_INFO,
            &mut info as *mut DarwinMachTaskBasicInfo as *mut Integer,
            &mut count,
        )
    };
    if result == KERN_SUCCESS {
        info.resident_size as usize
    } else {
        0
    }
}

pub struct Heap<T: Trace> {
    slots: Vec<Slot<T>>,
    colors: Vec<Color>,
    owners: Vec<usize>,
    generations: Vec<u16>,
    free_list: Vec<u32>,

    no_slot_reuse: bool,

    alloc_count: usize,

    threshold: usize,

    growth_permille: usize,

    soft_target_bytes: usize,

    freed_external: Vec<ExternalHandle>,

    external_bytes_since_collect: usize,

    external_byte_threshold: usize,

    oom: bool,

    oom_sentinel: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrossOwnerEdge {
    pub from: ObjectId,
    pub from_owner: usize,
    pub to: ObjectId,
    pub to_owner: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossOwnerEdgesError {
    pub edges: Vec<CrossOwnerEdge>,
}

impl<T: Trace> Default for Heap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Trace> Heap<T> {
    pub fn new() -> Self {
        Self::with_slot_reuse(std::env::var("CRUFT_GC_NO_SLOT_REUSE").is_err())
    }

    pub fn new_with_slot_reuse_for_test() -> Self {
        Self::with_slot_reuse(true)
    }

    fn with_slot_reuse(reuse_slots: bool) -> Self {

        let soft_target_bytes = Self::soft_target_from_env();
        let mut external_byte_threshold = Self::external_byte_threshold_from_env();

        if soft_target_bytes > 0 {
            external_byte_threshold =
                external_byte_threshold.min((soft_target_bytes / 2).max(1 << 20));
        }
        Self {
            slots: Vec::new(),
            colors: Vec::new(),
            owners: Vec::new(),
            generations: Vec::new(),
            free_list: Vec::new(),
            no_slot_reuse: !reuse_slots,
            alloc_count: 0,
            threshold: 1024,
            growth_permille: Self::growth_permille_from_env(),
            soft_target_bytes,
            freed_external: Vec::new(),
            external_bytes_since_collect: 0,
            external_byte_threshold,
            oom: false,
            oom_sentinel: None,
        }
    }

    fn external_byte_threshold_from_env() -> usize {
        match std::env::var("CRUFT_GC_EXTERNAL_MB") {
            Ok(s) => s
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|f| f.is_finite() && *f > 0.0)
                .map(|f| (f * 1_048_576.0) as usize)
                .unwrap_or(64 << 20),
            Err(_) => 64 << 20,
        }
    }

    pub fn note_external_alloc(&mut self, bytes: usize) {
        self.external_bytes_since_collect = self.external_bytes_since_collect.saturating_add(bytes);
    }

    fn soft_target_from_env() -> usize {
        match std::env::var("CRUFT_GC_TARGET_MB") {
            Ok(s) => s
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|f| f.is_finite() && *f > 0.0)
                .map(|mb| (mb * 1024.0 * 1024.0) as usize)
                .unwrap_or(0),
            Err(_) => 0,
        }
    }

    fn process_rss_bytes() -> usize {
        #[cfg(target_os = "linux")]
        {
            return Self::linux_rss_bytes();
        }

        #[cfg(target_os = "macos")]
        {
            return darwin_rss_bytes();
        }

        #[allow(unreachable_code)]
        0
    }

    #[cfg(test)]
    pub(crate) fn current_process_rss_bytes_for_test() -> usize {
        Self::process_rss_bytes()
    }

    #[cfg(target_os = "linux")]
    fn linux_rss_bytes() -> usize {
        std::fs::read_to_string("/proc/self/statm")
            .ok()
            .and_then(|s| s.split_whitespace().nth(1).map(str::to_string))
            .and_then(|pages| pages.parse::<usize>().ok())
            .map(|pages| pages.saturating_mul(4096))
            .unwrap_or(0)
    }

    fn growth_permille_from_env() -> usize {
        match std::env::var("CRUFT_GC_HEADROOM") {
            Ok(s) => s
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|f| f.is_finite() && *f > 0.0)
                .map(|f| ((f * 1000.0) as usize).max(1100))
                .unwrap_or(4000),
            Err(_) => 4000,
        }
    }

    pub fn set_growth_permille(&mut self, permille: usize) {
        self.growth_permille = permille.max(1100);
    }

    fn generation_matches(&self, index: usize, id: ObjectId) -> bool {
        self.generations
            .get(index)
            .copied()
            .is_some_and(|generation| generation == id.generation())
    }

    fn retire_slot_index(&mut self, index: usize) {
        if let Some(generation) = self.generations.get_mut(index) {
            if *generation < OBJECT_ID_MAX_GENERATION {
                *generation += 1;
                self.free_list.push(index as u32);
            }
        }
    }

    fn adapt_threshold_after_sweep(&mut self) {

        self.external_bytes_since_collect = 0;
        let live = self
            .slots
            .iter()
            .filter(|s| !matches!(s, Slot::Free))
            .count();
        let live_pressure = live.saturating_mul(2).max(1024);
        let reuse_pressure = if self.free_list.is_empty() {
            live_pressure
        } else {
            (self.free_list.len() / 2).max(1024)
        };

        let base = if self.no_slot_reuse {
            live_pressure.max(65_536)
        } else {
            live_pressure.min(reuse_pressure).max(1024)
        };

        let effective_permille = self.autotuned_permille();
        self.threshold = base
            .saturating_mul(effective_permille)
            .saturating_div(2000)
            .max(1024);
    }

    fn autotuned_permille(&self) -> usize {
        if self.soft_target_bytes == 0 {
            return self.growth_permille;
        }
        let rss = Self::process_rss_bytes();
        if rss == 0 {
            return self.growth_permille;
        }
        let target = self.soft_target_bytes as f64;
        let pressure = rss as f64 / target;
        let floor = 1100.0;
        let full = self.growth_permille as f64;
        let eff = if pressure <= 0.5 {
            full
        } else if pressure >= 1.0 {
            floor
        } else {

            let t = (pressure - 0.5) / 0.5;
            full + (floor - full) * t
        };
        (eff as usize).max(1100)
    }

    pub fn alloc_external(&mut self, handle: ExternalHandle) -> ObjectId {
        self.alloc_external_with_owner(handle, 0)
    }

    pub fn alloc_external_with_owner(&mut self, handle: ExternalHandle, owner: usize) -> ObjectId {
        self.alloc_count += 1;
        let idx = if let Some(idx) = if self.no_slot_reuse {
            None
        } else {
            self.free_list.pop()
        } {
            self.slots[idx as usize] = Slot::External(handle);
            self.colors[idx as usize] = Color::White;
            self.owners[idx as usize] = owner;
            idx
        } else {
            let idx = self.slots.len() as u32;
            assert!(
                idx <= OBJECT_ID_INDEX_MASK,
                "GC heap exceeded packed ObjectId slot capacity"
            );
            self.slots.push(Slot::External(handle));
            self.colors.push(Color::White);
            self.owners.push(owner);
            self.generations.push(0);
            idx
        };
        ObjectId::new(idx, self.generations[idx as usize])
    }

    pub fn is_external_handle(&self, id: ObjectId) -> bool {
        let i = id.slot_index();
        self.generation_matches(i, id) && matches!(self.slots.get(i), Some(Slot::External(_)))
    }

    pub fn take_freed_external_handles(&mut self) -> Vec<ExternalHandle> {
        core::mem::take(&mut self.freed_external)
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn free_len(&self) -> usize {
        self.free_list.len()
    }

    pub fn live_len(&self) -> usize {
        self.slots
            .iter()
            .filter(|s| !matches!(s, Slot::Free))
            .count()
    }

    pub fn alloc(&mut self, v: T) -> ObjectId {
        self.alloc_with_owner(v, 0)
    }

    pub fn alloc_with_owner(&mut self, v: T, owner: usize) -> ObjectId {
        self.alloc_count += 1;
        let idx = if let Some(idx) = if self.no_slot_reuse {
            None
        } else {
            self.free_list.pop()
        } {
            self.slots[idx as usize] = Slot::Object(v);
            self.colors[idx as usize] = Color::White;
            self.owners[idx as usize] = owner;
            idx
        } else {
            let idx = self.slots.len() as u32;
            if idx > OBJECT_ID_INDEX_MASK {

                if let Some(sentinel) = self.oom_sentinel {
                    self.oom = true;
                    let s = sentinel as usize;
                    self.slots[s] = Slot::Object(v);
                    self.colors[s] = Color::White;
                    self.owners[s] = owner;
                    return ObjectId::new(sentinel, self.generations[s]);
                }

                assert!(
                    idx <= OBJECT_ID_INDEX_MASK,
                    "GC heap exceeded packed ObjectId slot capacity"
                );
            } else if idx == OBJECT_ID_INDEX_MASK - OOM_SLOT_MARGIN {

                self.oom = true;
            }
            self.slots.push(Slot::Object(v));
            self.colors.push(Color::White);
            self.owners.push(owner);
            self.generations.push(0);
            idx
        };
        ObjectId::new(idx, self.generations[idx as usize])
    }

    pub fn reserve_oom_sentinel(&mut self, id: ObjectId) {
        self.oom_sentinel = Some(id.slot_index() as u32);
    }

    #[inline]
    pub fn oom_pending(&self) -> bool {
        self.oom
    }

    pub fn take_oom(&mut self) -> bool {
        core::mem::take(&mut self.oom)
    }

    pub fn owner(&self, id: ObjectId) -> Option<usize> {
        let i = id.slot_index();
        if !self.generation_matches(i, id) {
            return None;
        }
        match self.slots.get(i) {
            Some(Slot::Object(_) | Slot::External(_)) => self.owners.get(i).copied(),
            _ => None,
        }
    }

    pub fn set_owner(&mut self, id: ObjectId, owner: usize) -> bool {
        let i = id.slot_index();
        if !self.generation_matches(i, id) {
            return false;
        }
        match self.slots.get(i) {
            Some(Slot::Object(_) | Slot::External(_)) => {
                self.owners[i] = owner;
                true
            }
            _ => false,
        }
    }

    pub fn preserve_new_owner_allocation(&mut self, owner: usize, id: ObjectId) -> bool {
        let i = id.slot_index();
        if !self.generation_matches(i, id) {
            return false;
        }
        match self.slots.get(i) {
            Some(Slot::Object(_) | Slot::External(_))
                if self.owners.get(i).copied() == Some(owner) =>
            {
                self.colors[i] = Color::Black;
                true
            }
            _ => false,
        }
    }

    pub fn cross_owner_edges(&self) -> Vec<CrossOwnerEdge> {
        let mut out = Vec::new();
        for (i, slot) in self.slots.iter().enumerate() {
            let Slot::Object(obj) = slot else {
                continue;
            };
            let from_owner = self.owners[i];
            let mut refs = Vec::new();
            obj.trace(&mut refs);
            for to in refs {
                let ti = to.slot_index();
                if !self.generation_matches(ti, to)
                    || ti >= self.slots.len()
                    || matches!(self.slots[ti], Slot::Free)
                {
                    continue;
                }
                let to_owner = self.owners[ti];
                if from_owner != to_owner {
                    out.push(CrossOwnerEdge {
                        from: ObjectId::new(i as u32, self.generations[i]),
                        from_owner,
                        to,
                        to_owner,
                    });
                }
            }
        }
        out
    }

    pub fn has_cross_owner_edges(&self) -> bool {
        !self.cross_owner_edges().is_empty()
    }

    pub fn get(&self, id: ObjectId) -> Option<&T> {
        let i = id.slot_index();
        if !self.generation_matches(i, id) {
            return None;
        }
        match self.slots.get(i) {
            Some(Slot::Object(v)) => Some(v),
            _ => None,
        }
    }

    pub fn slots_base_ptr(&self) -> usize {
        self.slots.as_ptr() as usize
    }

    pub fn slot_stride() -> usize {
        core::mem::size_of::<Slot<T>>()
    }

    pub fn slot_object_payload_offset(probe: T) -> usize
    where
        T: Sized,
    {
        let slot = Slot::Object(probe);
        let base = &slot as *const Slot<T> as usize;
        let payload = match &slot {
            Slot::Object(v) => v as *const T as usize,
            _ => unreachable!(),
        };
        payload - base
    }

    pub fn get_mut(&mut self, id: ObjectId) -> Option<&mut T> {
        let i = id.slot_index();
        if !self.generation_matches(i, id) {
            return None;
        }
        match self.slots.get_mut(i) {
            Some(Slot::Object(v)) => Some(v),
            _ => None,
        }
    }

    pub fn live_object_ids(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.slots.iter().enumerate().filter_map(|(i, slot)| {
            if matches!(slot, Slot::Object(_)) {
                Some(ObjectId::new(i as u32, self.generations[i]))
            } else {
                None
            }
        })
    }

    pub fn is_free(&self, id: ObjectId) -> bool {
        let i = id.slot_index();
        !self.generation_matches(i, id) || matches!(self.slots.get(i), Some(Slot::Free) | None)
    }

    pub fn free(&mut self, id: ObjectId) {
        let i = id.slot_index();
        if self.generation_matches(i, id) && i < self.slots.len() {
            self.slots[i] = Slot::Free;
            self.colors[i] = Color::White;
            self.owners[i] = 0;
            self.retire_slot_index(i);
        }
    }

    pub fn sweep_realm(&mut self, owner: usize) -> usize {
        let mut freed = 0usize;
        for i in 0..self.slots.len() {
            if self.owners[i] != owner || matches!(self.slots[i], Slot::Free) {
                continue;
            }
            if let Slot::External(h) = &self.slots[i] {
                self.freed_external.push(*h);
            }
            self.slots[i] = Slot::Free;
            self.colors[i] = Color::White;
            self.owners[i] = 0;
            self.retire_slot_index(i);
            freed += 1;
        }
        freed
    }

    pub fn begin_mark_realm(&mut self, owner: usize, roots: impl IntoIterator<Item = ObjectId>) {
        for c in self.colors.iter_mut() {
            *c = Color::White;
        }
        for r in roots {
            let i = r.slot_index();
            if i < self.slots.len()
                && self.generation_matches(i, r)
                && self.owners[i] == owner
                && !matches!(self.slots[i], Slot::Free)
                && self.colors[i] == Color::White
            {
                self.colors[i] = Color::Gray;
            }
        }
    }

    pub fn enqueue_mark_owner(&mut self, owner: usize, root: ObjectId) {
        let i = root.slot_index();
        if i >= self.slots.len()
            || !self.generation_matches(i, root)
            || self.owners[i] != owner
            || matches!(self.slots[i], Slot::Free)
            || self.colors[i] == Color::Black
        {
            return;
        }
        self.colors[i] = Color::Gray;
    }

    pub fn mark_owner_slice(&mut self, owner: usize, budget: usize) -> usize {
        self.mark_owner_slice_with_cursor(owner, budget, &mut RealmMarkCursor::default())
    }

    pub fn mark_owner_slice_with_cursor(
        &mut self,
        owner: usize,
        mut budget: usize,
        cursor: &mut RealmMarkCursor,
    ) -> usize {
        if budget == 0 {
            return 0;
        }
        let mut processed = 0usize;
        let mut out_edges: Vec<ObjectId> = Vec::new();
        while budget > 0 {
            let (id, start_edge) = if let Some((id, start)) = cursor.object.take() {
                (id, start)
            } else {
                let Some(id) = self.next_gray_owner_object(owner, cursor) else {
                    break;
                };
                (id, 0)
            };
            let i = id.slot_index();
            if i >= self.slots.len()
                || !self.generation_matches(i, id)
                || self.owners[i] != owner
                || matches!(self.slots[i], Slot::Free)
                || self.colors[i] == Color::Black
            {
                continue;
            }
            processed += 1;
            out_edges.clear();
            let trace_slice = if let Slot::Object(obj) = &self.slots[i] {
                obj.trace_slice(start_edge, budget, &mut out_edges)
            } else {
                TraceSlice {
                    next_index: start_edge,
                    complete: true,
                }
            };
            budget = budget.saturating_sub(out_edges.len().max(1));
            if trace_slice.complete {
                self.colors[i] = Color::Black;
            } else {
                cursor.object = Some((id, trace_slice.next_index));
            }
            for e in out_edges.iter().copied() {
                let ei = e.slot_index();
                if ei < self.slots.len()
                    && self.generation_matches(ei, e)
                    && self.owners[ei] == owner
                    && !matches!(self.slots[ei], Slot::Free)
                    && self.colors[ei] == Color::White
                {
                    self.colors[ei] = Color::Gray;
                }
            }
        }
        processed
    }

    fn next_gray_owner_object(
        &self,
        owner: usize,
        cursor: &mut RealmMarkCursor,
    ) -> Option<ObjectId> {
        let len = self.slots.len();
        if len == 0 {
            cursor.next_index = 0;
            return None;
        }
        let start = cursor.next_index.min(len);
        for offset in 0..len {
            let i = (start + offset) % len;
            if self.colors[i] == Color::Gray
                && self.owners[i] == owner
                && !matches!(self.slots[i], Slot::Free)
            {
                cursor.next_index = (i + 1) % len;
                return Some(ObjectId::new(i as u32, self.generations[i]));
            }
        }
        cursor.next_index = 0;
        None
    }

    pub fn mark_from_owner(&mut self, owner: usize, root: ObjectId) {
        self.enqueue_mark_owner(owner, root);
        while self.mark_owner_slice(owner, usize::MAX) != 0 {}
    }

    pub fn drain_mark_realm(&mut self, owner: usize) {
        while self.mark_owner_slice(owner, usize::MAX) != 0 {}
    }

    pub fn sweep_marked_realm(&mut self, owner: usize) -> usize {
        let mut state = RealmSweepState::default();
        while !self.sweep_marked_realm_slice(owner, usize::MAX, &mut state) {}
        state.freed
    }

    pub fn sweep_marked_realm_slice(
        &mut self,
        owner: usize,
        mut budget: usize,
        state: &mut RealmSweepState,
    ) -> bool {
        while budget > 0 && state.next_index < self.slots.len() {
            let i = state.next_index;
            state.next_index += 1;
            budget -= 1;
            if self.owners[i] != owner {
                if self.colors[i] == Color::Black {
                    self.colors[i] = Color::White;
                }
                continue;
            }
            match (self.colors[i], &self.slots[i]) {
                (Color::White, Slot::External(h)) => {
                    self.freed_external.push(*h);
                    self.slots[i] = Slot::Free;
                    self.owners[i] = 0;
                    self.retire_slot_index(i);
                    state.freed += 1;
                }
                (Color::White, Slot::Object(_)) => {
                    self.slots[i] = Slot::Free;
                    self.owners[i] = 0;
                    self.retire_slot_index(i);
                    state.freed += 1;
                }
                (Color::Black, _) => self.colors[i] = Color::White,
                _ => {}
            }
        }

        if state.next_index < self.slots.len() {
            return false;
        }

        self.alloc_count = 0;
        self.adapt_threshold_after_sweep();
        true
    }

    pub fn collect(&mut self, roots: impl IntoIterator<Item = ObjectId>) -> usize {
        self.collect_with_ephemerons(roots, std::iter::empty())
    }

    pub fn collect_with_ephemerons(
        &mut self,
        roots: impl IntoIterator<Item = ObjectId>,
        ephemerons: impl IntoIterator<Item = (ObjectId, ObjectId)>,
    ) -> usize {

        for c in self.colors.iter_mut() {
            *c = Color::White;
        }
        let ephemerons: Vec<(ObjectId, ObjectId)> = ephemerons.into_iter().collect();

        let mut worklist: Vec<ObjectId> = Vec::new();
        for r in roots {
            let i = r.slot_index();
            if i < self.slots.len()
                && self.generation_matches(i, r)
                && !matches!(self.slots[i], Slot::Free)
                && self.colors[i] == Color::White
            {
                self.colors[i] = Color::Gray;
                worklist.push(r);
            }
        }

        while let Some(id) = worklist.pop() {
            let i = id.slot_index();
            if !self.generation_matches(i, id) {
                continue;
            }
            self.colors[i] = Color::Black;
            let mut out_edges: Vec<ObjectId> = Vec::new();
            if let Slot::Object(obj) = &self.slots[i] {
                obj.trace(&mut out_edges);
            }
            for e in out_edges {
                let ei = e.slot_index();
                if ei < self.slots.len()
                    && self.generation_matches(ei, e)
                    && !matches!(self.slots[ei], Slot::Free)
                    && self.colors[ei] == Color::White
                {
                    self.colors[ei] = Color::Gray;
                    worklist.push(e);
                }
            }
        }

        loop {
            let mut changed = false;
            for (key, value) in &ephemerons {
                let ki = key.slot_index();
                let vi = value.slot_index();
                if ki >= self.slots.len()
                    || vi >= self.slots.len()
                    || !self.generation_matches(ki, *key)
                    || !self.generation_matches(vi, *value)
                    || matches!(self.slots[ki], Slot::Free)
                    || matches!(self.slots[vi], Slot::Free)
                    || self.colors[ki] == Color::White
                    || self.colors[vi] != Color::White
                {
                    continue;
                }
                self.colors[vi] = Color::Gray;
                worklist.push(*value);
                changed = true;
            }
            while let Some(id) = worklist.pop() {
                let i = id.slot_index();
                if !self.generation_matches(i, id) {
                    continue;
                }
                self.colors[i] = Color::Black;
                let mut out_edges: Vec<ObjectId> = Vec::new();
                if let Slot::Object(obj) = &self.slots[i] {
                    obj.trace(&mut out_edges);
                }
                for e in out_edges {
                    let ei = e.slot_index();
                    if ei < self.slots.len()
                        && self.generation_matches(ei, e)
                        && !matches!(self.slots[ei], Slot::Free)
                        && self.colors[ei] == Color::White
                    {
                        self.colors[ei] = Color::Gray;
                        worklist.push(e);
                    }
                }
            }
            if !changed {
                break;
            }
        }

        let mut freed = 0usize;
        for i in 0..self.slots.len() {
            match (self.colors[i], &self.slots[i]) {
                (Color::White, Slot::External(h)) => {
                    self.freed_external.push(*h);
                    self.slots[i] = Slot::Free;
                    self.owners[i] = 0;
                    self.retire_slot_index(i);
                    freed += 1;
                }
                (Color::White, Slot::Object(_)) => {
                    self.slots[i] = Slot::Free;
                    self.owners[i] = 0;
                    self.retire_slot_index(i);
                    freed += 1;
                }
                (Color::Black, _) => self.colors[i] = Color::White,
                _ => {}
            }
        }

        self.alloc_count = 0;

        self.adapt_threshold_after_sweep();
        freed
    }

    pub fn collect_realm(
        &mut self,
        owner: usize,
        roots: impl IntoIterator<Item = ObjectId>,
    ) -> usize {
        self.collect_realm_with_ephemerons(owner, roots, std::iter::empty())
    }

    pub fn collect_realm_with_ephemerons(
        &mut self,
        owner: usize,
        roots: impl IntoIterator<Item = ObjectId>,
        ephemerons: impl IntoIterator<Item = (ObjectId, ObjectId)>,
    ) -> usize {
        for c in self.colors.iter_mut() {
            *c = Color::White;
        }
        let ephemerons: Vec<(ObjectId, ObjectId)> = ephemerons.into_iter().collect();

        let mut worklist: Vec<ObjectId> = Vec::new();
        for r in roots {
            let i = r.slot_index();
            if i < self.slots.len()
                && self.generation_matches(i, r)
                && self.owners[i] == owner
                && !matches!(self.slots[i], Slot::Free)
                && self.colors[i] == Color::White
            {
                self.colors[i] = Color::Gray;
                worklist.push(r);
            }
        }

        while let Some(id) = worklist.pop() {
            let i = id.slot_index();
            if i >= self.slots.len() || !self.generation_matches(i, id) || self.owners[i] != owner {
                continue;
            }
            self.colors[i] = Color::Black;
            let mut out_edges: Vec<ObjectId> = Vec::new();
            if let Slot::Object(obj) = &self.slots[i] {
                obj.trace(&mut out_edges);
            }
            for e in out_edges {
                let ei = e.slot_index();
                if ei < self.slots.len()
                    && self.generation_matches(ei, e)
                    && self.owners[ei] == owner
                    && !matches!(self.slots[ei], Slot::Free)
                    && self.colors[ei] == Color::White
                {
                    self.colors[ei] = Color::Gray;
                    worklist.push(e);
                }
            }
        }

        loop {
            let mut changed = false;
            for (key, value) in &ephemerons {
                let ki = key.slot_index();
                let vi = value.slot_index();
                if ki >= self.slots.len()
                    || vi >= self.slots.len()
                    || !self.generation_matches(ki, *key)
                    || !self.generation_matches(vi, *value)
                    || self.owners[ki] != owner
                    || self.owners[vi] != owner
                    || matches!(self.slots[ki], Slot::Free)
                    || matches!(self.slots[vi], Slot::Free)
                    || self.colors[ki] == Color::White
                    || self.colors[vi] != Color::White
                {
                    continue;
                }
                self.colors[vi] = Color::Gray;
                worklist.push(*value);
                changed = true;
            }
            while let Some(id) = worklist.pop() {
                let i = id.slot_index();
                if i >= self.slots.len()
                    || !self.generation_matches(i, id)
                    || self.owners[i] != owner
                {
                    continue;
                }
                self.colors[i] = Color::Black;
                let mut out_edges: Vec<ObjectId> = Vec::new();
                if let Slot::Object(obj) = &self.slots[i] {
                    obj.trace(&mut out_edges);
                }
                for e in out_edges {
                    let ei = e.slot_index();
                    if ei < self.slots.len()
                        && self.generation_matches(ei, e)
                        && self.owners[ei] == owner
                        && !matches!(self.slots[ei], Slot::Free)
                        && self.colors[ei] == Color::White
                    {
                        self.colors[ei] = Color::Gray;
                        worklist.push(e);
                    }
                }
            }
            if !changed {
                break;
            }
        }

        let mut freed = 0usize;
        for i in 0..self.slots.len() {
            if self.owners[i] != owner {
                if self.colors[i] == Color::Black {
                    self.colors[i] = Color::White;
                }
                continue;
            }
            match (self.colors[i], &self.slots[i]) {
                (Color::White, Slot::External(h)) => {
                    self.freed_external.push(*h);
                    self.slots[i] = Slot::Free;
                    self.owners[i] = 0;
                    self.retire_slot_index(i);
                    freed += 1;
                }
                (Color::White, Slot::Object(_)) => {
                    self.slots[i] = Slot::Free;
                    self.owners[i] = 0;
                    self.retire_slot_index(i);
                    freed += 1;
                }
                (Color::Black, _) => self.colors[i] = Color::White,
                _ => {}
            }
        }

        self.alloc_count = 0;
        self.adapt_threshold_after_sweep();
        freed
    }

    pub fn try_collect_realm(
        &mut self,
        owner: usize,
        roots: impl IntoIterator<Item = ObjectId>,
    ) -> Result<usize, CrossOwnerEdgesError> {
        self.try_collect_realm_with_ephemerons(owner, roots, std::iter::empty())
    }

    pub fn try_collect_realm_with_ephemerons(
        &mut self,
        owner: usize,
        roots: impl IntoIterator<Item = ObjectId>,
        ephemerons: impl IntoIterator<Item = (ObjectId, ObjectId)>,
    ) -> Result<usize, CrossOwnerEdgesError> {
        let edges: Vec<_> = self
            .cross_owner_edges()
            .into_iter()
            .filter(|edge| edge.to_owner == owner)
            .collect();
        if !edges.is_empty() {
            return Err(CrossOwnerEdgesError { edges });
        }
        Ok(self.collect_realm_with_ephemerons(owner, roots, ephemerons))
    }

    pub fn maybe_collect(&mut self, roots: impl IntoIterator<Item = ObjectId>) -> Option<usize> {
        if self.should_collect() {
            Some(self.collect(roots))
        } else {
            None
        }
    }

    pub fn should_collect(&self) -> bool {
        self.alloc_count >= self.threshold
            || self.external_bytes_since_collect >= self.external_byte_threshold
            || self.soft_target_pressure_exceeded()
    }

    fn soft_target_pressure_exceeded(&self) -> bool {
        self.soft_target_bytes > 0
            && self.alloc_count >= SOFT_TARGET_PRESSURE_ALLOC_QUANTUM
            && Self::process_rss_bytes() >= self.soft_target_bytes
    }

    pub fn alloc_count(&self) -> usize {
        self.alloc_count
    }

    pub fn live_count(&self) -> usize {
        self.slots
            .iter()
            .filter(|s| matches!(s, Slot::Object(_)))
            .count()
    }

    pub fn free_count(&self) -> usize {
        self.free_list.len()
    }
}

#[cfg(test)]
mod external_handle_tests {
    use super::*;

    struct Node(Vec<ObjectId>);
    impl Trace for Node {
        fn trace(&self, ids: &mut Vec<ObjectId>) {
            ids.extend(self.0.iter().copied());
        }
    }

    #[test]
    fn external_handle_opacity_and_refcount_coupling() {
        let mut h: Heap<Node> = Heap::new();
        let ext_live = h.alloc_external(ExternalHandle(42));
        let ext_dead = h.alloc_external(ExternalHandle(99));
        assert!(h.is_external_handle(ext_live));
        assert!(h.is_external_handle(ext_dead));

        let holder = h.alloc(Node(vec![ext_live]));

        let freed = h.collect([holder]);
        assert_eq!(freed, 1, "only the unheld Tier-2 handle is reclaimed");
        assert!(
            h.is_external_handle(ext_live),
            "reachable Tier-2 handle survives"
        );
        assert!(
            !h.is_external_handle(ext_dead),
            "unreachable Tier-2 handle freed"
        );

        assert_eq!(h.take_freed_external_handles(), vec![ExternalHandle(99)]);
        assert!(h.take_freed_external_handles().is_empty(), "drained once");

        assert_eq!(h.collect([holder]), 0);
        assert!(h.take_freed_external_handles().is_empty());
        assert_eq!(
            h.collect(std::iter::empty()),
            2,
            "holder + its Tier-2 handle"
        );
        assert_eq!(h.take_freed_external_handles(), vec![ExternalHandle(42)]);
    }

    #[test]
    fn sweep_threshold_scales_reclaimed_capacity_by_default_headroom() {
        let mut h: Heap<Node> = Heap::new_with_slot_reuse_for_test();
        let mut live_children = Vec::new();
        for _ in 0..3_000 {
            live_children.push(h.alloc(Node(Vec::new())));
        }
        let root = h.alloc(Node(live_children));
        for _ in 0..1_500 {
            h.alloc(Node(Vec::new()));
        }

        assert_eq!(h.collect([root]), 1_500);
        assert_eq!(h.free_count(), 1_500);
        assert_eq!(
            h.threshold, 2_048,
            "4x default headroom should collect after roughly the reclaimed-slot floor has room to amortize"
        );
    }

    #[test]
    fn sweep_threshold_scales_larger_reclaimed_capacity_by_default_headroom() {
        let mut h: Heap<Node> = Heap::new_with_slot_reuse_for_test();
        let mut live_children = Vec::new();
        for _ in 0..3_000 {
            live_children.push(h.alloc(Node(Vec::new())));
        }
        let root = h.alloc(Node(live_children));
        for _ in 0..8_000 {
            h.alloc(Node(Vec::new()));
        }

        assert_eq!(h.collect([root]), 8_000);
        assert_eq!(h.free_count(), 8_000);
        assert_eq!(
            h.threshold, 8_000,
            "larger churn mouths should collect after roughly the reclaimed slots are reused under 4x headroom"
        );
    }

    #[test]
    fn production_reuse_threshold_consumes_reclaimed_slots() {
        let mut h: Heap<Node> = Heap::new();
        let root = h.alloc(Node(Vec::new()));
        for _ in 0..8_000 {
            h.alloc(Node(Vec::new()));
        }

        assert_eq!(h.collect([root]), 8_000);
        assert_eq!(h.free_count(), 8_000);
        assert_eq!(
            h.threshold, 2_048,
            "production heaps reuse reclaimed ids safely, so tiny-live-set churn can keep the tight live-set threshold"
        );
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn soft_target_requests_collection_before_stale_large_threshold() {
        let mut h: Heap<Node> = Heap::new_with_slot_reuse_for_test();
        h.soft_target_bytes = 1;
        h.threshold = SOFT_TARGET_PRESSURE_ALLOC_QUANTUM * 16;
        for _ in 0..SOFT_TARGET_PRESSURE_ALLOC_QUANTUM {
            h.alloc(Node(Vec::new()));
        }
        assert!(
            h.should_collect(),
            "soft target pressure should request collection before an inflated post-spike threshold"
        );
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn process_rss_bytes_reports_current_process_residency() {
        let rss = Heap::<Node>::current_process_rss_bytes_for_test();
        assert!(
            rss > 0,
            "RSS-backed GC soft target should be live on supported hosts"
        );
    }

    struct SlicedNode(Vec<ObjectId>);
    impl Trace for SlicedNode {
        fn trace(&self, ids: &mut Vec<ObjectId>) {
            ids.extend(self.0.iter().copied());
        }

        fn trace_slice(&self, start: usize, budget: usize, ids: &mut Vec<ObjectId>) -> TraceSlice {
            let end = (start + budget).min(self.0.len());
            ids.extend(self.0[start..end].iter().copied());
            TraceSlice {
                next_index: end,
                complete: end == self.0.len(),
            }
        }
    }

    #[test]
    fn owner_mark_cursor_persists_scan_and_edge_state_across_slices() {
        let mut h: Heap<SlicedNode> = Heap::new();
        let owner = 7;
        let mut children = Vec::new();
        for _ in 0..32 {
            children.push(h.alloc_with_owner(SlicedNode(Vec::new()), owner));
        }
        let root = h.alloc_with_owner(SlicedNode(children.clone()), owner);
        let dead = h.alloc_with_owner(SlicedNode(Vec::new()), owner);
        h.begin_mark_realm(owner, [root]);

        let mut cursor = RealmMarkCursor::default();
        let first = h.mark_owner_slice_with_cursor(owner, 4, &mut cursor);
        assert_eq!(first, 1);
        assert_eq!(cursor.object, Some((root, 4)));

        let mut progressed = first;
        loop {
            let n = h.mark_owner_slice_with_cursor(owner, 4, &mut cursor);
            if n == 0 {
                break;
            }
            progressed += n;
        }
        assert!(
            progressed > 1,
            "wide sliced trace should require more than one mark slice"
        );
        let freed = h.sweep_marked_realm(owner);
        assert_eq!(freed, 1);
        assert!(h.get(root).is_some());
        for child in children {
            assert!(h.get(child).is_some());
        }
        assert!(h.get(dead).is_none());
    }

    #[test]
    fn sweep_realm_frees_only_owner_slots() {
        let mut h = Heap::<Node>::new();
        let owner_one = h.alloc_with_owner(Node(Vec::new()), 1);
        let owner_two = h.alloc_with_owner(Node(Vec::new()), 2);
        let external_one = h.alloc_external_with_owner(ExternalHandle(7), 1);

        assert_eq!(h.owner(owner_one), Some(1));
        assert_eq!(h.owner(owner_two), Some(2));
        assert_eq!(h.owner(external_one), Some(1));

        assert_eq!(h.sweep_realm(1), 2);
        assert!(h.is_free(owner_one));
        assert!(h.is_free(external_one));
        assert_eq!(h.owner(owner_one), None);
        assert_eq!(h.owner(owner_two), Some(2));
        assert!(h.get(owner_two).is_some());
        assert_eq!(h.take_freed_external_handles(), vec![ExternalHandle(7)]);
    }

    #[test]
    fn collect_realm_traces_only_owner_slots() {
        let mut h = Heap::<Node>::new();
        let owner_two_external = h.alloc_external_with_owner(ExternalHandle(8), 2);
        let owner_two = h.alloc_with_owner(Node(vec![owner_two_external]), 2);
        let owner_one = h.alloc_with_owner(Node(vec![owner_two]), 1);

        assert_eq!(
            h.collect_realm(2, [owner_one]),
            2,
            "owner-local collection must ignore direct cross-owner Tier-1 roots"
        );
        assert!(h.is_free(owner_two));
        assert!(h.is_free(owner_two_external));
        assert!(h.get(owner_one).is_some());
        assert_eq!(h.take_freed_external_handles(), vec![ExternalHandle(8)]);
    }

    #[test]
    fn collect_realm_ephemeron_live_key_marks_value() {
        let mut h = Heap::<Node>::new();
        let payload = h.alloc_with_owner(Node(Vec::new()), 2);
        let value = h.alloc_with_owner(Node(vec![payload]), 2);
        let key = h.alloc_with_owner(Node(Vec::new()), 2);

        assert_eq!(
            h.collect_realm_with_ephemerons(2, [key], [(key, value)]),
            0,
            "owner-local ephemeron fixpoint must keep value and its edges live"
        );
        assert!(h.get(key).is_some());
        assert!(h.get(value).is_some());
        assert!(h.get(payload).is_some());
    }

    #[test]
    fn collect_realm_ephemeron_cycle_does_not_self_retain_dead_key() {
        let mut h = Heap::<Node>::new();
        let key = h.alloc_with_owner(Node(Vec::new()), 2);
        let value = h.alloc_with_owner(Node(vec![key]), 2);

        assert_eq!(
            h.collect_realm_with_ephemerons(2, std::iter::empty(), [(key, value)]),
            2,
            "owner-local ephemeron values must not self-retain dead keys"
        );
        assert!(h.is_free(key));
        assert!(h.is_free(value));
    }

    #[test]
    fn cross_owner_edge_audit_reports_only_raw_tier1_edges() {
        let mut h = Heap::<Node>::new();
        let owner_two_external = h.alloc_external_with_owner(ExternalHandle(8), 2);
        let owner_two = h.alloc_with_owner(Node(vec![owner_two_external]), 2);
        let owner_one_local = h.alloc_with_owner(Node(Vec::new()), 1);
        let owner_one = h.alloc_with_owner(Node(vec![owner_one_local, owner_two]), 1);

        let edges = h.cross_owner_edges();
        assert_eq!(edges.len(), 1);
        assert_eq!(
            edges[0],
            CrossOwnerEdge {
                from: owner_one,
                from_owner: 1,
                to: owner_two,
                to_owner: 2,
            }
        );
        assert!(
            edges.iter().all(|edge| edge.to != owner_two_external),
            "External leaves are opaque slots, not traced Tier-1 edges"
        );
        assert!(h.has_cross_owner_edges());
    }

    #[test]
    fn try_collect_realm_refuses_unmediated_cross_owner_edges() {
        let mut h = Heap::<Node>::new();
        let owner_two = h.alloc_with_owner(Node(Vec::new()), 2);
        let owner_one = h.alloc_with_owner(Node(vec![owner_two]), 1);

        let err = h
            .try_collect_realm(2, [owner_one])
            .expect_err("raw cross-owner Tier-1 edge must block owner-local collection");
        assert_eq!(err.edges.len(), 1);
        assert_eq!(err.edges[0].from_owner, 1);
        assert_eq!(err.edges[0].to_owner, 2);
        assert!(
            h.get(owner_two).is_some(),
            "failed checked collection must not sweep anything"
        );

        h.free(owner_one);
        assert_eq!(h.try_collect_realm(2, [owner_two]), Ok(0));
    }

    #[test]
    fn try_collect_realm_allows_outgoing_cross_owner_edges() {
        let mut h = Heap::<Node>::new();
        let owner_one = h.alloc_with_owner(Node(Vec::new()), 1);
        let owner_two = h.alloc_with_owner(Node(vec![owner_one]), 2);

        assert_eq!(
            h.try_collect_realm(2, std::iter::empty()),
            Ok(1),
            "an outgoing edge from the collected owner must not block reclamation"
        );
        assert!(
            h.get(owner_two).is_none(),
            "unrooted owner-local object should be swept"
        );
        assert!(
            h.get(owner_one).is_some(),
            "foreign object reached only by an outgoing edge must remain untouched"
        );
    }
}

use core::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier2Handle(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CyclicSharedError;

struct Tier2Slot<T> {
    value: T,
    rc: AtomicU64,

    refs: Vec<Tier2Handle>,
}

pub struct Tier2Arena<T> {
    slots: Vec<Option<Tier2Slot<T>>>,

    retired: std::sync::Mutex<Vec<(u64, Tier2Handle)>>,

    epoch: AtomicU64,
}

impl<T> Default for Tier2Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Tier2Arena<T> {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            retired: std::sync::Mutex::new(Vec::new()),
            epoch: AtomicU64::new(0),
        }
    }

    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }

    pub fn advance_epoch(&self) -> u64 {
        self.epoch.fetch_add(1, Ordering::AcqRel) + 1
    }

    pub fn freeze(
        &mut self,
        value: T,
        refs: Vec<Tier2Handle>,
    ) -> Result<Tier2Handle, CyclicSharedError> {
        let new_id = self.slots.len() as u64;
        for r in &refs {

            if r.0 >= new_id
                || self
                    .slots
                    .get(r.0 as usize)
                    .map(|s| s.is_none())
                    .unwrap_or(true)
            {
                return Err(CyclicSharedError);
            }
        }
        for r in &refs {
            self.incref(*r);
        }
        self.slots.push(Some(Tier2Slot {
            value,
            rc: AtomicU64::new(1),
            refs,
        }));
        Ok(Tier2Handle(new_id))
    }

    pub fn get(&self, h: Tier2Handle) -> Option<&T> {
        self.slots
            .get(h.0 as usize)
            .and_then(|s| s.as_ref())
            .map(|s| &s.value)
    }

    pub fn incref(&self, h: Tier2Handle) {
        if let Some(Some(s)) = self.slots.get(h.0 as usize) {
            s.rc.fetch_add(1, Ordering::AcqRel);
        }
    }

    pub fn decref(&self, h: Tier2Handle) -> bool {
        let hit_zero = match self.slots.get(h.0 as usize).and_then(|s| s.as_ref()) {
            Some(s) => s.rc.fetch_sub(1, Ordering::AcqRel) == 1,
            None => false,
        };
        if hit_zero {

            self.retired
                .lock()
                .unwrap()
                .push((self.epoch.load(Ordering::Acquire), h));
        }
        hit_zero
    }

    pub fn retire(&mut self) -> usize {
        let epoch = self.epoch.load(Ordering::Acquire);
        let mut still_pending = Vec::new();
        let mut to_free = Vec::new();
        for (e, h) in core::mem::take(&mut *self.retired.lock().unwrap()) {
            if e + 2 <= epoch {
                to_free.push(h);
            } else {
                still_pending.push((e, h));
            }
        }
        *self.retired.lock().unwrap() = still_pending;
        let mut freed = 0;
        for h in to_free {
            if let Some(slot) = self.slots.get_mut(h.0 as usize) {
                if let Some(s) = slot.take() {
                    freed += 1;
                    let refs = s.refs;
                    drop(s.value);
                    for r in refs {
                        self.decref(r);
                    }
                }
            }
        }
        freed
    }

    pub fn live_count(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    pub fn refcount(&self, h: Tier2Handle) -> Option<u64> {
        self.slots
            .get(h.0 as usize)
            .and_then(|s| s.as_ref())
            .map(|s| s.rc.load(Ordering::Acquire))
    }
}

#[cfg(test)]
mod tier2_arena_tests {
    use super::*;

    #[test]
    fn atomic_refcount_lifecycle_and_acyclic_guard() {
        let mut a: Tier2Arena<i64> = Tier2Arena::new();
        let leaf = a.freeze(10, vec![]).unwrap();
        assert_eq!(a.refcount(leaf), Some(1));

        let parent = a.freeze(20, vec![leaf]).unwrap();
        assert_eq!(a.refcount(leaf), Some(2));
        assert_eq!(a.get(parent), Some(&20));

        assert_eq!(a.freeze(30, vec![Tier2Handle(999)]), Err(CyclicSharedError));

        a.incref(parent);
        assert_eq!(a.refcount(parent), Some(2));
        assert!(!a.decref(parent));
        assert!(a.decref(parent));
        assert_eq!(a.live_count(), 2, "retired but not yet freed (epoch grace)");

        assert_eq!(a.retire(), 0, "epoch 0: still inside the grace");
        a.advance_epoch();
        assert_eq!(a.retire(), 0, "epoch 1: still inside the grace");
        a.advance_epoch();

        assert_eq!(a.retire(), 1);
        assert_eq!(a.get(parent), None);
        assert_eq!(a.refcount(leaf), Some(1), "parent's hold on leaf released");

        assert!(a.decref(leaf));
        assert_eq!(a.retire(), 0, "leaf inside its own grace");
        a.advance_epoch();
        a.advance_epoch();
        assert_eq!(a.retire(), 1);
        assert_eq!(a.live_count(), 0);
    }
}

use std::collections::HashSet;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RefcountDeltaPlan {

    pub increments: Vec<Tier2Handle>,

    pub decrements: Vec<Tier2Handle>,

    pub targets_valid: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetNotRegisteredError;

pub struct Tier3<T> {
    arena: Tier2Arena<T>,
    registered: HashSet<u64>,
}

impl<T> Default for Tier3<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Tier3<T> {
    pub fn new() -> Self {
        Self {
            arena: Tier2Arena::new(),
            registered: HashSet::new(),
        }
    }

    pub fn arena(&self) -> &Tier2Arena<T> {
        &self.arena
    }
    pub fn arena_mut(&mut self) -> &mut Tier2Arena<T> {
        &mut self.arena
    }

    pub fn register(&mut self, compartment: u64) {
        self.registered.insert(compartment);
    }

    pub fn unregister(&mut self, compartment: u64) {
        self.registered.remove(&compartment);
    }

    pub fn is_registered(&self, compartment: u64) -> bool {
        self.registered.contains(&compartment)
    }

    pub fn plan_crossing(
        &self,
        clone_crossed: &[Tier2Handle],
        transfer_crossed: &[Tier2Handle],
        to_compartment: u64,
    ) -> RefcountDeltaPlan {
        RefcountDeltaPlan {
            increments: clone_crossed.to_vec(),
            decrements: transfer_crossed.to_vec(),
            targets_valid: self.is_registered(to_compartment),
        }
    }

    pub fn apply(&mut self, plan: &RefcountDeltaPlan) -> Result<(), TargetNotRegisteredError> {
        if !plan.targets_valid {
            return Err(TargetNotRegisteredError);
        }
        for h in &plan.increments {
            self.arena.incref(*h);
        }
        for h in &plan.decrements {
            self.arena.decref(*h);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tier3_tests {
    use super::*;

    #[test]
    fn plan_is_side_effect_free_apply_commits_and_identity_equivalence() {
        let mut t: Tier3<i64> = Tier3::new();
        let h = t.arena_mut().freeze(7, vec![]).unwrap();
        t.register(1);
        assert_eq!(t.arena().refcount(h), Some(1));

        let plan = t.plan_crossing(&[h], &[], 1);
        assert_eq!(t.arena().refcount(h), Some(1), "plan touches no atomic");
        assert!(plan.targets_valid);

        let empty = t.plan_crossing(&[], &[], 1);
        assert!(empty.increments.is_empty() && empty.decrements.is_empty());
        assert!(t.apply(&empty).is_ok());
        assert_eq!(t.arena().refcount(h), Some(1));

        t.apply(&plan).unwrap();
        assert_eq!(t.arena().refcount(h), Some(2));

        let bad = t.plan_crossing(&[h], &[], 999);
        assert!(!bad.targets_valid);
        assert_eq!(t.apply(&bad), Err(TargetNotRegisteredError));
        assert_eq!(
            t.arena().refcount(h),
            Some(2),
            "rejected send mutated nothing"
        );

        t.unregister(1);
        assert!(!t.is_registered(1));
        assert_eq!(
            t.apply(&t.plan_crossing(&[h], &[], 1)),
            Err(TargetNotRegisteredError)
        );
    }
}

pub fn tier3_atomic_touches(plan: &RefcountDeltaPlan) -> usize {
    plan.increments.len() + plan.decrements.len()
}

#[cfg(test)]
mod workload_class_benchmark {
    use super::*;

    fn arena_with(n: usize) -> (Tier3<i64>, Vec<Tier2Handle>) {
        let mut t: Tier3<i64> = Tier3::new();
        t.register(1);
        let handles = (0..n)
            .map(|i| t.arena_mut().freeze(i as i64, vec![]).unwrap())
            .collect();
        (t, handles)
    }

    #[test]
    fn workload_class_atomic_cost_model() {

        let (t, hs) = arena_with(1000);
        let w2 = t.plan_crossing(&hs, &[], 1);
        assert_eq!(tier3_atomic_touches(&w2), 1000);

        let w1 = t.plan_crossing(&[], &hs[..3], 1);
        assert_eq!(tier3_atomic_touches(&w1), 3);

        let mut total = 0;
        for _ in 0..10_000 {
            total += tier3_atomic_touches(&t.plan_crossing(&[], &[], 1));
        }
        assert_eq!(
            total, 0,
            "no-Shared sends touch no Tier-3 atomic, at any volume"
        );

        let mixed = t.plan_crossing(&hs[..10], &hs[10..15], 1);
        assert_eq!(tier3_atomic_touches(&mixed), 15);
    }
}

use std::sync::mpsc;

pub struct MailboxSender<M> {
    tx: mpsc::Sender<M>,
}

impl<M> Clone for MailboxSender<M> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailboxClosedError;

impl<M> MailboxSender<M> {

    pub fn enqueue(&self, msg: M) -> Result<(), MailboxClosedError> {
        self.tx.send(msg).map_err(|_| MailboxClosedError)
    }
}

pub struct Mailbox<M> {
    rx: mpsc::Receiver<M>,
    tx: mpsc::Sender<M>,
}

impl<M> Default for Mailbox<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> Mailbox<M> {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { rx, tx }
    }

    pub fn sender(&self) -> MailboxSender<M> {
        MailboxSender {
            tx: self.tx.clone(),
        }
    }

    pub fn try_dequeue(&self) -> Option<M> {
        self.rx.try_recv().ok()
    }

    pub fn drain(&self) -> Vec<M> {
        self.rx.try_iter().collect()
    }

    pub fn recv(&self) -> Option<M> {
        self.rx.recv().ok()
    }
}

#[cfg(test)]
mod mailbox_tests {
    use super::*;

    #[test]
    fn fifo_single_thread_and_cross_thread_enqueue() {
        let mb: Mailbox<u64> = Mailbox::new();

        let s = mb.sender();
        s.enqueue(1).unwrap();
        s.enqueue(2).unwrap();
        s.enqueue(3).unwrap();
        assert_eq!(mb.drain(), vec![1, 2, 3]);
        assert!(mb.try_dequeue().is_none(), "drained");

        let mut handles = Vec::new();
        for t in 0..4u64 {
            let st = mb.sender();
            handles.push(std::thread::spawn(move || {
                for i in 0..250u64 {
                    st.enqueue(t * 1000 + i).unwrap();
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }

        let mut got = Vec::new();
        while got.len() < 1000 {
            got.extend(mb.drain());
        }
        assert_eq!(got.len(), 1000);
        got.sort_unstable();
        let want: Vec<u64> = (0..4)
            .flat_map(|t| (0..250).map(move |i| t * 1000 + i))
            .collect();
        let mut want_sorted = want;
        want_sorted.sort_unstable();
        assert_eq!(
            got, want_sorted,
            "no message lost or duplicated across threads"
        );
    }
}
pub mod boundary;

type Job = Box<dyn FnOnce() + Send + 'static>;

enum WorkerMsg {
    Run(Job),
    Shutdown,
}

pub struct WorkerPool {
    senders: Vec<MailboxSender<WorkerMsg>>,
    handles: Vec<std::thread::JoinHandle<()>>,
}

const WORKER_POOL_THREAD_STACK_SIZE: usize = 16 * 1024 * 1024;

impl WorkerPool {

    pub fn new(n: usize) -> Self {
        let n = n.max(1);
        let mut senders = Vec::with_capacity(n);
        let mut handles = Vec::with_capacity(n);
        for _ in 0..n {
            let mb: Mailbox<WorkerMsg> = Mailbox::new();
            senders.push(mb.sender());

            let handle = std::thread::Builder::new()
                .name(format!(
                    "cruft-worker-{worker_idx}",
                    worker_idx = handles.len()
                ))
                .stack_size(WORKER_POOL_THREAD_STACK_SIZE)
                .spawn(move || {
                    while let Some(msg) = mb.recv() {
                        match msg {
                            WorkerMsg::Run(job) => job(),
                            WorkerMsg::Shutdown => break,
                        }
                    }
                })
                .expect("spawn cruft worker pool thread");
            handles.push(handle);
        }
        Self { senders, handles }
    }

    pub fn default_threads() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    }

    pub fn worker_count(&self) -> usize {
        self.senders.len()
    }

    pub fn submit<F: FnOnce() + Send + 'static>(
        &self,
        worker: usize,
        job: F,
    ) -> Result<(), MailboxClosedError> {
        self.senders[worker % self.senders.len()].enqueue(WorkerMsg::Run(Box::new(job)))
    }

    pub fn shutdown(self) {
        for s in &self.senders {
            let _ = s.enqueue(WorkerMsg::Shutdown);
        }
        for h in self.handles {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod worker_pool_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    #[test]
    fn bounded_pool_executes_affinity_jobs_and_shuts_down() {
        let pool = WorkerPool::new(4);
        assert_eq!(
            pool.worker_count(),
            4,
            "bounded to the requested thread count"
        );

        let counter = Arc::new(AtomicU64::new(0));

        for i in 0..1000u64 {
            let c = counter.clone();
            pool.submit((i % 4) as usize, move || {
                c.fetch_add(1, Ordering::AcqRel);
            })
            .unwrap();
        }

        pool.shutdown();
        assert_eq!(
            counter.load(Ordering::Acquire),
            1000,
            "every job ran exactly once"
        );
    }

    #[test]
    fn default_threads_is_bounded_and_at_least_one() {
        assert!(WorkerPool::default_threads() >= 1);
    }
}

pub struct Scheduler {
    pool: WorkerPool,

    affinity: std::collections::HashMap<u64, usize>,

    load: Vec<usize>,

    next: usize,
}

impl Scheduler {
    pub fn new(threads: usize) -> Self {
        let pool = WorkerPool::new(threads);
        let n = pool.worker_count();
        Self {
            pool,
            affinity: std::collections::HashMap::new(),
            load: vec![0; n],
            next: 0,
        }
    }

    pub fn worker_count(&self) -> usize {
        self.pool.worker_count()
    }

    pub fn assign(&mut self, compartment: u64) -> usize {
        if let Some(&w) = self.affinity.get(&compartment) {
            return w;
        }

        let n = self.load.len();
        let start = self.next % n;
        let mut best = start;
        for k in 0..n {
            let w = (start + k) % n;
            if self.load[w] < self.load[best] {
                best = w;
            }
        }
        self.next = (best + 1) % n;
        self.affinity.insert(compartment, best);
        self.load[best] += 1;
        best
    }

    pub fn worker_of(&self, compartment: u64) -> Option<usize> {
        self.affinity.get(&compartment).copied()
    }

    pub fn migrate(&mut self, compartment: u64, to_worker: usize) -> bool {
        let to = to_worker % self.load.len();
        match self.affinity.get(&compartment).copied() {
            Some(from) if from != to => {
                self.load[from] = self.load[from].saturating_sub(1);
                self.load[to] += 1;
                self.affinity.insert(compartment, to);
                true
            }
            _ => false,
        }
    }

    pub fn remove(&mut self, compartment: u64) {
        if let Some(w) = self.affinity.remove(&compartment) {
            self.load[w] = self.load[w].saturating_sub(1);
        }
    }

    pub fn load(&self) -> &[usize] {
        &self.load
    }

    pub fn submit_turn<F: FnOnce() + Send + 'static>(
        &mut self,
        compartment: u64,
        job: F,
    ) -> Result<(), MailboxClosedError> {
        let w = self.assign(compartment);
        self.pool.submit(w, job)
    }

    pub fn shutdown(self) {
        self.pool.shutdown();
    }
}

#[cfg(test)]
mod scheduler_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    #[test]
    fn affinity_initial_spread_migration_and_turn_routing() {
        let mut s = Scheduler::new(4);
        assert_eq!(s.worker_count(), 4);

        for c in 0..8u64 {
            s.assign(c);
        }
        assert_eq!(s.load(), &[2, 2, 2, 2], "even initial spread");

        let w = s.worker_of(0).unwrap();
        assert_eq!(s.assign(0), w, "re-assign returns existing affinity");

        let from = s.worker_of(0).unwrap();
        let to = (from + 1) % 4;
        assert!(s.migrate(0, to));
        assert_eq!(s.load()[from], 1);
        assert_eq!(s.load()[to], 3);
        assert_eq!(s.worker_of(0), Some(to));
        assert!(!s.migrate(0, to), "no-op when already there");

        let counter = Arc::new(AtomicU64::new(0));
        for c in 0..8u64 {
            for _ in 0..100 {
                let cc = counter.clone();
                s.submit_turn(c, move || {
                    cc.fetch_add(1, Ordering::AcqRel);
                })
                .unwrap();
            }
        }
        s.shutdown();
        assert_eq!(
            counter.load(Ordering::Acquire),
            800,
            "every turn ran exactly once"
        );
    }
}

#[cfg(test)]
mod tier2_arena_concurrency_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn concurrent_refcount_deltas_are_race_free() {
        let mut arena: Tier2Arena<i64> = Tier2Arena::new();
        let h = arena.freeze(42, vec![]).unwrap();
        let arena = Arc::new(arena);

        let mut handles = Vec::new();
        for _ in 0..8 {
            let a = arena.clone();
            handles.push(std::thread::spawn(move || {
                for _ in 0..10_000 {
                    a.incref(h);
                    let hit_zero = a.decref(h);
                    assert!(!hit_zero, "base rc keeps it live; no spurious retirement");
                }
            }));
        }
        for j in handles {
            j.join().unwrap();
        }
        assert_eq!(
            arena.refcount(h),
            Some(1),
            "all deltas committed atomically, net zero"
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundarySendError {

    TargetNotRegistered,

    MailboxClosed,
}

pub fn boundary_send<T, M>(
    arena: &Tier2Arena<T>,
    target_registered: bool,
    clone: &[Tier2Handle],
    transfer: &[Tier2Handle],
    target_mailbox: &MailboxSender<M>,
    envelope: M,
) -> Result<(), BoundarySendError> {

    if !target_registered {
        return Err(BoundarySendError::TargetNotRegistered);
    }

    for h in clone {
        arena.incref(*h);
    }
    for h in transfer {
        arena.decref(*h);
    }

    target_mailbox
        .enqueue(envelope)
        .map_err(|_| BoundarySendError::MailboxClosed)
}

#[cfg(test)]
mod boundary_send_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    struct Envelope {
        payload: i64,
        shared: Tier2Handle,
    }

    #[test]
    fn end_to_end_cross_thread_send_with_identity_equivalence() {

        let mut arena_owned: Tier2Arena<i64> = Tier2Arena::new();
        let h = arena_owned.freeze(777, vec![]).unwrap();
        let arena = Arc::new(arena_owned);

        let mailbox: Mailbox<Envelope> = Mailbox::new();
        let to_b = mailbox.sender();
        let processed = Arc::new(AtomicU64::new(0));

        let arena_b = arena.clone();
        let processed_b = processed.clone();
        let worker_b = std::thread::spawn(move || {

            if let Some(env) = mailbox.recv() {
                assert_eq!(env.payload, 9);
                assert_eq!(
                    arena_b.get(env.shared),
                    Some(&777),
                    "Shared value readable cross-thread"
                );
                processed_b.store(env.payload as u64, Ordering::Release);

                arena_b.decref(env.shared);
            }
        });

        let registered = true;
        let r = boundary_send(
            &arena,
            registered,
            &[h],
            &[],
            &to_b,
            Envelope {
                payload: 9,
                shared: h,
            },
        );
        assert!(r.is_ok());
        assert_eq!(
            arena.refcount(h),
            Some(2),
            "Phase B incref committed (sender's 1 + B's clone)"
        );

        worker_b.join().unwrap();
        assert_eq!(
            processed.load(Ordering::Acquire),
            9,
            "B processed the delivered envelope"
        );
        assert_eq!(
            arena.refcount(h),
            Some(1),
            "B released its clone; back to the sender's hold"
        );

        let before = arena.refcount(h);
        let bad = boundary_send(
            &arena,
            false,
            &[h],
            &[],
            &to_b,
            Envelope {
                payload: 0,
                shared: h,
            },
        );
        assert_eq!(bad, Err(BoundarySendError::TargetNotRegistered));
        assert_eq!(
            arena.refcount(h),
            before,
            "rejected send mutated no refcount"
        );
    }
}

#[cfg(test)]
mod phase4_integration_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    #[test]
    fn n_compartments_m_threads_no_data_race_workload_class_holds() {

        let mut arena_owned: Tier2Arena<i64> = Tier2Arena::new();
        let handles: Vec<Tier2Handle> = (0..16)
            .map(|i| arena_owned.freeze(i, vec![]).unwrap())
            .collect();
        let arena = Arc::new(arena_owned);

        let pool = WorkerPool::new(4);
        let delivered = Arc::new(AtomicU64::new(0));

        let mut senders = Vec::new();
        let mut joiners = Vec::new();
        for _ in 0..4u64 {
            let mb: Mailbox<Tier2Handle> = Mailbox::new();
            senders.push(mb.sender());
            let a = arena.clone();
            let d = delivered.clone();

            joiners.push(std::thread::spawn(move || {

                let mut seen = 0u64;
                while seen < 100 {
                    if let Some(h) = mb.recv() {

                        let _ = a.get(h);
                        a.decref(h);
                        seen += 1;
                        d.fetch_add(1, Ordering::AcqRel);
                    } else {
                        break;
                    }
                }
            }));
        }

        let total_atomics = Arc::new(AtomicU64::new(0));
        for s in 0..4usize {
            let arena_s = arena.clone();
            let sender = senders[s].clone();
            let hs = handles.clone();
            let ta = total_atomics.clone();
            pool.submit(s, move || {
                for round in 0..100u64 {
                    let h = hs[(round as usize) % hs.len()];

                    let plan_atomics = 1;
                    boundary_send(&arena_s, true, &[h], &[], &sender, h).unwrap();
                    ta.fetch_add(plan_atomics, Ordering::AcqRel);
                }
            })
            .unwrap();
        }
        pool.shutdown();

        for j in joiners {
            j.join().unwrap();
        }

        assert_eq!(
            delivered.load(Ordering::Acquire),
            400,
            "every cross-thread send delivered exactly once"
        );

        assert_eq!(total_atomics.load(Ordering::Acquire), 400);

        for (i, h) in handles.iter().enumerate() {
            assert_eq!(
                arena.refcount(*h),
                Some(1),
                "handle {i} refcount returned to base — no leak, no over-release"
            );
        }
    }
}
pub mod watchdog;
