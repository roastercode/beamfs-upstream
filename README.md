# beamfs-upstream

What a kernel patch series has to pass before it is mailed, as a
library beamfs-bench and beamfs-xfstests both link. 0.1.0 is the
checker of beamfs-bench 0.17.0 moved as it was; what it checks and
what it still lacks is described in beamfs-bench's README, "Before a
patch series is mailed", until this one takes it over.

Since 0.2.0 a build check counts only the diagnostics that name a
file the series changes or a path under fs/<fs>/; what the kernel's
own headers say is counted in the build log, not against the series.

Since 0.3.0 the series is derived, not kept: `upstream/series.toml` in
the filesystem repository says what each patch carries, and `series`
builds the branch from a commit of that repository on the newest
mainline tag, then the checks run on it. `beamfs-bench upstream --plan
<repo>/upstream/series.toml` does both.

Since 0.4.0 the checker ends the way an envoy would start: the mails
are applied with git am on the base and must give the branch back,
and git send-email --dry-run sends them nowhere, to the recipients
get_maintainer names; `send.sh` beside the patches is the real
command. fs/ is built at every commit, as the build robots do.

## Justified exceptions to `checkpatch --strict`

`--strict` blocks when a plan drives the run. A CHECK that mainline's
own code raises in the same form, such as the `TRACE_EVENT` layout or a
`BUFFER_FNS` name, is listed in the plan as a `[[series.strict_exception]]`
table with its file, its types and the mainline evidence. The run notes
each exception the findings use and fails on one nothing uses.

## Word sizes and byte orders

Besides the x86_64 and arm64 builds, the filesystem is built as a module
on i386, arm (LLVM), s390 (LLVM, big-endian) and hexagon (LLVM,
`comet_defconfig`), and the W=1 pass runs on each. Hexagon is there for
one reason: it has no divide instruction, so a 64-bit division that x86
and arm do inline, or through their own `__div64_32`, becomes a call to
`__hexagon_udivdi3`, which the kernel does not provide, and modpost
refuses the module. `div_u64()` and `do_div()` are the way to divide a
u64 in a module; the i386 and arm builds alone do not say so.

The MAINTAINERS block of the plan is placed by alphabetical order of its
title, case aside, between two sections that are themselves in order: the
kernel's file has some 130 titles that sort before the one above them,
and a block placed against one of those lands anywhere. The `maintainers`
check reads the built tree back and refuses an entry out of order.

## A base behind the newest tag

A series is based on the newest tag kernel.org lists, or on the branch of
the tree it is meant for, which has to hold that tag. When it does not and
the series is to be mailed anyway, the plan says why: `base = { url, branch,
behind = "<reason>" }`. The letter's `@@BASE@@` then names the tag the base
does hold, and the base check passes saying so, the reason in the summary.
