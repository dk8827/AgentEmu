import { RESETS as resets } from "./task-contracts.mjs";
export function validateReplay(value, romHash) {

  if (
    !value ||
    value.version !== 1 ||
    !Object.hasOwn(resets, value.game) ||
    value.reset !== resets[value.game]
  )
    throw Error("Unsupported replay format");
  if (
    value.game === "contra-base1" &&
    !/^[a-f0-9]{64}$/.test(value.entry_state_sha256 ?? "")
  )
    throw Error("Base replay needs its local entry-state hash");
  if (value.rom_sha256 !== romHash)
    throw Error("Replay requires its exact game ROM");
  if (!Number.isInteger(value.warmup) || value.warmup < 0 || value.warmup > 240)
    throw Error("Invalid replay warmup");
  if (
    !Array.isArray(value.inputs) ||
    value.inputs.length < 1 ||
    value.inputs.length > 108000 ||
    value.inputs.some(
      (pair) =>
        !Array.isArray(pair) ||
        pair.length !== 2 ||
        pair.some((v) => !Number.isInteger(v) || v < 0 || v > 255),
    )
  )
    throw Error("Invalid controller trace");
  if (
    !/^[a-f0-9]{64}$/.test(value.result?.final_state_sha256 ?? "") ||
    !/^[a-f0-9]{64}$/.test(value.result?.input_sha256 ?? "")
  )
    throw Error("Replay is missing verification hashes");
  return value;
}
export async function digest(bytes) {
  return [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))]
    .map((v) => v.toString(16).padStart(2, "0"))
    .join("");
}
