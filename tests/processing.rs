use something::engine::*;
use rust_decimal_macros::dec;

#[test]
fn test_deposit() {
    let mut engine = PaymentEngine::new();
    let tx = InputTransaction {
        transaction_type: TransactionType::Deposit,
        client_id: 1,
        tx_id: 1,
        amount: Some(dec!(100.0)),
    };
    engine.handle_deposit(tx);
    
    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, dec!(100.0));
    assert_eq!(account.held, dec!(0.0));
    assert_eq!(account.total(), dec!(100.0));
    assert!(!account.locked);

    let stored_tx = engine.transactions.get(&1).unwrap();
    assert_eq!(stored_tx.amount, dec!(100.0));
}

#[test]
fn test_withdrawal_success() {
    let mut engine = PaymentEngine::new();
    let deposit_tx = InputTransaction {
        transaction_type: TransactionType::Deposit,
        client_id: 1,
        tx_id: 1,
        amount: Some(dec!(100.0)),
    };
    engine.handle_deposit(deposit_tx);

    let withdrawal_tx = InputTransaction {
        transaction_type: TransactionType::Withdrawal,
        client_id: 1,
        tx_id: 2,
        amount: Some(dec!(50.0)),
    };
    engine.handle_withdrawal(withdrawal_tx);

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, dec!(50.0));
    assert_eq!(account.total(), dec!(50.0));
}

#[test]
fn test_withdrawal_insufficient_funds() {
    let mut engine = PaymentEngine::new();
    let deposit_tx = InputTransaction {
        transaction_type: TransactionType::Deposit,
        client_id: 1,
        tx_id: 1,
        amount: Some(dec!(100.0)),
    };
    engine.handle_deposit(deposit_tx);

    let withdrawal_tx = InputTransaction {
        transaction_type: TransactionType::Withdrawal,
        client_id: 1,
        tx_id: 2,
        amount: Some(dec!(150.0)),
    };
    engine.handle_withdrawal(withdrawal_tx);

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, dec!(100.0)); // Unchanged
}

#[test]
fn test_dispute_resolve_cycle() {
    let mut engine = PaymentEngine::new();
    let deposit_tx = InputTransaction {
        transaction_type: TransactionType::Deposit,
        client_id: 1,
        tx_id: 1,
        amount: Some(dec!(100.0)),
    };
    engine.handle_deposit(deposit_tx);

    let dispute_tx = InputTransaction {
        transaction_type: TransactionType::Dispute,
        client_id: 1,
        tx_id: 1,
        amount: None,
    };
    engine.handle_dispute(dispute_tx);

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, dec!(0.0));
    assert_eq!(account.held, dec!(100.0));
    assert_eq!(account.total(), dec!(100.0));
    assert!(engine.transactions.get(&1).unwrap().disputed);

    let resolve_tx = InputTransaction {
        transaction_type: TransactionType::Resolve,
        client_id: 1,
        tx_id: 1,
        amount: None,
    };
    engine.handle_resolve(resolve_tx);
    
    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, dec!(100.0));
    assert_eq!(account.held, dec!(0.0));
    assert_eq!(account.total(), dec!(100.0));
    assert!(!engine.transactions.get(&1).unwrap().disputed);
}

#[test]
fn test_dispute_chargeback_cycle() {
    let mut engine = PaymentEngine::new();
    let deposit_tx = InputTransaction {
        transaction_type: TransactionType::Deposit,
        client_id: 1,
        tx_id: 1,
        amount: Some(dec!(100.0)),
    };
    engine.handle_deposit(deposit_tx);

    let dispute_tx = InputTransaction {
        transaction_type: TransactionType::Dispute,
        client_id: 1,
        tx_id: 1,
        amount: None,
    };
    engine.handle_dispute(dispute_tx);

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.held, dec!(100.0));

    let chargeback_tx = InputTransaction {
        transaction_type: TransactionType::Chargeback,
        client_id: 1,
        tx_id: 1,
        amount: None,
    };
    engine.handle_chargeback(chargeback_tx);

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, dec!(0.0));
    assert_eq!(account.held, dec!(0.0));
    assert_eq!(account.total(), dec!(0.0));
    assert!(account.locked);
    assert!(!engine.transactions.get(&1).unwrap().disputed);
}

#[test]
fn test_locked_account_withdrawal() {
    let mut engine = PaymentEngine::new();
    engine.accounts.insert(1, Account { id: 1, available: dec!(100.0), held: dec!(0.0), locked: true });

    let withdrawal_tx = InputTransaction {
        transaction_type: TransactionType::Withdrawal,
        client_id: 1,
        tx_id: 1,
        amount: Some(dec!(50.0)),
    };
    engine.handle_withdrawal(withdrawal_tx);

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, dec!(100.0)); // Unchanged
}

#[test]
fn test_locked_account_deposit() {
    let mut engine = PaymentEngine::new();
    engine.accounts.insert(1, Account { id: 1, available: dec!(100.0), held: dec!(0.0), locked: true });

    let deposit_tx = InputTransaction {
        transaction_type: TransactionType::Deposit,
        client_id: 1,
        tx_id: 1,
        amount: Some(dec!(50.0)),
    };
    engine.handle_deposit(deposit_tx);

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, dec!(100.0)); // Unchanged, since we now block deposits to locked accounts
} 