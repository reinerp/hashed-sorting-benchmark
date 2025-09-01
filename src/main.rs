mod hashers;
mod u64_hash_set;
mod wide_merge_sort;

use dashmap::DashMap;
use fastrand;
use foldhash::fast::RandomState as FoldRandomState;
use hashers::{MulSwapMulHasher, MurmurHasher, NoopHasher, StatelessU64Hasher, U64Hasher};
use rayon::prelude::*;
use scc::HashSet as SccHashSet;
use std::collections::HashSet;
use std::hash::{BuildHasher, BuildHasherDefault, RandomState};
use std::time::{Duration, Instant};
use u64_hash_set::U64HashSet;
use voracious_radix_sort::RadixSort;
use wide_merge_sort::wide_merge_sort;


// Configuration choices:
const MASK_STYLE: MaskStyle = MaskStyle::SpreadOut2x;
const LG_ACCESSES_PER_ELEMENT: usize = 1;
const BENCHMARK_FILTERS: &[&str] = &["memcpy", "Hashed sorting (radix + MulSwapMul)", "HashSet (dense_table + MulSwapMul)"];
const SIZES: &[usize] = &[10, 15, 20, 25, 28];



#[derive(Debug)]
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

#[derive(Debug)]
#[allow(dead_code)]
enum AccessFrequency {
    /// Each number is inserted on average just once. Some elements by duplicates by chance, but on average most are not.
    OneVisitPerAverageElement,
    /// Each number is inserted on average 16 times. Most elements will very much be duplicates.
    ManyVisitsPerAverageElement,
}



fn count_unique_by_hash<Hasher: BuildHasher>(
    data: &[u64],
    hasher: Hasher,
    domain_size: usize,
) -> usize {
    let mut hasher = HashSet::with_capacity_and_hasher(domain_size, hasher);
    for d in data {
        hasher.insert(*d);
    }
    hasher.len()
}

fn count_unique_by_u64_hash<H: StatelessU64Hasher>(data: &[u64], domain_size: usize) -> usize {
    let mut set = U64HashSet::<H>::with_capacity(domain_size);
    for &d in data {
        set.insert(d);
    }
    set.len()
}

fn count_unique_by_parallel_hash<Hasher: BuildHasher + Clone + Send + Sync>(
    data: &[u64],
    hasher: Hasher,
    domain_size: usize,
) -> usize
where
    Hasher::Hasher: Send,
{
    let dashmap = DashMap::with_capacity_and_hasher(domain_size, hasher);
    data.par_iter().for_each(|&d| {
        dashmap.insert(d, ());
    });
    dashmap.len()
}

fn count_unique_by_scc_parallel_hash<Hasher: BuildHasher + Sync>(
    data: &[u64],
    hasher: Hasher,
    domain_size: usize,
) -> usize {
    let scc_set: SccHashSet<u64, Hasher> =
        SccHashSet::with_capacity_and_hasher(domain_size, hasher);
    data.par_iter().for_each(|&d| {
        let _result = scc_set.insert(d);
    });
    scc_set.len()
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
    let mut hashed_data = data.iter().map(|&d| H::hash(d)).collect::<Vec<_>>();
    hashed_data.voracious_sort();
    count_unique_in_sorted(&hashed_data)
}

fn count_unique_by_parallel_sort<F>(data: &[u64], sort_fn: F) -> usize
where
    F: FnOnce(&mut Vec<u64>),
{
    let mut sorted_data = Vec::new();
    data.par_iter().copied().collect_into_vec(&mut sorted_data);
    // let mut sorted_data = data.to_vec();
    sort_fn(&mut sorted_data);
    count_unique_in_sorted_parallel(&sorted_data)
}

fn count_unique_by_hashed_parallel_sort<H: StatelessU64Hasher>(
    data: &[u64],
    sort_fn: impl FnOnce(&mut Vec<u64>),
) -> usize {
    let mut sorted_data = Vec::new();
    data.par_iter()
        .map(|&d| H::hash(d))
        .collect_into_vec(&mut sorted_data);
    sort_fn(&mut sorted_data);
    count_unique_in_sorted_parallel(&sorted_data)
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

fn count_unique_in_sorted_parallel(sorted_data: &[u64]) -> usize {
    if sorted_data.is_empty() {
        return 0;
    }
    1 + sorted_data
        .par_windows(2)
        .map(|w| (w[0] != w[1]) as usize)
        .sum::<usize>()
}

fn benchmark(name: &str, repeats: usize, mut f: impl FnMut()) {
    let mut run_benchmark = false;
    for filter in BENCHMARK_FILTERS {
        if name.contains(filter) {
            run_benchmark = true;
            break;
        }
    }
    if !run_benchmark {
        return;
    }
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
    let mask_style = MASK_STYLE;
    let lg_accesses_per_element = LG_ACCESSES_PER_ELEMENT;
    println!(
        "mask style: {:?}, average accesses per element: 2^{}",
        mask_style, lg_accesses_per_element
    );

    // let num_threads = rayon::current_num_threads();
    let num_threads = 1;
    println!("Using {} threads for parallel algorithms", num_threads);

    let mut rng = fastrand::Rng::with_seed(0);
    for &lg_size in SIZES {
        let mut data = vec![0u64; 1 << lg_size];
        let lg_domain_size = lg_size.saturating_sub(lg_accesses_per_element);
        let domain_size = 1usize << lg_domain_size;
        let mask = match mask_style {
            MaskStyle::LowBits => (1u64 << lg_domain_size) - 1,
            MaskStyle::HighBits => (1u64 << lg_domain_size).wrapping_neg(),
            MaskStyle::SpreadOut2x => ((1u64 << (2 * lg_domain_size)) - 1) & 0x5555_5555_5555_5555,
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

        {
            let mut data_copy = vec![0u64; data.len() + 1];
            let mut i = 0;
            benchmark("memcpy", repeats, || {
                std::hint::black_box(&mut data_copy[i..data.len() + i]).copy_from_slice(std::hint::black_box(&data));
                i ^= std::hint::black_box(1);
            });
            std::hint::black_box(data_copy);
        }

        // For smaller benchmarks, we run all benchmarks. For larger benchmarks, we only run
        // the algorithms that are at least a certain speed.
        // let is_smaller = lg_size <= 25;
        let is_smaller = true;
        // Don't run NoOp hashing for huge sizes when the data is unfavorable to it; it takes forever.
        let noop_will_finish = lg_size < 25 || matches!(mask_style, MaskStyle::LowBits);
        let noop_will_be_fast = lg_size < 20 || matches!(mask_style, MaskStyle::LowBits);

        let sip_hasher = RandomState::new(); // Unfortunately not seedable :(
        let murmur_hasher = BuildHasherDefault::<U64Hasher<MurmurHasher>>::default();
        let mulswapmul_hasher = BuildHasherDefault::<U64Hasher<MulSwapMulHasher>>::default();
        let foldhash_hasher = FoldRandomState::default();

        if is_smaller {
            benchmark("HashSet (SwissTable + SipHash)", repeats, || {
                count_unique_by_hash(&data, sip_hasher.clone(), domain_size);
            });

            benchmark("HashSet (SwissTable + Murmur)", repeats, || {
                count_unique_by_hash(&data, murmur_hasher.clone(), domain_size);
            });

            benchmark("HashSet (SwissTable + FoldHash)", repeats, || {
                count_unique_by_hash(&data, foldhash_hasher.clone(), domain_size);
            });
        }

        benchmark("HashSet (SwissTable + MulSwapMul)", repeats, || {
            count_unique_by_hash(&data, mulswapmul_hasher.clone(), domain_size);
        });

        if is_smaller {
            if noop_will_finish {
                let noop_hasher = BuildHasherDefault::<U64Hasher<NoopHasher>>::default();
                benchmark(
                    "HashSet (SwissTable + NoOp)",
                    if noop_will_be_fast { repeats } else { 1 },
                    || {
                        count_unique_by_hash(&data, noop_hasher.clone(), domain_size);
                    },
                );
            }

            benchmark("HashSet (dense_table + Murmur)", repeats, || {
                count_unique_by_u64_hash::<MurmurHasher>(&data, domain_size);
            });

            if noop_will_finish {
                benchmark(
                    "HashSet (dense_table + NoOp)",
                    if noop_will_be_fast { repeats } else { 1 },
                    || {
                        count_unique_by_u64_hash::<NoopHasher>(&data, domain_size);
                    },
                );
            }
        }

        benchmark("HashSet (dense_table + MulSwapMul)", repeats, || {
            count_unique_by_u64_hash::<MulSwapMulHasher>(&data, domain_size);
        });

        if is_smaller {
            benchmark("Sorting (merge sort)", repeats, || {
                count_unique_by_sort(&data, |v| v.sort());
            });

            benchmark("Sorting (quick sort)", repeats, || {
                count_unique_by_sort(&data, |v| v.sort_unstable());
            });
        }

        benchmark("Sorting (radix sort)", repeats, || {
            count_unique_by_sort(&data, |v| v.voracious_sort());
        });

        benchmark("Sorting (wide merge sort)", repeats, || {
            count_unique_by_sort(&data, |v| wide_merge_sort(v));
        });

        if is_smaller {
            benchmark("Hashed sorting (radix + Murmur)", repeats, || {
                count_unique_by_hashed_sort::<MurmurHasher>(&data);
            });
            benchmark("Hashed sorting (radix + NoOp)", repeats, || {
                count_unique_by_hashed_sort::<NoopHasher>(&data);
            });
        }

        benchmark("Hashed sorting (radix + MulSwapMul)", repeats, || {
            count_unique_by_hashed_sort::<MulSwapMulHasher>(&data);
        });

        // Parallel benchmarks
        if is_smaller {
            benchmark("Parallel HashSet (dashmap + SipHash)", repeats, || {
                count_unique_by_parallel_hash(&data, sip_hasher.clone(), domain_size);
            });

            benchmark("Parallel HashSet (dashmap + Murmur)", repeats, || {
                count_unique_by_parallel_hash(&data, murmur_hasher.clone(), domain_size);
            });
        }

        benchmark("Parallel HashSet (dashmap + FoldHash)", repeats, || {
            count_unique_by_parallel_hash(&data, foldhash_hasher.clone(), domain_size);
        });

        if is_smaller {
            benchmark("Parallel HashSet (scc + SipHash)", repeats, || {
                count_unique_by_scc_parallel_hash(&data, sip_hasher.clone(), domain_size);
            });

            benchmark("Parallel HashSet (scc + Murmur)", repeats, || {
                count_unique_by_scc_parallel_hash(&data, murmur_hasher.clone(), domain_size);
            });
        }

        benchmark("Parallel HashSet (scc + FoldHash)", repeats, || {
            count_unique_by_scc_parallel_hash(&data, foldhash_hasher.clone(), domain_size);
        });

        if is_smaller {
            benchmark("Parallel sorting (merge sort)", repeats, || {
                count_unique_by_parallel_sort(&data, |v| v.par_sort());
            });

            benchmark("Parallel sorting (quick sort)", repeats, || {
                count_unique_by_parallel_sort(&data, |v| v.par_sort_unstable());
            });
        }

        benchmark("Parallel sorting (radix sort)", repeats, || {
            count_unique_by_parallel_sort(&data, |v| v.voracious_mt_sort(num_threads));
        });
        benchmark(
            "Parallel hashed sorting (radix + MulSwapMul)",
            repeats,
            || {
                count_unique_by_hashed_parallel_sort::<MulSwapMulHasher>(&data, |v| {
                    v.voracious_mt_sort(num_threads)
                });
            },
        );
    }
}
