use std::{env, process};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    match mole_cli::run_cli(&args) {
        Ok(output) => {
            println!("{output}");
        }
        Err(error) => {
            eprintln!("{error}");
            process::exit(2);
        }
    }
}
