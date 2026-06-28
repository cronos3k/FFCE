//! Integration test: drive the flow-field AI against itself and confirm it
//! always returns legal moves and the engine detects a terminal state.

use ffce_engine::ai::{choose_move, Rng};
use ffce_engine::board::Board;
use ffce_engine::constants::{center_weight, Params};
use ffce_engine::eval::evaluate_position;
use ffce_engine::extras::ExtrasConfig;

#[test]
fn ai_plays_legal_full_game() {
    let cw = center_weight();
    let params = Params::default();
    let config = ExtrasConfig::default();
    let mut rng = Rng::new(42);
    let mut board = Board::new();

    let mut plies = 0;
    loop {
        let legal = board.generate_legal_moves(board.side_to_move);
        if legal.is_empty() {
            // Terminal: checkmate or stalemate.
            break;
        }
        if board.halfmove >= 100 || plies >= 400 {
            break; // draw / safety cap
        }
        let m = choose_move(&board, &mut rng, &params, &cw, &config, 0.15)
            .expect("AI must return a move when legal moves exist");
        // Every AI move must be one of the legal moves.
        assert!(
            legal.iter().any(|x| x.fx == m.fx && x.fy == m.fy && x.tx == m.tx && x.ty == m.ty),
            "AI returned an illegal move"
        );
        board.apply_move(&m);
        plies += 1;
    }
    assert!(plies > 0, "game made no progress");
}

#[test]
fn ai_plays_legal_with_all_extras_on() {
    let cw = center_weight();
    let params = Params::default();
    let mut config = ExtrasConfig::default();
    config.enabled = [true; ffce_engine::extras::N_EXTRAS];
    let mut rng = Rng::new(7);
    let mut board = Board::new();
    for _ in 0..20 {
        let legal = board.generate_legal_moves(board.side_to_move);
        if legal.is_empty() {
            break;
        }
        let m = choose_move(&board, &mut rng, &params, &cw, &config, 0.1).unwrap();
        assert!(legal.iter().any(|x| x.fx == m.fx && x.fy == m.fy && x.tx == m.tx && x.ty == m.ty));
        board.apply_move(&m);
    }
}

#[test]
fn evaluation_is_finite_at_start() {
    let cw = center_weight();
    let params = Params::default();
    let config = ExtrasConfig::default();
    let board = Board::new();
    let s = evaluate_position(&board, &params, &cw, &config);
    assert!(s.is_finite(), "start eval must be finite, got {s}");
}
