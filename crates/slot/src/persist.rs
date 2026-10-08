use std::path::{Path, PathBuf};

use slot_store::save_seen::{self, SaveSeen};
use slot_store::{atomic_write, read_slot_state, write_slot_state, Core, Platform, StateRing};

pub trait Snapshot {
    fn state(&self) -> Option<Vec<u8>>;
    fn save_ram(&self) -> Option<Vec<u8>>;
    fn thumb(&self) -> Option<Vec<u8>>;
    fn load(&self, state: Vec<u8>);

    fn resume_trusted(&self) -> bool {
        true
    }

    fn save_ram_trusted(&self) -> bool {
        true
    }
}

pub fn flush(
    root: &Path,
    platform: Platform,
    core: Core,
    stem: &str,
    state: Option<&[u8]>,
    sav: Option<&[u8]>,
) -> std::io::Result<()> {
    if let Some(state) = state {
        StateRing::new(root, platform, core, stem).write_resume(state)?;
    }
    if let Some(sav) = sav {
        write_sav(root, platform, stem, sav)?;
    }
    Ok(())
}

pub fn eject(
    root: &Path,
    platform: Platform,
    core: Core,
    stem: &str,
    state: Option<&[u8]>,
    sav: Option<&[u8]>,
) -> std::io::Result<()> {
    flush(root, platform, core, stem, state, sav)?;
    let mut slot = read_slot_state(root);
    slot.cart = None;
    slot.cart_platform = None;
    write_slot_state(root, &slot)
}

pub fn write_sav(root: &Path, platform: Platform, stem: &str, sav: &[u8]) -> std::io::Result<bool> {
    let path = sav_path(root, platform, stem);
    if let Some(old) = read_sav(root, platform, stem) {
        if old == sav {
            note_save(root, platform, stem, sav);
            return Ok(false);
        }
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    atomic_write(&path, sav)?;
    note_save(root, platform, stem, sav);
    Ok(true)
}

fn note_save(root: &Path, platform: Platform, stem: &str, sav: &[u8]) {
    if save_seen::check(root, platform, stem, sav) == SaveSeen::Same {
        return;
    }
    if let Err(e) = save_seen::record(root, platform, stem, sav) {
        eprintln!("slot: save_seen: {stem}: {e}");
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Start {
    pub resume: Option<Vec<u8>>,
    pub edited_elsewhere: bool,
}

pub fn resume_for_start(
    root: &Path,
    platform: Platform,
    core: Core,
    stem: &str,
    sav: Option<&[u8]>,
    clean: bool,
    stamp: &str,
) -> Start {
    let edited_elsewhere = sav.is_some_and(|bytes| {
        let verdict = save_seen::check(root, platform, stem, bytes);
        note_save(root, platform, stem, bytes);
        verdict == SaveSeen::Changed
    });
    if edited_elsewhere {
        match StateRing::new(root, platform, core, stem).retire_resume(stamp) {
            Ok(Some(to)) => eprintln!(
                "slot: {stem}: the save was changed outside slot, resume kept as {}",
                to.display()
            ),
            Ok(None) => {}
            Err(e) => eprintln!("slot: {stem}: could not set the resume aside: {e}"),
        }
        return Start {
            resume: None,
            edited_elsewhere,
        };
    }
    let resume = if clean {
        None
    } else {
        read_resume(root, platform, core, stem)
    };
    Start {
        resume,
        edited_elsewhere,
    }
}

pub fn read_sav(root: &Path, platform: Platform, stem: &str) -> Option<Vec<u8>> {
    std::fs::read(sav_path(root, platform, stem))
        .or_else(|_| {
            std::fs::read(
                crate::root::saves_dir(root)
                    .join(platform.dir_name())
                    .join(format!("{stem}.srm")),
            )
        })
        .ok()
}

pub fn read_resume(root: &Path, platform: Platform, core: Core, stem: &str) -> Option<Vec<u8>> {
    StateRing::new(root, platform, core, stem)
        .read_resume()
        .ok()
        .flatten()
}

fn sav_path(root: &Path, platform: Platform, stem: &str) -> PathBuf {
    crate::root::saves_dir(root)
        .join(platform.dir_name())
        .join(format!("{stem}.sav"))
}
