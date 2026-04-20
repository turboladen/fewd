const MAX_SLUG_LEN: usize = 80;
const FALLBACK_SLUG: &str = "recipe";

/// Convert a human-readable name into a URL-safe slug.
///
/// Rules:
/// - lowercase ASCII
/// - common Latin accents stripped (é → e, ñ → n, etc.)
/// - any run of non-alphanumerics collapses to a single `-`
/// - leading/trailing `-` trimmed
/// - capped at 80 chars (on a word boundary when possible)
/// - if the result is empty (e.g., name was only emoji/punctuation), falls back to `"recipe"`
pub fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut last_was_sep = true;

    let mut push_ascii = |c: char| {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_was_sep = false;
        } else if !last_was_sep {
            out.push('-');
            last_was_sep = true;
        }
    };

    for ch in name.chars() {
        match fold(ch) {
            Some(expanded) => {
                for c in expanded.chars() {
                    push_ascii(c);
                }
            }
            None => push_ascii(ch),
        }
    }

    while out.ends_with('-') {
        out.pop();
    }

    if out.len() > MAX_SLUG_LEN {
        let cut = out[..MAX_SLUG_LEN]
            .rfind('-')
            .filter(|&i| i >= MAX_SLUG_LEN / 2)
            .unwrap_or(MAX_SLUG_LEN);
        out.truncate(cut);
        while out.ends_with('-') {
            out.pop();
        }
    }

    if out.is_empty() {
        return FALLBACK_SLUG.to_string();
    }

    out
}

/// Expand a non-ASCII char into an ASCII equivalent, or return `None` to pass through
/// (ASCII chars go through the default path; unknown non-ASCII gets discarded because
/// `is_ascii_alphanumeric` returns false).
fn fold(ch: char) -> Option<&'static str> {
    // Unicode-lowercase first so we only match on lowercase forms. `to_lowercase`
    // returns an iterator because some lowercase expansions are multi-char — for
    // every char we match here the expansion is single, so `.next()` is safe.
    let ch = ch.to_lowercase().next().unwrap_or(ch);
    Some(match ch {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'æ' => "ae",
        'ç' | 'ć' | 'č' => "c",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'į' => "i",
        'ñ' | 'ń' | 'ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => "o",
        'œ' => "oe",
        'ß' => "ss",
        'š' | 'ś' => "s",
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' => "u",
        'ý' | 'ÿ' => "y",
        'ž' | 'ź' | 'ż' => "z",
        '&' => " and ",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::slugify;

    #[test]
    fn lowercases_and_hyphenates() {
        assert_eq!(slugify("Pizza Margherita"), "pizza-margherita");
    }

    #[test]
    fn collapses_punctuation_and_whitespace() {
        assert_eq!(slugify("Grandma's  Sunday Roast!"), "grandma-s-sunday-roast");
    }

    #[test]
    fn strips_accents() {
        assert_eq!(slugify("Crème Brûlée"), "creme-brulee");
        assert_eq!(slugify("Jalapeño Poppers"), "jalapeno-poppers");
        // Uppercase accents fold via Unicode lowercase before the match.
        assert_eq!(slugify("ÉCLAIR"), "eclair");
        assert_eq!(slugify("CAFÉ"), "cafe");
    }

    #[test]
    fn falls_back_for_empty_after_strip() {
        assert_eq!(slugify("¿¡"), "recipe");
        assert_eq!(slugify("🍕🍕"), "recipe");
        assert_eq!(slugify(""), "recipe");
    }

    #[test]
    fn caps_length_on_word_boundary() {
        let long = "a".repeat(50) + " " + &"b".repeat(50);
        let s = slugify(&long);
        assert!(s.len() <= 80);
        assert!(!s.ends_with('-'));
    }

    #[test]
    fn expands_ampersand() {
        assert_eq!(slugify("Mac & Cheese"), "mac-and-cheese");
    }
}
