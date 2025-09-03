use std::time::Instant;

use crate::hashers::StatelessU64Hasher;

const LG_MAX_DIVERSION_SIZE: u32 = if DO_INSERTION_SORT { 0 } else { 5 };
const LG_RADIX: u32 = 10;
const RADIX: usize = 1 << LG_RADIX;
const WORD_BITS: u32 = 64;
const MAX_PASSES: usize = (WORD_BITS - LG_MAX_DIVERSION_SIZE).div_ceil(LG_RADIX) as usize;
const CHUNK_SIZE: usize = 4;
const DO_INSERTION_SORT: bool = true;

pub fn dlsd_sort<Hasher: StatelessU64Hasher>(orig_data: &[u64]) -> Vec<u64> {
    let passes = orig_data
        .len()
        .next_power_of_two()
        .ilog2()
        .saturating_sub(LG_MAX_DIVERSION_SIZE)
        .div_ceil(LG_RADIX) as usize;
    assert!(orig_data.len() % CHUNK_SIZE == 0);
    println!("passes: {}", passes);
    let counts_start = Instant::now();
    // First gather counts.
    let (mut data, counts) = match passes {
        0 => compute_counts::<0, Hasher>(orig_data),
        1 => compute_counts::<1, Hasher>(orig_data),
        2 => compute_counts::<2, Hasher>(orig_data),
        3 => compute_counts::<3, Hasher>(orig_data),
        4 => compute_counts::<4, Hasher>(orig_data),
        5 => compute_counts::<5, Hasher>(orig_data),
        6 => compute_counts::<6, Hasher>(orig_data),
        7 => compute_counts::<7, Hasher>(orig_data),
        8 => compute_counts::<8, Hasher>(orig_data),
        9 => compute_counts::<9, Hasher>(orig_data),
        10 => compute_counts::<10, Hasher>(orig_data),
        11 => compute_counts::<11, Hasher>(orig_data),
        12 => compute_counts::<12, Hasher>(orig_data),
        13 => compute_counts::<13, Hasher>(orig_data),
        _ => unreachable!("Too many passes!"),
    };
    println!("counts time: {:?}", counts_start.elapsed());
    let aux_alloc_start = Instant::now();
    let mut aux = vec![0u64; data.len()];
    println!("aux alloc time: {:?}", aux_alloc_start.elapsed());
    let passes_start = Instant::now();
    let mut from = &mut data[..];
    let mut to = &mut aux[..];
    // Now do passes. Non-last passes just do dealing.
    for pass in 0..passes - if DO_INSERTION_SORT { 1 } else { 0 } {
        let mut heads = [0usize; RADIX];
        let mut pos = 0;
        for i in 0..RADIX {
            heads[i] = pos;
            pos += counts[pass][i];
        }

        for chunk in from.as_chunks::<CHUNK_SIZE>().0 {
            for word in chunk {
                let radix = read_radix(*word, pass, passes);
                unsafe {
                    let pos = heads.get_unchecked_mut(radix);
                    *to.get_unchecked_mut(*pos) = *word;
                    *pos += 1;
                }
            }
        }
        std::mem::swap(&mut from, &mut to);
    }
    println!("normal passes time: {:?}", passes_start.elapsed());

    let last_pass_start = Instant::now();
    // Last pass does dealing and fused insertion sort.
    if DO_INSERTION_SORT {
        let pass = passes - 1;
        #[derive(Clone, Copy)]
        struct Head {
            start: usize,
            pos: usize,
        }
        let mut heads = [Head { start: 0, pos: 0 }; RADIX];
        let mut pos = 0;
        for i in 0..RADIX {
            heads[i] = Head {
                start: pos,
                pos: pos,
            };
            pos += counts[pass][i];
        }
        for chunk in from.as_chunks::<CHUNK_SIZE>().0 {
            for &word in chunk {
                let radix = read_radix(word, pass, passes);
                let head = unsafe { heads.get_unchecked_mut(radix) };
                // Insertion sort backwards towards the beginning of the group.
                let mut j = head.pos;
                while j > head.start && unsafe { *to.get_unchecked(j - 1) } > word {
                    unsafe { *to.get_unchecked_mut(j) = *to.get_unchecked(j - 1) };
                    j -= 1;
                }
                unsafe { *to.get_unchecked_mut(j) = word };
                head.pos += 1;
            }
        }
    }
    println!("last pass time: {:?}", last_pass_start.elapsed());
    let final_copy_start = Instant::now();
    if passes % 2 == 1 {
        to.copy_from_slice(from);
    }
    println!("final copy time: {:?}", final_copy_start.elapsed());
    // if !DO_INSERTION_SORT {
    //     let diversion_start = Instant::now();
    //     // Now we're done with from and to, go back to data. Do diversion.
    //     let sorted_group_mask = (1u64 << (WORD_BITS - passes as u32 * LG_RADIX)).wrapping_neg();
    //     let mut group_start = 0;
    //     let mut group_value = u64::MAX;
    //     let mut num_groups = 0usize;
    //     for i in 0..data.len() {
    //         let w = data[i];
    //         if w & sorted_group_mask != group_value {
    //             group_start = i;
    //             group_value = w & sorted_group_mask;
    //             num_groups += 1;
    //         }
    //     }
    //     data[group_start..].sort_unstable();
    //     println!("diversion time: {:?}", diversion_start.elapsed());
    //     println!("num groups: 2^{:.1}", (num_groups as f64).log2());
    // }
    data
}

fn compute_counts<const PASSES: usize, Hasher: StatelessU64Hasher>(
    orig_data: &[u64],
) -> (Vec<u64>, [[usize; RADIX]; MAX_PASSES]) {
    let mut counts = [[0; RADIX]; MAX_PASSES];
    let mut data = Vec::with_capacity(orig_data.len());
    data.extend(orig_data
        .as_chunks::<CHUNK_SIZE>().0
        .iter()
        .flat_map(|chunk| {
            chunk.map(|word| {
                let h = Hasher::hash(word);
                for pass in 0..PASSES {
                    let radix = read_radix(h, pass, PASSES);
                    unsafe {
                        *counts.get_unchecked_mut(pass).get_unchecked_mut(radix) += 1;
                    }
                }
                h
            })
        }));
    (data, counts)
}

#[inline(always)]
fn read_radix(word: u64, pass: usize, passes: usize) -> usize {
    const MASK: u64 = (1 << LG_RADIX) - 1;
    let shift = WORD_BITS - ((passes - pass) as u32 * LG_RADIX);
    ((word >> shift) & MASK) as usize
}
