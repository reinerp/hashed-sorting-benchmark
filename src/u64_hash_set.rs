//! A dense_hash_set for u64 keys.
//! 
//! Compared to std::collections::HashSet<u64>, this uses a different layout: no metadata table, just plain data.
//! This is similar to Google's dense_hash_map, which predates the SwissTable design. By avoiding a metadata table,
//! we may need to do longer probe sequences (each probe is 8 bytes, not 1 byte), but on the other hand we only take
//! 1 cache miss per access, not 2.

use crate::hashers::StatelessU64Hasher;

pub struct U64HashSet<H: StatelessU64Hasher> {
    table: Box<[Bucket]>,
    len: usize,
    marker: std::marker::PhantomData<H>,
    has_zero: bool,
}

const BUCKET_SIZE: usize = 8;

#[derive(Clone, Copy)]
#[repr(align(64))] // Cache line alignment
struct Bucket([u64; BUCKET_SIZE]);

impl<H: StatelessU64Hasher> U64HashSet<H> {
    pub fn with_capacity(capacity: usize) -> Self {
        // TODO: integer overflow...
        let num_buckets = (capacity.next_power_of_two() * 2).div_ceil(BUCKET_SIZE);
        let table = vec![Bucket([0u64; BUCKET_SIZE]); num_buckets].into_boxed_slice();
        Self {
            table,
            len: 0,
            marker: std::marker::PhantomData,
            has_zero: false,
        }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len + self.has_zero as usize
    }

    #[inline(always)]
    pub fn insert(&mut self, key: u64) {
        if key == 0 {
            self.len += !self.has_zero as usize;
            self.has_zero = true;
            return;
        }
        let hash64 = H::hash(key);
        let bucket_mask = self.table.len() - 1;
        let element_offset_in_bucket = (hash64 >> 61) as usize;
        let mut bucket_i = hash64 as usize;


        loop {
            // Safety: bucket_mask is correct because the number of buckets is a power of 2.
            let bucket = unsafe { self.table.get_unchecked_mut(bucket_i & bucket_mask) };
            for element_i in 0..BUCKET_SIZE {
                let element = &mut bucket.0[(element_i + element_offset_in_bucket) % BUCKET_SIZE];
                if *element == 0 {
                    *element = key;
                    self.len += 1;
                    return;
                }
                if *element == key {
                    return;
                }
            }
            bucket_i += 1;
        }
    }
}
