# Changelog

## 0.8.1

- `Built.base_behind`, missing from 0.8.0, which did not build.

## 0.8.0

- `base.behind = "<reason>"` in the plan: the series is built and checked on a
  base that does not hold the newest tag on kernel.org, the letter's `@@BASE@@`
  names the tag the base does hold (git describe), and the base check passes
  saying so, the reason in the signed summary. Without it a base behind the
  newest tag still refuses. v7.3 was tagged on 2026-10-18 while the series was
  measured on vfs.all (2c3ba1f1cb41, on v7.3-rc7), which has not moved since.

## 0.7.2

### Fixed
- `gate::tree_hash` counts the files git does not track yet: a plan
  edited under upstream/ before `git add` changed nothing in the hash,
  and an old verdict was reused for a new plan.

## 0.7.1

### Added
- `series.cc` in the plan: the addresses every mail carries in Cc; the
  gate sends them, beamfs-bench takes them over its `--cc`.

## 0.7.0

### Added
- `gate`: the code gate every measuring tool runs first. It builds the
  series from the plan on the working tree, runs the full profile, keeps
  the signed summary under a directory named by the hash of the tree it
  judged, and reuses it for the same tree; the cover letter placeholder
  the measurement fills is the one thing it leaves open. A code that does
  not pass is not measured.
- `session::prime_gpg`: the passphrase is asked at the start of a run,
  never in the middle; moved here from beamfs-bench so both tools do it
  the same way.
- `Config::cover_placeholders_allowed`.

## 0.6.1

### Fixed
- `newer`: when the base itself does not merge into a newer tree, the
  series has nothing to be judged against, for a cause outside it; the
  check ends not applicable, with the conflicts named, instead of skipped,
  which held the verdict although nothing of the series was at fault.

## 0.6.0

### Added
- `series.strict_exception` in the plan: the CHECKs of `checkpatch --strict`
  kept on purpose, per file and type, each with the mainline form it
  follows. `check` leaves those out of the count, notes every exception
  the findings use and fails on one that nothing uses any more, so the
  list cannot go stale. Every other CHECK blocks when a plan drives the
  run.

## 0.5.0

- The plan may name the tree the series is meant for (`base = { url,
  branch }`, the T: tree of its subsystem): fetched at build time and
  taken as the base, which has to hold the newest mainline tag; and
  the revisions to merge into and build on (`newer`). `@@BASE@@` then
  names the branch, the commit and the tag it holds.
- `newer`: when the base itself does not merge into a revision, the
  series is not judged there that day and the run says so, with the
  files in conflict; when the series does conflict, the files are
  named rather than the oid of the tree.
  When no revision could judge the series, `newer` is reported not
  run, and the series is not ready that day.

## 0.4.0

- `apply`: the mails applied with git am on the base in a worktree of
  their own; the result must be the branch, tree for tree, one commit
  per patch, every commit authored as mailed.
- `send`: git send-email --dry-run of every mail, to the people
  get_maintainer names, Cc its lists, the configured addresses and
  linux-api when the series has userspace interfaces; every mail must
  be from the author; the real command is written to send.sh beside
  the patches. Both run in the quick profile too.
- `bisect`: fs/ built at every commit of the series, x86_64 with the
  filesystem built in, as the build robots do.
- Messages: a subject may not be a file name nor repeat another's;
  the body stays within 75 columns, trailers and URLs excepted; the
  last Signed-off-by is the submitter's; Assisted-by may not list git,
  gcc, make and the other basic tools; more than 15 patches is noted.
- `interfaces`: a boot parameter, an ioctl or a /proc entry has to be
  documented where submit-checklist says.
- `docs`: the pages sphinx generates from Documentation/ABI/ are
  reported under the output directory; it is taken out of the
  comparison like the tree's path, so a series touching ABI/ is no
  longer charged with the base's own warnings.

## 0.3.0

- `series`: the series is built from a plan of the filesystem
  repository (`upstream/series.toml`: patches, messages, files, the
  slices of a file too large for one mail, anchored insertions in
  fs/Kconfig, fs/Makefile and the documentation index, the MAINTAINERS
  section) from a commit of that repository, never its working tree,
  on the newest mainline tag kernel.org lists, as a signed branch
  named after the commit, its cover letter in the branch description.
  `beamfs-bench upstream --plan` builds it then checks it.
- `maintainers` no longer demands the MAINTAINERS entry before or with
  the first patch that adds files: a series usually ends with it. The
  entry has to be in the series; its position is noted.
- `Config::quick`: the checks of seconds only (identity, commits,
  checkpatch, sizes, prose, kconfig, interfaces), as a gate before a
  measurement; builds, maintainers, newer and docs are reported not
  run, the run gives no verdict on mailing and ends with 0 when
  nothing it ran failed.
- `series::build` takes `WORKTREE` as the measured commit: the working
  tree as it is, through a commit `git stash create` makes and stores
  nowhere, so a change is checked before it is committed.
- The cover letter's `@@COMMIT@@` and `@@BASE@@` are replaced by the
  commit the series carries and the tag it sits on.

## 0.2.0

- The build checks count a diagnostic only when it names a file the
  series changes or a path under fs/<fs>/; the kernel's own headers
  answering clang or -W are counted in the log and not held against
  the series. On 2026-10-14 they failed build_clang, build_arm,
  build_s390 and build_w1 for every series. An error of modpost on
  the module is still counted.

## 0.1.0

- The `upstream` checker of beamfs-bench 0.17.0, its kernel.org tag
  reader, its manifest signing and its forbidden characters, moved
  here unchanged, so beamfs-bench and beamfs-xfstests run one checker
  at one version.
