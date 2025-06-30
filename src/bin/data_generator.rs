use rand::{seq::SliceRandom, Rng};
use rust_decimal::Decimal;
use serde::Serialize;
use something::engine::{InputTransaction, PaymentEngine, TransactionType};
use std::error::Error;

const NUM_CLIENTS: u16 = 50;
const NUM_TRANSACTIONS: u32 = 1000;
const OUTPUT_INPUT_FILE: &str = "e2e_input.csv";
const OUTPUT_EXPECTED_FILE: &str = "e2e_expected_output.csv";

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
    let mut rng = rand::thread_rng();
    let mut transactions = Vec::new();
    let mut engine = PaymentEngine::new();

    let mut valid_tx_ids: Vec<u32> = Vec::new();

    for tx_id in 1..=NUM_TRANSACTIONS {
        let client_id = rng.gen_range(1..=NUM_CLIENTS);
        let transaction_type = choose_transaction_type(&mut rng, &valid_tx_ids);

        let tx = match transaction_type {
            TransactionType::Deposit => {
                let amount = Decimal::new(rng.gen_range(1..10000), 2);
                valid_tx_ids.push(tx_id);
                InputTransaction {
                    transaction_type,
                    client_id,
                    tx_id,
                    amount: Some(amount),
                }
            }
            TransactionType::Withdrawal => {
                let amount = Decimal::new(rng.gen_range(1..5000), 2);
                 InputTransaction {
                    transaction_type,
                    client_id,
                    tx_id,
                    amount: Some(amount),
                }
            }
            TransactionType::Dispute | TransactionType::Resolve | TransactionType::Chargeback => {
                let target_tx_id = loop {
                    let id = *valid_tx_ids.choose(&mut rng).unwrap();
                    if engine.transactions.contains_key(&id) {
                        break id;
                    }
                };

                InputTransaction {
                    transaction_type,
                    client_id: engine.transactions.get(&target_tx_id).unwrap().client_id,
                    tx_id: target_tx_id,
                    amount: None,
                }
            }
        };
        
        // Process the transaction with our engine to calculate the expected state
        match tx.transaction_type {
            TransactionType::Deposit => engine.handle_deposit(tx.clone()),
            TransactionType::Withdrawal => engine.handle_withdrawal(tx.clone()),
            TransactionType::Dispute => engine.handle_dispute(tx.clone()),
            TransactionType::Resolve => engine.handle_resolve(tx.clone()),
            TransactionType::Chargeback => engine.handle_chargeback(tx.clone()),
        }
        transactions.push(tx);
    }

    // Write the generated transactions to the input file
    let mut wtr = csv::Writer::from_path(OUTPUT_INPUT_FILE)?;
    for tx in transactions {
        let gen_tx = GenTransaction {
            transaction_type: tx.transaction_type,
            client_id: tx.client_id,
            tx_id: tx.tx_id,
            amount: tx.amount,
        };
        wtr.serialize(gen_tx)?;
    }
    wtr.flush()?;
    
    // Write the expected final account states
    let mut wtr_expected = csv::Writer::from_path(OUTPUT_EXPECTED_FILE)?;
    let mut accounts: Vec<_> = engine.accounts.values().collect();
    accounts.sort_by_key(|a| a.id);

    for account in accounts {
        wtr_expected.serialize(something::engine::OutputAccount::from(account))?;
    }
    wtr_expected.flush()?;

    println!("Generated {} transactions for {} clients.", NUM_TRANSACTIONS, NUM_CLIENTS);
    println!("Input file: {}", OUTPUT_INPUT_FILE);
    println!("Expected output file: {}", OUTPUT_EXPECTED_FILE);
    
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