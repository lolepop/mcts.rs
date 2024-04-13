mod game;
mod tictactoe;
mod uno;

fn main() {
    // let mut game = TicTacToe::new();
    // game.print();

    // // game.place_move(4).unwrap();
    // // game.place_move(2).unwrap();
    // // game.place_move(0).unwrap();
    // // game.place_move(8).unwrap();
    // // game.print();

    // // let mut bot = Mcts::new(game.first_player_turn);
    // // let bot_move = bot.best_move(&game, 1028);
    // // bot.dump_tree();
    // // game.place_move(bot_move).unwrap();
    // // game.print();

    // loop {
    //     let player_turn = game.first_player_turn;
    //     let bot_move = Mcts::new(player_turn).best_move(&game, 2048);
    //     let s = game.place_move(bot_move).unwrap();
    //     game.print();
    //     match s {
    //         tictactoe::WinState::Win | tictactoe::WinState::Draw => break,
    //         _ => {},
    //     }
    // }

}

#[cfg(test)]
mod tests {
    #[cfg(not(target_env = "msvc"))]
    use tikv_jemallocator::Jemalloc;

    use std::{fs::File, io::Write};

    use rayon::prelude::*;
    use crate::{game::{Game, Mcts}, uno::{self, Uno}};
    use rand::seq::IteratorRandom;

    fn simulate_uno_win(move_budget: [usize; 2]) -> usize {
        let mut game = Uno::standard_deck(2);

        loop {
            // println!("{game}");

            let mut bot = Mcts::new(game.player_turn);
            let bot_move = if move_budget[game.player_turn] > 0 {
                bot.best_move(&game, move_budget[game.player_turn], true)
            } else {
                *game.possible_moves().iter().choose(&mut rand::thread_rng()).unwrap()
            };

            // println!("player {}: {:?} \n", game.player_turn, bot_move);

            // bot.dump_tree();
            match game.place_move(bot_move).unwrap() {
                uno::GameState::Win => {
                    // println!("player {} wins", game.player_turn);
                    return if game.player_turn == 0 { 1 } else { 0 };
                },
                _ => {},
            }
        }
    }


    #[test]
    #[ignore]
    fn bench_uno_stats() {
        let budgets = [0, 32, 128, 1024, 2048, 4096];
        const SAMPLE_SIZE: usize = 50; // 30

        let scores = budgets.par_iter()
            .zip((0..budgets.len()).into_par_iter().map(|_| budgets.par_iter()))
            .map(|(p1, p2s)|
                p2s
                .inspect(|p2| println!("{p1} vs {p2}"))
                .map(|p2|
                    (0..SAMPLE_SIZE).into_par_iter()
                        .map(|_| simulate_uno_win([*p1, *p2]))
                        .sum::<usize>()
                )
                .collect::<Vec<_>>()
            )
            .collect::<Vec<_>>();

        println!("{scores:?}");

        let mut f = File::create("./uno.csv").unwrap();
        for l in scores.iter() {
            let mut s = l.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(",");
            s.push_str("\n");
            f.write_all(s.as_bytes()).unwrap();
        }
    }
}
