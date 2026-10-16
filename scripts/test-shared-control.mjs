import test from "node:test";
import assert from "node:assert/strict";
import {
  GROUPS,
  PRESETS,
  actionNames,
  ownershipDescription,
  needsHandoff,
} from "../tests/reference-js/shared-control.mjs";
test("presets match the network action groups and do not own menu buttons", () => {
  assert.deepEqual(
    PRESETS.map((p) => p.mask),
    [2, 1, 3],
  );
  assert.deepEqual(
    GROUPS.map((g) => g.mask),
    [192, 48, 1, 2],
  );
});
test("descriptions respect game semantics", () => {
  assert.deepEqual(actionNames("contra-jungle"), ["Jump", "Fire"]);
  assert.match(actionNames("snowbros-floor1")[1], /launch snowball/);
  assert.match(ownershipDescription(2, "contra-jungle"), /fire/);
});
test("both neural modes require handoff independently of adapter gameplay flags", () => {
  for (const mode of ["agent", "shared"]) {
    assert.equal(needsHandoff(mode, "unsupported-gameplay"), true);
    for (const phase of ["menu", "transition", "supported-gameplay"])
      assert.equal(needsHandoff(mode, phase), false);
  }
  assert.equal(needsHandoff("human", "unsupported-gameplay"), false);
});
