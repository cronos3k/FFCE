//! Move selection: single-ply flow-field evaluation with weight noise.
//!
//! Faithful port of `ai.py` (`choose_move`). No search tree, no opening book.

use crate::board::{Board, Move};
use crate::constants::{Params, WHITE};
use crate::eval::evaluate_position;
use crate::extras::{ExtrasConfig, N_EXTRAS};
use crate::fields::Grid;

/// Tiny deterministic RNG (xorshift128+ style) with a Gaussian sampler.
/// Avoids pulling in the `rand` crate so the build stays dependency-free.
pub struct Rng {
    s0: u64,
    s1: u64,
    spare: Option<f32>,
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        // SplitMix64 to seed the two state words.
        let mut z = seed.wrapping_add(0x9E3779B97F4A7C15);
        let mut next = || {
            z = z.wrapping_add(0x9E3779B97F4A7C15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
            x ^ (x >> 31)
        };
        Rng { s0: next() | 1, s1: next() | 1, spare: None }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.s0;
        let y = self.s1;
        self.s0 = y;
        x ^= x << 23;
        x ^= x >> 17;
        x ^= y ^ (y >> 26);
        self.s1 = x;
        x.wrapping_add(y)
    }

    /// Uniform f32 in [0, 1).
    fn uniform(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) / ((1u64 << 24) as f32)
    }

    /// Gaussian sample via Box-Muller.
    pub fn normal(&mut self, mu: f32, sigma: f32) -> f32 {
        if let Some(s) = self.spare.take() {
            return mu + sigma * s;
        }
        let mut u1 = self.uniform();
        let u2 = self.uniform();
        if u1 < 1e-7 {
            u1 = 1e-7;
        }
        let mag = (-2.0 * u1.ln()).sqrt();
        let z0 = mag * (2.0 * core::f32::consts::PI * u2).cos();
        let z1 = mag * (2.0 * core::f32::consts::PI * u2).sin();
        self.spare = Some(z1);
        mu + sigma * z0
    }

    /// Uniform integer in [0, n). Returns 0 for n == 0.
    pub fn rand_usize(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }
}

/// Return the best move for the side to move by single-ply evaluation.
pub fn choose_move(
    board: &Board,
    rng: &mut Rng,
    params: &Params,
    cw: &Grid,
    config: &ExtrasConfig,
    noise_sigma: f32,
) -> Option<Move> {
    let moves = board.generate_legal_moves(board.side_to_move);
    if moves.is_empty() {
        return None;
    }
    let noisy = params.with_noise(rng, noise_sigma);
    let mut best_move: Option<Move> = None;
    let mut best_score = -1e9f32;
    for m in moves {
        let mut b = board.clone();
        b.apply_move(&m);
        let score = evaluate_position(&b, &noisy, cw, config);
        let mut side_score = if board.side_to_move == WHITE { score } else { -score };
        if board.fullmove == 1 {
            side_score += rng.normal(0.0, params.noise_start);
        }
        if side_score > best_score {
            best_score = side_score;
            best_move = Some(m);
        }
    }
    best_move
}

/// Would playing `m` from `board` land on a position we've already seen?
fn leads_to_repetition(board: &Board, m: &Move, history: &[u64]) -> bool {
    let mut b = board.clone();
    b.apply_move(m);
    history.contains(&b.position_hash())
}

/// Highest-scoring legal move (under `config`) whose resulting position is NOT
/// in `history`. `None` only if every legal move repeats.
fn best_non_repetition(
    board: &Board,
    rng: &mut Rng,
    params: &Params,
    cw: &Grid,
    config: &ExtrasConfig,
    noise_sigma: f32,
    history: &[u64],
) -> Option<Move> {
    let noisy = params.with_noise(rng, noise_sigma);
    let mut best: Option<Move> = None;
    let mut best_score = -1e9f32;
    for m in board.generate_legal_moves(board.side_to_move) {
        let mut b = board.clone();
        b.apply_move(&m);
        if history.contains(&b.position_hash()) {
            continue;
        }
        let score = evaluate_position(&b, &noisy, cw, config);
        let side_score = if board.side_to_move == WHITE { score } else { -score };
        if side_score > best_score {
            best_score = side_score;
            best = Some(m);
        }
    }
    best
}

/// Move selection with a loop/"dance" watchdog.
///
/// Picks the normal flow-field move; if that move would repeat a position
/// already seen in `history`, it switches on random extra algorithms one by one
/// (in random order) and re-evaluates until it finds a non-repeating move. If
/// toggling extras doesn't help, it falls back to the best legal move that
/// doesn't repeat. Only if *every* move repeats does it return the base move.
pub fn choose_move_avoiding(
    board: &Board,
    rng: &mut Rng,
    params: &Params,
    cw: &Grid,
    config: &ExtrasConfig,
    noise_sigma: f32,
    history: &[u64],
) -> Option<Move> {
    let base = choose_move(board, rng, params, cw, config, noise_sigma)?;
    if history.is_empty() || !leads_to_repetition(board, &base, history) {
        return Some(base);
    }

    // Watchdog: enable currently-disabled extras in random order until the
    // resulting best move no longer repeats.
    let mut disabled: Vec<usize> = (0..N_EXTRAS).filter(|&i| !config.enabled[i]).collect();
    let mut i = disabled.len();
    while i > 1 {
        i -= 1;
        let j = rng.rand_usize(i + 1);
        disabled.swap(i, j);
    }
    let mut cfg = config.clone();
    for &idx in &disabled {
        cfg.enabled[idx] = true;
        if let Some(cand) = choose_move(board, rng, params, cw, &cfg, noise_sigma) {
            if !leads_to_repetition(board, &cand, history) {
                return Some(cand);
            }
        }
    }

    // Fallback: best non-repeating move under the original config; if none
    // exists (every move repeats), accept the base move.
    best_non_repetition(board, rng, params, cw, config, noise_sigma, history).or(Some(base))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::center_weight;

    #[test]
    fn hash_changes_on_move_and_returns_on_dance() {
        let mut b = Board::new();
        let start = b.position_hash();
        b.apply_move(&Move::new(4, 1, 4, 3)); // e2-e4
        assert_ne!(b.position_hash(), start);
        // A full knight "dance" back to the initial position must hash equal.
        let mut d = Board::new();
        for (fx, fy, tx, ty) in [(1, 0, 2, 2), (1, 7, 2, 5), (2, 2, 1, 0), (2, 5, 1, 7)] {
            d.apply_move(&Move::new(fx, fy, tx, ty));
        }
        assert_eq!(d.position_hash(), Board::new().position_hash());
    }

    #[test]
    fn watchdog_avoids_a_known_repetition() {
        let b = Board::new();
        let params = Params::default();
        let cw = center_weight();
        let cfg = ExtrasConfig::default();
        // Seed history with the position that the natural best move would reach.
        let mut rng = Rng::new(99);
        let base = choose_move(&b, &mut rng, &params, &cw, &cfg, 0.0).unwrap();
        let mut after = b.clone();
        after.apply_move(&base);
        let history = vec![after.position_hash()];
        // The watchdog must return a move that does NOT land on that position.
        let m = choose_move_avoiding(&b, &mut rng, &params, &cw, &cfg, 0.0, &history).unwrap();
        let mut res = b.clone();
        res.apply_move(&m);
        assert!(!history.contains(&res.position_hash()));
    }
}
