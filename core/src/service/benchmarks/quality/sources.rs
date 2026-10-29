//! Pinned dataset files. Bumping a revision changes what a benchmark measures, so treat it like
//! a breaking change: runs before and after are no longer comparable.

use crate::infra::datasets::DatasetFile;

/// MMLU (Hendrycks et al., 2021), MIT. 14,042 test questions over 57 subjects.
pub const MMLU_TEST: DatasetFile = DatasetFile {
    repo: "cais/mmlu",
    revision: "c30699e8356da336a370243923dbaf21066bb9fe",
    path: "all/test-00000-of-00001.parquet",
    sha256: "74a41822ce7d3def56e1682f958469c04642a5336a5ce912fa375fdb90fb25d7",
};

/// MMLU's dev split: five few-shot examples per subject.
pub const MMLU_DEV: DatasetFile = DatasetFile {
    repo: "cais/mmlu",
    revision: "c30699e8356da336a370243923dbaf21066bb9fe",
    path: "all/dev-00000-of-00001.parquet",
    sha256: "2b19bde1ed8ca6b482fb283abc90e8e0d9d228947029c0b91795d64b28b3bc3f",
};

/// AI2 Reasoning Challenge, Challenge set (Clark et al., 2018), CC BY-SA 4.0. 1,172 test questions.
pub const ARC_CHALLENGE_TEST: DatasetFile = DatasetFile {
    repo: "allenai/ai2_arc",
    revision: "210d026faf9955653af8916fad021475a3f00453",
    path: "ARC-Challenge/test-00000-of-00001.parquet",
    sha256: "62f03257e737aed263f55c6abf87c7bb0028a44a6bdd2a26eb1279eb42c1d1e9",
};

/// ARC-Challenge train split, the source of few-shot examples.
pub const ARC_CHALLENGE_TRAIN: DatasetFile = DatasetFile {
    repo: "allenai/ai2_arc",
    revision: "210d026faf9955653af8916fad021475a3f00453",
    path: "ARC-Challenge/train-00000-of-00001.parquet",
    sha256: "e488c1587ffdcfc8443f916c53488a95cd471c5790e0746c6bfe4cecf20962cb",
};

/// GSM8K (Cobbe et al., 2021), MIT. 1,319 grade-school math word problems.
pub const GSM8K_TEST: DatasetFile = DatasetFile {
    repo: "openai/gsm8k",
    revision: "740312add88f781978c0658806c59bc2815b9866",
    path: "main/test-00000-of-00001.parquet",
    sha256: "ee7b8da9e381df27b9e3f7758a159ab2bdaa4dbaa910546cbbc47e0cb44e4f59",
};

/// IFEval (Zhou et al., 2023), Apache 2.0. 541 prompts with verifiable instructions.
pub const IFEVAL: DatasetFile = DatasetFile {
    repo: "google/IFEval",
    revision: "966cd89545d6b6acfd7638bc708b98261ca58e84",
    path: "ifeval_input_data.jsonl",
    sha256: "6a85310ca8ce15eff755aa08a3a4ff931c7e273e7515ebb3c492ea85fd8288f2",
};

/// HumanEval (Chen et al., 2021), MIT. 164 Python function-completion problems.
pub const HUMANEVAL: DatasetFile = DatasetFile {
    repo: "openai/openai_humaneval",
    revision: "7dce6050a7d6d172f3cc5c32aa97f52fa1a2e544",
    path: "openai_humaneval/test-00000-of-00001.parquet",
    sha256: "2f2871a15fbc95b6c683043359f4ed8e144c5a1c4f24f25f66bc51f598dfcfb6",
};

/// LiveCodeBench release v6 additions (Jain et al., 2024): 175 AtCoder and LeetCode
/// problems from January–April 2025, newer than most models' training data. 134 MB.
pub const LIVECODEBENCH_V6: DatasetFile = DatasetFile {
    repo: "livecodebench/code_generation_lite",
    revision: "0fe84c3912ea0c4d4a78037083943e8f0c4dd505",
    path: "test6.jsonl",
    sha256: "bb4c364f71921c4495a6ad15abe1a927350b720009f4933e2e71f8af0f6fd1f5",
};
