// Pointer ownership matters: lifting A+B must not release B held by another finger.
export class TouchState {
  pointers = new Map();
  set(id, mask) {
    if (mask) this.pointers.set(id, mask);
    else this.pointers.delete(id);
  }
  release(id) {
    this.pointers.delete(id);
  }
  clear() {
    this.pointers.clear();
  }
  get mask() {
    return normalize([...this.pointers.values()].reduce((a, b) => a | b, 0));
  }
}
export function normalize(mask) {
  if ((mask & 48) === 48) mask &= ~48;
  if ((mask & 192) === 192) mask &= ~192;
  return mask & 255;
}
export function directionAt(x, y, rect) {
  const dx = ((x - rect.left) / rect.width) * 2 - 1;
  const dy = ((y - rect.top) / rect.height) * 2 - 1;
  if (Math.abs(dx) > 1.15 || Math.abs(dy) > 1.15 || Math.hypot(dx, dy) < 0.22)
    return 0;
  const angle = Math.atan2(dy, dx),
    sector = (Math.round(angle / (Math.PI / 4)) + 8) % 8;
  return [128, 160, 32, 96, 64, 80, 16, 144][sector];
}
// Virtual menu taps are one frame, followed by a forced neutral frame between taps.
export class MenuPulses {
  queued = [0, 0];
  release = [false, false];
  push(slot, bits) {
    this.queued[slot] |= bits & 12;
  }
  take() {
    return this.queued.map((bits, slot) => {
      if (this.release[slot]) {
        this.release[slot] = false;
        return 0;
      }
      this.queued[slot] = 0;
      this.release[slot] = Boolean(bits);
      return bits;
    });
  }
  get pending() {
    return this.queued.some(Boolean) || this.release.some(Boolean);
  }
  clear() {
    this.queued.fill(0);
    this.release.fill(false);
  }
}

// Older builds allowed Escape to be bound as a game button; it now opens Pause.
export function migrateReservedKeys(saved, defaults) {
  const used = new Set(saved.flat().filter((code) => code !== "Escape"));
  return saved.map((row, p) =>
    row.map((code, i) => {
      if (code !== "Escape") return code;
      const replacement = [defaults[p][i], ...defaults.flat()].find(
        (key) => !used.has(key),
      );
      used.add(replacement);
      return replacement;
    }),
  );
}
