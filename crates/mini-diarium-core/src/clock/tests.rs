use super::*;

#[test]
fn encode_decode_round_trip() {
    let key = [1u8; 32];
    let now = unix_now();
    let encoded = encode_mark(&key, now);
    assert_eq!(decode_mark(&key, &encoded), Some(now));
}

#[test]
fn decode_rejects_wrong_key() {
    let key1 = [1u8; 32];
    let key2 = [2u8; 32];
    let now = unix_now();
    let encoded = encode_mark(&key1, now);
    assert_eq!(decode_mark(&key2, &encoded), None);
}

#[test]
fn decode_rejects_edited_seconds() {
    let key = [1u8; 32];
    let now = unix_now();
    let mut encoded = encode_mark(&key, now);
    encoded.replace_range(0..1, "9");
    assert_eq!(decode_mark(&key, &encoded), None);
}

#[test]
fn decode_rejects_malformed() {
    let key = [1u8; 32];
    assert_eq!(decode_mark(&key, ""), None);
    assert_eq!(decode_mark(&key, "no-colon"), None);
    assert_eq!(decode_mark(&key, "bad:hex"), None);
    assert_eq!(decode_mark(&key, "123:not-hex"), None);
}

#[test]
fn no_mark() {
    let key = [1u8; 32];
    let now = unix_now();
    let (check, mark) = check(&key, None, now);
    assert!(!check.suspicious());
    assert_eq!(check.effective_now, now);
    assert_eq!(decode_mark(&key, &mark), Some(now));
}

#[test]
fn behind_within_tolerance() {
    let key = [1u8; 32];
    let now = unix_now();
    let mark_time = now + TOLERANCE_SECS - 1;
    let encoded = encode_mark(&key, mark_time);
    let (check, _) = check(&key, Some(&encoded), now);
    assert!(!check.suspicious());
    assert_eq!(check.effective_now, mark_time);
}

#[test]
fn behind_beyond_tolerance() {
    let key = [1u8; 32];
    let now = unix_now();
    let mark_time = now + TOLERANCE_SECS + 1;
    let encoded = encode_mark(&key, mark_time);
    let (check, _) = check(&key, Some(&encoded), now);
    assert!(check.suspicious());
    assert_eq!(check.set_back_by, Some(mark_time - now));
    assert_eq!(check.effective_now, mark_time);
}

#[test]
fn ahead() {
    let key = [1u8; 32];
    let now = unix_now();
    let mark_time = now - 100;
    let encoded = encode_mark(&key, mark_time);
    let (check, _) = check(&key, Some(&encoded), now);
    assert!(!check.suspicious());
    assert_eq!(check.effective_now, now);
}

#[test]
fn tampered_mark() {
    let key = [1u8; 32];
    let now = unix_now();
    let encoded = encode_mark(&key, now);
    let mut edited = encoded;
    edited.pop();
    let (check, returned) = check(&key, Some(&edited), now);
    assert!(check.tampered);
    assert!(check.suspicious());
    assert_eq!(check.effective_now, now);
    assert_eq!(decode_mark(&key, &returned), Some(now));
}

#[test]
fn returned_mark_decodes_to_effective_now() {
    let key = [1u8; 32];
    let now = unix_now();
    let mark_time = now - TOLERANCE_SECS - 1;
    let encoded = encode_mark(&key, mark_time);
    let (check, returned) = check(&key, Some(&encoded), now);
    assert_eq!(decode_mark(&key, &returned), Some(check.effective_now));
}
