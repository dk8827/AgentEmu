// Real canvas input; diagnostics are read-only. Uses original homebrew only.
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawn, execFileSync } from "node:child_process";
const require = createRequire(new URL("../web/package.json", import.meta.url));
const { chromium } = require("playwright");
// Generate an original homebrew fixture only for tests; the product has no demo.
const fixtureDir = await mkdtemp(join(tmpdir(), "agentemu-ui-"));
const fixture = join(fixtureDir, "Input test.nes");
execFileSync(
  "sh",
  [
    "-c",
    '. scripts/env.sh && cargo run --locked --release -p nes-core --example ui_fixture -- "$1"',
    "sh",
    fixture,
  ],
  { stdio: "inherit" },
);
const port = 8879;
const packaged = process.env.AGENTEMU_PACKAGED === "1";
const prefix = packaged ? "/AgentEmu" : "";
const server = spawn(
  "python3",
  [
    packaged ? "scripts/serve-package.py" : "scripts/serve-web.py",
    "--port",
    String(port),
  ],
  { stdio: "ignore", cwd: new URL("..", import.meta.url).pathname },
);
const browser = await chromium.launch({
  headless: true,
  ...(process.env.PLAYWRIGHT_CHROME ? { channel: "chrome" } : {}),
  args: ["--enable-unsafe-swiftshader"],
});
const output = new URL("../web/test-results/rust-ui/", import.meta.url)
  .pathname;
await mkdir(output, { recursive: true });
const errors = [];
const state = (page) =>
  page.evaluate(() => JSON.parse(window.nesPlayer.diagnostics()));
// Wait for actual Rust UI updates, not just browser RAFs. Under software GPU
// rendering a requestAnimationFrame can precede the next egui interaction pass.
const paint = async (page) => {
  const frame = (await state(page)).ui_frames;
  await page.waitForFunction(
    (n) => JSON.parse(window.nesPlayer.diagnostics()).ui_frames >= n + 2,
    frame,
  );
  await page.evaluate(
    () =>
      new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(resolve)),
      ),
  );
};
async function click(page, x, y) {
  await page.mouse.move(x, y);
  await paint(page);
  await page.mouse.down();
  await paint(page);
  await page.mouse.up();
  await paint(page);
}
async function open(context, query = "") {
  const page = await context.newPage();
  await page.addInitScript(() => {
    const connect = AudioNode.prototype.connect;
    AudioNode.prototype.connect = function (destination, ...args) {
      const result = connect.call(this, destination, ...args);
      if (
        this instanceof GainNode &&
        destination instanceof AudioDestinationNode
      ) {
        const analyser = this.context.createAnalyser();
        analyser.fftSize = 2048;
        connect.call(this, analyser);
        window.__nesAudioAnalyser = analyser;
      }
      return result;
    };
  });
  if (process.env.UI_CPU_THROTTLE) {
    const cdp = await context.newCDPSession(page);
    await cdp.send("Emulation.setCPUThrottlingRate", {
      rate: Number(process.env.UI_CPU_THROTTLE),
    });
  }
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(`http://127.0.0.1:${port}${prefix}/play.html${query}`);
  await page.waitForFunction(() =>
    window.nesPlayer?.diagnostics().includes("rust-egui"),
  );
  await paint(page);
  await page.evaluate(() => {
    window.__uiActions = [];
    const send = nesPlayer.accessibility_action.bind(nesPlayer);
    nesPlayer.accessibility_action = (...args) => {
      const accepted = send(...args);
      __uiActions.push({
        args,
        accepted,
        dialog: JSON.parse(nesPlayer.diagnostics()).dialog,
      });
      return accepted;
    };
  });
  return page;
}
// Exercise the semantic controls emitted by the actual Rust widgets. Pointer and
// multi-touch gameplay still use real canvas events, never a test-only setter.
async function act(page, name) {
  const control = page.getByRole("button", { name, exact: true });
  await control.waitFor();
  await page.waitForFunction(
    (id) => {
      const el = document.getElementById(id);
      return el && !el.disabled;
    },
    await control.getAttribute("id"),
  );
  await control.focus();
  await paint(page);
  await control.press("Enter");
  await paint(page);
  await page.waitForTimeout(180);
  console.log(name, (await state(page)).dialog);
}
async function shot(page, name) {
  await paint(page);
  await page.mouse.move(1, 1);
  await page.screenshot({ path: `${output}${name}.png` });
}
async function menu(page) {
  if ((await state(page)).dialog) await page.keyboard.press("Escape");
  await paint(page);
  await page.keyboard.press("Escape");
  await paint(page);
}
async function loadFixture(page) {
  const choosing = page.waitForEvent("filechooser");
  await act(page, "Choose game ROM");
  await (await choosing).setFiles(fixture);
  await page.waitForFunction(
    () => JSON.parse(nesPlayer.diagnostics()).title === "Input test",
  );
  await paint(page);
  const play = page.getByRole("button", { name: "Play", exact: true }).first();
  await play.waitFor({ state: "visible" });
  await play.focus();
  await paint(page);
  await play.press("Enter");
  await paint(page);
}
try {
  const desktop = await browser.newContext({
    viewport: { width: 1440, height: 900 },
  });
  const page = await open(desktop);
  await shot(page, "desktop-welcome");
  assert.match(await page.title(), /^AgentEmu/);
  assert.equal(
    await page.getByRole("button", { name: /demo|playground/i }).count(),
    0,
  );
  assert.equal(
    await page
      .getByRole("button", { name: "Choose game ROM", exact: true })
      .count(),
    1,
  );
  await act(page, "Explore AI games");
  await shot(page, "gallery-desktop");
  await act(page, "Set up Contra");
  await shot(page, "gallery-setup");
  await page
    .getByRole("radio", { name: "Shared", exact: true })
    .first()
    .press("Enter");
  await paint(page);
  await shot(page, "gallery-shared");
  await page
    .getByRole("checkbox", { name: "No player 2", exact: true })
    .press("Space");
  await paint(page);
  await shot(page, "gallery-solo-shared");
  const mismatching = page.waitForEvent("filechooser");
  await act(page, "Choose game ROM");
  await (await mismatching).setFiles(fixture);
  await page.waitForFunction(
    () => JSON.parse(nesPlayer.diagnostics()).dialog === "error",
  );
  assert.match((await state(page)).error, /This agent needs Contra/);
  assert.match((await state(page)).error, /Expected SHA-256: [a-f0-9]{64}/);
  assert.match((await state(page)).error, /Selected SHA-256: [a-f0-9]{64}/);
  assert.equal((await state(page)).frame, null);
  await shot(page, "gallery-mismatch");
  await act(page, "Back");
  await act(page, "Player");
  await loadFixture(page);
  await page.waitForFunction(
    () => JSON.parse(nesPlayer.diagnostics()).frame > 20,
  );
  const focus = await state(page);
  await click(
    page,
    focus.game_rect[0] + focus.game_rect[2] / 2,
    focus.game_rect[1] + focus.game_rect[3] / 2,
  );
  await page.keyboard.down("d");
  await page.keyboard.down("l");
  await page.keyboard.down("j");
  await page.waitForFunction(
    () => JSON.parse(nesPlayer.diagnostics()).decisions?.[0].applied === 131,
  );
  await page.waitForFunction(() => {
    if (!window.__nesAudioAnalyser) return false;
    const pcm = new Float32Array(2048);
    __nesAudioAnalyser.getFloatTimeDomainData(pcm);
    return pcm.some((x) => Math.abs(x) > 0.001);
  });
  const beforeStall = await state(page);
  const duringStall = await page.evaluate(() => {
    const until = performance.now() + 350;
    while (performance.now() < until) {}
    const pcm = new Float32Array(2048);
    __nesAudioAnalyser.getFloatTimeDomainData(pcm);
    return Math.sqrt(pcm.reduce((sum, x) => sum + x * x, 0) / pcm.length);
  });
  assert.ok(duringStall > 0.001, "audio continues through presentation stalls");
  await page.keyboard.press("Escape");
  await page.waitForFunction(
    () => JSON.parse(nesPlayer.diagnostics()).dialog === "menu",
  );
  for (const key of ["d", "l", "j"]) await page.keyboard.up(key);
  const paused = await state(page);
  assert.equal(paused.runtime, null);
  assert.equal(paused.audio_queued, 0);
  assert.ok(paused.frame - beforeStall.frame >= 18);
  await act(page, "Saved games");
  await shot(page, "saves-empty");
  await page
    .getByRole("button", { name: "Save here", exact: true })
    .first()
    .press("Enter");
  await paint(page);
  await page.waitForFunction(
    () => JSON.parse(nesPlayer.diagnostics()).save_slots[0] != null,
  );
  let savedFrame = (await state(page)).save_slots[0];
  await shot(page, "saves-captured");
  await act(page, "Done");
  await act(page, "Resume");
  await page.waitForFunction(
    (n) => JSON.parse(nesPlayer.diagnostics()).frame > n + 8,
    savedFrame,
  );
  await page.keyboard.press("F5");
  await page.waitForFunction((n) => {
    const s = JSON.parse(nesPlayer.diagnostics());
    return s.runtime === null && s.save_slots[0] > n;
  }, savedFrame);
  savedFrame = (await state(page)).save_slots[0];
  await act(page, "Resume");
  await page.waitForFunction(
    (n) => JSON.parse(nesPlayer.diagnostics()).frame > n + 8,
    savedFrame,
  );
  await page.evaluate(() => window.dispatchEvent(new Event("blur")));
  await page.waitForFunction(() => {
    const s = JSON.parse(nesPlayer.diagnostics());
    return !s.running && s.runtime === null;
  });
  const frame = (await state(page)).frame;
  await paint(page);
  assert.equal((await state(page)).frame, frame);
  // Reload and reopen the homebrew: persisted slot and preview must survive.
  await page.reload();
  await page.waitForFunction(() =>
    window.nesPlayer?.diagnostics().includes("rust-egui"),
  );
  await paint(page);
  await loadFixture(page);
  await menu(page);
  await act(page, "Saved games");
  assert.equal((await state(page)).save_slots[0], savedFrame);
  await page
    .getByRole("button", { name: "Load…", exact: true })
    .first()
    .press("Enter");
  await paint(page);
  await shot(page, "load-confirmation");
  await act(page, "Load save");
  assert.equal((await state(page)).frame, savedFrame);
  const choosing = page.waitForEvent("filechooser");
  await act(page, "Open ROM");
  const input = await choosing;
  await input.setFiles({
    name: "invalid.nes",
    mimeType: "application/octet-stream",
    buffer: Buffer.from("invalid"),
  });
  await page.waitForFunction(
    () => JSON.parse(nesPlayer.diagnostics()).dialog === "error",
  );
  assert.equal((await state(page)).frame, savedFrame);
  await shot(page, "invalid-rom");
  assert.equal(
    await page
      .getByRole("button", { name: "Choose another ROM", exact: true })
      .count(),
    1,
  );
  await act(page, "Return to paused game");
  await menu(page);
  await act(page, "Settings");
  await shot(page, "settings-desktop");
  await act(page, "Touch layout");
  await shot(page, "touch-settings");
  await act(page, "‹ Back");
  assert.equal((await state(page)).dialog, "settings");
  await act(page, "Done");
  await menu(page);
  await act(page, "Help & about");
  await shot(page, "help");
  assert.equal(
    await page
      .getByRole("button", {
        name: /Share an agent|catalog|publish|Download model|Upload model/i,
      })
      .count(),
    0,
  );
  if (process.env.GALLERY_CONTRA_ROM) {
    await act(page, "Done");
    await act(page, "AI games");
    await act(page, "Set up Contra");
    await page
      .getByRole("radio", { name: "Shared", exact: true })
      .first()
      .press("Enter");
    await paint(page);
    await act(page, "AI handles B");
    await act(page, "AI handles A");
    await act(page, "AI handles A");
    await act(page, "AI handles B");
    await page
      .getByRole("checkbox", { name: "No player 2", exact: true })
      .press("Space");
    await paint(page);
    const choosing = page.waitForEvent("filechooser");
    await act(page, "Choose game ROM");
    await (await choosing).setFiles(process.env.GALLERY_CONTRA_ROM);
    // Replacing the test fixture is explicit and preserves the gallery configuration.
    await page.waitForFunction(
      () => JSON.parse(nesPlayer.diagnostics()).dialog === "confirm",
    );
    await act(page, "Open game");
    await page.waitForFunction(() => {
      const s = JSON.parse(nesPlayer.diagnostics());
      return s.running && s.frame > 120;
    });
    assert.deepEqual((await state(page)).modes, ["Shared", "No input"]);
    assert.equal((await state(page)).dialog, null);
    await shot(page, "contra-solo-shared-desktop");
    await page.waitForFunction(
      () => JSON.parse(nesPlayer.diagnostics()).frame > 610,
    );
    await act(page, "Player 1 Start");
    await page.waitForFunction(
      () =>
        JSON.parse(nesPlayer.diagnostics()).decisions?.[0].phase ===
        "supported-gameplay",
    );
    // NES Start returns keyboard focus to gameplay without another canvas click.
    await page.keyboard.down("d");
    await page.waitForFunction(() =>
      Boolean(JSON.parse(nesPlayer.diagnostics()).decisions?.[0].human & 128),
    );
    await page.keyboard.up("d");
    const enteredFrame = (await state(page)).frame;
    await page.waitForFunction(
      (n) => JSON.parse(nesPlayer.diagnostics()).frame > n + 45,
      enteredFrame,
    );
    await act(page, "Player 1 Player setup");
    await act(page, "Change agent");
    await shot(page, "contra-agents");
    const search = page.getByRole("textbox", {
      name: "Search agents",
      exact: true,
    });
    await search.fill("no-such-agent");
    await paint(page);
    await page.waitForTimeout(300);
    assert.ok(
      await page
        .getByText("No agents match your search.", { exact: false })
        .count(),
    );
    await shot(page, "agent-search-empty");
    await act(page, "Clear");
    await act(page, "‹ Back");
    assert.equal((await state(page)).dialog, "player");
    await act(page, "Done");
    await menu(page);
    await act(page, "Settings");
    await act(page, "Touch layout");
    await page
      .getByRole("checkbox", { name: "Show touch controls", exact: true })
      .press("Space");
    await paint(page);
    await act(page, "Done");
    await page.setViewportSize({ width: 390, height: 844 });
    await paint(page);
    await shot(page, "contra-shared-phone");
    await act(page, "Inputs");
    await shot(page, "contra-shared-trace-phone");
    await act(page, "Hide");
    await page.setViewportSize({ width: 844, height: 390 });
    await paint(page);
    await shot(page, "contra-shared-landscape");
    await page.setViewportSize({ width: 1440, height: 900 });
    await paint(page);
  }
  if (process.env.GALLERY_SOLO_ROM) {
    if ((await state(page)).dialog) await act(page, "Done");
    const choosing = page.waitForEvent("filechooser");
    await act(page, "Open ROM");
    await (await choosing).setFiles(process.env.GALLERY_SOLO_ROM);
    await page.waitForFunction(
      () => JSON.parse(nesPlayer.diagnostics()).dialog === "confirm",
    );
    await act(page, "Open game");
    assert.equal((await state(page)).players, 1);
    assert.deepEqual((await state(page)).modes, ["Human", "No input"]);
    assert.equal(
      await page
        .getByRole("button", { name: "+ Player 2", exact: true })
        .count(),
      0,
    );
    await shot(page, "solo-game-desktop");
    await act(page, "Player 1 Player setup");
    assert.ok(
      await page.getByRole("radio", { name: "AI", exact: true }).isDisabled(),
    );
    await shot(page, "solo-no-agent");
  }
  await desktop.close();
  for (const size of [
    { width: 390, height: 844 },
    { width: 320, height: 568 },
    { width: 844, height: 390 },
    { width: 768, height: 1024 },
  ]) {
    const context = await browser.newContext({
      viewport: size,
      isMobile: true,
      hasTouch: true,
      deviceScaleFactor: 1,
    });
    const p = await open(context, size.width === 390 ? "?renderer=webgl" : "");
    await shot(p, `welcome-${size.width}`);
    await act(p, "Explore AI games");
    await shot(p, `gallery-${size.width}`);
    await act(p, "Set up Contra");
    await shot(p, `setup-${size.width}`);
    await p
      .getByRole("radio", { name: "Shared", exact: true })
      .first()
      .press("Enter");
    await paint(p);
    await shot(p, `shared-${size.width}`);
    await p.keyboard.press("Escape");
    await paint(p);
    await act(p, "Player");
    await loadFixture(p);
    await p.waitForFunction(
      () => JSON.parse(nesPlayer.diagnostics()).frame > 20,
    );
    const cdp = await context.newCDPSession(p);
    const d = await state(p);
    const [pad, , b] = d.touch_layout;
    await cdp.send("Input.dispatchTouchEvent", {
      type: "touchStart",
      touchPoints: [
        { x: pad[0] + pad[2] * 0.8, y: pad[1] + pad[3] * 0.2, id: 1 },
        { x: b[0] + b[2] / 2, y: b[1] + b[3] / 2, id: 2 },
      ],
    });
    await p.waitForFunction(
      () => JSON.parse(nesPlayer.diagnostics()).decisions?.[0].applied === 146,
    );
    await shot(p, `touch-diagonal-${size.width}`);
    await cdp.send("Input.dispatchTouchEvent", {
      type: "touchEnd",
      touchPoints: [],
    });
    await p.waitForFunction(
      () => JSON.parse(nesPlayer.diagnostics()).decisions?.[0].applied === 0,
    );
    await menu(p);
    await act(p, "Settings");
    await act(p, "Touch layout");
    await p
      .getByRole("checkbox", { name: "Left-handed layout", exact: true })
      .press("Space");
    await paint(p);
    await p
      .getByRole("slider", { name: "Control size", exact: true })
      .press("End");
    await paint(p);
    await shot(p, `touch-settings-${size.width}`);
    await act(p, "Done");
    await p.setViewportSize({ width: 844, height: 390 });
    await paint(p);
    const rotated = await state(p);
    assert.ok(
      rotated.touch_layout[0][0] > rotated.touch_layout[1][0],
      "left-handed layout survives rotation",
    );
    assert.ok(
      rotated.game_rect[0] >= 0 &&
        rotated.game_rect[0] + rotated.game_rect[2] <= 844,
    );
    await shot(p, `mirrored-landscape-${size.width}`);
    if (size.width === 320) {
      await p.setViewportSize(size);
      await paint(p);
      await menu(p);
      await act(p, "Settings");
      await p
        .getByRole("slider", { name: "Interface size", exact: true })
        .press("End");
      await paint(p);
      await shot(p, "phone-large-interface");
      const done = await p
        .getByRole("button", { name: "Done", exact: true })
        .boundingBox();
      assert.ok(
        done.x >= 0 &&
          done.x + done.width <= 320 &&
          done.y >= 0 &&
          done.y + done.height <= 568,
      );
      await act(p, "Done");
    }
    await context.close();
  }
  assert.deepEqual(errors, []);
  console.log(
    "Rust UI passed: semantic navigation, setup, persistent save/restore, relevant errors, audio continuity, input focus, touch chords, mirrored rotation, responsive layouts and WebGL.",
  );
} catch (error) {
  for (const c of browser.contexts())
    for (const p of c.pages()) {
      console.error(await state(p).catch(() => "Not initialized"));
      console.error(await p.evaluate(() => window.__uiActions?.slice(-15)));
      await p.screenshot({ path: `${output}failure.png` }).catch(() => {});
    }
  throw error;
} finally {
  await browser.close();
  await rm(fixtureDir, { recursive: true, force: true });
  server.kill();
}
