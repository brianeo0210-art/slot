use slot_store::save_seen::{check, fingerprint, record, SaveSeen};
use slot_store::Platform;

#[test]
fn a_cart_never_recorded_is_first() {
    let d = tempfile::tempdir().unwrap();
    assert_eq!(
        check(d.path(), Platform::Gba, "Emerald", &[1, 2, 3]),
        SaveSeen::First
    );
}

#[test]
fn a_recorded_save_reads_back_as_same() {
    let d = tempfile::tempdir().unwrap();
    record(d.path(), Platform::Gba, "Emerald", &[1, 2, 3]).unwrap();
    assert_eq!(
        check(d.path(), Platform::Gba, "Emerald", &[1, 2, 3]),
        SaveSeen::Same
    );
}

#[test]
fn one_changed_byte_is_changed() {
    let d = tempfile::tempdir().unwrap();
    let mut sav = vec![0u8; 0x20000];
    record(d.path(), Platform::Gba, "Emerald", &sav).unwrap();
    sav[0x1fff0] = 1;
    assert_eq!(
        check(d.path(), Platform::Gba, "Emerald", &sav),
        SaveSeen::Changed
    );
}

#[test]
fn carts_and_platforms_are_kept_apart() {
    let d = tempfile::tempdir().unwrap();
    record(d.path(), Platform::Gba, "Tetris", &[1]).unwrap();
    record(d.path(), Platform::Gb, "Tetris", &[2]).unwrap();
    record(d.path(), Platform::Gba, "Emerald", &[3]).unwrap();
    assert_eq!(
        check(d.path(), Platform::Gba, "Tetris", &[1]),
        SaveSeen::Same
    );
    assert_eq!(
        check(d.path(), Platform::Gb, "Tetris", &[2]),
        SaveSeen::Same
    );
    assert_eq!(
        check(d.path(), Platform::Gba, "Emerald", &[3]),
        SaveSeen::Same
    );
    assert_eq!(
        check(d.path(), Platform::Gba, "Tetris", &[2]),
        SaveSeen::Changed
    );
}

#[test]
fn recording_again_replaces_the_old_fingerprint() {
    let d = tempfile::tempdir().unwrap();
    record(d.path(), Platform::Gba, "Emerald", &[1]).unwrap();
    record(d.path(), Platform::Gba, "Emerald", &[2]).unwrap();
    assert_eq!(
        check(d.path(), Platform::Gba, "Emerald", &[2]),
        SaveSeen::Same
    );
    let text =
        std::fs::read_to_string(d.path().join(slot_store::save_seen::SAVE_SEEN_FILE)).unwrap();
    assert_eq!(text.lines().count(), 1, "{text:?}");
}

#[test]
fn names_with_spaces_accents_and_brackets_work() {
    let d = tempfile::tempdir().unwrap();
    let stem = "Pokemon - Modern Emerald Version  (U) (patched)";
    record(d.path(), Platform::Gba, stem, &[9; 16]).unwrap();
    assert_eq!(
        check(d.path(), Platform::Gba, stem, &[9; 16]),
        SaveSeen::Same
    );
    let composed = "Pok\u{e9}mon";
    let decomposed = "Poke\u{301}mon";
    record(d.path(), Platform::Gba, composed, &[4]).unwrap();
    assert_eq!(
        check(d.path(), Platform::Gba, decomposed, &[4]),
        SaveSeen::Same
    );
}

#[test]
fn a_name_the_file_cannot_hold_is_an_error_not_a_panic() {
    let d = tempfile::tempdir().unwrap();
    assert!(record(d.path(), Platform::Gba, "A = B", &[1]).is_err());
    assert_eq!(
        check(d.path(), Platform::Gba, "A = B", &[1]),
        SaveSeen::First
    );
}

#[test]
fn the_fingerprint_depends_on_length_and_content() {
    assert_ne!(fingerprint(&[0; 4]), fingerprint(&[0; 5]));
    assert_ne!(fingerprint(&[1, 2]), fingerprint(&[2, 1]));
    assert_eq!(fingerprint(&[7; 100]), fingerprint(&[7; 100]));
}
