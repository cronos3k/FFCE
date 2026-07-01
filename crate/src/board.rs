//! Board representation, legal move generation and rule enforcement.
//!
//! Faithful port of `board.py` — this is the rules authority (validated by the
//! perft tests at the bottom of this file).

use crate::constants::*;
use crate::quaternion::{self, Quat};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Move {
    pub fx: i32,
    pub fy: i32,
    pub tx: i32,
    pub ty: i32,
    pub promotion: u8,
    pub is_en_passant: bool,
    pub is_castle: bool,
}

impl Move {
    pub fn new(fx: i32, fy: i32, tx: i32, ty: i32) -> Move {
        Move { fx, fy, tx, ty, promotion: 0, is_en_passant: false, is_castle: false }
    }
    pub fn promo(fx: i32, fy: i32, tx: i32, ty: i32, promotion: u8) -> Move {
        Move { fx, fy, tx, ty, promotion, is_en_passant: false, is_castle: false }
    }
}

#[derive(Clone)]
pub struct Piece {
    pub id: u16,
    pub ptype: u8,
    pub color: i32,
    pub x: i32,
    pub y: i32,
    pub quat: Quat,
    pub last_move: Option<(i32, i32, i32, i32)>,
    pub alive: bool,
}

#[derive(Clone)]
pub struct Board {
    /// Piece id per square, indexed [y][x]; 0 = empty.
    pub board: [[u16; 8]; 8],
    /// Pieces indexed by (id - 1); preserves insertion order like the Python dict.
    pub pieces: Vec<Piece>,
    pub side_to_move: i32,
    pub castling: u8,
    pub en_passant: Option<(i32, i32)>,
    pub halfmove: u32,
    pub fullmove: u32,
    pub history_w: [[f32; 8]; 8],
    pub history_b: [[f32; 8]; 8],
}

impl Board {
    pub fn new() -> Board {
        let mut b = Board {
            board: [[0u16; 8]; 8],
            pieces: Vec::with_capacity(32),
            side_to_move: WHITE,
            castling: CASTLE_WK | CASTLE_WQ | CASTLE_BK | CASTLE_BQ,
            en_passant: None,
            halfmove: 0,
            fullmove: 1,
            history_w: [[0.0; 8]; 8],
            history_b: [[0.0; 8]; 8],
        };
        b.setup_initial();
        b
    }

    pub fn empty() -> Board {
        Board {
            board: [[0u16; 8]; 8],
            pieces: Vec::new(),
            side_to_move: WHITE,
            castling: 0,
            en_passant: None,
            halfmove: 0,
            fullmove: 1,
            history_w: [[0.0; 8]; 8],
            history_b: [[0.0; 8]; 8],
        }
    }

    fn add_piece(&mut self, ptype: u8, color: i32, x: i32, y: i32) {
        let id = (self.pieces.len() + 1) as u16;
        self.pieces.push(Piece {
            id,
            ptype,
            color,
            x,
            y,
            quat: quaternion::identity(),
            last_move: None,
            alive: true,
        });
        self.board[y as usize][x as usize] = id;
    }

    fn setup_initial(&mut self) {
        for x in 0..8 {
            self.add_piece(PAWN, WHITE, x, 1);
            self.add_piece(PAWN, BLACK, x, 6);
        }
        let back = [ROOK, KNIGHT, BISHOP, QUEEN, KING, BISHOP, KNIGHT, ROOK];
        for (x, &t) in back.iter().enumerate() {
            self.add_piece(t, WHITE, x as i32, 0);
            self.add_piece(t, BLACK, x as i32, 7);
        }
    }

    #[inline]
    fn piece_at_id(&self, x: i32, y: i32) -> u16 {
        self.board[y as usize][x as usize]
    }

    #[inline]
    pub fn piece(&self, id: u16) -> &Piece {
        &self.pieces[(id - 1) as usize]
    }

    /// Position fingerprint by piece type/colour per square (+ side to move,
    /// castling, en-passant). Two positions that look identical hash equal,
    /// regardless of internal piece ids. Used for repetition/loop detection.
    pub fn position_hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a offset basis
        for y in 0..8 {
            for x in 0..8 {
                let id = self.board[y][x];
                let code: u8 = if id == 0 {
                    0
                } else {
                    let p = self.piece(id);
                    (p.ptype) | if p.color == WHITE { 0 } else { 0x40 }
                };
                h ^= code as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        }
        h ^= if self.side_to_move == WHITE { 1 } else { 2 };
        h = h.wrapping_mul(0x0100_0000_01b3);
        h ^= self.castling as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
        if let Some((ex, ey)) = self.en_passant {
            h ^= 0x100 | ((ex as u64) << 4) | (ey as u64);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        h
    }

    /// Color of the piece occupying a square (assumes occupied).
    #[inline]
    fn color_at(&self, x: i32, y: i32) -> i32 {
        let id = self.board[y as usize][x as usize];
        self.pieces[(id - 1) as usize].color
    }

    pub fn get_piece(&self, x: i32, y: i32) -> Option<&Piece> {
        let id = self.piece_at_id(x, y);
        if id == 0 {
            None
        } else {
            Some(self.piece(id))
        }
    }

    pub fn find_king(&self, color: i32) -> Option<(i32, i32)> {
        for p in &self.pieces {
            if p.alive && p.ptype == KING && p.color == color {
                return Some((p.x, p.y));
            }
        }
        None
    }

    pub fn is_square_attacked(&self, x: i32, y: i32, by_color: i32) -> bool {
        // Pawns.
        if by_color == WHITE {
            let py = y - 1;
            if py >= 0 {
                for px in [x - 1, x + 1] {
                    if px >= 0 && px < 8 {
                        if let Some(p) = self.get_piece(px, py) {
                            if p.alive && p.color == WHITE && p.ptype == PAWN {
                                return true;
                            }
                        }
                    }
                }
            }
        } else {
            let py = y + 1;
            if py < 8 {
                for px in [x - 1, x + 1] {
                    if px >= 0 && px < 8 {
                        if let Some(p) = self.get_piece(px, py) {
                            if p.alive && p.color == BLACK && p.ptype == PAWN {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        // Knights.
        for (dx, dy) in KNIGHT_OFFSETS {
            let (nx, ny) = (x + dx, y + dy);
            if in_bounds(nx, ny) {
                if let Some(p) = self.get_piece(nx, ny) {
                    if p.alive && p.color == by_color && p.ptype == KNIGHT {
                        return true;
                    }
                }
            }
        }
        // Bishops / queens (diagonal).
        for (dx, dy) in BISHOP_DIRS {
            let (mut nx, mut ny) = (x + dx, y + dy);
            while in_bounds(nx, ny) {
                if let Some(p) = self.get_piece(nx, ny) {
                    if p.alive && p.color == by_color && (p.ptype == BISHOP || p.ptype == QUEEN) {
                        return true;
                    }
                    break;
                }
                nx += dx;
                ny += dy;
            }
        }
        // Rooks / queens (orthogonal).
        for (dx, dy) in ROOK_DIRS {
            let (mut nx, mut ny) = (x + dx, y + dy);
            while in_bounds(nx, ny) {
                if let Some(p) = self.get_piece(nx, ny) {
                    if p.alive && p.color == by_color && (p.ptype == ROOK || p.ptype == QUEEN) {
                        return true;
                    }
                    break;
                }
                nx += dx;
                ny += dy;
            }
        }
        // King.
        for (dx, dy) in KING_OFFSETS {
            let (nx, ny) = (x + dx, y + dy);
            if in_bounds(nx, ny) {
                if let Some(p) = self.get_piece(nx, ny) {
                    if p.alive && p.color == by_color && p.ptype == KING {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn is_in_check(&self, color: i32) -> bool {
        match self.find_king(color) {
            Some((kx, ky)) => self.is_square_attacked(kx, ky, -color),
            None => false,
        }
    }

    pub fn generate_pseudo_moves(&self, color: i32) -> Vec<Move> {
        let mut moves: Vec<Move> = Vec::with_capacity(48);
        for p in &self.pieces {
            if !p.alive || p.color != color {
                continue;
            }
            let (x, y) = (p.x, p.y);
            match p.ptype {
                PAWN => {
                    let diry = if color == WHITE { 1 } else { -1 };
                    let start_rank = if color == WHITE { 1 } else { 6 };
                    let promo_rank = if color == WHITE { 7 } else { 0 };
                    let one_y = y + diry;
                    if one_y >= 0 && one_y < 8 && self.board[one_y as usize][x as usize] == EMPTY as u16 {
                        if one_y == promo_rank {
                            for promo in [QUEEN, ROOK, BISHOP, KNIGHT] {
                                moves.push(Move::promo(x, y, x, one_y, promo));
                            }
                        } else {
                            moves.push(Move::new(x, y, x, one_y));
                        }
                        let two_y = y + 2 * diry;
                        if y == start_rank && self.board[two_y as usize][x as usize] == EMPTY as u16 {
                            moves.push(Move::new(x, y, x, two_y));
                        }
                    }
                    for dx in [-1, 1] {
                        let cx = x + dx;
                        let cy = y + diry;
                        if in_bounds(cx, cy) {
                            let target = self.board[cy as usize][cx as usize];
                            if target != 0 && self.color_at(cx, cy) != color {
                                if cy == promo_rank {
                                    for promo in [QUEEN, ROOK, BISHOP, KNIGHT] {
                                        moves.push(Move::promo(x, y, cx, cy, promo));
                                    }
                                } else {
                                    moves.push(Move::new(x, y, cx, cy));
                                }
                            }
                            if self.en_passant == Some((cx, cy)) {
                                let mut m = Move::new(x, y, cx, cy);
                                m.is_en_passant = true;
                                moves.push(m);
                            }
                        }
                    }
                }
                KNIGHT => {
                    for (dx, dy) in KNIGHT_OFFSETS {
                        let (nx, ny) = (x + dx, y + dy);
                        if in_bounds(nx, ny) {
                            let target = self.board[ny as usize][nx as usize];
                            if target == 0 || self.color_at(nx, ny) != color {
                                moves.push(Move::new(x, y, nx, ny));
                            }
                        }
                    }
                }
                KING => {
                    for (dx, dy) in KING_OFFSETS {
                        let (nx, ny) = (x + dx, y + dy);
                        if in_bounds(nx, ny) {
                            let target = self.board[ny as usize][nx as usize];
                            if target == 0 || self.color_at(nx, ny) != color {
                                moves.push(Move::new(x, y, nx, ny));
                            }
                        }
                    }
                    self.add_castle_moves(color, &mut moves);
                }
                BISHOP => self.slide_moves(x, y, color, &BISHOP_DIRS, &mut moves),
                ROOK => self.slide_moves(x, y, color, &ROOK_DIRS, &mut moves),
                QUEEN => self.slide_moves(x, y, color, &QUEEN_DIRS, &mut moves),
                _ => {}
            }
        }
        moves
    }

    fn add_castle_moves(&self, color: i32, moves: &mut Vec<Move>) {
        let e = EMPTY as u16;
        if color == WHITE && (self.castling & CASTLE_WK) != 0 {
            if self.board[0][5] == e && self.board[0][6] == e
                && !self.is_square_attacked(4, 0, BLACK)
                && !self.is_square_attacked(5, 0, BLACK)
                && !self.is_square_attacked(6, 0, BLACK)
                && self.board[0][7] != e
            {
                moves.push(Move { fx: 4, fy: 0, tx: 6, ty: 0, promotion: 0, is_en_passant: false, is_castle: true });
            }
        }
        if color == WHITE && (self.castling & CASTLE_WQ) != 0 {
            if self.board[0][1] == e && self.board[0][2] == e && self.board[0][3] == e
                && !self.is_square_attacked(4, 0, BLACK)
                && !self.is_square_attacked(3, 0, BLACK)
                && !self.is_square_attacked(2, 0, BLACK)
                && self.board[0][0] != e
            {
                moves.push(Move { fx: 4, fy: 0, tx: 2, ty: 0, promotion: 0, is_en_passant: false, is_castle: true });
            }
        }
        if color == BLACK && (self.castling & CASTLE_BK) != 0 {
            if self.board[7][5] == e && self.board[7][6] == e
                && !self.is_square_attacked(4, 7, WHITE)
                && !self.is_square_attacked(5, 7, WHITE)
                && !self.is_square_attacked(6, 7, WHITE)
                && self.board[7][7] != e
            {
                moves.push(Move { fx: 4, fy: 7, tx: 6, ty: 7, promotion: 0, is_en_passant: false, is_castle: true });
            }
        }
        if color == BLACK && (self.castling & CASTLE_BQ) != 0 {
            if self.board[7][1] == e && self.board[7][2] == e && self.board[7][3] == e
                && !self.is_square_attacked(4, 7, WHITE)
                && !self.is_square_attacked(3, 7, WHITE)
                && !self.is_square_attacked(2, 7, WHITE)
                && self.board[7][0] != e
            {
                moves.push(Move { fx: 4, fy: 7, tx: 2, ty: 7, promotion: 0, is_en_passant: false, is_castle: true });
            }
        }
    }

    fn slide_moves(&self, x: i32, y: i32, color: i32, dirs: &[(i32, i32)], moves: &mut Vec<Move>) {
        for &(dx, dy) in dirs {
            let (mut nx, mut ny) = (x + dx, y + dy);
            while in_bounds(nx, ny) {
                let target = self.board[ny as usize][nx as usize];
                if target == 0 {
                    moves.push(Move::new(x, y, nx, ny));
                } else {
                    if self.color_at(nx, ny) != color {
                        moves.push(Move::new(x, y, nx, ny));
                    }
                    break;
                }
                nx += dx;
                ny += dy;
            }
        }
    }

    pub fn generate_legal_moves(&self, color: i32) -> Vec<Move> {
        let mut legal = Vec::with_capacity(40);
        for m in self.generate_pseudo_moves(color) {
            let mut b = self.clone();
            b.apply_move(&m);
            if !b.is_in_check(color) {
                legal.push(m);
            }
        }
        legal
    }

    fn path_squares(fx: i32, fy: i32, tx: i32, ty: i32) -> Vec<(i32, i32)> {
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

    fn update_history(&mut self, piece_type: u8, color: i32, weight_decay: f32, fx: i32, fy: i32, tx: i32, ty: i32) {
        let w = PIECE_VALUES[piece_type as usize] * weight_decay;
        for (sx, sy) in Board::path_squares(fx, fy, tx, ty) {
            if in_bounds(sx, sy) {
                if color == WHITE {
                    self.history_w[sy as usize][sx as usize] += w;
                } else {
                    self.history_b[sy as usize][sx as usize] += w;
                }
            }
        }
    }

    /// Apply a move, updating board state, castling/en-passant rights, and
    /// the trace history maps. Uses default params for the history constants.
    pub fn apply_move(&mut self, m: &Move) {
        self.apply_move_params(m, 0.96, 0.35, 1.0, 0.35);
    }

    pub fn apply_move_params(&mut self, m: &Move, history_decay: f32, history_weight: f32, quat_z0: f32, quat_alpha: f32) {
        let id = self.piece_at_id(m.fx, m.fy);
        if id == 0 {
            return;
        }
        // Decay history maps.
        for y in 0..8 {
            for x in 0..8 {
                self.history_w[y][x] *= history_decay;
                self.history_b[y][x] *= history_decay;
            }
        }
        let dx = m.tx - m.fx;
        let dy = m.ty - m.fy;
        let color = self.piece(id).color;
        let piece_type_before = self.piece(id).ptype;

        let mut captured_type: u8 = 0;
        let mut captured_color: i32 = 0;
        let mut captured_pos: Option<(i32, i32)> = None;

        if m.is_en_passant {
            let cap_y = m.ty - if color == WHITE { 1 } else { -1 };
            let cap_id = self.board[cap_y as usize][m.tx as usize];
            if cap_id != 0 {
                let cp = &mut self.pieces[(cap_id - 1) as usize];
                captured_type = cp.ptype;
                captured_color = cp.color;
                captured_pos = Some((m.tx, cap_y));
                cp.alive = false;
                self.board[cap_y as usize][m.tx as usize] = 0;
            }
        } else {
            let target = self.board[m.ty as usize][m.tx as usize];
            if target != 0 {
                let cp = &mut self.pieces[(target - 1) as usize];
                captured_type = cp.ptype;
                captured_color = cp.color;
                captured_pos = Some((m.tx, m.ty));
                cp.alive = false;
            }
        }

        self.board[m.fy as usize][m.fx as usize] = 0;
        self.board[m.ty as usize][m.tx as usize] = id;

        {
            let p = &mut self.pieces[(id - 1) as usize];
            p.x = m.tx;
            p.y = m.ty;
            p.quat = quaternion::update(p.quat, dx, dy, quat_z0, quat_alpha);
            p.last_move = Some((m.fx, m.fy, m.tx, m.ty));
            if m.promotion != 0 {
                p.ptype = m.promotion;
            }
        }
        self.update_history(piece_type_before, color, history_weight, m.fx, m.fy, m.tx, m.ty);

        if m.is_castle {
            let rook_y = if color == WHITE { 0 } else { 7 };
            let (rook_from, rook_to) = if m.tx == 6 { (7usize, 5usize) } else { (0usize, 3usize) };
            let rook_id = self.board[rook_y][rook_from];
            if rook_id != 0 {
                self.board[rook_y][rook_from] = 0;
                self.board[rook_y][rook_to] = rook_id;
                let rook = &mut self.pieces[(rook_id - 1) as usize];
                rook.x = rook_to as i32;
                rook.y = rook_y as i32;
                rook.last_move = Some((rook_from as i32, rook_y as i32, rook_to as i32, rook_y as i32));
                self.update_history(ROOK, color, history_weight, rook_from as i32, rook_y as i32, rook_to as i32, rook_y as i32);
            }
        }

        // Castling rights updates.
        if piece_type_before == KING {
            if color == WHITE {
                self.castling &= !(CASTLE_WK | CASTLE_WQ);
            } else {
                self.castling &= !(CASTLE_BK | CASTLE_BQ);
            }
        }
        if piece_type_before == ROOK {
            if color == WHITE {
                if (m.fx, m.fy) == (0, 0) {
                    self.castling &= !CASTLE_WQ;
                } else if (m.fx, m.fy) == (7, 0) {
                    self.castling &= !CASTLE_WK;
                }
            } else {
                if (m.fx, m.fy) == (0, 7) {
                    self.castling &= !CASTLE_BQ;
                } else if (m.fx, m.fy) == (7, 7) {
                    self.castling &= !CASTLE_BK;
                }
            }
        }
        if captured_type == ROOK {
            if let Some(pos) = captured_pos {
                if captured_color == WHITE {
                    if pos == (0, 0) {
                        self.castling &= !CASTLE_WQ;
                    } else if pos == (7, 0) {
                        self.castling &= !CASTLE_WK;
                    }
                } else {
                    if pos == (0, 7) {
                        self.castling &= !CASTLE_BQ;
                    } else if pos == (7, 7) {
                        self.castling &= !CASTLE_BK;
                    }
                }
            }
        }

        self.en_passant = None;
        if piece_type_before == PAWN && dy.abs() == 2 {
            let mid_y = (m.fy + m.ty) / 2;
            self.en_passant = Some((m.fx, mid_y));
        }
        if piece_type_before == PAWN || captured_type != 0 {
            self.halfmove = 0;
        } else {
            self.halfmove += 1;
        }
        if self.side_to_move == BLACK {
            self.fullmove += 1;
        }
        self.side_to_move = -self.side_to_move;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perft(board: &Board, depth: u32) -> u64 {
        if depth == 0 {
            return 1;
        }
        let mut nodes = 0u64;
        for m in board.generate_legal_moves(board.side_to_move) {
            let mut child = board.clone();
            child.apply_move(&m);
            nodes += perft(&child, depth - 1);
        }
        nodes
    }

    #[test]
    fn perft_depth_1() {
        let b = Board::new();
        assert_eq!(perft(&b, 1), 20);
    }

    #[test]
    fn perft_depth_2() {
        let b = Board::new();
        assert_eq!(perft(&b, 2), 400);
    }

    #[test]
    fn perft_depth_3() {
        let b = Board::new();
        assert_eq!(perft(&b, 3), 8902);
    }

    #[test]
    fn perft_depth_4() {
        let b = Board::new();
        assert_eq!(perft(&b, 4), 197281);
    }

    #[test]
    fn start_position_has_two_kings() {
        let b = Board::new();
        assert!(b.find_king(WHITE).is_some());
        assert!(b.find_king(BLACK).is_some());
    }

    // Regression for the "stalemate should be a win" report: lone black king on
    // a7 with White Qc8 + Rb3 (+ bishop) covering every escape square. Black is
    // NOT in check, so standard rules call it a draw; we assert the genuine
    // stalemate conditions here. ffce_status() maps no-legal-moves to a win for
    // the trapper, so this position is scored as a White win, not a draw.
    #[test]
    fn trapped_king_is_genuine_stalemate() {
        let mut b = Board::empty();
        // files a..h = x 0..7, ranks 1..8 = y 0..7
        b.add_piece(QUEEN, WHITE, 2, 7); // Qc8
        b.add_piece(ROOK, WHITE, 1, 2); // Rb3 (covers b-file incl. b6)
        b.add_piece(BISHOP, WHITE, 4, 1); // Be2 (covers a6 too)
        b.add_piece(KING, WHITE, 7, 0); // wKh1 (just to be a valid position)
        b.add_piece(KING, BLACK, 0, 6); // bKa7
        b.side_to_move = BLACK;

        assert!(
            !b.is_in_check(BLACK),
            "black king must not be in check (else it'd be checkmate, not stalemate)"
        );
        assert!(
            b.generate_legal_moves(BLACK).is_empty(),
            "black must have no legal moves: a8/b8/b7/a6 covered by queen+bishop, b6 by rook"
        );
    }
}
