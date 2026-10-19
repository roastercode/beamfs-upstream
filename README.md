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
