//! Minimal reader for PS3 `PARAM.SFO` metadata files.
//!
//! Layout (all integers little-endian):
//! ```text
//! 0x00  magic "\0PSF"
//! 0x04  version
//! 0x08  key table offset
//! 0x0C  data table offset
//! 0x10  entry count
//! 0x14  index table: 16 bytes per entry
//!         u16 key offset, u16 format, u32 used length, u32 max length, u32 data offset
//! ```

use std::fmt;

const MAGIC: &[u8; 4] = b"\0PSF";
const HEADER_LEN: usize = 0x14;
const INDEX_ENTRY_LEN: usize = 0x10;

const FMT_UTF8_SPECIAL: u16 = 0x0004;
const FMT_UTF8: u16 = 0x0204;
const FMT_INT32: u16 = 0x0404;

#[derive(Debug, Clone, PartialEq)]
pub enum SfoValue {
    Str(String),
    Int(u32),
    /// Raw bytes (format 0x0004, or anything unrecognised).
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone)]
pub struct Sfo {
    entries: Vec<(String, SfoValue)>,
}

#[derive(Debug)]
pub struct SfoError(String);

impl fmt::Display for SfoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid PARAM.SFO: {}", self.0)
    }
}

impl std::error::Error for SfoError {}

fn le_u16(b: &[u8], off: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(off..off + 2)?.try_into().ok()?))
}

fn le_u32(b: &[u8], off: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(off..off + 4)?.try_into().ok()?))
}

impl Sfo {
    pub fn parse(bytes: &[u8]) -> Result<Self, SfoError> {
        let err = |msg: &str| SfoError(msg.to_owned());

        if bytes.len() < HEADER_LEN || &bytes[..4] != MAGIC {
            return Err(err("bad magic"));
        }
        let key_table = le_u32(bytes, 0x08).ok_or_else(|| err("truncated header"))? as usize;
        let data_table = le_u32(bytes, 0x0C).ok_or_else(|| err("truncated header"))? as usize;
        let count = le_u32(bytes, 0x10).ok_or_else(|| err("truncated header"))? as usize;

        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            let idx = HEADER_LEN + i * INDEX_ENTRY_LEN;
            let truncated = || err("truncated index table");
            let key_off = le_u16(bytes, idx).ok_or_else(truncated)? as usize;
            let fmt = le_u16(bytes, idx + 2).ok_or_else(truncated)?;
            let len = le_u32(bytes, idx + 4).ok_or_else(truncated)? as usize;
            let data_off = le_u32(bytes, idx + 12).ok_or_else(truncated)? as usize;

            let key_start = key_table + key_off;
            let key_bytes = bytes
                .get(key_start..)
                .ok_or_else(|| err("key out of range"))?;
            let key_end = key_bytes
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(key_bytes.len());
            let key = String::from_utf8_lossy(&key_bytes[..key_end]).into_owned();

            let data_start = data_table + data_off;
            let data = bytes
                .get(data_start..data_start + len)
                .ok_or_else(|| err("value out of range"))?;

            let value = match fmt {
                FMT_UTF8 => {
                    let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
                    SfoValue::Str(String::from_utf8_lossy(&data[..end]).into_owned())
                }
                FMT_INT32 if len == 4 => SfoValue::Int(le_u32(data, 0).unwrap()),
                FMT_UTF8_SPECIAL | _ => SfoValue::Bytes(data.to_vec()),
            };
            entries.push((key, value));
        }

        Ok(Self { entries })
    }

    pub fn get(&self, key: &str) -> Option<&SfoValue> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            SfoValue::Str(s) => Some(s),
            _ => None,
        }
    }
}
