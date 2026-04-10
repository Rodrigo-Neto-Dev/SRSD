.PHONY: build test test-specific demo reset clean shell dev help logs

# Colors for output
GREEN := \033[0;32m
YELLOW := \033[1;33m
CYAN := \033[0;36m
RESET := \033[0m

build: ## Build Docker images
	@echo "$(CYAN)Building Docker images...$(RESET)"
	docker-compose build

build-runtime: ## Build only runtime image (faster) she
	@echo "$(CYAN)Building runtime image...$(RESET)"
	docker build --target runtime -t gallery-log .

build-builder: ## Build only builder image with Rust
	@echo "$(CYAN)Building builder image...$(RESET)"
	docker build --target builder -t gallery-log-builder .

test: build ## Run all tests in Docker
	@echo "$(CYAN)Running all tests...$(RESET)"
	docker-compose run --rm gallery-test

test-specific: build ## Run a specific test (usage: make test-specific TEST=test_name)
	@echo "$(CYAN)Running test: $(TEST)$(RESET)"
	docker-compose run --rm gallery-test-specific cargo test $(TEST) -- --nocapture

demo: build-runtime ## Run interactive demo in Docker
	@echo "$(CYAN)Starting interactive demo...$(RESET)"
	docker-compose run --rm gallery-demo

reset: build-runtime ## Clear all logs
	@echo "$(CYAN)Clearing logs...$(RESET)"
	docker-compose run --rm gallery-reset

shell: build-runtime ## Open interactive shell in runtime container
	@echo "$(CYAN)Opening shell in runtime container...$(RESET)"
	docker-compose run --rm gallery-log /bin/bash

dev: build-builder ## Open development shell with Rust/cargo
	@echo "$(CYAN)Opening development shell...$(RESET)"
	docker-compose run --rm gallery-dev

clean: ## Remove logs and Docker artifacts
	@echo "$(CYAN)Cleaning up...$(RESET)"
	-rm -rf logs/*.log logs/*.txt
	-docker-compose down -v
	@echo "$(GREEN)✓ Cleanup complete$(RESET)"

clean-all: clean ## Also remove Docker images
	@echo "$(CYAN)Removing Docker images...$(RESET)"
	-docker rmi gallery-log gallery-log-builder 2>/dev/null || true
	@echo "$(GREEN)✓ All clean$(RESET)"

rebuild: clean-all build ## Clean everything and rebuild
	@echo "$(GREEN)✓ Rebuild complete$(RESET)"

# Local development commands (if you have Rust installed)
local-build: ## Build locally with cargo
	cargo build --release

local-test: ## Run tests locally
	cargo test --release

local-demo: ## Run demo locally
	./demo.sh

local-clean: ## Clean local build artifacts
	cargo clean
	rm -rf logs/*.log logs/*.txt