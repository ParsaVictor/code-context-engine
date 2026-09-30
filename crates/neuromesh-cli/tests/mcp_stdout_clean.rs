//! `neuromesh mcp` speaks line-delimited JSON-RPC on stdout, and the same
//! process also starts the local dashboard. Anything else written to stdout
//! (a banner, a log line) breaks strict MCP clients that parse every line.
//! This spawns the real binary and checks that every stdout line is JSON.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[test]
fn mcp_stdout_carries_only_json_rpc() {
    let home = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(
        workspace.path().join("lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    )
    .unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_neuromesh"))
        .arg("mcp")
        .current_dir(workspace.path())
        .env("NEUROMESH_HOME", home.path())
        .env("NEUROMESH_NO_BROWSER", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn neuromesh mcp");

    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(l) => {
                    if tx.send(l).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut stdin = child.stdin.take().unwrap();
    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"stdout-test","version":"0"}}}"#;
    let list = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#;
    writeln!(stdin, "{init}").unwrap();
    writeln!(stdin, "{list}").unwrap();
    stdin.flush().unwrap();

    let deadline = Instant::now() + Duration::from_secs(90);
    let mut lines = Vec::new();
    let mut saw_list = false;
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => {
                if line.contains("\"id\":2") {
                    saw_list = true;
                }
                lines.push(line);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        // Give the dashboard task time to bind and print after the replies.
        if saw_list && lines.len() >= 2 {
            std::thread::sleep(Duration::from_secs(2));
            while let Ok(line) = rx.try_recv() {
                lines.push(line);
            }
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();

    assert!(saw_list, "no tools/list reply on stdout; got {lines:?}");
    for line in lines.iter().filter(|l| !l.trim().is_empty()) {
        assert!(
            serde_json::from_str::<serde_json::Value>(line).is_ok(),
            "non-JSON line on MCP stdout: {line:?}"
        );
    }
}
