//! Which member list a channel shows, named as the web client names it.

use super::*;

/// The id Discord gives a channel's member list, computed as the web client
/// does: `everyone` when the @everyone role has VIEW_CHANNEL guild-wide
/// (`everyone`) and no overwrite of the channel denies it, else a
/// MurmurHash3 of the overwrites that touch VIEW_CHANNEL.
pub fn list_id(everyone: u64, overwrites: &[Overwrite]) -> String {
    let denied = overwrites.iter().any(|o| o.deny & permissions::VIEW_CHANNEL != 0);
    if everyone & permissions::VIEW_CHANNEL != 0 && !denied {
        return "everyone".into();
    }
    let mut parts: Vec<String> = overwrites
        .iter()
        .filter_map(|o| {
            if o.allow & permissions::VIEW_CHANNEL != 0 {
                Some(format!("allow:{}", o.id))
            } else if o.deny & permissions::VIEW_CHANNEL != 0 {
                Some(format!("deny:{}", o.id))
            } else {
                None
            }
        })
        .collect();
    if parts.is_empty() {
        return "everyone".into();
    }
    parts.sort();
    murmur3(parts.join(",").as_bytes(), 0).to_string()
}

/// MurmurHash3, x86 32-bit variant.
pub(super) fn murmur3(data: &[u8], seed: u32) -> u32 {
    const C1: u32 = 0xcc9e_2d51;
    const C2: u32 = 0x1b87_3593;
    let mix = |k: u32| k.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
    let mut hash = seed;
    let (chunks, tail) = data.as_chunks::<4>();
    for chunk in chunks {
        hash ^= mix(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        hash = hash.rotate_left(13).wrapping_mul(5).wrapping_add(0xe654_6b64);
    }
    if !tail.is_empty() {
        let k = tail.iter().rev().fold(0u32, |k, byte| (k << 8) | *byte as u32);
        hash ^= mix(k);
    }
    hash ^= data.len() as u32;
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x85eb_ca6b);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(0xc2b2_ae35);
    hash ^ (hash >> 16)
}
