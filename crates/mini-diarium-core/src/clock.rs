//! Clock-change protection.
//!
//! A journal remembers the latest time it has seen ("high-water mark"), MACed with a key
//! derived from the journal's master key so it can't be edited without the key. On each
//! unlock the system clock is compared with it:
//!
//! * clock **behind** the mark by more than [`TOLERANCE_SECS`] → the clock was set back,
//!   and the user is warned;
//! * a missing mark (first unlock after upgrading) is not a warning; it is just recorded;
//! * a mark whose MAC doesn't verify was edited or corrupted → warned as tampering.
//!
//! Time-based features must use [`ClockCheck::effective_now`], which never goes backwards,
//! so setting the clock back can't extend a deadline. A clock set *forward* can't be told
//! apart from a long absence and is not flagged; it can only make deadlines arrive sooner.
//!
//! Limits: someone who copies the database file can later restore an older copy (with an
//! older, validly MACed mark). Nothing stored only on this computer can prevent that.

use hkdf::Hkdf;
use sha2::Sha256;

/// Setting key holding `"<unix seconds>:<hex MAC>"`.
pub const HIGH_WATER_SETTING: &str = "clock_high_water";

/// How far the clock may be behind the mark before it counts as set back. Covers
/// ordinary clock corrections (NTP adjustments, a dead CMOS battery fixed by sync).
pub const TOLERANCE_SECS: i64 = 10 * 60;

/// Result of comparing the system clock with the stored high-water mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockCheck {
    /// `max(now, valid mark)`: the time that time-based features should use.
    pub effective_now: i64,
    /// Seconds the clock is behind the mark, when beyond the tolerance.
    pub set_back_by: Option<i64>,
    /// The stored mark is present but malformed or its MAC doesn't verify.
    pub tampered: bool,
}

impl ClockCheck {
    /// Whether the user should be warned.
    pub fn suspicious(&self) -> bool {
        self.set_back_by.is_some() || self.tampered
    }
}

fn mac(master_key: &[u8; 32], secs: i64) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(b"rat-journal:clock:v1"), master_key);
    let mut out = [0u8; 32];
    hk.expand(&secs.to_be_bytes(), &mut out)
        .expect("32-byte HKDF output always fits");
    out
}

fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Encodes a high-water mark for storage.
pub fn encode_mark(master_key: &[u8; 32], secs: i64) -> String {
    format!("{}:{}", secs, hex::encode(mac(master_key, secs)))
}

/// Decodes a stored mark. `None` means malformed or failing the MAC.
pub fn decode_mark(master_key: &[u8; 32], stored: &str) -> Option<i64> {
    let (secs, tag) = stored.split_once(':')?;
    let secs: i64 = secs.parse().ok()?;
    let tag = hex::decode(tag).ok()?;
    ct_eq(&tag, &mac(master_key, secs)).then_some(secs)
}

/// Compares `now` with the stored mark (if any). Returns the check and the mark to store
/// next, which is `effective_now` re-encoded (so a tampered mark is replaced by `now`).
pub fn check(master_key: &[u8; 32], stored: Option<&str>, now: i64) -> (ClockCheck, String) {
    let (mark, tampered) = match stored.map(|s| decode_mark(master_key, s)) {
        None => (None, false),
        Some(Some(m)) => (Some(m), false),
        Some(None) => (None, true),
    };
    let set_back_by = mark
        .map(|m| m - now)
        .filter(|behind| *behind > TOLERANCE_SECS);
    let effective_now = mark.map_or(now, |m| m.max(now));
    let result = ClockCheck {
        effective_now,
        set_back_by,
        tampered,
    };
    (result, encode_mark(master_key, effective_now))
}

/// Checks the clock against an open journal's mark and stores the new mark.
pub fn update_journal_mark(db: &crate::db::DatabaseConnection) -> Result<ClockCheck, String> {
    let stored = crate::db::get_db_setting(db, HIGH_WATER_SETTING);
    let (result, mark) = check(db.key().as_bytes(), stored.as_deref(), unix_now());
    crate::db::set_db_setting(db, HIGH_WATER_SETTING, &mark)?;
    Ok(result)
}

/// Current Unix time in seconds (0 if the clock is before 1970).
pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
