use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::io;

/// A unique identifier for a client.
pub type ClientId = u16;
/// A unique identifier for a transaction.
pub type TransactionId = u32;

/// The type of a transaction.
#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

/// Represents a transaction read from the input CSV.
#[derive(Debug, Deserialize, Clone)]
pub struct InputTransaction {
    /// The type of the transaction.
    #[serde(rename = "type")]
    pub transaction_type: TransactionType,
    /// The ID of the client performing the transaction.
    #[serde(rename = "client")]
    pub client_id: ClientId,
    /// The ID of the transaction.
    #[serde(rename = "tx")]
    pub tx_id: TransactionId,
    /// The amount of the transaction, if applicable.
    pub amount: Option<Decimal>,
}

/// Represents a client account for serialization to CSV.
#[derive(Debug, Serialize, Deserialize)]
pub struct OutputAccount {
    #[serde(rename = "client")]
    id: ClientId,
    #[serde(with = "serde_decimal")]
    available: Decimal,
    #[serde(with = "serde_decimal")]
    held: Decimal,
    #[serde(with = "serde_decimal")]
    total: Decimal,
    locked: bool,
}

mod serde_decimal {
    use rust_decimal::Decimal;
    use serde::{self, Deserializer, Serializer, Deserialize};

    pub fn serialize<S>(val: &Decimal, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{:.4}", val))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse::<Decimal>().map_err(serde::de::Error::custom)
    }
}

impl<'a> From<&'a Account> for OutputAccount {
    fn from(account: &'a Account) -> Self {
        Self {
            id: account.id,
            available: account.available,
            held: account.held,
            total: account.total(),
            locked: account.locked,
        }
    }
}

/// Represents the state of a client's account.
#[derive(Debug)]
pub struct Account {
    pub id: ClientId,
    pub available: Decimal,
    pub held: Decimal,
    pub locked: bool,
}

impl Account {
    /// Creates a new, empty account for a client.
    pub fn new(id: ClientId) -> Self {
        Self {
            id,
            available: Decimal::ZERO,
            held: Decimal::ZERO,
            locked: false,
        }
    }

    /// Calculates the total funds in the account (available + held).
    pub fn total(&self) -> Decimal {
        self.available + self.held
    }
}

/// Represents a deposit or withdrawal transaction that is stored for potential disputes.
#[derive(Debug)]
pub struct StoredTransaction {
    pub client_id: ClientId,
    pub amount: Decimal,
    pub disputed: bool,
}

/// The main payment processing engine.
pub struct PaymentEngine {
    /// A map of client IDs to their accounts.
    pub accounts: HashMap<ClientId, Account>,
    /// A map of transaction IDs to their details, for dispute handling.
    pub transactions: HashMap<TransactionId, StoredTransaction>,
}

impl PaymentEngine {
    /// Creates a new `PaymentEngine`.
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            transactions: HashMap::new(),
        }
    }

    /// Processes all transactions from a given reader and updates account states.
    ///
    /// Transactions are expected to be in CSV format. Invalid transactions are ignored.
    pub fn process_transactions<R: io::Read>(&mut self, reader: R) -> Result<(), Box<dyn Error>> {
        let mut rdr = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(reader);

        for result in rdr.deserialize::<InputTransaction>() {
            if let Ok(tx) = result {
                match tx.transaction_type {
                    TransactionType::Deposit => self.handle_deposit(tx),
                    TransactionType::Withdrawal => self.handle_withdrawal(tx),
                    TransactionType::Dispute => self.handle_dispute(tx),
                    TransactionType::Resolve => self.handle_resolve(tx),
                    TransactionType::Chargeback => self.handle_chargeback(tx),
                }
            }
        }
        Ok(())
    }

    /// Handles a deposit transaction.
    /// Increases the client's available funds and records the transaction.
    /// Ignores deposits to locked accounts or with negative amounts.
    pub fn handle_deposit(&mut self, tx: InputTransaction) {
        let Some(amount) = tx.amount else { return };
        if amount.is_sign_negative() {
            return;
        }

        let account = self
            .accounts
            .entry(tx.client_id)
            .or_insert_with(|| Account::new(tx.client_id));
        if account.locked {
            return;
        }

        account.available += amount;
        self.transactions.insert(
            tx.tx_id,
            StoredTransaction {
                client_id: tx.client_id,
                amount,
                disputed: false,
            },
        );
    }

    /// Handles a withdrawal transaction.
    /// Decreases the client's available funds if sufficient funds are available.
    /// Ignores withdrawals from locked accounts or with negative amounts.
    pub fn handle_withdrawal(&mut self, tx: InputTransaction) {
        let Some(amount) = tx.amount else { return };
        if amount.is_sign_negative() {
            return;
        }

        let account = self
            .accounts
            .entry(tx.client_id)
            .or_insert_with(|| Account::new(tx.client_id));
        if account.locked || account.available < amount {
            return;
        }

        account.available -= amount;
        self.transactions.insert(
            tx.tx_id,
            StoredTransaction {
                client_id: tx.client_id,
                amount,
                disputed: false,
            },
        );
    }

    /// Handles a dispute transaction.
    /// Moves funds from available to held for the disputed transaction.
    /// The referenced transaction must exist and not be disputed already.
    pub fn handle_dispute(&mut self, tx: InputTransaction) {
        if let Some(disputed_tx) = self.transactions.get_mut(&tx.tx_id) {
            if disputed_tx.client_id != tx.client_id {
                return;
            }

            if let Some(account) = self.accounts.get_mut(&tx.client_id) {
                if account.locked {
                    return;
                }

                if !disputed_tx.disputed {
                    account.available -= disputed_tx.amount;
                    account.held += disputed_tx.amount;
                    disputed_tx.disputed = true;
                }
            }
        }
    }

    /// Handles a resolve transaction.
    /// Moves funds from held back to available, resolving the dispute.
    /// The referenced transaction must exist and be under dispute.
    pub fn handle_resolve(&mut self, tx: InputTransaction) {
        if let Some(disputed_tx) = self.transactions.get_mut(&tx.tx_id) {
            if disputed_tx.client_id != tx.client_id || !disputed_tx.disputed {
                return;
            }

            if let Some(account) = self.accounts.get_mut(&tx.client_id) {
                if account.locked {
                    return;
                }

                account.available += disputed_tx.amount;
                account.held -= disputed_tx.amount;
                disputed_tx.disputed = false;
            }
        }
    }

    /// Handles a chargeback transaction.
    /// Moves funds from held to withdrawn and freezes the client's account.
    /// The referenced transaction must exist and be under dispute.
    pub fn handle_chargeback(&mut self, tx: InputTransaction) {
        if let Some(disputed_tx) = self.transactions.get_mut(&tx.tx_id) {
            if disputed_tx.client_id != tx.client_id || !disputed_tx.disputed {
                return;
            }
            if let Some(account) = self.accounts.get_mut(&tx.client_id) {
                if account.locked {
                    return;
                }
                account.held -= disputed_tx.amount;
                account.locked = true;
            }
        }
    }
}

/// Writes the final state of all accounts to a given writer in CSV format.
pub fn export_accounts<W: io::Write>(
    accounts: &HashMap<ClientId, Account>,
    writer: W,
) -> Result<(), Box<dyn Error>> {
    let mut wtr = csv::Writer::from_writer(writer);
    let mut accounts: Vec<_> = accounts.values().collect();
    accounts.sort_by_key(|a| a.id);

    for account in accounts {
        wtr.serialize(OutputAccount::from(account))?;
    }
    wtr.flush()?;
    Ok(())
}