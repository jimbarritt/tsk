//! A reader that closes the pipe early, as `tsk thread list | head -1` does, ends the
//! process quietly, as other command-line tools do. Rust ignores SIGPIPE by default, so
//! a `println!` on the closed pipe would panic with "failed printing to stdout".

#![cfg(unix)]

use std::process::{Command, Stdio};

#[test]
fn a_closed_stdout_ends_the_process_without_a_panic() {
    let (reader, writer) = std::io::pipe().unwrap();
    // The read end is closed before the process writes, so its first write fails.
    drop(reader);
    let output = Command::new(env!("CARGO_BIN_EXE_tsk"))
        .args(["guide", "import"])
        .stdin(Stdio::null())
        .stdout(writer)
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("panicked"), "{}", stderr);
    assert!(!stderr.contains("Broken pipe"), "{}", stderr);
    assert!(!stderr.contains("failed printing"), "{}", stderr);
    assert_ne!(
        output.status.code(),
        Some(101),
        "exited like a panic: {}",
        stderr
    );
}
