
#[derive(Clone, Debug)]
pub enum SmallVec<T, const N: usize> {

    Inline { buf: [Option<T>; N], len: usize },

    Heap(Vec<T>),
}

impl<T, const N: usize> SmallVec<T, N> {

    pub fn new() -> Self {
        SmallVec::Inline {
            buf: std::array::from_fn(|_| None),
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            SmallVec::Inline { len, .. } => *len,
            SmallVec::Heap(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn push(&mut self, value: T) {
        match self {
            SmallVec::Inline { buf, len } if *len < N => {
                buf[*len] = Some(value);
                *len += 1;
            }
            SmallVec::Inline { buf, len } => {

                let mut v = Vec::with_capacity(*len + 1);
                for slot in buf.iter_mut().take(*len) {
                    v.push(slot.take().expect("inline slot < len is Some"));
                }
                v.push(value);
                *self = SmallVec::Heap(v);
            }
            SmallVec::Heap(v) => v.push(value),
        }
    }

    pub fn iter(&self) -> Iter<'_, T> {
        match self {
            SmallVec::Inline { buf, len } => Iter::Inline(buf[..*len].iter()),
            SmallVec::Heap(v) => Iter::Heap(v.iter()),
        }
    }
}

impl<T, const N: usize> Default for SmallVec<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

pub enum Iter<'a, T> {
    Inline(std::slice::Iter<'a, Option<T>>),
    Heap(std::slice::Iter<'a, T>),
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        match self {
            Iter::Inline(it) => it.next().map(|slot| {
                slot.as_ref()
                    .expect("Inline iterator ranges over buf[..len], all Some")
            }),
            Iter::Heap(it) => it.next(),
        }
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a SmallVec<T, N> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

pub enum IntoIter<T, const N: usize> {
    Inline {
        buf: [Option<T>; N],
        idx: usize,
        len: usize,
    },
    Heap(std::vec::IntoIter<T>),
}

impl<T, const N: usize> Iterator for IntoIter<T, N> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        match self {
            IntoIter::Inline { buf, idx, len } => {
                if *idx < *len {
                    let v = buf[*idx].take().expect("buf[..len] is Some");
                    *idx += 1;
                    Some(v)
                } else {
                    None
                }
            }
            IntoIter::Heap(it) => it.next(),
        }
    }
}

impl<T, const N: usize> IntoIterator for SmallVec<T, N> {
    type Item = T;
    type IntoIter = IntoIter<T, N>;
    fn into_iter(self) -> IntoIter<T, N> {
        match self {
            SmallVec::Inline { buf, len } => IntoIter::Inline { buf, idx: 0, len },
            SmallVec::Heap(v) => IntoIter::Heap(v.into_iter()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_empty_inline() {
        let v: SmallVec<(String, u32), 8> = SmallVec::new();
        assert_eq!(v.len(), 0);
        assert!(v.is_empty());
        assert!(matches!(v, SmallVec::Inline { len: 0, .. }));
        assert_eq!(v.iter().count(), 0);
    }

    #[test]
    fn push_stays_inline_up_to_cap() {
        let mut v: SmallVec<u32, 4> = SmallVec::new();
        for i in 0..4 {
            v.push(i);
        }
        assert_eq!(v.len(), 4);
        assert!(
            matches!(v, SmallVec::Inline { len: 4, .. }),
            "exactly N stays inline"
        );
        let got: Vec<u32> = v.iter().copied().collect();
        assert_eq!(got, vec![0, 1, 2, 3]);
    }

    #[test]
    fn push_spills_to_heap_beyond_cap() {
        let mut v: SmallVec<u32, 4> = SmallVec::new();
        for i in 0..5 {
            v.push(i);
        }
        assert_eq!(v.len(), 5);
        assert!(matches!(v, SmallVec::Heap(_)), "N+1 spills to heap");
        let got: Vec<u32> = v.iter().copied().collect();
        assert_eq!(
            got,
            vec![0, 1, 2, 3, 4],
            "spill preserves order, no dup/loss"
        );
    }

    #[test]
    fn spill_preserves_non_copy_elements() {

        let mut v: SmallVec<(String, u32), 2> = SmallVec::new();
        v.push(("a".into(), 0));
        v.push(("b".into(), 1));
        v.push(("c".into(), 2));
        let got: Vec<(String, u32)> = v.iter().cloned().collect();
        assert_eq!(got, vec![("a".into(), 0), ("b".into(), 1), ("c".into(), 2)]);
    }

    #[test]
    fn iter_find_matches_shapes_lookup() {

        let mut v: SmallVec<(String, u32), 8> = SmallVec::new();
        v.push(("x".into(), 10));
        v.push(("y".into(), 20));
        let hit = v.iter().find(|(n, _)| n == "y").map(|(_, s)| *s);
        assert_eq!(hit, Some(20));
        assert_eq!(v.iter().find(|(n, _)| n == "z").map(|(_, s)| *s), None);
    }

    #[test]
    fn clone_is_independent_inline_and_heap() {
        let mut a: SmallVec<u32, 2> = SmallVec::new();
        a.push(1);
        let mut b = a.clone();
        b.push(2);
        b.push(3);
        assert_eq!(a.iter().copied().collect::<Vec<_>>(), vec![1]);
        assert_eq!(b.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3]);
        assert!(matches!(a, SmallVec::Inline { .. }));
        assert!(matches!(b, SmallVec::Heap(_)));
    }

    #[test]
    fn owned_into_iter_drains_by_value() {

        let mut inline: SmallVec<(String, u32), 4> = SmallVec::new();
        inline.push(("a".into(), 1));
        inline.push(("b".into(), 2));
        let drained: Vec<(String, u32)> = inline.into_iter().collect();
        assert_eq!(drained, vec![("a".into(), 1), ("b".into(), 2)]);

        let mut heap: SmallVec<u32, 2> = SmallVec::new();
        for i in 0..5 {
            heap.push(i);
        }
        assert!(matches!(heap, SmallVec::Heap(_)));
        let drained: Vec<u32> = heap.into_iter().collect();
        assert_eq!(drained, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn into_iter_for_ref() {
        let mut v: SmallVec<u32, 4> = SmallVec::new();
        v.push(7);
        v.push(8);
        let mut sum = 0;
        for x in &v {
            sum += *x;
        }
        assert_eq!(sum, 15);
    }
}
