pub const MAX_SLUG_LEN: usize = 80;
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

/// Build a collision candidate: `base` for attempt 1, `base-N` for attempt >= 2.
/// Respects MAX_SLUG_LEN — if appending `-N` would exceed the cap, the base is
/// truncated (preferring a hyphen boundary near the cut point) before the suffix
/// is added. Callers should keep incrementing `attempt` until the DB accepts the
/// slug via its UNIQUE constraint.
pub fn with_suffix(base: &str, attempt: u32) -> String {
    debug_assert!(attempt >= 1, "attempt is 1-indexed");
    if attempt <= 1 {
        return base.to_string();
    }
    let suffix = format!("-{}", attempt);
    let room = MAX_SLUG_LEN.saturating_sub(suffix.len());
    if base.len() <= room {
        return format!("{}{}", base, suffix);
    }
    let trimmed = &base[..room];
    let cut = trimmed
        .rfind('-')
        .filter(|&i| i >= room / 2)
        .unwrap_or(room);
    format!("{}{}", &base[..cut], suffix)
}

#[cfg(test)]
mod tests {
    use super::{slugify, with_suffix, MAX_SLUG_LEN};

    #[test]
    fn lowercases_and_hyphenates() {
        assert_eq!(slugify("Pizza Margherita"), "pizza-margherita");
    }

    #[test]
    fn collapses_punctuation_and_whitespace() {
        assert_eq!(
            slugify("Grandma's  Sunday Roast!"),
            "grandma-s-sunday-roast"
        );
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

    #[test]
    fn with_suffix_keeps_base_at_attempt_one() {
        assert_eq!(with_suffix("pasta", 1), "pasta");
    }

    #[test]
    fn with_suffix_appends_for_collisions() {
        assert_eq!(with_suffix("pasta", 2), "pasta-2");
        assert_eq!(with_suffix("pasta", 9), "pasta-9");
    }

    #[test]
    fn with_suffix_respects_max_len_for_long_bases() {
        // A max-length base that'd overflow once a suffix is tacked on.
        let max_base = "a".repeat(MAX_SLUG_LEN);
        let s = with_suffix(&max_base, 42);
        assert!(s.len() <= MAX_SLUG_LEN);
        assert!(s.ends_with("-42"));
    }

    #[test]
    fn with_suffix_prefers_hyphen_boundary_when_truncating() {
        // Build a base at MAX_SLUG_LEN where a hyphen sits in the trim zone.
        // Construction: 70 `a`s, a hyphen, then 9 `b`s (total = 80).
        let mut base = "a".repeat(70);
        base.push('-');
        base.push_str(&"b".repeat(9));
        assert_eq!(base.len(), MAX_SLUG_LEN);

        let s = with_suffix(&base, 2);
        assert!(s.len() <= MAX_SLUG_LEN);
        // Should have trimmed back to the hyphen boundary, not mid-`a`-run.
        assert!(s.starts_with(&"a".repeat(70)));
        assert!(s.ends_with("-2"));
    }
}
