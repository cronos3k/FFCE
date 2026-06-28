//! Tests for the 13 extra-field algorithms: finiteness, that enabling an extra
//! changes the evaluation, and that the weight multiplier scales linearly.

use ffce_engine::ai::{choose_move, Rng};
use ffce_engine::board::{Board, Move};
use ffce_engine::constants::{center_weight, Params};
use ffce_engine::eval::{analyze, evaluate_position};
use ffce_engine::extras::{self, ExtrasConfig};

/// A spread of distinct positions so every extra has at least one position
/// where it produces a nonzero king-zone contribution.
fn sample_positions() -> Vec<Board> {
    let cw = center_weight();
    let params = Params::default();
    let cfg = ExtrasConfig::default();
    let mut boards = vec![Board::new()];

    // After 1.e4 e5 2.Nf3 Nc6 3.Bb5 (asymmetric, pieces developed).
    let mut b = Board::new();
    for m in [
        Move::new(4, 1, 4, 3), // e4
        Move::new(4, 6, 4, 4), // e5
        Move::new(6, 0, 5, 2), // Nf3
        Move::new(1, 7, 2, 5), // Nc6
        Move::new(5, 0, 1, 4), // Bb5
    ] {
        b.apply_move(&m);
    }
    boards.push(b.clone());

    // Continue with several AI moves to reach varied middlegame shapes.
    let mut rng = Rng::new(99);
    for _ in 0..16 {
        if b.generate_legal_moves(b.side_to_move).is_empty() {
            break;
        }
        let m = choose_move(&b, &mut rng, &params, &cw, &cfg, 0.0).unwrap();
        b.apply_move(&m);
        boards.push(b.clone());
    }
    boards
}

#[test]
fn all_extra_fields_finite_on_start() {
    let cw = center_weight();
    let params = Params::default();
    let board = Board::new();
    let an = analyze(&board, &params, &cw);
    let ctx = an.ctx();
    for i in 0..extras::N_EXTRAS {
        let field = extras::compute_extra_field(i, &board, &ctx, &params);
        for y in 0..8 {
            for x in 0..8 {
                assert!(
                    field[y][x].is_finite(),
                    "extra {} ({}) produced non-finite value at ({},{})",
                    i,
                    extras::EXTRA_KEYS[i],
                    x,
                    y
                );
            }
        }
    }
}

#[test]
fn enabling_each_extra_changes_evaluation() {
    let cw = center_weight();
    let params = Params::default();
    let boards = sample_positions();
    for i in 0..extras::N_EXTRAS {
        let mut changed = false;
        for board in &boards {
            let base = evaluate_position(board, &params, &cw, &ExtrasConfig::default());
            let mut cfg = ExtrasConfig::default();
            cfg.enabled[i] = true;
            let with = evaluate_position(board, &params, &cw, &cfg);
            if (with - base).abs() > 1e-6 {
                changed = true;
                break;
            }
        }
        assert!(
            changed,
            "enabling extra {} ({}) changed the evaluation in none of the {} sample positions",
            i,
            extras::EXTRA_KEYS[i],
            boards.len()
        );
    }
}

#[test]
fn weight_multiplier_scales_contribution_linearly() {
    let cw = center_weight();
    let params = Params::default();
    let boards = sample_positions();
    for i in 0..extras::N_EXTRAS {
        // Find a position where this extra has a nonzero contribution.
        let mut tested = false;
        for board in &boards {
            let base = evaluate_position(board, &params, &cw, &ExtrasConfig::default());
            let mut c1 = ExtrasConfig::default();
            c1.enabled[i] = true;
            c1.mult[i] = 1.0;
            let contrib1 = evaluate_position(board, &params, &cw, &c1) - base;
            if contrib1.abs() <= 1e-6 {
                continue;
            }
            let mut c2 = ExtrasConfig::default();
            c2.enabled[i] = true;
            c2.mult[i] = 2.0;
            let contrib2 = evaluate_position(board, &params, &cw, &c2) - base;
            assert!(
                (contrib2 - 2.0 * contrib1).abs() <= 1e-3 * (1.0 + contrib1.abs()),
                "extra {} ({}) not linear: c1={}, c2={}",
                i,
                extras::EXTRA_KEYS[i],
                contrib1,
                contrib2
            );
            tested = true;
            break;
        }
        assert!(tested, "extra {} ({}) never produced a nonzero contribution to scale-test", i, extras::EXTRA_KEYS[i]);
    }
}
