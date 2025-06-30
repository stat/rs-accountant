use crossbeam_channel::{unbounded, Sender};
use csv::StringRecord;
use something::engine::{export_accounts, PaymentEngine};
use std::collections::HashMap;
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

    let num_cpus = num_cpus::get();

    let (senders, receivers): (Vec<_>, Vec<_>) =
        (0..num_cpus).map(|_| unbounded::<Vec<StringRecord>>()).unzip();

    let mut handles = Vec::new();
    let headers = get_headers(file_path)?;

    for receiver in receivers {
        let headers = headers.clone();
        let handle = thread::spawn(move || {
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
        handles.push(handle);
    }

    let file_path_clone = file_path.to_string();
    let dispatch_handle = thread::spawn(move || {
        dispatch_transactions(&file_path_clone, &senders).unwrap();
        // Drop senders to signal workers to finish
        for sender in senders {
            drop(sender);
        }
    });

    let mut final_accounts = HashMap::new();
    for handle in handles {
        let accounts = handle.join().unwrap();
        final_accounts.extend(accounts);
    }

    dispatch_handle.join().unwrap();
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
    senders: &[Sender<Vec<StringRecord>>],
) -> Result<(), Box<dyn Error>> {
    let num_senders = senders.len();
    let mut batches: Vec<Vec<StringRecord>> =
        (0..num_senders).map(|_| Vec::with_capacity(BATCH_SIZE)).collect();

    let file = std::fs::File::open(file_path)
        .map_err(|e| format!("Error opening file '{}': {}", file_path, e))?;

    let mut rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(file);

    for result in rdr.records() {
        let record = result?;
        let client_id_str = record.get(1).ok_or("Missing client_id")?;
        let client_id: u16 = client_id_str.trim().parse()?;
        let shard_index = (client_id as usize) % num_senders;

        batches[shard_index].push(record);

        if batches[shard_index].len() >= BATCH_SIZE {
            let full_batch =
                std::mem::replace(&mut batches[shard_index], Vec::with_capacity(BATCH_SIZE));
            senders[shard_index].send(full_batch)?;
        }
    }

    // Send any remaining partial batches
    for (i, batch) in batches.into_iter().enumerate() {
        if !batch.is_empty() {
            senders[i].send(batch)?;
        }
    }

    Ok(())
}
