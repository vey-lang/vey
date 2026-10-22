//! Suggestions for names that are close to a known one.

/// The number of edits between two words.
///
/// The two letters that are swapped count as one edit, so that a name that
/// is written in the wrong order is still near the one it was meant to be.
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }

    // The table of the distances, with one row and one column more than the
    // words need.
    let mut table = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (index, _) in a.iter().enumerate() {
        table[index + 1][0] = index + 1;
    }
    for (index, _) in b.iter().enumerate() {
        table[0][index + 1] = index + 1;
    }

    for i in 0..a.len() {
        for j in 0..b.len() {
            let cost = usize::from(a[i] != b[j]);
            let mut best = (table[i][j] + 1)
                .min(table[i + 1][j] + 1)
                .min(table[i][j + 1] + 1);
            if cost == 0 {
                best = best.min(table[i][j]);
            }
            // Two letters that are swapped are one edit.
            if i > 0 && j > 0 && a[i] == b[j - 1] && a[i - 1] == b[j] {
                best = best.min(table[i - 1][j - 1] + 1);
            }
            table[i + 1][j + 1] = best;
        }
    }

    table[a.len()][b.len()]
}

/// How near a name may be and still be suggested.
///
/// A short name has few letters, so a single edit is already a lot. The
/// limit grows with the name, so that a long name may be wrong in more
/// places.
pub fn limit_for(name: &str) -> usize {
    let length = name.chars().count();
    match length {
        0..=3 => 1,
        4..=7 => 2,
        _ => 3,
    }
}

/// The name in `candidates` that is closest to `name`.
///
/// Only names that are close enough are suggested, so that a wrong name is
/// not replaced by something unrelated. The case is not counted, because a
/// name that is only written with other letters is the name that was meant.
pub fn closest<'a, I>(name: &str, candidates: I) -> Option<&'a str>
where
    I: IntoIterator<Item = &'a str>,
{
    let limit = limit_for(name);
    let wanted = name.to_lowercase();
    let mut best: Option<(usize, &str)> = None;
    for candidate in candidates {
        if candidate == name {
            return Some(candidate);
        }
        let distance = edit_distance(&wanted, &candidate.to_lowercase());
        if distance > limit {
            continue;
        }
        if best.as_ref().is_none_or(|(best, _)| distance < *best) {
            best = Some((distance, candidate));
        }
    }
    best.map(|(_, candidate)| candidate)
}

/// A hint that names a similar known name.
pub fn did_you_mean<'a, I>(name: &str, candidates: I) -> Option<String>
where
    I: IntoIterator<Item = &'a str>,
{
    closest(name, candidates).map(|candidate| format!("did you mean '{}'?", candidate))
}
