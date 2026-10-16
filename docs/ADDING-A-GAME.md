# Add a game integration

Start with a cartridge the emulator already runs. Mapper support, game-state
observations and a useful neural policy are separate milestones.

1. Record the exact file SHA-256, region/revision and input devices in
   `catalog/games.json`. Add a local compatibility trace and verify independent
   player input. Keep ROMs and save states outside Git.
2. Implement a decoder in `crates/game-integrations/src/`. Use immutable peeks;
   validate positions, health, deaths and completion with controlled experiments.
   Record uncertain fields as codes rather than inventing semantic names.
3. Define menu, transition, supported gameplay and unsupported gameplay. Test
   title/attract mode, pause, death, completion and state restoration. A cleared
   enemy list alone is usually insufficient evidence of stage completion.
4. Add `integrations/GAME/task.json` and `observation.json`, using an existing task
   as the format reference. Pin the architecture, reset, feature ordering,
   normalization, reserved zeros and action descriptions. Generate contracts with
   `python3 scripts/sync-integrations.py`.
5. Implement ordinary-input reset and episode reward/termination in the task
   integration. Rewards need progress high-water marks to prevent farming.
   Keep time limits as truncations. Test both ports and invalid actions.
6. Add training data collection/teacher logic in Python. Teachers may use only the
   declared observation when that is the training claim. Search demonstrations
   must replay from reset without intermediate restores before acceptance.
7. Freeze selection and held-out evaluation cases before tuning. Compare neural,
   scripted, random, idle and mixed-port profiles. Report every failure, survival,
   deaths and frame budget. Do not repeatedly tune against the held-out set.
8. Export safetensors through the validated exporter; require logit/action parity
   and complete native/WASM episode parity. Document commands and provenance in
   the model card. Add weights only after the evaluation is recorded.
9. Register the bundle in `catalog/models.json`, set its status and notes, then run
   `python3 scripts/sync-catalog.py`. Verify gallery setup, both player slots,
   Shared ownership and unsupported-stage handoff in the current Rust UI.

Use a new observation ID when changing feature meanings. Architecture compatibility
alone does not make weights interchangeable between games. Human assistance needs
its own evaluation; autonomous stage clears do not establish Shared-play quality.
