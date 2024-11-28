use std::{cmp::Ordering, fmt};

use dialoguer::{theme::ColorfulTheme, Select};
use rand::seq::IteratorRandom;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

fn get_select_option() -> usize {
  let selections = &["Draw Card", "Quit"];

  let selection = Select::with_theme(&ColorfulTheme::default())
    .with_prompt("Pick an option:")
    .default(0)
    .items(&selections[..])
    .interact()
    .unwrap();

  selection
}

fn next_move(option: usize, cards: &mut PlayerCardList) {
  match option {
    0 => {
      cards.draw_card();
    }
    1 => {
      println!("Dealer's turn.\n");
    }
    _ => println!("No valid choice"),
  }
}

struct PlayerCardList(Vec<Cards>);

#[derive(EnumIter, Debug, PartialEq)]
enum Cards {
  TWO,
  THREE,
  FOUR,
  FIVE,
  SIX,
  SEVEN,
  EIGHT,
  NINE,
  TEN,
  JACK,
  QUEEN,
  KING,
  ACE,
}

#[derive(EnumIter, Debug, PartialEq)]
enum Players {
  YOU,
  DEALER,
}

impl fmt::Display for Players {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}

impl Cards {
  fn value(&self) -> i32 {
    match *self {
      Cards::TWO => 2,
      Cards::THREE => 3,
      Cards::FOUR => 4,
      Cards::FIVE => 5,
      Cards::SIX => 6,
      Cards::SEVEN => 7,
      Cards::EIGHT => 8,
      Cards::NINE => 9,
      Cards::TEN => 10,
      Cards::JACK => 10,
      Cards::KING => 10,
      Cards::QUEEN => 10,
      Cards::ACE => 11,
    }
  }
}

impl fmt::Display for Cards {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}

impl<'a> PartialEq<&'a Cards> for Cards {
  fn eq(&self, other: &&'a Cards) -> bool {
    self == *other
  }
}

impl<'a> PartialEq<Cards> for &'a Cards {
  fn eq(&self, other: &Cards) -> bool {
    *self == other
  }
}

impl PlayerCardList {
  fn draw_card(&mut self) {
    let mut rng = rand::thread_rng();
    let card = Cards::iter().choose(&mut rng).unwrap();
    self.0.push(card);
  }

  fn print_first_card(&mut self) {
    let first_card = &self.0[0];
    println!("DEALERS cards:\n");
    println!("Card: {} -> Value: {}", first_card, first_card.value());
    println!("Card: {} -> Value: {}\n", "???", "???");
  }

  fn print_cards(&mut self, player: Players) {
    println!("Cards from {}:\n", player);
    for card in &self.0 {
      println!("Card: {} -> Value: {}", card, card.value());
    }
    println!();
  }

  fn get_sum(&mut self) -> i32 {
    self.0.iter().map(|x| x.value()).sum()
  }
}

fn get_winner(user_points: i32, dealer_points: i32) -> &'static str {
  if user_points > 21 && dealer_points > 21 {
    return "Nobody";
  }
  if user_points <= 21 {
    if dealer_points > 21 {
      return "You";
    }
    match user_points.cmp(&dealer_points) {
      Ordering::Less => "Dealer",
      Ordering::Greater => "You",
      Ordering::Equal => "Dealer",
    }
  } else {
    return "Dealer";
  }
}

fn main() {
  let mut user_cards = PlayerCardList(vec![]);
  let mut dealer_cards = PlayerCardList(vec![]);
  // user_cards.0.push(Cards::ACE);
  // user_cards.0.push(Cards::ACE);
  user_cards.draw_card();
  user_cards.draw_card();
  dealer_cards.draw_card();
  dealer_cards.draw_card();

  // Dealer
  dealer_cards.print_first_card();
  while dealer_cards.get_sum() < 17 {
    dealer_cards.draw_card();
  }

  // Player
  loop {
    user_cards.print_cards(Players::YOU);
    let sum = user_cards.get_sum();
    println!("SUMME: {sum}\n");
    if sum >= 21 {
      break;
    }
    let opt = get_select_option();
    print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
    if opt == 1 && sum < 17 {
      println!("You need at least a score of 17 to proceed.\nYour current score is {sum}");
      continue;
    } else if opt == 1 {
      break;
    }
    next_move(opt, &mut user_cards);
  }

  dealer_cards.print_cards(Players::DEALER);
  println!("SUMME DEALER: {}\n", dealer_cards.get_sum());

  user_cards.print_cards(Players::YOU);
  println!("SUMME SPIELER: {}\n", user_cards.get_sum());

  let winner = get_winner(user_cards.get_sum(), dealer_cards.get_sum());
  print!("{winner} won the game!");
}
