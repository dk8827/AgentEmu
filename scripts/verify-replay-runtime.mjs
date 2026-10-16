// Exact local replay verification through the browser's headless Rust runtime.
import {readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {assertAcceptance} from './compatibility-evidence.mjs';
import assert from 'node:assert/strict';
import init,{NesMachine,SessionRuntime} from '../web/pkg/emulator.js';
const [romPath,replayPath,acceptancePath]=process.argv.slice(2);
if(!romPath || !replayPath)throw Error('usage: node scripts/verify-replay-runtime.mjs ROM REPLAY');
await init({module_or_path:await readFile('web/pkg/emulator_bg.wasm')});
const rom=await readFile(romPath),replay=JSON.parse(await readFile(replayPath,'utf8'));
const start=new NesMachine(rom);
try {
  if(replay.game==='contra-jungle') {
    start.start_contra();for(let i=0;i<replay.warmup;i++)start.frame();start.audio_samples();
  } else start.start_task_with_warmup(replay.game,replay.warmup);
  const saved=start.snapshot(),rate=start.frame_rate();
  function run(r) {
    const setup={name:'Replay verification',models:[null,null],modes:['Human','Human'],ownership:[2,2],alternate:false,replay:[r,0]};
    const runtime=new SessionRuntime(rom,saved,JSON.stringify(setup));
    try {
      for(let i=0;i<r.inputs.length+10;i++) {
        runtime.update(i/rate,-1);runtime.audio();
        const view=JSON.parse(runtime.view());if(view.finished)return {...view,snapshot:runtime.snapshot()};
      }
      throw Error('runtime did not finish');
    }finally{runtime.free();}
  }
  const result=run(replay);assert.equal(result.error,null);assert.equal(result.replay_index,replay.inputs.length);
  if(replay.version===2){
    const bad=structuredClone(replay);bad.checkpoints[0].state_sha256='1'.repeat(64);
    // Do not invalidate final metadata if the trace only has one checkpoint.
    if(bad.checkpoints.length>1){const stopped=run(bad);assert.match(stopped.error,/Replay diverged at frame/);assert.equal(stopped.replay_index,bad.checkpoints[0].frame);}
  }
  if(acceptancePath) {
    const spec=JSON.parse(await readFile(acceptancePath,'utf8'));
    assert.equal(spec.game,replay.game);assert.equal(spec.rom_sha256,replay.rom_sha256);
    // The acceptance recipe is tied to exact inputs, independent of replay envelope version.
    assert.equal(spec.input_sha256,createHash('sha256').update(Buffer.from(replay.inputs.flat())).digest('hex'));
    start.restore(result.snapshot);
    const terminal=JSON.parse(replay.game==='contra-jungle'?start.contra_state():start.task_state(replay.game));
    assertAcceptance(terminal,spec.expectations);
    await writeFile('.cache/game-acceptance.json',JSON.stringify({version:1,game:spec.game,rom_sha256:replay.rom_sha256,
      input_sha256:spec.input_sha256,frames:result.replay_index,status:'passed',
      evidence:'game-adapter-milestone',independent_audiovisual_reference:'not-assessed',expectations:spec.expectations},null,2)+'\n');
    console.log(`Gameplay acceptance passed: ${spec.description}`);
  }
  console.log(`Verified ${result.replay_index} replay frames and early divergence detection in WASM runtime`);
}finally{start.free();}
