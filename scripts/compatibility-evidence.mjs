// Passing transport/determinism checks is not evidence of game completion.
import assert from 'node:assert/strict';
export function parityEvidence({frames, scripted, checks}) {
  assert.ok(Number.isInteger(frames) && frames > 0);
  for(const check of ['exact_revision','native_wasm_state','native_wasm_pcm','deterministic_restore'])
    assert.equal(checks[check],true,`missing evidence: ${check}`);
  return {version:1, frames, input:scripted?'game-specific-script':'generic-boot-input',
    checks, gameplay:'not-assessed',completion:'not-assessed',audiovisual_reference:'not-assessed'};
}
export function assertAcceptance(state, expectations) {
  assert.ok(expectations.length>0,'acceptance must contain assertions');
  for(const {path,equals} of expectations) {
    assert.ok(typeof path==='string' && path.length>0);
    let actual=state;
    for(const part of path.split('.')) {
      assert.ok(actual != null && Object.hasOwn(actual,part),`missing acceptance field ${path}`);
      actual=actual[part];
    }
    assert.deepEqual(actual,equals,`gameplay acceptance: ${path}`);
  }
}
