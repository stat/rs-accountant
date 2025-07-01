use rs_accountant::engine::*;
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
    assert_eq!(engine.transactions.get(&1).unwrap().dispute_status, DisputeStatus::Disputed);

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
    assert_eq!(engine.transactions.get(&1).unwrap().dispute_status, DisputeStatus::Resolved);
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
    assert_eq!(engine.transactions.get(&1).unwrap().dispute_status, DisputeStatus::ChargedBack);
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
    assert_eq!(account.available, dec!(100.0)); // Unchanged, since deposits are blocked to locked accounts
}

#[test]
fn test_re_dispute_resolved_transaction() {
    let mut engine = PaymentEngine::new();
    
    // Create a deposit
    let deposit_tx = InputTransaction {
        transaction_type: TransactionType::Deposit,
        client_id: 1,
        tx_id: 1,
        amount: Some(dec!(100.0)),
    };
    engine.handle_deposit(deposit_tx);

    // Dispute it
    let dispute_tx = InputTransaction {
        transaction_type: TransactionType::Dispute,
        client_id: 1,
        tx_id: 1,
        amount: None,
    };
    engine.handle_dispute(dispute_tx);
    
    // Resolve it
    let resolve_tx = InputTransaction {
        transaction_type: TransactionType::Resolve,
        client_id: 1,
        tx_id: 1,
        amount: None,
    };
    engine.handle_resolve(resolve_tx);
    
    // Verify it's resolved
    assert_eq!(engine.transactions.get(&1).unwrap().dispute_status, DisputeStatus::Resolved);
    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, dec!(100.0));
    assert_eq!(account.held, dec!(0.0));
    
    // Now dispute it again - this should be allowed
    let dispute_tx2 = InputTransaction {
        transaction_type: TransactionType::Dispute,
        client_id: 1,
        tx_id: 1,
        amount: None,
    };
    engine.handle_dispute(dispute_tx2);
    
    // Verify the re-dispute worked
    assert_eq!(engine.transactions.get(&1).unwrap().dispute_status, DisputeStatus::Disputed);
    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, dec!(0.0));
    assert_eq!(account.held, dec!(100.0));
}

#[test]
fn test_cannot_dispute_charged_back_transaction() {
    let mut engine = PaymentEngine::new();
    
    // Create a deposit
    let deposit_tx = InputTransaction {
        transaction_type: TransactionType::Deposit,
        client_id: 1,
        tx_id: 1,
        amount: Some(dec!(100.0)),
    };
    engine.handle_deposit(deposit_tx);

    // Dispute it
    let dispute_tx = InputTransaction {
        transaction_type: TransactionType::Dispute,
        client_id: 1,
        tx_id: 1,
        amount: None,
    };
    engine.handle_dispute(dispute_tx);
    
    // Chargeback
    let chargeback_tx = InputTransaction {
        transaction_type: TransactionType::Chargeback,
        client_id: 1,
        tx_id: 1,
        amount: None,
    };
    engine.handle_chargeback(chargeback_tx);
    
    // Verify it's charged back and account is locked
    assert_eq!(engine.transactions.get(&1).unwrap().dispute_status, DisputeStatus::ChargedBack);
    let account = engine.accounts.get(&1).unwrap();
    assert!(account.locked);
    assert_eq!(account.available, dec!(0.0));
    assert_eq!(account.held, dec!(0.0));
    
    // Try to dispute it again - this should be blocked
    let dispute_tx2 = InputTransaction {
        transaction_type: TransactionType::Dispute,
        client_id: 1,
        tx_id: 1,
        amount: None,
    };
    engine.handle_dispute(dispute_tx2);
    
    // Verify the dispute was blocked - status should remain ChargedBack
    assert_eq!(engine.transactions.get(&1).unwrap().dispute_status, DisputeStatus::ChargedBack);
    let account = engine.accounts.get(&1).unwrap();
    assert!(account.locked);
    assert_eq!(account.available, dec!(0.0));
    assert_eq!(account.held, dec!(0.0));
}

#[test]
fn test_dispute_with_insufficient_funds_creates_negative_balance() {
    let mut engine = PaymentEngine::new();
    
    // Create a deposit of $100
    let deposit_tx = InputTransaction {
        transaction_type: TransactionType::Deposit,
        client_id: 1,
        tx_id: 1,
        amount: Some(dec!(100.0)),
    };
    engine.handle_deposit(deposit_tx);
    
    // Withdraw $80, leaving $20 available
    let withdrawal_tx = InputTransaction {
        transaction_type: TransactionType::Withdrawal,
        client_id: 1,
        tx_id: 2,
        amount: Some(dec!(80.0)),
    };
    engine.handle_withdrawal(withdrawal_tx);
    
    // Verify account state before dispute
    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, dec!(20.0));
    assert_eq!(account.held, dec!(0.0));
    assert_eq!(account.total(), dec!(20.0));
    
    // Now dispute the original $100 deposit - this should be allowed even though
    // we only have $20 available, creating a negative balance
    let dispute_tx = InputTransaction {
        transaction_type: TransactionType::Dispute,
        client_id: 1,
        tx_id: 1,
        amount: None,
    };
    engine.handle_dispute(dispute_tx);
    
    // Verify the dispute created a negative available balance
    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, dec!(-80.0)); // 20 - 100 = -80
    assert_eq!(account.held, dec!(100.0));
    assert_eq!(account.total(), dec!(20.0)); // total should still be correct
    assert_eq!(engine.transactions.get(&1).unwrap().dispute_status, DisputeStatus::Disputed);
} 