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

pub struct U64Hasher<Hasher: StatelessU64Hasher> {
    result: u64, 
    function: std::marker::PhantomData<Hasher>,
}

impl<Hasher: StatelessU64Hasher> std::hash::Hasher for U64Hasher<Hasher> {
    fn write(&mut self, bytes: &[u8]) {
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
