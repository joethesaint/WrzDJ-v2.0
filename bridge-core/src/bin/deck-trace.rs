//! `cargo run --release --bin deck-trace < traces.json`: one JSON line per trace (see `trace`).
use std::io::Read;

fn main() {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .expect("read stdin");
    let traces: Vec<wrzdj_bridge_core::trace::Trace> =
        serde_json::from_str(&input).expect("traces JSON");
    for trace in &traces {
        println!("{}", wrzdj_bridge_core::trace::run(trace));
    }
}
