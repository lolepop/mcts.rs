use crate::{game::{Game, Mcts}, tictactoe::TicTacToe};

mod game;
mod tictactoe;

fn main() {
    let mut game = TicTacToe::new();
    game.print();

    // game.place_move(4).unwrap();
    // game.place_move(2).unwrap();
    // game.place_move(0).unwrap();
    // game.place_move(8).unwrap();
    // game.print();

    // let mut bot = Mcts::new(game.first_player_turn);
    // let bot_move = bot.best_move(&game, 1028);
    // bot.dump_tree();
    // game.place_move(bot_move).unwrap();
    // game.print();

    loop {
        let player_turn = game.first_player_turn;
        let bot_move = Mcts::new(player_turn).best_move(&game, 2048);
        let s = game.place_move(bot_move).unwrap();
        game.print();
        match s {
            tictactoe::WinState::Win | tictactoe::WinState::Draw => break,
            _ => {},
        }
    }
    
}
