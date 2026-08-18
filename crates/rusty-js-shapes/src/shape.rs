
use rusty_js_smallvec::SmallVec;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

const SLOTS_INLINE_CAP: usize = 8;
const TRANSITIONS_INLINE_CAP: usize = 4;

pub type SlotIndex = u32;

pub struct Shape {

    slots: SmallOrLargeSlotMap,

    transitions: RefCell<SmallOrLargeTransitionMap>,

    parent: Option<Rc<Shape>>,

    slot_count: SlotIndex,
}

enum SmallOrLargeSlotMap {
    Small(SmallVec<(String, SlotIndex), SLOTS_INLINE_CAP>),
    Large(Vec<(String, SlotIndex)>, HashMap<String, SlotIndex>),
}

impl SmallOrLargeSlotMap {
    fn new() -> Self {
        Self::Small(SmallVec::new())
    }

    fn len(&self) -> usize {
        match self {
            Self::Small(v) => v.len(),
            Self::Large(v, _) => v.len(),
        }
    }

    fn get(&self, name: &str) -> Option<SlotIndex> {
        match self {
            Self::Small(v) => v.iter().find(|(n, _)| n == name).map(|(_, s)| *s),
            Self::Large(_, h) => h.get(name).copied(),
        }
    }

    fn iter(&self) -> Box<dyn Iterator<Item = (&str, SlotIndex)> + '_> {
        match self {
            Self::Small(v) => Box::new(v.iter().map(|(n, s)| (n.as_str(), *s))),
            Self::Large(v, _) => Box::new(v.iter().map(|(n, s)| (n.as_str(), *s))),
        }
    }

    fn push(&mut self, name: String) -> SlotIndex {
        let new_slot = self.len() as SlotIndex;
        match self {
            Self::Small(v) if v.len() < SLOTS_INLINE_CAP => {
                v.push((name, new_slot));
            }
            Self::Small(_) => {

                let Self::Small(old) =
                    std::mem::replace(self, Self::Large(Vec::new(), HashMap::new()))
                else {
                    unreachable!()
                };
                let Self::Large(vec, map) = self else {
                    unreachable!()
                };
                for (n, s) in old {
                    map.insert(n.clone(), s);
                    vec.push((n, s));
                }
                map.insert(name.clone(), new_slot);
                vec.push((name, new_slot));
            }
            Self::Large(v, h) => {
                h.insert(name.clone(), new_slot);
                v.push((name, new_slot));
            }
        }
        new_slot
    }

    fn cloned(&self) -> Self {
        match self {
            Self::Small(v) => Self::Small(v.clone()),
            Self::Large(v, h) => Self::Large(v.clone(), h.clone()),
        }
    }
}

enum SmallOrLargeTransitionMap {
    Small(SmallVec<(String, Weak<Shape>), TRANSITIONS_INLINE_CAP>),

    Large(HashMap<String, Weak<Shape>>, usize),
}

impl SmallOrLargeTransitionMap {
    fn new() -> Self {
        Self::Small(SmallVec::new())
    }

    fn len(&self) -> usize {
        match self {
            Self::Small(v) => v.len(),
            Self::Large(h, _) => h.len(),
        }
    }

    fn get(&self, name: &str) -> Option<Rc<Shape>> {
        match self {
            Self::Small(v) => v
                .iter()
                .find(|(n, _)| n == name)
                .and_then(|(_, w)| w.upgrade()),
            Self::Large(h, _) => h.get(name).and_then(|w| w.upgrade()),
        }
    }

    fn insert(&mut self, name: String, shape: &Rc<Shape>) {
        let weak = Rc::downgrade(shape);
        match self {

            Self::Small(v) => {
                let mut kept: Vec<(String, Weak<Shape>)> = Vec::new();
                for (n, w) in v.iter() {
                    if *n != name && w.strong_count() > 0 {
                        kept.push((n.clone(), w.clone()));
                    }
                }
                if kept.len() < TRANSITIONS_INLINE_CAP {
                    let mut nv: SmallVec<(String, Weak<Shape>), TRANSITIONS_INLINE_CAP> =
                        SmallVec::new();
                    for e in kept {
                        nv.push(e);
                    }
                    nv.push((name, weak));
                    *self = Self::Small(nv);
                } else {
                    let mut h: HashMap<String, Weak<Shape>> = HashMap::new();
                    for (n, w) in kept {
                        h.insert(n, w);
                    }
                    h.insert(name, weak);
                    *self = Self::Large(h, 64);
                }
            }
            Self::Large(h, reap_at) => {

                if h.len() >= *reap_at {
                    h.retain(|_, w| w.strong_count() > 0);
                    *reap_at = h.len() * 2 + 64;
                }
                h.insert(name, weak);
            }
        }
    }
}

impl Shape {

    pub fn root() -> Rc<Shape> {
        thread_local! {
            static ROOT: Rc<Shape> = Rc::new(Shape {
                slots: SmallOrLargeSlotMap::new(),
                transitions: RefCell::new(SmallOrLargeTransitionMap::new()),
                parent: None,
                slot_count: 0,
            });
        }
        ROOT.with(|r| Rc::clone(r))
    }

    pub fn transition_to(self: &Rc<Shape>, name: &str) -> Rc<Shape> {

        if let Some(existing) = self.transitions.borrow().get(name) {
            return existing;
        }

        let mut child_slots = self.slots.cloned();
        let _new_slot = child_slots.push(name.to_string());
        let child = Rc::new(Shape {
            slots: child_slots,
            transitions: RefCell::new(SmallOrLargeTransitionMap::new()),
            parent: Some(Rc::clone(self)),
            slot_count: self.slot_count + 1,
        });

        self.transitions
            .borrow_mut()
            .insert(name.to_string(), &child);
        child
    }

    pub fn slot_of(&self, name: &str) -> Option<SlotIndex> {
        self.slots.get(name)
    }

    pub fn slot_count(&self) -> SlotIndex {
        self.slot_count
    }

    pub fn iter_slots(&self) -> impl Iterator<Item = (&str, SlotIndex)> + '_ {
        self.slots.iter()
    }

    pub fn parent(self: &Rc<Shape>) -> Option<Rc<Shape>> {
        self.parent.as_ref().map(Rc::clone)
    }

    pub fn as_raw_ptr(self: &Rc<Shape>) -> *const Shape {
        Rc::as_ptr(self)
    }

    pub fn transition_count(&self) -> usize {
        self.transitions.borrow().len()
    }
}

impl std::fmt::Debug for Shape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shape")
            .field("slot_count", &self.slot_count)
            .field("transition_count", &self.transition_count())
            .field("slots", &self.slots.iter().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_is_empty() {
        let root = Shape::root();
        assert_eq!(root.slot_count(), 0);
        assert!(root.slot_of("any").is_none());
        assert!(root.parent().is_none());
        assert_eq!(root.iter_slots().count(), 0);
    }

    #[test]
    fn single_transition_assigns_slot_zero() {
        let root = Shape::root();
        let s = root.transition_to("x");
        assert_eq!(s.slot_count(), 1);
        assert_eq!(s.slot_of("x"), Some(0));
        assert!(s.parent().is_some());
        assert!(Rc::ptr_eq(&s.parent().unwrap(), &root));
    }

    #[test]
    fn same_transition_same_shape() {
        let root = Shape::root();
        let a = root.transition_to("x");
        let b = root.transition_to("x");
        assert!(
            Rc::ptr_eq(&a, &b),
            "same transition must reuse shape (Pred-shape.2)"
        );
    }

    #[test]
    fn different_transitions_distinct_shapes() {
        let root = Shape::root();
        let a = root.transition_to("x");
        let b = root.transition_to("y");
        assert!(!Rc::ptr_eq(&a, &b));
        assert_eq!(a.slot_of("x"), Some(0));
        assert_eq!(b.slot_of("y"), Some(0));
        assert!(a.slot_of("y").is_none());
        assert!(b.slot_of("x").is_none());
    }

    #[test]
    fn chain_preserves_insertion_order_and_identity() {
        let root = Shape::root();
        let path_a = root
            .transition_to("x")
            .transition_to("y")
            .transition_to("z");
        let path_b = root
            .transition_to("x")
            .transition_to("y")
            .transition_to("z");
        assert!(Rc::ptr_eq(&path_a, &path_b));
        assert_eq!(path_a.slot_count(), 3);
        assert_eq!(path_a.slot_of("x"), Some(0));
        assert_eq!(path_a.slot_of("y"), Some(1));
        assert_eq!(path_a.slot_of("z"), Some(2));
        let names: Vec<&str> = path_a.iter_slots().map(|(n, _)| n).collect();
        assert_eq!(names, vec!["x", "y", "z"]);
    }

    #[test]
    fn order_divergent_chains_distinct() {
        let root = Shape::root();
        let xy = root.transition_to("x").transition_to("y");
        let yx = root.transition_to("y").transition_to("x");
        assert!(!Rc::ptr_eq(&xy, &yx));
        assert_eq!(xy.slot_of("x"), Some(0));
        assert_eq!(xy.slot_of("y"), Some(1));
        assert_eq!(yx.slot_of("y"), Some(0));
        assert_eq!(yx.slot_of("x"), Some(1));
    }

    #[test]
    fn slot_map_promotes_past_inline_cap() {
        let root = Shape::root();
        let mut cur = root;
        for i in 0..(SLOTS_INLINE_CAP + 2) {
            cur = cur.transition_to(&format!("p{}", i));
        }
        assert_eq!(cur.slot_count() as usize, SLOTS_INLINE_CAP + 2);
        for i in 0..(SLOTS_INLINE_CAP + 2) {
            assert_eq!(cur.slot_of(&format!("p{}", i)), Some(i as SlotIndex));
        }

        let names: Vec<String> = cur.iter_slots().map(|(n, _)| n.to_string()).collect();
        let expected: Vec<String> = (0..(SLOTS_INLINE_CAP + 2))
            .map(|i| format!("p{}", i))
            .collect();
        assert_eq!(names, expected);
    }

    #[test]
    fn transition_map_promotes_past_inline_cap() {
        let root = Shape::root();
        let mut children: Vec<Rc<Shape>> = Vec::new();
        for i in 0..(TRANSITIONS_INLINE_CAP + 2) {
            children.push(root.transition_to(&format!("k{}", i)));
        }

        for (i, child) in children.iter().enumerate() {
            let again = root.transition_to(&format!("k{}", i));
            assert!(
                Rc::ptr_eq(child, &again),
                "identity must hold across map promotion"
            );
        }
        assert_eq!(root.transition_count(), TRANSITIONS_INLINE_CAP + 2);
    }

    #[test]
    fn shape_count_linear_in_unique_paths() {
        let root = Shape::root();

        let pool: Vec<String> = (0..10).map(|i| format!("p{}", i)).collect();

        let sequences: Vec<Vec<&String>> = vec![
            vec![&pool[0], &pool[1], &pool[2], &pool[3], &pool[4]],
            vec![&pool[0], &pool[1], &pool[2], &pool[3], &pool[5]],
            vec![&pool[0], &pool[1], &pool[2], &pool[6], &pool[7]],
            vec![&pool[0], &pool[1], &pool[8], &pool[9], &pool[4]],
            vec![&pool[5], &pool[6], &pool[7], &pool[8], &pool[9]],
        ];
        let mut leaf_shapes: Vec<Rc<Shape>> = Vec::new();
        for _obj in 0..100 {
            for seq in &sequences {
                let mut cur = Rc::clone(&root);
                for name in seq {
                    cur = cur.transition_to(name);
                }
                leaf_shapes.push(cur);
            }
        }

        let mut distinct_leaves: Vec<*const Shape> =
            leaf_shapes.iter().map(|s| Rc::as_ptr(s)).collect();
        distinct_leaves.sort();
        distinct_leaves.dedup();
        assert_eq!(
            distinct_leaves.len(),
            5,
            "leaf shape count must equal distinct-sequence count (Pred-shape.3)"
        );
    }

    #[test]
    fn as_raw_ptr_is_rc_pointer() {
        let root = Shape::root();
        let s = root.transition_to("x");
        let ptr = s.as_raw_ptr();
        assert_eq!(ptr, Rc::as_ptr(&s));

        let s2 = Rc::clone(&s);
        assert_eq!(s2.as_raw_ptr(), ptr);
    }
}
