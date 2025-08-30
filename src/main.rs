use fastrand;
use foldhash::fast::RandomState as FoldRandomState;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasher, BuildHasherDefault, RandomState};
use std::time::{Duration, Instant};
mod hashers;
use hashers::{MurmurHasher, NoopHasher, U64Hasher};

fn count_unique_by_hash<Hasher: BuildHasher>(data: &[u64], hasher: Hasher) -> usize {
    let mut hasher = HashSet::with_capacity_and_hasher(data.len(), hasher);
    for d in data {
        hasher.insert(*d);
    }
    hasher.len()
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

fn main() {
    let mut rng = fastrand::Rng::with_seed(0);
    for lg_size in [10, 15, 20, 25] {
        let mut data = vec![0u64; 1 << lg_size];
        // Use a mask that has the high lg_size bits set. This way we will have a small
        // but nonzero number of duplicates.
        // let mask = (1u64 << lg_size).wrapping_neg();
        let mask = (1u64 << lg_size) - 1;
        for d in &mut data {
            *d = rng.u64(..) & mask;
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

        let noop_hasher = BuildHasherDefault::<U64Hasher<NoopHasher>>::default();
        benchmark("HashSet (NoOp)", repeats, || {
            count_unique_by_hash(&data, noop_hasher.clone());
        });

        let murmur_hasher = BuildHasherDefault::<U64Hasher<MurmurHasher>>::default();
        benchmark("HashSet (Murmur)", repeats, || {
            count_unique_by_hash(&data, murmur_hasher.clone());
        });

        let foldhash_hasher = FoldRandomState::default();
        benchmark("HashSet (FoldHash)", repeats, || {
            count_unique_by_hash(&data, foldhash_hasher.clone());
        });
    }
}
