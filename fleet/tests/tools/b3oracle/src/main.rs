fn main() {
    let p = std::env::args().nth(1).expect("usage: b3oracle <file>");
    let d = std::fs::read(&p).unwrap_or_else(|e| { eprintln!("read {p}: {e}"); std::process::exit(3) });
    println!("{}", blake3::hash(&d).to_hex());
}
