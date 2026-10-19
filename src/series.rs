// SPDX-License-Identifier: GPL-2.0-only
//! A kernel patch series built from the filesystem repository by a plan.
//!
//! On 2026-10-09 the beamfs series was assembled by hand on a branch of
//! the kernel repository. By 2026-10-14 the filesystem had moved by
//! three thousand lines and nobody had rebuilt the branch: every check
//! ran on code the measurements no longer described. The series is now
//! derived: a plan in the repository (`upstream/series.toml`) names the
//! patches, their messages and their files, and this module builds the
//! branch from a commit of the repository, never from its working tree,
//! on the newest mainline tag kernel.org lists. The branch is named
//! after the commit it carries, and the identity check holds by
//! construction.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The plan, `upstream/series.toml` of the filesystem repository.
#[derive(Deserialize)]
pub struct Plan {
    pub series: Series,
    #[serde(rename = "patch")]
    pub patches: Vec<Patch>,
}

/// What every patch shares.
#[derive(Deserialize)]
pub struct Series {
    /// Directory of the filesystem under fs/.
    pub fs: String,
    /// Branch name, before -vN-<commit>.
    pub branch: String,
    /// Version of the series, the N of -vN.
    pub version: u32,
    /// Subject prefix of the mails, "PATCH" or "RFC PATCH".
    pub subject_prefix: String,
    /// Author, From and Signed-off-by of every patch.
    pub author: String,
    /// Value of the Assisted-by line, empty for none.
    #[serde(default)]
    pub assisted_by: String,
    /// Cover letter body, relative to the plan.
    pub cover: String,
    /// Addresses put in Cc of every mail, besides what get_maintainer names.
    #[serde(default)]
    pub cc: Vec<String>,
    /// Largest mail accepted, in bytes.
    #[serde(default = "default_max_mail")]
    pub max_mail_bytes: u64,
    /// The tree the series is meant for, taken as the base in place of
    /// the newest mainline tag, which it has to hold.
    #[serde(default)]
    pub base: Option<BaseTree>,
    /// The revisions the series must merge into and build on; the
    /// checker's default when absent.
    #[serde(default)]
    pub newer: Option<Vec<String>>,
    /// The CHECKs of checkpatch --strict kept on purpose, with the
    /// mainline form each follows.
    #[serde(default, rename = "strict_exception")]
    pub strict_exceptions: Vec<StrictException>,
}

/// CHECKs of one file kept on purpose: `types` are checkpatch's type
/// names, `why` names the mainline form they follow.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct StrictException {
    pub file: String,
    pub types: Vec<String>,
    pub why: String,
}

/// A branch of a tree, fetched at build time.
#[derive(Deserialize)]
pub struct BaseTree {
    pub url: String,
    pub branch: String,
}

fn default_max_mail() -> u64 {
    100_000
}

/// One patch: its message file (subject on the first line, body after
/// a blank one) and what it adds to the tree.
#[derive(Deserialize)]
pub struct Patch {
    pub message: String,
    /// Files of the repository root, placed under fs/<fs>/.
    #[serde(default)]
    pub add: Vec<String>,
    /// Files under upstream/tree/, placed at their path.
    #[serde(default)]
    pub tree: Vec<String>,
    /// The next slice of a file of the repository root.
    #[serde(default)]
    pub part: Option<Part>,
    /// Lines inserted after an anchor in files of the tree.
    #[serde(default)]
    pub insert: Vec<Insert>,
    /// A block placed in MAINTAINERS by alphabetical order of its title.
    #[serde(default)]
    pub maintainers: Option<String>,
}

/// A slice of a file: from where the previous slice ended, up to the
/// line `upto` excluded, or to the end when `upto` is absent.
#[derive(Deserialize)]
pub struct Part {
    pub file: String,
    #[serde(default)]
    pub upto: Option<String>,
}

/// One line inserted after the first line equal to `after`.
#[derive(Deserialize)]
pub struct Insert {
    pub file: String,
    pub after: String,
    pub line: String,
}

/// What a build left: the branch, in the kernel repository, and what
/// the checker needs to know about it.
pub struct Built {
    pub branch: String,
    pub base: String,
    pub base_sha: String,
    pub measured: String,
    pub repo: PathBuf,
    pub tree: PathBuf,
    /// The revisions the plan wants the series checked against.
    pub newer: Option<Vec<String>>,
    /// The CHECKs of --strict the plan keeps on purpose.
    pub strict_exceptions: Vec<StrictException>,
    /// The Cc addresses of the plan.
    pub cc: Vec<String>,
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let o = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .with_context(|| format!("git {} in {}", args.join(" "), dir.display()))?;
    if !o.status.success() {
        bail!(
            "git {} in {}: {}",
            args.join(" "),
            dir.display(),
            String::from_utf8_lossy(&o.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim_end().to_string())
}

/// A file of the repository at the measured commit, byte for byte: its
/// last newline is part of it.
fn at(repo: &Path, rev: &str, file: &str) -> Result<String> {
    let spec = format!("{rev}:{file}");
    let o = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["show", &spec])
        .output()
        .with_context(|| format!("git show {spec} in {}", repo.display()))?;
    if !o.status.success() {
        bail!(
            "{file} is not in commit {rev}: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

/// Read the plan beside its files.
///
/// # Errors
///
/// The plan unreadable or not a plan.
pub fn read(plan: &Path) -> Result<Plan> {
    let text = fs::read_to_string(plan).with_context(|| format!("read {}", plan.display()))?;
    toml::from_str(&text).with_context(|| format!("{} is not a plan", plan.display()))
}

/// The lines of `text` from `from`, up to the first line equal to
/// `upto` after it, excluded; to the end without `upto`.
fn slice(text: &str, from: usize, upto: Option<&str>) -> Result<(String, usize)> {
    let lines: Vec<&str> = text.lines().collect();
    if from >= lines.len() {
        bail!("nothing left after line {from}");
    }
    let end = match upto {
        Some(u) => lines
            .iter()
            .enumerate()
            .skip(from + 1)
            .find(|(_, l)| **l == u)
            .map(|(i, _)| i)
            .ok_or_else(|| anyhow::anyhow!("no line \"{u}\" after line {}", from + 1))?,
        None => lines.len(),
    };
    let mut s = lines[from..end].join("\n");
    s.push('\n');
    Ok((s, end))
}

/// `text` with `line` inserted after the first line equal to `after`.
fn inserted(text: &str, after: &str, line: &str) -> Result<String> {
    if text.lines().any(|l| l == line) {
        bail!("the line \"{line}\" is already there");
    }
    let mut out = String::new();
    let mut done = false;
    for l in text.lines() {
        out.push_str(l);
        out.push('\n');
        if !done && l == after {
            out.push_str(line);
            out.push('\n');
            done = true;
        }
    }
    if !done {
        bail!("no line \"{after}\" to insert after");
    }
    Ok(out)
}

/// MAINTAINERS with `block` (a title line, its fields) placed before the
/// first section whose title sorts after the block's, with a blank
/// line between sections.
fn with_section(text: &str, block: &str) -> Result<String> {
    let title = block.lines().next().unwrap_or("").trim_end();
    if title.is_empty() {
        bail!("the MAINTAINERS block has no title line");
    }
    if text.lines().any(|l| l == title) {
        bail!("MAINTAINERS already has a section \"{title}\"");
    }
    let lines: Vec<&str> = text.lines().collect();
    let mut at = None;
    for i in 1..lines.len().saturating_sub(1) {
        let is_title = lines[i - 1].is_empty()
            && !lines[i].is_empty()
            && lines[i + 1].len() > 2
            && &lines[i + 1][1..2] == ":"
            && lines[i].starts_with(|c: char| c.is_ascii_uppercase() || c.is_ascii_digit());
        if is_title && lines[i] > title {
            at = Some(i);
            break;
        }
    }
    let Some(i) = at else {
        bail!("no section of MAINTAINERS sorts after \"{title}\"");
    };
    let mut out = lines[..i].join("\n");
    out.push('\n');
    out.push_str(block.trim_end());
    out.push_str("\n\n");
    out.push_str(&lines[i..].join("\n"));
    out.push('\n');
    Ok(out)
}

/// The worktrees that have `branch` checked out, from
/// `git worktree list --porcelain`: a previous build of the same
/// commit, elsewhere, holds the branch until it is removed.
fn worktrees_on(listed: &str, branch: &str) -> Vec<String> {
    let want = format!("branch refs/heads/{branch}");
    let mut out = Vec::new();
    let mut path: Option<&str> = None;
    for l in listed.lines() {
        if let Some(p) = l.strip_prefix("worktree ") {
            path = Some(p);
        } else if l == want {
            if let Some(p) = path {
                out.push(p.to_string());
            }
        }
    }
    out
}

fn write(tree: &Path, rel: &str, text: &str) -> Result<()> {
    let p = tree.join(rel);
    if let Some(d) = p.parent() {
        fs::create_dir_all(d).with_context(|| format!("create {}", d.display()))?;
    }
    fs::write(&p, text).with_context(|| format!("write {}", p.display()))
}

fn append(tree: &Path, rel: &str, text: &str) -> Result<()> {
    let p = tree.join(rel);
    let mut s = fs::read_to_string(&p).unwrap_or_default();
    s.push_str(text);
    write(tree, rel, &s)
}

/// Build the series of `plan` from commit `measured` of the repository
/// holding it, on the newest mainline tag, as a branch of `linux`
/// checked out in `tree`. Commits are signed by the author's key.
/// `measured` may be `WORKTREE`: the working tree as it is, through a
/// commit `git stash create` makes and stores nowhere.
///
/// # Errors
///
/// The plan or a file of it missing, the tag not fetchable, a cut or
/// an anchor not found, git refusing a step.
pub fn build(plan_path: &Path, measured: &str, linux: &Path, tree: &Path) -> Result<Built> {
    let plan = read(plan_path)?;
    let updir = plan_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("{} has no directory", plan_path.display()))?;
    let repo = updir
        .parent()
        .ok_or_else(|| anyhow::anyhow!("{} is not inside a repository", updir.display()))?
        .to_path_buf();
    let sha = if measured == "WORKTREE" {
        // A commit of the working tree as it is, made by `git stash
        // create` and stored nowhere: no branch moves, the index is
        // left alone, and the series carries uncommitted work so it
        // can be checked before it is committed. Empty when the tree
        // is clean, and HEAD is then what the tree holds.
        let s = git(&repo, &["stash", "create"])?;
        if s.is_empty() {
            git(&repo, &["rev-parse", "HEAD^{commit}"])?
        } else {
            s
        }
    } else {
        git(&repo, &["rev-parse", &format!("{measured}^{{commit}}")])?
    };
    let tag = crate::kernelorg::latest().map_err(|e| anyhow::anyhow!("kernel.org: {e}"))?;
    let held = format!("{}^{{commit}}", tag.commit);
    if git(linux, &["cat-file", "-e", &held]).is_err() {
        git(
            linux,
            &["fetch", "-q", crate::kernelorg::MAINLINE, "tag", &tag.name],
        )
        .with_context(|| format!("fetch {}", tag.name))?;
    }
    // The base: the newest mainline tag, or the branch of the tree the
    // series is meant for, which has to hold that tag.
    let (base_name, base_sha, base_text) = match &plan.series.base {
        None => (tag.name.clone(), tag.commit.clone(), tag.name.clone()),
        Some(b) => {
            git(linux, &["fetch", "-q", &b.url, &b.branch])
                .with_context(|| format!("fetch {} of {}", b.branch, b.url))?;
            let sha = git(linux, &["rev-parse", "FETCH_HEAD^{commit}"])?;
            if git(linux, &["merge-base", "--is-ancestor", &tag.commit, &sha]).is_err() {
                bail!(
                    "{} of {} ({}) does not hold {}, the newest tag on kernel.org",
                    b.branch,
                    b.url,
                    &sha[..12],
                    tag.name
                );
            }
            let short = b.url.rsplit('/').next().unwrap_or(&b.url).to_string();
            (
                sha.clone(),
                sha.clone(),
                format!(
                    "{} of {} (commit {}, on {})",
                    b.branch,
                    short,
                    &sha[..12],
                    tag.name
                ),
            )
        }
    };
    let branch = format!(
        "{}-v{}-{}",
        plan.series.branch,
        plan.series.version,
        &sha[..12]
    );
    if tree.exists() {
        git(
            linux,
            &["worktree", "remove", "--force", &tree.to_string_lossy()],
        )?;
    }
    let _ = git(linux, &["worktree", "prune"]);
    for held in worktrees_on(&git(linux, &["worktree", "list", "--porcelain"])?, &branch) {
        git(linux, &["worktree", "remove", "--force", &held])
            .with_context(|| format!("the worktree {held} holds {branch}"))?;
    }
    if git(
        linux,
        &[
            "rev-parse",
            "--verify",
            "-q",
            &format!("refs/heads/{branch}"),
        ],
    )
    .is_ok()
    {
        git(linux, &["branch", "-D", &branch])?;
    }
    git(
        linux,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            &branch,
            &tree.to_string_lossy(),
            &base_sha,
        ],
    )?;
    let fsdir = format!("fs/{}", plan.series.fs);
    let (name, mail) = plan
        .series
        .author
        .split_once(" <")
        .map(|(n, m)| (n.to_string(), m.trim_end_matches('>').to_string()))
        .ok_or_else(|| anyhow::anyhow!("author \"{}\" is not Name <mail>", plan.series.author))?;
    let mut consumed: BTreeMap<String, usize> = BTreeMap::new();
    for (n, p) in plan.patches.iter().enumerate() {
        let n = n + 1;
        let msg = fs::read_to_string(updir.join(&p.message))
            .with_context(|| format!("patch {n}: read {}", p.message))?;
        let mut paths: Vec<String> = Vec::new();
        for f in &p.add {
            let rel = format!("{fsdir}/{f}");
            write(
                tree,
                &rel,
                &at(&repo, &sha, f).with_context(|| format!("patch {n}"))?,
            )?;
            paths.push(rel);
        }
        for f in &p.tree {
            let src = updir.join("tree").join(f);
            let text = fs::read_to_string(&src)
                .with_context(|| format!("patch {n}: read {}", src.display()))?;
            write(tree, f, &text)?;
            paths.push(f.clone());
        }
        if let Some(part) = &p.part {
            let text = at(&repo, &sha, &part.file).with_context(|| format!("patch {n}"))?;
            let from = consumed.get(&part.file).copied().unwrap_or(0);
            let (s, end) = slice(&text, from, part.upto.as_deref())
                .with_context(|| format!("patch {n}: {}", part.file))?;
            let rel = format!("{fsdir}/{}", part.file);
            append(tree, &rel, &s)?;
            consumed.insert(part.file.clone(), end);
            paths.push(rel);
        }
        for ins in &p.insert {
            let p0 = tree.join(&ins.file);
            let text =
                fs::read_to_string(&p0).with_context(|| format!("patch {n}: read {}", ins.file))?;
            let out = inserted(&text, &ins.after, &ins.line)
                .with_context(|| format!("patch {n}: {}", ins.file))?;
            write(tree, &ins.file, &out)?;
            paths.push(ins.file.clone());
        }
        if let Some(m) = &p.maintainers {
            let block = fs::read_to_string(updir.join(m))
                .with_context(|| format!("patch {n}: read {m}"))?;
            let text = fs::read_to_string(tree.join("MAINTAINERS"))?;
            write(
                tree,
                "MAINTAINERS",
                &with_section(&text, &block).with_context(|| format!("patch {n}"))?,
            )?;
            paths.push("MAINTAINERS".to_string());
        }
        if paths.is_empty() {
            bail!("patch {n} adds nothing");
        }
        let mut add = vec!["add", "--"];
        add.extend(paths.iter().map(String::as_str));
        git(tree, &add)?;
        let mut full = msg.trim_end().to_string();
        full.push_str("\n\n");
        if !plan.series.assisted_by.is_empty() {
            full.push_str(&format!("Assisted-by: {}\n", plan.series.assisted_by));
        }
        full.push_str(&format!("Signed-off-by: {}\n", plan.series.author));
        let msg_path = tree.join(".git-series-message");
        fs::write(&msg_path, &full)?;
        let author = format!("--author={}", plan.series.author);
        git(
            tree,
            &[
                "-c",
                &format!("user.name={name}"),
                "-c",
                &format!("user.email={mail}"),
                "commit",
                "-q",
                "-S",
                &author,
                "-F",
                &msg_path.to_string_lossy(),
            ],
        )
        .with_context(|| format!("patch {n}: commit"))?;
        fs::remove_file(&msg_path)?;
    }
    for (f, end) in &consumed {
        let total = at(&repo, &sha, f)?.lines().count();
        if *end != total {
            bail!("{f}: {end} of {total} lines placed, the last part must have no upto");
        }
    }
    // What the generator knows and the letter should not repeat by
    // hand: the commit the series carries and the tag it sits on.
    let cover = fs::read_to_string(updir.join(&plan.series.cover))
        .with_context(|| format!("read {}", plan.series.cover))?
        .replace("@@COMMIT@@", &sha[..12])
        .replace("@@BASE@@", &base_text);
    git(
        linux,
        &["config", &format!("branch.{branch}.description"), &cover],
    )?;
    Ok(Built {
        branch,
        base: base_name,
        base_sha,
        measured: sha,
        repo,
        tree: tree.to_path_buf(),
        newer: plan.series.newer.clone(),
        strict_exceptions: plan.series.strict_exceptions.clone(),
        cc: plan.series.cc.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::{inserted, slice, with_section, worktrees_on};

    #[test]
    fn the_worktrees_holding_a_branch_are_named() {
        let l = "worktree /a\nHEAD 1111\nbranch refs/heads/x\n\nworktree /b\nHEAD 2222\nbranch refs/heads/y\n\nworktree /c\nHEAD 3333\ndetached\n";
        assert_eq!(worktrees_on(l, "y"), vec!["/b".to_string()]);
        assert!(worktrees_on(l, "z").is_empty());
    }

    #[test]
    fn slices_follow_one_another() {
        let t = "a\nb\nc\nd\n";
        let (s, end) = slice(t, 0, Some("c")).unwrap();
        assert_eq!((s.as_str(), end), ("a\nb\n", 2));
        let (s, end) = slice(t, end, None).unwrap();
        assert_eq!((s.as_str(), end), ("c\nd\n", 4));
        assert!(slice(t, 4, None).is_err());
        assert!(slice(t, 0, Some("z")).is_err());
    }

    #[test]
    fn a_line_goes_after_its_anchor_once() {
        assert_eq!(inserted("x\ny\n", "x", "n").unwrap(), "x\nn\ny\n");
        assert!(inserted("x\ny\n", "q", "n").is_err());
        assert!(inserted("x\nn\n", "x", "n").is_err());
    }

    #[test]
    fn a_section_lands_in_alphabetical_order() {
        let m = "AAA\nM:\tsomeone\n\nBDISP\nM:\tsomeone\nF:\tdrivers/x\n\nBECKHOFF\nM:\tx\n";
        let out = with_section(m, "BEAMFS FILE SYSTEM\nM:\tme\nF:\tfs/beamfs/\n").unwrap();
        assert_eq!(
            out,
            "AAA\nM:\tsomeone\n\nBDISP\nM:\tsomeone\nF:\tdrivers/x\n\nBEAMFS FILE SYSTEM\nM:\tme\nF:\tfs/beamfs/\n\nBECKHOFF\nM:\tx\n"
        );
        assert!(with_section(&out, "BEAMFS FILE SYSTEM\nM:\tme\n").is_err());
        assert!(with_section("AAA\nM:\tx\n", "ZZZ\nM:\tx\n").is_err());
    }
}
