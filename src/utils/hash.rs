pub const FNV_OFFSET_BASIS32: u32 = 0x811c9dc5;
pub const FNV_PRIME32: u32 = 0x01000193;

pub fn hash_string(s: &str) -> u32 {
    let mut hash = FNV_OFFSET_BASIS32;
    for byte in s.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(FNV_PRIME32);
    }
    hash
}
