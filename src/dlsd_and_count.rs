use crate::hashers::StatelessU64Hasher;

const LG_RADIX: u32 = 10;
const RADIX: usize = 1 << LG_RADIX;
const WORD_BITS: u32 = 64;
const MAX_PASSES: usize = WORD_BITS.div_ceil(LG_RADIX) as usize;
const CHUNK_SIZE: usize = 4;

pub fn dlsd_sort_and_count<Hasher: StatelessU64Hasher>(orig_data: &[u64]) -> usize {
    let sum_of_radixes = orig_data.len().next_power_of_two().ilog2();
    let passes = sum_of_radixes.div_ceil(LG_RADIX) as usize;
    let last_pass_radix = sum_of_radixes - (passes as u32 - 1) * LG_RADIX;
    assert!(orig_data.len() % CHUNK_SIZE == 0);
    // First gather counts.
    let (mut data, counts) = match passes {
        0 => compute_counts::<0, Hasher>(orig_data, last_pass_radix),
        1 => compute_counts::<1, Hasher>(orig_data, last_pass_radix),
        2 => compute_counts::<2, Hasher>(orig_data, last_pass_radix),
        3 => compute_counts::<3, Hasher>(orig_data, last_pass_radix),
        4 => compute_counts::<4, Hasher>(orig_data, last_pass_radix),
        5 => compute_counts::<5, Hasher>(orig_data, last_pass_radix),
        6 => compute_counts::<6, Hasher>(orig_data, last_pass_radix),
        7 => compute_counts::<7, Hasher>(orig_data, last_pass_radix),
        8 => compute_counts::<8, Hasher>(orig_data, last_pass_radix),
        9 => compute_counts::<9, Hasher>(orig_data, last_pass_radix),
        10 => compute_counts::<10, Hasher>(orig_data, last_pass_radix),
        11 => compute_counts::<11, Hasher>(orig_data, last_pass_radix),
        12 => compute_counts::<12, Hasher>(orig_data, last_pass_radix),
        13 => compute_counts::<13, Hasher>(orig_data, last_pass_radix),
        _ => unreachable!("Too many passes!"),
    };
    let mut aux = vec![0u64; data.len()];  // TODO: MaybeUninit
    let mut from = &mut data[..];
    let mut to = &mut aux[..];
    // Now do passes. Non-last passes just do dealing.
    for pass in 0..passes - 1 {
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

    // Last pass does dealing and fused insertion sort and counting.
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
    let sorted_bits_mask = (1u64 << (WORD_BITS - (passes as u32 * LG_RADIX))).wrapping_neg();
    let mut unique_count = 0;
    for chunk in from.as_chunks::<CHUNK_SIZE>().0 {
        for &word in chunk {
            let radix = read_last_pass_radix(word, last_pass_radix);
            let head = unsafe { heads.get_unchecked_mut(radix) };
            // Insertion sort backwards towards the beginning of the group.
            let mut j = head.pos;
            while j > head.start && unsafe { *to.get_unchecked(j - 1) } > word {
                unsafe { *to.get_unchecked_mut(j) = *to.get_unchecked(j - 1) };
                j -= 1;
            }
            unsafe { *to.get_unchecked_mut(j) = word };
            if j > head.start {
                let prev_word = unsafe { *to.get_unchecked(j - 1) };
                unique_count += (prev_word < word) as usize;
                if (prev_word & sorted_bits_mask) != (word & sorted_bits_mask) {
                    // Stay in cache: once we've finished with a group, reset back to the beginning of the group.
                    //
                    // This is because we don't actually care about sorted order: we just care about the count.
                    head.pos = head.start;
                }
            } else {
                unique_count += 1;
            }
            head.pos += 1;
        }
    }
    unique_count
}

fn compute_counts<const PASSES: usize, Hasher: StatelessU64Hasher>(
    orig_data: &[u64],
    last_pass_radix: u32,
) -> (Vec<u64>, [[usize; RADIX]; MAX_PASSES]) {
    let mut counts = [[0; RADIX]; MAX_PASSES];
    let mut data = Vec::with_capacity(orig_data.len());
    data.extend(orig_data
        .as_chunks::<CHUNK_SIZE>().0
        .iter()
        .flat_map(|chunk| {
            chunk.map(|word| {
                let h = Hasher::hash(word);
                for pass in 0..PASSES - 1 {
                    let radix = read_radix(h, pass, PASSES);
                    unsafe {
                        *counts.get_unchecked_mut(pass).get_unchecked_mut(radix) += 1;
                    }
                }
                let radix = read_last_pass_radix(h, last_pass_radix);
                unsafe {
                    *counts.get_unchecked_mut(PASSES - 1).get_unchecked_mut(radix) += 1;
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

fn read_last_pass_radix(word: u64, last_pass_radix: u32) -> usize {
    (word >> (WORD_BITS - last_pass_radix)) as usize
}