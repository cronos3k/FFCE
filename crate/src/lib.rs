//! FFCE (Flow Field Chess Engine) — Rust core compiled to WebAssembly.
//!
//! The browser shell talks to this crate through a small C-ABI surface (no
//! wasm-bindgen needed). Board state is shared via a pointer into linear
//! memory; moves are exchanged as packed integers / byte arrays.

pub mod ai;
pub mod board;
pub mod constants;
pub mod eval;
pub mod extras;
pub mod fields;
pub mod quaternion;

use ai::{choose_move, Rng};
use board::{Board, Move};
use constants::{Params, BLACK, WHITE};
use extras::ExtrasConfig;
use fields::Grid;

/// Encode a piece for the JS shell: 0 = empty, white = type (1..6),
/// black = type + 8 (9..14).
fn encode_piece(ptype: u8, color: i32) -> u8 {
    if color == WHITE {
        ptype
    } else {
        ptype + 8
    }
}

struct Engine {
    board: Board,
    rng: Rng,
    params: Params,
    config: ExtrasConfig,
    cw: Grid,
    board_buf: [u8; 64],
    move_buf: [u8; 1280], // up to 256 moves * 5 bytes
    move_count: usize,
    overlay_buf: [f32; 64],
    analysis: Option<eval::Analysis>,
}

impl Engine {
    fn new(seed: u64) -> Engine {
        Engine {
            board: Board::new(),
            rng: Rng::new(seed),
            params: Params::default(),
            config: ExtrasConfig::default(),
            cw: constants::center_weight(),
            board_buf: [0u8; 64],
            move_buf: [0u8; 1280],
            move_count: 0,
            overlay_buf: [0.0f32; 64],
            analysis: None,
        }
    }

    /// Invalidate the cached root analysis (after any board mutation).
    fn invalidate(&mut self) {
        self.analysis = None;
    }

    /// Ensure the cached root analysis matches the current position.
    fn ensure_analysis(&mut self) {
        if self.analysis.is_none() {
            self.analysis = Some(eval::analyze(&self.board, &self.params, &self.cw));
        }
    }

    fn refresh_board_buf(&mut self) {
        for y in 0..8usize {
            for x in 0..8usize {
                let id = self.board.board[y][x];
                self.board_buf[y * 8 + x] = if id == 0 {
                    0
                } else {
                    let p = self.board.piece(id);
                    encode_piece(p.ptype, p.color)
                };
            }
        }
    }
}

static mut ENGINE: Option<Engine> = None;

#[allow(static_mut_refs)]
fn engine() -> &'static mut Engine {
    unsafe {
        if ENGINE.is_none() {
            ENGINE = Some(Engine::new(0x1234_5678));
        }
        ENGINE.as_mut().unwrap()
    }
}

/// (Re)initialize the engine with a fresh start position and RNG seed.
#[no_mangle]
pub extern "C" fn ffce_new(seed: u32) {
    unsafe {
        ENGINE = Some(Engine::new(seed as u64));
    }
}

/// Reset to the start position, keeping the current RNG stream + config.
#[no_mangle]
pub extern "C" fn ffce_reset() {
    let e = engine();
    e.board = Board::new();
    e.invalidate();
}

/// Pointer to a 64-byte board snapshot (index = y*8 + x; y=0 is rank 1).
#[no_mangle]
pub extern "C" fn ffce_board_ptr() -> *const u8 {
    let e = engine();
    e.refresh_board_buf();
    e.board_buf.as_ptr()
}

/// Side to move: 1 = White, -1 = Black.
#[no_mangle]
pub extern "C" fn ffce_side_to_move() -> i32 {
    engine().board.side_to_move
}

#[no_mangle]
pub extern "C" fn ffce_fullmove() -> i32 {
    engine().board.fullmove as i32
}

/// Generate legal moves for the side to move into the move buffer.
/// Returns the move count; bytes are [fx, fy, tx, ty, promo] per move.
#[no_mangle]
pub extern "C" fn ffce_gen_moves() -> i32 {
    let e = engine();
    let moves = e.board.generate_legal_moves(e.board.side_to_move);
    let n = moves.len().min(256);
    for (i, m) in moves.iter().take(n).enumerate() {
        let off = i * 5;
        e.move_buf[off] = m.fx as u8;
        e.move_buf[off + 1] = m.fy as u8;
        e.move_buf[off + 2] = m.tx as u8;
        e.move_buf[off + 3] = m.ty as u8;
        e.move_buf[off + 4] = m.promotion;
    }
    e.move_count = n;
    n as i32
}

#[no_mangle]
pub extern "C" fn ffce_moves_ptr() -> *const u8 {
    engine().move_buf.as_ptr()
}

/// Look up a legal move matching (fx,fy,tx,ty[,promo]) and return it.
fn find_legal_move(board: &Board, fx: i32, fy: i32, tx: i32, ty: i32, promo: u8) -> Option<Move> {
    for m in board.generate_legal_moves(board.side_to_move) {
        if m.fx == fx && m.fy == fy && m.tx == tx && m.ty == ty {
            if m.promotion == 0 || m.promotion == promo {
                return Some(m);
            }
        }
    }
    None
}

/// Apply a human move if legal. Returns 1 on success, 0 if illegal.
#[no_mangle]
pub extern "C" fn ffce_make_move(fx: i32, fy: i32, tx: i32, ty: i32, promo: i32) -> i32 {
    let e = engine();
    let want_promo = if promo < 0 { 0u8 } else { promo as u8 };
    match find_legal_move(&e.board, fx, fy, tx, ty, want_promo) {
        Some(m) => {
            e.board.apply_move(&m);
            e.invalidate();
            1
        }
        None => 0,
    }
}

fn pack_move(m: &Move) -> i32 {
    (m.fx as i32)
        | ((m.fy as i32) << 3)
        | ((m.tx as i32) << 6)
        | ((m.ty as i32) << 9)
        | ((m.promotion as i32) << 12)
}

/// Compute and apply the flow-field AI's move for the side to move.
/// Returns the packed move (fx|fy<<3|tx<<6|ty<<9|promo<<12), or -1 if none.
#[no_mangle]
pub extern "C" fn ffce_ai_move(noise_sigma: f32) -> i32 {
    let e = engine();
    // Borrow split: choose_move needs &board + &mut rng + &params + &cw + &config.
    let chosen = {
        let board = &e.board;
        let rng = &mut e.rng;
        let params = &e.params;
        let cw = &e.cw;
        let config = &e.config;
        choose_move(board, rng, params, cw, config, noise_sigma)
    };
    match chosen {
        Some(m) => {
            e.board.apply_move(&m);
            e.invalidate();
            pack_move(&m)
        }
        None => -1,
    }
}

/// Game status: 0 ongoing, 1 White wins, 2 Black wins, 3 stalemate, 4 draw.
#[no_mangle]
pub extern "C" fn ffce_status() -> i32 {
    let e = engine();
    if e.board.halfmove >= 100 {
        return 4;
    }
    let moves = e.board.generate_legal_moves(e.board.side_to_move);
    if !moves.is_empty() {
        return 0;
    }
    if e.board.is_in_check(e.board.side_to_move) {
        // Side to move is checkmated; the other side wins.
        if e.board.side_to_move == WHITE {
            2
        } else {
            1
        }
    } else {
        3
    }
}

#[no_mangle]
pub extern "C" fn ffce_in_check(color: i32) -> i32 {
    let c = if color >= 0 { WHITE } else { BLACK };
    if engine().board.is_in_check(c) {
        1
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// Extras configuration (enable + 0..2 weight multiplier per algorithm).
// ---------------------------------------------------------------------------

/// Number of extra algorithms.
#[no_mangle]
pub extern "C" fn ffce_extra_count() -> i32 {
    extras::N_EXTRAS as i32
}

/// Set enable flag and weight multiplier for extra `idx` (0..12).
#[no_mangle]
pub extern "C" fn ffce_set_extra(idx: i32, enabled: i32, mult: f32) {
    let e = engine();
    if idx < 0 || idx as usize >= extras::N_EXTRAS {
        return;
    }
    let i = idx as usize;
    e.config.enabled[i] = enabled != 0;
    e.config.mult[i] = mult;
    e.invalidate();
}

/// Default (constants.py) evaluation weight for extra `idx`.
#[no_mangle]
pub extern "C" fn ffce_extra_default_weight(idx: i32) -> f32 {
    if idx < 0 || idx as usize >= extras::N_EXTRAS {
        return 0.0;
    }
    engine().params.extra_weight(idx as usize)
}

/// Whether extra `idx` is currently enabled.
#[no_mangle]
pub extern "C" fn ffce_extra_enabled(idx: i32) -> i32 {
    if idx < 0 || idx as usize >= extras::N_EXTRAS {
        return 0;
    }
    if engine().config.enabled[idx as usize] {
        1
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// Field overlays / histogram support.
// ---------------------------------------------------------------------------

/// Fill the overlay buffer with the field for `mode` (see eval::overlay_field)
/// and return a pointer to 64 f32 values (index = y*8 + x).
#[no_mangle]
pub extern "C" fn ffce_overlay(mode: i32, selected: i32) -> *const f32 {
    let e = engine();
    e.ensure_analysis();
    let an = e.analysis.as_ref().unwrap();
    let side = e.board.side_to_move;
    let grid = eval::overlay_field(an, &e.board, &e.params, &e.config, mode, selected, side);
    for y in 0..8usize {
        for x in 0..8usize {
            e.overlay_buf[y * 8 + x] = grid[y][x];
        }
    }
    e.overlay_buf.as_ptr()
}

/// Fill the overlay buffer with the raw field of extra `idx` (for the
/// histogram) and return a pointer to 64 f32 values.
#[no_mangle]
pub extern "C" fn ffce_extra_field(idx: i32) -> *const f32 {
    let e = engine();
    if idx < 0 || idx as usize >= extras::N_EXTRAS {
        for v in e.overlay_buf.iter_mut() {
            *v = 0.0;
        }
        return e.overlay_buf.as_ptr();
    }
    e.ensure_analysis();
    let an = e.analysis.as_ref().unwrap();
    let grid = extras::compute_extra_field(idx as usize, &e.board, &an.ctx(), &e.params);
    for y in 0..8usize {
        for x in 0..8usize {
            e.overlay_buf[y * 8 + x] = grid[y][x];
        }
    }
    e.overlay_buf.as_ptr()
}

/// Signed king-zone contribution of extra `idx` at the current position
/// (effective-weight * field_score) — used for the histogram ordering / sign.
#[no_mangle]
pub extern "C" fn ffce_extra_contribution(idx: i32) -> f32 {
    let e = engine();
    if idx < 0 || idx as usize >= extras::N_EXTRAS {
        return 0.0;
    }
    e.ensure_analysis();
    let an = e.analysis.as_ref().unwrap();
    let i = idx as usize;
    let field = extras::compute_extra_field(i, &e.board, &an.ctx(), &e.params);
    let w = e.params.extra_weight(i) * e.config.mult[i];
    w * extras::field_score(&field, &e.board, an.scale)
}
