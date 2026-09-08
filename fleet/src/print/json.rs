//! `serde_json` pretty-print wrapper + error mapping, shared across `dispatch/*`.

pub fn print_pretty<T: serde::Serialize>(value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(text) => println!("{text}"),
        Err(e) => eprintln!("json encode failed: {e}"),
    }
}
