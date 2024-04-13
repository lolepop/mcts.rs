use core::fmt;
use hashbrown::HashMap;
use anyhow::{anyhow, Result};

use crate::game::{Game, MoveScore};

pub(crate) enum GameState {
    Win,
    Continue
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum Colour { Red, Yellow, Green, Blue }

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum Card {
    Number(Colour, u8),
    Draw(Colour, u8),
    Reverse(Colour),
    Skip(Colour),
    Wild(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum PlayerMove {
    Number(Colour, u8),
    Draw(Colour, u8),
    Reverse(Colour),
    Skip(Colour),
    Wild(Colour, u8),
    // non placements
    ActionDraw
}
impl PlayerMove {
    fn as_card(self) -> Option<Card> {
        match self {
            PlayerMove::Number(c, n) => Some(Card::Number(c, n)),
            PlayerMove::Draw(c, n) => Some(Card::Draw(c, n)),
            PlayerMove::Reverse(c) => Some(Card::Reverse(c)),
            PlayerMove::Skip(c) => Some(Card::Skip(c)),
            PlayerMove::Wild(_, n) => Some(Card::Wild(n)),
            _ => None
        }
    }

    fn colour(&self) -> Option<Colour> {
        match self {
            PlayerMove::Number(c, _) | PlayerMove::Draw(c, _) | PlayerMove::Reverse(c) | PlayerMove::Skip(c) | PlayerMove::Wild(c, _) => Some(*c),
            _ => None,
        }
    }

    fn is_number(&self) -> bool {
        if let PlayerMove::Number(_, _) = self { true } else { false }
    }
}
impl Default for PlayerMove {
    fn default() -> Self {
        Self::Number(Colour::Red, 255)
    }
}


/// uno but all player hands are visible to one another
#[derive(Debug, Clone)]
pub(crate) struct Uno {
    deck: HashMap<Card, u8>,
    pub player_turn: usize,
    player_cards: Vec<HashMap<Card, u8>>,
    last_play: PlayerMove,
    card_purgatory: Vec<Card>, // card is left in here as part of playing stack, mixed back into deck once cards have run out
    reversed: bool,
    depth: usize
}

impl Uno {
    pub fn new(deck: HashMap<Card, u8>, num_players: usize, initial_player_cards: usize) -> Self {
        let mut o = Self {
            deck,
            player_turn: 0,
            player_cards: (0..num_players).map(|_| HashMap::new()).collect(),
            reversed: false,
            card_purgatory: Vec::new(),
            last_play: PlayerMove::default(),
            depth: 0,
        };

        for player in 0..num_players {
            o.draw_card_for_player(player, initial_player_cards).unwrap();
        }

        let first_card = *o.draw_hand(1, true).unwrap().keys().next().unwrap();
        let Card::Number(first_colour, first_number) = first_card else { unreachable!() };
        o.last_play = PlayerMove::Number(first_colour, first_number);
        o.card_purgatory = vec![first_card];

        o
    }

    fn draw_card_for_player(&mut self, player: usize, cards: usize) -> Result<()> {
        let drawn = self.draw_hand(cards, false)?;
        let deck = self.player_cards.get_mut(player).ok_or_else(|| anyhow!("invalid player"))?;
        // merge cards into existing deck
        for (c, n) in drawn.iter() {
            if let Some(existing_count) = deck.get_mut(c) {
                *existing_count += n;
            } else {
                let _ = deck.insert(*c, *n);
            }
        }
        Ok(())
    }

    fn draw_hand(&mut self, cards: usize, number_only: bool) -> Result<HashMap<Card, u8>> {
        let mut hand = HashMap::new();
        let mut drawn = 0;
        let mut failed_once = false;
        while drawn < cards {
            if let Some(chosen_card) = self.random_weighted_card(number_only) {
                if let Some(n) = hand.get_mut(&chosen_card) {
                    *n += 1;
                } else {
                    hand.insert(chosen_card, 1);
                }
                *self.deck.get_mut(&chosen_card).unwrap() -= 1;
                drawn += 1;
            } else {
                self.shift_purgatory_into_stack();
                if failed_once {
                    return Err(anyhow!("not enough cards to deal hand: require {cards}, have {:?}", self.deck));
                }
                failed_once = true;
            }
        }
        Ok(hand)
    }

    fn random_weighted_card(&self, number_only: bool) -> Option<Card> {
        let random = rand::random::<f32>();
        let valid_cards = self.deck.iter()
            .filter(|(c, n)|
                **n > 0 &&
                    (!number_only || match c { Card::Number(_, _) => true, _ => false })
            );
        let total_cards: f32 = valid_cards
            .clone()
            .map(|(_, n)| *n as f32)
            .sum();
        let mut acc_card_weight = 0f32;
        for (card, n) in valid_cards {
            acc_card_weight += (*n as f32) / total_cards;
            if acc_card_weight >= random {
                return Some(*card);
            }
        }
        None
    }

    fn player_card_count(&self, player: usize) -> usize {
        self.player_cards[player].iter()
            .filter_map(|(_, n)| (*n > 0).then(|| *n as usize))
            .sum()
    }

    fn shift_purgatory_into_stack(&mut self) {
        if self.card_purgatory.len() > 1 {
            for c in self.card_purgatory.drain(0..self.card_purgatory.len() - 1) {
                *self.deck.get_mut(&c).unwrap() += 1;
            }
        }
    }

    pub fn standard_deck(num_players: usize) -> Self {
        let deck = [Colour::Red, Colour::Yellow, Colour::Green, Colour::Blue].iter()
            .flat_map(|colour|
                (1..=9)
                    .map(|n| (Card::Number(*colour, n), 2))
                    .chain([
                        (Card::Number(*colour, 0), 1),
                        (Card::Draw(*colour, 2), 2),
                        (Card::Reverse(*colour), 2),
                        (Card::Skip(*colour), 2),
                        (Card::Wild(0), 2),
                        (Card::Wild(4), 2),
                    ])
            )
            .collect::<HashMap<_, u8>>();
        Self::new(deck, num_players, 7)
    }

    fn update_move(&mut self, card: Option<Card>, pmove: PlayerMove) -> Result<bool> {
        if let Some(card) = card {
            *self.player_cards[self.player_turn].get_mut(&card).ok_or_else(|| anyhow!("card does not exist in player deck: {pmove:?}"))? -= 1;
            self.card_purgatory.push(card);
            self.last_play = pmove;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn next_player(&mut self, scale: u8) {
        let rev = self.reversed.then(|| -1).unwrap_or(1);
        let num_players = self.player_cards.len() as isize;
        let next_turn = (self.player_turn as isize) + (scale as isize) * (rev as isize);
        let next_turn = ((next_turn % num_players) + num_players) % num_players;
        self.player_turn = next_turn as usize;
    }

}

impl Game for Uno {
    const IS_PERFECT_INFORMATION: bool = false;

    type Move = PlayerMove;
    type GameState = GameState;
    type Player = usize;

    // TODO: possibly randomise all other players hands on first move for regular uno
    fn possible_moves(&self) -> Vec<Self::Move> {
        let player_deck = &self.player_cards[self.player_turn];
        let number_cards_in_deck = self.player_card_count(self.player_turn);
        if number_cards_in_deck == 0 {
            return Vec::new();
        }

        let is_last_card = number_cards_in_deck == 1;
        let last_move_colour = self.last_play.colour().unwrap();

        // turn into for loop, move conditions in move to 
        let mut moves = vec![];
        for (card, _) in player_deck.iter().filter(|(_, n)| **n > 0) {
            match card {
                // same colour or same number
                Card::Number(c, n) => {
                    let m = PlayerMove::Number(*c, *n);
                    if last_move_colour == *c {
                        moves.push(m);
                    } else if let PlayerMove::Number(_, ln) = self.last_play {
                        if *n == ln {
                            moves.push(m);
                        }
                    }
                },
                // same colour only
                Card::Draw(c, n) if last_move_colour == *c && !is_last_card => {
                    moves.push(PlayerMove::Draw(*c, *n));
                },
                Card::Reverse(c) if last_move_colour == *c && !is_last_card => {
                    moves.push(PlayerMove::Reverse(*c));
                },
                Card::Skip(c) if last_move_colour == *c && !is_last_card => {
                    moves.push(PlayerMove::Skip(*c));
                },
                Card::Wild(n) if !is_last_card => {
                    for c in [Colour::Red, Colour::Yellow, Colour::Green, Colour::Blue] {
                        moves.push(PlayerMove::Wild(c, *n))
                    }
                },
                _ => {}
            }
        }

        // disallow hoarding all cards
        if moves.len() == 0 {
            moves.push(PlayerMove::ActionDraw);
        }
        moves
    }

    // each player gets single move per turn, drawing will count as a move and move to next player even if card can be placed
    fn place_move(&mut self, movement: Self::Move) -> anyhow::Result<Self::GameState> {
        // println!("attempt {movement:?}");
        // place move on top, shift card to purgatory
        // add on to draw card move if more are placed
        // let player_deck = &self.player_cards[self.player_turn];
        let possible_moves = self.possible_moves();
        let is_valid_move = possible_moves.iter().any(|m| m == &movement);
        if !is_valid_move {
            println!("{self}");
            return Err(anyhow!("move specified is invalid: {movement:?}, valid moves: {possible_moves:?}"));
        }

        // if drawing a card fails (when there are no more cards left), will throw error. should fix by skipping player turn until they can place a card. happens infrequently though
        let mut handled = false;
        let mut next_turn_scale = 1;
        match self.last_play {
            PlayerMove::Draw(c, pending_cards) | PlayerMove::Wild(c, pending_cards) => {
                // println!("{pending_cards}");
                // this is so stupidly annoying to deal with
                match movement {
                    PlayerMove::Draw(_, n) => {
                        self.update_move(movement.as_card(), PlayerMove::Draw(c, n + pending_cards))?;
                        handled = true;
                    },
                    PlayerMove::Wild(c, n) => {
                        self.update_move(movement.as_card(), PlayerMove::Wild(c, n + pending_cards))?;
                        handled = true;
                    },
                    PlayerMove::ActionDraw => {
                        if pending_cards > 0 {
                            self.draw_card_for_player(self.player_turn, pending_cards as usize - 1)?;
                        }
                    },
                    _ => {
                        self.draw_card_for_player(self.player_turn, pending_cards as usize)?;
                    }
                }
            },
            _ => {}
        }

        if !handled {
            match movement {
                PlayerMove::Reverse(_) => self.reversed = !self.reversed,
                PlayerMove::Skip(_) => next_turn_scale = 2,
                PlayerMove::ActionDraw => self.draw_card_for_player(self.player_turn, 1)?,
                _ => {}
            }
            self.update_move(movement.as_card(), movement)?;
        }

        self.depth += 1;

        if self.player_card_count(self.player_turn) == 0 {
            Ok(GameState::Win)
        } else {
            self.next_player(next_turn_scale);
            Ok(GameState::Continue)
        }
    }

    fn score_state(&self, state: Self::GameState, player: Self::Player) -> crate::game::MoveScore {
        match state {
            GameState::Win => MoveScore::Terminal(if self.player_turn == player { 1.0 } else { 0.0 }),
            GameState::Continue => 
            // we can wait lmfao
            // if self.depth > 1000 {
            //     MoveScore::Terminal(-(self.depth as f32) * 0.5)
            // } else {
                MoveScore::None
            // }
        }
    }
}

impl fmt::Display for Uno {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let player_cards = self.player_cards[self.player_turn].iter()
            .flat_map(|(c, n)| (0..*n).map(|_| *c))
            .collect::<Vec<_>>();
        write!(f, "player {}\n{:?}\nlast move: {:?}\nmoves: {:?}\nhand: {:?}", self.player_turn, self.card_purgatory.last().unwrap(), self.last_play, self.possible_moves(), player_cards)
    }
}

