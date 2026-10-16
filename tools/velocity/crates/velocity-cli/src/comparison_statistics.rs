const MIN_SIGNIFICANCE_SAMPLES: usize = 5;
const EXACT_PERMUTATION_LIMIT: u128 = 200_000;
const MONTE_CARLO_PERMUTATIONS: usize = 20_000;

/// A two-sided permutation p-value for the difference in sample medians.
/// Exact enumeration is used for small samples; larger sets use a deterministic
/// Monte Carlo permutation sequence so repeated comparisons remain reproducible.
pub(crate) fn median_permutation_p_value(baseline: &[f64], current: &[f64]) -> Option<f64> {
    let baseline = baseline
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    let current = current
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if baseline.len() < MIN_SIGNIFICANCE_SAMPLES || current.len() < MIN_SIGNIFICANCE_SAMPLES {
        return None;
    }
    let selected_count = baseline.len();
    let observed = (median(&current)? - median(&baseline)?).abs();
    let mut pooled = baseline;
    pooled.extend(current);
    let combinations = combination_count(pooled.len(), selected_count);
    if combinations <= EXACT_PERMUTATION_LIMIT {
        let mut extreme = 0_u128;
        let mut total = 0_u128;
        let mut selected = Vec::with_capacity(selected_count);
        enumerate_groups(
            &pooled,
            selected_count,
            0,
            &mut selected,
            &mut extreme,
            &mut total,
            observed,
        );
        return (total > 0).then_some(extreme as f64 / total as f64);
    }

    let mut seed = pooled
        .iter()
        .fold(0x9e37_79b9_7f4a_7c15_u64, |state, value| {
            state.rotate_left(7) ^ value.to_bits().wrapping_add(0x517c_c1b7_2722_0a95)
        });
    let mut shuffled = pooled;
    let mut extreme = 0_usize;
    for _ in 0..MONTE_CARLO_PERMUTATIONS {
        for index in (1..shuffled.len()).rev() {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            shuffled.swap(index, seed as usize % (index + 1));
        }
        let difference =
            (median(&shuffled[..selected_count])? - median(&shuffled[selected_count..])?).abs();
        if difference + 1e-12 >= observed {
            extreme += 1;
        }
    }
    Some((extreme + 1) as f64 / (MONTE_CARLO_PERMUTATIONS + 1) as f64)
}

fn enumerate_groups(
    pooled: &[f64],
    selected_count: usize,
    next: usize,
    selected: &mut Vec<f64>,
    extreme: &mut u128,
    total: &mut u128,
    observed: f64,
) {
    if selected.len() == selected_count {
        let mut other = pooled.to_vec();
        for value in selected.iter() {
            if let Some(index) = other
                .iter()
                .position(|candidate| candidate.to_bits() == value.to_bits())
            {
                other.remove(index);
            }
        }
        if let (Some(first), Some(second)) = (median(selected), median(&other)) {
            *total += 1;
            if (first - second).abs() + 1e-12 >= observed {
                *extreme += 1;
            }
        }
        return;
    }
    let remaining = selected_count - selected.len();
    let last = pooled.len().saturating_sub(remaining);
    for index in next..=last {
        selected.push(pooled[index]);
        enumerate_groups(
            pooled,
            selected_count,
            index + 1,
            selected,
            extreme,
            total,
            observed,
        );
        selected.pop();
    }
}

fn combination_count(total: usize, selected: usize) -> u128 {
    let selected = selected.min(total.saturating_sub(selected));
    (0..selected).fold(1_u128, |value, index| {
        value
            .saturating_mul((total - index) as u128)
            .checked_div((index + 1) as u128)
            .unwrap_or(u128::MAX)
    })
}

fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    Some(if sorted.len().is_multiple_of(2) {
        (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
    } else {
        sorted[sorted.len() / 2]
    })
}

#[cfg(test)]
mod tests {
    use super::median_permutation_p_value;

    #[test]
    fn strong_median_shift_has_significant_permutation_evidence() {
        let baseline = [10.0, 10.1, 10.2, 10.3, 10.4];
        let current = [12.0, 12.1, 12.2, 12.3, 12.4];
        assert!(median_permutation_p_value(&baseline, &current).unwrap() < 0.05);
    }

    #[test]
    fn overlapping_samples_do_not_support_a_significant_change() {
        let baseline = [10.0, 10.1, 10.2, 10.3, 10.4, 10.5];
        let current = [10.1, 10.2, 10.3, 10.4, 10.5, 10.6];
        assert!(median_permutation_p_value(&baseline, &current).unwrap() >= 0.05);
    }

    #[test]
    fn fewer_than_five_samples_per_run_is_inconclusive() {
        assert_eq!(
            median_permutation_p_value(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]),
            None
        );
    }
}
