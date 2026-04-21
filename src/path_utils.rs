//! Path utilities for fuzzy matching and suggestions.
//!
//! This module provides utilities for finding similar paths when an exact match fails,
//! helping users discover the correct path when they make typos or case errors.

use crate::folder_hierarchy::FolderHierarchy;

/// Find similar folder paths using case-insensitive matching.
///
/// # Arguments
/// * `hierarchy` - The folder hierarchy to search
/// * `target_path` - The path that was not found
///
/// # Returns
/// A vector of suggested paths, sorted by similarity (most similar first)
pub fn find_similar_paths(hierarchy: &FolderHierarchy, target_path: &str) -> Vec<String> {
    let target_normalized = target_path.to_lowercase();
    let mut suggestions: Vec<(String, usize)> = Vec::new();

    for folder_node in hierarchy.nodes.values() {
        let folder_path = hierarchy
            .get_path_for_folder(folder_node.uuid())
            .unwrap_or_else(|| folder_node.name().to_string());

        let folder_normalized = folder_path.to_lowercase();

        // Check for case-insensitive match
        if folder_normalized == target_normalized {
            // Perfect case-insensitive match - highest priority
            return vec![folder_path];
        }

        // Check for partial matches (contains the target or vice versa)
        let similarity = calculate_similarity(&target_normalized, &folder_normalized);
        if similarity > 0 {
            suggestions.push((folder_path, similarity));
        }
    }

    // Sort by similarity (highest first) and return top suggestions
    suggestions.sort_by_key(|s| std::cmp::Reverse(s.1));
    suggestions
        .into_iter()
        .take(3)
        .map(|(path, _)| path)
        .collect()
}

/// Calculate a simple similarity score between two strings.
///
/// Uses a combination of:
/// - Common prefix length
/// - Common substring matching
/// - Levenshtein distance (for short strings)
///
/// # Arguments
/// * `s1` - First string (already normalized)
/// * `s2` - Second string (already normalized)
///
/// # Returns
/// A similarity score (higher = more similar)
fn calculate_similarity(s1: &str, s2: &str) -> usize {
    // Check if one contains the other
    if s1.contains(s2) || s2.contains(s1) {
        return s1.len().min(s2.len()) * 2;
    }

    // Calculate common prefix
    let common_prefix = s1
        .chars()
        .zip(s2.chars())
        .take_while(|(c1, c2)| c1 == c2)
        .count();

    // Calculate Levenshtein distance for short strings
    if s1.len() < 50 && s2.len() < 50 {
        let distance = levenshtein_distance(s1, s2);
        let max_len = s1.len().max(s2.len());
        if max_len > 0 {
            return (max_len - distance) * 2 + common_prefix;
        }
    }

    // For longer strings, just use common prefix
    common_prefix
}

/// Calculate the Levenshtein distance between two strings.
///
/// The Levenshtein distance is the minimum number of single-character edits
/// (insertions, deletions, or substitutions) required to change one string into the other.
///
/// # Arguments
/// * `s1` - First string
/// * `s2` - Second string
///
/// # Returns
/// The Levenshtein distance
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let s1_chars: Vec<char> = s1.chars().collect();
    let s2_chars: Vec<char> = s2.chars().collect();

    let len1 = s1_chars.len();
    let len2 = s2_chars.len();

    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    // Create a matrix of distances
    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

    // Initialize first row and column
    #[allow(clippy::needless_range_loop)]
    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    #[allow(clippy::needless_range_loop)]
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    // Fill in the rest of the matrix
    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if s1_chars[i - 1] == s2_chars[j - 1] {
                0
            } else {
                1
            };

            matrix[i][j] = (matrix[i - 1][j] + 1) // deletion
                .min(matrix[i][j - 1] + 1) // insertion
                .min(matrix[i - 1][j - 1] + cost); // substitution
        }
    }

    matrix[len1][len2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(levenshtein_distance("", "abc"), 3);
        assert_eq!(levenshtein_distance("abc", ""), 3);
        assert_eq!(levenshtein_distance("abc", "abc"), 0);
        assert_eq!(levenshtein_distance("abc", "abd"), 1);
    }

    #[test]
    fn test_similarity_scoring() {
        // Exact match should have high similarity
        assert!(calculate_similarity("test", "test") > 0);

        // Similar strings should have positive similarity
        assert!(calculate_similarity("test", "tent") > 0);

        // Very different strings should have low or zero similarity
        assert_eq!(calculate_similarity("abc", "xyz"), 0);
    }
}
