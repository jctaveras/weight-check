use std::io::Write;
use std::time::Duration;

use flate2::Compression;
use flate2::write::ZlibEncoder;

use super::*;
use crate::infra::sandbox::{CodeRunner, PythonRunner};

/// What Python's `pickle.dumps(s, protocol=4)` produces for a str: PROTO, FRAME, BINUNICODE.
fn pickle_str(s: &str) -> Vec<u8> {
    let mut body = vec![b'X'];
    body.extend((s.len() as u32).to_le_bytes());
    body.extend(s.as_bytes());
    body.extend([0x94, b'.']); // MEMOIZE, STOP
    let mut out = vec![0x80, 4, 0x95];
    out.extend((body.len() as u64).to_le_bytes());
    out.extend(body);
    out
}

fn encode(pickled: &[u8]) -> String {
    let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
    z.write_all(pickled).unwrap();
    STANDARD.encode(z.finish().unwrap())
}

const TESTS_JSON: &str = r#"[{"input": "1 2\n", "output": "3\n", "testtype": "stdin"}]"#;

#[test]
fn decodes_pickled_compressed_and_plain_tests() {
    let want = vec![TestCase {
        input: "1 2\n".into(),
        output: "3\n".into(),
        testtype: "stdin".into(),
    }];
    assert_eq!(
        decode_tests(&encode(&pickle_str(TESTS_JSON))).unwrap(),
        want
    );
    assert_eq!(decode_tests(TESTS_JSON).unwrap(), want);

    // SHORT_BINUNICODE without a frame (protocol 2–3 style).
    let mut short = vec![0x80, 3, 0x8c, TESTS_JSON.len() as u8];
    short.extend(TESTS_JSON.as_bytes());
    assert_eq!(decode_tests(&encode(&short)).unwrap(), want);
}

#[test]
fn rejects_anything_but_a_pickled_string() {
    // A pickled list (EMPTY_LIST) is not decoded, let alone executed.
    assert!(decode_tests(&encode(&[0x80, 4, b']', b'.'])).is_err());
    assert!(decode_tests("not base64!").is_err());
    // Truncated length.
    assert!(pickled_str(&[0x80, 4, b'X', 9, 0, 0, 0, b'a']).is_none());
    assert!(pickled_str(&[]).is_none());
}

fn problem(starter: &str, metadata: &str) -> Problem {
    Problem {
        question_id: "q1".into(),
        question_content: "Add two numbers.".into(),
        platform: "atcoder".into(),
        contest_date: "2025-02-01T00:00:00".into(),
        difficulty: "easy".into(),
        starter_code: starter.into(),
        public_test_cases: TESTS_JSON.into(),
        private_test_cases: encode(&pickle_str(
            r#"[{"input": "2 2", "output": "4", "testtype": "stdin"}, {"input": "5 5", "output": "10", "testtype": "stdin"}]"#,
        )),
        metadata: metadata.into(),
    }
}

#[test]
fn tests_put_public_first_and_respect_the_cap() {
    let p = problem("", "{}");
    let all = tests(&p, 10).unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].input, "1 2\n");
    assert_eq!(tests(&p, 2).unwrap().len(), 2);
}

#[test]
fn prompts_follow_the_livecodebench_format() {
    let stdin = messages(&problem("", "{}"));
    assert_eq!(stdin[0].role, "system");
    assert!(
        stdin[1].content.starts_with(
            "### Question:\nAdd two numbers.\n\n### Format: Read the inputs from stdin"
        )
    );
    assert!(stdin[1].content.contains("# YOUR CODE HERE"));

    let starter = messages(&problem(
        "class Solution:\n    def add(self, a, b):",
        "{\"func_name\": \"add\"}",
    ));
    assert!(
        starter[1]
            .content
            .contains("```python\nclass Solution:\n    def add(self, a, b):\n```")
    );
    assert!(
        starter[1]
            .content
            .ends_with("### Answer: (use the provided format with backticks)\n\n")
    );
}

#[test]
fn func_name_comes_from_metadata() {
    assert_eq!(
        func_name(&problem("", "{\"func_name\": \"add\"}")).as_deref(),
        Some("add")
    );
    assert_eq!(func_name(&problem("", "{}")), None);
    assert_eq!(func_name(&problem("", "garbage")), None);
}

#[test]
fn output_comparison() {
    assert!(stdout_matches("3\n", "3"));
    assert!(stdout_matches("  1 2 \n3\n\n", "1 2\n3"));
    assert!(stdout_matches("0.3333333", "0.33333333"));
    assert!(!stdout_matches("1 2", "1\n2"));
    assert!(!stdout_matches("4", "3"));

    assert!(functional_matches("[1, 2]\n", "[1,2]"));
    assert!(functional_matches("2.0000000001", "2"));
    assert!(functional_matches("\"ab\"", "\"ab\""));
    assert!(!functional_matches("[1, 2]", "[2, 1]"));
    assert!(!functional_matches("true", "1"));
    assert!(!functional_matches("not json", "1"));
}

/// The real harnesses under python3 (skipped when it isn't installed).
#[tokio::test]
async fn harnesses_run_under_python() {
    let Ok(out) = std::process::Command::new("python3")
        .arg("--version")
        .output()
    else {
        return;
    };
    if !out.status.success() {
        return;
    }
    let py = PythonRunner::new("python3");
    let timeout = Duration::from_secs(10);

    let program = stdin_program("a, b = map(int, input().split())\nprint(a + b)");
    let out = py.run(&program, "2 3\n", timeout).await.unwrap();
    assert!(out.success, "{}", out.stderr);
    assert!(stdout_matches(&out.stdout, "5"));

    let code = "class Solution:\n    def add(self, nums: List[int], k: int) -> List[int]:\n        return [n + k for n in nums]";
    let out = py
        .run(&functional_program(code, "add"), "[1, 2]\n3", timeout)
        .await
        .unwrap();
    assert!(out.success, "{}", out.stderr);
    assert!(functional_matches(&out.stdout, "[4, 5]"), "{}", out.stdout);
}
