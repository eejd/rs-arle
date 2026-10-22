//! `arle serve --served-model-name` end to end on the CPU backend (no GPU, no
//! weights, a minimal tokenizer): the process that `arle serve` starts lists the
//! name on `GET /v1/models`.
#![cfg(all(
    feature = "cli",
    feature = "cpu",
    not(feature = "metal"),
    not(feature = "cuda")
))]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const TOKENIZER: &str = r#"{"version":"1.0","truncation":null,"padding":null,"added_tokens":[],
    "normalizer":null,"pre_tokenizer":{"type":"Whitespace"},"post_processor":null,"decoder":null,
    "model":{"type":"WordLevel","vocab":{"[UNK]":0,"hello":1},"unk_token":"[UNK]"}}"#;

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port()
}

fn get_models(port: u16) -> Option<serde_json::Value> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream
        .write_all(b"GET /v1/models HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    serde_json::from_str(response.split_once("\r\n\r\n")?.1.trim()).ok()
}

/// The ids `arle serve` lists for `model_dir`, started with `extra` arguments.
fn listed_ids(model_dir: &std::path::Path, extra: &[&str]) -> Vec<String> {
    let port = free_port();
    let child = Command::new(env!("CARGO_BIN_EXE_arle"))
        .args(["serve", "--backend", "cpu", "--bind", "127.0.0.1", "--port"])
        .arg(port.to_string())
        .arg("--model-path")
        .arg(model_dir)
        .args(extra)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn arle serve");
    let _server = Server(child);
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(models) = get_models(port) {
            return models["data"]
                .as_array()
                .expect("data array")
                .iter()
                .map(|card| card["id"].as_str().expect("id").to_owned())
                .collect();
        }
        assert!(
            Instant::now() < deadline,
            "arle serve did not answer within 60 s"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[test]
fn v1_models_lists_the_served_name_else_the_repo_name_of_a_hub_snapshot() {
    let dir = tempfile::tempdir().expect("tempdir");
    // a hub cache snapshot: the directory name is a revision hash
    let snapshot = dir
        .path()
        .join("models--acme--Tiny-4bit/snapshots/0123456789abcdef0123456789abcdef01234567");
    std::fs::create_dir_all(&snapshot).expect("snapshot dir");
    std::fs::write(snapshot.join("tokenizer.json"), TOKENIZER).expect("tokenizer");

    assert_eq!(
        listed_ids(&snapshot, &["--served-model-name", "tiny"]),
        ["tiny"]
    );
    assert_eq!(listed_ids(&snapshot, &[]), ["Tiny-4bit"]);
}
