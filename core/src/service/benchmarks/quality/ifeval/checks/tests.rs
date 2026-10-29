use serde_json::json;

use super::*;

fn run(id: &str, kwargs: Value, response: &str) -> Option<bool> {
    check(id, kwargs.as_object().unwrap(), response)
}

/// (instruction, kwargs, passing response, failing response)
fn cases() -> Vec<(&'static str, Value, &'static str, &'static str)> {
    vec![
        (
            "punctuation:no_comma",
            json!({}),
            "no commas here",
            "one, two",
        ),
        (
            "detectable_format:number_highlighted_sections",
            json!({"num_highlights": 2}),
            "*one* and **two**",
            "*one* and ** **",
        ),
        (
            "length_constraints:number_words",
            json!({"num_words": 3, "relation": "at least"}),
            "one two three",
            "one two",
        ),
        (
            "length_constraints:number_words",
            json!({"num_words": 3, "relation": "less than"}),
            "one two",
            "one two three",
        ),
        (
            "length_constraints:number_sentences",
            json!({"num_sentences": 2, "relation": "less than"}),
            "Dr. Smith paid $3.50 today.",
            "One. Two! Three?",
        ),
        (
            "length_constraints:number_paragraphs",
            json!({"num_paragraphs": 2}),
            "first\n***\nsecond\n***",
            "first\n***\n\n***\nsecond",
        ),
        (
            "length_constraints:nth_paragraph_first_word",
            json!({"num_paragraphs": 2, "nth_paragraph": 2, "first_word": "Then"}),
            "First part.\n\n\"Then, the rest.",
            "First part.\n\nAfter that.",
        ),
        (
            "detectable_content:number_placeholders",
            json!({"num_placeholders": 2}),
            "Dear [name], at [address]",
            "Dear [name]",
        ),
        (
            "detectable_content:postscript",
            json!({"postscript_marker": "P.S."}),
            "Hello\np.s. see you",
            "Hello",
        ),
        (
            "detectable_content:postscript",
            json!({"postscript_marker": "P.P.S"}),
            "Hello\nP.P.S more",
            "Hello\nP.S. more",
        ),
        (
            "detectable_format:number_bullet_lists",
            json!({"num_bullets": 2}),
            "* one\n- two",
            "* one\n* two\n* three",
        ),
        (
            "detectable_format:constrained_response",
            json!({}),
            "  My answer is maybe. ",
            "Perhaps",
        ),
        (
            "detectable_format:json_format",
            json!({}),
            "```json\n{\"a\": 1}\n```",
            "{a: 1}",
        ),
        (
            "detectable_format:multiple_sections",
            json!({"section_spliter": "Section", "num_sections": 2}),
            "Section 1\nfoo\nSection 2\nbar",
            "Section 1\nfoo",
        ),
        (
            "detectable_format:title",
            json!({}),
            "<<My Title>>\nbody",
            "<< >>\nbody",
        ),
        (
            "combination:repeat_prompt",
            json!({"prompt_to_repeat": "Say hi."}),
            "say hi. Hi!",
            "Hi! Say hi.",
        ),
        (
            "combination:two_responses",
            json!({}),
            "first\n******\nsecond",
            "same\n******\nsame",
        ),
        (
            "startend:end_checker",
            json!({"end_phrase": "Any other questions?"}),
            "Done. Any other questions?\"",
            "Any other questions? Done.",
        ),
        ("startend:quotation", json!({}), " \"quoted\" ", "\"half"),
        (
            "change_case:capital_word_frequency",
            json!({"capital_frequency": 2, "capital_relation": "at least"}),
            "THIS is VERY loud",
            "THIS is quiet",
        ),
        (
            "change_case:english_capital",
            json!({}),
            "ALL CAPS 123",
            "Not caps",
        ),
        (
            "change_case:english_lowercase",
            json!({}),
            "all lower 1",
            "Not lower",
        ),
        (
            "keywords:existence",
            json!({"keywords": ["moon", "c++"]}),
            "The Moon and C++",
            "The moon",
        ),
        (
            "keywords:forbidden_words",
            json!({"forbidden_words": ["bad"]}),
            "badge is fine",
            "this is Bad",
        ),
        (
            "keywords:frequency",
            json!({"keyword": "cat", "frequency": 2, "relation": "at least"}),
            "Cat, cat",
            "cat",
        ),
        (
            "keywords:letter_frequency",
            json!({"letter": "z", "let_frequency": 2, "let_relation": "less than"}),
            "zoo",
            "Zizz",
        ),
    ]
}

#[test]
fn every_supported_instruction_passes_and_fails_as_in_the_reference() {
    for (id, kwargs, pass, fail) in cases() {
        assert_eq!(
            run(id, kwargs.clone(), pass),
            Some(true),
            "{id} should pass: {pass:?}"
        );
        assert_eq!(
            run(id, kwargs, fail),
            Some(false),
            "{id} should fail: {fail:?}"
        );
    }
}

#[test]
fn cases_cover_every_supported_instruction() {
    let covered: Vec<&str> = cases().iter().map(|c| c.0).collect();
    for id in SUPPORTED {
        assert!(covered.contains(&id), "no case for {id}");
    }
}

#[test]
fn unsupported_instructions_and_bad_kwargs() {
    assert_eq!(
        run(
            "language:response_language",
            json!({"language": "fr"}),
            "Bonjour"
        ),
        None
    );
    // Missing or mistyped kwargs fail rather than panic.
    assert_eq!(
        run("length_constraints:number_words", json!({}), "hi"),
        Some(false)
    );
    assert_eq!(
        run(
            "length_constraints:number_words",
            json!({"num_words": 1, "relation": "about"}),
            "hi"
        ),
        Some(false)
    );
    assert_eq!(
        run("keywords:existence", json!({"keywords": [1]}), "1"),
        Some(false)
    );
}

#[test]
fn loose_variants_drop_edge_lines_and_markdown() {
    let v = loose_variants("Sure!\n**body**\nHope this helps");
    assert_eq!(v.len(), 8);
    assert!(v.contains(&"**body**".to_string()));
    assert!(v.contains(&"body".to_string()));
    assert!(v.contains(&"Sure!\nbody\nHope this helps".to_string()));
    assert_eq!(loose_variants("one line")[3], "");
}
