# Cuckoo Filter

**A probabilistic data structure for approximate set membership testing** that supports both insertion *and* deletion — overcoming the key limitation of Bloom filters. It uses cuckoo hashing to achieve 95%+ space efficiency while providing O(1) amortized operations.

## Why It Matters

Bloom filters answer "is this item in the set?" using sub-linear space, but they have a fatal flaw: **no deletion**. Once an item is inserted, it can't be removed without rebuilding the entire filter. This makes Bloom filters unsuitable for dynamic membership sets (e.g., active connections, cache entries that expire).

The Cuckoo Filter, introduced by Fan et al. (2014), solves this by storing small *fingerprints* (partial hashes) in a cuckoo hash table rather than setting bits in a bit array. Because fingerprints are stored as discrete entries, they can be individually removed.

**Performance comparison:**
- **Lookup:** O(1) worst-case (check exactly 2 buckets)
- **Insert:** O(1) amortized (O(1) expected, rarely triggers displacement chain)
- **Delete:** O(1) (check 2 buckets, remove fingerprint)
- **Space:** ~12 bits per item at 95% load factor (vs. ~10 bits for Bloom, but Bloom can't delete)

**Real-world usage:** Google's open-source implementations, Cassandra 4.0's read path (replacing Bloom filters for some workloads), network filtering rules, and cache invalidation systems.

## How It Works

The data structure is a table of *buckets*, each holding up to `BUCKET_SIZE = 4` entries. Each entry is an 8-bit *fingerprint* — a partial hash of the item.

**Insertion (cuckoo hashing):**
1. Compute the item's fingerprint `fp` and its primary index `i1 = hash(item)`.
2. Compute a secondary index `i2 = i1 ⊕ hash(fp)` using partial-key cuckoo hashing.
3. Try inserting `fp` into bucket `i1`. If full, try `i2`.
4. If both are full, **kick** (evict) a random entry from one bucket, place `fp` there, then try to re-insert the evicted fingerprint in its alternate location.
5. Repeat up to `MAX_KICKS = 500` times. If exceeded, the filter is full.

**The XOR trick:** The alternate index is computed as `i2 = i1 ⊕ hash(fp)`. Because XOR is its own inverse, `i1 ⊕ hash(fp) ⊕ hash(fp) = i1`. This means given *either* index and the fingerprint, you can compute the *other* index — no need to store the original item or re-hash it.

**Lookup:** Compute `fp`, `i1`, `i2`. Check if `fp` exists in either bucket's fingerprint array. O(1) — exactly two bucket checks.

**Deletion:** Compute `fp`, `i1`, `i2`. Remove `fp` from whichever bucket contains it. O(1). This is correct (no false negatives) *if* the item was actually inserted — deleting an item that was never inserted has a small false-positive rate.

**False positive rate:** With 8-bit fingerprints and 4 entries per bucket, the false positive rate is approximately 0.03% at 95% load factor. Longer fingerprints reduce this further.

## Quick Start

```rust
use cuckoo_filter::CuckooFilter;

let mut filter: CuckooFilter<&str> = CuckooFilter::new(10_000);

// Insert items
filter.insert(&"alice@example.com");
filter.insert(&"bob@example.com");

// Membership test
assert!(filter.contains(&"alice@example.com"));  // definitely yes
assert!(!filter.contains(&"charlie@example.com")); // almost certainly no

// Delete — the operation Bloom filters can't do!
filter.remove(&"alice@example.com");
assert!(!filter.contains(&"alice@example.com"));

println!("Load factor: {:.1}%", filter.load_factor() * 100.0);
```

## API

### `CuckooFilter<T: Hash>`
- `new(capacity: usize) -> Self` — Create a filter sized for `capacity` items
- `insert(&mut self, data: &T) -> bool` — Insert item. Returns `false` if filter is full. O(1) amortized
- `contains(&self, data: &T) -> bool` — Check membership (may have false positives, never false negatives). O(1)
- `remove(&mut self, data: &T) -> bool` — Remove item. Returns `false` if not found. O(1)
- `len(&self) -> usize` — Number of items stored
- `capacity(&self) -> usize` — Maximum items before forced eviction fails
- `load_factor(&self) -> f64` — Current utilization (0.0–1.0)
- `is_empty(&self) -> bool` — Whether no items are stored

## Architecture Notes

Used in SuperInstance's data infrastructure for dynamic set membership — tracking active sessions, deduplicating messages, and managing cache invalidation where entries must be both added and removed. Complements the Count-Min Sketch (frequency estimation) and Bloom filter concepts.

See the full architecture: [ARCHITECTURE.md](https://github.com/SuperInstance/SuperInstance/blob/main/ARCHITECTURE.md)

## License

MIT
