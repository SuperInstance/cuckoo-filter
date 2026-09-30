//! A probabilistic data structure for approximate set membership testing,
//! similar to Bloom filters but supports deletion and has better space efficiency.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::marker::PhantomData;

const BUCKET_SIZE: usize = 4;
const MAX_KICKS: usize = 500;
const FINGERPRINT_BITS: u8 = 8;

#[derive(Debug, Clone)]
struct Bucket {
    fingerprints: Vec<Option<u8>>,
}

impl Bucket {
    fn new() -> Self {
        let fingerprints = (0..BUCKET_SIZE).map(|_| None).collect();
        Self { fingerprints }
    }

    fn insert(&mut self, fp: u8) -> bool {
        for slot in &mut self.fingerprints {
            if slot.is_none() {
                *slot = Some(fp);
                return true;
            }
        }
        false
    }

    fn contains(&self, fp: u8) -> bool {
        self.fingerprints.iter().any(|s| *s == Some(fp))
    }

    fn remove(&mut self, fp: u8) -> bool {
        for slot in &mut self.fingerprints {
            if *slot == Some(fp) {
                *slot = None;
                return true;
            }
        }
        false
    }

    fn is_full(&self) -> bool {
        self.fingerprints.iter().all(|s| s.is_some())
    }

    fn swap(&mut self, fp: u8) -> u8 {
        let idx = (fp as usize) % BUCKET_SIZE;
        let old = self.fingerprints[idx].replace(fp);
        old.unwrap_or(fp)
    }
}

#[derive(Debug)]
pub struct CuckooFilter<T: Hash> {
    buckets: Vec<Bucket>,
    capacity: usize,
    len: usize,
    _marker: PhantomData<T>,
}

impl<T: Hash> CuckooFilter<T> {
    pub fn new(capacity: usize) -> Self {
        let num_buckets = (capacity + BUCKET_SIZE - 1) / BUCKET_SIZE;
        let buckets = (0..num_buckets).map(|_| Bucket::new()).collect();
        Self {
            buckets,
            capacity: num_buckets * BUCKET_SIZE,
            len: 0,
            _marker: PhantomData,
        }
    }

    fn fingerprint(data: &T) -> u8 {
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        let h = hasher.finish();
        let fp = ((h >> 32) ^ h) as u8;
        if fp == 0 { 1 } else { fp }
    }

    fn index(&self, data: &T) -> usize {
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        (hasher.finish() as usize) % self.buckets.len()
    }

    fn alt_index(&self, idx: usize, fp: u8) -> usize {
        let mut hasher = DefaultHasher::new();
        fp.hash(&mut hasher);
        (idx ^ (hasher.finish() as usize)) % self.buckets.len()
    }

    pub fn insert(&mut self, data: &T) -> bool {
        let fp = Self::fingerprint(data);
        let i1 = self.index(data);
        let i2 = self.alt_index(i1, fp);

        if self.buckets[i1].insert(fp) {
            self.len += 1;
            return true;
        }
        if self.buckets[i2].insert(fp) {
            self.len += 1;
            return true;
        }

        // Must kick out existing entries
        let mut idx = if rand_fp() { i1 } else { i2 };
        let mut current_fp = fp;
        for _ in 0..MAX_KICKS {
            current_fp = self.buckets[idx].swap(current_fp);
            idx = self.alt_index(idx, current_fp);
            if self.buckets[idx].insert(current_fp) {
                self.len += 1;
                return true;
            }
        }
        false
    }

    pub fn contains(&self, data: &T) -> bool {
        let fp = Self::fingerprint(data);
        let i1 = self.index(data);
        let i2 = self.alt_index(i1, fp);
        self.buckets[i1].contains(fp) || self.buckets[i2].contains(fp)
    }

    pub fn remove(&mut self, data: &T) -> bool {
        let fp = Self::fingerprint(data);
        let i1 = self.index(data);
        let i2 = self.alt_index(i1, fp);
        if self.buckets[i1].remove(fp) || self.buckets[i2].remove(fp) {
            self.len -= 1;
            return true;
        }
        false
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn load_factor(&self) -> f64 {
        self.len as f64 / self.capacity as f64
    }
}

fn rand_fp() -> bool {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos()
        % 2
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_lookup() {
        let mut cf: CuckooFilter<&str> = CuckooFilter::new(1000);
        assert!(cf.insert(&"hello"));
        assert!(cf.contains(&"hello"));
        assert!(!cf.contains(&"world"));
    }

    #[test]
    fn test_delete() {
        let mut cf: CuckooFilter<&str> = CuckooFilter::new(1000);
        cf.insert(&"test");
        assert!(cf.remove(&"test"));
        assert!(!cf.contains(&"test"));
    }
}

/// FNV-1a 64 — the digest every substrate in the SuperInstance fleet agrees on.
pub const FNV_OFFSET: u64 = 0xcbf29ce484222325;
pub const FNV_PRIME: u64 = 0x100000001b3;

#[inline]
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h = FNV_OFFSET;
    for &b in bytes {
        h = (h ^ b as u64).wrapping_mul(FNV_PRIME);
    }
    h
}

/// True if this crate's FNV-1a still agrees with the rest of the fleet.
pub fn canary_holds() -> bool {
    fnv1a64("café Δ 日本語".as_bytes()) == 0x024a555471370b18d
}
