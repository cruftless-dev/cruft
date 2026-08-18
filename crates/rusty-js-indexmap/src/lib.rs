
use std::borrow::Borrow;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::Index;

const INDEX_THRESHOLD: usize = 16;

#[inline]
fn key_hash<Q: Hash + ?Sized>(q: &Q) -> u64 {
    let mut h = DefaultHasher::new();
    q.hash(&mut h);
    h.finish()
}

#[derive(Clone, Debug)]
pub struct IndexMap<K, V> {
    entries: Vec<(K, V)>,

    index: Option<std::collections::HashMap<u64, Vec<usize>>>,
}

impl<K: PartialEq, V: PartialEq> PartialEq for IndexMap<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}
impl<K: Eq, V: Eq> Eq for IndexMap<K, V> {}

impl<K, V> IndexMap<K, V> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: None,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            index: None,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            inner: self.entries.iter(),
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        IterMut {
            inner: self.entries.iter_mut(),
        }
    }

    pub fn keys(&self) -> Keys<'_, K, V> {
        Keys { inner: self.iter() }
    }

    pub fn values(&self) -> Values<'_, K, V> {
        Values { inner: self.iter() }
    }

    pub fn values_mut(&mut self) -> ValuesMut<'_, K, V> {
        ValuesMut {
            inner: self.iter_mut(),
        }
    }

    pub fn get_index(&self, index: usize) -> Option<(&K, &V)> {
        self.entries.get(index).map(|(k, v)| (k, v))
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index = None;
    }

    pub fn retain<F>(&mut self, mut keep: F)
    where
        F: FnMut(&K, &mut V) -> bool,
    {
        self.entries.retain_mut(|(k, v)| keep(k, v));

        self.index = None;
    }
}

impl<K: Eq + Hash, V> IndexMap<K, V> {

    fn scan_pos<Q>(&self, key: &Q) -> Option<usize>
    where
        K: Borrow<Q>,
        Q: Eq + ?Sized,
    {
        self.entries.iter().position(|(k, _)| k.borrow() == key)
    }

    fn build_index(&mut self) {
        let mut idx: std::collections::HashMap<u64, Vec<usize>> =
            std::collections::HashMap::with_capacity(self.entries.len());
        for (i, (k, _)) in self.entries.iter().enumerate() {
            idx.entry(key_hash(k)).or_default().push(i);
        }
        self.index = Some(idx);
    }

    #[inline]
    fn ensure_index(&mut self) {
        if self.index.is_none() && self.entries.len() >= INDEX_THRESHOLD {
            self.build_index();
        }
    }

    fn find_pos<Q>(&self, key: &Q) -> Option<usize>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        if let Some(idx) = &self.index {
            let cands = idx.get(&key_hash(key))?;
            for &i in cands {
                if self.entries[i].0.borrow() == key {
                    return Some(i);
                }
            }
            return None;
        }
        self.scan_pos(key)
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if let Some(pos) = self.find_pos(&key) {
            return Some(std::mem::replace(&mut self.entries[pos].1, value));
        }
        self.entries.push((key, value));
        let i = self.entries.len() - 1;
        if let Some(idx) = &mut self.index {
            idx.entry(key_hash(&self.entries[i].0)).or_default().push(i);
        } else {
            self.ensure_index();
        }
        None
    }

    pub fn entry(&mut self, key: K) -> Entry<'_, K, V> {
        match self.find_pos(&key) {
            Some(index) => Entry::Occupied(OccupiedEntry { map: self, index }),
            None => Entry::Vacant(VacantEntry { map: self, key }),
        }
    }

    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.find_pos(key).is_some()
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.find_pos(key).map(|i| &self.entries[i].1)
    }

    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        let pos = self.find_pos(key)?;
        Some(&mut self.entries[pos].1)
    }

    pub fn shift_remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        let index = self.find_pos(key)?;
        let removed = self.entries.remove(index).1;

        if let Some(idx) = &mut self.index {
            if self.entries.len() < INDEX_THRESHOLD {

                self.index = None;
            } else {
                idx.retain(|_, positions| {
                    positions.retain(|&p| p != index);
                    for p in positions.iter_mut() {
                        if *p > index {
                            *p -= 1;
                        }
                    }
                    !positions.is_empty()
                });
            }
        }
        Some(removed)
    }

    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.shift_remove(key)
    }
}

impl<K, V> Default for IndexMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

pub enum Entry<'a, K, V> {
    Occupied(OccupiedEntry<'a, K, V>),
    Vacant(VacantEntry<'a, K, V>),
}

impl<'a, K: Eq + Hash, V> Entry<'a, K, V> {
    pub fn or_insert(self, default: V) -> &'a mut V {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default),
        }
    }

    pub fn or_insert_with<F>(self, default: F) -> &'a mut V
    where
        F: FnOnce() -> V,
    {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default()),
        }
    }
}

pub struct OccupiedEntry<'a, K, V> {
    map: &'a mut IndexMap<K, V>,
    index: usize,
}

impl<'a, K, V> OccupiedEntry<'a, K, V> {
    pub fn get_mut(&mut self) -> &mut V {
        &mut self.map.entries[self.index].1
    }

    pub fn into_mut(self) -> &'a mut V {
        &mut self.map.entries[self.index].1
    }

    pub fn insert(self, value: V) -> V {
        std::mem::replace(&mut self.map.entries[self.index].1, value)
    }
}

pub struct VacantEntry<'a, K, V> {
    map: &'a mut IndexMap<K, V>,
    key: K,
}

impl<'a, K: Eq + Hash, V> VacantEntry<'a, K, V> {
    pub fn insert(self, value: V) -> &'a mut V {
        self.map.entries.push((self.key, value));
        let index = self.map.entries.len() - 1;

        if let Some(idx) = &mut self.map.index {
            idx.entry(key_hash(&self.map.entries[index].0))
                .or_default()
                .push(index);
        } else {
            self.map.ensure_index();
        }
        &mut self.map.entries[index].1
    }
}

pub struct Iter<'a, K, V> {
    inner: std::slice::Iter<'a, (K, V)>,
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(k, v)| (k, v))
    }
}

pub struct IterMut<'a, K, V> {
    inner: std::slice::IterMut<'a, (K, V)>,
}

impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(k, v)| (&*k, v))
    }
}

pub struct Keys<'a, K, V> {
    inner: Iter<'a, K, V>,
}

impl<'a, K, V> Iterator for Keys<'a, K, V> {
    type Item = &'a K;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(k, _)| k)
    }
}

pub struct Values<'a, K, V> {
    inner: Iter<'a, K, V>,
}

impl<'a, K, V> Iterator for Values<'a, K, V> {
    type Item = &'a V;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(_, v)| v)
    }
}

pub struct ValuesMut<'a, K, V> {
    inner: IterMut<'a, K, V>,
}

impl<'a, K, V> Iterator for ValuesMut<'a, K, V> {
    type Item = &'a mut V;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(_, v)| v)
    }
}

impl<'a, K, V> IntoIterator for &'a IndexMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<K, V> IntoIterator for IndexMap<K, V> {
    type Item = (K, V);
    type IntoIter = std::vec::IntoIter<(K, V)>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<K: Eq + std::hash::Hash, V> FromIterator<(K, V)> for IndexMap<K, V> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let mut map = Self::new();
        for (key, value) in iter {
            map.insert(key, value);
        }
        map
    }
}

impl<K, Q, V> Index<&Q> for IndexMap<K, V>
where
    K: Borrow<Q> + Eq + std::hash::Hash,
    Q: Eq + std::hash::Hash + ?Sized,
{
    type Output = V;

    fn index(&self, index: &Q) -> &Self::Output {
        self.get(index).expect("index not found")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndexSet<T> {
    map: IndexMap<T, ()>,
}

impl<T: Eq + std::hash::Hash> IndexSet<T> {
    pub fn new() -> Self {
        Self {
            map: IndexMap::new(),
        }
    }

    pub fn insert(&mut self, value: T) -> bool {
        self.map.insert(value, ()).is_none()
    }

    pub fn contains<Q>(&self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Eq + std::hash::Hash + ?Sized,
    {
        self.map.contains_key(value)
    }

    pub fn shift_remove<Q>(&mut self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Eq + std::hash::Hash + ?Sized,
    {
        self.map.shift_remove(value).is_some()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn iter(&self) -> Keys<'_, T, ()> {
        self.map.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::{Hash, Hasher};

    #[derive(Debug, Eq, PartialEq)]
    struct CollidingKey(&'static str);

    impl Hash for CollidingKey {
        fn hash<H: Hasher>(&self, state: &mut H) {
            0xfeed_face_cafe_beefu64.hash(state);
        }
    }

    #[test]
    fn map_preserves_insertion_order() {
        let mut map = IndexMap::new();
        map.insert("b", 2);
        map.insert("a", 1);
        map.insert("c", 3);
        let keys: Vec<_> = map.keys().copied().collect();
        assert_eq!(keys, vec!["b", "a", "c"]);
        let values: Vec<_> = map.values().copied().collect();
        assert_eq!(values, vec![2, 1, 3]);
    }

    #[test]
    fn hash_index_engages_past_threshold_preserving_order_and_lookups() {

        let mut map = IndexMap::new();
        for i in 0..200 {
            assert_eq!(map.insert(format!("k{i}"), i), None);
        }
        assert!(map.index.is_some(), "index should be built past threshold");
        assert_eq!(map.len(), 200);
        for i in 0..200 {
            assert_eq!(map.get(&format!("k{i}")), Some(&i));
        }
        assert_eq!(map.get("missing"), None);
        assert_eq!(map.insert("k100".to_string(), 999), Some(100));

        assert_eq!(map.get_index(0).map(|(k, _)| k.as_str()), Some("k0"));
        assert_eq!(
            map.get_index(100).map(|(k, v)| (k.as_str(), *v)),
            Some(("k100", 999))
        );
    }

    #[test]
    fn hash_index_uses_verified_candidates_under_collisions() {
        let mut map = IndexMap::new();
        for i in 0..64 {
            let key = Box::leak(format!("k{i}").into_boxed_str());
            assert_eq!(map.insert(CollidingKey(key), i), None);
        }
        assert!(map.index.is_some());
        assert_eq!(map.get(&CollidingKey("k17")), Some(&17));
        assert_eq!(map.get(&CollidingKey("missing")), None);
        assert_eq!(map.insert(CollidingKey("k17"), 1700), Some(17));
        assert_eq!(map.get(&CollidingKey("k17")), Some(&1700));
    }

    #[test]
    fn hash_index_stays_correct_across_removes() {

        let mut map = IndexMap::new();
        for i in 0..100 {
            map.insert(format!("k{i}"), i);
        }
        assert_eq!(map.shift_remove("k10"), Some(10));
        assert_eq!(map.shift_remove("k50"), Some(50));
        assert_eq!(map.get("k10"), None);
        assert_eq!(map.get("k50"), None);

        for i in (0..100).filter(|i| *i != 10 && *i != 50) {
            assert_eq!(map.get(&format!("k{i}")), Some(&i), "survivor k{i}");
        }
        assert_eq!(map.len(), 98);
    }

    #[test]
    fn incremental_shift_remove_survives_interleaved_inserts_and_threshold_drop() {

        let mut map = IndexMap::new();
        for i in 0..40 {
            map.insert(format!("k{i}"), i);
        }
        assert!(map.index.is_some());

        for k in ["k5", "k20", "k39"] {
            assert!(map.shift_remove(k).is_some());
        }

        for i in 100..105 {
            assert_eq!(map.insert(format!("k{i}"), i), None);
        }
        for i in (0..40).filter(|i| ![5, 20, 39].contains(i)) {
            assert_eq!(map.get(&format!("k{i}")), Some(&i), "old survivor k{i}");
        }
        for i in 100..105 {
            assert_eq!(map.get(&format!("k{i}")), Some(&i), "new key k{i}");
        }
        assert_eq!(map.get("k5"), None);

        let live: Vec<String> = map.keys().cloned().collect();
        for k in live.iter().take(live.len() - 8) {
            map.shift_remove(k.as_str());
        }
        assert!(map.index.is_none(), "index dropped below threshold");
        assert_eq!(map.len(), 8);
        for k in live.iter().skip(live.len() - 8) {
            assert!(map.get(k.as_str()).is_some(), "post-drop scan finds {k}");
        }
    }

    #[test]
    fn overwrite_preserves_original_order() {
        let mut map = IndexMap::new();
        assert_eq!(map.insert("a", 1), None);
        assert_eq!(map.insert("b", 2), None);
        assert_eq!(map.insert("a", 10), Some(1));
        let entries: Vec<_> = map.iter().map(|(k, v)| (*k, *v)).collect();
        assert_eq!(entries, vec![("a", 10), ("b", 2)]);
    }

    #[test]
    fn shift_remove_compacts_without_reordering_survivors() {
        let mut map = IndexMap::new();
        map.insert("a".to_string(), 1);
        map.insert("b".to_string(), 2);
        map.insert("c".to_string(), 3);
        assert_eq!(map.shift_remove("b"), Some(2));
        let entries: Vec<_> = map.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        assert_eq!(entries, vec![("a", 1), ("c", 3)]);
    }

    #[test]
    fn entry_or_insert_only_appends_vacant_keys() {
        let mut map = IndexMap::new();
        *map.entry("a").or_insert(1) = 2;
        map.entry("b").or_insert(3);
        map.entry("a").or_insert(9);
        let entries: Vec<_> = map.iter().map(|(k, v)| (*k, *v)).collect();
        assert_eq!(entries, vec![("a", 2), ("b", 3)]);
    }

    #[test]
    fn borrowed_lookup_supports_string_keys() {
        let mut map = IndexMap::new();
        map.insert("alpha".to_string(), 7);
        assert!(map.contains_key("alpha"));
        assert_eq!(map.get("alpha"), Some(&7));
        *map.get_mut("alpha").unwrap() = 8;
        assert_eq!(map.get("alpha"), Some(&8));
    }

    #[test]
    fn values_mut_updates_in_order() {
        let mut map = IndexMap::new();
        map.insert("a", 1);
        map.insert("b", 2);
        for value in map.values_mut() {
            *value *= 10;
        }
        let values: Vec<_> = map.values().copied().collect();
        assert_eq!(values, vec![10, 20]);
    }

    #[test]
    fn set_preserves_unique_insertion_order() {
        let mut set = IndexSet::new();
        assert!(set.insert("b"));
        assert!(set.insert("a"));
        assert!(!set.insert("b"));
        assert!(set.contains("a"));
        let values: Vec<_> = set.iter().copied().collect();
        assert_eq!(values, vec!["b", "a"]);
    }

    #[test]
    fn retained_clear_collect_index_and_get_index_match_consumed_runtime_mouth() {
        let mut map: IndexMap<String, i32> = [
            ("a".to_string(), 1),
            ("b".to_string(), 2),
            ("c".to_string(), 3),
        ]
        .into_iter()
        .collect();
        assert_eq!(
            map.get_index(1).map(|(k, v)| (k.as_str(), *v)),
            Some(("b", 2))
        );
        assert_eq!(map["b"], 2);
        map.retain(|key, value| {
            *value *= 10;
            key != "b"
        });
        let entries: Vec<_> = map.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        assert_eq!(entries, vec![("a", 10), ("c", 30)]);
        map.clear();
        assert!(map.is_empty());
    }

    #[test]
    fn entry_or_insert_with_is_lazy_for_occupied_key() {
        let mut map = IndexMap::new();
        map.insert("a", 1);
        let mut called = false;
        let value = map.entry("a").or_insert_with(|| {
            called = true;
            2
        });
        assert_eq!(*value, 1);
        assert!(!called);
    }
}
