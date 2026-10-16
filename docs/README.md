# Documentation

## Playing

- [Player guide](PLAYER.md): controls, audio, saves and touch layouts.
- [ROM matching](ROM-MATCHING.md): cartridge revisions and mismatch recovery.
- [Model catalog](MODELS.md): included agents, results and limitations.
- [Shared control](SHARED-CONTROL.md): sharing one controller with an agent.
- [Compatibility](COMPATIBILITY.md): supported hardware and recorded checks.

## Contributing

- [Setup and checks](../CONTRIBUTING.md)
- [Architecture](ARCHITECTURE.md)
- [Adding a game integration](ADDING-A-GAME.md)
- [Training and evaluation](TRAINING.md)
- [Contra integration](CONTRA-AGENTS.md), [Base 1 and Snow Bros.](AGENT-TASKS.md)
- [Hardware conformance](CONFORMANCE.md), [PPU](PPU.md), [APU](APU.md)
- [Cartridge hardware and regional limits](EXPANDED-HARDWARE.md)
- [Recording evaluation evidence](EVIDENCE.md)

## Maintaining a release

- [Release checklist](RELEASE.md)
- [Physical-device checks](DEVICE-ACCEPTANCE.md)
- [Security reporting](../SECURITY.md)

Task and observation contracts live in `integrations/`; cartridge metadata and
model selections live in `catalog/`. Each model card owns its performance claims.
