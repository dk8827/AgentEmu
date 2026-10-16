# Releasing AgentEmu

## Source release

1. Run the named checks in [CONTRIBUTING.md](../CONTRIBUTING.md) from a fresh
   checkout, including a fresh Python training install and packaged browser tests.
2. Review every tracked file against `release-files.json`. It is an explicit
   publication allowlist, not a list to regenerate blindly. Add a path only after
   reviewing its purpose, provenance and contents. The release check rejects
   unreviewed files and missing approved files.
3. Run `python3 scripts/check.py release` and a secret scan on a clean source
   export. Keep ROMs, saves, datasets, study logs, candidate checkpoints, game
   captures, credentials and personal paths out of the release.
4. Verify model hashes, licenses and cards. Preserve measured failures and
   distinguish solo, cooperative and Shared-control results. Do not silently
   substitute newly trained candidates for reviewed bundled models.
5. Regenerate [dependency notices](../THIRD_PARTY_NOTICES.md) when dependencies
   change. Review artwork and third-party test licenses separately from code.
6. Configure private vulnerability reporting, dependency alerts and branch
   protection. GitHub Actions is disabled; workflows are manual-only if enabled
   later. Local checks are required before publication.

## Installers and device claims

`python3 scripts/package-web.py` creates `dist/web/` after building WASM.
`scripts/package-native.py` packages a previously built native executable.
Neither publishes a site or release. Inspect package contents and notices first.

Complete [physical-device acceptance](DEVICE-ACCEPTANCE.md) before claiming
phone, gamepad, screen-reader or target-OS installer support. Browser viewport
tests do not replace real touch hardware, speakers or accessibility testing.
Document signing and notarization before distributing verified installers.
