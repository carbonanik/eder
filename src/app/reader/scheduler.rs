/// Visible pages first; then bounded neighbors in the current scroll direction.
pub fn desired_pages(visible: &[u32], total: u32, forward: bool) -> Vec<u32> {
    let Some(first) = visible.iter().copied().min() else {
        return Vec::new();
    };
    let last = visible.iter().copied().max().unwrap();
    let mut desired: Vec<u32> = visible
        .iter()
        .copied()
        .filter(|p| (1..=total).contains(p))
        .collect();
    for distance in 1..=3 {
        let ahead = last.checked_add(distance);
        let behind = first.checked_sub(distance);
        let pair = if forward {
            [ahead, behind]
        } else {
            [behind, ahead]
        };
        for page in pair.into_iter().flatten() {
            if (1..=total).contains(&page) && !desired.contains(&page) {
                desired.push(page);
            }
        }
    }
    desired
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visible_pages_precede_directional_neighbors_and_stay_in_bounds() {
        assert_eq!(
            desired_pages(&[4, 5], 8, true),
            vec![4, 5, 6, 3, 7, 2, 8, 1]
        );
        assert_eq!(desired_pages(&[1], 3, false), vec![1, 2, 3]);
        assert!(desired_pages(&[], 3, true).is_empty());
    }
}
