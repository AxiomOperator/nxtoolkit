//! Tiny shell-quoting helpers for generated scripts.

/// Quote a single word for POSIX shells, leaving obviously-safe words bare.
pub fn quote(word: &str) -> String {
    let safe = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._+:@/=,-%".contains(c));
    if safe {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}

/// Quote and space-join a list of words.
pub fn join<S: AsRef<str>>(words: &[S]) -> String {
    words
        .iter()
        .map(|w| quote(w.as_ref()))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting() {
        assert_eq!(quote("firefox"), "firefox");
        assert_eq!(quote("org.mozilla.firefox"), "org.mozilla.firefox");
        assert_eq!(quote("a b"), "'a b'");
        assert_eq!(quote("it's"), r"'it'\''s'");
        assert_eq!(quote(""), "''");
        assert_eq!(join(&["a", "b c"]), "a 'b c'");
    }
}
