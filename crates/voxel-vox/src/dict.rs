//! STRING / DICT helpers for MagicaVoxel extension chunks.

use crate::{Result, VoxError};
use std::collections::HashMap;

pub fn read_string(data: &[u8], offset: &mut usize) -> Result<String> {
    if *offset + 4 > data.len() {
        return Err(VoxError::BadChunk("STRING"));
    }
    let n = i32::from_le_bytes(data[*offset..*offset + 4].try_into().unwrap()) as usize;
    *offset += 4;
    if *offset + n > data.len() {
        return Err(VoxError::BadChunk("STRING"));
    }
    let s = String::from_utf8_lossy(&data[*offset..*offset + n]).into_owned();
    *offset += n;
    Ok(s)
}

pub fn write_string(out: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    out.extend_from_slice(&(bytes.len() as i32).to_le_bytes());
    out.extend_from_slice(bytes);
}

pub fn read_dict(data: &[u8], offset: &mut usize) -> Result<HashMap<String, String>> {
    if *offset + 4 > data.len() {
        return Err(VoxError::BadChunk("DICT"));
    }
    let n = i32::from_le_bytes(data[*offset..*offset + 4].try_into().unwrap());
    *offset += 4;
    let mut map = HashMap::new();
    for _ in 0..n {
        let k = read_string(data, offset)?;
        let v = read_string(data, offset)?;
        map.insert(k, v);
    }
    Ok(map)
}

pub fn write_dict(out: &mut Vec<u8>, pairs: &[(&str, &str)]) {
    out.extend_from_slice(&(pairs.len() as i32).to_le_bytes());
    for (k, v) in pairs {
        write_string(out, k);
        write_string(out, v);
    }
}

pub fn dict_get_i32(map: &HashMap<String, String>, key: &str) -> Option<i32> {
    map.get(key)?.parse().ok()
}

pub fn dict_get_ivec3(map: &HashMap<String, String>, key: &str) -> Option<(i32, i32, i32)> {
    let s = map.get(key)?;
    let parts: Vec<_> = s.split_whitespace().collect();
    if parts.len() != 3 {
        return None;
    }
    Some((parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?))
}
