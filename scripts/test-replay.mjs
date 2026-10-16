import { test } from "node:test";
import assert from "node:assert/strict";
import { validateReplay, digest } from "../tests/reference-js/replay.mjs";
const hash = "a".repeat(64);
const trace = {
  version: 1,
  game: "contra-jungle",
  reset: "contra-two-player-v1",
  rom_sha256: hash,
  warmup: 223,
  inputs: [
    [0, 0],
    [128, 2],
  ],
  result: { input_sha256: hash, final_state_sha256: hash },
};
test("replays enforce exact ROM, bounds and verification metadata", () => {
  assert.equal(validateReplay(trace, hash), trace);
  for (const change of [
    { warmup: 241 },
    { warmup: -1 },
    { inputs: [[256, 0]] },
    { inputs: [[0]] },
    { inputs: [[1.5, 0]] },
    { inputs: [] },
    { game: "unknown" },
    { result: {} },
    { version: 2 },
  ])
    assert.throws(() => validateReplay({ ...trace, ...change }, hash));
  assert.throws(() => validateReplay(trace, "b".repeat(64)));
});
test("replay SHA256 hashes bytes consistently", async () =>
  assert.equal(
    await digest(new Uint8Array()),
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  ));

test("TMNT replay requires its normal two-player reset contract", () => {
  const candidate = {
    ...trace,
    game: "tmnt2-scene1",
    reset: "tmnt2-two-player-v1",
  };
  assert.equal(validateReplay(candidate, hash), candidate);
  assert.throws(() =>
    validateReplay({ ...candidate, reset: "contra-two-player-v1" }, hash),
  );
});
