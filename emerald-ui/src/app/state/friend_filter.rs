//! The Friends page's search: which friends a query keeps.

/// Whether a friend shown as `name` with `username` matches `query`: by
/// either, ignoring case and accents; `@query` looks at usernames only.
pub fn friend_matches(query: &str, name: &str, username: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return true;
    }
    match query.strip_prefix('@') {
        Some(username_query) => fold(username).contains(&fold(username_query)),
        None => {
            let query = fold(query);
            fold(name).contains(&query) || fold(username).contains(&query)
        }
    }
}

/// Lowercase without the accents Latin letters carry, so "João" and "joao"
/// compare equal.
fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            'ý' | 'ÿ' => 'y',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friend_search_ignores_case_and_accents() {
        assert!(friend_matches("joao", "João", "jj"));
        assert!(friend_matches("JOÃO", "joão", "jj"));
    }

    #[test]
    fn at_sign_searches_usernames_only() {
        assert!(friend_matches("@jj", "João", "jj"));
        assert!(!friend_matches("@joao", "João", "jj"));
    }

    #[test]
    fn an_empty_query_matches_everyone() {
        assert!(friend_matches("", "a", "b"));
        assert!(friend_matches("  ", "a", "b"));
    }

    #[test]
    fn plain_queries_also_find_usernames() {
        assert!(friend_matches("jj", "João", "jj"));
    }
}
