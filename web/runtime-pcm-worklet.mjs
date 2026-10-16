// Small bounded audio-device adapter. Receives PCM directly from the emulation
// worker; UI paints and SharedArrayBuffer availability are irrelevant.
class PcmSink extends AudioWorkletProcessor {
  constructor() {
    super();
    this.buffer = new Float32Array(8192);
    this.read = 0;
    this.write = 0;
    this.phase = 0;
    this.samples = 0;
    this.underruns = 0;
    this.overruns = 0;
    this.blocks = 0;
    this.source = null;
    this.epoch = 0;
    this.serial = 0;
    this.started = false;
    this.port.onmessage = ({ data: m }) => {
      if (m.clear) {
        this.read = this.write;
        this.phase = 0;
        this.started = false;
      }
      if (m.stop) {
        this.epoch = -1;
        this.read = this.write;
        this.phase = 0;
        this.started = false;
      }
      if (m.source) {
        this.source?.close();
        this.source = m.source;
        this.epoch = m.epoch;
        this.read = this.write;
        this.phase = 0;
        this.started = false;
        this.source.onmessage = ({ data: v }) => {
          if (v.epoch !== this.epoch) return;
          if (v.stop) {
            this.read = this.write;
            this.phase = 0;
            this.started = false;
            this.epoch = -1;
            return;
          }
          if (!(v.samples instanceof Float32Array)) return;
          this.serial = v.serial;
          for (const sample of v.samples) {
            if (this.write - this.read >= this.buffer.length) {
              this.overruns++;
              break;
            }
            this.buffer[this.write++ % this.buffer.length] = sample;
            this.samples++;
          }
        };
        this.source.start();
      }
    };
  }
  process(_inputs, outputs) {
    const channels = outputs[0];
    if (!channels?.length) return true;
    const ratio = 48000 / sampleRate;
    if (!this.started && this.write - this.read >= 1600) this.started = true;
    for (let i = 0; i < channels[0].length; i++) {
      let value = 0;
      if (
        this.started &&
        this.write - this.read >= Math.max(2, Math.floor(this.phase + ratio))
      ) {
        const a = this.buffer[this.read % 8192],
          b = this.buffer[(this.read + 1) % 8192];
        value = a + (b - a) * this.phase;
        this.phase += ratio;
        const used = Math.floor(this.phase);
        this.phase -= used;
        this.read += used;
      } else if (this.started) {
        this.underruns++;
        this.started = false;
      }
      for (const channel of channels) channel[i] = value;
    }
    if (++this.blocks % 8 === 0) {
      const m = {
        serial: this.serial,
        queued: this.write - this.read,
        samples: this.samples,
        underruns: this.underruns,
        overruns: this.overruns,
      };
      this.port.postMessage(m);
      this.source?.postMessage(m);
    }
    return true;
  }
}
registerProcessor("nes-runtime-pcm", PcmSink);
