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
    Some(match ch {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'Ā' | 'Ă' | 'Ą' => "a",
        'æ' | 'Æ' => "ae",
        'ç' | 'ć' | 'č' | 'Ç' | 'Ć' | 'Č' => "c",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'È' | 'É' | 'Ê' | 'Ë' | 'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => "e",
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'į' | 'Ì' | 'Í' | 'Î' | 'Ï' | 'Ī' | 'Į' => "i",
        'ñ' | 'ń' | 'ň' | 'Ñ' | 'Ń' | 'Ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => "o",
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' | 'Ō' | 'Ő' => "o",
        'œ' | 'Œ' => "oe",
        'ß' => "ss",
        'š' | 'ś' | 'Š' | 'Ś' => "s",
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' => "u",
        'Ù' | 'Ú' | 'Û' | 'Ü' | 'Ū' | 'Ů' | 'Ű' => "u",
        'ý' | 'ÿ' | 'Ý' | 'Ÿ' => "y",
        'ž' | 'ź' | 'ż' | 'Ž' | 'Ź' | 'Ż' => "z",
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
