//! `Map<K, V>` — an open-addressing hash map over the arch allocator.
//!
//! The `HashMap` analog for the zero-std floor. There is no `alloc` crate, so
//! `Map` obtains its bucket array straight from the arch
//! [`allocator`](crate::alloc) and is fallible where growth can fail
//! ([`try_insert`](Map::try_insert) surfaces [`AllocError`]).
//!
//! ## Probing and deletion strategy
//!
//! Open addressing with **linear probing** over a power-of-two bucket array.
//! Power-of-two capacity lets the bucket index be a mask (`hash & (cap - 1)`)
//! rather than a modulo, and keeps the wrap arithmetic a single `& mask`.
//!
//! Deletion uses **backward-shift** (tombstone-free): when a key is removed,
//! the following run of probe-displaced entries is shifted back to close the
//! gap, so a lookup never has to skip a dead marker. This keeps probe chains
//! as short as the live load demands and avoids the tombstone-accretion that
//! degrades a long-lived map's lookups — the floor's maps are long-lived, so
//! the one-time shift cost at delete is the right trade.
//!
//! ## Hasher
//!
//! [`FxHasher`] is an in-repo FxHash-style multiply-xor hasher: each written
//! word is folded in via `hash = (hash.rotate_left(5) ^ word) * SEED`. `SEED`
//! is the widely-published `FxHash` 64-bit constant `0x51_7c_c1_b7_27_22_0a_95`
//! (rustc's `rustc_hash` / Firefox's `FxHash`). It is reimplemented here with no
//! dependency (DAG5) — the constant's provenance is documented, the code is
//! ours. It is not collision-resistant; the floor's keys are trusted internal
//! identifiers, not adversarial input.

use core::{
    alloc::Layout,
    hash::{Hash, Hasher},
    ptr::NonNull,
};

use crate::alloc::{AllocError, alloc, dealloc};

/// The `FxHash` 64-bit seed constant (widely published; rustc `rustc_hash`,
/// Firefox `FxHash`). Reimplemented here, no dependency.
const FX_SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

/// The bucket count a non-empty `Map` first grows to. Power of two so the
/// index is a mask; growth doubles from here.
const FIRST_CAPACITY: usize = 8;

/// An in-repo `FxHash`-style hasher (multiply-xor). Deterministic; not
/// collision-resistant. See the module docs for the seed provenance.
///
/// ```rust
/// use core::hash::{Hash, Hasher as _};
/// use reovim_arch::ds::FxHasher;
///
/// let mut h = FxHasher::default();
/// 42u64.hash(&mut h);
/// let a = h.finish();
///
/// let mut h2 = FxHasher::default();
/// 42u64.hash(&mut h2);
/// assert_eq!(h2.finish(), a); // deterministic
/// ```
#[derive(Default)]
pub struct FxHasher {
    hash: u64,
}

impl FxHasher {
    /// Folds one 64-bit word into the running hash.
    const fn fold(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(FX_SEED);
    }
}

impl Hasher for FxHasher {
    fn finish(&self) -> u64 {
        self.hash
    }

    fn write(&mut self, bytes: &[u8]) {
        // Fold whole 8-byte words, then the trailing tail as one padded word.
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            // Copy into a fixed array: `chunk` is exactly 8 bytes, so this is
            // infallible by construction with no panic/fallback branch.
            let mut word = [0u8; 8];
            word.copy_from_slice(chunk);
            self.fold(u64::from_le_bytes(word));
        }
        let rem = chunks.remainder();
        if !rem.is_empty() {
            let mut tail = [0u8; 8];
            tail[..rem.len()].copy_from_slice(rem);
            self.fold(u64::from_le_bytes(tail));
        }
    }
}

/// One bucket slot: `None` is empty, `Some((k, v))` is an occupied pair.
///
/// Modeled as an `Option` rather than a bespoke enum so lookups extract the
/// value through `Option`'s combinators (`as_ref`/`map`/`take`) — core-library
/// code with no branch in this crate, keeping the map's own MC/DC free of the
/// structurally-unreachable "occupied slot was actually empty" arm an explicit
/// enum match would force.
type Bucket<K, V> = Option<(K, V)>;

/// An open-addressing hash map of `K` to `V`.
///
/// Owns a single allocation of `cap` bucket slots (a power of two), of which
/// `len` are occupied. An empty `Map` holds no allocation, so
/// [`new`](Map::new) never allocates.
///
/// ```rust
/// use reovim_arch::ds::Map;
///
/// let mut m: Map<u32, &str> = Map::new();
/// assert!(m.is_empty());
/// m.try_insert(1, "one").unwrap();
/// m.try_insert(2, "two").unwrap();
/// assert_eq!(m.get(&1), Some(&"one"));
/// assert_eq!(m.get(&3), None);
/// assert_eq!(m.remove(&2), Some("two"));
/// assert_eq!(m.len(), 1);
/// ```
pub struct Map<K, V> {
    ptr: NonNull<Bucket<K, V>>,
    len: usize,
    cap: usize,
}

impl<K, V> Map<K, V> {
    /// The `Layout` of `n` bucket slots. `n` is non-zero at the call sites.
    /// (Unbounded impl: `Drop` needs this without the `Eq + Hash` bounds.)
    fn layout_for(n: usize) -> Result<Layout, AllocError> {
        Layout::array::<Bucket<K, V>>(n).map_err(|_| AllocError)
    }
}

impl<K: Eq + Hash, V> Map<K, V> {
    /// Creates an empty `Map` with no allocation.
    ///
    /// ```rust
    /// use reovim_arch::ds::Map;
    /// let m: Map<u32, u32> = Map::new();
    /// assert!(m.is_empty());
    /// assert_eq!(m.len(), 0);
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ptr: NonNull::dangling(),
            len: 0,
            cap: 0,
        }
    }

    /// The number of stored entries.
    ///
    /// ```rust
    /// use reovim_arch::ds::Map;
    /// let mut m: Map<u32, u32> = Map::new();
    /// assert_eq!(m.len(), 0);
    /// m.try_insert(1, 10).unwrap();
    /// assert_eq!(m.len(), 1);
    /// ```
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the map holds no entries.
    ///
    /// ```rust
    /// use reovim_arch::ds::Map;
    /// let mut m: Map<u32, u32> = Map::new();
    /// assert!(m.is_empty());
    /// m.try_insert(1, 1).unwrap();
    /// assert!(!m.is_empty());
    /// ```
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The hash of `key`. Masking to a bucket index happens via
    /// [`index_of`](Map::index_of).
    fn hash_of(key: &K) -> u64 {
        let mut h = FxHasher::default();
        key.hash(&mut h);
        h.finish()
    }

    /// The bucket index for `key` under the current `mask` (`cap - 1`).
    ///
    /// `hash & (mask as u64)` is `< cap <= usize::MAX`, so the narrowing cast
    /// is lossless; the scoped allow documents that the mask bounds the value
    /// (clippy cannot prove it through the `&`).
    #[allow(clippy::cast_possible_truncation)]
    fn index_of(key: &K, mask: usize) -> usize {
        (Self::hash_of(key) & mask as u64) as usize
    }

    /// The buckets as a slice (empty when unallocated).
    const fn buckets(&self) -> &[Bucket<K, V>] {
        // SAFETY: the first `cap` slots are all initialized (`None` on alloc,
        // `Some` on insert); a `cap == 0` map yields an empty slice from the
        // dangling-but-aligned pointer.
        unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), self.cap) }
    }

    /// The buckets as a mutable slice.
    const fn buckets_mut(&mut self) -> &mut [Bucket<K, V>] {
        // SAFETY: as `buckets`; `&mut self` makes the slice exclusive.
        unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.cap) }
    }

    /// Allocates `cap` (power-of-two) bucket slots, all initialized to the
    /// empty (`None`) state.
    fn alloc_buckets(cap: usize) -> Result<NonNull<Bucket<K, V>>, AllocError> {
        let layout = Self::layout_for(cap)?;
        let raw = alloc(layout)?.cast::<Bucket<K, V>>();
        // Initialize every slot to `None` so reads are always defined.
        for i in 0..cap {
            // SAFETY: `i < cap`, so the slot is within the fresh allocation and
            // uninitialized; a `write` initializes it without dropping garbage.
            unsafe {
                core::ptr::write(raw.as_ptr().add(i), None);
            }
        }
        Ok(raw)
    }

    /// Inserts `key`/`value`, returning the displaced value if `key` was
    /// present.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError`] when a needed growth allocation is refused; on
    /// error the map is unchanged.
    ///
    /// ```rust
    /// use reovim_arch::ds::Map;
    /// let mut m: Map<u32, u32> = Map::new();
    /// assert_eq!(m.try_insert(1, 10).unwrap(), None);
    /// // Re-inserting the same key returns the old value.
    /// assert_eq!(m.try_insert(1, 20).unwrap(), Some(10));
    /// assert_eq!(m.get(&1), Some(&20));
    /// ```
    pub fn try_insert(&mut self, key: K, value: V) -> Result<Option<V>, AllocError> {
        // Grow before insert when at or above the 7/8 load-factor threshold,
        // counting the new entry. `(len + 1) * 8 > cap * 7` is the integer form
        // of `(len + 1) / cap > 7/8`.
        if self.cap == 0 || (self.len + 1) * 8 > self.cap * 7 {
            self.grow()?;
        }
        Ok(self.insert_into_buckets(key, value))
    }

    /// Inserts into the current (sufficiently-sized) bucket array. Linear
    /// probe to the key or the first empty slot.
    fn insert_into_buckets(&mut self, key: K, value: V) -> Option<V> {
        let mask = self.cap - 1;
        let mut idx = Self::index_of(&key, mask);
        let buckets = self.buckets_mut();
        loop {
            match &mut buckets[idx] {
                None => {
                    buckets[idx] = Some((key, value));
                    self.len += 1;
                    return None;
                }
                Some((k, v)) if *k == key => {
                    return Some(core::mem::replace(v, value));
                }
                Some(_) => {
                    idx = (idx + 1) & mask;
                }
            }
        }
    }

    /// Doubles the bucket array (initial [`FIRST_CAPACITY`]) and rehashes.
    ///
    /// On allocation failure the map is left unchanged (the new array is the
    /// only thing that fails, before any old entry is moved).
    fn grow(&mut self) -> Result<(), AllocError> {
        let new_cap = if self.cap == 0 {
            FIRST_CAPACITY
        } else {
            self.cap * 2
        };
        let new_ptr = Self::alloc_buckets(new_cap)?;
        let old_ptr = self.ptr;
        let old_cap = self.cap;
        // Swap in the empty larger array; reinsert each old entry.
        self.ptr = new_ptr;
        self.cap = new_cap;
        self.len = 0;
        for i in 0..old_cap {
            // SAFETY: `i < old_cap`, so the old slot is initialized; reading it
            // out by value moves the bucket, leaving the old slot logically
            // dead (the old array is freed below without re-dropping).
            let bucket = unsafe { core::ptr::read(old_ptr.as_ptr().add(i)) };
            if let Some((k, v)) = bucket {
                self.insert_into_buckets(k, v);
            }
        }
        if old_cap != 0 {
            let old_layout = Self::layout_for(old_cap).expect("layout valid at alloc time");
            // SAFETY: `old_ptr`/`old_layout` name the prior live allocation; its
            // entries were moved out above, so no element is double-dropped.
            unsafe {
                dealloc(old_ptr.cast(), old_layout);
            }
        }
        Ok(())
    }

    /// Returns a reference to the value for `key`, or `None`.
    ///
    /// ```rust
    /// use reovim_arch::ds::Map;
    /// let mut m: Map<u32, &str> = Map::new();
    /// m.try_insert(5, "five").unwrap();
    /// assert_eq!(m.get(&5), Some(&"five"));
    /// assert_eq!(m.get(&6), None);
    /// ```
    #[must_use]
    pub fn get(&self, key: &K) -> Option<&V> {
        let idx = self.probe_find(key)?;
        // The slot is occupied (`probe_find` only returns occupied indices);
        // `Option::as_ref().map` extracts the value via core combinators.
        self.buckets()[idx].as_ref().map(|(_, v)| v)
    }

    /// Returns a mutable reference to the value for `key`, or `None`.
    ///
    /// ```rust
    /// use reovim_arch::ds::Map;
    /// let mut m: Map<u32, u32> = Map::new();
    /// m.try_insert(1, 10).unwrap();
    /// if let Some(v) = m.get_mut(&1) { *v = 99; }
    /// assert_eq!(m.get(&1), Some(&99));
    /// ```
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let idx = self.probe_find(key)?;
        self.buckets_mut()[idx].as_mut().map(|(_, v)| v)
    }

    /// Probes for `key`, returning its bucket index, or `None` if absent.
    fn probe_find(&self, key: &K) -> Option<usize> {
        if self.cap == 0 {
            return None;
        }
        let mask = self.cap - 1;
        let mut idx = Self::index_of(key, mask);
        let buckets = self.buckets();
        loop {
            match &buckets[idx] {
                // An empty slot ends the probe: a present key would have landed
                // at or before it (backward-shift keeps chains gap-free).
                None => return None,
                Some((k, _)) if k == key => return Some(idx),
                Some(_) => idx = (idx + 1) & mask,
            }
        }
    }

    /// Removes `key`, returning its value if present.
    ///
    /// Uses backward-shift to keep probe chains gap-free (no tombstones).
    ///
    /// # Panics
    ///
    /// Panics only if the map's internal probe invariant is violated
    /// (`probe_find` returned an index whose slot is empty) — unreachable
    /// short of a bug in this module.
    ///
    /// ```rust
    /// use reovim_arch::ds::Map;
    /// let mut m: Map<u32, &str> = Map::new();
    /// m.try_insert(1, "one").unwrap();
    /// assert_eq!(m.remove(&1), Some("one"));
    /// assert_eq!(m.remove(&1), None);
    /// ```
    pub fn remove(&mut self, key: &K) -> Option<V> {
        // Probe for the occupied index, then take the value out of that slot.
        // `probe_find` only returns occupied indices, so the slot is `Some`;
        // `expect` matches the crate's established pattern for a value known
        // valid by a prior check (cf. `Seq`'s `layout_for(...).expect(...)`).
        let idx = self.probe_find(key)?;
        let mask = self.cap - 1;
        self.len -= 1;
        let buckets = self.buckets_mut();
        let (_, removed) = buckets[idx]
            .take()
            .expect("probe_find located an occupied slot");
        // Backward-shift: walk the run after the hole; any entry whose ideal
        // slot is at or before the hole must move into the hole to keep the
        // probe chain contiguous.
        let mut hole = idx;
        let mut scan = (idx + 1) & mask;
        while let Some((k, _)) = &buckets[scan] {
            let ideal = Self::index_of(k, mask);
            // `ideal` stays put when the entry can still be found by probing
            // from `hole` through `scan`: i.e. `ideal` is strictly inside the
            // open-start arc `(hole, scan]` (cyclically). Otherwise it shifts
            // into the hole to keep the probe chain contiguous.
            if !cyclic_in_range(hole, scan, ideal) {
                buckets[hole] = buckets[scan].take();
                hole = scan;
            }
            scan = (scan + 1) & mask;
        }
        Some(removed)
    }

    /// Iterates the stored entries by reference, in bucket order.
    ///
    /// ```rust
    /// use reovim_arch::ds::Map;
    /// let mut m: Map<u32, u32> = Map::new();
    /// m.try_insert(1, 10).unwrap();
    /// m.try_insert(2, 20).unwrap();
    /// let mut pairs: Vec<(u32, u32)> = m.iter().map(|(&k, &v)| (k, v)).collect();
    /// pairs.sort();
    /// assert_eq!(pairs, vec![(1, 10), (2, 20)]);
    /// ```
    #[must_use]
    pub const fn iter(&self) -> MapIter<'_, K, V> {
        MapIter {
            buckets: self.buckets(),
            pos: 0,
        }
    }
}

/// Whether `x` lies in the open-start, closed-end cyclic arc `(hole, scan]`.
///
/// Used by backward-shift to decide whether an entry must stay put: an entry
/// whose ideal slot is strictly between the hole and its current slot can
/// still be found from the hole, so it stays; otherwise it shifts into the
/// hole.
const fn cyclic_in_range(hole: usize, scan: usize, x: usize) -> bool {
    if hole < scan {
        x > hole && x <= scan
    } else {
        // The arc wraps past the end of the array.
        x > hole || x <= scan
    }
}

/// Iterator over a [`Map`]'s `(&K, &V)` entries, in bucket order.
///
/// See [`Map::iter`] for usage.
pub struct MapIter<'a, K, V> {
    buckets: &'a [Bucket<K, V>],
    pos: usize,
}

impl<'a, K, V> Iterator for MapIter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        while self.pos < self.buckets.len() {
            let i = self.pos;
            self.pos += 1;
            if let Some((k, v)) = &self.buckets[i] {
                return Some((k, v));
            }
        }
        None
    }
}

impl<K: Eq + Hash, V> Default for Map<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, K: Eq + Hash, V> IntoIterator for &'a Map<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = MapIter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<K, V> Drop for Map<K, V> {
    fn drop(&mut self) {
        if self.cap == 0 {
            return;
        }
        // Drop every initialized bucket (each is Empty or Full, both Drop-safe
        // as an enum); then free the array.
        // SAFETY: the first `cap` slots are initialized; dropping the slice in
        // place runs each bucket's destructor exactly once.
        unsafe {
            core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(
                self.ptr.as_ptr(),
                self.cap,
            ));
        }
        let layout = Self::layout_for(self.cap).expect("layout valid at alloc time");
        // SAFETY: `self.ptr`/`layout` name the live allocation from the last
        // grow; no bucket is referenced after the drops above.
        unsafe {
            dealloc(self.ptr.cast(), layout);
        }
    }
}

// L12 layout (#785 Phase 5): tests live in the sibling file `map_tests.rs`,
// declared as a `#[path]` child so `super::` reaches `cyclic_in_range` and
// `FIRST_CAPACITY`.
#[cfg(feature = "selftest")]
#[path = "map_tests.rs"]
mod tests;
