use anyhow::{anyhow, Ok, Result};

use crate::game::Game;

#[derive(Debug, Clone, Copy)]
pub enum WinState {
    Win,
    // Loss,
    Draw,
    Continue
}

#[derive(Clone, Debug)]
pub(crate) struct TicTacToe {
    board: [Option<bool>; 9],
    pub first_player_turn: bool,
    pub game_ended: bool,
}

impl TicTacToe {
    pub fn new() -> Self {
        Self {
            board: [None; 9],
            first_player_turn: true,
            game_ended: false
        }
    }

    fn next_player(&mut self) {
        self.first_player_turn = !self.first_player_turn;
    }

    fn end_state(&self) -> WinState {
        let score = |a: &Option<bool>| a.map_or(0, |m| if m { 1 } else { -1 });

        // diagonal
        let winning_diag = [[0,4,8], [2,4,6]];
        for c in winning_diag {
            let s = c.iter()
                .map(|p| score(&self.board[*p as usize]))
                .sum::<i8>();
            if s.abs() == 3 {
                return WinState::Win;
            }
        }

        // horizontal and vertical
        let mut h = [0i8; 3];
        let mut v = [0i8; 3];
        for a in 0..3 {
            for b in 0..3 {
                h[a] += score(&self.board[a*3+b]);
                v[a] += score(&self.board[b*3+a]);
            }
        }
        if let Some(score) = h.iter().chain(v.iter()).find(|s| s.abs() == 3) {
            return WinState::Win;
            // should not happen since you cant place a move after the other player has already placed a winning move
        }

        if self.possible_moves().len() == 0 {
            WinState::Draw
        } else {
            WinState::Continue
        }
    }

    pub fn print(&self) {
        for a in (0..9).step_by(3) {
            let s = self.board[a..a+3].iter().map(|m| match m {
                Some(p) => if *p { "o" } else { "x" },
                None => "-",
            }).collect::<String>();
            println!("{s}");
        }
        println!("");
    }
}

impl Game for TicTacToe {
    const IS_PERFECT_INFORMATION: bool = true;

    type Move = usize;
    type GameState = WinState;
    type Player = bool;

    fn possible_moves(&self) -> Vec<Self::Move> {
        self.board.iter()
            .enumerate()
            .filter_map(|(i, t)| t.is_none().then(|| i))
            .collect::<Vec<_>>()
    }

    fn place_move(&mut self, movement: Self::Move) -> Result<Self::GameState> {
        if self.game_ended {
            return Err(anyhow!("game has ended"));
        }

        if let Some(p) = self.board.get_mut(movement) {
            if p.is_some() {
                return Err(anyhow!("move already exists at position"));
            }

            *p = Some(self.first_player_turn);
            let end_state = self.end_state();
            match end_state {
                WinState::Continue => self.next_player(),
                _ => {
                    self.game_ended = true;
                    return Ok(end_state);
                }
            }
            Ok(WinState::Continue)
        } else {
            Err(anyhow!("invalid position provided"))
        }
    }

    fn score_state(&self, state: Self::GameState, player: Self::Player) -> Option<f32> {
        match state {
            WinState::Win => Some(if self.first_player_turn == player { 1f32 } else { -3f32 }),
            WinState::Draw => Some(0.5f32),
            _ => None
        }
    }
}
