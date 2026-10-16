// Battery-backed cartridge RAM only, keyed by the exact original ROM digest.
// Save states remain explicit downloadable files.
const key = (hash) => `nes-battery-v1:${hash}`;
export function readBattery(storage, hash) {
  const raw = storage.getItem(key(hash));
  if (raw === null) return null;
  const values = JSON.parse(raw);
  if (
    !Array.isArray(values) ||
    ![1024, 8192].includes(values.length) ||
    values.some((v) => !Number.isInteger(v) || v < 0 || v > 255)
  )
    throw Error(
      "Invalid battery save. Your downloaded save files are unaffected.",
    );
  return new Uint8Array(values);
}
export function writeBattery(storage, hash, bytes) {
  if (![1024, 8192].includes(bytes.length))
    throw Error("Unexpected battery RAM size");
  storage.setItem(key(hash), JSON.stringify([...bytes]));
}
