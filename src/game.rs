use anyhow::Result;
use std::{collections::VecDeque, fmt::Debug, fs::File, io::Write, ops::Deref};
use rand::seq::{IteratorRandom, SliceRandom};

fn uct(score: f32, visits: u32, total_visits: u32, c: f32) -> f32 {
    if visits == 0 {
        f32::INFINITY
    } else {
        score / (visits as f32) + c * ((total_visits as f32).ln() / (visits as f32)).sqrt()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MoveScore {
    Terminal(f32),
    NonTerminal(f32),
    None
}

impl MoveScore {
    fn is_terminal(&self) -> bool {
        match self {
            MoveScore::Terminal(_) => true,
            _ => false
        }
    }

    fn score(&self) -> f32 {
        match self {
            MoveScore::Terminal(s) | MoveScore::NonTerminal(s) => *s,
            MoveScore::None => 0f32,
        }
    }
}

pub trait Game: Clone + Debug {
    const IS_PERFECT_INFORMATION: bool;

    type Move: Default + Debug + Clone + PartialEq;
    type GameState;
    type Player: Clone + Debug;

    fn possible_moves(&self) -> Vec<Self::Move>;
    fn place_move(&mut self, movement: Self::Move) -> Result<Self::GameState>;
    /// returns score used for backpropagation.
    /// none if state is not terminal
    fn score_state(&self, state: Self::GameState, player: Self::Player) -> MoveScore;
}

#[derive(Debug, Clone, Copy)]
struct NodeId(usize);
impl Deref for NodeId {
    type Target = usize;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
    
}

#[derive(Clone, Debug)]
struct MctsNode<Move: Default + Debug> {
    pub placement_move: Move,
    pub score: f32,
    pub visits: u32,
}

struct MctsTree<Move: Default + Debug> {
    nodes: Vec<MctsNode<Move>>,
    children: Vec<Vec<NodeId>>,
}

impl<Move: Default + Debug> MctsTree<Move> {
    fn new() -> Self {
        Self {
            nodes: vec![MctsNode { placement_move: Move::default(), score: 0f32, visits: 0 }],
            children: vec![Vec::new()],
        }
    }

    fn add_child(&mut self, node: NodeId, placement_move: Move) -> Option<NodeId> {
        if *node < self.nodes.len() {
            let id = NodeId(self.nodes.len());
            self.children[*node].push(id);
            self.nodes.push(MctsNode { placement_move, score: 0f32, visits: 0 });
            self.children.push(Vec::new());
            Some(id)
        } else {
            None
        }
    }

    fn children(&self, node: NodeId) -> Option<Vec<NodeId>> {
        self.children.get(*node).cloned()
    }

    fn node(&self, node: NodeId) -> Option<&MctsNode<Move>> {
        self.nodes.get(*node)
    }

    fn dump(&self) {
        let mut f = File::create("./out.dot").unwrap();
        f.write("digraph G {overlap=\"scalexy;\"".as_bytes()).unwrap();
        let mut queue = VecDeque::from([0usize]);
        while let Some(parent) = queue.pop_front() {
            if let Some(children) = self.children.get(parent) {
                for child in children {
                    let child = **child;
                    let child_stats = &self.nodes[child];
                    if child_stats.visits > 0 {
                        f.write(format!("{parent}->{child};").as_bytes()).unwrap();
                        f.write(format!("{child} [label=<{child}<br/>move={:?}<br/>score={}<br/>visits={}>];", child_stats.placement_move, child_stats.score, child_stats.visits).as_bytes()).unwrap();
                    }
                    queue.push_back(child);
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
    root: NodeId,
}

impl<G: Game> Mcts<G> {
    pub fn new(player_id: G::Player) -> Self {
        Self { tree: MctsTree::new(), root: NodeId(0), player_id }
    }

    fn diff_existing_children(&self, existing: &Vec<NodeId>, truth: &Vec<G::Move>) -> Option<Vec<G::Move>> {
        let diff = existing.iter()
            .filter_map(|n| self.tree.node(*n))
            .filter_map(|n| truth.contains(&n.placement_move)
                .then(|| n.placement_move.clone())
            )
            .collect::<Vec<_>>();
        (diff.len() > 0).then(|| diff)
    }

    fn select(&mut self, game: &mut G) -> Vec<(NodeId, MoveScore)> {
        let mut traversal = vec![(self.root, MoveScore::None)];
        let mut pending_move_diff: Option<Vec<G::Move>> = None;
        loop {
            let (last_id, _) = *traversal.last().unwrap();
            let node_children = self.tree.children(last_id).unwrap();
            if node_children.len() == 0 {
                // println!("selection: {traversal:?}");
                break;
            }

            // scan current moves, checking if does not match all moves in existing children nodes
            if !G::IS_PERFECT_INFORMATION {
                if let Some(diff) = self.diff_existing_children(&node_children, &game.possible_moves()) {
                    pending_move_diff.replace(diff);
                    break;
                }
            }

            let stats = node_children.iter().filter_map(|n| self.tree.node(*n));
            // let total_visits = stats.clone().fold(0, |acc, s| acc + s.visits);
            let total_visits = self.tree.node(last_id).unwrap().visits;
            let (selected_node, _best_uct, placement_move) = stats.enumerate()
                    .map(|(i, s)| (i, uct(s.score, s.visits, total_visits, 2f32.sqrt()), s.placement_move.clone()))
                    .max_by(|(_, a, _), (_, b, _)| a.total_cmp(b))
                    .unwrap();
            
            let s = game.place_move(placement_move).unwrap();
            let last_score = game.score_state(s, self.player_id.clone());
            traversal.push((node_children[selected_node], last_score));

        }

        
        // expansion step
        let (selected_node, last_score) = *traversal.last().unwrap();

        // exit early if terminal node was selected
        if last_score.is_terminal() {
            return traversal;
        }

        let next_selection = if let Some(mut pending_move_diff) = pending_move_diff {
            // extension of selection, expand but take into account only new nodes - only randomly select from new nodes
            *pending_move_diff.iter_mut()
                .filter_map(|placement_move| self.tree.add_child(selected_node, placement_move.clone()))
                .collect::<Vec<_>>() // hack
                .choose(&mut rand::thread_rng())
                .unwrap()
        } else {
            for m in game.possible_moves() {
                self.tree.add_child(selected_node, m);
            }
    
            *self.tree.children(selected_node).unwrap().choose(&mut rand::thread_rng()).unwrap()
        };

        let s = game.place_move(self.tree.node(next_selection).unwrap().placement_move.clone()).unwrap();
        let last_score = game.score_state(s, self.player_id.clone());

        traversal.push((next_selection, last_score));
        traversal
    }

    // only returns scoring of terminal state
    fn rollout(&mut self, game: &mut G) -> f32 {
        let mut acc_score = 0f32;
        loop {
            let random_move = game.possible_moves().choose(&mut rand::thread_rng()).unwrap().clone();
            let s = game.place_move(random_move).unwrap();
            let score = game.score_state(s, self.player_id.clone());
            acc_score += score.score();
            if score.is_terminal() {
                return acc_score;
            }
        }
    }

    fn backpropagate(&mut self, traversal: &Vec<(NodeId, MoveScore)>, rollout_score: f32) {
        let mut acc_score = rollout_score;
        for (id, move_score) in traversal.iter().rev() {
            acc_score += move_score.score();
            let n = &mut self.tree.nodes[**id];
            n.visits += 1;
            n.score += acc_score;
        }
    }

    // calculate best average score
    fn best_descendant(&self) -> (&MctsNode<<G as Game>::Move>, f32) {
        self.tree.children[*self.root].iter()
            .filter_map(|n| self.tree.node(*n))
            // .map(|n| (n, n.score / (n.visits as f32)))
            .map(|n| (n, n.visits as f32))
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .unwrap()
    }

    pub fn best_move(&mut self, base_game: &G, iterations: usize) -> G::Move {
        for _ in 0..iterations {
            let mut game = base_game.clone();
            // let mut last_score: Option<f32> = None;
            // select and expand
            let selected = self.select(&mut game);

            let (_, last_score) = selected.last().unwrap();

            // rollout
            let rollout_score = if !last_score.is_terminal() {
                self.rollout(&mut game)
            } else {
                0f32
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
