//! Position scoring: aggregates the fields into a single scalar.
//!
//! Faithful port of `compute_score` / `evaluate_position` from `fields.py`,
//! including the blend of the enabled `extras.py` algorithms.

use crate::board::Board;
use crate::constants::*;
use crate::extras::{self, ExtraCtx, ExtrasConfig};
use crate::fields::*;

const INITIAL_MATERIAL: f32 = 2.0 * (8.0 * 1.0 + 2.0 * 3.2 + 2.0 * 3.3 + 2.0 * 5.0 + 9.0);

fn endgame_phase(board: &Board) -> f32 {
    let mut total = 0.0;
    for p in &board.pieces {
        if !p.alive || p.ptype == KING {
            continue;
        }
        total += PIECE_VALUES[p.ptype as usize];
    }
    if INITIAL_MATERIAL <= 1e-6 {
        return 0.0;
    }
    let phase = 1.0 - (total / INITIAL_MATERIAL).min(1.0);
    phase.clamp(0.0, 1.0)
}

fn piece_mobility(board: &Board, piece: &crate::board::Piece) -> f32 {
    let mut count = 0i32;
    let (x, y) = (piece.x, piece.y);
    let color = piece.color;
    if piece.ptype == KNIGHT {
        for (dx, dy) in KNIGHT_OFFSETS {
            let (nx, ny) = (x + dx, y + dy);
            if in_bounds(nx, ny) {
                let target = board.board[ny as usize][nx as usize];
                if target == 0 || board.piece(target).color != color {
                    count += 1;
                }
            }
        }
    } else if piece.ptype == BISHOP || piece.ptype == ROOK || piece.ptype == QUEEN {
        let dirs = slider_dirs(piece.ptype);
        for &(dx, dy) in dirs {
            let (mut nx, mut ny) = (x + dx, y + dy);
            while in_bounds(nx, ny) {
                count += 1;
                if board.board[ny as usize][nx as usize] != 0 {
                    break;
                }
                nx += dx;
                ny += dy;
            }
        }
    }
    count as f32
}

fn capacity(board: &Board, color: i32, damp: &Grid, p0: &Grid) -> f32 {
    let mut total = 0.0f32;
    for piece in &board.pieces {
        if !piece.alive || piece.color != color {
            continue;
        }
        let w = ATTACK_WEIGHTS[piece.ptype as usize];
        let mob = MOBILITY_WEIGHTS[piece.ptype as usize];
        let mut targets: Vec<(i32, i32, f32)> = Vec::new();
        match piece.ptype {
            PAWN => {
                let diry = if color == WHITE { 1 } else { -1 };
                let x = piece.x;
                let y = piece.y + diry;
                if in_bounds(x, y) && board.board[y as usize][x as usize] == 0 {
                    targets.push((x, y, 1.0));
                }
                for dx in [-1, 1] {
                    let (cx, cy) = (piece.x + dx, piece.y + diry);
                    if in_bounds(cx, cy) {
                        let target = board.board[cy as usize][cx as usize];
                        if target != 0 && board.piece(target).color != color {
                            targets.push((cx, cy, 0.7));
                        }
                    }
                }
            }
            KNIGHT => {
                for (dx, dy) in KNIGHT_OFFSETS {
                    let (x, y) = (piece.x + dx, piece.y + dy);
                    if in_bounds(x, y) {
                        let target = board.board[y as usize][x as usize];
                        if target == 0 || board.piece(target).color != color {
                            targets.push((x, y, 1.0));
                        }
                    }
                }
            }
            KING => {
                for (dx, dy) in KING_OFFSETS {
                    let (x, y) = (piece.x + dx, piece.y + dy);
                    if in_bounds(x, y) {
                        let target = board.board[y as usize][x as usize];
                        if target == 0 || board.piece(target).color != color {
                            targets.push((x, y, 0.8));
                        }
                    }
                }
            }
            _ => {
                let dirs = slider_dirs(piece.ptype);
                let bonus_step = mobility_step_bonus(piece.ptype);
                for &(dx, dy) in dirs {
                    let (mut x, mut y) = (piece.x + dx, piece.y + dy);
                    let mut step = 1i32;
                    while in_bounds(x, y) {
                        if board.board[y as usize][x as usize] != 0 {
                            if board.piece(board.board[y as usize][x as usize]).color != color {
                                targets.push((x, y, 1.0 + bonus_step * step as f32));
                            }
                            break;
                        }
                        targets.push((x, y, 1.0 + bonus_step * step as f32));
                        x += dx;
                        y += dy;
                        step += 1;
                    }
                }
            }
        }
        for (tx, ty, bonus) in targets {
            total += w * mob * bonus * damp[ty as usize][tx as usize]
                * (1.0 + 0.4 * sigmoid(p0[ty as usize][tx as usize]));
        }
    }
    total
}

/// All fields needed by scoring and the UI overlays.
pub struct Fields {
    pub a_w: Grid,
    pub a_b: Grid,
    pub r_w: Grid,
    pub r_b: Grid,
    pub t_w: TimeField,
    pub t_b: TimeField,
    pub f_w: TimeField,
    pub f_b: TimeField,
    pub p: TimeField,
}

pub fn compute_fields(board: &Board, params: &Params, cw: &Grid) -> Fields {
    let time_depth = params.time_depth;
    let a_w = compute_attack(board, WHITE, cw);
    let a_b = compute_attack(board, BLACK, cw);
    let r_w = compute_resistance(board, WHITE, &a_b, params);
    let r_b = compute_resistance(board, BLACK, &a_w, params);
    let t_w = compute_trace(board, WHITE, time_depth, params);
    let t_b = compute_trace(board, BLACK, time_depth, params);
    let (f_w, f_b, p) = solve_fields(&a_w, &a_b, &r_w, &r_b, &t_w, &t_b, params);
    Fields { a_w, a_b, r_w, r_b, t_w, t_b, f_w, f_b, p }
}

pub fn compute_score(
    board: &Board,
    fld: &Fields,
    params: &Params,
    cw: &Grid,
    config: &ExtrasConfig,
) -> f32 {
    let time_depth = fld.p.len();
    let (a_w, a_b, r_w, r_b) = (&fld.a_w, &fld.a_b, &fld.r_w, &fld.r_b);
    let (t_w, t_b, f_w, f_b, p) = (&fld.t_w, &fld.t_b, &fld.f_w, &fld.f_b, &fld.p);

    // Material.
    let mut material = 0.0f32;
    for piece in &board.pieces {
        if !piece.alive {
            continue;
        }
        material += PIECE_VALUES[piece.ptype as usize] * if piece.color == WHITE { 1.0 } else { -1.0 };
    }

    let time_weights: Vec<f32> = (0..time_depth).map(|i| params.beta_time.powi(i as i32)).collect();
    let sum_tw: f32 = time_weights.iter().sum();
    let scale = (1.0 / sum_tw) * (1.0 / (BOARD_SIZE * BOARD_SIZE) as f32);
    let beta_p = params.beta_p;

    // Weighted means over time.
    let mut mean_p = grid_zero();
    for k in 0..time_depth {
        for y in 0..8 {
            for x in 0..8 {
                mean_p[y][x] += time_weights[k] * p[k][y][x];
            }
        }
    }
    for y in 0..8 {
        for x in 0..8 {
            mean_p[y][x] /= sum_tw;
        }
    }

    // ctrl, trace, tres, potential, king.
    let king_w = board.find_king(WHITE);
    let king_b = board.find_king(BLACK);
    let king_white = king_w.map(|(x, y)| king_zone_mask(x, y)).unwrap_or_else(grid_zero);
    let king_black = king_b.map(|(x, y)| king_zone_mask(x, y)).unwrap_or_else(grid_zero);

    let mut ctrl = 0.0f32;
    let mut trace_score = 0.0f32;
    let mut tres = 0.0f32;
    let mut potential_score = 0.0f32;
    let mut king_score = 0.0f32;
    let mut mean_t_w = grid_zero();
    let mut mean_t_b = grid_zero();
    for k in 0..time_depth {
        let tw = time_weights[k];
        for y in 0..8 {
            for x in 0..8 {
                let tanh_p = (beta_p * p[k][y][x]).tanh();
                ctrl += tw * cw[y][x] * tanh_p;
                trace_score += tw * (t_w[k][y][x] - t_b[k][y][x]) * tanh_p * cw[y][x];
                tres += tw * (t_w[k][y][x] * r_w[y][x] - t_b[k][y][x] * r_b[y][x]);
                potential_score += tw * (f_w[k][y][x] - f_b[k][y][x]);
                king_score += tw * (king_black[y][x] * p[k][y][x] - king_white[y][x] * p[k][y][x]);
                mean_t_w[y][x] += tw * t_w[k][y][x];
                mean_t_b[y][x] += tw * t_b[k][y][x];
            }
        }
    }
    for y in 0..8 {
        for x in 0..8 {
            mean_t_w[y][x] /= sum_tw;
            mean_t_b[y][x] /= sum_tw;
        }
    }
    ctrl *= scale;
    trace_score *= scale;
    tres *= scale;
    potential_score *= scale;
    king_score *= scale;

    // Capacity (mobility through damping, biased by first-slice pressure).
    let mut damp_w = grid_zero();
    let mut damp_b = grid_zero();
    let mut p0_w = grid_zero();
    let mut p0_b = grid_zero();
    for y in 0..8 {
        for x in 0..8 {
            damp_w[y][x] = 1.0 / (1.0 + r_w[y][x]);
            damp_b[y][x] = 1.0 / (1.0 + r_b[y][x]);
            p0_w[y][x] = p[0][y][x];
            p0_b[y][x] = -p[0][y][x];
        }
    }
    let cap = capacity(board, WHITE, &damp_w, &p0_w) - capacity(board, BLACK, &damp_b, &p0_b);

    // SEE / future-presence per-piece terms.
    let attack_w = compute_attack_lists(board, WHITE);
    let attack_b = compute_attack_lists(board, BLACK);
    let pawn_w = compute_pawn_attack_map(board, WHITE);
    let pawn_b = compute_pawn_attack_map(board, BLACK);
    let future_w = compute_future_map(board, WHITE, time_depth, params);
    let future_b = compute_future_map(board, BLACK, time_depth, params);

    let mut future_w_mean = grid_zero();
    let mut future_b_mean = grid_zero();
    for k in 0..time_depth {
        for y in 0..8 {
            for x in 0..8 {
                future_w_mean[y][x] += time_weights[k] * future_w[k][y][x];
                future_b_mean[y][x] += time_weights[k] * future_b[k][y][x];
            }
        }
    }
    for y in 0..8 {
        for x in 0..8 {
            future_w_mean[y][x] /= sum_tw;
            future_b_mean[y][x] /= sum_tw;
        }
    }

    // Trap / ambush.
    let mut trap_w = 0.0f32;
    let mut trap_b = 0.0f32;
    for k in 0..time_depth {
        for y in 0..8 {
            for x in 0..8 {
                let empty = if board.board[y][x] == 0 { 1.0 } else { 0.0 };
                let ambush = (beta_p * mean_p[y][x]).tanh() * empty;
                trap_w += time_weights[k] * future_b[k][y][x] * ambush;
                trap_b += time_weights[k] * future_w[k][y][x] * (-ambush);
            }
        }
    }
    let trap_score = scale * (trap_w - trap_b);

    let endgame = endgame_phase(board);
    let floor = params.target_attack_floor;
    let mut safety = 0.0f32;
    let mut target_pressure = 0.0f32;
    let mut capture_score = 0.0f32;
    let mut pawn_strike = 0.0f32;
    let mut future_threat = 0.0f32;
    for piece in &board.pieces {
        if !piece.alive {
            continue;
        }
        let (x, y) = (piece.x as usize, piece.y as usize);
        let value = PIECE_VALUES[piece.ptype as usize];
        if piece.color == WHITE {
            let attackers = &attack_b[y][x];
            let defenders = &attack_w[y][x];
            let gain = see_exchange(value, attackers, defenders);
            if gain > 0.0 {
                safety -= gain;
            }
            let attack_factor = floor + (1.0 - floor) * (attackers.len() as f32 / 2.0).min(1.0);
            let pressure = (beta_p * mean_p[y][x]).tanh();
            target_pressure -= value * pressure * attack_factor;
            let threat = (future_b_mean[y][x] - future_w_mean[y][x]).max(0.0);
            future_threat -= value * threat;
            if !attackers.is_empty() {
                let cap_gain = see_exchange(value, attackers, defenders);
                if cap_gain > 0.0 {
                    capture_score -= cap_gain;
                }
            }
            let future_adv = future_b_mean[y][x] - future_w_mean[y][x];
            if future_adv > params.future_adv_floor {
                capture_score -= params.future_capture_scale * value * future_adv;
                if pawn_b[y][x] > 0.0 {
                    pawn_strike -= value * future_adv;
                }
            }
        } else {
            let attackers = &attack_w[y][x];
            let defenders = &attack_b[y][x];
            let gain = see_exchange(value, attackers, defenders);
            if gain > 0.0 {
                safety += gain;
            }
            let attack_factor = floor + (1.0 - floor) * (attackers.len() as f32 / 2.0).min(1.0);
            let pressure = (beta_p * mean_p[y][x]).tanh();
            target_pressure += value * pressure * attack_factor;
            let threat = (future_w_mean[y][x] - future_b_mean[y][x]).max(0.0);
            future_threat += value * threat;
            if !attackers.is_empty() {
                let cap_gain = see_exchange(value, attackers, defenders);
                if cap_gain > 0.0 {
                    capture_score += cap_gain;
                }
            }
            let future_adv = future_w_mean[y][x] - future_b_mean[y][x];
            if future_adv > params.future_adv_floor {
                capture_score += params.future_capture_scale * value * future_adv;
                if pawn_w[y][x] > 0.0 {
                    pawn_strike += value * future_adv;
                }
            }
        }
    }

    // Pawn structure / officer activity.
    let mut pawn_gravity = 0.0f32;
    let mut pawn_promo = 0.0f32;
    let mut officer_activity = 0.0f32;
    for piece in &board.pieces {
        if !piece.alive {
            continue;
        }
        let sign = if piece.color == WHITE { 1.0 } else { -1.0 };
        if piece.ptype == PAWN {
            let diry = if piece.color == WHITE { 1 } else { -1 };
            let dist = if piece.color == WHITE { 7 - piece.y } else { piece.y };
            let progress = ((7 - dist) as f32 / 7.0).clamp(0.0, 1.0);
            pawn_gravity += sign * progress.powf(1.35);
            let mut path_clear = true;
            let mut risk = 0.0f32;
            for step in 1..=dist {
                let ny = piece.y + diry * step;
                if !in_bounds(piece.x, ny) {
                    break;
                }
                if board.board[ny as usize][piece.x as usize] != 0 {
                    path_clear = false;
                    break;
                }
                if piece.color == WHITE {
                    risk += a_b[ny as usize][piece.x as usize];
                } else {
                    risk += a_w[ny as usize][piece.x as usize];
                }
            }
            let mut passed = true;
            if piece.color == WHITE {
                for ny in (piece.y + 1)..8 {
                    let target = board.board[ny as usize][piece.x as usize];
                    if target != 0 {
                        let tp = board.piece(target);
                        if tp.alive && tp.color == BLACK && tp.ptype == PAWN {
                            passed = false;
                            break;
                        }
                    }
                }
            } else {
                let mut ny = piece.y - 1;
                while ny >= 0 {
                    let target = board.board[ny as usize][piece.x as usize];
                    if target != 0 {
                        let tp = board.piece(target);
                        if tp.alive && tp.color == WHITE && tp.ptype == PAWN {
                            passed = false;
                            break;
                        }
                    }
                    ny -= 1;
                }
            }
            let safety_factor = 1.0 / (1.0 + 0.25 * risk);
            let clear_factor = if path_clear { 1.0 } else { 0.35 };
            let passed_factor = if passed { 1.0 } else { 0.6 };
            let promo_potential = clear_factor * passed_factor * safety_factor * progress.powf(1.7);
            pawn_promo += sign * (PIECE_VALUES[QUEEN as usize] - PIECE_VALUES[PAWN as usize]) * promo_potential;
        } else if piece.ptype == KNIGHT || piece.ptype == BISHOP || piece.ptype == ROOK || piece.ptype == QUEEN {
            let mobility = piece_mobility(board, piece);
            officer_activity += sign * mobility * (PIECE_VALUES[piece.ptype as usize] / 10.0);
        }
    }
    pawn_gravity *= 0.4 + 0.6 * endgame;
    pawn_promo *= 0.3 + 0.7 * endgame;
    officer_activity *= 0.4 + 0.6 * endgame;

    // Extras blend (compute_extras_score): king-zone-weighted sum of enabled fields.
    let ctx = ExtraCtx {
        mean_p: &mean_p,
        a_w,
        a_b,
        r_w,
        r_b,
        mean_t_w: &mean_t_w,
        mean_t_b: &mean_t_b,
        scale,
    };
    let extras_score = extras::compute_extras_score(board, &ctx, params, config);

    params.eval_mat * material
        + params.eval_ctrl * ctrl
        + params.eval_king * king_score
        + params.eval_trace * trace_score
        - params.eval_tres * tres
        + params.eval_cap * cap
        + params.eval_hang * safety
        + params.eval_potential * potential_score
        + params.eval_target * target_pressure
        + params.eval_trap * trap_score
        + params.eval_capture * capture_score
        + params.eval_pawn_strike * pawn_strike
        + params.eval_future_threat * future_threat
        + params.eval_pawn_gravity * pawn_gravity
        + params.eval_pawn_promo * pawn_promo
        + params.eval_officer_activity * officer_activity
        + extras_score
}

pub fn evaluate_position(board: &Board, params: &Params, cw: &Grid, config: &ExtrasConfig) -> f32 {
    let fld = compute_fields(board, params, cw);
    compute_score(board, &fld, params, cw, config)
}

// ---------------------------------------------------------------------------
// Overlay / analysis support (for the UI field visualizations + histogram).
// ---------------------------------------------------------------------------

/// Cached fields + time-means for the current root position; reused by the
/// overlay and histogram queries so core fields are solved only once per frame.
pub struct Analysis {
    pub p0: Grid,
    pub mean_p: Grid,
    pub a_w: Grid,
    pub a_b: Grid,
    pub r_w: Grid,
    pub r_b: Grid,
    pub t_w0: Grid,
    pub t_b0: Grid,
    pub f_w0: Grid,
    pub f_b0: Grid,
    pub mean_t_w: Grid,
    pub mean_t_b: Grid,
    pub scale: f32,
}

impl Analysis {
    pub fn ctx(&self) -> ExtraCtx<'_> {
        ExtraCtx {
            mean_p: &self.mean_p,
            a_w: &self.a_w,
            a_b: &self.a_b,
            r_w: &self.r_w,
            r_b: &self.r_b,
            mean_t_w: &self.mean_t_w,
            mean_t_b: &self.mean_t_b,
            scale: self.scale,
        }
    }
}

pub fn analyze(board: &Board, params: &Params, cw: &Grid) -> Analysis {
    let fld = compute_fields(board, params, cw);
    let time_depth = fld.p.len();
    let time_weights: Vec<f32> = (0..time_depth).map(|i| params.beta_time.powi(i as i32)).collect();
    let sum_tw: f32 = time_weights.iter().sum();
    let scale = (1.0 / sum_tw) * (1.0 / (BOARD_SIZE * BOARD_SIZE) as f32);

    let mut mean_p = grid_zero();
    let mut mean_t_w = grid_zero();
    let mut mean_t_b = grid_zero();
    for k in 0..time_depth {
        let tw = time_weights[k];
        for y in 0..8 {
            for x in 0..8 {
                mean_p[y][x] += tw * fld.p[k][y][x];
                mean_t_w[y][x] += tw * fld.t_w[k][y][x];
                mean_t_b[y][x] += tw * fld.t_b[k][y][x];
            }
        }
    }
    for y in 0..8 {
        for x in 0..8 {
            mean_p[y][x] /= sum_tw;
            mean_t_w[y][x] /= sum_tw;
            mean_t_b[y][x] /= sum_tw;
        }
    }

    Analysis {
        p0: fld.p[0],
        mean_p,
        a_w: fld.a_w,
        a_b: fld.a_b,
        r_w: fld.r_w,
        r_b: fld.r_b,
        t_w0: fld.t_w[0],
        t_b0: fld.t_b[0],
        f_w0: fld.f_w[0],
        f_b0: fld.f_b[0],
        mean_t_w,
        mean_t_b,
        scale,
    }
}

/// Build the overlay field for a given mode at the current root position.
/// Modes (match the original keys 2..7): 1 net-pressure, 2 resistance,
/// 3 trace, 4 attack, 5 extras-sum, 6 selected-extra, 7 flow.
/// `side` is the side to move (1 white, -1 black); single-side fields follow it.
pub fn overlay_field(
    an: &Analysis,
    board: &Board,
    params: &Params,
    config: &ExtrasConfig,
    mode: i32,
    selected: i32,
    side: i32,
) -> Grid {
    match mode {
        1 => an.p0,
        2 => if side == WHITE { an.r_w } else { an.r_b },
        3 => if side == WHITE { an.t_w0 } else { an.t_b0 },
        4 => if side == WHITE { an.a_w } else { an.a_b },
        5 => {
            // extras-sum: sum of enabled (effective-weight * field).
            let ctx = an.ctx();
            let mut total = grid_zero();
            for i in 0..extras::N_EXTRAS {
                if !config.enabled[i] {
                    continue;
                }
                let field = extras::compute_extra_field(i, board, &ctx, params);
                let w = params.extra_weight(i) * config.mult[i];
                for y in 0..8 {
                    for x in 0..8 {
                        total[y][x] += w * field[y][x];
                    }
                }
            }
            total
        }
        6 => {
            let i = selected.clamp(0, extras::N_EXTRAS as i32 - 1) as usize;
            extras::compute_extra_field(i, board, &an.ctx(), params)
        }
        7 => {
            let mut d = grid_zero();
            for y in 0..8 {
                for x in 0..8 {
                    d[y][x] = an.f_w0[y][x] - an.f_b0[y][x];
                }
            }
            d
        }
        _ => grid_zero(),
    }
}
