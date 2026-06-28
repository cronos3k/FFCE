# FFCE Web — Flow Field Chess

### ▶ [Play the live demo](https://gamedev.tech/games/ffce/play/) &nbsp;·&nbsp; 📄 [Read the paper (PDF)](https://gamedev.tech/games/ffce/paper/flow-field-chess.pdf)

A browser-playable chess game whose AI thinks in **flow fields** instead of a
search tree. The engine (rules + AI) is written in **Rust and compiled to
WebAssembly**; the browser layer is a thin HTML/CSS/JS shell. No server is
required to play — the folder is statically deployable.

This is the first game for the **gamedev.tech** platform and follows its north
star: *Rust/WASM owns the engine logic; JS is a thin browser shell.*

It is a faithful port of the original Python Flow Field Chess reference
implementation (board, constants, fields, AI, quaternion modules).

## What is Flow Field Chess?

Standard chess rules (8x8, full legal moves, validated by perft). The novelty
is the AI: each turn it builds five fields over the board and picks the move
that yields the best resulting field score — **no opening book, no search tree.**

The five core fields:

1. **Pressure (P = F_w − F_b)** — signed terrain of advantage.
2. **Attack (A)** — weighted reach map of every piece.
3. **Resistance (R)** — friction from occupancy, enemy attacks, pawn locks, king zones.
4. **Trace (T)** — movement memory + quaternion-driven forward intent.
5. **Flow (F)** — diffusion of sources through resistance over a time horizon.

The result is "clever, sometimes baffling, always interesting" play — not engine
strength.

## Layout

```
ffce-web/
  crate/            Rust engine (compiles to native lib + wasm)
    src/
      constants.rs  piece values, weights, tunable params
      quaternion.rs motion-intent quaternion math
      board.rs      board state + legal move generation (rules authority)
      fields.rs     Attack / Resistance / Trace / Future / Flow fields
      extras.rs     the 13 toggleable extra-field algorithms
      eval.rs       position scoring + overlay/analysis API
      ai.rs         single-ply move selection + RNG
      lib.rs        C-ABI exports for the browser (no wasm-bindgen)
    tests/
      game_play.rs    AI-vs-AI legality / termination test
      extras_tests.rs extra-field finiteness / wiring / linear-scaling tests
  web/              static site (open or serve this folder)
    index.html
    style.css       dark theme mirroring the desktop GUI
    engine.js       thin WASM loader / wrapper
    main.js         board UI, overlays, panel, input
    overlay.js      heatmap + histogram colors and canvas drawing
    docs.js         Docs-tab text (core + 13 extras)
    ffce_engine.wasm
    wasm_inline.js  base64-inlined wasm (lets the page run from file://)
    assets/pieces/  piece PNGs from the original FFCE set
  build.ps1         build wasm + stage web assets
  serve.ps1         serve web/ on a free port
```

## Build

Requires the Rust toolchain and the `wasm32-unknown-unknown` target
(`rustup target add wasm32-unknown-unknown`). No `wasm-pack`/`wasm-bindgen`
needed — the engine exposes a plain C-ABI and the JS shell talks to it via
WebAssembly linear memory directly.

```powershell
# From this folder:
./build.ps1
```

`build.ps1` runs the tests, builds the release wasm, copies it into `web/`, and
regenerates `web/wasm_inline.js`. All build artifacts stay inside this project
(`crate/target/`); nothing is written to user/global directories.

Manual equivalent:

```powershell
cd crate
$env:CARGO_TARGET_DIR = "$PWD/target"
cargo test --release
cargo build --release --target wasm32-unknown-unknown
# copy crate/target/wasm32-unknown-unknown/release/ffce_engine.wasm -> web/
```

## Run / preview

```powershell
./serve.ps1          # picks the first free port in a fallback list, serves web/
```

Then open the printed `http://127.0.0.1:<port>/`.

Because the wasm is also base64-inlined into `web/wasm_inline.js`, you can also
just **open `web/index.html` directly** (file://) in most browsers.

## How to play

- You are **White** by default; the flow-field AI is **Black**. Choose your side
  with the dropdown, then **New game**.
- Click a piece, then a highlighted square to move. Legal targets show as dots;
  captures as rings; the last move and a king in check are highlighted.
- Pawn promotion shows a small picker (Q/R/B/N).
- The **AI weirdness (noise)** slider perturbs the evaluation weights for more or
  less erratic play.

## Tests / perft

```powershell
cd crate
cargo test --release
```

Perft sanity checks from the start position (in `src/board.rs`):

| depth | nodes   |
|-------|---------|
| 1     | 20      |
| 2     | 400     |
| 3     | 8902    |
| 4     | 197281  |

`tests/game_play.rs` drives the AI against itself for a full game (with and
without all extras) and asserts every move is legal and the engine reaches a
terminal state. `tests/extras_tests.rs` checks all 13 extra fields are finite,
that enabling each one changes `evaluate_position`, and that the weight
multiplier scales its contribution linearly.

## The 13 extra algorithms

Faithful ports of `extras.py` live in `src/extras.rs`: reaction-diffusion
(Gray-Scott), cellular automata, resistor network, Ising spin, wave resonance,
lattice-Boltzmann, spectral low-frequency, Hodge/curl, ant pheromone, fuzzy
future, topo persistence, latent channels, tensor kernel.

Each is an 8x8 field. The evaluation blend equals the core score plus, for every
**enabled** extra, `weight * field_score(field)` where `field_score` is the
king-zone-weighted sum (`scale * (sum(field·blackKingZone) - sum(field·whiteKingZone))`),
exactly as in `compute_extras_score`. The effective weight is the constants.py
default (e.g. `eval_rd=0.8`) times the panel slider (0.0–2.0, default 1.0×). The
AI's per-move weight noise (ai.py `NOISE_KEYS`) also perturbs all 13.

## UI parity with the desktop GUI

Mirrors `ui.py`: dark chrome (panel rgb(22,22,22), border rgb(40,40,40)), board
squares light rgb(240,235,220) / dark rgb(120,160,130), the original FFCE piece
PNGs, and the highlight palette (select 235,200,80; move
80,180,120; capture 200,80,80; last-move 80,120,200; check 220,60,60).

- **Right panel tabs** — Extras / Docs / File.
  - *Extras*: All On / All Off, plus a checkbox + 0.0–2.0 weight slider + live
    effective-weight value for each of the 13 algorithms, each tagged with its
    overlay color. Clicking an algorithm name opens its Docs entry.
  - *Docs*: formula/explanation text — `EXTRA_DOCS` for the 13 extras and the
    core-field summaries from `docs/algos/core_*.md`; a dropdown selects any.
- **Overlays** (selector + keys 1–7): none, net pressure, resistance, trace,
  attack, extras-sum, selected-extra (plus Flow in the dropdown). Rendered as an
  alpha-blended heatmap on a canvas using the original per-field colors
  (`_field_color_variant`): pressure (220,80,80), attack (90,180,130),
  resistance (90,120,200), trace (210,180,80), flow (180,90,200), and the 13
  algo palette colors for the extras.
- **Histogram** (`H`): per enabled extra, a horizontal bar per cell in the algo
  color, length ∝ |value|/max (port of `_draw_histogram`).
- **Keyboard**: `1`–`7` overlay, `H` histogram, `E` toggle all extras, `X` cycle
  selected extra, `A` cycle AI mode (black/white/both/none), `N` step one AI
  move, `R` reset.

## Implemented (full parity)

- Full legal move generation: castling, en passant, promotion, check,
  checkmate, stalemate, 50-move draw. (Perft-validated to depth 4.)
- The five core flow fields **and all 13 extra algorithms**, with per-algorithm
  enable + weight scaling, blended into the faithful single-ply evaluation
  (material, control, king safety, trace, trace×resistance, capacity/mobility,
  SEE hanging, potential, target pressure, ambush/trap, capture pressure, pawn
  strike, future threat, pawn gravity/promotion, officer activity, extras).
- Quaternion-based piece "intent" feeding the trace/future fields.
- Field overlays + contributions histogram exported from WASM and rendered on a
  canvas; tabbed Extras/Docs/File panel; keyboard shortcuts.

## Deferred / not ported

- Desktop-only plumbing from the Python app that is out of scope for a static
  web build: the REST/GPU plugin bridges, CSV logging, save/restore to disk,
  the frustration model, and the localization JSON drop-down (the UI strings
  are inlined in English).

## Notes

- Piece icons are the original FFCE set in `web/assets/pieces/`; the UI uses them
  directly (16x16, scaled with nearest-neighbor), with a Unicode-glyph fallback
  if an image is missing.
- The AI is intentionally not strong. It evaluates fields, not lines. With all
  13 extras enabled a turn still resolves in roughly 20 ms in the browser.

## License

[GPL-3.0](LICENSE). © Gregor Koch (cronos3k).
