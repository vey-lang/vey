//! Suggestions for names that are close to a known one.

/// The number of edits between two words.
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0; b.len() + 1];
    for (i, left) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, right) in b.iter().enumerate() {
            let cost = usize::from(left != right);
            current[j + 1] = (previous[j + 1] + 1)
                .min(current[j] + 1)
                .min(previous[j] + cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

/// The name in `candidates` that is closest to `name`.
///
/// Only names that are close enough are suggested, so that a wrong name is
/// not replaced by something unrelated.
pub fn closest<'a, I>(name: &str, candidates: I) -> Option<&'a str>
where
    I: IntoIterator<Item = &'a str>,
{
    let limit = (name.chars().count() / 3).max(2);
    let mut best: Option<(usize, &str)> = None;
    for candidate in candidates {
        let distance = edit_distance(name, candidate);
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
