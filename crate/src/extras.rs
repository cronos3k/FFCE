//! The 13 optional "extra" field generators (faithful port of `extras.py`).
//!
//! Each returns an 8x8 field derived from core signals; the score is the
//! king-zone-weighted sum of the enabled fields (see `compute_extras_score`).

use crate::board::Board;
use crate::constants::*;
use crate::fields::{grid_zero, king_zone_mask, Grid};

/// EXTRA_DEFS order (key, weight): index is the public algorithm id.
pub const N_EXTRAS: usize = 13;
pub const EXTRA_KEYS: [&str; N_EXTRAS] = [
    "reaction_diffusion",
    "cellular_automata",
    "resistor_network",
    "ising_spin",
    "wave_resonance",
    "lattice_boltzmann",
    "spectral_lowfreq",
    "hodge_curl",
    "ant_pheromone",
    "fuzzy_future",
    "topo_persistence",
    "latent_channels",
    "tensor_kernel",
];

/// Per-algorithm enable flag + 0..2 weight multiplier (the GUI slider).
#[derive(Clone)]
pub struct ExtrasConfig {
    pub enabled: [bool; N_EXTRAS],
    pub mult: [f32; N_EXTRAS],
}

impl Default for ExtrasConfig {
    fn default() -> Self {
        ExtrasConfig { enabled: [false; N_EXTRAS], mult: [1.0; N_EXTRAS] }
    }
}

/// Context fields the extras read (mirrors the ctx dict in fields.py).
pub struct ExtraCtx<'a> {
    pub mean_p: &'a Grid,
    pub a_w: &'a Grid,
    pub a_b: &'a Grid,
    pub r_w: &'a Grid,
    pub r_b: &'a Grid,
    pub mean_t_w: &'a Grid,
    pub mean_t_b: &'a Grid,
    pub scale: f32,
}

// ---------------------------------------------------------------------------
// Grid helpers
// ---------------------------------------------------------------------------

fn laplacian(f: &Grid) -> Grid {
    let mut out = grid_zero();
    for y in 0..8usize {
        for x in 0..8usize {
            let mut v = -4.0 * f[y][x];
            if y >= 1 { v += f[y - 1][x]; }
            if y + 1 < 8 { v += f[y + 1][x]; }
            if x >= 1 { v += f[y][x - 1]; }
            if x + 1 < 8 { v += f[y][x + 1]; }
            out[y][x] = v;
        }
    }
    out
}

fn smooth(f: &Grid) -> Grid {
    let mut out = grid_zero();
    for y in 0..8usize {
        for x in 0..8usize {
            let mut v = 0.4 * f[y][x];
            if y >= 1 { v += 0.15 * f[y - 1][x]; }
            if y + 1 < 8 { v += 0.15 * f[y + 1][x]; }
            if x >= 1 { v += 0.15 * f[y][x - 1]; }
            if x + 1 < 8 { v += 0.15 * f[y][x + 1]; }
            out[y][x] = v;
        }
    }
    out
}

/// np.roll along axis 0 (rows): out[y][x] = f[(y-sh) mod 8][x].
fn roll0(f: &Grid, sh: i32) -> Grid {
    let mut out = grid_zero();
    for y in 0..8i32 {
        let sy = (((y - sh) % 8) + 8) % 8;
        for x in 0..8usize {
            out[y as usize][x] = f[sy as usize][x];
        }
    }
    out
}

/// np.roll along axis 1 (cols): out[y][x] = f[y][(x-sh) mod 8].
fn roll1(f: &Grid, sh: i32) -> Grid {
    let mut out = grid_zero();
    for y in 0..8usize {
        for x in 0..8i32 {
            let sx = (((x - sh) % 8) + 8) % 8;
            out[y][x as usize] = f[y][sx as usize];
        }
    }
    out
}

fn source_map(board: &Board) -> Grid {
    let mut src = grid_zero();
    for p in &board.pieces {
        if !p.alive {
            continue;
        }
        let sign = if p.color == WHITE { 1.0 } else { -1.0 };
        src[p.y as usize][p.x as usize] += sign * PIECE_VALUES[p.ptype as usize];
    }
    src
}

fn king_masks(board: &Board) -> (Grid, Grid) {
    let wmask = match board.find_king(WHITE) {
        Some((x, y)) => king_zone_mask(x, y),
        None => grid_zero(),
    };
    let bmask = match board.find_king(BLACK) {
        Some((x, y)) => king_zone_mask(x, y),
        None => grid_zero(),
    };
    (wmask, bmask)
}

// Central-difference gradient (interior only; edges left at 0), matching the
// numpy slicing `g[:, 1:-1] = 0.5 * (f[:, 2:] - f[:, :-2])`.
fn grad_x(f: &Grid) -> Grid {
    let mut g = grid_zero();
    for y in 0..8usize {
        for x in 1..7usize {
            g[y][x] = 0.5 * (f[y][x + 1] - f[y][x - 1]);
        }
    }
    g
}
fn grad_y(f: &Grid) -> Grid {
    let mut g = grid_zero();
    for y in 1..7usize {
        for x in 0..8usize {
            g[y][x] = 0.5 * (f[y + 1][x] - f[y - 1][x]);
        }
    }
    g
}

// 7x7 Gaussian kernel (radius 3, sigma 2), normalized to max 1.
fn gaussian_kernel() -> [[f32; 7]; 7] {
    let radius = 3i32;
    let sigma = 2.0f32;
    let mut k = [[0.0f32; 7]; 7];
    let mut mx = 0.0f32;
    for y in 0..7i32 {
        for x in 0..7i32 {
            let dx = (x - radius) as f32;
            let dy = (y - radius) as f32;
            let v = (-(dx * dx + dy * dy) / (2.0 * sigma * sigma)).exp();
            k[y as usize][x as usize] = v;
            if v > mx { mx = v; }
        }
    }
    for y in 0..7 {
        for x in 0..7 {
            k[y][x] /= mx;
        }
    }
    k
}

fn add_kernel(field: &mut Grid, cx: i32, cy: i32, kernel: &[[f32; 7]; 7], scale: f32) {
    let radius = 3i32;
    for ky in 0..7i32 {
        for kx in 0..7i32 {
            let x = cx + (kx - radius);
            let y = cy + (ky - radius);
            if in_bounds(x, y) {
                field[y as usize][x as usize] += scale * kernel[ky as usize][kx as usize];
            }
        }
    }
}

fn add_kernel3(field: &mut Grid, cx: i32, cy: i32, kernel: &[[f32; 3]; 3], scale: f32) {
    for ky in 0..3i32 {
        for kx in 0..3i32 {
            let x = cx + (kx - 1);
            let y = cy + (ky - 1);
            if in_bounds(x, y) {
                field[y as usize][x as usize] += scale * kernel[ky as usize][kx as usize];
            }
        }
    }
}

fn tensor_kernel_for(t: u8) -> [[f32; 3]; 3] {
    match t {
        PAWN => [[0.0, 0.2, 0.0], [0.2, 0.4, 0.2], [0.0, 0.2, 0.0]],
        KNIGHT => [[0.1, 0.2, 0.1], [0.2, 0.4, 0.2], [0.1, 0.2, 0.1]],
        BISHOP => [[0.2, 0.0, 0.2], [0.0, 0.4, 0.0], [0.2, 0.0, 0.2]],
        ROOK => [[0.0, 0.2, 0.0], [0.2, 0.4, 0.2], [0.0, 0.2, 0.0]],
        QUEEN => [[0.2, 0.2, 0.2], [0.2, 0.5, 0.2], [0.2, 0.2, 0.2]],
        KING => [[0.1, 0.2, 0.1], [0.2, 0.3, 0.2], [0.1, 0.2, 0.1]],
        _ => [[0.0; 3]; 3],
    }
}

// Connected components (4-connectivity) that touch the zone mask -> 1.0 map.
fn components_touching_map(mask: &[[bool; 8]; 8], zone: &Grid) -> Grid {
    let mut visited = [[false; 8]; 8];
    let mut out = grid_zero();
    for sy in 0..8usize {
        for sx in 0..8usize {
            if !mask[sy][sx] || visited[sy][sx] {
                continue;
            }
            let mut stack = vec![(sx as i32, sy as i32)];
            visited[sy][sx] = true;
            let mut cells = Vec::new();
            let mut touches = false;
            while let Some((cx, cy)) = stack.pop() {
                cells.push((cx, cy));
                if zone[cy as usize][cx as usize] > 0.0 {
                    touches = true;
                }
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (cx + dx, cy + dy);
                    if in_bounds(nx, ny)
                        && mask[ny as usize][nx as usize]
                        && !visited[ny as usize][nx as usize]
                    {
                        visited[ny as usize][nx as usize] = true;
                        stack.push((nx, ny));
                    }
                }
            }
            if touches {
                for (cx, cy) in cells {
                    out[cy as usize][cx as usize] = 1.0;
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The 13 field generators
// ---------------------------------------------------------------------------

fn f_reaction_diffusion(_board: &Board, ctx: &ExtraCtx, p: &Params) -> Grid {
    let mut u = [[1.0f32; 8]; 8];
    let mut v = grid_zero();
    for y in 0..8 {
        for x in 0..8 {
            v[y][x] = (0.5 + 0.5 * ctx.mean_p[y][x].tanh()).clamp(0.0, 1.0);
        }
    }
    let (du, dv, feed, kill) = (0.16f32, 0.08f32, 0.035f32, 0.065f32);
    for _ in 0..p.rd_iters {
        let lu = laplacian(&u);
        let lv = laplacian(&v);
        for y in 0..8 {
            for x in 0..8 {
                let uvv = u[y][x] * v[y][x] * v[y][x];
                u[y][x] += du * lu[y][x] - uvv + feed * (1.0 - u[y][x]);
                v[y][x] += dv * lv[y][x] + uvv - (feed + kill) * v[y][x];
            }
        }
    }
    v
}

fn f_cellular_automata(_board: &Board, ctx: &ExtraCtx, p: &Params) -> Grid {
    let mut grid = [[false; 8]; 8];
    for y in 0..8 {
        for x in 0..8 {
            grid[y][x] = ctx.mean_p[y][x] > 0.0;
        }
    }
    for _ in 0..p.ca_iters {
        let mut n = [[0i32; 8]; 8];
        for y in 0..8i32 {
            for x in 0..8i32 {
                let mut c = 0;
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, 1), (1, -1), (-1, -1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if in_bounds(nx, ny) && grid[ny as usize][nx as usize] {
                        c += 1;
                    }
                }
                n[y as usize][x as usize] = c;
            }
        }
        let mut next = [[false; 8]; 8];
        for y in 0..8 {
            for x in 0..8 {
                next[y][x] = (grid[y][x] && n[y][x] == 2) || n[y][x] == 3;
            }
        }
        grid = next;
    }
    let mut out = grid_zero();
    for y in 0..8 {
        for x in 0..8 {
            out[y][x] = if grid[y][x] { 1.0 } else { 0.0 };
        }
    }
    out
}

fn f_resistor_network(board: &Board, ctx: &ExtraCtx, _p: &Params) -> Grid {
    let src = source_map(board);
    let mut resistance = grid_zero();
    for y in 0..8 {
        for x in 0..8 {
            resistance[y][x] = 0.5 * (ctx.r_w[y][x] + ctx.r_b[y][x]);
        }
    }
    let mut v = grid_zero();
    for _ in 0..8 {
        let up = roll0(&v, 1);
        let dn = roll0(&v, -1);
        let lf = roll1(&v, 1);
        let rt = roll1(&v, -1);
        for y in 0..8 {
            for x in 0..8 {
                let avg = 0.25 * (up[y][x] + dn[y][x] + lf[y][x] + rt[y][x]);
                v[y][x] = (avg + src[y][x]) / (1.0 + resistance[y][x]);
            }
        }
    }
    v
}

fn f_ising_spin(_board: &Board, ctx: &ExtraCtx, p: &Params) -> Grid {
    let mut s = grid_zero();
    for y in 0..8 {
        for x in 0..8 {
            s[y][x] = ctx.mean_p[y][x].tanh();
        }
    }
    let (beta, j) = (1.2f32, 0.8f32);
    for _ in 0..p.ising_iters {
        let up = roll0(&s, 1);
        let dn = roll0(&s, -1);
        let lf = roll1(&s, 1);
        let rt = roll1(&s, -1);
        for y in 0..8 {
            for x in 0..8 {
                let neighbor = up[y][x] + dn[y][x] + lf[y][x] + rt[y][x];
                s[y][x] = (beta * (j * neighbor + ctx.mean_p[y][x])).tanh();
            }
        }
    }
    s
}

fn f_wave_resonance(_board: &Board, ctx: &ExtraCtx, p: &Params) -> Grid {
    let mut source = grid_zero();
    for y in 0..8 {
        for x in 0..8 {
            source[y][x] = ctx.a_w[y][x] - ctx.a_b[y][x];
        }
    }
    let mut u_prev = grid_zero();
    let mut u = *ctx.mean_p;
    let (c2, damp) = (0.3f32, 0.08f32);
    for _ in 0..p.wave_iters {
        let lap = laplacian(&u);
        let mut u_next = grid_zero();
        for y in 0..8 {
            for x in 0..8 {
                u_next[y][x] = (2.0 - damp) * u[y][x] - (1.0 - damp) * u_prev[y][x]
                    + c2 * lap[y][x] + 0.1 * source[y][x];
            }
        }
        u_prev = u;
        u = u_next;
    }
    let mut energy = grid_zero();
    for y in 0..8 {
        for x in 0..8 {
            energy[y][x] = u[y][x].abs();
        }
    }
    energy
}

fn f_lattice_boltzmann(board: &Board, ctx: &ExtraCtx, p: &Params) -> Grid {
    let mut rho = source_map(board);
    let mut occ = grid_zero();
    for pc in &board.pieces {
        if pc.alive {
            occ[pc.y as usize][pc.x as usize] = 1.0;
        }
    }
    let vx = grad_x(ctx.mean_p);
    let vy = grad_y(ctx.mean_p);
    for _ in 0..p.lbm_iters {
        let gx = grad_x(&rho);
        let gy = grad_y(&rho);
        let lap = laplacian(&rho);
        for y in 0..8 {
            for x in 0..8 {
                let adv = -(vx[y][x] * gx[y][x] + vy[y][x] * gy[y][x]);
                rho[y][x] = rho[y][x] + 0.2 * adv + 0.1 * lap[y][x];
                rho[y][x] *= 1.0 - 0.6 * occ[y][x];
            }
        }
    }
    rho
}

fn f_spectral_lowfreq(_board: &Board, ctx: &ExtraCtx, p: &Params) -> Grid {
    let mut low = *ctx.mean_p;
    for _ in 0..p.spectral_iters {
        low = smooth(&low);
    }
    low
}

fn f_hodge_curl(_board: &Board, ctx: &ExtraCtx, _p: &Params) -> Grid {
    let lap = laplacian(ctx.mean_p);
    let mut out = grid_zero();
    for y in 0..8 {
        for x in 0..8 {
            out[y][x] = lap[y][x].abs();
        }
    }
    out
}

fn f_ant_pheromone(board: &Board, _ctx: &ExtraCtx, p: &Params) -> Grid {
    let mut pher = source_map(board);
    for _ in 0..p.ant_iters {
        let sm = smooth(&pher);
        for y in 0..8 {
            for x in 0..8 {
                pher[y][x] = 0.85 * pher[y][x] + 0.15 * sm[y][x];
            }
        }
    }
    pher
}

fn f_fuzzy_future(board: &Board, _ctx: &ExtraCtx, _p: &Params) -> Grid {
    let kernel = gaussian_kernel();
    let mut poss_w = grid_zero();
    let mut poss_b = grid_zero();
    for pc in &board.pieces {
        if !pc.alive {
            continue;
        }
        let dest = if pc.color == WHITE { &mut poss_w } else { &mut poss_b };
        add_kernel(dest, pc.x, pc.y, &kernel, PIECE_VALUES[pc.ptype as usize]);
    }
    let mut diff = grid_zero();
    for y in 0..8 {
        for x in 0..8 {
            diff[y][x] = poss_w[y][x] - poss_b[y][x];
        }
    }
    diff
}

fn f_topo_persistence(board: &Board, ctx: &ExtraCtx, _p: &Params) -> Grid {
    let (wmask, bmask) = king_masks(board);
    let mut pos = [[false; 8]; 8];
    let mut neg = [[false; 8]; 8];
    for y in 0..8 {
        for x in 0..8 {
            pos[y][x] = ctx.mean_p[y][x] > 0.3;
            neg[y][x] = ctx.mean_p[y][x] < -0.3;
        }
    }
    let pos_map = components_touching_map(&pos, &bmask);
    let neg_map = components_touching_map(&neg, &wmask);
    let mut out = grid_zero();
    for y in 0..8 {
        for x in 0..8 {
            out[y][x] = pos_map[y][x] - neg_map[y][x];
        }
    }
    out
}

fn f_latent_channels(board: &Board, ctx: &ExtraCtx, _p: &Params) -> Grid {
    let gx = grad_x(ctx.mean_p);
    let gy = grad_y(ctx.mean_p);
    let mut out = grid_zero();
    for y in 0..8 {
        for x in 0..8 {
            let hist = board.history_w[y][x] - board.history_b[y][x];
            let grad_mag = (gx[y][x] * gx[y][x] + gy[y][x] * gy[y][x]).sqrt();
            let h0 = (ctx.mean_p[y][x] + hist).tanh();
            let h1 = grad_mag.tanh();
            let h2 = (ctx.mean_t_w[y][x] - ctx.mean_t_b[y][x]).tanh();
            out[y][x] = h0 + h1 + h2;
        }
    }
    out
}

fn f_tensor_kernel(board: &Board, _ctx: &ExtraCtx, _p: &Params) -> Grid {
    let mut field = grid_zero();
    for pc in &board.pieces {
        if !pc.alive {
            continue;
        }
        let kernel = tensor_kernel_for(pc.ptype);
        let sign = if pc.color == WHITE { 1.0 } else { -1.0 };
        add_kernel3(&mut field, pc.x, pc.y, &kernel, sign * PIECE_VALUES[pc.ptype as usize]);
    }
    field
}

/// Compute a single extra field by index (0..12).
pub fn compute_extra_field(idx: usize, board: &Board, ctx: &ExtraCtx, p: &Params) -> Grid {
    match idx {
        0 => f_reaction_diffusion(board, ctx, p),
        1 => f_cellular_automata(board, ctx, p),
        2 => f_resistor_network(board, ctx, p),
        3 => f_ising_spin(board, ctx, p),
        4 => f_wave_resonance(board, ctx, p),
        5 => f_lattice_boltzmann(board, ctx, p),
        6 => f_spectral_lowfreq(board, ctx, p),
        7 => f_hodge_curl(board, ctx, p),
        8 => f_ant_pheromone(board, ctx, p),
        9 => f_fuzzy_future(board, ctx, p),
        10 => f_topo_persistence(board, ctx, p),
        11 => f_latent_channels(board, ctx, p),
        12 => f_tensor_kernel(board, ctx, p),
        _ => grid_zero(),
    }
}

/// king-zone-weighted scalar score for a field (port of `_field_score`).
pub fn field_score(field: &Grid, board: &Board, scale: f32) -> f32 {
    let (wmask, bmask) = king_masks(board);
    let mut sb = 0.0f32;
    let mut sw = 0.0f32;
    for y in 0..8 {
        for x in 0..8 {
            sb += field[y][x] * bmask[y][x];
            sw += field[y][x] * wmask[y][x];
        }
    }
    scale * (sb - sw)
}

/// Sum of enabled extras' contributions (port of `compute_extras_score`).
/// Effective weight = default eval weight * slider multiplier.
pub fn compute_extras_score(board: &Board, ctx: &ExtraCtx, params: &Params, config: &ExtrasConfig) -> f32 {
    let mut total = 0.0f32;
    for i in 0..N_EXTRAS {
        if !config.enabled[i] {
            continue;
        }
        let field = compute_extra_field(i, board, ctx, params);
        let w = params.extra_weight(i) * config.mult[i];
        total += w * field_score(&field, board, ctx.scale);
    }
    total
}
