// Browser host only. Layout, controls, dialogs and gameplay live in Rust.
import { attachAccessibility } from "./accessibility.mjs";
import init, { WebPlayer } from "./pkg/player_ui.js";
let canvas = document.getElementById("player");
Object.assign(document.documentElement.style, {
  width: "100%",
  height: "100%",
  overflow: "hidden",
});
Object.assign(document.body.style, {
  margin: "0",
  width: "100%",
  height: "100%",
  background: "#f7f5ef",
});
Object.assign(document.body.style, {
  boxSizing: "border-box",
  padding:
    "env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left)",
});
Object.assign(canvas.style, {
  display: "block",
  width: "100%",
  height: "100%",
  touchAction: "none",
});
try {
  await init();
  let player = new WebPlayer();
  const webgl =
    new URLSearchParams(location.search).get("renderer") === "webgl";
  try {
    await player.start(canvas, webgl);
  } catch (error) {
    if (webgl) throw error;
    player.destroy();
    // A canvas cannot switch context types after a partially initialized
    // WebGPU attempt. Keep the same host attributes on a fresh canvas.
    const fallback = canvas.cloneNode(false);
    canvas.replaceWith(fallback);
    canvas = fallback;
    player = new WebPlayer();
    await player.start(canvas, true);
  }
  // Read-only diagnostics are useful for support and acceptance tests.
  window.nesPlayer = player;
  attachAccessibility(player, canvas);
  document.getElementById("loading").remove();
} catch (error) {
  document.getElementById("loading").textContent =
    `Could not start the player: ${error}. Reload to try again.`;
  console.error(error);
}
