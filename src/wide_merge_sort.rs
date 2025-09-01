const N: usize = 256;

#[inline(always)]
fn merge256(mut srcs: [std::slice::Iter<u64>; N], dst: &mut [u64]) {
    // Head of each list. u64::MAX sentinel if the list is exhausted.
    let mut keys: [u64; N] = std::array::from_fn(|i| srcs[i].next().copied().unwrap_or(u64::MAX));

    // Tournament tree for the merge. loser_table[0] is the winner; loser_table[i] for i>0 is the loser of the match at that node of the tournament.
    //
    // "Losing" a tournament means you are a *bigger* u64.
    let mut loser_table: [u8; N] = [0; N];


    //// Initialize loser table, leaves upwards towards the root. We maintain a temporary array of winners.
    {
        let mut winners: [u8; N * 2] = std::array::from_fn(|i| (i % N) as u8);
        for i in (1..N).rev() {
            let left_child = 2 * i;
            let right_child = left_child + 1;
            let winner = if keys[winners[left_child] as usize] < keys[winners[right_child] as usize] { left_child } else { right_child };
            winners[i] = winners[winner];
            let loser = winner ^ 1;
            loser_table[i] = winners[loser];
        }
        loser_table[0] = winners[1];
    }

    for d in dst {
        // Advance winner.
        let mut winner_i = loser_table[0] as usize;
        *d = keys[winner_i];
        keys[winner_i] = srcs[winner_i].next().copied().unwrap_or(u64::MAX);

        // Update loser table.

        // Look up all the loser indices on the path to the root.
        let leaf_i = winner_i + N;  // [N, 2N)
        let mut loser7_i = loser_table[leaf_i >> 1] as usize;   // [N/2, N)
        let mut loser6_i = loser_table[leaf_i >> 2] as usize;   // [N/4, N/2)
        let mut loser5_i = loser_table[leaf_i >> 3] as usize;
        let mut loser4_i = loser_table[leaf_i >> 4] as usize;
        let mut loser3_i = loser_table[leaf_i >> 5] as usize;
        let mut loser2_i = loser_table[leaf_i >> 6] as usize;
        let mut loser1_i = loser_table[leaf_i >> 7] as usize;
        let mut loser0_i = loser_table[1] as usize;  // We know it's at the root; no need to compute its address dynamically
        // Look up the loser values.
        let loser7 = keys[loser7_i];
        let loser6 = keys[loser6_i];
        let loser5 = keys[loser5_i];
        let loser4 = keys[loser4_i];
        let loser3 = keys[loser3_i];
        let loser2 = keys[loser2_i];
        let loser1 = keys[loser1_i];
        let loser0 = keys[loser0_i];

        // Replay the new entrant against all losers on the path.
        let mut winner = keys[winner_i];
        (winner_i, winner, loser7_i) = if winner < loser7 { (winner_i, winner, loser7_i) } else { (loser7_i, loser7, winner_i) };
        (winner_i, winner, loser6_i) = if winner < loser6 { (winner_i, winner, loser6_i) } else { (loser6_i, loser6, winner_i) };
        (winner_i, winner, loser5_i) = if winner < loser5 { (winner_i, winner, loser5_i) } else { (loser5_i, loser5, winner_i) };
        (winner_i, winner, loser4_i) = if winner < loser4 { (winner_i, winner, loser4_i) } else { (loser4_i, loser4, winner_i) };
        (winner_i, winner, loser3_i) = if winner < loser3 { (winner_i, winner, loser3_i) } else { (loser3_i, loser3, winner_i) };
        (winner_i, winner, loser2_i) = if winner < loser2 { (winner_i, winner, loser2_i) } else { (loser2_i, loser2, winner_i) };
        (winner_i, winner, loser1_i) = if winner < loser1 { (winner_i, winner, loser1_i) } else { (loser1_i, loser1, winner_i) };
        (winner_i, winner, loser0_i) = if winner < loser0 { (winner_i, winner, loser0_i) } else { (loser0_i, loser0, winner_i) };
        _ = winner;  // Unused

        // Update loser table.
        loser_table[leaf_i >> 1] = loser7_i as u8;
        loser_table[leaf_i >> 2] = loser6_i as u8;
        loser_table[leaf_i >> 3] = loser5_i as u8;
        loser_table[leaf_i >> 4] = loser4_i as u8;
        loser_table[leaf_i >> 5] = loser3_i as u8;
        loser_table[leaf_i >> 6] = loser2_i as u8;
        loser_table[leaf_i >> 7] = loser1_i as u8;
        loser_table[1] = loser0_i as u8;
        loser_table[0] = winner_i as u8;
    }

}

pub fn wide_merge_sort(data: &mut [u64]) {
    if data.len() <= 1024 {
        data.sort_unstable();
        return;
    }
    
    // Single allocation for auxiliary buffer
    let mut aux = vec![0u64; data.len()];
    wide_merge_sort_recursive(data, &mut aux, false);
}

/// Recursively sorts the data using 256-way merge sort.
/// 
/// If write_to_aux is true, writes the result to aux. Otherwise, writes the result to data.
fn wide_merge_sort_recursive(data: &mut [u64], aux: &mut [u64], write_to_aux: bool) {
    let len = data.len();
    
    // Base case: use sort_unstable for small arrays
    // Output: data (in-place sort)
    if len <= 1024 {
        data.sort_unstable();
        if write_to_aux {
            aux.copy_from_slice(data);
        }
        return;
    }

    // Recurse on chunks.
    let not_write_to_aux = !write_to_aux;
    for i in 0..N {
        let chunk_start = (len * i) / N;
        let chunk_end = (len * (i + 1)) / N;
        let chunk_range = chunk_start..chunk_end;
        wide_merge_sort_recursive(&mut data[chunk_range.clone()], &mut aux[chunk_range], not_write_to_aux);
    }
    // Merge.
    let (merge_src, merge_dst) = if write_to_aux {
        (data, aux)
    } else {
        (aux, data)
    };
    let srcs = std::array::from_fn(|i| {
        let chunk_start = (len * i) / N;
        let chunk_end = (len * (i + 1)) / N;
        merge_src[chunk_start..chunk_end].iter()
    });
    merge256(srcs, merge_dst)
}
