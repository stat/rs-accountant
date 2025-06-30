use crossbeam_channel::unbounded;
use csv::StringRecord;
use something::engine::{export_accounts, InputTransaction, PaymentEngine};
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

    // Channel 1: From I/O thread to Deserialization thread
    let (raw_sender, raw_receiver) = unbounded::<Vec<StringRecord>>();
    // Channel 2: From Deserialization thread to Processing thread
    let (tx_sender, tx_receiver) = unbounded::<Vec<InputTransaction>>();

    let headers = get_headers(file_path)?;

    // Thread 3: Processing
    // Receives fully typed transactions and processes them.
    let processing_handle = thread::spawn(move || {
        let mut engine = PaymentEngine::new();
        while let Ok(batch) = tx_receiver.recv() {
            for tx in batch {
                match tx.transaction_type {
                    something::engine::TransactionType::Deposit => engine.handle_deposit(tx),
                    something::engine::TransactionType::Withdrawal => engine.handle_withdrawal(tx),
                    something::engine::TransactionType::Dispute => engine.handle_dispute(tx),
                    something::engine::TransactionType::Resolve => engine.handle_resolve(tx),
                    something::engine::TransactionType::Chargeback => engine.handle_chargeback(tx),
                }
            }
        }
        engine.accounts
    });

    // Thread 2: Deserialization
    // Receives raw records, deserializes them, and sends typed transactions to the next stage.
    let deserialization_handle = thread::spawn(move || {
        while let Ok(batch) = raw_receiver.recv() {
            let mut tx_batch = Vec::with_capacity(batch.len());
            for record in batch {
                if let Ok(tx) = record.deserialize::<InputTransaction>(Some(&headers)) {
                    tx_batch.push(tx);
                }
            }
            if tx_sender.send(tx_batch).is_err() {
                break;
            }
        }
        drop(tx_sender);
    });

    // Thread 1: I/O
    // Reads the file and sends raw records to the deserialization stage.
    let file_path_clone = file_path.to_string();
    let io_handle = thread::spawn(move || {
        read_and_parse_transactions(&file_path_clone, raw_sender).unwrap();
    });

    io_handle.join().unwrap();
    deserialization_handle.join().unwrap();
    let final_accounts = processing_handle.join().unwrap();

    export_accounts(&final_accounts, io::stdout())?;

    Ok(())
}

fn get_headers(file_path: &str) -> Result<csv::StringRecord, Box<dyn Error>> {
    let mut rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_path(file_path)?;
    Ok(rdr.headers()?.clone())
}

fn read_and_parse_transactions(
    file_path: &str,
    sender: crossbeam_channel::Sender<Vec<StringRecord>>,
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
