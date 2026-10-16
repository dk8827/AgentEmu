import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import vm from 'node:vm';
const code=await readFile(new URL('../web/runtime-pcm-worklet.mjs',import.meta.url),'utf8');
function make(rate=48000) {
  let Sink;
  class Port { postMessage(m){this.last=m;}start(){}close(){this.closed=true;} }
  vm.runInNewContext(code,{AudioWorkletProcessor:class{constructor(){this.port=new Port();}},
    Float32Array,sampleRate:rate,registerProcessor:(_name,type)=>Sink=type});
  const sink=new Sink(),source=new Port();
  sink.port.onmessage({data:{source,epoch:3}});
  return {sink,source,push:(n,value=.25,epoch=3)=>source.onmessage({data:{samples:new Float32Array(n).fill(value),epoch,serial:1}}),
    render:()=>{const out=[new Float32Array(128),new Float32Array(128)];sink.process([], [out]);return out;}};
}
test('audio prebuffers, resamples at device rate and keeps both channels identical',()=>{
  for(const rate of [44100,48000,96000]) {
    const q=make(rate);q.push(800);assert.ok(q.render()[0].every(x=>x===0));
    q.push(2400);
    for(let i=0;i<8;i++){const [a,b]=q.render();assert.ok(a.every(x=>x===.25));assert.deepEqual(a,b);}
    assert.ok(q.sink.write-q.sink.read<=8192);
    assert.ok(q.source.last.queued<=8192);
  }
});
test('bounded audio rejects packets from an old session and stops immediately',()=>{
  const q=make();q.push(10000);assert.equal(q.sink.write-q.sink.read,8192);
  assert.equal(q.sink.overruns,1);q.render();
  q.sink.port.onmessage({data:{stop:true}});q.push(2000);
  assert.ok(q.render()[0].every(x=>x===0));
  q.sink.port.onmessage({data:{source:q.source,epoch:4}});
  q.push(2000,.5,3);assert.ok(q.render()[0].every(x=>x===0));
  q.push(2000,.5,4);assert.ok(q.render()[0].every(x=>x===.5));
});
