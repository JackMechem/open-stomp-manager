//! Decode a capture log into readable events: `cargo run -p stomp-proto --example decode -- <log> [--all]`.

use stomp_proto::capture::{Entry, parse};
use stomp_proto::{Category, DeviceState, Origin};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.iter().find(|a| !a.starts_with("--")).expect("usage: decode <log> [--all]");
    let all = args.iter().any(|a| a == "--all");
    let mut st = DeviceState::default();
    for e in parse(&std::fs::read_to_string(path).expect("read log")) {
        match e {
            Entry::Mark { text, .. } => println!("\n===== {text} ====="),
            Entry::Message { origin, bytes, .. } => {
                let ev = st.apply(&bytes, origin);
                if (ev.category == Category::Protocol || ev.redundant) && !all {
                    continue;
                }
                let who = if origin == Origin::Pedal { "PEDAL" } else { "HOST " };
                println!("{who} {:8} {:40} {}", ev.category.label(), ev.subject, ev.value);
            }
        }
    }
}
