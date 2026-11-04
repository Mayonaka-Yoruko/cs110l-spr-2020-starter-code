use std::env;
use std::fs::File;
use std::io::{self, BufRead};
use std::process;

fn read_file_lines(filename: &str) -> Result<Vec<String>, io::Error> {
    let mut res: Vec<String> = Vec::new();
    let file = File::open(filename)?;
    for line in io::BufReader::new(file).lines() {
        res.push(line?);
    }
    Ok(res)
}

/// 依次打印：单词数、行数、字符数。
fn print_word_line_chcount(seq: &[String]) {
    let words: usize = seq.iter().map(|l| l.split_whitespace().count()).sum();
    let lines = seq.len();
    let chars: usize = seq.iter().map(|l| l.chars().count()).sum();
    println!("{words} {lines} {chars}");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Too few arguments.");
        process::exit(1);
    }
    let filename = &args[1];

    let seq = match read_file_lines(filename) {
        Ok(seq) => seq,
        Err(e) => {
            eprintln!("rwc: {filename}: {e}");
            process::exit(1);
        }
    };
    print_word_line_chcount(&seq);
}
