import test from 'node:test';
import assert from 'node:assert/strict';
import {parityEvidence,assertAcceptance} from './compatibility-evidence.mjs';
test('backend agreement never certifies gameplay, completion or independent AV accuracy',()=>{
  const checks={exact_revision:true,native_wasm_state:true,native_wasm_pcm:true,deterministic_restore:true};
  for(const scripted of [false,true]){
    const evidence=parityEvidence({frames:3600,scripted,checks});
    assert.equal(evidence.gameplay,'not-assessed');assert.equal(evidence.completion,'not-assessed');
    assert.equal(evidence.audiovisual_reference,'not-assessed');
  }
  assert.throws(()=>parityEvidence({frames:3600,scripted:true,checks:{...checks,native_wasm_pcm:false}}));
});
test('gameplay milestones require explicit measured fields, not a success label in a recording',()=>{
  const expectations=[{path:'level',equals:1},{path:'players.0.active',equals:true}];
  assertAcceptance({level:1,players:[{active:true}]},expectations);
  assert.throws(()=>assertAcceptance({success:true,level:0,players:[{active:true}]},expectations));
  assert.throws(()=>assertAcceptance({level:1,players:[{}]},expectations));
  assert.throws(()=>assertAcceptance({},[]));
});
