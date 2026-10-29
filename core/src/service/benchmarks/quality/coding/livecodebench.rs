//! LiveCodeBench problems: the official prompts, test decoding and output comparison.

use std::io::Read;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use flate2::read::ZlibDecoder;
use serde::Deserialize;
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::infra::llama::ChatMessage;

#[derive(Debug, Clone, Deserialize)]
pub struct Problem {
    pub question_id: String,
    pub question_content: String,
    pub platform: String,
    pub contest_date: String,
    pub difficulty: String,
    pub starter_code: String,
    /// JSON list of test cases.
    pub public_test_cases: String,
    /// JSON, or a base64 + zlib + pickled string of JSON.
    pub private_test_cases: String,
    /// JSON object; LeetCode problems name the method to call (`func_name`).
    pub metadata: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TestCase {
    pub input: String,
    pub output: String,
    /// `stdin` (a whole program) or `functional` (a `Solution` method).
    pub testtype: String,
}

const SYSTEM: &str = "You are an expert Python programmer. You will be given a question (problem specification) and will generate a correct Python program that matches the specification and passes all tests.";

const FORMAT_STDIN: &str = "Read the inputs from stdin solve the problem and write the answer to stdout (do not directly test on the sample inputs). Enclose your code within delimiters as follows. Ensure that when the python program runs, it reads the inputs, runs the algorithm and writes output to STDOUT.";

const FORMAT_STARTER: &str = "You will use the following starter code to write the solution to the problem and enclose your code within delimiters.";

/// The prompt LiveCodeBench uses for chat models.
pub fn messages(p: &Problem) -> Vec<ChatMessage> {
    let mut user = format!("### Question:\n{}\n\n", p.question_content);
    if p.starter_code.trim().is_empty() {
        user.push_str(&format!(
            "### Format: {FORMAT_STDIN}\n```python\n# YOUR CODE HERE\n```\n\n"
        ));
    } else {
        user.push_str(&format!(
            "### Format: {FORMAT_STARTER}\n```python\n{}\n```\n\n",
            p.starter_code
        ));
    }
    user.push_str("### Answer: (use the provided format with backticks)\n\n");
    vec![
        ChatMessage {
            role: "system".into(),
            content: SYSTEM.into(),
        },
        ChatMessage::user(user),
    ]
}

fn decode_err(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("decoding LiveCodeBench tests: {e}"))
}

/// The string payload of a pickle that holds a single `str`, without running the unpickler:
/// `PROTO n`, an optional `FRAME`, then `SHORT_BINUNICODE`, `BINUNICODE` or `BINUNICODE8`.
fn pickled_str(b: &[u8]) -> Option<&str> {
    let mut i = 0;
    if *b.first()? == 0x80 {
        i = 2;
    }
    if *b.get(i)? == 0x95 {
        i += 9;
    }
    let op = *b.get(i)?;
    i += 1;
    let len_bytes = match op {
        0x8c => 1,
        b'X' => 4,
        0x8d => 8,
        _ => return None,
    };
    let mut len = [0u8; 8];
    len[..len_bytes].copy_from_slice(b.get(i..i + len_bytes)?);
    i += len_bytes;
    let len = usize::try_from(u64::from_le_bytes(len)).ok()?;
    std::str::from_utf8(b.get(i..i.checked_add(len)?)?).ok()
}

/// Decodes `private_test_cases`: plain JSON, or base64(zlib(pickle(str of JSON))).
pub fn decode_tests(raw: &str) -> AppResult<Vec<TestCase>> {
    if raw.trim_start().starts_with('[') {
        return serde_json::from_str(raw).map_err(decode_err);
    }
    let compressed = STANDARD.decode(raw.trim()).map_err(decode_err)?;
    let mut pickled = Vec::new();
    ZlibDecoder::new(compressed.as_slice())
        .read_to_end(&mut pickled)
        .map_err(decode_err)?;
    let json = pickled_str(&pickled).ok_or_else(|| decode_err("not a pickled string"))?;
    serde_json::from_str(json).map_err(decode_err)
}

/// Public then private tests, at most `max`.
pub fn tests(p: &Problem, max: usize) -> AppResult<Vec<TestCase>> {
    let mut all: Vec<TestCase> = serde_json::from_str(&p.public_test_cases).map_err(decode_err)?;
    all.extend(decode_tests(&p.private_test_cases)?);
    all.truncate(max);
    Ok(all)
}

pub fn func_name(p: &Problem) -> Option<String> {
    let meta: Value = serde_json::from_str(&p.metadata).ok()?;
    meta.get("func_name")?.as_str().map(str::to_string)
}

/// Imports LiveCodeBench makes available to every solution.
const PRELUDE: &str = "from string import *\nfrom re import *\nfrom datetime import *\nfrom collections import *\nfrom heapq import *\nfrom bisect import *\nfrom copy import *\nfrom math import *\nfrom random import *\nfrom statistics import *\nfrom itertools import *\nfrom functools import *\nfrom operator import *\nfrom io import *\nfrom sys import *\nfrom json import *\nfrom builtins import *\nfrom typing import *\nimport string\nimport re\nimport datetime\nimport collections\nimport heapq\nimport bisect\nimport copy\nimport math\nimport random\nimport statistics\nimport itertools\nimport functools\nimport operator\nimport io\nimport sys\nimport json\nsys.setrecursionlimit(50000)\n";

/// A program that runs a stdin-style solution as is.
pub fn stdin_program(code: &str) -> String {
    format!("{PRELUDE}\n{code}\n")
}

/// A program that reads one JSON argument per stdin line, calls `Solution().<func>(*args)` and
/// prints the result as JSON.
pub fn functional_program(code: &str, func: &str) -> String {
    format!(
        "{PRELUDE}\n{code}\n\nimport json as __json, sys as __sys\n__args = [__json.loads(l) for l in __sys.stdin.read().split(\"\\n\") if l.strip()]\nprint(__json.dumps(Solution().{func}(*__args)))\n"
    )
}

fn close(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(x), Ok(y)) => (x - y).abs() <= 1e-6 * y.abs().max(1.0),
        _ => false,
    }
}

/// stdin problems: same lines and tokens, ignoring surrounding whitespace, with a float tolerance.
pub fn stdout_matches(actual: &str, expected: &str) -> bool {
    let lines = |s: &str| -> Vec<Vec<String>> {
        s.trim()
            .lines()
            .map(|l| l.split_whitespace().map(str::to_string).collect())
            .collect()
    };
    let (a, e) = (lines(actual), lines(expected));
    a.len() == e.len()
        && a.iter()
            .zip(&e)
            .all(|(la, le)| la.len() == le.len() && la.iter().zip(le).all(|(x, y)| close(x, y)))
}

fn json_matches(a: &Value, e: &Value) -> bool {
    match (a, e) {
        (Value::Number(x), Value::Number(y)) => match (x.as_f64(), y.as_f64()) {
            (Some(x), Some(y)) => (x - y).abs() <= 1e-6 * y.abs().max(1.0),
            _ => x == y,
        },
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(a, e)| json_matches(a, e))
        }
        _ => a == e,
    }
}

/// Functional problems: the printed JSON equals the expected JSON (floats within tolerance).
pub fn functional_matches(actual: &str, expected: &str) -> bool {
    match (
        serde_json::from_str::<Value>(actual.trim()),
        serde_json::from_str::<Value>(expected.trim()),
    ) {
        (Ok(a), Ok(e)) => json_matches(&a, &e),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
