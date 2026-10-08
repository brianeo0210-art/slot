use std::path::Path;

use crate::platform::Platform;

pub const SAVE_SEEN_FILE: &str = "Config/save_seen.ini";

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SaveSeen {
    First,
    Same,
    Changed,
}

pub fn fingerprint(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{} {hash:016x}", bytes.len())
}

pub fn key(platform: Platform, stem: &str) -> String {
    format!("{}/{}", platform.dir_name(), crate::cart_shell::key(stem))
}

pub fn check(root: &Path, platform: Platform, stem: &str, bytes: &[u8]) -> SaveSeen {
    match crate::ini::value(root, SAVE_SEEN_FILE, &key(platform, stem)) {
        None => SaveSeen::First,
        Some(seen) if seen == fingerprint(bytes) => SaveSeen::Same,
        Some(_) => SaveSeen::Changed,
    }
}

pub fn record(root: &Path, platform: Platform, stem: &str, bytes: &[u8]) -> std::io::Result<()> {
    crate::ini::write(
        root,
        SAVE_SEEN_FILE,
        &key(platform, stem),
        &fingerprint(bytes),
    )
}
