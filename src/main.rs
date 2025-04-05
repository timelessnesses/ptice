#![feature(try_from)]

//! i honestly ported this from latest rust (1.86.0) to 1.33

use std::io::Write;
use std::convert::{TryFrom, TryInto};

#[derive(Debug, Clone, Copy)]
enum StupidIndividualGuessingTheTest {
    Adrian,
    Bruno,
    Goran,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Choice {
    A,
    B,
    C,
}

impl TryFrom<char> for Choice {
    type Error = String;

    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            'A' => Ok(Choice::A),
            'B' => Ok(Choice::B),
            'C' => Ok(Choice::C),
            _ => Err(format!("Invalid choice: {}", value)),
        }
    }
}

impl StupidIndividualGuessingTheTest {
    fn get_pattern(&self) -> &'static [Choice] {
        match self {
            StupidIndividualGuessingTheTest::Adrian => &[Choice::A, Choice::B, Choice::C],
            StupidIndividualGuessingTheTest::Bruno => &[Choice::B, Choice::A, Choice::B, Choice::C],
            StupidIndividualGuessingTheTest::Goran => &[
                Choice::C,
                Choice::C,
                Choice::A,
                Choice::A,
                Choice::B,
                Choice::B,
            ],
        }
    }

    fn get_all_individuals() -> Vec<StupidIndividualGuessingTheTest> {
        vec![
            StupidIndividualGuessingTheTest::Adrian,
            StupidIndividualGuessingTheTest::Bruno,
            StupidIndividualGuessingTheTest::Goran,
        ]
    }
}

fn main() {
    // the original problem provided us with amount of tests we going to get (which is useful if you use C to solve this lmao)
    let _: u128 = input(None::<&str>);
    let revealed_correct_answers: Vec<Choice> = input::<String, _>(None::<&str>)
        .chars()
        .map(|c| c.try_into().unwrap())
        .collect();
    let mut scores: Vec<(StupidIndividualGuessingTheTest, u128)> =
        StupidIndividualGuessingTheTest::get_all_individuals()
            .iter()
            .map(|individual| {
                let pattern = individual.get_pattern();
                let score = revealed_correct_answers
                    .iter()
                    .zip(pattern.iter().cycle())
                    .filter(|(a, b)| a == b)
                    .count() as u128;
                (*individual, score)
            })
            .collect();
    scores.sort_unstable_by(|a, b| b.1.cmp(&a.1));
    let max_score = scores[0].1;
    let winners: Vec<StupidIndividualGuessingTheTest> = scores
        .iter()
        .filter(|(_, score)| *score == max_score)
        .map(|(individual, _)| *individual)
        .collect();
    let mut winners_str = winners
        .iter()
        .map(|individual| format!("{:?}", individual))
        .collect::<Vec<String>>();
    winners_str.sort(); // alphabetical
    println!("{}", max_score);
    println!("{}", winners_str.join("\n"));
}

fn input<T, U>(ask: Option<U>) -> T
where
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Debug,
    U: AsRef<str>
{
    let mut input = String::new();
    if let Some(ask) = ask {
        print!("{}: ", ask.as_ref());
        std::io::stdout().flush().unwrap();
    }
    std::io::stdin().read_line(&mut input).unwrap();
    input.trim().parse().unwrap()
}
