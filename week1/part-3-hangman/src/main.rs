// Simple Hangman Program
// User gets five incorrect guesses
// Word chosen randomly from words.txt
// Inspiration from: https://doc.rust-lang.org/book/ch02-00-guessing-game-tutorial.html
// This assignment will introduce you to some fundamental syntax in Rust:
// - variable declaration
// - string manipulation
// - conditional statements
// - loops
// - vectors
// - files
// - user input
// We've tried to limit/hide Rust's quirks since we'll discuss those details
// more in depth in the coming lectures.
extern crate rand;
use rand::Rng;
use std::fs;
use std::io;
use std::io::Write;
const NUM_INCORRECT_GUESSES: u32 = 5;
const WORDS_PATH: &str = "words.txt";
const BLANK: char = '-';

fn pick_a_random_word() -> String {
    let file_string = fs::read_to_string(WORDS_PATH).expect("Unable to read file.");
    let words: Vec<&str> = file_string
        .split('\n')
        .map(str::trim)
        .filter(|word| !word.is_empty())
        .collect();
    assert!(!words.is_empty(), "words.txt contains no words");
    words[rand::thread_rng().gen_range(0, words.len())].to_string()
}

enum Invalid {
    FlushErr,
    ReadlineErr,
}

impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Invalid::FlushErr => write!(f, "could not write to stdout"),
            Invalid::ReadlineErr => write!(f, "could not read from stdin"),
        }
    }
}

// Is `ch` already among the characters guessed so far?
fn is_in_schars(schars: &[char], ch: char) -> bool {
    schars.contains(&ch)
}

// Prompts until the user types exactly one ASCII letter.
fn user_guess() -> Result<char, Invalid> {
    loop {
        print!("Please guess a letter: ");
        io::stdout().flush().map_err(|_| Invalid::FlushErr)?;

        let mut guess = String::new();
        let bytes_read = io::stdin()
            .read_line(&mut guess)
            .map_err(|_| Invalid::ReadlineErr)?;
        if bytes_read == 0 {
            // EOF: nothing more will ever arrive.
            return Err(Invalid::ReadlineErr);
        }

        let mut chars = guess.trim().chars();
        if let (Some(ch), None) = (chars.next(), chars.next()) {
            if ch.is_ascii_alphabetic() {
                return Ok(ch.to_ascii_lowercase());
            }
        }
        println!("Please enter exactly one letter (a-z).");
    }
}

// Game body. Prints exactly the format the grader expects.
fn hangman(secret_word: &[char]) {
    println!("Welcome to CS110L Hangman!");

    let mut revealed = vec![false; secret_word.len()];
    let mut guessed: Vec<char> = Vec::new();
    let mut incorrect: u32 = 0;

    loop {
        let word_so_far: String = secret_word
            .iter()
            .zip(&revealed)
            .map(|(&ch, &shown)| if shown { ch } else { BLANK })
            .collect();
        println!("The word so far is {}", word_so_far);
        println!(
            "You have guessed the following letters: {}",
            guessed.iter().collect::<String>()
        );
        println!(
            "You have {} guesses left",
            NUM_INCORRECT_GUESSES - incorrect
        );

        let guess = match user_guess() {
            Ok(ch) => ch,
            Err(err) => {
                println!("\nInput error: {}. Exiting.", err);
                return;
            }
        };

        if is_in_schars(&guessed, guess) {
            println!("You already guessed that letter");
            println!();
            continue;
        }
        guessed.push(guess);

        let mut hit = false;
        for (i, &ch) in secret_word.iter().enumerate() {
            if ch == guess {
                revealed[i] = true;
                hit = true;
            }
        }

        if !hit {
            println!("Sorry, that letter is not in the word");
            incorrect += 1;
        }

        println!();

        if revealed.iter().all(|&shown| shown) {
            println!(
                "Congratulations you guessed the secret word: {}!",
                secret_word.iter().collect::<String>()
            );
            return;
        }
        if incorrect >= NUM_INCORRECT_GUESSES {
            println!("Sorry, you ran out of guesses!");
            return;
        }
    }
}

fn main() {
    let secret_word = pick_a_random_word();
    // Note: given what you know about Rust so far, it's easier to pull characters out of a
    // vector than it is to pull them out of a string. You can get the ith character of
    // secret_word by doing secret_word_chars[i].
    let secret_word_chars: Vec<char> = secret_word.chars().collect();
    // Uncomment for debugging:
    // println!("random word: {}", secret_word);

    hangman(&secret_word_chars);
}
 
