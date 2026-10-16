// Semantic mirror of the Rust/egui AccessKit tree. No independent product state.
// All actions are dispatched back to the same widgets rendered on the canvas.
export function attachAccessibility(player, canvas) {
  const root = document.createElement("div");
  root.id = "nes-accessibility";
  root.setAttribute("aria-label", "AgentEmu");
  Object.assign(root.style, {
    position: "fixed",
    inset: "0",
    pointerEvents: "none",
    zIndex: "50",
  });
  document.body.append(root);
  canvas.setAttribute("aria-hidden", "true");
  canvas.tabIndex = -1;
  let last = "",
    scope = "",
    keyboard = false,
    lastFocused = null;
  // The canvas and its semantic controls are one application. Moving between
  // them must not look like switching away from the game.
  const syncFocus = () =>
    player.accessibility_focus(document.hasFocus() && !document.hidden);
  window.addEventListener("focus", syncFocus);
  window.addEventListener("blur", () => player.accessibility_focus(false));
  document.addEventListener("visibilitychange", syncFocus);
  syncFocus();
  const elements = new Map();
  const send = (id, action, value = "") => {
    const label = elements.get(id)?.textContent || "";
    const accepted = player.accessibility_action(id, action, String(value));
    if (
      accepted &&
      action === "click" &&
      /^(Play|Resume|Resume game|Player [12] (Start|Select)|Game screen · .*)$/.test(
        label,
      )
    ) {
      canvas.focus();
    }
    return accepted;
  };
  function refresh() {
    if (document.hidden) return;
    const text = player.accessibility_tree();
    if (!text || text === last) return;
    last = text;
    const tree = JSON.parse(text),
      nodes = new Map(tree.nodes.map((n) => [n.id, n]));
    const modal = nodes.get(tree.modal);
    const nextScope = modal?.id || tree.root;
    if (nextScope !== scope) {
      scope = nextScope;
      root.setAttribute("role", modal ? "dialog" : "region");
      root.setAttribute("aria-label", modal?.label || "AgentEmu");
      if (modal) root.setAttribute("aria-modal", "true");
      else root.removeAttribute("aria-modal");
    }
    const ordered = [];
    const visited = new Set();
    function walk(id) {
      const n = nodes.get(id);
      if (!n || visited.has(id) || n.hidden) return;
      visited.add(id);
      if (n.role === "InlineTextBox" || n.role === "TextRun") return;
      if (n.label || n.value || n.focusable || n.click) ordered.push(n);
      for (const child of n.children) walk(child);
    }
    walk(nextScope);
    // Combo-box popups are separate egui Areas, outside the dialog subtree.
    if (modal)
      for (const n of nodes.values())
        if (
          !visited.has(n.id) &&
          !n.disabled &&
          !n.hidden &&
          (n.focusable || n.click) &&
          n.role !== "Label" &&
          n.role !== "TextRun"
        )
          ordered.push(n);
    const keep = new Set();
    for (const n of ordered) {
      if (n.role === "Dialog" || n.role === "Window") continue;
      const control = n.role !== "Label" && (n.focusable || n.click);
      const role = n.role;
      const tag =
        role === "TextInput" ||
        role === "MultilineTextInput" ||
        role === "Slider" ||
        role === "SpinButton"
          ? "input"
          : control
            ? "button"
            : "span";
      // Unlabelled layout containers have no useful browser semantics.
      if (!control && !n.label && !n.value) continue;
      let el = elements.get(n.id);
      if (el && el.tagName.toLowerCase() !== tag) {
        el.remove();
        elements.delete(n.id);
        el = null;
      }
      if (!el) {
        el = document.createElement(tag);
        el.id = `nes-a11y-${n.id}`;
        el.className = control ? "semantic-control" : "semantic-text";
        if (control) {
          el.addEventListener("focus", () => {
            keyboard = true;
            lastFocused = n.id;
            send(n.id, "focus");
          });
          el.addEventListener("click", (e) => {
            e.preventDefault();
            send(n.id, "click");
          });
          el.addEventListener("keydown", (e) => {
            if (tag === "button" && (e.key === "Enter" || e.key === " ")) {
              e.preventDefault();
              e.stopPropagation();
              if (!e.repeat) send(n.id, "click");
              return;
            }
            if (e.key === "Tab" && root.hasAttribute("aria-modal")) {
              const controls = [
                ...root.querySelectorAll(
                  "button:not(:disabled),input:not(:disabled)",
                ),
              ];
              const i = controls.indexOf(el);
              if (
                (e.shiftKey && i === 0) ||
                (!e.shiftKey && i === controls.length - 1)
              ) {
                e.preventDefault();
                controls[e.shiftKey ? controls.length - 1 : 0]?.focus();
              }
            }
            if (e.key === "Escape") {
              e.preventDefault();
              e.stopPropagation();
              send(n.id, "escape");
            }
            if (tag === "input") e.stopPropagation();
          });
          if (tag === "input")
            el.addEventListener("input", () =>
              send(
                n.id,
                el.type === "range" || el.type === "number" ? "number" : "text",
                el.value,
              ),
            );
        }
        elements.set(n.id, el);
      }
      keep.add(n.id);
      if (tag === "input") {
        el.type =
          role === "Slider"
            ? "range"
            : role === "SpinButton"
              ? "number"
              : "text";
        if (n.min != null) el.min = n.min;
        if (n.max != null) el.max = n.max;
        el.step = n.step || "any";
        const value = n.number ?? n.value ?? "";
        if (document.activeElement !== el) el.value = value;
        el.setAttribute(
          "aria-label",
          n.label || (role === "TextInput" ? "Search agents" : "Value"),
        );
      } else {
        el.textContent = n.label || n.value || role;
      }
      if (control) {
        el.disabled = !!n.disabled;
        if (n.selected != null)
          el.setAttribute("aria-pressed", String(n.selected));
        else el.removeAttribute("aria-pressed");
        if (n.checked != null) {
          el.setAttribute(
            "role",
            role === "RadioButton" ? "radio" : "checkbox",
          );
          el.setAttribute(
            "aria-checked",
            n.checked === "True"
              ? "true"
              : n.checked === "Mixed"
                ? "mixed"
                : "false",
          );
        }
        if (n.bounds) {
          const scale =
            (tree.pixels_per_point || devicePixelRatio) / devicePixelRatio;
          const [x, y, w, h] = n.bounds.map((v) => v * scale);
          Object.assign(el.style, {
            left: `${Math.max(0, x)}px`,
            top: `${Math.max(0, Math.min(innerHeight - 44, y))}px`,
            width: `${Math.max(44, Math.min(w, innerWidth - x))}px`,
            height: `${Math.max(36, h)}px`,
          });
        }
      }
      // Keep stable DOM order without moving an already-focused editor every frame.
      const index = ordered.filter((o) => keep.has(o.id)).length - 1;
      if (root.children[index] !== el)
        root.insertBefore(el, root.children[index] || null);
    }
    for (const [id, el] of elements)
      if (!keep.has(id)) {
        el.remove();
        elements.delete(id);
      }
    if (keyboard && lastFocused && !keep.has(lastFocused)) {
      lastFocused = null;
      const target = modal
        ? root.querySelector("button:not(:disabled),input:not(:disabled)")
        : canvas;
      target?.focus();
    }
  }
  // Escape must also work immediately after a view change removes the focused
  // DOM control. Canvas-targeted keys remain owned by eframe.
  document.addEventListener(
    "keydown",
    (e) => {
      if (e.key === "Escape" && e.target !== canvas && last) {
        e.preventDefault();
        e.stopImmediatePropagation();
        send(JSON.parse(last).root, "escape");
      }
    },
    true,
  );
  const timer = setInterval(refresh, 100);
  refresh();
  window.addEventListener("pagehide", () => clearInterval(timer), {
    once: true,
  });
}
