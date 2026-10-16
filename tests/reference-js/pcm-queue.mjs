// Bounded mono PCM ring with fractional linear resampling from core's 48 kHz.
export class PcmQueue {
  constructor(outputRate) {
    this.buffer = new Float32Array(48000);
    this.ratio = 48000 / outputRate;
    this.reset();
  }
  reset() {
    this.read = 0;
    this.write = 0;
    this.size = 0;
    this.phase = 0;
    this.consumed = 0;
    this.dropped = 0;
    this.underruns = 0;
  }
  push(samples) {
    for (const sample of samples) {
      if (this.size === this.buffer.length) {
        this.read = (this.read + 1) % this.buffer.length;
        this.size--;
        this.consumed++;
        this.dropped++;
      }
      this.buffer[this.write] = Number.isFinite(sample) ? sample : 0;
      this.write = (this.write + 1) % this.buffer.length;
      this.size++;
    }
  }
  render(output) {
    for (let i = 0; i < output.length; i++) {
      const advance = Math.floor(this.phase + this.ratio);
      if (this.size < Math.max(2, advance)) {
        output[i] = 0;
        this.underruns++;
        continue;
      }
      const a = this.buffer[this.read],
        b = this.buffer[(this.read + 1) % this.buffer.length];
      output[i] = a + (b - a) * this.phase;
      this.phase += this.ratio;
      const used = Math.floor(this.phase);
      this.phase -= used;
      this.read = (this.read + used) % this.buffer.length;
      this.size -= used;
      this.consumed += used;
    }
  }
}
