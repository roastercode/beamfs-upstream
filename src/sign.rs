// SPDX-License-Identifier: GPL-2.0-only
//! Signing a manifest with the key gpg-agent holds.

use std::path::PathBuf;
use std::process::Command;

// ---------------------------------------------------------------------
// Phase 8.2
// ---------------------------------------------------------------------
/// How long gpg is given to sign a manifest.
pub const SIGN_TIMEOUT: std::time::Duration = std::time::Duration::from_mins(2);

/// Whether gpg-agent holds a key it can sign with, without asking.
///
/// The question session priming asks at start, asked again here: the
/// cache has a lifetime of its own (max-cache-ttl), and a pipeline that
/// started with the key cached can end after it has expired.
pub fn gpg_key_cached() -> bool {
    Command::new("gpg-connect-agent")
        .args(["KEYINFO --list", "/bye"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .is_some_and(|o| {
            String::from_utf8_lossy(&o.stdout).lines().any(|l| {
                let t: Vec<&str> = l.split_whitespace().collect();
                l.starts_with("S KEYINFO ") && t.len() >= 7 && t[6] == "1"
            })
        })
}

/// Sign `path` into `<path>.asc` with `gpg` (the program and its leading
/// arguments), within `timeout`.
///
/// The signature is written to `<path>.asc.part` and renamed once gpg has
/// succeeded. On a refusal or a timeout gpg is killed and nothing is
/// left beside the manifest.
pub fn sign_manifest(
    gpg: &[&str],
    path: &std::path::Path,
    timeout: std::time::Duration,
) -> std::result::Result<PathBuf, String> {
    let asc = PathBuf::from(format!("{}.asc", path.display()));
    let part = PathBuf::from(format!("{}.asc.part", path.display()));
    let _ = std::fs::remove_file(&part);
    let (prog, lead) = gpg.split_first().ok_or("no signing program")?;
    let mut child = Command::new(prog)
        .args(lead)
        .args([
            "--batch",
            "--pinentry-mode",
            "loopback",
            "--detach-sign",
            "--armor",
            "--yes",
            "--output",
        ])
        .arg(&part)
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("{prog} did not start: {e}"))?;
    let t0 = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if t0.elapsed() < timeout => {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_file(&part);
                return Err(format!(
                    "{prog} did not finish within {}s and was killed",
                    timeout.as_secs()
                ));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_file(&part);
                return Err(format!("waiting for {prog}: {e}"));
            }
        }
    };
    let mut said = String::new();
    if let Some(mut err) = child.stderr.take() {
        let _ = std::io::Read::read_to_string(&mut err, &mut said);
    }
    if !status.success() {
        let _ = std::fs::remove_file(&part);
        return Err(format!("{prog} exited with {status}: {}", said.trim()));
    }
    std::fs::rename(&part, &asc).map_err(|e| format!("rename {}: {e}", part.display()))?;
    Ok(asc)
}
