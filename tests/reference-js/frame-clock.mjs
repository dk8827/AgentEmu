// Retain elapsed time while a worker is busy. Bound recovery after long stalls.
export class FrameClock {
  constructor() {
    this.rate = 60.098813897;
    this.reset(0);
  }
  reset(now) {
    this.last = now;
    this.debt = 0;
  }
  tick(now, running, audioDriven) {
    const elapsed = Math.max(0, Math.min(100, now - this.last));
    this.last = now;
    if (running && !audioDriven)
      this.debt = Math.min(8, this.debt + (elapsed * this.rate) / 1000);
    else this.debt = 0;
  }
  take(limit = 4) {
    const count = Math.min(limit, Math.floor(this.debt));
    this.debt -= count;
    return count;
  }
}
