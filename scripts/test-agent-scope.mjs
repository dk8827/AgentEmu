import test from "node:test";
import assert from "node:assert/strict";
import {
  matchesAgentGame,
  observationScope,
  testedStartTiming,
} from "../tests/reference-js/agent-scope.mjs";

test("UI task matching accepts a validated revised schema", () => {
  assert.equal(
    matchesAgentGame("doubledragon2-mission1-256-v2", "doubledragon2-mission1"),
    true,
  );
  assert.equal(matchesAgentGame("contra-jungle-256-v1", "contra-jungle"), true);
  assert.equal(matchesAgentGame("contra-base1-256-v1", "contra-jungle"), false);
  assert.equal(
    observationScope("doubledragon2-mission1-256-v2"),
    "doubledragon2 mission1",
  );
});

test("malformed or absent schemas never match a game", () => {
  for (const schema of [
    null,
    undefined,
    "contra-jungle",
    "contra-jungle-256-v0",
    "contra-jungle-256-v1-extra",
    "contra-jungle-128-v1",
  ])
    assert.equal(matchesAgentGame(schema, "contra-jungle"), false);
  assert.equal(matchesAgentGame(null, null), false);
});

test("tested starts require bounded timing and agreement from assigned models", () => {
  const model = (value) => ({
    observation: "rivercity-moose-256-v1",
    training: { start_warmup_frames: value },
  });
  assert.equal(
    testedStartTiming([model(40), model(40)], "rivercity-moose"),
    40,
  );
  assert.equal(testedStartTiming([model(0)], "rivercity-moose"), 0);
  assert.equal(testedStartTiming([model(240)], "rivercity-moose"), 240);
  assert.equal(
    testedStartTiming([model(40), model(0)], "rivercity-moose"),
    null,
  );
  assert.equal(testedStartTiming([], "rivercity-moose"), null);
  assert.equal(testedStartTiming([model(40)], "worldcup-firstmatch"), null);
  for (const value of [-1, 241, 1.5, "40", [], {}, NaN, Infinity])
    assert.equal(testedStartTiming([model(value)], "rivercity-moose"), null);
});
