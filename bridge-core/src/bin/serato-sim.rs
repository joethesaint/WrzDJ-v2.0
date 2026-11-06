//! A fake Serato: writes a live session file the way Serato DJ does while a DJ plays.
//!
//!   serato-sim <sessions dir> [step ms, default 1000]
//!
//! Steps (one step = <step ms>): start a session; deck 1 plays A; deck 2 plays B; deck 1 plays A
//! again (no repeat should be sent); deck 1 plays C, written in two halves; a new session file
//! with D on deck 1; that file is deleted. Each action is printed with its time. Point the Node
//! bridge (HOME=<home>, WRZDJ_PLUGIN=serato) or the Rust one (WRZDJ_SERATO_PATH=<dir>) at it.
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use wrzdj_bridge_core::serato::write;

fn append(path: &Path, bytes: &[u8]) {
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap()
        .write_all(bytes)
        .unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(dir) = args.first().map(Path::new) else {
        eprintln!("usage: serato-sim <sessions dir> [step ms]");
        std::process::exit(2);
    };
    let step = Duration::from_millis(args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1000));
    std::fs::create_dir_all(dir).unwrap();
    let start = Instant::now();
    let at = |steps: u32| {
        let due = start + step * steps;
        if let Some(wait) = due.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
    };
    let say = |what: &str| {
        println!(
            "[serato-sim {:>6.1}s] {what}",
            start.elapsed().as_secs_f64()
        )
    };
    let one = dir.join("1.session");
    let two = dir.join("2.session");

    append(&one, &write::header());
    say("session 1 started");
    at(3);
    append(&one, &write::track("Water", "Tyla", "TYLA", 1));
    say("deck 1: Water (Tyla)");
    at(25);
    append(&one, &write::track("Calm Down", "Rema", "Rave & Roses", 2));
    say("deck 2: Calm Down (Rema)");
    at(50);
    append(&one, &write::track("Water", "Tyla", "TYLA", 1));
    say("deck 1: Water again (should not be sent twice)");
    at(55);
    let c = write::track("Ojuelegba", "Wizkid", "Ayo", 1);
    let half = c.len() / 2;
    append(&one, &c[..half]);
    say("deck 1: Ojuelegba, first half written");
    at(57);
    append(&one, &c[half..]);
    say("deck 1: Ojuelegba, rest written");
    at(85);
    let mut s2 = write::header();
    s2.extend(write::track("Essence", "Wizkid", "Made in Lagos", 1));
    append(&two, &s2);
    say("session 2 started, deck 1: Essence (Wizkid)");
    at(115);
    std::fs::remove_file(&two).unwrap();
    say("session 2 deleted");
    at(120);
    say("done");
}
