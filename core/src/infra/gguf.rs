//! Minimal GGUF header reader: extracts the scalar metadata we need (architecture,
//! layer count, quantization) without loading tensors. Arrays (e.g. the tokenizer
//! vocabulary) are skipped.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, Read, Seek};
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GgufMetadata {
    pub architecture: Option<String>,
    pub name: Option<String>,
    pub size_label: Option<String>,
    pub file_type: Option<u32>,
    pub block_count: Option<u32>,
}

impl GgufMetadata {
    /// Human-readable quantization name for `general.file_type` (llama_ftype).
    pub fn quantization(&self) -> Option<&'static str> {
        Some(match self.file_type? {
            0 => "F32",
            1 => "F16",
            2 => "Q4_0",
            3 => "Q4_1",
            7 => "Q8_0",
            8 => "Q5_0",
            9 => "Q5_1",
            10 => "Q2_K",
            11 => "Q3_K_S",
            12 => "Q3_K_M",
            13 => "Q3_K_L",
            14 => "Q4_K_S",
            15 => "Q4_K_M",
            16 => "Q5_K_S",
            17 => "Q5_K_M",
            18 => "Q6_K",
            19 => "IQ2_XXS",
            20 => "IQ2_XS",
            21 => "Q2_K_S",
            22 => "IQ3_XS",
            23 => "IQ3_XXS",
            24 => "IQ1_S",
            25 => "IQ4_NL",
            26 => "IQ3_S",
            27 => "IQ3_M",
            28 => "IQ2_S",
            29 => "IQ2_M",
            30 => "IQ4_XS",
            31 => "IQ1_M",
            32 => "BF16",
            36 => "TQ1_0",
            37 => "TQ2_0",
            38 => "MXFP4",
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
enum Value {
    Str(String),
    Int(i64),
    Other,
}

const MAX_STRING_LEN: u64 = 16 * 1024 * 1024;

pub fn read_metadata(path: &Path) -> io::Result<GgufMetadata> {
    let mut r = BufReader::new(File::open(path)?);

    let mut magic = [0u8; 4];
    r.read_exact(&mut magic)?;
    if &magic != b"GGUF" {
        return Err(invalid("not a GGUF file"));
    }
    let version = read_u32(&mut r)?;
    if version < 2 {
        return Err(invalid("unsupported GGUF version"));
    }
    let _tensor_count = read_u64(&mut r)?;
    let kv_count = read_u64(&mut r)?;

    let mut kv = HashMap::new();
    for _ in 0..kv_count {
        let key = read_string(&mut r)?;
        let ty = read_u32(&mut r)?;
        let value = read_value(&mut r, ty)?;
        if !matches!(value, Value::Other) {
            kv.insert(key, value);
        }
    }

    let string = |k: &str| match kv.get(k) {
        Some(Value::Str(s)) => Some(s.clone()),
        _ => None,
    };
    let int = |k: &str| match kv.get(k) {
        Some(Value::Int(i)) => u32::try_from(*i).ok(),
        _ => None,
    };

    let architecture = string("general.architecture");
    let block_count = architecture
        .as_deref()
        .and_then(|a| int(&format!("{a}.block_count")));
    Ok(GgufMetadata {
        name: string("general.name"),
        size_label: string("general.size_label"),
        file_type: int("general.file_type"),
        block_count,
        architecture,
    })
}

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

fn read_u32<R: Read>(r: &mut R) -> io::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn read_u64<R: Read>(r: &mut R) -> io::Result<u64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b)?;
    Ok(u64::from_le_bytes(b))
}

fn read_string<R: Read>(r: &mut R) -> io::Result<String> {
    let len = read_u64(r)?;
    if len > MAX_STRING_LEN {
        return Err(invalid("GGUF string too long"));
    }
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Byte size of fixed-width GGUF scalar types.
fn scalar_size(ty: u32) -> Option<i64> {
    match ty {
        0 | 1 | 7 => Some(1),
        2 | 3 => Some(2),
        4..=6 => Some(4),
        10..=12 => Some(8),
        _ => None,
    }
}

fn read_value<R: Read + Seek>(r: &mut R, ty: u32) -> io::Result<Value> {
    let mut read_int = |n: usize, signed: bool| -> io::Result<i64> {
        let mut b = [0u8; 8];
        r.read_exact(&mut b[..n])?;
        Ok(match (n, signed) {
            (1, false) => b[0] as i64,
            (1, true) => b[0] as i8 as i64,
            (2, false) => u16::from_le_bytes([b[0], b[1]]) as i64,
            (2, true) => i16::from_le_bytes([b[0], b[1]]) as i64,
            (4, false) => u32::from_le_bytes(b[..4].try_into().unwrap()) as i64,
            (4, true) => i32::from_le_bytes(b[..4].try_into().unwrap()) as i64,
            (_, false) => u64::from_le_bytes(b) as i64,
            (_, true) => i64::from_le_bytes(b),
        })
    };
    Ok(match ty {
        0 => Value::Int(read_int(1, false)?),
        1 => Value::Int(read_int(1, true)?),
        2 => Value::Int(read_int(2, false)?),
        3 => Value::Int(read_int(2, true)?),
        4 => Value::Int(read_int(4, false)?),
        5 => Value::Int(read_int(4, true)?),
        10 => Value::Int(read_int(8, false)?),
        11 => Value::Int(read_int(8, true)?),
        6 | 7 | 12 => {
            r.seek_relative(scalar_size(ty).unwrap())?;
            Value::Other
        }
        8 => Value::Str(read_string(r)?),
        9 => {
            let elem_ty = read_u32(r)?;
            let count = read_u64(r)?;
            if let Some(size) = scalar_size(elem_ty) {
                r.seek_relative(size * count as i64)?;
            } else {
                for _ in 0..count {
                    read_value(r, elem_ty)?;
                }
            }
            Value::Other
        }
        _ => return Err(invalid("unknown GGUF value type")),
    })
}

#[cfg(test)]
mod tests;
