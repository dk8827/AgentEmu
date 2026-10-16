export const GROUPS = [
  { name: "Left / Right", mask: 192 },
  { name: "Up / Down", mask: 48 },
  { name: "A", mask: 1 },
  { name: "B", mask: 2 },
];
export const PRESETS = [
  { name: "AI handles B", mask: 2 },
  { name: "AI handles A", mask: 1 },
  { name: "AI handles A+B", mask: 3 },
];
export const usesAI = (mode) => mode === "agent" || mode === "shared";
export function actionNames(game) {
  if (game?.startsWith("contra-")) return ["Jump", "Fire"];
  return (
    {
      "snowbros-floor1": ["Jump", "Throw snow / launch snowball"],
      "bubblebobble-round1": ["Jump", "Blow bubbles"],
      "iceclimber-mountain1": ["Jump", "Hammer"],
      "rivercity-moose": ["Punch", "Kick"],
      "tmnt2-scene1": ["Jump", "Attack"],
      "doubledragon2-mission1": ["Right attack", "Left attack"],
      "worldcup-firstmatch": ["Pass / tackle", "Shoot / slide"],
      "chipndale-zone0": ["Jump", "Pick up / throw"],
      "jackal-stage1": ["Grenade", "Machine gun"],
      "lifeforce-stage1": ["Upgrade", "Fire"],
    }[game] ?? ["A button", "B button"]
  );
}
export function ownershipDescription(mask, game) {
  const [a, b] = actionNames(game);
  if (game?.startsWith("contra-")) {
    if (mask === 2) return "You move and aim. The AI fires.";
    if (mask === 1) return "You move, aim and fire. The AI jumps.";
    if (mask === 3) return "You move and aim. The AI jumps and fires.";
  }
  const actions = [
    mask & 192 ? "left / right" : null,
    mask & 48 ? "up / down" : null,
    mask & 1 ? a.toLowerCase() : null,
    mask & 2 ? b.toLowerCase() : null,
  ].filter(Boolean);
  return actions.length
    ? `AI: ${actions.join(" · ")}. You control the other groups.`
    : "You control every gameplay group.";
}
export function needsHandoff(mode, phase) {
  return usesAI(mode) && phase === "unsupported-gameplay";
}
export const TRACE_KEYS = [
  [64, "←"],
  [16, "↑"],
  [32, "↓"],
  [128, "→"],
  [1, "A"],
  [2, "B"],
  [4, "Select"],
  [8, "Start"],
];
