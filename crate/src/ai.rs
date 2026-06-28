//! Move selection: single-ply flow-field evaluation with weight noise.
//!
//! Faithful port of `ai.py` (`choose_move`). No search tree, no opening book.

use crate::board::{Board, Move};
use crate::constants::{Params, WHITE};
use crate::eval::evaluate_position;
use crate::extras::ExtrasConfig;
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
