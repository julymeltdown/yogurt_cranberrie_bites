//! Shared handling for user-supplied search input.
//!
//! Search terms reach SQL LIKE patterns directly, and LIKE treats percent and
//! underscore as wildcards. An unescaped term therefore changes the query's
//! meaning: a search for a literal percent sign matched every row, and an
//! underscore matched any single character. Both are also unindexable, so the
//! query degraded into a sequential scan.
//!
//! Escaping alone is not enough to make a search safe, so a length bound is
//! enforced here too: a one-character term matches a large fraction of any table
//! and cannot be useful to a caller.

/// Shortest term accepted. A single character is not a meaningful search and
/// would match an unhelpfully large portion of the table.
pub const MIN_SEARCH_TERM_CHARS: usize = 2;

/// Longest term accepted. Bounds both the pattern size and the work it implies.
pub const MAX_SEARCH_TERM_CHARS: usize = 100;

/// Why a search term was rejected.
///
/// Returned rather than folded into the term so the caller can produce a
/// transport-appropriate error instead of silently searching for something else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchTermError {
    /// The term is empty once surrounding whitespace is removed.
    Empty,
    /// The term is shorter than MIN_SEARCH_TERM_CHARS.
    TooShort,
    /// The term is longer than MAX_SEARCH_TERM_CHARS.
    TooLong,
}

impl std::fmt::Display for SearchTermError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchTermError::Empty => formatter.write_str("Search query must not be empty"),
            SearchTermError::TooShort => write!(
                formatter,
                "Search query must be at least {MIN_SEARCH_TERM_CHARS} characters"
            ),
            SearchTermError::TooLong => write!(
                formatter,
                "Search query must be at most {MAX_SEARCH_TERM_CHARS} characters"
            ),
        }
    }
}

impl std::error::Error for SearchTermError {}

/// Validate and normalize a raw search term.
///
/// Returns the trimmed, length-checked term. The caller escapes it separately
/// with escape_like when binding a pattern, so validation errors and pattern
/// construction stay independent.
pub fn normalize_search_term(raw: &str) -> Result<String, SearchTermError> {
    let term = raw.trim();
    if term.is_empty() {
        return Err(SearchTermError::Empty);
    }
    // Count characters, not bytes, so a multi-byte term is judged by its visible
    // length rather than being rejected for its encoding.
    let length = term.chars().count();
    if length < MIN_SEARCH_TERM_CHARS {
        return Err(SearchTermError::TooShort);
    }
    if length > MAX_SEARCH_TERM_CHARS {
        return Err(SearchTermError::TooLong);
    }
    Ok(term.to_owned())
}

/// Escape the wildcard characters that LIKE would otherwise interpret.
///
/// The escape character and both wildcards are escaped. The caller must pair
/// this with an explicit escape clause in the query, so a backslash is read as
/// an escape rather than as a literal character.
pub fn escape_like(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for character in input.chars() {
        match character {
            // The escape character is handled first: escaping a wildcard
            // introduces a backslash that must not itself be re-escaped.
            '\\' => escaped.push_str("\\\\"),
            '%' => escaped.push_str("\\%"),
            '_' => escaped.push_str("\\_"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// Build the case-insensitive contains pattern used by search queries.
///
/// Lowercasing matches the lower(column) like ... form used in the queries, so
/// the comparison is case-insensitive without an extra function around the bound
/// parameter.
pub fn contains_pattern(input: &str) -> String {
    format!("%{}%", escape_like(&input.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcards_are_escaped_so_they_match_literally() {
        // Without escaping, this pattern matched every row in the table.
        assert_eq!(contains_pattern("%"), "%\\%%");
        assert_eq!(contains_pattern("_"), "%\\_%");
        // A real term is unaffected.
        assert_eq!(contains_pattern("hana"), "%hana%");
    }

    #[test]
    fn the_escape_character_is_escaped_before_the_wildcards() {
        // A term containing a backslash must not turn the following character
        // into an escape sequence.
        assert_eq!(escape_like("a\\b"), "a\\\\b");
    }

    #[test]
    fn surrounding_whitespace_is_trimmed_and_length_is_measured_in_characters() {
        assert_eq!(normalize_search_term("  hana  ").expect("term"), "hana");
        // Two multi-byte characters are long enough; length is not byte count.
        assert!(normalize_search_term("한나").is_ok());
    }

    #[test]
    fn terms_outside_the_bounds_are_rejected_with_a_reason() {
        assert_eq!(normalize_search_term("   "), Err(SearchTermError::Empty));
        assert_eq!(normalize_search_term("h"), Err(SearchTermError::TooShort));
        assert_eq!(
            normalize_search_term(&"h".repeat(MAX_SEARCH_TERM_CHARS + 1)),
            Err(SearchTermError::TooLong)
        );
        assert!(normalize_search_term(&"h".repeat(MAX_SEARCH_TERM_CHARS)).is_ok());
    }

    #[test]
    fn a_literal_wildcard_term_is_a_valid_search() {
        // The term is allowed; the pattern must treat the percent sign as data.
        assert_eq!(normalize_search_term("50%").expect("term"), "50%");
        assert_eq!(contains_pattern("50%"), "%50\\%%");
    }
}
