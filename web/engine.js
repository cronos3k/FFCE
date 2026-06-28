// Thin JS wrapper around the FFCE Rust/WASM core.
// All engine logic lives in Rust; this only marshals calls and reads the
// shared board snapshot out of WebAssembly linear memory.

export const PIECE = { PAWN: 1, KNIGHT: 2, BISHOP: 3, ROOK: 4, QUEEN: 5, KING: 6 };
export const STATUS = { ONGOING: 0, WHITE_WINS: 1, BLACK_WINS: 2, STALEMATE: 3, DRAW: 4 };

export class Engine {
  constructor(instance) {
    this.e = instance.exports;
    this.mem = this.e.memory;
  }

  // Load the wasm module. Prefers an inlined base64 blob (so the page works
  // straight from file://); falls back to fetching the .wasm next to it.
  static async load() {
    let bytes = null;
    if (typeof globalThis.FFCE_WASM_BASE64 === "string") {
      const bin = atob(globalThis.FFCE_WASM_BASE64);
      bytes = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    } else {
      const resp = await fetch("ffce_engine.wasm");
      bytes = new Uint8Array(await resp.arrayBuffer());
    }
    const { instance } = await WebAssembly.instantiate(bytes, {});
    return new Engine(instance);
  }

  // Fresh memory view (the buffer can detach when wasm memory grows).
  _u8() {
    return new Uint8Array(this.mem.buffer);
  }

  newGame(seed) {
    this.e.ffce_new(seed >>> 0);
  }

  reset() {
    this.e.ffce_reset();
  }

  // Returns a flat 64-length array; index = y*8 + x (y=0 is rank 1).
  // Each entry: 0 empty, 1..6 white piece type, 9..14 black piece type.
  board() {
    const ptr = this.e.ffce_board_ptr();
    const mem = this._u8();
    return Array.from(mem.subarray(ptr, ptr + 64));
  }

  sideToMove() {
    return this.e.ffce_side_to_move(); // 1 white, -1 black
  }

  fullmove() {
    return this.e.ffce_fullmove();
  }

  // Array of legal moves: { fx, fy, tx, ty, promo }.
  legalMoves() {
    const n = this.e.ffce_gen_moves();
    const ptr = this.e.ffce_moves_ptr();
    const mem = this._u8();
    const out = [];
    for (let i = 0; i < n; i++) {
      const o = ptr + i * 5;
      out.push({
        fx: mem[o], fy: mem[o + 1], tx: mem[o + 2], ty: mem[o + 3], promo: mem[o + 4],
      });
    }
    return out;
  }

  makeMove(fx, fy, tx, ty, promo = 0) {
    return this.e.ffce_make_move(fx, fy, tx, ty, promo) === 1;
  }

  // Computes & applies the AI move. Returns the move {fx,fy,tx,ty,promo} or null.
  aiMove(noiseSigma = 0.18) {
    const packed = this.e.ffce_ai_move(noiseSigma);
    if (packed < 0) return null;
    return {
      fx: packed & 7,
      fy: (packed >> 3) & 7,
      tx: (packed >> 6) & 7,
      ty: (packed >> 9) & 7,
      promo: (packed >> 12) & 7,
    };
  }

  status() {
    return this.e.ffce_status();
  }

  inCheck(color) {
    return this.e.ffce_in_check(color) === 1;
  }

  // ---- Extras configuration ------------------------------------------------

  extraCount() {
    return this.e.ffce_extra_count();
  }

  setExtra(i, enabled, mult) {
    this.e.ffce_set_extra(i, enabled ? 1 : 0, mult);
  }

  extraDefaultWeight(i) {
    return this.e.ffce_extra_default_weight(i);
  }

  extraEnabled(i) {
    return this.e.ffce_extra_enabled(i) === 1;
  }

  // ---- Field overlays / histogram -----------------------------------------

  // Returns a copied Float32Array(64) for the requested overlay mode.
  // mode: 0 none, 1 net-pressure, 2 resistance, 3 trace, 4 attack,
  //       5 extras-sum, 6 selected-extra, 7 flow.
  overlay(mode, selected) {
    const ptr = this.e.ffce_overlay(mode, selected);
    return new Float32Array(this.mem.buffer, ptr, 64).slice();
  }

  // Raw 8x8 field of a single extra (for the histogram).
  extraField(i) {
    const ptr = this.e.ffce_extra_field(i);
    return new Float32Array(this.mem.buffer, ptr, 64).slice();
  }

  // Signed king-zone contribution of an extra at the current position.
  extraContribution(i) {
    return this.e.ffce_extra_contribution(i);
  }
}
