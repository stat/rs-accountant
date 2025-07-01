use rand::{seq::SliceRandom, Rng};
use rust_decimal::Decimal;
use serde::Serialize;
use rs_accountant::engine::TransactionType;
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Write};

const NUM_CLIENTS: u16 = 1000;
const NUM_TRANSACTIONS: u32 = 35_000_000; // Approx. 1GB
const OUTPUT_FILE: &str = "large_input.csv";

#[derive(Debug, Serialize)]
struct GenTransaction {
    #[serde(rename = "type")]
    transaction_type: TransactionType,
    #[serde(rename = "client")]
    client_id: u16,
    #[serde(rename = "tx")]
    tx_id: u32,
    amount: Option<Decimal>,
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("Generating stress test file: {}...", OUTPUT_FILE);
    let file = File::create(OUTPUT_FILE)?;
    let mut wtr = BufWriter::new(file);

    writeln!(wtr, "type,client,tx,amount")?;

    let mut rng = rand::thread_rng();
    let mut valid_tx_ids: Vec<u32> = Vec::new();

    for tx_id in 1..=NUM_TRANSACTIONS {
        let client_id = rng.gen_range(1..=NUM_CLIENTS);
        let transaction_type = choose_transaction_type(&mut rng, &valid_tx_ids);

        let tx_id_for_dispute = if !valid_tx_ids.is_empty() {
            *valid_tx_ids.choose(&mut rng).unwrap()
        } else {
            1 // Should not be hit after first deposit
        };
        
        match transaction_type {
            TransactionType::Deposit => {
                // Generate amounts with varying decimal precision (2-4 decimal places)
                let scale = rng.gen_range(2..=4);
                let max_value = 10_i64.pow(scale + 2); // Adjust range based on scale
                let amount = Decimal::new(rng.gen_range(1..max_value), scale);
                writeln!(wtr, "deposit,{},{},{}", client_id, tx_id, amount)?;
                if valid_tx_ids.len() < 1000 { // Keep the list of disputable txs small
                    valid_tx_ids.push(tx_id);
                }
            }
            TransactionType::Withdrawal => {
                // Generate amounts with varying decimal precision (2-4 decimal places)
                let scale = rng.gen_range(2..=4);
                let max_value = 10_i64.pow(scale + 1); // Smaller range for withdrawals
                let amount = Decimal::new(rng.gen_range(1..max_value), scale);
                writeln!(wtr, "withdrawal,{},{},{}", client_id, tx_id, amount)?;
            }
            TransactionType::Dispute => {
                writeln!(wtr, "dispute,{},{},", client_id, tx_id_for_dispute)?;
            }
            TransactionType::Resolve => {
                writeln!(wtr, "resolve,{},{},", client_id, tx_id_for_dispute)?;
            }
            TransactionType::Chargeback => {
                 writeln!(wtr, "chargeback,{},{},", client_id, tx_id_for_dispute)?;
            }
        }
    }

    wtr.flush()?;
    println!("Successfully generated {} transactions to {}.", NUM_TRANSACTIONS, OUTPUT_FILE);
    Ok(())
}

fn choose_transaction_type(rng: &mut impl Rng, valid_tx_ids: &[u32]) -> TransactionType {
    if valid_tx_ids.is_empty() {
        return TransactionType::Deposit;
    }

    *[
        TransactionType::Deposit,
        TransactionType::Withdrawal,
        TransactionType::Dispute,
        TransactionType::Resolve,
        TransactionType::Chargeback,
    ]
    .choose_weighted(rng, |item| match item {
        TransactionType::Deposit => 40,
        TransactionType::Withdrawal => 30,
        TransactionType::Dispute => 10,
        TransactionType::Resolve => 10,
        TransactionType::Chargeback => 10,
    })
    .unwrap()
} 