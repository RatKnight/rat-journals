use super::*;
use crate::clock::ClockCheck;

#[test]
fn test_deadline_expired() {
    let dm = DeadMan {
        back_secs: 0,
        days: 1,
        last_unlock: 1_000_000,
    };
    assert_eq!(dm.deadline(), 1_000_000 + DAY_SECS);
    assert!(!dm.expired(1_000_000));
    assert!(dm.expired(1_000_000 + DAY_SECS + 1));
}

#[test]
fn test_warning() {
    let dm = DeadMan {
        back_secs: 0,
        days: 5,
        last_unlock: 1_000_000,
    };
    assert_eq!(dm.warning(1_000_000 + 4 * DAY_SECS), Some(DAY_SECS));
    assert_eq!(dm.warning(1_000_000 + 3 * DAY_SECS), Some(2 * DAY_SECS));
    assert_eq!(dm.warning(1_000_000 + 5 * DAY_SECS + 1), None);
    assert_eq!(dm.warning(1_000_000 - 1), None);
}

#[test]
fn test_validate_days() {
    assert_eq!(validate_days(0), Ok(None));
    assert_eq!(validate_days(1), Ok(Some(1)));
    assert_eq!(validate_days(MAX_DAYS), Ok(Some(MAX_DAYS)));
    assert_eq!(
        validate_days(MAX_DAYS + 1),
        Err("The period must be between 1 and 3650 days".to_string())
    );
}

#[test]
fn test_encode_decode_round_trip() {
    let key = [7u8; 32];
    let dm = DeadMan {
        back_secs: 0,
        days: 10,
        last_unlock: 1_000_000,
    };
    let encoded = encode(&key, &dm);
    assert_eq!(decode(&key, &encoded), Some(dm));
}

#[test]
fn test_decode_wrong_key() {
    let key1 = [7u8; 32];
    let key2 = [8u8; 32];
    let dm = DeadMan {
        back_secs: 0,
        days: 10,
        last_unlock: 1_000_000,
    };
    let encoded = encode(&key1, &dm);
    assert_eq!(decode(&key2, &encoded), None);
}

#[test]
fn test_decode_tampered() {
    let key = [7u8; 32];
    let dm = DeadMan {
        back_secs: 0,
        days: 10,
        last_unlock: 1_000_000,
    };
    let mut encoded = encode(&key, &dm);
    encoded.pop();
    assert_eq!(decode(&key, &encoded), None);
}

#[test]
fn test_judge_off() {
    let clock = ClockCheck {
        effective_now: 1_000_000,
        set_back_by: None,
        tampered: false,
    };
    assert_eq!(judge(None, &clock), Verdict::Off);
}

#[test]
fn test_judge_erase_tampered() {
    let clock = ClockCheck {
        effective_now: 1_000_000,
        set_back_by: None,
        tampered: false,
    };
    assert_eq!(judge(Some(None), &clock), Verdict::Erase);
}

#[test]
fn test_judge_clock_set_back() {
    let clock = ClockCheck {
        effective_now: 1_000_000,
        set_back_by: Some(100),
        tampered: false,
    };
    let dm = DeadMan {
        back_secs: 0,
        days: 10,
        last_unlock: 1_000_000,
    };
    assert_eq!(judge(Some(Some(dm)), &clock), Verdict::ClockSetBack);
}

#[test]
fn test_judge_erase_expired() {
    let clock = ClockCheck {
        effective_now: 1_000_000 + DAY_SECS + 1,
        set_back_by: None,
        tampered: false,
    };
    let dm = DeadMan {
        back_secs: 0,
        days: 1,
        last_unlock: 1_000_000,
    };
    assert_eq!(judge(Some(Some(dm)), &clock), Verdict::Erase);
}

#[test]
fn test_judge_renew() {
    let clock = ClockCheck {
        effective_now: 1_000_000,
        set_back_by: None,
        tampered: false,
    };
    let dm = DeadMan {
        back_secs: 0,
        days: 10,
        last_unlock: 1_000_000,
    };
    assert_eq!(
        judge(Some(Some(dm)), &clock),
        Verdict::Renew(DeadMan {
            back_secs: 0,
            days: 10,
            last_unlock: 1_000_000
        })
    );
}

#[test]
fn journal_unarmed_is_off() {
    let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
    let db = crate::db::create_database(tmp.path(), "pw".to_string()).unwrap();

    let result = journal_after_unlock_at(&db, 1_000_000).unwrap();
    assert!(result.1.is_none());
    assert!(journal_dead_man(&db).is_none());
}

#[test]
fn journal_arm_and_renew() {
    let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
    let db = crate::db::create_database(tmp.path(), "pw".to_string()).unwrap();

    let dm = set_journal_dead_man(&db, Some(3)).unwrap().unwrap();
    assert_eq!(dm.days, 3);
    let t = dm.last_unlock;

    let result = journal_after_unlock_at(&db, t + DAY_SECS).unwrap();
    assert!(result.1.is_some());
    assert_eq!(result.1.unwrap().days, 3);
    assert_eq!(journal_dead_man(&db).unwrap().last_unlock, t + DAY_SECS);
}

#[test]
fn journal_expired_says_erase() {
    let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
    let db = crate::db::create_database(tmp.path(), "pw".to_string()).unwrap();

    let dm = set_journal_dead_man(&db, Some(1)).unwrap().unwrap();
    let t = dm.last_unlock;

    let result = journal_after_unlock_at(&db, t + 2 * DAY_SECS);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ERR_ERASED.to_string());
}

#[test]
fn journal_clock_set_back_refuses() {
    let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
    let db = crate::db::create_database(tmp.path(), "pw".to_string()).unwrap();

    let dm = set_journal_dead_man(&db, Some(5)).unwrap().unwrap();
    let t = dm.last_unlock;

    journal_after_unlock_at(&db, t + 100).unwrap(); // Should be OK

    let result = journal_after_unlock_at(&db, t - 3600);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ERR_CLOCK_SET_BACK.to_string());
}

#[test]
fn journal_tampered_record_says_erase() {
    let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
    let db = crate::db::create_database(tmp.path(), "pw".to_string()).unwrap();

    set_journal_dead_man(&db, Some(5)).unwrap();

    crate::db::set_db_setting(&db, JOURNAL_SETTING, "5:0:00").unwrap();

    let result = journal_after_unlock_at(&db, crate::clock::unix_now());
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ERR_ERASED.to_string());
}

#[test]
fn journal_disarm() {
    let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
    let db = crate::db::create_database(tmp.path(), "pw".to_string()).unwrap();

    set_journal_dead_man(&db, Some(3)).unwrap();
    assert!(journal_dead_man(&db).is_some());

    let result = set_journal_dead_man(&db, None).unwrap();
    assert!(result.is_none());
    assert!(journal_dead_man(&db).is_none());
}

#[test]
fn restore_keeps_the_live_switch_and_panic_password() {
    let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
    let db = crate::db::create_database(tmp.path(), "pw".to_string()).unwrap();
    set_journal_dead_man(&db, Some(5)).unwrap();
    crate::duress::set(&db, "panic").unwrap();
    let kept = keep_settings(&db);

    // What an old snapshot brings back: an expired deadline and no panic password.
    let old = DeadMan {
        back_secs: 0,
        days: 5,
        last_unlock: 0,
    };
    crate::db::set_db_setting(&db, JOURNAL_SETTING, &encode(db.key().as_bytes(), &old)).unwrap();
    crate::duress::clear(&db).unwrap();

    let renewed = reapply_settings(&db, &kept).unwrap().unwrap();
    assert_eq!(renewed.days, 5);
    assert!(renewed.last_unlock > 0);
    assert!(journal_after_unlock(&db).is_ok());
    assert!(crate::duress::is_set(&db));
}

#[test]
fn restore_does_not_rearm_a_switch_turned_off_since() {
    let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
    let db = crate::db::create_database(tmp.path(), "pw".to_string()).unwrap();
    let kept = keep_settings(&db);
    set_journal_dead_man(&db, Some(1)).unwrap(); // as in the old snapshot
    assert_eq!(reapply_settings(&db, &kept).unwrap(), None);
    assert!(journal_dead_man(&db).is_none());
}

#[test]
fn peek_reads_the_record_without_the_key() {
    let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
    let db = crate::db::create_database(tmp.path(), "pw".to_string()).unwrap();
    assert_eq!(peek_journal(tmp.path()), None);
    let dm = set_journal_dead_man(&db, Some(9)).unwrap().unwrap();
    drop(db);
    assert_eq!(peek_journal(tmp.path()), Some(dm));
}

fn clock(effective_now: i64, set_back_by: Option<i64>) -> ClockCheck {
    ClockCheck {
        effective_now,
        set_back_by,
        tampered: false,
    }
}

fn dm(days: u32, back_secs: u64) -> DeadMan {
    DeadMan {
        days,
        last_unlock: 1_000_000,
        back_secs,
    }
}

#[test]
fn clock_rule_erases_only_beyond_its_threshold() {
    let both = dm(30, 3600);
    // Set back 2 hours, threshold 1 hour: erase.
    assert_eq!(
        judge(Some(Some(both)), &clock(1_000_100, Some(7200))),
        Verdict::Erase
    );
    // Set back 30 minutes: under the threshold; the "not unlocked" rule then refuses.
    assert_eq!(
        judge(Some(Some(both)), &clock(1_000_100, Some(1800))),
        Verdict::ClockSetBack
    );
    // Clock rule alone, under its threshold: opens and renews, keeping the rule.
    let clock_only = dm(0, 3600);
    assert_eq!(
        judge(Some(Some(clock_only)), &clock(1_000_100, Some(1800))),
        Verdict::Renew(DeadMan {
            last_unlock: 1_000_100,
            ..clock_only
        })
    );
    // Without the clock rule a set-back never erases.
    assert_eq!(
        judge(Some(Some(dm(30, 0))), &clock(1_000_100, Some(99_999))),
        Verdict::ClockSetBack
    );
}

#[test]
fn not_unlocked_rule_off_never_expires() {
    let clock_only = dm(0, 3600);
    assert!(!clock_only.expired(i64::MAX / 2));
    assert_eq!(clock_only.warning(1_000_000), None);
    assert!(matches!(
        judge(Some(Some(clock_only)), &clock(9_000_000_000, None)),
        Verdict::Renew(_)
    ));
}

#[test]
fn record_with_clock_rule_round_trips_and_resists_edits() {
    let key = [7u8; 32];
    let both = dm(30, 3600);
    let enc = encode(&key, &both);
    assert!(enc.starts_with("2:"));
    assert_eq!(decode(&key, &enc), Some(both));
    let edited = enc.replacen(":3600:", ":999999:", 1);
    assert_eq!(decode(&key, &edited), None);
    assert_eq!(decode(&[8u8; 32], &enc), None);
    // Records written before the clock rule still read.
    let old = dm(30, 0);
    let enc_old = encode(&key, &old);
    assert!(!enc_old.starts_with("2:"));
    assert_eq!(decode(&key, &enc_old), Some(old));
}

#[test]
fn validate_rules_bounds() {
    assert_eq!(validate_rules(Rules::default()), Ok(None));
    assert!(validate_rules(Rules {
        days: 0,
        back_secs: MIN_BACK_SECS
    })
    .unwrap()
    .is_some());
    assert!(validate_rules(Rules {
        days: 0,
        back_secs: MIN_BACK_SECS - 1
    })
    .is_err());
    assert!(validate_rules(Rules {
        days: 0,
        back_secs: MAX_BACK_SECS + 1
    })
    .is_err());
    assert!(validate_rules(Rules {
        days: MAX_DAYS + 1,
        back_secs: 0
    })
    .is_err());
}

#[test]
fn fires_before_unlock_uses_the_latest_known_time() {
    let clock_only = dm(0, 3600);
    // The file's clock mark says 2 hours later than now.
    assert!(clock_only.fires_before_unlock(1_000_000, 1_007_200));
    assert!(!clock_only.fires_before_unlock(1_000_000, 1_001_800));
    assert!(!clock_only.fires_before_unlock(1_000_000, 0));
}

#[test]
fn journal_clock_rule_erases_when_the_clock_goes_back_too_far() {
    let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
    let db = crate::db::create_database(tmp.path(), "pw".to_string()).unwrap();
    let armed = set_journal_rules(
        &db,
        Rules {
            days: 0,
            back_secs: 3600,
        },
    )
    .unwrap()
    .unwrap();
    let t = armed.last_unlock;
    assert!(journal_after_unlock_at(&db, t - 1800).is_ok());
    assert_eq!(
        journal_after_unlock_at(&db, t - 7200).err(),
        Some(ERR_ERASED.to_string())
    );
    // The record keeps both rules through a renewal and a restore.
    let kept = keep_settings(&db);
    assert_eq!(
        reapply_settings(&db, &kept).unwrap().unwrap().back_secs,
        3600
    );
}
