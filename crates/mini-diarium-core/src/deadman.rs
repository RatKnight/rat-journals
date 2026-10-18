//! Dead man's switch (opt-in).
//!
//! A journal or vault armed with `days = N` is erased when it is opened more than N days
//! after it was last unlocked. The authoritative record lives where it can't be edited
//! without the key: MACed in a journal's `db_settings`, inside a vault's encrypted index.
//! A plaintext copy (config.json for a journal, the vault header) lets the app erase an
//! expired journal or vault before anyone unlocks it; editing that copy can't extend the
//! deadline, because unlocking checks the keyed record.
//!
//! Time comes from [`crate::clock`]: an armed journal or vault refuses to open while the
//! clock is behind its high-water mark, so setting the clock back can't skip a deadline.
//!
//! Limits: it acts only when this app opens or lists the journal or vault; copies made
//! earlier (cloud version history or trash, backups elsewhere) are not reached; and a
//! clock set back to a time between the last unlock and the deadline can't be detected.

use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

pub const DAY_SECS: i64 = 86_400;
/// Days before the deadline from which the app warns.
pub const WARN_DAYS: i64 = 3;
/// Longest allowed period.
pub const MAX_DAYS: u32 = 3650;

/// Setting key holding `"<days>:<last_unlock>:<hex MAC>"` in a journal's `db_settings`.
pub const JOURNAL_SETTING: &str = "dead_man";

/// Shown when an expired journal or vault was erased.
pub const ERR_ERASED: &str =
    "This was erased because it was not unlocked within the time set for its dead man's switch";
/// Shown when an armed journal or vault is opened with the clock set back.
pub const ERR_CLOCK_SET_BACK: &str =
    "This computer's clock is earlier than the last time this was open. Correct the date and time, then try again";

/// An armed switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeadMan {
    pub days: u32,
    /// Unix seconds of the last unlock.
    pub last_unlock: i64,
}

impl DeadMan {
    pub fn deadline(&self) -> i64 {
        self.last_unlock + i64::from(self.days) * DAY_SECS
    }

    pub fn expired(&self, now: i64) -> bool {
        now > self.deadline()
    }

    /// Seconds left, when within the warning window and not yet expired.
    pub fn warning(&self, now: i64) -> Option<i64> {
        let left = self.deadline() - now;
        (0..=WARN_DAYS * DAY_SECS).contains(&left).then_some(left)
    }
}

/// Validates a period from the UI: 0 disarms, otherwise 1..=MAX_DAYS.
pub fn validate_days(days: u32) -> Result<Option<u32>, String> {
    match days {
        0 => Ok(None),
        1..=MAX_DAYS => Ok(Some(days)),
        _ => Err(format!(
            "The period must be between 1 and {} days",
            MAX_DAYS
        )),
    }
}

fn mac(master_key: &[u8; 32], days: u32, last_unlock: i64) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(b"rat-journal:deadman:v1"), master_key);
    let mut info = [0u8; 12];
    info[..4].copy_from_slice(&days.to_be_bytes());
    info[4..].copy_from_slice(&last_unlock.to_be_bytes());
    let mut out = [0u8; 32];
    hk.expand(&info, &mut out)
        .expect("32-byte HKDF output always fits");
    out
}

/// Encodes a journal's record for `db_settings`.
pub fn encode(master_key: &[u8; 32], dm: &DeadMan) -> String {
    format!(
        "{}:{}:{}",
        dm.days,
        dm.last_unlock,
        hex::encode(mac(master_key, dm.days, dm.last_unlock))
    )
}

/// Decodes a journal's record. `None` means malformed or failing the MAC (tampered).
pub fn decode(master_key: &[u8; 32], stored: &str) -> Option<DeadMan> {
    let mut parts = stored.splitn(3, ':');
    let days: u32 = parts.next()?.parse().ok()?;
    let last_unlock: i64 = parts.next()?.parse().ok()?;
    let tag = hex::decode(parts.next()?).ok()?;
    let expected = mac(master_key, days, last_unlock);
    let equal = tag.len() == expected.len()
        && tag
            .iter()
            .zip(&expected)
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0;
    equal.then_some(DeadMan { days, last_unlock })
}

/// What an unlock must do about the switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Not armed.
    Off,
    /// Armed and in time; the record should be renewed to this.
    Renew(DeadMan),
    /// Armed and the clock is behind the high-water mark: refuse to open.
    ClockSetBack,
    /// Expired, or the keyed record was tampered with: erase.
    Erase,
}

/// Decides what an unlock does. `record` is `None` when not armed, `Some(None)` when the
/// keyed record is present but tampered.
pub fn judge(record: Option<Option<DeadMan>>, clock: &crate::clock::ClockCheck) -> Verdict {
    match record {
        None => Verdict::Off,
        Some(None) => Verdict::Erase,
        Some(Some(_)) if clock.set_back_by.is_some() => Verdict::ClockSetBack,
        Some(Some(dm)) if dm.expired(clock.effective_now) => Verdict::Erase,
        Some(Some(dm)) => Verdict::Renew(DeadMan {
            days: dm.days,
            last_unlock: clock.effective_now,
        }),
    }
}

/// Reads a journal's switch: `None` = not armed, `Some(None)` = tampered.
fn read_journal(db: &crate::db::DatabaseConnection) -> Option<Option<DeadMan>> {
    crate::db::get_db_setting(db, JOURNAL_SETTING).map(|s| decode(db.key().as_bytes(), &s))
}

/// The journal's armed switch, if any (a tampered record reads as not armed here; the
/// next unlock erases the journal).
pub fn journal_dead_man(db: &crate::db::DatabaseConnection) -> Option<DeadMan> {
    read_journal(db).flatten()
}

/// Arms (`Some(days)`) or disarms (`None`) a journal's switch, counting from now.
pub fn set_journal_dead_man(
    db: &crate::db::DatabaseConnection,
    days: Option<u32>,
) -> Result<Option<DeadMan>, String> {
    let Some(days) = days else {
        crate::db::delete_db_setting(db, JOURNAL_SETTING)?;
        return Ok(None);
    };
    let check = crate::clock::update_journal_mark(db)?;
    let dm = DeadMan {
        days,
        last_unlock: check.effective_now,
    };
    crate::db::set_db_setting(db, JOURNAL_SETTING, &encode(db.key().as_bytes(), &dm))?;
    Ok(Some(dm))
}

/// Runs right after a journal unlocks: checks the clock and the switch, and renews both.
/// `Err(ERR_ERASED)` means the caller must close and erase the journal now;
/// `Err(ERR_CLOCK_SET_BACK)` means refuse to open. Returns the renewed switch, if armed.
pub fn journal_after_unlock(
    db: &crate::db::DatabaseConnection,
) -> Result<(crate::clock::ClockCheck, Option<DeadMan>), String> {
    journal_after_unlock_at(db, crate::clock::unix_now())
}

/// [`journal_after_unlock`] at a given time (for tests).
pub fn journal_after_unlock_at(
    db: &crate::db::DatabaseConnection,
    now: i64,
) -> Result<(crate::clock::ClockCheck, Option<DeadMan>), String> {
    use crate::clock::{self, HIGH_WATER_SETTING};
    let key = db.key().as_bytes();
    let stored_mark = crate::db::get_db_setting(db, HIGH_WATER_SETTING);
    let (check, mark) = clock::check(key, stored_mark.as_deref(), now);
    let renewed = match judge(read_journal(db), &check) {
        Verdict::Off => None,
        Verdict::ClockSetBack => return Err(ERR_CLOCK_SET_BACK.to_string()),
        Verdict::Erase => return Err(ERR_ERASED.to_string()),
        Verdict::Renew(dm) => {
            crate::db::set_db_setting(db, JOURNAL_SETTING, &encode(key, &dm))?;
            Some(dm)
        }
    };
    crate::db::set_db_setting(db, HIGH_WATER_SETTING, &mark)?;
    Ok((check, renewed))
}

#[cfg(test)]
mod tests;
