use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{channel, Sender};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncStatus {
    Off,
    Connecting,
    Running,
    NeedsWifi,
    CantConnect,
    NotInstalled,
}

impl SyncStatus {
    pub fn parse(word: &str) -> SyncStatus {
        match word.trim() {
            "starting" => SyncStatus::Connecting,
            "running" => SyncStatus::Running,
            "no-wifi" => SyncStatus::NeedsWifi,
            "no-binary" => SyncStatus::NotInstalled,
            "no-ip" | "error" => SyncStatus::CantConnect,
            _ => SyncStatus::Off,
        }
    }
}

pub struct SyncRadio {
    root: PathBuf,
    run: PathBuf,
    jobs: Option<Sender<bool>>,
    asked: bool,
}

impl SyncRadio {
    pub fn new(root: &Path) -> SyncRadio {
        let run = std::env::var_os("AGS_RUN").map_or_else(|| PathBuf::from("/run"), PathBuf::from);
        SyncRadio::with_run(root, &run)
    }

    pub fn with_run(root: &Path, run: &Path) -> SyncRadio {
        SyncRadio {
            root: root.to_path_buf(),
            run: run.to_path_buf(),
            jobs: None,
            asked: false,
        }
    }

    pub fn available(&self) -> bool {
        self.root.join("System/sync.sh").is_file()
    }

    pub fn set(&mut self, want: bool) {
        if want == self.asked || !self.available() {
            return;
        }
        self.asked = want;
        let root = self.root.clone();
        let tx = self.jobs.get_or_insert_with(|| {
            let (tx, rx) = channel::<bool>();
            std::thread::spawn(move || {
                for want in rx {
                    let _ = command(&root, if want { "start" } else { "stop" }).status();
                }
            });
            tx
        });
        let _ = tx.send(want);
    }

    pub fn stop_now(&mut self) {
        if !self.available() {
            return;
        }
        if self.asked || self.status() != SyncStatus::Off {
            let _ = command(&self.root, "stop").status();
        }
        self.asked = false;
    }

    pub fn status(&self) -> SyncStatus {
        std::fs::read_to_string(self.run.join("slot-sync.state"))
            .map_or(SyncStatus::Off, |w| SyncStatus::parse(&w))
    }
}

fn command(root: &Path, verb: &str) -> Command {
    let mut cmd = Command::new("/bin/sh");
    cmd.arg(root.join("System/sync.sh")).arg(verb);
    cmd.env("SLOT_ROOT", root);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scripts_words_map_to_statuses() {
        assert_eq!(SyncStatus::parse("running\n"), SyncStatus::Running);
        assert_eq!(SyncStatus::parse("starting"), SyncStatus::Connecting);
        assert_eq!(SyncStatus::parse("no-wifi"), SyncStatus::NeedsWifi);
        assert_eq!(SyncStatus::parse("no-binary"), SyncStatus::NotInstalled);
        assert_eq!(SyncStatus::parse("no-ip"), SyncStatus::CantConnect);
        assert_eq!(SyncStatus::parse("error"), SyncStatus::CantConnect);
        assert_eq!(SyncStatus::parse("off"), SyncStatus::Off);
        assert_eq!(SyncStatus::parse("garbage"), SyncStatus::Off);
    }

    #[test]
    fn a_card_without_the_script_never_runs_anything() {
        let d = tempfile::tempdir().unwrap();
        let mut radio = SyncRadio::with_run(d.path(), d.path());
        assert!(!radio.available());
        radio.set(true);
        radio.stop_now();
        assert_eq!(radio.status(), SyncStatus::Off);
    }

    #[test]
    fn stop_now_runs_the_stop_verb_and_waits_for_it() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("System")).unwrap();
        std::fs::write(
            d.path().join("System/sync.sh"),
            "#!/bin/sh\necho \"$1 $SLOT_ROOT\" >> \"$0.log\"\n",
        )
        .unwrap();
        let mut radio = SyncRadio::with_run(d.path(), d.path());
        radio.set(true);
        radio.stop_now();
        let log = std::fs::read_to_string(d.path().join("System/sync.sh.log")).unwrap();
        assert!(log.lines().any(|l| l.starts_with("stop ")), "{log}");
    }

    #[test]
    fn stop_now_with_nothing_asked_and_nothing_running_does_nothing() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("System")).unwrap();
        std::fs::write(
            d.path().join("System/sync.sh"),
            "#!/bin/sh\ntouch \"$0.ran\"\n",
        )
        .unwrap();
        let mut radio = SyncRadio::with_run(d.path(), d.path());
        radio.stop_now();
        assert!(!d.path().join("System/sync.sh.ran").exists());
    }
}
