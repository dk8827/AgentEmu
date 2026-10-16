// Platform transport only: all game rules and frame accounting execute in Rust.
import init, { SessionRuntime } from "./pkg/emulator.js";
let runtime,
  timer,
  audioPort,
  queued = -1,
  epoch = 0,
  inflight = false,
  stopping = false,
  audioSent = 0,
  audioAck = 0,
  lastAudioStats = 0;
const ready = init();
const send = (kind, extra = {}) => {
  const pixels = runtime.pixels(),
    battery = runtime.battery();
  const message = { kind, view: runtime.view(), pixels, battery, ...extra };
  const transfer = [pixels.buffer, battery.buffer];
  if (message.state) transfer.push(message.state.buffer);
  postMessage(message, transfer);
};
function stop(error) {
  if (stopping) return;
  stopping = true;
  clearTimeout(timer);
  if (!runtime) return;
  runtime.pause();
  send("stopped", {
    state: runtime.snapshot(),
    ...(error
      ? {
          view: JSON.stringify({
            ...JSON.parse(runtime.view()),
            error: String(error),
            finished: true,
          }),
        }
      : {}),
  });
  audioPort?.postMessage({ stop: true, epoch });
  audioPort?.close();
  runtime.free();
  runtime = null;
}
function tick() {
  try {
    if (
      runtime.update(
        performance.now() / 1000,
        performance.now() - lastAudioStats < 150 ? queued : -1,
      )
    ) {
      const samples = runtime.audio();
      if (audioPort && samples.length && audioSent - audioAck < 4)
        audioPort.postMessage({ samples, epoch, serial: ++audioSent }, [
          samples.buffer,
        ]);
      const view = JSON.parse(runtime.view());
      // At most one presentation packet is in flight. Audio has its own channel.
      if (!inflight) {
        send("frame");
        inflight = true;
      }
      if (view.finished) {
        stop();
        return;
      }
    }
    timer = setTimeout(tick, 2);
  } catch (e) {
    if (runtime) stop(e);
    else postMessage({ kind: "error", error: String(e) });
  }
}
onmessage = async ({ data }) => {
  try {
    if (data.kind === "init") {
      await ready;
      if (stopping) {
        postMessage({ kind: "canceled" });
        return;
      }
      runtime = new SessionRuntime(data.rom, data.state, data.setup);
      tick();
    } else if (data.kind === "input") {
      runtime?.input(data.input);
      postMessage({ kind: "input-ack" });
    } else if (data.kind === "ack") {
      inflight = false;
    } else if (data.kind === "stop") {
      stop();
    } else if (data.kind === "audio") {
      audioPort = data.port;
      epoch = data.epoch;
      audioPort.onmessage = ({ data: m }) => {
        queued = Number.isFinite(m.queued) ? m.queued : -1;
        audioAck = m.serial ?? audioAck;
        lastAudioStats = performance.now();
      };
      audioPort.start();
    }
  } catch (e) {
    if (runtime) stop(e);
    else postMessage({ kind: "error", error: String(e) });
  }
};
