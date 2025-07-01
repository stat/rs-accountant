# rs-accountant

This project implements a toy payments engine in Rust that processes a stream of transactions from a CSV file, updates client account balances, and outputs the final state of all accounts to a CSV.

## Assumptions

In line with the prompt to make sensible assumptions for a financial system, the engine operates with the following rules:

- **Arbitrary Decimal Precision**: The engine supports arbitrary decimal precision for monetary values, with a minimum requirement to handle at least 2 decimal places for standard currency operations.
- **Locked Accounts**: Once an account is locked due to a chargeback, no further transactions (deposits, withdrawals, or disputes) are processed for that account.
- **Negative Amounts**: Any deposit or withdrawal transaction with a negative amount is considered invalid and ignored.
- **Dispute Ownership**: A dispute is only considered valid if the client ID on the dispute record matches the client ID of the original transaction being disputed.

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

### End-to-End Testing

To run an end-to-end test with randomly generated data:

```sh
make test-e2e
```

This will:
1. Generate random test data using the built-in `data-generator`
2. Run the engine on this data
3. Compare the output against expected results
4. Clean up temporary files

### Stress Testing

For performance testing with large datasets, the project includes stress testing capabilities:

#### Generate Large Test Data

To generate a ~1GB test file with approximately 35 million transactions:

```sh
make generate-stress-input
```

This creates a file called `large_input.csv` in the current directory.

#### Run Stress Test

To benchmark the engine against the large dataset:

```sh
make stress-test
```

This will run the engine on `large_input.csv` and measure execution time using the `time` utility. The output is discarded to focus purely on performance measurement.

**Note**: The stress test requires system resources (CPU and disk space for the ~1GB input file). On modern systems, expect the test to complete in under a minute.

## Architectural Evolution & Performance

The engine was optimized for a large (1GB, 35M transactions) dataset. Several architectures were tested to find the right balance of parallelism and overhead.

### 1. Single-Threaded (Winner)

- **Design**: The simplest approach. All work (I/O, deserialization, and processing) happens sequentially on the main thread.
- **Outcome**: Surprisingly, this was the most performant model. The overhead of creating threads and managing communication channels outweighed the benefits of parallel execution for this specific, CPU-bound workload.

```mermaid
graph TD;
    A[Main Thread <br/> 1. Read CSV <br/> 2. Process TXs <br/> 3. Write Output];
```

### 2. Multi-Worker Sharding

- **Design**: An initial attempt at parallelism involved a single I/O thread dispatching raw CSV records to a pool of worker threads based on `client_id`. Each worker processed all transactions for its assigned clients.
- **Outcome**: This model proved to be inefficient. The overhead of routing records and managing many threads was too high.

```mermaid
graph TD;
    subgraph " "
        direction LR
        A[I/O Thread]
    end
    subgraph " "
        B[Worker 1]
        C[Worker 2]
        D[Worker N]
    end
    A -- "Records (sharded by client_id)" --> B;
    A -- "..." --> C;
    A -- "..." --> D;
```

### 3. Two-Stage Pipeline

- **Design**: The architecture was simplified to a two-thread pipeline. A dedicated I/O thread reads and parses the file, sending batches of raw records to a single, dedicated processing thread.
- **Outcome**: This was a significant improvement. By creating a clean separation between I/O and processing, allowing both tasks to run concurrently.

```mermaid
graph TD;
    A[Thread 1: I/O] -- "Vec<StringRecord>" --> B[Thread 2: Processing];
```

### 4. Three-Stage Pipeline

- **Design**: To further refine the pipeline, the processing work was split into two stages, creating a three-thread pipeline for I/O, Deserialization, and Processing.
- **Outcome**: This design also proved to be highly efficient and was the fastest of the multi-threaded approaches.

```mermaid
graph TD;
    A[Thread 1: I/O] -- "Vec<StringRecord>" --> B[Thread 2: Deserialization];
    B -- "Vec<InputTransaction>" --> C[Thread 3: Processing];
```

### Benchmark Summary

The final, surprising result of the performance tuning was that a simple, single-threaded architecture was the most performant for this specific workload. The overhead of creating threads and managing communication between them ultimately outweighed the benefits of parallel execution.

Below are the benchmark results for each approach, as measured by the `time` utility on a large (1GB, 35M transactions) dataset.

| Architecture             | Real Time (Wall Clock) | User Time (Total CPU) |
| ------------------------ | ---------------------- | --------------------- |
| **Single-Threaded**      | **`~28.8s`**           | **`~27.6s`**          |
| Three-Stage Pipeline     | `~30.2s`               | `~1m 2s`              |
| Two-Stage Pipeline       | `~33.3s`               | `~1m 2s`              |
| Multi-Worker Sharding    | `~40.9s`               | `~1m 46s`             |

## Detailed Design & Implementation

### Core Logic

- **Streaming for Scalability**: The engine processes the input CSV as a stream using the `csv` crate's deserialization capabilities. This approach is highly memory-efficient, as it does not require loading the entire transaction file into memory. This ensures the application can scale to handle very large data sets without consuming excessive system resources.

- **Precise Financial Calculations**: Floating-point arithmetic can introduce precision errors, which are unacceptable in financial applications. To ensure correctness, this engine uses the `rust_decimal` crate for all monetary calculations. It provides a `Decimal` type that handles fixed-precision arithmetic accurately.

- **Efficient Data Structures**:
  - **Accounts**: A `HashMap<ClientId, Account>` is used to store client accounts. This provides average O(1) time complexity for lookups, insertions, and updates, which is ideal for quickly accessing account data.
  - **Transactions**: A `HashMap<TransactionId, StoredTransaction>` stores deposit and withdrawal transactions that may be disputed later. This allows for efficient lookups when a `dispute`, `resolve`, or `chargeback` transaction refers to an earlier one by its ID.

### Code Organization

The project is structured as a Rust library and a binary.
- The core logic (the `PaymentEngine`, data structures, and processing functions) resides in the library (`src/lib.rs` and `src/engine.rs`).
- The executable (`src/main.rs`) is a thin wrapper responsible for parsing command-line arguments and coordinating the engine's execution.

This separation of concerns makes the code more modular, easier to test, and reusable.

### Testing Strategy

The correctness of the transaction processing logic is validated through a suite of integration tests located in the `tests` directory. These tests cover all critical scenarios, including:
- Simple deposits and withdrawals.
- Withdrawals with insufficient funds.
- The full dispute/resolve/chargeback lifecycle.
- Transactions on locked accounts.

This test-driven approach helps guarantee that the engine behaves as expected under various conditions.

## TODO

### Testing & Quality Assurance
- [ ] **Determine test coverage** - Add tooling to measure and report code coverage metrics
- [ ] **Expand test suite** - Add more edge cases and comprehensive scenario testing
- [ ] **Property-based testing** - Implement property-based tests using `proptest` or `quickcheck`
- [ ] **Benchmark suite** - Add formal benchmarking with `criterion` for performance regression detection

### Features & Enhancements
- [ ] **Enable user-defined dataset size for stress testing** - Allow configurable transaction count and file size for stress tests
- [ ] **Transaction validation** - Add more robust input validation and error reporting
- [ ] **Configurable precision** - Allow users to specify decimal precision for monetary values
- [ ] **Multiple output formats** - Support JSON, XML, or other output formats beyond CSV
- [ ] **Logging and observability** - Add structured logging for debugging and monitoring

### Performance & Scalability
- [ ] **Memory usage profiling** - Analyze and optimize memory consumption patterns
- [ ] **Streaming output** - Implement streaming CSV output for very large result sets
- [ ] **Database backend** - Add optional database storage for persistent state
- [ ] **Compression support** - Support reading compressed input files (gzip, etc.)

### Developer Experience
- [ ] **CI/CD pipeline** - Set up automated testing and release workflows
- [ ] **Docker support** - Add Dockerfile, Compose, and container deployment options
- [ ] **Performance monitoring** - Add built-in performance metrics and reporting

### Contributing
- [ ] **Contributing guidelines** - Add CONTRIBUTING.md with development setup and code standards
- [ ] **Code linting** - Ensure all code passes `cargo clippy` without warnings before submitting PRs
- [ ] **Pre-commit hooks** - Set up automated formatting and linting checks
- [ ] **Issue templates** - Create GitHub issue templates for bugs and feature requests