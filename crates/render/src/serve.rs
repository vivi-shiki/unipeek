//! `serve`: one JSON request per line on stdin, one JSON reply per line on stdout. A host runs it
//! in a child process so a crashing graphics driver cannot take the host down.
use std::io::{BufRead, Write};

/// Serve requests until the input ends. Returns the process exit code (0 unless the output broke).
pub fn serve(_input: impl BufRead, _output: impl Write) -> i32 {
    todo!()
}
