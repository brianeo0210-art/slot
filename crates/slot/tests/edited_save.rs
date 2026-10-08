mod common;

use std::path::Path;
use std::time::{Duration, Instant};

use common::tmp_root_with_carts;
use slot::app::Phase;
use slot::persist::{self, resume_for_start};
use slot::session::Session;
use slot_input::{Btn, RawEvent};
use slot_store::{Core, Platform, StateRing};

const STAMP: &str = "2026-10-08_20-00-00";

fn start(root: &Path, sav: Option<&[u8]>, clean: bool) -> persist::Start {
    resume_for_start(
        root,
        Platform::Gba,
        Core::Mgba,
        "Emerald",
        sav,
        clean,
        STAMP,
    )
}

fn refused(root: &Path) -> Vec<String> {
    let dir = root.join("States/GBA/mgba/Emerald");
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    rd.filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.starts_with("resume-refused-"))
        .collect()
}

#[test]
fn the_first_time_a_save_is_seen_it_is_not_an_edit() {
    let d = tmp_root_with_carts(&["Emerald"]);
    StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
        .write_resume(&[7u8; 64])
        .unwrap();
    let got = start(d.path(), Some(&[1u8; 32]), false);
    assert!(!got.edited_elsewhere);
    assert_eq!(got.resume, Some(vec![7u8; 64]));
    assert!(refused(d.path()).is_empty());
}

#[test]
fn an_unchanged_save_keeps_the_resume() {
    let d = tmp_root_with_carts(&["Emerald"]);
    StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
        .write_resume(&[7u8; 64])
        .unwrap();
    start(d.path(), Some(&[1u8; 32]), false);
    let again = start(d.path(), Some(&[1u8; 32]), false);
    assert!(!again.edited_elsewhere);
    assert_eq!(again.resume, Some(vec![7u8; 64]));
}

#[test]
fn a_save_changed_outside_sets_the_resume_aside_and_starts_fresh() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let ring = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    ring.write_resume(&[7u8; 64]).unwrap();
    start(d.path(), Some(&[1u8; 32]), false);

    let got = start(d.path(), Some(&[2u8; 32]), false);

    assert!(got.edited_elsewhere);
    assert_eq!(got.resume, None, "the old snapshot would undo the edit");
    assert_eq!(ring.read_resume().unwrap(), None);
    assert_eq!(
        refused(d.path()),
        vec![format!("resume-refused-{STAMP}.state")],
        "the snapshot was not kept as a backup"
    );
    let kept = std::fs::read(d.path().join(format!(
        "States/GBA/mgba/Emerald/resume-refused-{STAMP}.state"
    )))
    .unwrap();
    assert_eq!(kept, vec![7u8; 64]);
}

#[test]
fn an_edit_is_noticed_once_and_not_on_every_start() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let ring = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    ring.write_resume(&[7u8; 64]).unwrap();
    start(d.path(), Some(&[1u8; 32]), false);
    assert!(start(d.path(), Some(&[2u8; 32]), false).edited_elsewhere);

    ring.write_resume(&[8u8; 64]).unwrap();
    let after = start(d.path(), Some(&[2u8; 32]), false);
    assert!(!after.edited_elsewhere);
    assert_eq!(after.resume, Some(vec![8u8; 64]), "a new snapshot was lost");
}

#[test]
fn a_hold_start_still_sets_aside_a_snapshot_the_edit_made_stale() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let ring = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    ring.write_resume(&[7u8; 64]).unwrap();
    start(d.path(), Some(&[1u8; 32]), true);
    let got = start(d.path(), Some(&[2u8; 32]), true);
    assert!(got.edited_elsewhere);
    assert_eq!(got.resume, None);
    assert_eq!(ring.read_resume().unwrap(), None);
    assert_eq!(refused(d.path()).len(), 1);
}

#[test]
fn a_hold_start_without_an_edit_leaves_the_resume_on_disk() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let ring = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    ring.write_resume(&[7u8; 64]).unwrap();
    let got = start(d.path(), Some(&[1u8; 32]), true);
    assert_eq!(got.resume, None);
    assert_eq!(ring.read_resume().unwrap(), Some(vec![7u8; 64]));
}

#[test]
fn a_cart_with_no_save_is_never_an_edit() {
    let d = tmp_root_with_carts(&["Emerald"]);
    StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
        .write_resume(&[7u8; 64])
        .unwrap();
    let got = start(d.path(), None, false);
    assert!(!got.edited_elsewhere);
    assert_eq!(got.resume, Some(vec![7u8; 64]));
}

#[test]
fn what_slot_writes_itself_is_never_taken_for_an_edit() {
    let d = tmp_root_with_carts(&["Emerald"]);
    start(d.path(), Some(&[1u8; 32]), false);
    persist::flush(
        d.path(),
        Platform::Gba,
        Core::Mgba,
        "Emerald",
        Some(&[7u8; 64]),
        Some(&[5u8; 32]),
    )
    .unwrap();
    let sav = persist::read_sav(d.path(), Platform::Gba, "Emerald").unwrap();
    assert_eq!(sav, vec![5u8; 32]);
    let got = start(d.path(), Some(&sav), false);
    assert!(!got.edited_elsewhere, "slot flagged its own save");
    assert_eq!(got.resume, Some(vec![7u8; 64]));
}

#[test]
fn an_identical_save_still_counts_as_slots_own() {
    let d = tmp_root_with_carts(&["Emerald"]);
    persist::write_sav(d.path(), Platform::Gba, "Emerald", &[5u8; 32]).unwrap();
    assert!(!persist::write_sav(d.path(), Platform::Gba, "Emerald", &[5u8; 32]).unwrap());
    let got = start(d.path(), Some(&[5u8; 32]), false);
    assert!(!got.edited_elsewhere);
}

#[test]
fn an_srm_edited_outside_is_noticed_when_there_is_no_sav() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let srm = d.path().join("Saves/GBA/Emerald.srm");
    std::fs::write(&srm, [1u8; 32]).unwrap();
    let ring = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    ring.write_resume(&[7u8; 64]).unwrap();
    let sav = persist::read_sav(d.path(), Platform::Gba, "Emerald").unwrap();
    start(d.path(), Some(&sav), false);

    std::fs::write(&srm, [2u8; 32]).unwrap();
    let sav = persist::read_sav(d.path(), Platform::Gba, "Emerald").unwrap();
    let got = start(d.path(), Some(&sav), false);
    assert!(got.edited_elsewhere);
    assert_eq!(got.resume, None);
}

fn counter_after_start(root: &Path) -> u64 {
    common::clocked(root);
    let mut s = Session::boot(root.to_path_buf());
    s.feed([RawEvent::Down(Btn::A)], 16);
    s.feed([RawEvent::Up(Btn::A)], 32);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        s.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    }
    s.app_mut().tick_ms(60_000);
    s.app_mut().settle_saves();
    let state = persist::read_resume(root, Platform::Gba, Core::Mgba, "Emerald")
        .expect("nothing was flushed");
    u64::from_le_bytes(state.try_into().expect("the mock's state is 8 bytes"))
}

#[test]
fn a_tap_after_an_outside_edit_does_not_resume_the_old_snapshot() {
    let _core = common::core_lock();
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    std::fs::write(d.path().join("Saves/GBA/Emerald.srm"), [1u8; 32]).unwrap();
    persist::flush(
        d.path(),
        Platform::Gba,
        Core::Mgba,
        "Emerald",
        Some(&500_000u64.to_le_bytes()),
        None,
    )
    .unwrap();
    assert!(
        counter_after_start(d.path()) >= 500_000,
        "an untouched save should still resume"
    );

    std::fs::write(d.path().join("Saves/GBA/Emerald.srm"), [9u8; 32]).unwrap();
    persist::flush(
        d.path(),
        Platform::Gba,
        Core::Mgba,
        "Emerald",
        Some(&500_000u64.to_le_bytes()),
        None,
    )
    .unwrap();
    assert!(
        counter_after_start(d.path()) < 500_000,
        "a tap resumed a snapshot the outside edit made stale"
    );
    assert_eq!(refused(d.path()).len(), 1, "the old snapshot was not kept");
}
