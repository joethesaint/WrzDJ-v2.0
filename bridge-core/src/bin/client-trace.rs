//! `cargo run --release --bin client-trace < traces.json`: one JSON line per trace.
use std::io::Read;

fn main() {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .expect("read stdin");
    let traces: Vec<wrzdj_bridge_core::client_trace::ClientTrace> =
        serde_json::from_str(&input).expect("traces JSON");
    for trace in &traces {
        println!("{}", wrzdj_bridge_core::client_trace::run(trace));
    }
}
