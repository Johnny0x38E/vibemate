//! Fuzzy search over a provider's models (P12).
//!
//! Search runs in memory in Rust, not in SQLite. SQLite's `LIKE` can only say
//! "contains or not": it cannot rank prefix above contains or find subsequences,
//! and its `%`/`_` wildcards would need escaping. One provider has at most 5000
//! fetched models, and scoring three short fields per row is a linear scan of a
//! few hundred kilobytes, so SQLite only filters by provider and selection.
//!
//! Rules:
//! - **Normalize**: Unicode lowercase. `-`, `_`, `/`, `.` and any whitespace are
//!   one kind of separator. Text is split into words only at separators, never
//!   per character, so `深度求索` is one word.
//! - **Fields**: model ID, upstream name, alias.
//! - **Word tiers** (best over the fields): exact 3 (equals a word of the field),
//!   prefix 2 (starts a word, or the field without separators), contains 1 (inside
//!   the field without separators), subsequence 0 (its characters appear in order
//!   in the field without separators). Every query word must match.
//! - **Rank**: the whole query equals a whole field first, then the weakest word's
//!   tier, then the sum of tiers, then the model ID (byte order) for a stable order.
//!
//! The query is matched as plain characters: `%`, `\`, quotes, `*` and other
//! regex or `LIKE` metacharacters have no special meaning.

/// Longest accepted query, counted in characters.
pub const MAX_QUERY_CHARS: usize = 200;

/// The query is longer than `MAX_QUERY_CHARS` characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryTooLong;

/// A parsed, non-empty query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    /// Normalized words, in query order.
    terms: Vec<String>,
    /// The words joined by one space, compared with whole fields.
    whole: String,
}

/// How well one model matches. Larger is better (the derived order compares the
/// fields top to bottom).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MatchScore {
    /// The whole query equals a whole field (after normalization).
    pub full_exact: bool,
    /// The tier of the weakest-matching word.
    pub weakest: u8,
    /// The sum of every word's tier.
    pub sum: u32,
}

const EXACT: u8 = 3;
const PREFIX: u8 = 2;
const CONTAINS: u8 = 1;
const SUBSEQUENCE: u8 = 0;

fn is_separator(c: char) -> bool {
    matches!(c, '-' | '_' | '/' | '.') || c.is_whitespace()
}

/// Lowercase `text` and split it into words at separators.
fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(is_separator)
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

impl SearchQuery {
    /// Parse a raw query.
    ///
    /// Returns `Ok(None)` when nothing is left after normalization (empty,
    /// whitespace only, or separators only), which callers treat as "no query".
    ///
    /// # Errors
    /// `QueryTooLong` when `raw` has more than `MAX_QUERY_CHARS` characters.
    pub fn parse(raw: &str) -> Result<Option<Self>, QueryTooLong> {
        if raw.chars().count() > MAX_QUERY_CHARS {
            return Err(QueryTooLong);
        }
        let terms = words(raw);
        if terms.is_empty() {
            return Ok(None);
        }
        let whole = terms.join(" ");
        Ok(Some(Self { terms, whole }))
    }

    /// Score a model by its fields; `None` when some word matches no field.
    pub fn score<'a>(&self, fields: impl IntoIterator<Item = &'a str>) -> Option<MatchScore> {
        let fields: Vec<Field> = fields.into_iter().map(Field::new).collect();
        let full_exact = fields.iter().any(|field| field.whole == self.whole);
        let mut weakest = EXACT;
        let mut sum = 0_u32;
        for term in &self.terms {
            let tier = fields.iter().filter_map(|field| field.tier(term)).max()?;
            weakest = weakest.min(tier);
            sum += u32::from(tier);
        }
        Some(MatchScore {
            full_exact,
            weakest,
            sum,
        })
    }
}

/// One normalized field.
struct Field {
    words: Vec<String>,
    /// Words joined without separators, for prefix, contains and subsequence.
    joined: String,
    /// Words joined by one space, for the whole-query comparison.
    whole: String,
}

impl Field {
    fn new(text: &str) -> Self {
        let words = words(text);
        Self {
            joined: words.concat(),
            whole: words.join(" "),
            words,
        }
    }

    fn tier(&self, term: &str) -> Option<u8> {
        if self.words.iter().any(|word| word == term) {
            Some(EXACT)
        } else if self.joined.starts_with(term) || self.words.iter().any(|w| w.starts_with(term)) {
            Some(PREFIX)
        } else if self.joined.contains(term) {
            Some(CONTAINS)
        } else if is_subsequence(term, &self.joined) {
            Some(SUBSEQUENCE)
        } else {
            None
        }
    }
}

/// Whether the characters of `needle` appear in `haystack` in order.
fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut rest = haystack.chars();
    needle.chars().all(|wanted| rest.any(|c| c == wanted))
}

/// Sort scored items best first; equal scores fall back to the key (model ID).
pub fn sort_ranked<T>(items: &mut [(MatchScore, T)], key: impl Fn(&T) -> &str) {
    items.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| key(left).cmp(key(right)))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(raw: &str) -> SearchQuery {
        SearchQuery::parse(raw).unwrap().expect("non-empty query")
    }

    fn score(raw: &str, fields: &[&str]) -> Option<MatchScore> {
        query(raw).score(fields.iter().copied())
    }

    /// Rank model IDs (used as the only field) and return the matches in order.
    fn rank(raw: &str, ids: &[&'static str]) -> Vec<&'static str> {
        let query = query(raw);
        let mut scored: Vec<(MatchScore, &'static str)> = ids
            .iter()
            .filter_map(|id| query.score([*id]).map(|score| (score, *id)))
            .collect();
        sort_ranked(&mut scored, |id| id);
        scored.into_iter().map(|(_, id)| id).collect()
    }

    #[test]
    fn empty_whitespace_and_separator_only_queries_are_no_query() {
        for raw in ["", "   ", "\t\n", "- _ / .", "__", "...", "\u{3000}"] {
            assert_eq!(SearchQuery::parse(raw), Ok(None), "{raw:?}");
        }
    }

    #[test]
    fn length_is_counted_in_characters() {
        let ascii_200 = "a".repeat(200);
        let cjk_200 = "深".repeat(200); // 600 bytes, still 200 characters
        assert!(SearchQuery::parse(&ascii_200).unwrap().is_some());
        assert!(SearchQuery::parse(&cjk_200).unwrap().is_some());
        assert_eq!(SearchQuery::parse(&"a".repeat(201)), Err(QueryTooLong));
        assert_eq!(SearchQuery::parse(&"深".repeat(201)), Err(QueryTooLong));
        // Separators count too: 201 characters is too long even if mostly spaces.
        assert_eq!(
            SearchQuery::parse(&format!("a{}", " ".repeat(200))),
            Err(QueryTooLong)
        );
    }

    #[test]
    fn matching_is_case_insensitive_and_separators_are_equivalent() {
        let id = "Mistral/Mistral-Large_4.1";
        for raw in [
            "mistral large",
            "MISTRAL-LARGE",
            "mistral/large",
            "mistral_large",
            "mistral.large",
            "large 4 1",
        ] {
            assert!(score(raw, &[id]).is_some(), "{raw}");
        }
        // The whole query equals the whole field whatever the separators are.
        assert!(
            score("mistral mistral large 4 1", &[id])
                .unwrap()
                .full_exact
        );
        assert!(
            score("mistral-mistral/large.4_1", &[id])
                .unwrap()
                .full_exact
        );
        assert!(score("mistral large", &["mistral/mistral-large-4"]).is_some());
    }

    #[test]
    fn chinese_names_are_words_not_characters() {
        let fields = ["deepseek-v4", "深度求索"];
        // "深度求索" is one word, so "求索" is inside it (contains), not a word.
        assert_eq!(score("求索", &fields).map(|s| s.weakest), Some(CONTAINS));
        assert_eq!(score("深求", &fields).map(|s| s.weakest), Some(SUBSEQUENCE));
        assert_eq!(score("深度", &fields).map(|s| s.weakest), Some(PREFIX));
        let exact = score("深度求索", &fields).unwrap();
        assert_eq!((exact.weakest, exact.full_exact), (EXACT, true));
        assert_eq!(
            score("索求", &fields),
            None,
            "order matters for subsequences"
        );
        // Mixed scripts across fields, and a separator inside Chinese text.
        assert!(score("求索 v4", &fields).is_some());
        assert_eq!(
            score("深度", &["深度-求索"]).map(|s| s.weakest),
            Some(EXACT)
        );
    }

    #[test]
    fn every_word_must_match_but_words_may_hit_different_fields() {
        assert_eq!(
            rank("ds chat", &["deepseek-chat", "deepseek-reasoner"]),
            ["deepseek-chat"]
        );
        let ds_chat = score("ds chat", &["deepseek-chat"]).unwrap();
        assert_eq!(
            ds_chat,
            MatchScore {
                full_exact: false,
                weakest: SUBSEQUENCE,
                sum: 3
            }
        );
        // "flash" is in the ID, "我的" only in the alias.
        assert!(
            score(
                "flash 我的",
                &["deepseek-v4-flash", "DeepSeek V4", "我的模型"]
            )
            .is_some()
        );
        assert!(score("flash pro", &["deepseek-v4-flash"]).is_none());
    }

    #[test]
    fn special_characters_are_literal() {
        let ids = [
            "gpt-4o",
            "weird%model",
            r"back\slash",
            "it's-quoted",
            "a+b(c)",
        ];
        for raw in [
            "%", r"\", "'", "\"", "*", "+", "(", ")", "[", "]", "^", "$", "|", "?", "{", "}",
        ] {
            let hits = rank(raw, &ids);
            assert!(
                hits.len() <= 1,
                "{raw:?} must not match everything: {hits:?}"
            );
        }
        assert_eq!(rank("%", &ids), ["weird%model"]);
        assert_eq!(rank(r"\", &ids), [r"back\slash"]);
        assert_eq!(rank("'", &ids), ["it's-quoted"]);
        assert_eq!(rank("b(c", &ids), ["a+b(c)"]);
        assert_eq!(
            rank(".*", &ids),
            Vec::<&str>::new(),
            "`.` separates, `*` is literal"
        );
        // `_` is a separator, so it alone is no query rather than a wildcard.
        assert_eq!(SearchQuery::parse("_"), Ok(None));
    }

    #[test]
    fn tiers_order_results_and_ties_are_stable_by_id() {
        let ids = [
            "x-chatty",      // prefix of a word
            "chat",          // exact, and the whole field
            "deepseek-chat", // exact word
            "c-h-a-t-x",     // prefix of the joined field ("chatx")
            "mychatbot",     // contains
            "cohabit",       // subsequence c..h..a..t
            "openai/chat",   // exact word, sorts after deepseek-chat
            "nothing",
        ];
        assert_eq!(
            rank("chat", &ids),
            [
                "chat",
                "deepseek-chat",
                "openai/chat",
                "c-h-a-t-x",
                "x-chatty",
                "mychatbot",
                "cohabit"
            ]
        );
        // The weakest word decides before the sum: (3, 0) loses to (2, 2).
        let strong_weak = score("chat zq", &["chat-zxq"]).unwrap();
        let even = score("chat zx", &["chatty-zxy"]).unwrap();
        assert_eq!((strong_weak.weakest, strong_weak.sum), (SUBSEQUENCE, 3));
        assert_eq!((even.weakest, even.sum), (PREFIX, 4));
        assert!(even > strong_weak);
        // The same input always gives the same order.
        let mut shuffled = ids;
        shuffled.reverse();
        assert_eq!(rank("chat", &shuffled), rank("chat", &ids));
    }
}
