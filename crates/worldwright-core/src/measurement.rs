//! Pure measurement-recipe indexing; no clock, workload, I/O or capacity claim.

/// Integer nearest rank, with an ascending, nonempty sample set and percent 1..=100.
pub const NEAREST_RANK_RECIPE: &str = "nearest-rank-integer-v1";

/// Select the zero-based index ceil(N * percent / 100) - 1.
/// The caller sorts samples. Invalid dimensions return None rather than panic.
/// Split quotient/remainder arithmetic avoids overflowing N * percent.
pub fn nearest_rank_index(sample_count: usize, percent: usize) -> Option<usize> {
    if sample_count == 0 || !(1..=100).contains(&percent) {
        return None;
    }
    let rank = (sample_count / 100) * percent
        + ((sample_count % 100) * percent).div_ceil(100);
    Some(rank - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_rejects_invalid_and_selects_boundaries() {
        assert_eq!(nearest_rank_index(0, 50), None);
        assert_eq!(nearest_rank_index(1, 0), None);
        assert_eq!(nearest_rank_index(1, 101), None);
        for percent in 1..=100 {
            assert_eq!(nearest_rank_index(1, percent), Some(0));
        }
        assert_eq!(nearest_rank_index(2, 50), Some(0));
        assert_eq!(nearest_rank_index(3, 50), Some(1));
        assert_eq!(nearest_rank_index(20, 95), Some(18));
        assert_eq!(nearest_rank_index(100, 99), Some(98));
        assert_eq!(nearest_rank_index(7_500, 95), Some(7_124));
        assert_eq!(nearest_rank_index(7_500, 99), Some(7_424));
        assert_eq!(nearest_rank_index(usize::MAX, 100), Some(usize::MAX - 1));
    }

    #[test]
    fn nearest_rank_matches_independent_wide_integer_reference() {
        for count in (1..=1_000).chain([7_500, usize::MAX]) {
            for percent in 1..=100 {
                let expected = ((count as u128 * percent as u128).div_ceil(100) - 1) as usize;
                assert_eq!(nearest_rank_index(count, percent), Some(expected));
            }
        }
    }
}
