# Toy Payments Engine

This project implements a toy payments engine in Rust that processes a stream of transactions from a CSV file, updates client account balances, and outputs the final state of all accounts to a CSV.

## Features

- Processes five types of transactions: `deposit`, `withdrawal`, `dispute`, `resolve`, and `chargeback`.
- Handles client accounts, including available funds, held funds, and locked status.
- Reads from a CSV file and writes the resulting account states to standard output.
- Built to be efficient and robust, capable of handling large datasets.

## How to Run

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) toolchain
- `make`

### Build

A `Makefile` is provided for convenience. To build the application in release mode, run:
```sh
make build
```
This is a shortcut for `cargo build --release`.

### Run

To run the application, use the `make run` command and pass the input file path as a variable. The resulting accounts CSV will be written to standard output.

```sh
make run file=transactions.csv > accounts.csv
```

This is a shortcut for `cargo run --release -- transactions.csv`.

### Test

To run the suite of integration tests:

```sh
make test
```
This is a shortcut for `cargo test`.

## Design Choices

### 1. Streaming for Scalability

The engine processes the input CSV as a stream using the `csv` crate's deserialization capabilities. This approach is highly memory-efficient, as it does not require loading the entire transaction file into memory. This ensures the application can scale to handle very large data sets without consuming excessive system resources.

### 2. Precise Financial Calculations

Floating-point arithmetic can introduce precision errors, which are unacceptable in financial applications. To ensure correctness, this engine uses the `rust_decimal` crate for all monetary calculations. It provides a `Decimal` type that handles fixed-precision arithmetic accurately.

### 3. Data Structures for Efficiency

- **Accounts**: A `HashMap<ClientId, Account>` is used to store client accounts. This provides average O(1) time complexity for lookups, insertions, and updates, which is ideal for quickly accessing account data.
- **Transactions**: A `HashMap<TransactionId, StoredTransaction>` stores deposit and withdrawal transactions that may be disputed later. This allows for efficient lookups when a `dispute`, `resolve`, or `chargeback` transaction refers to an earlier one by its ID.

### 4. Code Organization

The project is structured as a Rust library and a binary.
- The core logic (the `PaymentEngine`, data structures, and processing functions) resides in the library (`src/lib.rs` and `src/engine.rs`).
- The executable (`src/main.rs`) is a thin wrapper responsible only for parsing command-line arguments and coordinating the I/O.

This separation of concerns makes the code more modular, easier to test, and reusable.

### 5. Testing Strategy

The correctness of the transaction processing logic is validated through a suite of integration tests located in the `tests` directory. These tests cover all critical scenarios, including:
- Simple deposits and withdrawals.
- Withdrawals with insufficient funds.
- The full dispute/resolve/chargeback lifecycle.
- Transactions on locked accounts.

This test-driven approach helps guarantee that the engine behaves as expected under various conditions.

## Assumptions Made

In line with the prompt to make sensible assumptions for a financial system, the engine operates with the following rules:

- **Locked Accounts**: Once an account is locked due to a chargeback, no further transactions (deposits, withdrawals, or disputes) are processed for that account. This is a security measure to freeze activity on potentially fraudulent accounts.
- **Negative Amounts**: Any deposit or withdrawal transaction with a negative amount is considered invalid and ignored.
- **Dispute Ownership**: A dispute is only considered valid if the client ID on the dispute record matches the client ID of the original transaction being disputed. This prevents one client from being able to dispute another client's transactions. 