//! Core field computation: Attack, Resistance, Trace, Future and Flow.
//!
//! Faithful port of the field-building half of `fields.py`. The scoring half
//! lives in `eval.rs`.

use crate::board::Board;
use crate::constants::*;
use crate::quaternion::forward_dir_xy;

pub type Grid = [[f32; 8]; 8];
pub type TimeField = Vec<Grid>;

pub fn grid_zero() -> Grid {
    [[0.0; 8]; 8]
}

pub fn tf_zero(t: usize) -> TimeField {
    vec![[[0.0; 8]; 8]; t]
}

/// 3x3 weighted diffusion stamp (port of `_diffuse2d`).
pub fn diffuse2d(f: &Grid, w0: f32, w1: f32, w2: f32) -> Grid {
    let mut out = grid_zero();
    for y in 0..8usize {
        for x in 0..8usize {
            let mut v = w0 * f[y][x];
            if y >= 1 {
                v += w1 * f[y - 1][x];
            }
            if y + 1 < 8 {
                v += w1 * f[y + 1][x];
            }
            if x >= 1 {
                v += w1 * f[y][x - 1];
            }
            if x + 1 < 8 {
                v += w1 * f[y][x + 1];
            }
            if y >= 1 && x >= 1 {
                v += w2 * f[y - 1][x - 1];
            }
            if y + 1 < 8 && x >= 1 {
                v += w2 * f[y + 1][x - 1];
            }
            if y >= 1 && x + 1 < 8 {
                v += w2 * f[y - 1][x + 1];
            }
            if y + 1 < 8 && x + 1 < 8 {
                v += w2 * f[y + 1][x + 1];
            }
            out[y][x] = v;
        }
    }
    out
}

pub fn king_zone_mask(kx: i32, ky: i32) -> Grid {
    let mut w = grid_zero();
    for y in 0..8i32 {
        for x in 0..8i32 {
            let dist = (x - kx).abs().max((y - ky).abs());
            if dist == 1 {
                w[y as usize][x as usize] = 1.0;
            } else if dist == 2 {
                w[y as usize][x as usize] = 0.5;
            }
        }
    }
    w
}

pub fn path_squares(fx: i32, fy: i32, tx: i32, ty: i32) -> Vec<(i32, i32)> {
    let mut squares = Vec::new();
    let dx = tx - fx;
    let dy = ty - fy;
    let step_x = if dx == 0 { 0 } else if dx > 0 { 1 } else { -1 };
    let step_y = if dy == 0 { 0 } else if dy > 0 { 1 } else { -1 };
    if dx == 0 || dy == 0 || dx.abs() == dy.abs() {
        let (mut x, mut y) = (fx + step_x, fy + step_y);
        loop {
            squares.push((x, y));
            if x == tx && y == ty {
                break;
            }
            x += step_x;
            y += step_y;
        }
    } else {
        squares.push((tx, ty));
    }
    squares
}

/// Weighted attack reach map for one side.
pub fn compute_attack(board: &Board, color: i32, cw: &Grid) -> Grid {
    let mut a = grid_zero();
    for p in &board.pieces {
        if !p.alive || p.color != color {
            continue;
        }
        let w = ATTACK_WEIGHTS[p.ptype as usize];
        match p.ptype {
            PAWN => {
                let diry = if color == WHITE { 1 } else { -1 };
                for dx in [-1, 1] {
                    let x = p.x + dx;
                    let y = p.y + diry;
                    if in_bounds(x, y) {
                        a[y as usize][x as usize] += w * cw[y as usize][x as usize];
                    }
                }
            }
            KNIGHT => {
                for (dx, dy) in KNIGHT_OFFSETS {
                    let (x, y) = (p.x + dx, p.y + dy);
                    if in_bounds(x, y) {
                        a[y as usize][x as usize] += w * cw[y as usize][x as usize];
                    }
                }
            }
            KING => {
                for (dx, dy) in KING_OFFSETS {
                    let (x, y) = (p.x + dx, p.y + dy);
                    if in_bounds(x, y) {
                        a[y as usize][x as usize] += w * cw[y as usize][x as usize];
                    }
                }
            }
            _ => {
                let dirs = slider_dirs(p.ptype);
                let decay = slider_decay(p.ptype);
                for &(dx, dy) in dirs {
                    let mut step = 1i32;
                    let (mut x, mut y) = (p.x + dx, p.y + dy);
                    while in_bounds(x, y) {
                        let target = board.board[y as usize][x as usize];
                        if target != 0 {
                            if board.piece(target).color != color {
                                a[y as usize][x as usize] += w * decay.powi(step - 1) * cw[y as usize][x as usize];
                            }
                            break;
                        }
                        a[y as usize][x as usize] += w * decay.powi(step - 1) * cw[y as usize][x as usize];
                        x += dx;
                        y += dy;
                        step += 1;
                    }
                }
            }
        }
    }
    a
}

/// Binary pawn attack map.
pub fn compute_pawn_attack_map(board: &Board, color: i32) -> Grid {
    let mut a = grid_zero();
    for p in &board.pieces {
        if !p.alive || p.color != color || p.ptype != PAWN {
            continue;
        }
        let diry = if color == WHITE { 1 } else { -1 };
        for dx in [-1, 1] {
            let (x, y) = (p.x + dx, p.y + diry);
            if in_bounds(x, y) {
                a[y as usize][x as usize] = 1.0;
            }
        }
    }
    a
}

/// Per-square list of attacking piece values (for SEE exchange evaluation).
pub fn compute_attack_lists(board: &Board, color: i32) -> Vec<Vec<Vec<f32>>> {
    let mut lists = vec![vec![Vec::<f32>::new(); 8]; 8];
    for p in &board.pieces {
        if !p.alive || p.color != color {
            continue;
        }
        let v = PIECE_VALUES[p.ptype as usize];
        match p.ptype {
            PAWN => {
                let diry = if color == WHITE { 1 } else { -1 };
                for dx in [-1, 1] {
                    let (x, y) = (p.x + dx, p.y + diry);
                    if in_bounds(x, y) {
                        lists[y as usize][x as usize].push(v);
                    }
                }
            }
            KNIGHT => {
                for (dx, dy) in KNIGHT_OFFSETS {
                    let (x, y) = (p.x + dx, p.y + dy);
                    if in_bounds(x, y) {
                        lists[y as usize][x as usize].push(v);
                    }
                }
            }
            KING => {
                for (dx, dy) in KING_OFFSETS {
                    let (x, y) = (p.x + dx, p.y + dy);
                    if in_bounds(x, y) {
                        lists[y as usize][x as usize].push(v);
                    }
                }
            }
            _ => {
                let dirs = slider_dirs(p.ptype);
                for &(dx, dy) in dirs {
                    let (mut x, mut y) = (p.x + dx, p.y + dy);
                    while in_bounds(x, y) {
                        lists[y as usize][x as usize].push(v);
                        if board.board[y as usize][x as usize] != 0 {
                            break;
                        }
                        x += dx;
                        y += dy;
                    }
                }
            }
        }
    }
    lists
}

/// Static-exchange-evaluation helper (port of `_see_exchange`).
pub fn see_exchange(target_value: f32, attackers: &[f32], defenders: &[f32]) -> f32 {
    if attackers.is_empty() {
        return 0.0;
    }
    let mut a = attackers.to_vec();
    a.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let mut d = defenders.to_vec();
    d.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let mut gain = vec![target_value];
    let mut side = 0;
    let mut ai = 0;
    let mut di = 0;
    let mut depth = 0usize;
    loop {
        let attacker_val;
        if side == 0 {
            if ai >= a.len() {
                break;
            }
            attacker_val = a[ai];
            ai += 1;
        } else {
            if di >= d.len() {
                break;
            }
            attacker_val = d[di];
            di += 1;
        }
        depth += 1;
        gain.push(attacker_val - gain[depth - 1]);
        if (-gain[depth - 1]).max(gain[depth]) < 0.0 {
            break;
        }
        side ^= 1;
    }
    let mut i = depth as i64 - 1;
    while i >= 0 {
        let iu = i as usize;
        gain[iu] = -((-gain[iu]).max(gain[iu + 1]));
        i -= 1;
    }
    gain[0]
}

/// Trace field: past path memory + forward intent with exponential decay.
pub fn compute_trace(board: &Board, color: i32, time_depth: usize, p: &Params) -> TimeField {
    let mut t = tf_zero(time_depth);
    let time_decay: Vec<f32> = (0..time_depth)
        .map(|i| (-(i as f32) / p.trace_tau).exp())
        .collect();
    let sigma = p.trace_sigma;
    for piece in &board.pieces {
        if !piece.alive || piece.color != color {
            continue;
        }
        let w = ATTACK_WEIGHTS[piece.ptype as usize];
        if let Some((fx, fy, tx, ty)) = piece.last_move {
            for (sx, sy) in path_squares(fx, fy, tx, ty) {
                if in_bounds(sx, sy) {
                    t[0][sy as usize][sx as usize] += p.trace_past * w;
                }
            }
        }
        let u_xy = forward_dir_xy(&piece.quat);
        let use_base = (u_xy[0] * u_xy[0] + u_xy[1] * u_xy[1]).sqrt() < 1e-3;
        let targets = piece_targets(board, piece, color, false);
        for (tx, ty) in targets {
            let dx = (tx - piece.x) as f32;
            let dy = (ty - piece.y) as f32;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < 1e-4 {
                continue;
            }
            let d_hat = [dx / dist, dy / dist];
            let mut align = u_xy[0] * d_hat[0] + u_xy[1] * d_hat[1];
            if use_base {
                align = align.max(p.trace_base_align);
            } else {
                align = align.max(0.0);
            }
            let value = p.trace_future * w * (-dist / sigma).exp() * align;
            if value <= 0.0 {
                continue;
            }
            for k in 0..time_depth {
                t[k][ty as usize][tx as usize] += value * time_decay[k];
            }
        }
    }
    t
}

/// Future presence map with history influence and intent alignment.
pub fn compute_future_map(board: &Board, color: i32, time_depth: usize, p: &Params) -> TimeField {
    let mut f = tf_zero(time_depth);
    let time_decay: Vec<f32> = (0..time_depth)
        .map(|i| (-(i as f32) / p.future_tau).exp())
        .collect();
    let sigma = p.future_sigma;
    let history = if color == WHITE { &board.history_w } else { &board.history_b };
    for k in 0..time_depth {
        for y in 0..8 {
            for x in 0..8 {
                f[k][y][x] += p.history_influence * history[y][x] * time_decay[k];
            }
        }
    }
    for piece in &board.pieces {
        if !piece.alive || piece.color != color {
            continue;
        }
        let w = PIECE_VALUES[piece.ptype as usize];
        f[0][piece.y as usize][piece.x as usize] += w;
        let u_xy = forward_dir_xy(&piece.quat);
        let use_base = (u_xy[0] * u_xy[0] + u_xy[1] * u_xy[1]).sqrt() < 1e-3;
        let targets = piece_targets(board, piece, color, true);
        for (tx, ty) in targets {
            let dx = (tx - piece.x) as f32;
            let dy = (ty - piece.y) as f32;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < 1e-4 {
                continue;
            }
            let d_hat = [dx / dist, dy / dist];
            let mut align = u_xy[0] * d_hat[0] + u_xy[1] * d_hat[1];
            if use_base {
                align = align.max(p.future_base_align);
            } else {
                align = align.max(0.0);
            }
            let value = w * (-dist / sigma).exp() * align;
            if value <= 0.0 {
                continue;
            }
            for k in 0..time_depth {
                f[k][ty as usize][tx as usize] += value * time_decay[k];
            }
        }
    }
    f
}

/// Reachable target squares for trace/future fields.
/// `include_forward_pawn` adds the pawn's single push square (future map only).
fn piece_targets(board: &Board, piece: &crate::board::Piece, color: i32, include_forward_pawn: bool) -> Vec<(i32, i32)> {
    let mut targets = Vec::new();
    match piece.ptype {
        PAWN => {
            let diry = if color == WHITE { 1 } else { -1 };
            if include_forward_pawn {
                let one_y = piece.y + diry;
                if in_bounds(piece.x, one_y) {
                    targets.push((piece.x, one_y));
                }
            }
            for dx in [-1, 1] {
                let (x, y) = (piece.x + dx, piece.y + diry);
                if in_bounds(x, y) {
                    targets.push((x, y));
                }
            }
        }
        KNIGHT => {
            for (dx, dy) in KNIGHT_OFFSETS {
                let (x, y) = (piece.x + dx, piece.y + dy);
                if in_bounds(x, y) {
                    targets.push((x, y));
                }
            }
        }
        KING => {
            for (dx, dy) in KING_OFFSETS {
                let (x, y) = (piece.x + dx, piece.y + dy);
                if in_bounds(x, y) {
                    targets.push((x, y));
                }
            }
        }
        _ => {
            let dirs = slider_dirs(piece.ptype);
            for &(dx, dy) in dirs {
                let (mut x, mut y) = (piece.x + dx, piece.y + dy);
                while in_bounds(x, y) {
                    if board.board[y as usize][x as usize] != 0 {
                        // Trace stops at enemy occupant; future includes the square.
                        if include_forward_pawn {
                            targets.push((x, y));
                        } else if board.piece(board.board[y as usize][x as usize]).color != color {
                            targets.push((x, y));
                        }
                        break;
                    }
                    targets.push((x, y));
                    x += dx;
                    y += dy;
                }
            }
        }
    }
    targets
}

/// Resistance field from occupancy, enemy pressure, pawn locks, king zone.
pub fn compute_resistance(board: &Board, color: i32, enemy_attack: &Grid, p: &Params) -> Grid {
    let mut occ = grid_zero();
    let mut pawn_lock = grid_zero();
    for piece in &board.pieces {
        if !piece.alive {
            continue;
        }
        occ[piece.y as usize][piece.x as usize] = 1.0;
        if piece.ptype == PAWN && piece.color == color {
            let diry = if color == WHITE { 1 } else { -1 };
            let by = piece.y + diry;
            if in_bounds(piece.x, by) && board.board[by as usize][piece.x as usize] != 0 {
                pawn_lock[piece.y as usize][piece.x as usize] = 1.0;
            }
        }
    }
    let king_zone = match board.find_king(color) {
        Some((kx, ky)) => king_zone_mask(kx, ky),
        None => grid_zero(),
    };
    let mut r = grid_zero();
    for y in 0..8 {
        for x in 0..8 {
            r[y][x] = p.res_occ * occ[y][x]
                + p.res_enemy * enemy_attack[y][x]
                + p.res_lock * pawn_lock[y][x]
                + p.res_king * king_zone[y][x];
        }
    }
    r
}

fn propagate(s: &TimeField, damp: &Grid, gamma: f32, w0: f32, w1: f32, w2: f32) -> TimeField {
    let t = s.len();
    let mut f = tf_zero(t);
    f[0] = s[0];
    for k in 0..t - 1 {
        let diff = diffuse2d(&f[k], w0, w1, w2);
        for y in 0..8 {
            for x in 0..8 {
                f[k + 1][y][x] = gamma * diff[y][x] * damp[y][x] + s[k + 1][y][x];
            }
        }
    }
    f
}

/// Solve the coupled flow fields by damped relaxation (port of `solve_fields`).
/// Returns (F_w, F_b, P) where P = F_w - F_b.
pub fn solve_fields(
    a_w: &Grid,
    a_b: &Grid,
    r_w: &Grid,
    r_b: &Grid,
    t_w: &TimeField,
    t_b: &TimeField,
    p: &Params,
) -> (TimeField, TimeField, TimeField) {
    let time_depth = t_w.len();
    let mut f_w = tf_zero(time_depth);
    let mut f_b = tf_zero(time_depth);
    let mut damp_w = grid_zero();
    let mut damp_b = grid_zero();
    for y in 0..8 {
        for x in 0..8 {
            damp_w[y][x] = 1.0 / (1.0 + r_w[y][x]);
            damp_b[y][x] = 1.0 / (1.0 + r_b[y][x]);
        }
    }
    let (gamma, w0, w1, w2) = (p.gamma, p.diff_w0, p.diff_w1, p.diff_w2);
    let mu = p.relax_mu;
    let beta_p = p.beta_p;

    for _ in 0..p.relax_iters {
        // P = F_w - F_b
        let mut s_w = tf_zero(time_depth);
        let mut s_b = tf_zero(time_depth);
        for k in 0..time_depth {
            for y in 0..8 {
                for x in 0..8 {
                    let pv = f_w[k][y][x] - f_b[k][y][x];
                    let tp = (beta_p * pv).tanh();
                    s_w[k][y][x] = a_w[y][x] + t_w[k][y][x] + p.trace_align * tp;
                    s_b[k][y][x] = a_b[y][x] + t_b[k][y][x] + p.trace_align * (beta_p * (-pv)).tanh();
                }
            }
        }
        let f_w_new = propagate(&s_w, &damp_w, gamma, w0, w1, w2);
        let f_b_new = propagate(&s_b, &damp_b, gamma, w0, w1, w2);
        let mut delta = 0.0f32;
        for k in 0..time_depth {
            for y in 0..8 {
                for x in 0..8 {
                    delta = delta.max((f_w_new[k][y][x] - f_w[k][y][x]).abs());
                    delta = delta.max((f_b_new[k][y][x] - f_b[k][y][x]).abs());
                }
            }
        }
        for k in 0..time_depth {
            for y in 0..8 {
                for x in 0..8 {
                    f_w[k][y][x] = (1.0 - mu) * f_w[k][y][x] + mu * f_w_new[k][y][x];
                    f_b[k][y][x] = (1.0 - mu) * f_b[k][y][x] + mu * f_b_new[k][y][x];
                }
            }
        }
        if delta < p.relax_eps {
            break;
        }
    }
    let mut press = tf_zero(time_depth);
    for k in 0..time_depth {
        for y in 0..8 {
            for x in 0..8 {
                press[k][y][x] = f_w[k][y][x] - f_b[k][y][x];
            }
        }
    }
    (f_w, f_b, press)
}

#[inline]
pub fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}
