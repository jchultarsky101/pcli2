//! Groups of assets that match each other.
//!
//! A match report lists pairs. When ten copies of one part are in a folder that
//! is forty-five rows, and what someone cleaning up duplicates wants to know is
//! that those ten files are one part. Two assets are in the same group when a
//! chain of matches connects them (A matches B, B matches C: A, B and C are one
//! group), which is a union-find over the pairs.

use std::collections::HashMap;

/// The group each asset belongs to, numbered from 1 in the order the assets
/// first appear in the pairs, so the same report always numbers its groups the
/// same way.
#[derive(Debug, Default)]
pub struct MatchGroups {
    index: HashMap<String, usize>,
    parent: Vec<usize>,
    /// For each root, its group number and size, filled in by `finish`.
    numbering: Vec<(usize, usize)>,
}

impl MatchGroups {
    /// Group the assets of `pairs` (reference, candidate).
    pub fn from_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut groups = MatchGroups::default();
        for (a, b) in pairs {
            let a = groups.node(a);
            let b = groups.node(b);
            groups.union(a, b);
        }
        groups.finish();
        groups
    }

    fn node(&mut self, key: &str) -> usize {
        if let Some(&i) = self.index.get(key) {
            return i;
        }
        let i = self.parent.len();
        self.parent.push(i);
        self.index.insert(key.to_string(), i);
        i
    }

    fn root(&mut self, mut i: usize) -> usize {
        while self.parent[i] != i {
            self.parent[i] = self.parent[self.parent[i]];
            i = self.parent[i];
        }
        i
    }

    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.root(a), self.root(b));
        if ra != rb {
            // The earlier node stays the root, which keeps numbering stable.
            let (keep, merge) = if ra < rb { (ra, rb) } else { (rb, ra) };
            self.parent[merge] = keep;
        }
    }

    fn finish(&mut self) {
        let n = self.parent.len();
        let roots: Vec<usize> = (0..n).map(|i| self.root(i)).collect();
        let mut sizes = vec![0usize; n];
        for &r in &roots {
            sizes[r] += 1;
        }
        let mut numbers = vec![0usize; n];
        let mut next = 0;
        // Nodes were created in order of first appearance; the first node of each
        // group is its root, so walking nodes in order numbers groups in order.
        for i in 0..n {
            if roots[i] == i {
                next += 1;
                numbers[i] = next;
            }
        }
        self.numbering = roots.iter().map(|&r| (numbers[r], sizes[r])).collect();
    }

    /// The group number and size of `key`, if it appeared in any pair.
    pub fn group_of(&self, key: &str) -> Option<(usize, usize)> {
        self.index.get(key).map(|&i| self.numbering[i])
    }

    /// How many groups there are.
    pub fn count(&self) -> usize {
        self.numbering
            .iter()
            .map(|(number, _)| *number)
            .max()
            .unwrap_or(0)
    }

    /// How many distinct assets appear in the pairs.
    pub fn asset_count(&self) -> usize {
        self.index.len()
    }
}

/// The groups of a match table, from its `REFERENCE_ASSET_UUID` and
/// `CANDIDATE_ASSET_UUID` columns, without changing it.
pub fn groups_of_table(headers: &[String], rows: &[Vec<String>]) -> Option<MatchGroups> {
    let reference = headers.iter().position(|h| h == "REFERENCE_ASSET_UUID")?;
    let candidate = headers.iter().position(|h| h == "CANDIDATE_ASSET_UUID")?;
    Some(MatchGroups::from_pairs(rows.iter().map(|row| {
        (row[reference].as_str(), row[candidate].as_str())
    })))
}

/// Append `GROUP_ID` and `GROUP_SIZE` to a match table, grouping on the
/// `REFERENCE_ASSET_UUID` and `CANDIDATE_ASSET_UUID` columns. Returns the groups,
/// or `None` (and leaves the table alone) when those columns are missing.
pub fn add_group_columns(
    headers: &mut Vec<String>,
    rows: &mut [Vec<String>],
) -> Option<MatchGroups> {
    let reference = headers.iter().position(|h| h == "REFERENCE_ASSET_UUID")?;
    let candidate = headers.iter().position(|h| h == "CANDIDATE_ASSET_UUID")?;
    let groups = MatchGroups::from_pairs(
        rows.iter()
            .map(|row| (row[reference].as_str(), row[candidate].as_str())),
    );
    let columns: Vec<(usize, usize)> = rows
        .iter()
        .map(|row| groups.group_of(&row[reference]).unwrap_or((0, 0)))
        .collect();
    for (row, (number, size)) in rows.iter_mut().zip(columns) {
        row.push(number.to_string());
        row.push(size.to_string());
    }
    headers.push("GROUP_ID".to_string());
    headers.push("GROUP_SIZE".to_string());
    Some(groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chains_of_matches_form_one_group() {
        let groups = MatchGroups::from_pairs([("a", "b"), ("c", "d"), ("b", "e"), ("e", "a")]);
        assert_eq!(groups.group_of("a"), Some((1, 3)));
        assert_eq!(groups.group_of("e"), Some((1, 3)));
        assert_eq!(groups.group_of("c"), Some((2, 2)));
        assert_eq!(groups.count(), 2);
        assert_eq!(groups.asset_count(), 5);
        assert_eq!(groups.group_of("z"), None);
    }

    #[test]
    fn a_late_link_merges_two_groups_under_the_first_number() {
        let groups = MatchGroups::from_pairs([("a", "b"), ("c", "d"), ("d", "a")]);
        assert_eq!(groups.group_of("c"), Some((1, 4)));
        assert_eq!(groups.count(), 1);
    }

    #[test]
    fn group_columns_are_appended_last() {
        let mut headers = vec![
            "REFERENCE_ASSET_UUID".to_string(),
            "CANDIDATE_ASSET_UUID".to_string(),
            "MATCH_PERCENTAGE".to_string(),
        ];
        let mut rows = vec![
            vec!["a".into(), "b".into(), "99".into()],
            vec!["c".into(), "d".into(), "98".into()],
            vec!["b".into(), "a".into(), "99".into()],
        ];
        let groups = add_group_columns(&mut headers, &mut rows).unwrap();
        assert_eq!(headers[3..], ["GROUP_ID", "GROUP_SIZE"]);
        assert_eq!(rows[0][3..], ["1", "2"]);
        assert_eq!(rows[1][3..], ["2", "2"]);
        assert_eq!(rows[2][3..], ["1", "2"]);
        assert_eq!(groups.count(), 2);
    }
}
