use anyhow::Result;
use std::{collections::VecDeque, fmt::Debug, fs::File, io::Write};
use rand::seq::SliceRandom;

fn uct(score: f32, visits: u32, total_visits: u32, c: f32) -> f32 {
    if visits == 0 {
        f32::INFINITY
    } else {
        score / (visits as f32) + c * ((total_visits as f32).ln() / (visits as f32)).sqrt()
    }
}

pub trait Game: Clone + Debug {
    type Move: Default + Debug + Clone;
    type GameState;
    type Player: Clone + Debug;

    fn possible_moves(&self) -> Vec<Self::Move>;
    fn place_move(&mut self, movement: Self::Move) -> Result<Self::GameState>;
    /// returns score used for backpropagation.
    /// none if state is not terminal
    fn score_state(&self, state: Self::GameState, player: Self::Player) -> Option<f32>;
}

#[derive(Clone, Debug)]
struct MctsNode<Move: Default + Debug> {
    pub placement_move: Move,
    pub score: f32,
    pub visits: u32,
}

struct MctsTree<Move: Default + Debug> {
    nodes: Vec<MctsNode<Move>>,
    children: Vec<Vec<usize>>,
}

impl<Move: Default + Debug> MctsTree<Move> {
    fn new() -> Self {
        Self {
            nodes: vec![MctsNode { placement_move: Move::default(), score: 0f32, visits: 0 }],
            children: vec![Vec::new()],
        }
    }

    fn add_child(&mut self, node: usize, placement_move: Move) -> Option<usize> {
        if node < self.nodes.len() {
            let id = self.nodes.len();
            self.children[node].push(id);
            self.nodes.push(MctsNode { placement_move, score: 0f32, visits: 0 });
            self.children.push(Vec::new());
            Some(id)
        } else {
            None
        }
    }

    fn children(&self, node: usize) -> Option<Vec<usize>> {
        self.children.get(node).cloned()
    }

    fn node(&self, node: usize) -> Option<&MctsNode<Move>> {
        self.nodes.get(node)
    }

    fn dump(&self) {
        let mut f = File::create("./out.dot").unwrap();
        f.write("digraph G {overlap=\"scalexy;\"".as_bytes()).unwrap();
        let mut queue = VecDeque::from([0usize]);
        while let Some(parent) = queue.pop_front() {
            if let Some(children) = self.children.get(parent) {
                for child in children {
                    let child_stats = &self.nodes[*child];
                    if child_stats.visits > 0 {
                        f.write(format!("{parent}->{child};").as_bytes()).unwrap();
                        f.write(format!("{child} [label=<{child}<br/>move={:?}<br/>score={}<br/>visits={}>];", child_stats.placement_move, child_stats.score, child_stats.visits).as_bytes()).unwrap();
                    }
                    queue.push_back(*child);
                }
            }
        }
        f.write("}".as_bytes()).unwrap();
        f.flush().unwrap();
    }

}

pub struct Mcts<G: Game> {
    tree: MctsTree<G::Move>,
    player_id: G::Player,
    root: usize,
}

impl<G: Game> Mcts<G> {
    pub fn new(player_id: G::Player) -> Self {
        Self { tree: MctsTree::new(), root: 0, player_id }
    }

    fn select(&self) -> Vec<usize> {
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

    fn expand(&mut self, game: &G, mut traversal: Vec<usize>) -> Vec<usize> {
        let selected_node = *traversal.last().unwrap();
        for m in game.possible_moves() {
            self.tree.add_child(selected_node, m);
        }

        let next_selection = *self.tree.children(selected_node).unwrap().choose(&mut rand::thread_rng()).unwrap();
        traversal.push(next_selection);
        traversal
    }

    fn rollout(&mut self, game: &mut G) -> f32 {
        loop {
            let random_move = game.possible_moves().choose(&mut rand::thread_rng()).unwrap().clone();
            let score = game.place_move(random_move).unwrap();
            if let Some(score) = game.score_state(score, self.player_id.clone()) {
                return score;
            }
        }
    }

    fn backpropagate(&mut self, traversal: &Vec<usize>, score: f32) {
        for id in traversal {
            let n = &mut self.tree.nodes[*id];
            n.visits += 1;
            n.score += score;
        }
    }

    // calculate best average score
    fn best_descendant(&self) -> (&MctsNode<<G as Game>::Move>, f32) {
        self.tree.children[self.root].iter()
            .filter_map(|n| self.tree.node(*n))
            // .map(|n| (n, n.score / (n.visits as f32)))
            .map(|n| (n, n.visits as f32))
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .unwrap()
    }

    pub fn best_move(&mut self, game: &G, iterations: usize) -> G::Move {
        for _ in 0..iterations {
            let mut base_game = game.clone();
            let mut last_score: Option<f32> = None;
            // select
            let mut selected = self.select();
            for m in selected.iter().skip(1) {
                let m = self.tree.node(*m).unwrap().placement_move.clone();
                let s = base_game.place_move(m).unwrap();
                last_score = base_game.score_state(s, self.player_id.clone());
            }

            // expand
            if last_score.is_none() {
                selected = self.expand(&base_game, selected);
                let expanded_placement = self.tree.node(*selected.last().unwrap()).unwrap().placement_move.clone();
                let s = base_game.place_move(expanded_placement).unwrap();
                last_score = base_game.score_state(s, self.player_id.clone());
            }
            // rollout
            let rollout_score = if let Some(premature_end_score) = last_score {
                premature_end_score
            } else {
                self.rollout(&mut base_game)
            };
            // backprop
            self.backpropagate(&selected, rollout_score);
        }

        // self.tree.dump();
        // todo!()

        let (best_move, best_score) = self.best_descendant();
        println!("player {:?}: move {:?} ({best_score})", self.player_id, best_move.placement_move);
        best_move.placement_move.clone()
    }

    pub fn dump_tree(&self) {
        self.tree.dump();
    }
}
