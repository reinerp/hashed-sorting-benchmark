use fastrand;
use foldhash::fast::RandomState as FoldRandomState;
use std::collections::HashSet;
use std::hash::{BuildHasher, BuildHasherDefault, RandomState};
use std::time::{Duration, Instant};
use voracious_radix_sort::RadixSort;
mod hashers;
use hashers::{MulSwapMulHasher, MurmurHasher, NoopHasher, StatelessU64Hasher, U64Hasher};

fn count_unique_by_hash<Hasher: BuildHasher>(data: &[u64], hasher: Hasher) -> usize {
    let mut hasher = HashSet::with_capacity_and_hasher(data.len(), hasher);
    for d in data {
        hasher.insert(*d);
    }
    hasher.len()
}

fn count_unique_by_sort<F>(data: &[u64], sort_fn: F) -> usize 
where
    F: FnOnce(&mut Vec<u64>),
{
    let mut sorted_data = data.to_vec();
    sort_fn(&mut sorted_data);
    count_unique_in_sorted(&sorted_data)
}

fn count_unique_by_hashed_sort<H: StatelessU64Hasher>(data: &[u64]) -> usize {
    let mut hashed_data = data.to_vec();
    for d in &mut hashed_data {
        *d = H::hash(*d);
    }
    hashed_data.voracious_sort();
    count_unique_in_sorted(&hashed_data)
}

fn count_unique_in_sorted(sorted_data: &[u64]) -> usize {
    if sorted_data.is_empty() {
        return 0;
    }
    
    let mut count = 1;
    let mut prev = sorted_data[0];
    
    for &current in &sorted_data[1..] {
        count += (current != prev) as usize;
        prev = current;
    }
    
    count
}

fn benchmark(name: &str, repeats: usize, mut f: impl FnMut()) {
    // Warmup.
    for _ in 0..repeats {
        f();
    }
    let start = Instant::now();
    for _ in 0..repeats {
        f();
    }
    let duration = start.elapsed();
    println!("  {}: {}", name, human_time(repeats, duration));
}

fn human_time(repeats: usize, duration: Duration) -> String {
    let mut duration = duration.as_nanos() as f64 / repeats as f64;
    if duration < 1000.0 {
        return format!("{:.1}ns", duration);
    }
    duration /= 1000.0;
    if duration < 1000.0 {
        return format!("{:.1}us", duration);
    }
    duration /= 1000.0;
    if duration < 1000.0 {
        return format!("{:.1}ms", duration);
    }
    duration /= 1000.0;
    format!("{:.1}s", duration)
}

fn human_size(size: usize) -> String {
    if size < 1024 {
        return format!("{}B", size);
    }
    let mut size = size as f64;
    size /= 1024.0;
    if size < 1024.0 {
        return format!("{}KiB", size);
    }
    size /= 1024.0;
    if size < 1024.0 {
        return format!("{}MiB", size);
    }
    size /= 1024.0;
    format!("{}GiB", size)
}

#[allow(dead_code)]
enum MaskStyle {
    /// All the entropy is in the low bits. Friendly to most algorithms, even with Noop hashing.
    LowBits,
    /// All the entropy is in the high bits. Unfriendly to most hashing algorithms, which need entropy in the low bits.
    /// Friendly enough for radix sort algorithms which are adaptive to where the entropy is.
    HighBits,
    /// The entropy is spread out over pairs of bits: each even bit is equal to the next odd bit. This tends to be
    /// unfriendly to NoOp hashing both for hashing and radix sort algorithms.
    SpreadOut2x,
}

fn main() {
    let mut rng = fastrand::Rng::with_seed(0);
    let mask_style = MaskStyle::LowBits;
    for lg_size in [10, 15, 20] {
        let mut data = vec![0u64; 1 << lg_size];
        let mask = match mask_style {
            MaskStyle::LowBits => (1u64 << lg_size) - 1,
            MaskStyle::HighBits => (1u64 << lg_size).wrapping_neg(),
            MaskStyle::SpreadOut2x => ((1u64 << (2 * lg_size)) - 1) & 0x5555_5555_5555_5555,
        };
        for d in &mut data {
            let random = rng.u64(..);
            let mut masked = random & mask;
            if matches!(mask_style, MaskStyle::SpreadOut2x) {
                masked = masked | (masked << 1);
            }
            *d = masked;
        }
        
        let repeats = 1usize << 25usize.saturating_sub(lg_size);
        println!(
            "size: {}",
            human_size(std::mem::size_of::<u64>() * data.len())
        );

        let sip_hasher = RandomState::new(); // Unfortunately not seedable :(
        benchmark("HashSet (SipHash)", repeats, || {
            count_unique_by_hash(&data, sip_hasher.clone());
        });

        // Don't run NoOp hashing for huge sizes when the data is unfavorable to it; it takes forever.
        let noop_will_finish = lg_size < 25 || matches!(mask_style, MaskStyle::LowBits);
        if noop_will_finish {
            let noop_will_be_fast = lg_size < 20 || matches!(mask_style, MaskStyle::LowBits);
            let noop_hasher = BuildHasherDefault::<U64Hasher<NoopHasher>>::default();
            benchmark("HashSet (NoOp)", if noop_will_be_fast { repeats } else { 1 }, || {
                count_unique_by_hash(&data, noop_hasher.clone());
            });
        }

        let murmur_hasher = BuildHasherDefault::<U64Hasher<MurmurHasher>>::default();
        benchmark("HashSet (Murmur)", repeats, || {
            count_unique_by_hash(&data, murmur_hasher.clone());
        });

        let foldhash_hasher = FoldRandomState::default();
        benchmark("HashSet (FoldHash)", repeats, || {
            count_unique_by_hash(&data, foldhash_hasher.clone());
        });

        benchmark("Sorting (merge sort)", repeats, || {
            count_unique_by_sort(&data, |v| v.sort());
        });

        benchmark("Sorting (quick sort)", repeats, || {
            count_unique_by_sort(&data, |v| v.sort_unstable());
        });

        benchmark("Sorting (radix sort)", repeats, || {
            count_unique_by_sort(&data, |v| v.voracious_sort());
        });

        benchmark("Hashed sorting (radix + Murmur)", repeats, || {
            count_unique_by_hashed_sort::<MurmurHasher>(&data);
        });

        benchmark("Hashed sorting (radix + MulSwapMul)", repeats, || {
            count_unique_by_hashed_sort::<MulSwapMulHasher>(&data);
        });

        benchmark("Hashed sorting (radix + NoOp)", repeats, || {
            count_unique_by_hashed_sort::<NoopHasher>(&data);
        });
    }
}
