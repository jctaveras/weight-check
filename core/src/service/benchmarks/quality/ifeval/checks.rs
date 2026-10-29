//! The IFEval instruction checkers, ported from Google's reference implementation
//! (`instruction_following_eval/instructions.py`). Two deliberate differences:
//! - Words and sentences are split with simple regexes rather than NLTK, so counts can differ
//!   by a word or two from the reference near a threshold.
//! - `change_case:english_*` checks the case but not that the text is English, and
//!   `language:response_language` is not supported at all (no language detector).

use std::sync::LazyLock;

use regex::{Regex, RegexBuilder};
use serde_json::{Map, Value};

/// The instruction ids `check` understands.
pub const SUPPORTED: [&str; 24] = [
    "change_case:capital_word_frequency",
    "change_case:english_capital",
    "change_case:english_lowercase",
    "combination:repeat_prompt",
    "combination:two_responses",
    "detectable_content:number_placeholders",
    "detectable_content:postscript",
    "detectable_format:constrained_response",
    "detectable_format:json_format",
    "detectable_format:multiple_sections",
    "detectable_format:number_bullet_lists",
    "detectable_format:number_highlighted_sections",
    "detectable_format:title",
    "keywords:existence",
    "keywords:forbidden_words",
    "keywords:frequency",
    "keywords:letter_frequency",
    "length_constraints:nth_paragraph_first_word",
    "length_constraints:number_paragraphs",
    "length_constraints:number_sentences",
    "length_constraints:number_words",
    "punctuation:no_comma",
    "startend:end_checker",
    "startend:quotation",
];

/// Typed access to an instruction's kwargs. A missing or mistyped kwarg fails the check.
struct Kwargs<'a>(&'a Map<String, Value>);

impl Kwargs<'_> {
    fn str(&self, key: &str) -> Option<&str> {
        self.0.get(key)?.as_str()
    }

    fn num(&self, key: &str) -> Option<usize> {
        self.0.get(key)?.as_u64().map(|n| n as usize)
    }

    fn strings(&self, key: &str) -> Option<Vec<&str>> {
        self.0
            .get(key)?
            .as_array()?
            .iter()
            .map(Value::as_str)
            .collect()
    }
}

/// `"less than"` → `count < n`; `"at least"` → `count >= n`.
fn compare(relation: Option<&str>, count: usize, n: usize) -> bool {
    match relation {
        Some("less than") => count < n,
        Some("at least") => count >= n,
        _ => false,
    }
}

fn ci(pattern: &str) -> Regex {
    RegexBuilder::new(pattern)
        .case_insensitive(true)
        .build()
        .expect("escaped pattern")
}

fn multiline(pattern: &str) -> Regex {
    RegexBuilder::new(pattern)
        .multi_line(true)
        .build()
        .expect("static pattern")
}

static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\w+").unwrap());
static HIGHLIGHT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*[^\n\*]*\*").unwrap());
static DOUBLE_HIGHLIGHT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\*\*[^\n\*]*\*\*").unwrap());
static PLACEHOLDER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[.*?\]").unwrap());
static STAR_BULLET: LazyLock<Regex> = LazyLock::new(|| multiline(r"^\s*\*[^\*].*$"));
static DASH_BULLET: LazyLock<Regex> = LazyLock::new(|| multiline(r"^\s*-.*$"));
static TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<<[^\n]+>>").unwrap());
static PARAGRAPH_RULE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s?\*\*\*\s?").unwrap());
static SENTENCE_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[.!?]+(\s+|$)").unwrap());
static ABBREVIATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(Mr|Mrs|Ms|Dr|Prof|St|Jr|Sr|vs|etc|e\.g|i\.e|U\.S)\.").unwrap()
});
static DECIMAL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d)\.(\d)").unwrap());

fn count_words(text: &str) -> usize {
    WORD.find_iter(text).count()
}

/// Approximates the reference sentence splitter: protect decimals and common abbreviations,
/// then split after runs of `.`, `!` or `?`.
fn count_sentences(text: &str) -> usize {
    let protected = DECIMAL.replace_all(text, "$1<prd>$2");
    let protected = ABBREVIATION.replace_all(&protected, |c: &regex::Captures| {
        format!("{}<prd>", c[1].replace('.', "<prd>"))
    });
    SENTENCE_END
        .split(&protected)
        .filter(|s| !s.trim().is_empty())
        .count()
}

/// Python's `str.isupper`: at least one cased character and no lowercase ones.
fn is_upper(s: &str) -> bool {
    s.chars().any(char::is_uppercase) && !s.chars().any(char::is_lowercase)
}

/// Python's `str.islower`.
fn is_lower(s: &str) -> bool {
    s.chars().any(char::is_lowercase) && !s.chars().any(char::is_uppercase)
}

/// Whether `response` follows the instruction `id`; `None` for an unsupported instruction.
pub fn check(id: &str, kwargs: &Map<String, Value>, response: &str) -> Option<bool> {
    let kw = Kwargs(kwargs);
    let r = response;
    let ok = match id {
        "punctuation:no_comma" => !r.contains(','),

        "detectable_format:number_highlighted_sections" => {
            let nonempty = |m: regex::Match| !m.as_str().trim_matches('*').trim().is_empty();
            let count = HIGHLIGHT.find_iter(r).filter(|m| nonempty(*m)).count()
                + DOUBLE_HIGHLIGHT
                    .find_iter(r)
                    .filter(|m| nonempty(*m))
                    .count();
            kw.num("num_highlights").is_some_and(|n| count >= n)
        }

        "length_constraints:number_words" => kw
            .num("num_words")
            .is_some_and(|n| compare(kw.str("relation"), count_words(r), n)),

        "length_constraints:number_sentences" => kw
            .num("num_sentences")
            .is_some_and(|n| compare(kw.str("relation"), count_sentences(r), n)),

        "length_constraints:number_paragraphs" => {
            let parts: Vec<&str> = PARAGRAPH_RULE.split(r).collect();
            let mut count = parts.len();
            let mut valid = true;
            for (i, p) in parts.iter().enumerate() {
                if p.trim().is_empty() {
                    if i == 0 || i == parts.len() - 1 {
                        count -= 1;
                    } else {
                        valid = false;
                    }
                }
            }
            valid && kw.num("num_paragraphs") == Some(count)
        }

        "length_constraints:nth_paragraph_first_word" => {
            let (Some(want), Some(nth), Some(first)) = (
                kw.num("num_paragraphs"),
                kw.num("nth_paragraph"),
                kw.str("first_word"),
            ) else {
                return Some(false);
            };
            let parts: Vec<&str> = r.split("\n\n").collect();
            let count = parts.iter().filter(|p| !p.trim().is_empty()).count();
            let paragraph = (nth >= 1 && nth <= count)
                .then(|| parts.get(nth - 1).map(|p| p.trim()))
                .flatten()
                .filter(|p| !p.is_empty());
            let word: String = paragraph
                .and_then(|p| p.split_whitespace().next())
                .map(|w| w.trim_start_matches('\'').trim_start_matches('"'))
                .unwrap_or("")
                .chars()
                .take_while(|c| !matches!(c, '.' | ',' | '?' | '!' | '\'' | '"'))
                .flat_map(char::to_lowercase)
                .collect();
            count == want && paragraph.is_some() && word == first.to_lowercase()
        }

        "detectable_content:number_placeholders" => kw
            .num("num_placeholders")
            .is_some_and(|n| PLACEHOLDER.find_iter(r).count() >= n),

        "detectable_content:postscript" => {
            let Some(marker) = kw.str("postscript_marker") else {
                return Some(false);
            };
            let pattern = match marker {
                "P.P.S" => r"\s*p\.\s?p\.\s?s.*$".to_string(),
                "P.S." => r"\s*p\.\s?s\..*$".to_string(),
                m => format!(r"\s*{}.*$", regex::escape(&m.to_lowercase())),
            };
            multiline(&pattern).is_match(&r.to_lowercase())
        }

        "detectable_format:number_bullet_lists" => kw.num("num_bullets").is_some_and(|n| {
            STAR_BULLET.find_iter(r).count() + DASH_BULLET.find_iter(r).count() == n
        }),

        "detectable_format:constrained_response" => {
            let r = r.trim();
            [
                "My answer is yes.",
                "My answer is no.",
                "My answer is maybe.",
            ]
            .iter()
            .any(|o| r.contains(o))
        }

        "detectable_format:json_format" => {
            let mut v = r.trim();
            for prefix in ["```json", "```Json", "```JSON", "```"] {
                if let Some(rest) = v.strip_prefix(prefix) {
                    v = rest;
                    break;
                }
            }
            let v = v.strip_suffix("```").unwrap_or(v).trim();
            serde_json::from_str::<Value>(v).is_ok()
        }

        "detectable_format:multiple_sections" => {
            let (Some(splitter), Some(n)) = (kw.str("section_spliter"), kw.num("num_sections"))
            else {
                return Some(false);
            };
            let pattern = Regex::new(&format!(r"\s?{}\s?\d+\s?", regex::escape(splitter)))
                .expect("escaped pattern");
            // Splitting yields one more piece (the preamble) than there are sections.
            pattern.split(r).count() > n
        }

        "detectable_format:title" => TITLE.find_iter(r).any(|m| {
            !m.as_str()
                .trim_start_matches('<')
                .trim_end_matches('>')
                .trim()
                .is_empty()
        }),

        "combination:repeat_prompt" => kw.str("prompt_to_repeat").is_some_and(|p| {
            r.trim()
                .to_lowercase()
                .starts_with(&p.trim().to_lowercase())
        }),

        "combination:two_responses" => {
            let parts: Vec<&str> = r.split("******").collect();
            let mut valid = Vec::new();
            for (i, p) in parts.iter().enumerate() {
                if p.trim().is_empty() {
                    if i != 0 && i != parts.len() - 1 {
                        return Some(false);
                    }
                } else {
                    valid.push(p.trim());
                }
            }
            valid.len() == 2 && valid[0] != valid[1]
        }

        "startend:end_checker" => kw.str("end_phrase").is_some_and(|p| {
            r.trim()
                .trim_matches('"')
                .to_lowercase()
                .ends_with(&p.trim().to_lowercase())
        }),

        "startend:quotation" => {
            let r = r.trim();
            r.len() > 1 && r.starts_with('"') && r.ends_with('"')
        }

        "change_case:capital_word_frequency" => kw.num("capital_frequency").is_some_and(|n| {
            let capitals = WORD.find_iter(r).filter(|w| is_upper(w.as_str())).count();
            compare(kw.str("capital_relation"), capitals, n)
        }),

        "change_case:english_capital" => is_upper(r),
        "change_case:english_lowercase" => is_lower(r),

        "keywords:existence" => kw
            .strings("keywords")
            .is_some_and(|words| words.iter().all(|w| ci(&regex::escape(w)).is_match(r))),

        "keywords:forbidden_words" => kw.strings("forbidden_words").is_some_and(|words| {
            !words
                .iter()
                .any(|w| ci(&format!(r"\b{}\b", regex::escape(w))).is_match(r))
        }),

        "keywords:frequency" => match (kw.str("keyword"), kw.num("frequency")) {
            (Some(word), Some(n)) => {
                let count = ci(&regex::escape(word)).find_iter(r).count();
                compare(kw.str("relation"), count, n)
            }
            _ => false,
        },

        "keywords:letter_frequency" => match (kw.str("letter"), kw.num("let_frequency")) {
            (Some(letter), Some(n)) => {
                let count = r.to_lowercase().matches(&letter.to_lowercase()).count();
                compare(kw.str("let_relation"), count, n)
            }
            _ => false,
        },

        _ => return None,
    };
    Some(ok)
}

/// The variants loose scoring tries: the response with its first and/or last line removed,
/// each with and without markdown `*`.
pub fn loose_variants(response: &str) -> Vec<String> {
    let lines: Vec<&str> = response.split('\n').collect();
    let n = lines.len();
    let join = |from: usize, to: usize| {
        if from >= to {
            String::new()
        } else {
            lines[from..to].join("\n").trim().to_string()
        }
    };
    let bases = [
        response.to_string(),
        join(1, n),
        join(0, n.saturating_sub(1)),
        join(1, n.saturating_sub(1)),
    ];
    let mut variants: Vec<String> = bases.to_vec();
    variants.extend(bases.iter().map(|b| b.replace('*', "")));
    variants
}

#[cfg(test)]
mod tests;
