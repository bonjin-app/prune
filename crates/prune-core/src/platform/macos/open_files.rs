//! Which files this user's running programs have open, from `lsof`.
//!
//! `lsof` ships with every macOS install at a fixed path and already knows every kind of
//! "open" that matters for a temporary folder: open files and memory-mapped ones, working
//! directories, and Unix sockets bound to a path (a browser's `SingletonSocket`, an editor's
//! `host.sock`). Asking it once for the whole user and matching the answer against the
//! candidates costs about a second on a busy machine, once per scan, alongside the other
//! providers rather than before them.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::platform::{match_open_paths, InUse};

const LSOF: &str = "/usr/sbin/lsof";
const ID: &str = "/usr/bin/id";
/// `lsof` can stall on an unresponsive network volume. A scan should not.
const TIMEOUT: Duration = Duration::from_secs(15);

pub fn in_use(candidates: &[PathBuf]) -> InUse {
    if candidates.is_empty() {
        return InUse::none();
    }
    match open_paths() {
        Ok(open) => InUse::Known(match_open_paths(candidates, open)),
        Err(reason) => InUse::Unknown(reason),
    }
}

fn current_uid() -> Result<String, String> {
    let out = Command::new(ID)
        .arg("-u")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("could not run {ID}: {e}"))?;
    let uid = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || uid.is_empty() || !uid.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("{ID} -u did not report a user id"));
    }
    Ok(uid)
}

/// Every path this user's processes have open, with the program holding it.
fn open_paths() -> Result<Vec<(PathBuf, Option<String>)>, String> {
    let uid = current_uid()?;
    // -n -P: never resolve addresses or ports (no DNS, ever). -w: no warnings about processes
    // that cannot be read. +c0: the whole program name. -F: machine-readable fields, here
    // p(id), c(ommand) and n(ame); f(d) comes along regardless.
    let mut child = Command::new(LSOF)
        .args(["-n", "-P", "-w", "+c0", "-Fpcn", "-u", &uid])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("could not run {LSOF}: {e}"))?;

    // The output runs to megabytes; read it while waiting, or a full pipe stalls lsof.
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        stdout.read_to_end(&mut buf).map(|_| buf)
    });

    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "{LSOF} did not finish within {} seconds",
                    TIMEOUT.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => return Err(format!("{LSOF} failed: {e}")),
        }
    };
    let output = reader
        .join()
        .map_err(|_| format!("reading {LSOF} output failed"))?
        .map_err(|e| format!("reading {LSOF} output failed: {e}"))?;

    // lsof exits 1 when some process could not be read, and still lists the rest. Only an
    // empty answer is no answer: this process alone always has files open.
    if output.is_empty() {
        return Err(format!("{LSOF} reported nothing (exit status {status})"));
    }
    Ok(parse(&output))
}

/// Reads `lsof -F pcn` output: a `p` line starts a process, `c` names it, and each `n` line is
/// one open file of that process.
fn parse(output: &[u8]) -> Vec<(PathBuf, Option<String>)> {
    let mut program: Option<String> = None;
    let mut open = Vec::new();
    for line in output.split(|&b| b == b'\n') {
        let Some((&field, value)) = line.split_first() else {
            continue;
        };
        match field {
            b'p' => program = None,
            b'c' => program = Some(String::from_utf8_lossy(&unescape(value)).into_owned()),
            b'n' if value.first() == Some(&b'/') => {
                open.push((bytes_to_path(unescape(value)), program.clone()));
            }
            _ => {}
        }
    }
    open
}

/// lsof writes every byte it does not consider printable as `\xNN`, which includes each byte
/// of a non-ASCII name. Undoing that is what lets a folder called `한글` match at all.
fn unescape(value: &[u8]) -> Vec<u8> {
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(value.len());
    let mut i = 0;
    while i < value.len() {
        if value[i] == b'\\' && value.get(i + 1) == Some(&b'x') {
            if let (Some(hi), Some(lo)) = (
                value.get(i + 2).copied().and_then(hex),
                value.get(i + 3).copied().and_then(hex),
            ) {
                out.push(hi << 4 | lo);
                i += 4;
                continue;
            }
        }
        out.push(value[i]);
        i += 1;
    }
    out
}

fn bytes_to_path(bytes: Vec<u8>) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;
    PathBuf::from(std::ffi::OsString::from_vec(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_programs_and_their_open_paths() {
        let output = b"p101\ncGoogle Chrome\nfcwd\nn/\nf12\nn/var/folders/T/com.google.Chrome.x/SingletonSocket\n\
p202\nccodex\nf3\nn/private/var/folders/T/\\xed\\x95\\x9c dir/a.txt\nf4\nn*:5000\n";
        let open = parse(output);
        assert_eq!(
            open,
            vec![
                (PathBuf::from("/"), Some("Google Chrome".into())),
                (
                    PathBuf::from("/var/folders/T/com.google.Chrome.x/SingletonSocket"),
                    Some("Google Chrome".into())
                ),
                (
                    PathBuf::from("/private/var/folders/T/한 dir/a.txt"),
                    Some("codex".into())
                ),
            ]
        );
    }

    #[test]
    fn leaves_a_backslash_that_is_not_an_escape_alone() {
        assert_eq!(unescape(br"a\xZZ\x4"), br"a\xZZ\x4".to_vec());
        assert_eq!(unescape(br"\x41b"), b"Ab".to_vec());
    }
}
