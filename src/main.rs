use crossbeam_channel::{unbounded, Sender};
use csv::StringRecord;
use something::engine::{export_accounts, PaymentEngine};
use std::error::Error;
use std::io;
use std::thread;

const BATCH_SIZE: usize = 1024;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: cargo run -- <input_file.csv>");
        return Err("Invalid arguments".into());
    }
    let file_path = &args[1];

    let (sender, receiver) = unbounded::<Vec<StringRecord>>();

    let headers = get_headers(file_path)?;
    let worker_handle = thread::spawn(move || {
        let mut engine = PaymentEngine::new();
        while let Ok(batch) = receiver.recv() {
            for record in batch {
                if let Ok(tx) =
                    record.deserialize::<something::engine::InputTransaction>(Some(&headers))
                {
                    match tx.transaction_type {
                        something::engine::TransactionType::Deposit => engine.handle_deposit(tx),
                        something::engine::TransactionType::Withdrawal => {
                            engine.handle_withdrawal(tx)
                        }
                        something::engine::TransactionType::Dispute => engine.handle_dispute(tx),
                        something::engine::TransactionType::Resolve => engine.handle_resolve(tx),
                        something::engine::TransactionType::Chargeback => {
                            engine.handle_chargeback(tx)
                        }
                    }
                }
            }
        }
        engine.accounts
    });

    let file_path_clone = file_path.to_string();
    let dispatch_handle = thread::spawn(move || {
        dispatch_transactions(&file_path_clone, sender).unwrap();
    });

    dispatch_handle.join().unwrap();
    let final_accounts = worker_handle.join().unwrap();

    export_accounts(&final_accounts, io::stdout())?;

    Ok(())
}

fn get_headers(file_path: &str) -> Result<csv::StringRecord, Box<dyn Error>> {
    let mut rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_path(file_path)?;
    Ok(rdr.headers()?.clone())
}

fn dispatch_transactions(
    file_path: &str,
    sender: Sender<Vec<StringRecord>>,
) -> Result<(), Box<dyn Error>> {
    let mut batch: Vec<StringRecord> = Vec::with_capacity(BATCH_SIZE);

    let file = std::fs::File::open(file_path)
        .map_err(|e| format!("Error opening file '{}': {}", file_path, e))?;

    let mut rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(file);

    for result in rdr.records() {
        batch.push(result?);

        if batch.len() >= BATCH_SIZE {
            let full_batch = std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE));
            if sender.send(full_batch).is_err() {
                break;
            }
        }
    }

    if !batch.is_empty() {
        let _ = sender.send(batch);
    }

    Ok(())
}
