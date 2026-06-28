// FFCE Web — board UI controller. Thin shell: rendering, overlays, input.
import { Engine, STATUS } from "./engine.js";
import { DOCS, EXTRA_LABELS, EXTRA_KEYS, CORE_KEYS, CORE_LABELS } from "./docs.js";
import { ALGO_PALETTE, overlayBaseColor, drawHeatmap, drawHistogram } from "./overlay.js";

const WHITE = 1, BLACK = -1;
const PNAME = { 1: "pawn", 2: "knight", 3: "bishop", 4: "rook", 5: "queen", 6: "king" };
const GLYPH = { 1: "♟", 2: "♞", 3: "♝", 4: "♜", 5: "♛", 6: "♚" };

const state = {
  engine: null,
  humanSide: WHITE,      // board orientation (side shown at the bottom)
  aiMode: "black",       // black | white | both | none
  selected: null,
  legal: [],
  lastMove: null,
  busy: false,
  noise: 0.18,
  overlayMode: 0,        // 0 none .. 7 flow
  selectedExtra: 0,
  showHist: false,
  nExtras: 13,
  extras: [],            // [{enabled, mult, def}]
  lastAiMs: 0,
};

const boardEl = document.getElementById("board");
const canvas = document.getElementById("overlayCanvas");
const g2d = canvas.getContext("2d");
const statusEl = document.getElementById("status");
const turnEl = document.getElementById("turn");

// ---- orientation --------------------------------------------------------
function toScreen(x, y) {
  return state.humanSide === WHITE ? { r: 7 - y, c: x } : { r: y, c: 7 - x };
}
function fromScreen(r, c) {
  return state.humanSide === WHITE ? { x: c, y: 7 - r } : { x: 7 - c, y: r };
}
function decode(code) {
  if (code === 0) return null;
  if (code < 8) return { color: WHITE, type: code };
  return { color: BLACK, type: code - 8 };
}
function aiControls(side) {
  if (state.aiMode === "both") return true;
  if (state.aiMode === "black") return side === BLACK;
  if (state.aiMode === "white") return side === WHITE;
  return false;
}

// ---- board build / render ----------------------------------------------
function buildGrid() {
  boardEl.innerHTML = "";
  for (let r = 0; r < 8; r++) {
    for (let c = 0; c < 8; c++) {
      const cell = document.createElement("div");
      cell.className = "cell";
      cell.dataset.r = r;
      cell.dataset.c = c;
      cell.addEventListener("click", onCellClick);
      boardEl.appendChild(cell);
    }
  }
}

function pieceNode(p) {
  const img = document.createElement("img");
  img.className = "piece";
  const color = p.color === WHITE ? "white" : "black";
  img.src = `assets/pieces/${color}_${PNAME[p.type]}.png`;
  img.alt = "";
  img.draggable = false;
  // Fallback to a Unicode glyph if the PNG is missing.
  img.onerror = () => {
    const span = document.createElement("span");
    span.className = "piece";
    span.style.fontSize = "min(9vw,58px)";
    span.style.color = p.color === WHITE ? "#fafafa" : "#1c1c1c";
    span.style.textShadow = p.color === WHITE ? "0 0 2px #000" : "0 0 2px #fff";
    span.textContent = GLYPH[p.type];
    img.replaceWith(span);
  };
  return img;
}

function render() {
  const board = state.engine.board();
  const cells = boardEl.children;
  const side = state.engine.sideToMove();
  const checkW = state.engine.inCheck(WHITE);
  const checkB = state.engine.inCheck(BLACK);

  for (let r = 0; r < 8; r++) {
    for (let c = 0; c < 8; c++) {
      const cell = cells[r * 8 + c];
      const { x, y } = fromScreen(r, c);
      cell.className = "cell " + ((r + c) % 2 === 0 ? "light" : "dark");
      cell.innerHTML = "";
      const p = decode(board[y * 8 + x]);
      if (p) {
        cell.appendChild(pieceNode(p));
        if (p.type === 6 && ((p.color === WHITE && checkW) || (p.color === BLACK && checkB))) {
          cell.classList.add("check");
        }
      }
      if (state.lastMove &&
          ((x === state.lastMove.fx && y === state.lastMove.fy) ||
           (x === state.lastMove.tx && y === state.lastMove.ty))) {
        cell.classList.add("lastmove");
      }
    }
  }

  if (state.selected) {
    const s = toScreen(state.selected.x, state.selected.y);
    cells[s.r * 8 + s.c].classList.add("selected");
    for (const m of state.legal) {
      if (m.fx === state.selected.x && m.fy === state.selected.y) {
        const t = toScreen(m.tx, m.ty);
        const occupied = decode(board[m.ty * 8 + m.tx]) !== null;
        cells[t.r * 8 + t.c].classList.add(occupied ? "capture" : "target");
      }
    }
  }

  drawOverlay(side);
  updateStatus(side, checkW, checkB);
}

function drawOverlay(side) {
  const px = boardEl.clientWidth;
  if (px === 0) return;
  if (canvas.width !== px) { canvas.width = px; canvas.height = px; }
  g2d.clearRect(0, 0, px, px);
  const sq = px / 8;

  if (state.overlayMode > 0) {
    const grid = state.engine.overlay(state.overlayMode, state.selectedExtra);
    const base = overlayBaseColor(state.overlayMode, state.selectedExtra);
    drawHeatmap(g2d, grid, base, sq, toScreen);
  }
  if (state.showHist) {
    const entries = [];
    for (let i = 0; i < state.nExtras; i++) {
      if (state.extras[i].enabled) {
        entries.push({ idx: i, field: state.engine.extraField(i), color: ALGO_PALETTE[i % ALGO_PALETTE.length] });
      }
    }
    drawHistogram(g2d, entries, sq, toScreen, state.selectedExtra);
  }
}

function updateStatus(side, checkW, checkB) {
  document.getElementById("aimode").textContent =
    state.aiMode.charAt(0).toUpperCase() + state.aiMode.slice(1);
  const st = state.engine.status();
  if (st === STATUS.ONGOING) {
    const chk = (side === WHITE && checkW) || (side === BLACK && checkB);
    turnEl.textContent = (side === WHITE ? "White" : "Black") + " to move" + (chk ? " — check!" : "");
    statusEl.textContent = state.busy ? "AI is thinking…" : (state.lastAiMs ? `AI moved in ${state.lastAiMs} ms` : "");
  } else {
    turnEl.textContent = "Game over";
    statusEl.textContent =
      st === STATUS.WHITE_WINS ? "Checkmate — White wins" :
      st === STATUS.BLACK_WINS ? "Checkmate — Black wins" :
      st === STATUS.STALEMATE ? "Stalemate — draw" : "Draw (50-move rule)";
  }
}

function refreshLegal() { state.legal = state.engine.legalMoves(); }

// ---- input --------------------------------------------------------------
function onCellClick(ev) {
  if (state.busy) return;
  if (state.engine.status() !== STATUS.ONGOING) return;
  const side = state.engine.sideToMove();
  if (aiControls(side)) return; // not a human turn

  const r = +ev.currentTarget.dataset.r;
  const c = +ev.currentTarget.dataset.c;
  const { x, y } = fromScreen(r, c);

  if (state.selected) {
    const matches = state.legal.filter(
      (m) => m.fx === state.selected.x && m.fy === state.selected.y && m.tx === x && m.ty === y
    );
    if (matches.length > 0) {
      if (matches.length > 1) {
        promptPromotion((promo) => doHumanMove(state.selected.x, state.selected.y, x, y, promo));
      } else {
        doHumanMove(state.selected.x, state.selected.y, x, y, matches[0].promo);
      }
      return;
    }
  }
  const p = decode(state.engine.board()[y * 8 + x]);
  state.selected = p && p.color === side ? { x, y } : null;
  render();
}

function doHumanMove(fx, fy, tx, ty, promo) {
  if (!state.engine.makeMove(fx, fy, tx, ty, promo)) return;
  state.lastMove = { fx, fy, tx, ty };
  state.selected = null;
  state.lastAiMs = 0;
  refreshLegal();
  render();
  scheduleAi();
}

function scheduleAi() {
  if (state.engine.status() !== STATUS.ONGOING) return;
  if (!aiControls(state.engine.sideToMove())) return;
  state.busy = true;
  render();
  setTimeout(() => {
    const t0 = performance.now();
    const m = state.engine.aiMove(state.noise);
    state.lastAiMs = Math.round(performance.now() - t0);
    if (m) state.lastMove = { fx: m.fx, fy: m.fy, tx: m.tx, ty: m.ty };
    state.busy = false;
    state.selected = null;
    refreshLegal();
    render();
    scheduleAi(); // chain for AI-vs-AI (both) mode
  }, 30);
}

function stepAi() {
  if (state.busy || state.engine.status() !== STATUS.ONGOING) return;
  state.busy = true;
  render();
  setTimeout(() => {
    const t0 = performance.now();
    const m = state.engine.aiMove(state.noise);
    state.lastAiMs = Math.round(performance.now() - t0);
    if (m) state.lastMove = { fx: m.fx, fy: m.fy, tx: m.tx, ty: m.ty };
    state.busy = false;
    state.selected = null;
    refreshLegal();
    render();
  }, 10);
}

function promptPromotion(cb) {
  const overlay = document.getElementById("promo");
  overlay.classList.add("show");
  const pick = (t) => {
    overlay.classList.remove("show");
    overlay.querySelectorAll("button").forEach((b) => (b.onclick = null));
    cb(t);
  };
  [5, 4, 3, 2].forEach((t) => {
    overlay.querySelector(`[data-p="${t}"]`).onclick = () => pick(t);
  });
}

// ---- panel: extras, overlays, docs --------------------------------------
function buildExtrasPanel() {
  const list = document.getElementById("extrasList");
  list.innerHTML = "";
  state.extras = [];
  for (let i = 0; i < state.nExtras; i++) {
    const def = state.engine.extraDefaultWeight(i);
    state.extras.push({ enabled: false, mult: 1.0, def });
    const row = document.createElement("div");
    row.className = "extra-row";
    const color = ALGO_PALETTE[i % ALGO_PALETTE.length];
    row.innerHTML = `
      <div class="extra-head">
        <input type="checkbox" data-i="${i}" class="ex-check" />
        <span class="extra-name" data-i="${i}" style="border-left-color:rgb(${color[0]},${color[1]},${color[2]})">${EXTRA_LABELS[i]}</span>
        <span class="extra-val" id="exval-${i}">${def.toFixed(2)}</span>
      </div>
      <input type="range" min="0" max="2" step="0.05" value="1" data-i="${i}" class="ex-slider" />`;
    list.appendChild(row);
  }
  list.querySelectorAll(".ex-check").forEach((el) =>
    el.addEventListener("change", (e) => onExtraChange(+e.target.dataset.i)));
  list.querySelectorAll(".ex-slider").forEach((el) =>
    el.addEventListener("input", (e) => onExtraChange(+e.target.dataset.i)));
  list.querySelectorAll(".extra-name").forEach((el) =>
    el.addEventListener("click", (e) => showDoc(EXTRA_KEYS[+e.target.dataset.i], true)));
}

function onExtraChange(i) {
  const check = document.querySelector(`.ex-check[data-i="${i}"]`);
  const slider = document.querySelector(`.ex-slider[data-i="${i}"]`);
  const enabled = check.checked;
  const mult = parseFloat(slider.value);
  state.extras[i].enabled = enabled;
  state.extras[i].mult = mult;
  document.getElementById(`exval-${i}`).textContent = (state.extras[i].def * mult).toFixed(2);
  state.engine.setExtra(i, enabled, mult);
  drawOverlay(state.engine.sideToMove());
}

function syncExtrasUi() {
  for (let i = 0; i < state.nExtras; i++) {
    document.querySelector(`.ex-check[data-i="${i}"]`).checked = state.extras[i].enabled;
    document.querySelector(`.ex-slider[data-i="${i}"]`).value = state.extras[i].mult;
    document.getElementById(`exval-${i}`).textContent = (state.extras[i].def * state.extras[i].mult).toFixed(2);
    state.engine.setExtra(i, state.extras[i].enabled, state.extras[i].mult);
  }
}

function setAllExtras(on) {
  for (let i = 0; i < state.nExtras; i++) state.extras[i].enabled = on;
  syncExtrasUi();
  drawOverlay(state.engine.sideToMove());
}

function buildSelectors() {
  const sel = document.getElementById("selExtra");
  sel.innerHTML = "";
  EXTRA_LABELS.forEach((label, i) => {
    const opt = document.createElement("option");
    opt.value = i; opt.textContent = label;
    sel.appendChild(opt);
  });
  sel.addEventListener("change", (e) => {
    state.selectedExtra = +e.target.value;
    showDoc(EXTRA_KEYS[state.selectedExtra], false);
    drawOverlay(state.engine.sideToMove());
  });

  const doc = document.getElementById("docSelect");
  doc.innerHTML = "";
  const og1 = document.createElement("optgroup"); og1.label = "Core fields";
  CORE_KEYS.forEach((k) => {
    const o = document.createElement("option"); o.value = k; o.textContent = CORE_LABELS[k]; og1.appendChild(o);
  });
  doc.appendChild(og1);
  const og2 = document.createElement("optgroup"); og2.label = "Extra algorithms";
  EXTRA_KEYS.forEach((k, i) => {
    const o = document.createElement("option"); o.value = k; o.textContent = EXTRA_LABELS[i]; og2.appendChild(o);
  });
  doc.appendChild(og2);
  doc.addEventListener("change", (e) => showDoc(e.target.value, false));
}

function showDoc(key, switchTab) {
  document.getElementById("docText").textContent = DOCS[key] || "";
  document.getElementById("docSelect").value = key;
  const ei = EXTRA_KEYS.indexOf(key);
  if (ei >= 0) {
    state.selectedExtra = ei;
    document.getElementById("selExtra").value = ei;
  }
  if (switchTab) setTab("docs");
}

function setTab(name) {
  document.querySelectorAll(".tab").forEach((t) => t.classList.toggle("active", t.dataset.tab === name));
  document.querySelectorAll(".tabpane").forEach((p) => p.classList.toggle("active", p.id === "tab-" + name));
}

function setOverlayMode(m) {
  state.overlayMode = m;
  document.getElementById("overlayMode").value = m;
  render();
}

// ---- new game -----------------------------------------------------------
function newGame() {
  state.humanSide = document.getElementById("side").value === "black" ? BLACK : WHITE;
  state.aiMode = state.humanSide === WHITE ? "black" : "white";
  state.noise = parseFloat(document.getElementById("noise").value);
  state.engine.newGame((Math.random() * 0xffffffff) >>> 0);
  // Re-apply current extras config to the fresh engine.
  for (let i = 0; i < state.nExtras; i++) {
    state.engine.setExtra(i, state.extras[i].enabled, state.extras[i].mult);
  }
  state.selected = null;
  state.lastMove = null;
  state.busy = false;
  state.lastAiMs = 0;
  refreshLegal();
  render();
  scheduleAi();
}

// ---- keyboard -----------------------------------------------------------
function onKey(e) {
  const tag = (e.target.tagName || "").toLowerCase();
  if (tag === "input" || tag === "select" || tag === "textarea") return;
  const k = e.key.toLowerCase();
  if (k >= "1" && k <= "7") { setOverlayMode(parseInt(k, 10) - 1); return; }
  switch (k) {
    case "h":
      state.showHist = !state.showHist;
      document.getElementById("histToggle").checked = state.showHist;
      render();
      break;
    case "e": {
      const anyOn = state.extras.some((x) => x.enabled);
      setAllExtras(!anyOn);
      break;
    }
    case "x":
      state.selectedExtra = (state.selectedExtra + 1) % state.nExtras;
      document.getElementById("selExtra").value = state.selectedExtra;
      showDoc(EXTRA_KEYS[state.selectedExtra], false);
      render();
      break;
    case "a": {
      const order = ["black", "white", "both", "none"];
      state.aiMode = order[(order.indexOf(state.aiMode) + 1) % order.length];
      render();
      scheduleAi();
      break;
    }
    case "n": stepAi(); break;
    case "r": newGame(); break;
  }
}

// ---- boot ---------------------------------------------------------------
async function boot() {
  state.engine = await Engine.load();
  state.nExtras = state.engine.extraCount();
  buildGrid();
  buildSelectors();
  buildExtrasPanel();
  showDoc("core_pressure", false);

  document.getElementById("newgame").addEventListener("click", newGame);
  document.getElementById("noise").addEventListener("input", (e) => {
    state.noise = parseFloat(e.target.value);
    document.getElementById("noiseval").textContent = state.noise.toFixed(2);
  });
  document.getElementById("overlayMode").addEventListener("change", (e) => setOverlayMode(+e.target.value));
  document.getElementById("histToggle").addEventListener("change", (e) => {
    state.showHist = e.target.checked; render();
  });
  document.getElementById("allOn").addEventListener("click", () => setAllExtras(true));
  document.getElementById("allOff").addEventListener("click", () => setAllExtras(false));
  document.querySelectorAll(".tab").forEach((t) =>
    t.addEventListener("click", () => setTab(t.dataset.tab)));
  window.addEventListener("keydown", onKey);
  window.addEventListener("resize", () => render());

  newGame();
}

boot().catch((err) => {
  statusEl.textContent = "Failed to load engine: " + err;
  console.error(err);
});
