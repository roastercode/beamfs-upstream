// SPDX-License-Identifier: GPL-2.0-only
//! The credentials a run needs before it starts, asked for at the start.
//!
//! The series is built from signed commits, so a run that has not got
//! the GPG passphrase cached stops in the middle of its work on a
//! pinentry nobody answers, and the agent times it out. Asked at the
//! start, once, it is there for the run; a keep-alive is the caller's
//! business.
use anyhow::{bail, Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};

/// Whether gpg-agent holds a cached passphrase for some key.
pub fn gpg_cached() -> Result<bool> {
    let out = Command::new("gpg-connect-agent")
        .args(["KEYINFO --list", "/bye"])
        .env("PAGER", "cat")
        .output()
        .context("spawn gpg-connect-agent")?;
    if !out.status.success() {
        bail!(
            "gpg-connect-agent failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    Ok(stdout
        .lines()
        .any(|line| line.starts_with("S KEYINFO ") && line.split_whitespace().nth(6) == Some("1")))
}

/// Make sure gpg-agent holds the passphrase: when it does not, sign a
/// line interactively now, so the pinentry comes at the start of the run
/// and not in the middle of it. `tool` names who is asking, in the text
/// signed and on the terminal.
pub fn prime_gpg(tool: &str) -> Result<()> {
    print!("[priming] checking gpg-agent cache state... ");
    std::io::stdout().flush().ok();
    if gpg_cached()? {
        println!("primed");
        return Ok(());
    }
    println!("empty, asking for the passphrase now");
    let mut child = Command::new("gpg")
        .args(["--clearsign", "--armor", "-o", "/dev/null"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .context("spawn gpg --clearsign")?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(format!("{tool} session priming\n").as_bytes())
            .context("write to gpg stdin")?;
    }
    let status = child.wait().context("wait gpg")?;
    if !status.success() {
        bail!("gpg sign returned {status} (passphrase declined)");
    }
    println!("[priming] gpg cache populated");
    Ok(())
}
