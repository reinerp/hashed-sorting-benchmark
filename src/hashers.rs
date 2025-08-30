pub trait StatelessU64Hasher {
    fn hash(value: u64) -> u64;
}

pub struct NoopHasher;

impl StatelessU64Hasher for NoopHasher {
    #[inline(always)]
    fn hash(value: u64) -> u64 {
        value
    }
}

pub struct MurmurHasher;

impl MurmurHasher {
    #[inline(always)]
    pub fn hash_u64(value: u64) -> u64 {
        // MurmurHash3 64-bit finalizer
        let mut h = value;
        h ^= h >> 33;
        h = h.wrapping_mul(0xff51afd7ed558ccd);
        h ^= h >> 33;
        h = h.wrapping_mul(0xc4ceb9fe1a85ec53);
        h ^= h >> 33;
        h
    }
}

impl StatelessU64Hasher for MurmurHasher {
    #[inline(always)]
    fn hash(value: u64) -> u64 {
        Self::hash_u64(value)
    }
}

pub struct MulSwapMulHasher;

impl StatelessU64Hasher for MulSwapMulHasher {
    #[inline(always)]
    fn hash(value: u64) -> u64 {
        // Cheap bijective hasher: multiply-byteswap-multiply
        let mut h = value;
        h = h.wrapping_mul(0x9e3779b97f4a7c15); // First odd constant
        h = h.swap_bytes(); // Byte swap
        h = h.wrapping_mul(0xc2b2ae3d27d4eb4f); // Second odd constant
        h
    }
}

pub struct U64Hasher<Hasher: StatelessU64Hasher> {
    result: u64, 
    function: std::marker::PhantomData<Hasher>,
}

impl<Hasher: StatelessU64Hasher> std::hash::Hasher for U64Hasher<Hasher> {
    fn write(&mut self, _bytes: &[u8]) {
        unreachable!("Expected u64, got bytes");
    }

    #[inline(always)]
    fn write_u64(&mut self, value: u64) {
        self.result = Hasher::hash(value);
    }

    #[inline(always)]
    fn finish(&self) -> u64 {
        self.result
    }
}

impl<Hasher: StatelessU64Hasher> Default for U64Hasher<Hasher> {
    fn default() -> Self {
        Self {
            result: 0,
            function: std::marker::PhantomData,
        }
    }
}
