# Shared control

In **Players & controls**, choose a player, select **Shared**, and choose a compatible agent. The default is **AI handles B**: in Contra, you move,
aim and jump while the agent fires. In Snow Bros., B throws snow or launches a
snowball according to the game situation. Presets also cover A and A+B.

Each port has independent ownership for Left/Right, Up/Down, A and B. Direction
pairs cannot be split. The chosen source has exclusive control of each group;
human and neural buttons are not simply combined. Start and Select always belong
to the human. Title menus accept human input while the policy is inactive.

Changing ownership opens a paused session and releases held inputs. Closing the
dialog keeps it paused. **Take full control** removes the policy from that port,
returns it to Human and stays paused. Resume explicitly when ready.

The live strip shows **Human → AI proposal → Applied**. Human input is blue,
AI input purple. Dashed, faded keys remain visible when that source does not own
the group. Applied buttons come from the actual Rust frame decision. While paused,
the strip labels its last frame. Both ports appear on desktop; phones can switch
which port they inspect. AI-owned touch A/B buttons use the same purple color.

Networks still propose their full action. Existing 256-input models remain
compatible. New versioned 268-input assistance contracts append human buttons
and group ownership, using one Rust encoder shared by training and deployment.
The shipped models use autonomous observations. The assistance contract is an
API for future training; no assistance-trained candidate is included. Human
assistance requires its own evaluation.

## Runtime behavior

`game-integrations::control` defines ownership, opposite-direction normalization and
menu/transition/supported/unsupported phases. Native and WASM use the same agent
decisions. Unsupported gameplay stops before advancing another agent-controlled
frame; browser handoffs also run immediately after loading a save and before
Resume. An inference error pauses the frontend. A fatal CPU fault is latched in
`Machine` until reset or a successful valid-state restoration.

Shared settings and the live strip use the same Rust UI on native desktop and
in desktop/mobile browsers. Human Start/Select, disconnect pauses and unsupported
stage guards use the common session engine.

Training actions are neutral outside gameplay. Gym validates actions even during
those phases, and the Rust episode also enforces the gate. A complete 12,000-frame
Contra episode matched deployed WASM's final snapshot byte for byte. That episode
ended at its horizon, without clearing the stage; this validates parity only.

## Repeatable local checks

Build WASM and install the training package first. Supply your own ROMs:

```sh
node scripts/validate-shared-control.mjs /path/to/Contra.nes
node scripts/validate-agent-phases.mjs /path/to/rom-directory
node scripts/test-env-deployment.mjs /path/to/Contra.nes
node scripts/test-task-start-timing.mjs /path/to/game.nes TASK
```

The runtime tested-start contract reads a bounded 0–240 neutral-frame
warmup documented in the model manifest. Assigned agents must agree on it. It
requires restart confirmation and never loads a controller script or save state.

## Autonomous policies and assistance training

The recommended models observe game state, not the human's current button choices.
Shared mode assigns exclusive button ownership around their proposals. The separate
268-feature assistance contract adds human inputs and ownership. No trained candidate is bundled; real-human aiming, weapon changes, hesitation and recovery require
the [hands-on evaluation](DEVICE-ACCEPTANCE.md).
