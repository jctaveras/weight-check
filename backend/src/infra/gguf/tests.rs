use super::*;
use std::io::{Cursor, Write};

fn put_str(buf: &mut Vec<u8>, s: &str) {
    buf.extend((s.len() as u64).to_le_bytes());
    buf.extend(s.as_bytes());
}

/// A minimal valid GGUF header with architecture, block count, file type and name.
pub(crate) fn gguf_bytes(arch: &str, block_count: u32, file_type: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend(b"GGUF");
    buf.extend(3u32.to_le_bytes());
    buf.extend(0u64.to_le_bytes());
    buf.extend(4u64.to_le_bytes());
    put_str(&mut buf, "general.architecture");
    buf.extend(8u32.to_le_bytes());
    put_str(&mut buf, arch);
    put_str(&mut buf, &format!("{arch}.block_count"));
    buf.extend(4u32.to_le_bytes());
    buf.extend(block_count.to_le_bytes());
    put_str(&mut buf, "general.file_type");
    buf.extend(4u32.to_le_bytes());
    buf.extend(file_type.to_le_bytes());
    put_str(&mut buf, "general.size_label");
    buf.extend(8u32.to_le_bytes());
    put_str(&mut buf, "1B");
    buf
}

fn value(ty: u32, bytes: &[u8]) -> io::Result<Value> {
    let mut cursor = Cursor::new(bytes.to_vec());
    let v = read_value(&mut cursor, ty)?;
    assert_eq!(
        cursor.position() as usize,
        bytes.len(),
        "type {ty} consumed"
    );
    Ok(v)
}

fn int(ty: u32, bytes: &[u8]) -> i64 {
    match value(ty, bytes).unwrap() {
        Value::Int(i) => i,
        other => panic!("type {ty}: expected int, got {other:?}"),
    }
}

#[test]
fn reads_every_integer_width_and_sign() {
    assert_eq!(int(0, &[200]), 200);
    assert_eq!(int(1, &[0xff]), -1);
    assert_eq!(int(2, &60000u16.to_le_bytes()), 60000);
    assert_eq!(int(3, &(-2i16).to_le_bytes()), -2);
    assert_eq!(int(4, &4_000_000_000u32.to_le_bytes()), 4_000_000_000);
    assert_eq!(int(5, &(-3i32).to_le_bytes()), -3);
    assert_eq!(int(10, &7u64.to_le_bytes()), 7);
    assert_eq!(int(11, &(-4i64).to_le_bytes()), -4);
}

#[test]
fn skips_floats_bools_and_arrays_of_strings() {
    assert!(matches!(value(6, &1.5f32.to_le_bytes()), Ok(Value::Other)));
    assert!(matches!(value(7, &[1]), Ok(Value::Other)));
    assert!(matches!(value(12, &2.5f64.to_le_bytes()), Ok(Value::Other)));

    let mut arr = Vec::new();
    arr.extend(8u32.to_le_bytes());
    arr.extend(2u64.to_le_bytes());
    put_str(&mut arr, "a");
    put_str(&mut arr, "bc");
    assert!(matches!(value(9, &arr), Ok(Value::Other)));
}

#[test]
fn rejects_unknown_types_and_oversized_strings() {
    assert!(value(99, &[]).is_err());
    assert!(value(8, &(MAX_STRING_LEN + 1).to_le_bytes()).is_err());
}

#[test]
fn rejects_old_versions_and_truncated_files() {
    let mut old = Vec::new();
    old.extend(b"GGUF");
    old.extend(1u32.to_le_bytes());
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(&old).unwrap();
    assert!(read_metadata(f.path()).is_err());

    let full = gguf_bytes("llama", 16, 7);
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(&full[..full.len() - 3]).unwrap();
    assert!(read_metadata(f.path()).is_err());
}

#[test]
fn quantization_names() {
    let q = |file_type| GgufMetadata {
        file_type,
        ..Default::default()
    };
    assert_eq!(q(Some(0)).quantization(), Some("F32"));
    assert_eq!(q(Some(7)).quantization(), Some("Q8_0"));
    assert_eq!(q(Some(18)).quantization(), Some("Q6_K"));
    assert_eq!(q(Some(30)).quantization(), Some("IQ4_XS"));
    assert_eq!(q(Some(32)).quantization(), Some("BF16"));
    assert_eq!(q(Some(38)).quantization(), Some("MXFP4"));
    assert_eq!(q(Some(4)).quantization(), None);
    assert_eq!(q(None).quantization(), None);
}

#[test]
fn reads_scalar_metadata_and_skips_arrays() {
    let mut buf = Vec::new();
    buf.extend(b"GGUF");
    buf.extend(3u32.to_le_bytes());
    buf.extend(0u64.to_le_bytes()); // tensors
    buf.extend(5u64.to_le_bytes()); // kv count

    put_str(&mut buf, "general.architecture");
    buf.extend(8u32.to_le_bytes());
    put_str(&mut buf, "llama");

    put_str(&mut buf, "tokenizer.ggml.tokens");
    buf.extend(9u32.to_le_bytes());
    buf.extend(8u32.to_le_bytes());
    buf.extend(2u64.to_le_bytes());
    put_str(&mut buf, "<s>");
    put_str(&mut buf, "</s>");

    put_str(&mut buf, "tokenizer.ggml.scores");
    buf.extend(9u32.to_le_bytes());
    buf.extend(6u32.to_le_bytes());
    buf.extend(2u64.to_le_bytes());
    buf.extend(0.5f32.to_le_bytes());
    buf.extend(0.25f32.to_le_bytes());

    put_str(&mut buf, "llama.block_count");
    buf.extend(4u32.to_le_bytes());
    buf.extend(32u32.to_le_bytes());

    put_str(&mut buf, "general.file_type");
    buf.extend(4u32.to_le_bytes());
    buf.extend(15u32.to_le_bytes());

    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(&buf).unwrap();

    let meta = read_metadata(f.path()).unwrap();
    assert_eq!(meta.architecture.as_deref(), Some("llama"));
    assert_eq!(meta.block_count, Some(32));
    assert_eq!(meta.quantization(), Some("Q4_K_M"));
}

#[test]
fn rejects_non_gguf() {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(b"NOPE0000").unwrap();
    assert!(read_metadata(f.path()).is_err());
}
