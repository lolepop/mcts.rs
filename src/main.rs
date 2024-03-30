use std::{collections::VecDeque, fs::File, io::Write};

use anyhow::{anyhow, Ok, Result};
use rand::seq::SliceRandom;

#[derive(Debug, Clone, Copy)]
enum WinState {
    Win,
    // Loss,
    Draw,
    Continue
}

#[derive(Clone, Debug)]
struct TicTacToe {
    board: [Option<bool>; 9],
    first_player_turn: bool,
    pub game_ended: bool,
}

impl TicTacToe {
    fn new() -> Self {
        Self {
            board: [None; 9],
            first_player_turn: true,
            game_ended: false
        }
    }

    fn possible_moves(&self) -> Vec<usize> {
        self.board.iter()
            .enumerate()
            .filter_map(|(i, t)| t.is_none().then(|| i))
            .collect::<Vec<_>>()
    }

    fn place_move(&mut self, position: usize) -> Result<WinState> {
        if self.game_ended {
            return Err(anyhow!("game has ended"));
        }

        if let Some(p) = self.board.get_mut(position) {
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

    fn print(&self) {
        for a in (0..9).step_by(3) {
            let s = self.board[a..a+3].iter().map(|m| match m {
                Some(p) => if *p { "o" } else { "x" },
                None => "-",
            }).collect::<String>();
            println!("{s}");
        }
    }
}

#[derive(Clone, Debug)]
struct MctsNode {
    pub placement_move: usize,
    pub score: i32,
    pub visits: u32,
}

struct MctsTree {
    nodes: Vec<MctsNode>,
    children: Vec<Vec<usize>>,
}

impl MctsTree {
    fn new() -> Self {
        Self {
            nodes: vec![MctsNode { placement_move: 0, score: 0, visits: 0 }],
            children: vec![Vec::new()],
        }
    }

    fn add_child(&mut self, node: usize, placement_move: usize) -> Option<usize> {
        if node < self.nodes.len() {
            let id = self.nodes.len();
            self.children[node].push(id);
            self.nodes.push(MctsNode { placement_move, score: 0, visits: 0 });
            self.children.push(Vec::new());
            Some(id)
        } else {
            None
        }
    }

    fn children(&self, node: usize) -> Option<Vec<usize>> {
        self.children.get(node).cloned()
    }

    fn node(&self, node: usize) -> Option<&MctsNode> {
        self.nodes.get(node)
    }

    fn dump(&self) {
        let mut f = File::create("./out.dot").unwrap();
        f.write("digraph G {\noverlap=\"scalexy\"".as_bytes()).unwrap();
        let mut queue = VecDeque::from([0usize]);
        while let Some(parent) = queue.pop_front() {
            if let Some(children) = self.children.get(parent) {
                for child in children {
                    let child_stats = &self.nodes[*child];
                    if child_stats.visits > 0 {
                        f.write(format!("{parent}->{child};").as_bytes()).unwrap();
                        f.write(format!("{child} [label=<{child}<br/>move={}<br/>score={}<br/>visits={}>];", child_stats.placement_move, child_stats.score, child_stats.visits).as_bytes()).unwrap();
                    }
                    queue.push_back(*child);
                }
            }
        }
        f.write("}".as_bytes()).unwrap();
        f.flush().unwrap();
    }

}

fn uct(score: i32, visits: u32, total_visits: u32, c: f32) -> f32 {
    if visits == 0 {
        f32::INFINITY
    } else {
        (score as f32) / (visits as f32) + c * ((total_visits as f32).ln() / (visits as f32)).sqrt()
    }
}

struct Mcts {
    tree: MctsTree,
    is_first_player: bool,
    root: usize,
}

impl Mcts {
    fn new(is_first_player: bool) -> Self {
        Self { tree: MctsTree::new(), root: 0, is_first_player }
    }

    fn select(&self, game: &mut TicTacToe) -> Vec<usize> {
        let mut traversal = vec![self.root];
        loop {
            let last_id = *traversal.last().unwrap();
            let node_children = self.tree.children(last_id).unwrap();
            if node_children.len() == 0 {
                // println!("selection: {traversal:?}");
                return traversal;
            }
            let stats = node_children.iter().map(|n| self.tree.node(*n).unwrap());
            let total_visits = stats.clone().fold(0, |acc, s| acc + s.visits);
            let (selected_node, best_uct) = stats.enumerate()
                    .map(|(i, s)| (i, uct(s.score, s.visits, total_visits, 2f32.sqrt())))
                    .max_by(|(_, a), (_, b)| a.total_cmp(b))
                    .unwrap();
            traversal.push(node_children[selected_node]);
            // game.place_move(position)
        }
    }

    fn expand(&mut self, game: &TicTacToe, mut traversal: Vec<usize>) -> Vec<usize> {
        let selected_node = *traversal.last().unwrap();
        for m in game.possible_moves() {
            self.tree.add_child(selected_node, m);
        }

        let next_selection = *self.tree.children(selected_node).unwrap().choose(&mut rand::thread_rng()).unwrap();
        traversal.push(next_selection);
        traversal
    }

    fn rollout(&mut self, game: &mut TicTacToe, traversal: &Vec<usize>) -> i32 {
        loop {
            let random_move = *game.possible_moves().choose(&mut rand::thread_rng()).unwrap();
            if let Some(score) = self.score_state(game.place_move(random_move).unwrap(), game) {
                return score;
            }
        }
    }

    fn score_state(&self, state: WinState, game: &TicTacToe) -> Option<i32> {
        match state {
            WinState::Win => Some(if game.first_player_turn == self.is_first_player { 1 } else { -1 }),
            WinState::Draw => Some(0),
            _ => None
        }
    }

    fn backpropagate(&mut self, traversal: &Vec<usize>, score: i32) {
        for id in traversal {
            let n = &mut self.tree.nodes[*id];
            n.visits += 1;
            n.score += score;
        }
    }

    fn best_move(&mut self, game: &TicTacToe) -> usize {
        for _ in 0..1000 {
            let mut base_game = game.clone();
            let mut last_score: Option<i32> = None;
            // select
            let mut selected = self.select(&mut base_game);
            for m in selected.iter().skip(1) {
                let m = self.tree.node(*m).unwrap().placement_move;
                last_score = self.score_state(base_game.place_move(m).unwrap(), &base_game);
            }

            // expand
            if last_score.is_none() {
                selected = self.expand(&base_game, selected);
                let expanded_placement = self.tree.node(*selected.last().unwrap()).unwrap().placement_move;
                last_score = self.score_state(base_game.place_move(expanded_placement).unwrap(), &base_game);
            }
            // rollout
            let rollout_score = if let Some(premature_end_score) = last_score {
                premature_end_score
            } else {
                self.rollout(&mut base_game, &selected)
            };
            // backprop
            self.backpropagate(&selected, rollout_score);
        }

        self.tree.dump();
        todo!()
    }
}

fn main() {
    let mut game = TicTacToe::new();
    // game.place_move(4).unwrap();
    // game.place_move(3).unwrap();
    // game.place_move(6).unwrap();
    // game.place_move(2).unwrap();
    // game.place_move(8).unwrap();
    // game.place_move(0).unwrap();
    // game.print();
    Mcts::new(true).best_move(&game);
    // while !game.game_ended {
    //     let m = *game.possible_moves().choose(&mut rand::thread_rng()).unwrap();
    //     game.place_move(m)
    //         .inspect_err(|e| eprintln!("{e:?}"))
    //         .inspect(|r| match *r { WinState::Continue => {}, _ => println!("{r:?}") })
    //         .unwrap();
    //     game.print();
    //     println!("");
    // }
    // game.place_move(0);
    // game.place_move(1);
    // game.place_move(3);
    // game.place_move(4);
    // game.place_move(6);
    println!("{:?}", game.possible_moves());
}
