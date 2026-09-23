//! A reader that stops early (`pcli2 ... | head`) is not a failure.
//!
//! It used to panic inside `println!` and exit 101, the documented *network error*
//! code, so a script following the exit-code table blamed the network for a `head`.

use assert_cmd::prelude::*;
use std::io::Read;
use std::process::{Command, Stdio};

fn pcli2() -> Command {
    let dir = std::env::temp_dir().join(format!("pcli2-pipe-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut cmd = Command::cargo_bin("pcli2").unwrap();
    cmd.env("PCLI2_CONFIG_DIR", &dir)
        .env("PCLI2_CACHE_DIR", dir.join("cache"))
        .env("PCLI2_NO_UPDATE_CHECK", "1")
        .env_remove("PCLI2_ERROR_FORMAT")
        .stdin(Stdio::null());
    cmd
}

/// Run a command whose output is far larger than a pipe buffer, read a few bytes,
/// then close the pipe, as `head -c 16` would.
fn run_and_close_early(args: &[&str]) -> std::process::Output {
    let mut child = pcli2()
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut first = [0u8; 16];
    stdout.read_exact(&mut first).unwrap();
    drop(stdout);
    child.wait_with_output().unwrap()
}

#[cfg(unix)]
#[test]
fn closing_the_pipe_early_exits_zero_without_a_panic() {
    // Shell completions are several hundred kilobytes, well past the 64 KiB pipe
    // buffer, so the write after the reader has gone is certain to fail.
    let output = run_and_close_early(&["completions", "powershell"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "stderr: {stderr}");
    assert!(!stderr.contains("panicked"), "stderr: {stderr}");
}
