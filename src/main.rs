use something::engine::PaymentEngine;
use std::error::Error;
use std::fs::File;
use std::io;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: cargo run -- <input_file.csv>");
        return Err("Invalid arguments".into());
    }
    let file_path = &args[1];
    let file = File::open(file_path)?;

    let mut engine = PaymentEngine::new();
    engine.process_transactions(file)?;
    engine.export_accounts(io::stdout())?;

    Ok(())
}
