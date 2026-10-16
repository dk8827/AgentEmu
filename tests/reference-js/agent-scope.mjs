// Used after the Rust loader has validated the exact schema, ROM and architecture.
// The UI compares the task identity; schema revisions are not different games.
export function observationGame(schema) {
  return typeof schema === "string"
    ? (/^(.+)-(?:256|268-shared)-v[1-9]\d*$/.exec(schema)?.[1] ?? null)
    : null;
}

export function matchesAgentGame(schema, game) {
  return typeof game === "string" && observationGame(schema) === game;
}

export function observationScope(schema) {
  return (observationGame(schema) ?? String(schema)).replaceAll("-", " ");
}

// A model may document a tested timing, never a controller script or save state.
// Different assigned models must agree; otherwise retain the ordinary zero start.
export function testedStartTiming(manifests, game) {
  if (!manifests.length) return null;
  const timings = manifests.map((m) => {
    if (!matchesAgentGame(m?.observation, game)) return null;
    const value = m?.training?.start_warmup_frames ?? 0;
    return Number.isInteger(value) && value >= 0 && value <= 240 ? value : null;
  });
  return timings.every((v) => v !== null && v === timings[0])
    ? timings[0]
    : null;
}
