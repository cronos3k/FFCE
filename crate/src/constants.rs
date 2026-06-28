//! Board constants, piece values, and tunable evaluation parameters.
//!
//! Mirrors the reference `constants.py` from the Python FFCE project so the
//! ported fields and evaluation behave faithfully.

pub const BOARD_SIZE: usize = 8;

pub const WHITE: i32 = 1;
pub const BLACK: i32 = -1;

pub const EMPTY: u8 = 0;
pub const PAWN: u8 = 1;
pub const KNIGHT: u8 = 2;
pub const BISHOP: u8 = 3;
pub const ROOK: u8 = 4;
pub const QUEEN: u8 = 5;
pub const KING: u8 = 6;

// Castling rights bitflags.
pub const CASTLE_WK: u8 = 1;
pub const CASTLE_WQ: u8 = 2;
pub const CASTLE_BK: u8 = 4;
pub const CASTLE_BQ: u8 = 8;

/// Material values indexed by piece type (index 0 unused).
pub const PIECE_VALUES: [f32; 7] = [0.0, 1.0, 3.2, 3.3, 5.0, 9.0, 0.0];

/// Attack-map weights indexed by piece type.
pub const ATTACK_WEIGHTS: [f32; 7] = [0.0, 1.2, 3.4, 3.4, 5.2, 9.4, 2.0];

/// Mobility weights indexed by piece type.
pub const MOBILITY_WEIGHTS: [f32; 7] = [0.0, 0.6, 1.0, 1.1, 1.2, 1.4, 0.5];

pub const SLIDER_DECAY_DEFAULT: f32 = 0.9;

/// Per-piece slider decay (bishop/rook/queen). Others fall back to default.
pub fn slider_decay(t: u8) -> f32 {
    match t {
        BISHOP => 0.9,
        ROOK => 0.92,
        QUEEN => 0.9,
        _ => SLIDER_DECAY_DEFAULT,
    }
}

/// Per-piece mobility step bonus for sliders.
pub fn mobility_step_bonus(t: u8) -> f32 {
    match t {
        BISHOP => 0.06,
        ROOK => 0.08,
        QUEEN => 0.07,
        _ => 0.0,
    }
}

pub const KNIGHT_OFFSETS: [(i32, i32); 8] = [
    (1, 2), (2, 1), (-1, 2), (-2, 1),
    (1, -2), (2, -1), (-1, -2), (-2, -1),
];

pub const KING_OFFSETS: [(i32, i32); 8] = [
    (1, 1), (1, 0), (1, -1),
    (0, 1), (0, -1),
    (-1, 1), (-1, 0), (-1, -1),
];

pub const BISHOP_DIRS: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
pub const ROOK_DIRS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
pub const QUEEN_DIRS: [(i32, i32); 8] = [
    (1, 1), (1, -1), (-1, 1), (-1, -1),
    (1, 0), (-1, 0), (0, 1), (0, -1),
];

/// Return the move directions for a sliding piece type.
pub fn slider_dirs(t: u8) -> &'static [(i32, i32)] {
    match t {
        BISHOP => &BISHOP_DIRS,
        ROOK => &ROOK_DIRS,
        _ => &QUEEN_DIRS,
    }
}

#[inline]
pub fn in_bounds(x: i32, y: i32) -> bool {
    x >= 0 && x < BOARD_SIZE as i32 && y >= 0 && y < BOARD_SIZE as i32
}

/// Center-weight map (mirrors `_center_weights` in constants.py).
pub fn center_weight() -> [[f32; 8]; 8] {
    let mut w = [[0.0f32; 8]; 8];
    for y in 0..8 {
        for x in 0..8 {
            let dx = (3.5 - x as f32).abs();
            let dy = (3.5 - y as f32).abs();
            let dist = dx + dy;
            w[y][x] = 1.0 + 0.15 * (4.0 - dist);
        }
    }
    w
}

/// Tunable parameters; mirrors the `PARAMS` dict (subset used by the v1 port).
#[derive(Clone)]
pub struct Params {
    pub time_depth: usize,
    pub gamma: f32,
    pub relax_mu: f32,
    pub relax_iters: usize,
    pub relax_eps: f32,
    pub diff_w0: f32,
    pub diff_w1: f32,
    pub diff_w2: f32,
    pub trace_past: f32,
    pub trace_future: f32,
    pub trace_tau: f32,
    pub trace_sigma: f32,
    pub trace_align: f32,
    pub trace_base_align: f32,
    pub quat_alpha: f32,
    pub quat_z0: f32,
    pub res_occ: f32,
    pub res_enemy: f32,
    pub res_lock: f32,
    pub res_king: f32,
    pub beta_p: f32,
    pub beta_time: f32,
    pub noise_start: f32,
    pub history_decay: f32,
    pub history_weight: f32,
    pub history_influence: f32,
    pub future_tau: f32,
    pub future_sigma: f32,
    pub future_base_align: f32,
    pub target_attack_floor: f32,
    pub future_adv_floor: f32,
    pub future_capture_scale: f32,
    // Evaluation term weights (these are the ones perturbed by noise).
    pub eval_mat: f32,
    pub eval_ctrl: f32,
    pub eval_king: f32,
    pub eval_trace: f32,
    pub eval_tres: f32,
    pub eval_cap: f32,
    pub eval_hang: f32,
    pub eval_potential: f32,
    pub eval_target: f32,
    pub eval_trap: f32,
    pub eval_capture: f32,
    pub eval_future_threat: f32,
    pub eval_pawn_strike: f32,
    pub eval_pawn_gravity: f32,
    pub eval_pawn_promo: f32,
    pub eval_officer_activity: f32,
    // Extra-algorithm evaluation weights (13 toggleable fields).
    pub eval_rd: f32,
    pub eval_ca: f32,
    pub eval_resistor: f32,
    pub eval_ising: f32,
    pub eval_wave: f32,
    pub eval_lbm: f32,
    pub eval_spectral: f32,
    pub eval_hodge: f32,
    pub eval_ant: f32,
    pub eval_fuzzy: f32,
    pub eval_topo: f32,
    pub eval_latent: f32,
    pub eval_tensor: f32,
    // Iteration counts for the extra-field simulations.
    pub rd_iters: usize,
    pub ca_iters: usize,
    pub ising_iters: usize,
    pub wave_iters: usize,
    pub lbm_iters: usize,
    pub spectral_iters: usize,
    pub ant_iters: usize,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            time_depth: 32,
            gamma: 0.97,
            relax_mu: 0.6,
            relax_iters: 10,
            relax_eps: 1e-3,
            diff_w0: 0.4,
            diff_w1: 0.1,
            diff_w2: 0.05,
            trace_past: 0.6,
            trace_future: 0.4,
            trace_tau: 6.0,
            trace_sigma: 3.0,
            trace_align: 1.0,
            trace_base_align: 0.25,
            quat_alpha: 0.35,
            quat_z0: 1.0,
            res_occ: 1.2,
            res_enemy: 0.6,
            res_lock: 0.8,
            res_king: 1.5,
            beta_p: 0.7,
            beta_time: 0.98,
            noise_start: 0.05,
            history_decay: 0.96,
            history_weight: 0.35,
            history_influence: 0.55,
            future_tau: 6.0,
            future_sigma: 3.5,
            future_base_align: 0.3,
            target_attack_floor: 0.35,
            future_adv_floor: 0.15,
            future_capture_scale: 0.3,
            eval_mat: 4.0,
            eval_ctrl: 3.0,
            eval_king: 4.0,
            eval_trace: 2.0,
            eval_tres: 2.0,
            eval_cap: 3.0,
            eval_hang: 4.0,
            eval_potential: 2.5,
            eval_target: 2.0,
            eval_trap: 2.0,
            eval_capture: 3.0,
            eval_future_threat: 2.0,
            eval_pawn_strike: 1.2,
            eval_pawn_gravity: 1.4,
            eval_pawn_promo: 1.6,
            eval_officer_activity: 1.1,
            eval_rd: 0.8,
            eval_ca: 0.6,
            eval_resistor: 0.8,
            eval_ising: 0.6,
            eval_wave: 0.6,
            eval_lbm: 0.6,
            eval_spectral: 0.6,
            eval_hodge: 0.6,
            eval_ant: 0.6,
            eval_fuzzy: 0.8,
            eval_topo: 0.6,
            eval_latent: 0.8,
            eval_tensor: 0.8,
            rd_iters: 6,
            ca_iters: 3,
            ising_iters: 6,
            wave_iters: 6,
            lbm_iters: 6,
            spectral_iters: 6,
            ant_iters: 6,
        }
    }
}

impl Params {
    /// Return a copy with the evaluation weights multiplicatively perturbed,
    /// mirroring `_apply_noise` over `NOISE_KEYS` in `ai.py`.
    pub fn with_noise(&self, rng: &mut crate::ai::Rng, sigma: f32) -> Params {
        let mut p = self.clone();
        if sigma <= 0.0 {
            return p;
        }
        let mut scale = |base: f32| -> f32 {
            let s = (1.0 + rng.normal(0.0, sigma)).max(0.05);
            base * s
        };
        p.eval_mat = scale(p.eval_mat);
        p.eval_ctrl = scale(p.eval_ctrl);
        p.eval_king = scale(p.eval_king);
        p.eval_trace = scale(p.eval_trace);
        p.eval_tres = scale(p.eval_tres);
        p.eval_cap = scale(p.eval_cap);
        p.eval_hang = scale(p.eval_hang);
        p.eval_potential = scale(p.eval_potential);
        p.eval_target = scale(p.eval_target);
        p.eval_trap = scale(p.eval_trap);
        p.eval_capture = scale(p.eval_capture);
        p.eval_future_threat = scale(p.eval_future_threat);
        p.eval_pawn_strike = scale(p.eval_pawn_strike);
        p.eval_pawn_gravity = scale(p.eval_pawn_gravity);
        p.eval_pawn_promo = scale(p.eval_pawn_promo);
        p.eval_officer_activity = scale(p.eval_officer_activity);
        // The 13 extra weights are part of NOISE_KEYS in ai.py too.
        p.eval_rd = scale(p.eval_rd);
        p.eval_ca = scale(p.eval_ca);
        p.eval_resistor = scale(p.eval_resistor);
        p.eval_ising = scale(p.eval_ising);
        p.eval_wave = scale(p.eval_wave);
        p.eval_lbm = scale(p.eval_lbm);
        p.eval_spectral = scale(p.eval_spectral);
        p.eval_hodge = scale(p.eval_hodge);
        p.eval_ant = scale(p.eval_ant);
        p.eval_fuzzy = scale(p.eval_fuzzy);
        p.eval_topo = scale(p.eval_topo);
        p.eval_latent = scale(p.eval_latent);
        p.eval_tensor = scale(p.eval_tensor);
        p
    }

    /// Effective default weight for extra index 0..12 (matches EXTRA_DEFS order).
    pub fn extra_weight(&self, i: usize) -> f32 {
        match i {
            0 => self.eval_rd,
            1 => self.eval_ca,
            2 => self.eval_resistor,
            3 => self.eval_ising,
            4 => self.eval_wave,
            5 => self.eval_lbm,
            6 => self.eval_spectral,
            7 => self.eval_hodge,
            8 => self.eval_ant,
            9 => self.eval_fuzzy,
            10 => self.eval_topo,
            11 => self.eval_latent,
            12 => self.eval_tensor,
            _ => 0.0,
        }
    }
}
