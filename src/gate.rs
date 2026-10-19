// SPDX-License-Identifier: GPL-2.0-only
//! The code gate: nothing is measured before the code has passed every
//! check kernel.org would put it through.
//!
//! A tool about to measure calls [`require`] first. The gate builds the
//! series from the plan of the repository, from the working tree as it
//! stands, and runs the full checker on it -- builds, sparse, smatch,
//! coccinelle, checkpatch with --strict, apply, send, bisect, docs, the
//! newer trees. The result is kept under a directory named by the hash
//! of the tree it judged, so the same tree is not checked twice and a
//! tree that changed by one byte is checked again. The one thing left
//! open is the cover letter's placeholder for the sweep's own result,
//! which is what the measurement is about to produce.
use crate::{check, series, session};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// What a tool asks of the gate.
pub struct Gate<'a> {
    /// The filesystem repository, holding upstream/series.toml.
    pub repo: &'a Path,
    /// Where the gate keeps its verdicts, one directory per tree hash.
    pub root: &'a Path,
    /// The tool asking, for the terminal and the signed line.
    pub tool: &'a str,
    /// Cover letter placeholders the measurement will fill.
    pub placeholders: Vec<String>,
}

/// A code that passed: what to write in the record of the measurement.
#[derive(Debug, Clone)]
pub struct Passed {
    /// The commit the series carried: the working tree as a commit.
    pub measured: String,
    /// The hash of that tree, the key of the verdict kept.
    pub tree: String,
    /// The signed summary of the checker.
    pub summary: PathBuf,
    /// Whether a verdict kept for this very tree was reused.
    pub reused: bool,
}

impl Passed {
    /// One line for a record's meta.txt.
    #[must_use]
    pub fn meta_line(&self) -> String {
        format!(
            "code_gate=passed tree={} measured={} summary={} reused={} beamfs-upstream={}",
            self.tree,
            self.measured,
            self.summary.display(),
            if self.reused { "yes" } else { "no" },
            env!("CARGO_PKG_VERSION")
        )
    }
}

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let o = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .with_context(|| format!("git {}", args.join(" ")))?;
    if !o.status.success() {
        bail!(
            "git {} in {}: {}",
            args.join(" "),
            repo.display(),
            String::from_utf8_lossy(&o.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// The hash of the working tree as it stands: the tree of the commit
/// `git stash create` makes when something is not committed (of HEAD
/// otherwise), and every file git does not track yet and does not
/// ignore, by path and content. Two trees with the same files have the
/// same hash whatever commit or time produced them; a plan edited under
/// upstream/ before it is added changes it like a source does.
pub fn tree_hash(repo: &Path) -> Result<String> {
    let stash = git(repo, &["stash", "create"])?;
    let rev = if stash.is_empty() {
        "HEAD".to_string()
    } else {
        stash
    };
    let tracked = git(repo, &["rev-parse", &format!("{rev}^{{tree}}")])?;
    let others = git(repo, &["ls-files", "--others", "--exclude-standard"])?;
    if others.is_empty() {
        return Ok(tracked);
    }
    let mut lines = vec![format!("tree {tracked}")];
    for path in others.lines() {
        let blob = git(repo, &["hash-object", "--", path])?;
        lines.push(format!("{blob} {path}"));
    }
    let mut o = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["hash-object", "--stdin"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .context("git hash-object --stdin")?;
    {
        use std::io::Write;
        let mut stdin = o.stdin.take().context("stdin of git hash-object")?;
        stdin
            .write_all(lines.join("\n").as_bytes())
            .context("write to git hash-object")?;
    }
    let out = o.wait_with_output().context("git hash-object")?;
    if !out.status.success() {
        bail!("git hash-object failed");
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Whether a kept verdict says the code passed.
fn kept_verdict(summary: &Path) -> Option<(bool, String)> {
    let text = std::fs::read_to_string(summary).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let pass = v.get("pass")?.as_bool()?;
    let measured = v.get("measured_sha")?.as_str()?.to_string();
    Some((pass, measured))
}

/// Pass the code through the gate, or say why it is refused.
///
/// Prints the checker's report as it goes. The error names the checks
/// that failed; a tool that gets it measures nothing.
pub fn require(g: &Gate) -> Result<Passed> {
    let plan = g.repo.join("upstream/series.toml");
    if !plan.is_file() {
        bail!(
            "no plan at {}: the code cannot be checked, so it is not measured",
            plan.display()
        );
    }
    let tree = tree_hash(g.repo)?;
    let dir = g.root.join(&tree);
    let out = dir.join("check");
    let summary = out.join("upstream-summary.json");
    if let Some((true, measured)) = kept_verdict(&summary) {
        println!(
            "  code    : checked before as tree {}: {}",
            &tree[..12],
            summary.display()
        );
        return Ok(Passed {
            measured,
            tree,
            summary,
            reused: true,
        });
    }
    println!(
        "  code    : tree {} not checked yet: beamfs-upstream {} runs its full profile first",
        &tree[..12],
        env!("CARGO_PKG_VERSION")
    );
    session::prime_gpg(g.tool)?;
    std::fs::create_dir_all(&dir).with_context(|| format!("mkdir {}", dir.display()))?;
    let text = std::fs::read_to_string(&plan).with_context(|| plan.display().to_string())?;
    let p: series::Plan = toml::from_str(&text).with_context(|| plan.display().to_string())?;
    let linux = check::linux_repo();
    let series_tree = dir.join("series");
    // What an attempt that did not pass left behind: the checker wants a
    // directory of its own, and a series tree is a worktree of the kernel
    // repository that git must forget before the same path holds a new one.
    for stale in [&out, &series_tree] {
        if stale.exists() {
            std::fs::remove_dir_all(stale)
                .with_context(|| format!("remove {}", stale.display()))?;
        }
    }
    let _ = git(&linux, &["worktree", "prune"]);
    let built = series::build(&plan, "WORKTREE", &linux, &series_tree)
        .context("the series could not be built from the plan")?;
    println!(
        "  series  : {} on {} ({}), from {}",
        built.branch,
        built.base,
        &built.base_sha[..12.min(built.base_sha.len())],
        &built.measured[..12.min(built.measured.len())]
    );
    let cfg = check::Config {
        linux,
        beamfs: built.repo.clone(),
        series: built.branch.clone(),
        base: built.base.clone(),
        measured: built.measured.clone(),
        same_text_as: None,
        fs: p.series.fs.clone(),
        newer: built.newer.clone().unwrap_or_default(),
        fetch: true,
        subject_prefix: p.series.subject_prefix.clone(),
        author: p.series.author.clone(),
        assisted_by: p.series.assisted_by.clone(),
        cc: p.series.cc.clone(),
        max_mail_bytes: p.series.max_mail_bytes,
        strict_blocking: true,
        strict_exceptions: built.strict_exceptions.clone(),
        cover_placeholders_allowed: g.placeholders.clone(),
        cross: true,
        stack_limit: 512,
        out: out.clone(),
        quick: false,
    };
    let rc = check::run(&cfg)?;
    match kept_verdict(&summary) {
        Some((true, measured)) => Ok(Passed {
            measured,
            tree,
            summary,
            reused: false,
        }),
        Some((false, _)) => bail!(
            "the code does not pass the kernel.org checks (beamfs-upstream exit {rc}): \
             nothing is measured until it does; the report is {}",
            out.join("upstream-report.txt").display()
        ),
        None => bail!(
            "beamfs-upstream left no summary in {} (exit {rc})",
            out.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kept_verdict_is_read_from_its_json() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("upstream-summary.json");
        std::fs::write(&p, r#"{"pass": true, "measured_sha": "abc"}"#).unwrap();
        assert_eq!(kept_verdict(&p), Some((true, "abc".to_string())));
        std::fs::write(&p, r#"{"pass": false, "measured_sha": "abc"}"#).unwrap();
        assert_eq!(kept_verdict(&p), Some((false, "abc".to_string())));
        std::fs::write(&p, "not json").unwrap();
        assert_eq!(kept_verdict(&p), None);
    }

    #[test]
    fn the_tree_hash_is_the_trees() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let run = |a: &[&str]| git(r, a).unwrap();
        run(&["init", "-q"]);
        run(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "x",
        ]);
        let clean = tree_hash(r).unwrap();
        std::fs::write(r.join("f"), "1").unwrap();
        run(&["add", "f"]);
        let dirty = tree_hash(r).unwrap();
        assert_ne!(clean, dirty);
        let again = tree_hash(r).unwrap();
        assert_eq!(
            dirty, again,
            "the same tree hashes the same whatever stash commit was made"
        );
        std::fs::write(r.join("untracked"), "plan").unwrap();
        let with_untracked = tree_hash(r).unwrap();
        assert_ne!(
            dirty, with_untracked,
            "a file git does not track yet still counts"
        );
        std::fs::write(r.join("untracked"), "plan edited").unwrap();
        assert_ne!(
            with_untracked,
            tree_hash(r).unwrap(),
            "and so does its content"
        );
    }
}
