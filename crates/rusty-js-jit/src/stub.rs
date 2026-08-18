
use std::cell::RefCell;

pub type ICSiteId = u32;

pub const MISS_THRESHOLD: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ICState {

    Cold,

    WarmMono,

    ColdAfterMiss,

    Degraded,
}

#[derive(Debug)]
pub struct ICEntry {

    pub cached_shape: *const rusty_js_shapes::Shape,

    pub cached_slot: u32,

    pub pinned_shape_holder: Option<std::rc::Rc<rusty_js_shapes::Shape>>,

    pub miss_count: u32,

    pub degraded: bool,
}

impl ICEntry {
    pub fn new_cold() -> Self {
        Self {
            cached_shape: std::ptr::null(),
            cached_slot: 0,
            pinned_shape_holder: None,
            miss_count: 0,
            degraded: false,
        }
    }

    pub fn state(&self) -> ICState {
        if self.degraded {
            ICState::Degraded
        } else if self.cached_shape.is_null() {
            ICState::Cold
        } else if self.miss_count > 0 {
            ICState::ColdAfterMiss
        } else {
            ICState::WarmMono
        }
    }

    pub fn observe(&mut self, shape: std::rc::Rc<rusty_js_shapes::Shape>, slot: u32) {
        if self.degraded {
            return;
        }
        if !self.cached_shape.is_null() {

            self.miss_count = self.miss_count.saturating_add(1);
            if self.miss_count > MISS_THRESHOLD {
                self.degraded = true;
                self.cached_shape = std::ptr::null();
                self.cached_slot = 0;
                self.pinned_shape_holder = None;
                return;
            }
        }
        let ptr = std::rc::Rc::as_ptr(&shape);
        self.cached_shape = ptr;
        self.cached_slot = slot;
        self.pinned_shape_holder = Some(shape);
    }

    pub fn observe_miss_no_shape(&mut self) {
        if self.degraded {
            return;
        }
        if !self.cached_shape.is_null() {
            self.miss_count = self.miss_count.saturating_add(1);
            if self.miss_count > MISS_THRESHOLD {
                self.degraded = true;
                self.cached_shape = std::ptr::null();
                self.cached_slot = 0;
                self.pinned_shape_holder = None;
            }
        }
    }
}

pub struct ICStubCache {
    sites: Vec<ICEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IcSiteSnapshot {
    pub site_id: ICSiteId,
    pub site_count: usize,
    pub state: ICState,
    pub cached_shape: usize,
    pub cached_slot: u32,
    pub miss_count: u32,
    pub degraded: bool,
}

impl ICStubCache {
    pub fn new() -> Self {
        Self { sites: Vec::new() }
    }

    pub fn alloc_site(&mut self) -> ICSiteId {
        let id = self.sites.len() as ICSiteId;
        self.sites.push(ICEntry::new_cold());
        id
    }

    pub fn entry(&self, id: ICSiteId) -> &ICEntry {
        &self.sites[id as usize]
    }

    pub fn entry_opt(&self, id: ICSiteId) -> Option<&ICEntry> {
        self.sites.get(id as usize)
    }

    pub fn entry_mut(&mut self, id: ICSiteId) -> &mut ICEntry {
        &mut self.sites[id as usize]
    }

    pub fn len(&self) -> usize {
        self.sites.len()
    }

    pub fn state_histogram(&self) -> (usize, usize, usize, usize) {
        let mut cold = 0;
        let mut warm = 0;
        let mut cam = 0;
        let mut deg = 0;
        for s in self.sites.iter().map(|e| e.state()) {
            match s {
                ICState::Cold => cold += 1,
                ICState::WarmMono => warm += 1,
                ICState::ColdAfterMiss => cam += 1,
                ICState::Degraded => deg += 1,
            }
        }
        (cold, warm, cam, deg)
    }

    pub fn snapshot(&self, id: ICSiteId) -> Option<IcSiteSnapshot> {
        let entry = self.entry_opt(id)?;
        Some(IcSiteSnapshot {
            site_id: id,
            site_count: self.sites.len(),
            state: entry.state(),
            cached_shape: entry.cached_shape as usize,
            cached_slot: entry.cached_slot,
            miss_count: entry.miss_count,
            degraded: entry.degraded,
        })
    }
}

impl Default for ICStubCache {
    fn default() -> Self {
        Self::new()
    }
}

thread_local! {

    pub static IC_STUB_CACHE: RefCell<ICStubCache> = RefCell::new(ICStubCache::new());
}

pub fn alloc_ic_site() -> ICSiteId {
    IC_STUB_CACHE.with(|c| c.borrow_mut().alloc_site())
}

pub fn observe_at_site(id: ICSiteId, shape: std::rc::Rc<rusty_js_shapes::Shape>, slot: u32) {
    IC_STUB_CACHE.with(|c| c.borrow_mut().entry_mut(id).observe(shape, slot));
}

pub fn observe_miss_no_shape_at_site(id: ICSiteId) {
    IC_STUB_CACHE.with(|c| c.borrow_mut().entry_mut(id).observe_miss_no_shape());
}

pub fn ic_site_snapshot(id: ICSiteId) -> Option<IcSiteSnapshot> {
    IC_STUB_CACHE.with(|c| c.borrow().snapshot(id))
}

pub fn ic_site_count() -> usize {
    IC_STUB_CACHE.with(|c| c.borrow().len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_js_shapes::Shape;

    #[test]
    fn ic_site_snapshot_reports_state_without_panicking_on_missing_site() {
        let shape = Shape::root().transition_to("value");
        let mut cache = ICStubCache::new();
        let site = cache.alloc_site();

        let cold = cache
            .snapshot(site)
            .expect("allocated site should snapshot");
        assert_eq!(cold.site_id, site);
        assert_eq!(cold.site_count, 1);
        assert_eq!(cold.state, ICState::Cold);
        assert_eq!(cold.cached_shape, 0);
        assert!(cache.snapshot(site + 1).is_none());

        cache.entry_mut(site).observe(shape, 0);
        let warm = cache.snapshot(site).expect("warm site should snapshot");
        assert_eq!(warm.state, ICState::WarmMono);
        assert_ne!(warm.cached_shape, 0);
        assert_eq!(warm.cached_slot, 0);
        assert_eq!(warm.miss_count, 0);
        assert!(!warm.degraded);
    }
}
