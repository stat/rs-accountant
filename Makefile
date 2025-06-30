.PHONY: all build test test-e2e clean run generate-stress-input stress-test

# Default target
all: build

# Build the application in release mode
build:
	@echo "Building the application..."
	@cargo build --release

# Run the unit and integration test suite
test:
	@echo "Running tests..."
	@cargo test

# Run end-to-end test
test-e2e:
	@echo "Building binaries for E2E test..."
	@cargo build --release --bins
	@echo "Generating E2E test data..."
	@./target/release/data-generator
	@echo "Running E2E test..."
	@./target/release/something e2e_input.csv > e2e_actual_output.csv
	@echo "Comparing results..."
	@diff e2e_expected_output.csv e2e_actual_output.csv
	@echo "E2E test passed!"
	@echo "Cleaning up generated files..."
	# @rm e2e_input.csv e2e_expected_output.csv e2e_actual_output.csv

# Clean up build artifacts
clean:
	@echo "Cleaning up build artifacts..."
	@cargo clean

# Run the application with a sample input file
# Usage: make run file=path/to/your.csv
run:
	@cargo run --release -- $(file)

# --- Stress Testing ---

# Generate a large (~1GB) input file for stress testing
generate-stress-input:
	@echo "Building stress test data generator..."
	@cargo build --release --bin generate-stress-input
	@echo "Generating large_input.csv..."
	@./target/release/generate-stress-input

# Run the engine against the large input file to benchmark performance
stress-test: build
	@echo "Running stress test on large_input.csv..."
	@time ./target/release/something large_input.csv > /dev/null
	@echo "Stress test complete."