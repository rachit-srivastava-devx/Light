//! Loads the 50-row human-labelled `agent-ui-human.jsonl` benchmark. Network is used only to
//! re-fetch it if the cached copy at `/tmp/fleet-bench/agent-ui-human.jsonl` is missing.

use serde_json::Value;
use std::path::Path;

pub struct Row {
    pub id: String,
    pub preferred_ui: String,
    pub prompt: String,
}

const CACHE: &str = "/tmp/fleet-bench/agent-ui-human.jsonl";
const SRC: &str = "https://datasets-server.huggingface.co/rows?dataset=akashnaren%2Fagent-ui-human&config=default&split=train&offset=0&length=100";

pub fn load() -> Vec<Row> {
    if !Path::new(CACHE).exists() {
        refetch();
    }
    let text = std::fs::read_to_string(CACHE).expect("read cached benchmark jsonl");
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let v: Value = serde_json::from_str(line).expect("benchmark row is valid JSON");
            Row {
                id: v["id"].as_str().unwrap().to_owned(),
                preferred_ui: v["preferred_ui"].as_str().unwrap().to_owned(),
                prompt: v["prompt"].as_str().unwrap().to_owned(),
            }
        })
        .collect()
}

fn refetch() {
    std::fs::create_dir_all("/tmp/fleet-bench").expect("create /tmp/fleet-bench");
    let resp: Value = reqwest::blocking::get(SRC)
        .expect("fetch benchmark dataset")
        .json()
        .expect("dataset response is JSON");
    let rows = resp["rows"].as_array().expect("rows array");
    let lines: Vec<String> = rows
        .iter()
        .map(|r| r["row"].to_string())
        .collect();
    std::fs::write(CACHE, lines.join("\n")).expect("write cached benchmark jsonl");
}
