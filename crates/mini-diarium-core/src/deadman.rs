//! Dead man's switch (opt-in): erases a journal or vault when either of its rules fires.
//!
//! * **Not unlocked for** `days` (0 = rule off): it is opened more than that long after
//!   its last unlock.
//! * **Clock set back** by more than `back_secs` (0 = rule off): it is opened with the
//!   computer's clock that far behind the latest time it has seen ([`crate::clock`]).
//!
//! The authoritative record lives where it can't be edited without the key: MACed in a
//! journal's `db_settings`, inside a vault's encrypted index. Before unlocking, the app reads
//! the journal file's record without the MAC, or the vault header's plaintext copy, so it
//! can erase before anyone unlocks; editing those can't extend a deadline, because every
//! unlock checks the keyed record. While the first rule is on, a journal or vault opened
//! with the clock set back (beyond [`crate::clock::TOLERANCE_SECS`], and not far enough for
//! the second rule) refuses to open, so setting the clock back can't skip the deadline.
//!
//! A clock set *forward* can't be told apart from time really passing, so there is no rule
//! for it; it simply counts toward the first rule.
//!
//! Limits: it acts only when this app starts or opens the journal or vault; copies made
//! earlier (cloud version history or trash, backups elsewhere) are not reached; and a
//! clock set back to a time between the last unlock and the deadline can't be detected.

use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

pub const DAY_SECS: i64 = 86_400;
/// Days before the deadline from which the app warns.
pub const WARN_DAYS: i64 = 3;
/// Longest "not unlocked for" period (10 years).
pub const MAX_DAYS: u32 = 3650;
/// Smallest "clock set back" threshold: ordinary clock corrections stay below it.
pub const MIN_BACK_SECS: u64 = crate::clock::TOLERANCE_SECS as u64;
/// Largest "clock set back" threshold (1 year).
pub const MAX_BACK_SECS: u64 = 365 * 86_400;

/// Setting key holding the MACed record in a journal's `db_settings`:
/// `"<days>:<last_unlock>:<MAC>"` (rule 1 only), or `"2:<days>:<last_unlock>:<back_secs>:<MAC>"`.
pub const JOURNAL_SETTING: &str = "dead_man";

/// Shown when a journal or vault was erased by its dead man's switch.
pub const ERR_ERASED: &str = "This was erased by its dead man's switch";
/// Shown when an armed journal or vault is opened with the clock set back.
pub const ERR_CLOCK_SET_BACK: &str =
    "This computer's clock is earlier than the last time this was open. Correct the date and time, then try again";

/// The rules a user sets; 0 turns a rule off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Rules {
    pub days: u32,
    pub back_secs: u64,
}

impl Rules {
    pub fn is_off(&self) -> bool {
        self.days == 0 && self.back_secs == 0
    }
}

fn is_zero(v: &u64) -> bool {
    *v == 0
}

/// An armed switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeadMan {
    /// "Not unlocked for" period in days; 0 = rule off.
    pub days: u32,
    /// Unix seconds of the last unlock.
    pub last_unlock: i64,
    /// "Clock set back" threshold in seconds; 0 = rule off.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub back_secs: u64,
}

impl DeadMan {
    pub fn rules(&self) -> Rules {
        Rules {
            days: self.days,
            back_secs: self.back_secs,
        }
    }

    pub fn deadline(&self) -> i64 {
        self.last_unlock + i64::from(self.days) * DAY_SECS
    }

    /// The "not unlocked for" rule has fired.
    pub fn expired(&self, now: i64) -> bool {
        self.days > 0 && now > self.deadline()
    }

    /// The "clock set back" rule fires for a clock `behind` seconds behind the latest time
    /// seen.
    pub fn set_back_fires(&self, behind: i64) -> bool {
        self.back_secs > 0 && behind > 0 && behind as u64 > self.back_secs
    }

    /// Whether to erase before unlocking: `latest_seen` is the latest time known without
    /// the key (the clock mark read without its MAC, or the last unlock).
    pub fn fires_before_unlock(&self, now: i64, latest_seen: i64) -> bool {
        self.expired(now) || self.set_back_fires(latest_seen.max(self.last_unlock) - now)
    }

    /// Seconds left, when within the warning window and not yet expired.
    pub fn warning(&self, now: i64) -> Option<i64> {
        if self.days == 0 {
            return None;
        }
        let left = self.deadline() - now;
        (0..=WARN_DAYS * DAY_SECS).contains(&left).then_some(left)
    }
}

/// Validates a "not unlocked for" period from the UI: 0 disarms, otherwise 1..=MAX_DAYS.
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

/// Validates both rules from the UI. `None` = switch off.
pub fn validate_rules(rules: Rules) -> Result<Option<Rules>, String> {
    validate_days(rules.days)?;
    if rules.back_secs != 0 && !(MIN_BACK_SECS..=MAX_BACK_SECS).contains(&rules.back_secs) {
        return Err("The clock threshold must be between 10 minutes and 1 year".to_string());
    }
    Ok((!rules.is_off()).then_some(rules))
}

fn mac(master_key: &[u8; 32], dm: &DeadMan, version: u8) -> [u8; 32] {
    let (salt, info): (&[u8], Vec<u8>) = match version {
        1 => (
            b"rat-journal:deadman:v1",
            [&dm.days.to_be_bytes()[..], &dm.last_unlock.to_be_bytes()].concat(),
        ),
        _ => (
            b"rat-journal:deadman:v2",
            [
                &dm.days.to_be_bytes()[..],
                &dm.last_unlock.to_be_bytes(),
                &dm.back_secs.to_be_bytes(),
            ]
            .concat(),
        ),
    };
    let hk = Hkdf::<Sha256>::new(Some(salt), master_key);
    let mut out = [0u8; 32];
    hk.expand(&info, &mut out)
        .expect("32-byte HKDF output always fits");
    out
}

/// Encodes a journal's record for `db_settings`.
pub fn encode(master_key: &[u8; 32], dm: &DeadMan) -> String {
    if dm.back_secs == 0 {
        format!(
            "{}:{}:{}",
            dm.days,
            dm.last_unlock,
            hex::encode(mac(master_key, dm, 1))
        )
    } else {
        format!(
            "2:{}:{}:{}:{}",
            dm.days,
            dm.last_unlock,
            dm.back_secs,
            hex::encode(mac(master_key, dm, 2))
        )
    }
}

/// Parses a record without checking its MAC. Returns the switch, the format version and
/// the MAC text.
fn parse(stored: &str) -> Option<(DeadMan, u8, &str)> {
    let parts: Vec<&str> = stored.split(':').collect();
    match parts.as_slice() {
        [days, last, tag] => Some((
            DeadMan {
                days: days.parse().ok()?,
                last_unlock: last.parse().ok()?,
                back_secs: 0,
            },
            1,
            tag,
        )),
        ["2", days, last, back, tag] => Some((
            DeadMan {
                days: days.parse().ok()?,
                last_unlock: last.parse().ok()?,
                back_secs: back.parse().ok()?,
            },
            2,
            tag,
        )),
        _ => None,
    }
}

/// Decodes a journal's record. `None` means malformed or failing the MAC (tampered).
pub fn decode(master_key: &[u8; 32], stored: &str) -> Option<DeadMan> {
    let (dm, version, tag) = parse(stored)?;
    let tag = hex::decode(tag).ok()?;
    let expected = mac(master_key, &dm, version);
    let equal = tag.len() == expected.len()
        && tag
            .iter()
            .zip(&expected)
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0;
    equal.then_some(dm)
}

/// What an unlock must do about the switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Not armed.
    Off,
    /// Armed and in time; the record should be renewed to this.
    Renew(DeadMan),
    /// The clock is set back while the "not unlocked for" rule is on: refuse to open.
    ClockSetBack,
    /// A rule fired, or the keyed record was tampered with: erase.
    Erase,
}

/// Decides what an unlock does. `record` is `None` when not armed, `Some(None)` when the
/// keyed record is present but tampered.
pub fn judge(record: Option<Option<DeadMan>>, clock: &crate::clock::ClockCheck) -> Verdict {
    let dm = match record {
        None => return Verdict::Off,
        Some(None) => return Verdict::Erase,
        Some(Some(dm)) => dm,
    };
    if let Some(behind) = clock.set_back_by {
        if dm.set_back_fires(behind) {
            return Verdict::Erase;
        }
        if dm.days > 0 {
            return Verdict::ClockSetBack;
        }
    }
    if dm.expired(clock.effective_now) {
        return Verdict::Erase;
    }
    Verdict::Renew(DeadMan {
        last_unlock: clock.effective_now,
        ..dm
    })
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

/// Arms a journal's "not unlocked for" rule only (`Some(days)`), or disarms it (`None`).
pub fn set_journal_dead_man(
    db: &crate::db::DatabaseConnection,
    days: Option<u32>,
) -> Result<Option<DeadMan>, String> {
    set_journal_rules(
        db,
        Rules {
            days: days.unwrap_or(0),
            back_secs: 0,
        },
    )
}

/// Sets a journal's rules (both 0 disarms), counting from now.
pub fn set_journal_rules(
    db: &crate::db::DatabaseConnection,
    rules: Rules,
) -> Result<Option<DeadMan>, String> {
    if rules.is_off() {
        crate::db::delete_db_setting(db, JOURNAL_SETTING)?;
        return Ok(None);
    }
    let check = crate::clock::update_journal_mark(db)?;
    let dm = DeadMan {
        days: rules.days,
        last_unlock: check.effective_now,
        back_secs: rules.back_secs,
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

/// What the journal file at `db_path` says without the key: its switch and its clock mark.
/// For decisions made before unlocking (the startup sweep, warnings). MACs aren't checked:
/// an edit can at most hasten erasure (anyone who can edit the file can delete it), and
/// every unlock checks the MACs. Unlike config.json's copy, the file is current on every
/// computer that shares the journal.
pub fn peek_journal_full(db_path: &std::path::Path) -> Option<(DeadMan, Option<i64>)> {
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    let get = |key| crate::db::queries::db_settings::get_db_setting_conn(&conn, key);
    let (dm, _, _) = parse(&get(JOURNAL_SETTING)?)?;
    let mark = get(crate::clock::HIGH_WATER_SETTING)
        .and_then(|m| m.split_once(':').and_then(|(secs, _)| secs.parse().ok()));
    Some((dm, mark))
}

/// The switch recorded in the journal file at `db_path` (see [`peek_journal_full`]).
pub fn peek_journal(db_path: &std::path::Path) -> Option<DeadMan> {
    peek_journal_full(db_path).map(|(dm, _)| dm)
}

/// Security settings of the live journal that a backup restore must keep: restoring an old
/// snapshot must neither bring back an old deadline (which could erase the restored journal
/// at the next unlock) nor a panic password that was removed since.
#[derive(Debug, Clone, Default)]
pub struct KeptSettings {
    dead_man: Rules,
    duress_check: Option<String>,
}

/// Reads the settings to keep from the live journal, before a restore.
pub fn keep_settings(db: &crate::db::DatabaseConnection) -> KeptSettings {
    KeptSettings {
        dead_man: journal_dead_man(db)
            .map(|dm| dm.rules())
            .unwrap_or_default(),
        duress_check: crate::db::get_db_setting(db, crate::duress::SETTING),
    }
}

/// Writes the kept settings onto the restored journal, counting the deadline from now.
/// Returns the switch now in force.
pub fn reapply_settings(
    db: &crate::db::DatabaseConnection,
    kept: &KeptSettings,
) -> Result<Option<DeadMan>, String> {
    match &kept.duress_check {
        Some(check) => crate::db::set_db_setting(db, crate::duress::SETTING, check)?,
        None => crate::db::delete_db_setting(db, crate::duress::SETTING)?,
    }
    // A restored clock mark is older than the live one; record the present.
    crate::clock::update_journal_mark(db)?;
    set_journal_rules(db, kept.dead_man)
}

#[cfg(test)]
mod tests;
