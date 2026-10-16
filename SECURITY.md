# Security policy

Report vulnerabilities privately through [GitHub security advisories](https://github.com/dk8827/AgentEmu/security/advisories/new).
Include the affected revision, platform, reproduction steps and expected impact.
Do not put credentials, personal files or commercial ROMs in a report. If a ROM is
needed to reproduce an issue, provide its hash and a description first.

The current main branch is maintained. This is an early project without a response
SLA or supported older release branches. Ordinary bugs belong in GitHub Issues;
security vulnerabilities should stay private until a fix is available.

The player reads local ROMs and model tensors. It does not upload ROMs or execute
Python model files. Size, schema and hash checks reduce risk but are not a security
sandbox. Training tools are intended for trusted local inputs; do not load unknown
optimizer checkpoints or Python scripts.
