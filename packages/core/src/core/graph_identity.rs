use super::graph_builder::{GraphKind, PathSeg};

const FNV_OFFSET_BASIS_64: u64 = 1_469_598_103_934_665_603;
const FNV_PRIME_64: u64 = 1_099_511_628_211;

#[inline]
fn hash_byte64(seed: u64, byte: u8) -> u64 {
    (seed ^ u64::from(byte)).wrapping_mul(FNV_PRIME_64)
}

fn hash_bytes64(seed: u64, bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(seed, |hash, byte| hash_byte64(hash, *byte))
}

fn hash_u32(seed: u64, value: u32) -> u64 {
    let mut hash = seed;
    for byte in value.to_le_bytes() {
        hash = hash_byte64(hash, byte);
    }
    hash
}

fn hash_i32(seed: u64, value: i32) -> u64 {
    hash_u32(seed, value as u32)
}

pub fn stable_node_id(kind: GraphKind, path: &[PathSeg]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS_64;
    hash = hash_byte64(hash, kind as u8);
    for segment in path {
        match segment {
            PathSeg::Key(key) => {
                hash = hash_byte64(hash, 0);
                hash = hash_bytes64(hash, key.as_bytes());
            }
            PathSeg::Index(index) => {
                hash = hash_byte64(hash, 1);
                hash = hash_i32(hash, *index as i32);
            }
        }
    }
    hash
}

pub fn stable_node_id_text(stable_id: u64) -> String {
    stable_id.to_string()
}

pub fn canonical_path_key(path: &[PathSeg]) -> String {
    let mut out = String::new();
    for (index, segment) in path.iter().enumerate() {
        if index != 0 {
            out.push('|');
        }
        match segment {
            PathSeg::Key(key) => {
                out.push_str("k:");
                out.push_str(key);
            }
            PathSeg::Index(value) => {
                out.push_str("i:");
                out.push_str(&value.to_string());
            }
        }
    }
    out
}
