# Original Contra-architecture test fixture

These weights are randomly initialized with PyTorch seed 912, not trained game agents. The 32 observation vectors are synthetic. `scripts/export-contra-fixtures.py` regenerates the files; native Rust, Python bindings and WASM compare the reference logits and decoded controller masks. No commercial ROM, game state or game assets are included.
