# E2E Fixtures — vendored IWADs

Provenance record for the large binary fixtures the e2e layer (F9) and the
M5 soak plan consume. WAD files are stored via git-lfs (`.gitattributes`:
`doom1.wad` exact-name entry, `*.wad` pattern); they are never committed as
regular git blobs (AGENTS.md, constraint 1).

## `doom1.wad` (shareware IWAD, primary regression vector)

- Source: supplied by the maintainer (shareware `doom1.wad`, v1.9).
- Copyrighted id Software asset — present in the repo for the demo-exact
  regression suite only; never redistribute outside this private fork.
- Used by: all five scenario flows (`shells/web/scripts/scenarios/*.json`),
  the host twin (`cargo test -p room --test scenario_harness`), and upstream
  room's `demo_playthrough` test.

## `freedoom1.wad` / `freedoom2.wad` (Freedoom 0.13.0, M5 soak targets)

Vendored 2026-09-26 by Task 7 of the F9 e2e-infra plan (maintainer-approved
2026-09-25) as storage-only fixtures for the M5 soak job: they are not
consumed by any manifest or test yet; the M5 soak plan will reference them
as `target_iwad`.

- Release: **Freedoom v0.13.0**, published 2024-01-29.
- Release URL: <https://github.com/freedoom/freedoom/releases/tag/v0.13.0>
- Artifact: `freedoom-0.13.0.zip`
  (<https://github.com/freedoom/freedoom/releases/download/v0.13.0/freedoom-0.13.0.zip>)
- Archive integrity: SHA256 `3f9b264f3e3ce503b4fb7f6bdcb1f419d93c7b546f4df3e874dd878db9688f59`,
  matching the project-signed checksum manifest
  (`freedoom-0.13.0-CHECKSUM`, PGP-signed by the Freedoom release key).
- License verification: the archive ships `COPYING.txt`, which is the
  **BSD-3-Clause** license verbatim ("Copyright © 2001-2024, Contributors to
  the Freedoom project"; redistribution in source and binary forms
  permitted with the three standard conditions). Redistribution of the WADs
  is therefore allowed with the license notice preserved — this file is the
  repo's sole attribution pointer for the vendored Freedoom binaries; no
  other licensing doc in the repo carries the notice.
- Per-file SHA256 (as extracted from the verified archive):

  | file | size (bytes) | sha256 |
  |---|---|---|
  | `freedoom1.wad` | 28795076 | `7323bcc168c5a45ff10749b339960e98314740a734c30d4b9f3337001f9e703d` |
  | `freedoom2.wad` | 28787748 | `a8772e088847032510d97ba2312406a6998f21cbab44d4ff10696faa9c0ecd4b` |

- Fallback path (gitignore + CI fetch) was NOT needed: the license check
  passed on first inspection.
