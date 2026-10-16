# Contributor instructions

- Read CONTRIBUTING.md for setup and named checks. Use project-local Python environments.
- Keep emulation in nes-core, game semantics in game-integrations, frame execution
  in session-engine, and presentation in player-ui. Native and browser hosts share rules.
- Edit integrations/ and catalog/ sources, then regenerate their derived files.
  Never bypass ROM, schema or model validation by changing hashes.
- Add regression coverage for functional changes. Run the relevant named checks;
  report checks that require unavailable hardware or commercial ROMs explicitly.
- Keep ROMs, save states, credentials, local environments and generated builds out of Git.
- Preserve unrelated changes. Never force-push. Follow the human contributor's
  explicit instructions for commits, pushes and publication.
- Allow long development/training processes to finish. Poll in proportion to their
  expected duration, and preserve checkpoints and reports.
