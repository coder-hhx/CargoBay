# CrateBay Makefile
# Rust workspace + Tauri desktop app + React frontend

CARGO       := cargo
NPM         := npm
GUI_DIR     := crates/cratebay-gui
TAURI_DIR   := $(GUI_DIR)/src-tauri
PROTO_DIR   := proto

# ── Build ────────────────────────────────────────────────────
.PHONY: build build-release build-gui build-cli build-daemon build-mcp

build:                          ## Build all Rust crates (debug)
	$(CARGO) build --workspace

build-release:                  ## Build all Rust crates (release)
	$(CARGO) build --workspace --release

build-cli:                      ## Build cratebay CLI only
	$(CARGO) build -p cratebay-cli

build-daemon:                   ## Build cratebay-daemon only
	$(CARGO) build -p cratebay-daemon

build-mcp:                      ## Build cratebay-mcp only
	$(CARGO) build -p cratebay-mcp

build-gui:                      ## Build Tauri desktop app (debug)
	cd $(GUI_DIR) && $(NPM) run tauri build -- --debug

# ── Frontend ─────────────────────────────────────────────────
.PHONY: fe-install fe-build fe-dev fe-lint fe-test fe-test-e2e

fe-install:                     ## Install frontend dependencies
	cd $(GUI_DIR) && $(NPM) install

fe-build:                       ## Build frontend (Vite)
	cd $(GUI_DIR) && $(NPM) run build

fe-dev:                         ## Start frontend dev server
	@echo "Run manually: cd $(GUI_DIR) && npm run dev"

fe-lint:                        ## Lint frontend code
	cd $(GUI_DIR) && $(NPM) run lint

fe-test:                        ## Run frontend unit tests
	cd $(GUI_DIR) && npx vitest --run

fe-test-e2e:                    ## Run frontend E2E tests (Playwright)
	cd $(GUI_DIR) && npx playwright test

# ── Rust Quality ─────────────────────────────────────────────
.PHONY: fmt fmt-check clippy test test-unit

fmt:                            ## Format all Rust code
	$(CARGO) fmt --all

fmt-check:                      ## Check Rust formatting
	$(CARGO) fmt --all -- --check

clippy:                         ## Run clippy on workspace
	$(CARGO) clippy --workspace --all-targets -- -D warnings

test:                           ## Run all Rust tests
	$(CARGO) test --workspace

test-unit:                      ## Run Rust tests (lib only, no integration)
	$(CARGO) test --workspace --lib

# ── Proto ────────────────────────────────────────────────────
.PHONY: proto

proto:                          ## Regenerate protobuf code (via build.rs)
	$(CARGO) build -p cratebay-daemon

# ── Release ──────────────────────────────────────────────────
.PHONY: release-macos release-windows release-gui

release-macos:                  ## macOS release build
	bash scripts/build-release-macos.sh

release-windows:                ## Windows release build
	bash scripts/build-release-windows.sh

release-gui:                    ## Build Tauri desktop app (release)
	cd $(GUI_DIR) && $(NPM) run tauri build

# ── Dev Workflow ─────────────────────────────────────────────
.PHONY: setup dev ci-local clean

setup:                          ## Setup development environment
	bash scripts/setup-dev.sh
	cd $(GUI_DIR) && $(NPM) install

dev:                            ## Start Tauri dev mode (frontend + backend)
	cd $(GUI_DIR) && $(NPM) run tauri dev

ci-local:                       ## Run local CI checks
	bash scripts/ci-local.sh

check: fmt-check clippy test    ## Run all checks (fmt + clippy + test)

check-all: check fe-lint fe-test ## Run all checks including frontend

clean:                          ## Clean all build artifacts
	$(CARGO) clean
	rm -rf $(GUI_DIR)/dist $(GUI_DIR)/node_modules/.vite

# ── Help ─────────────────────────────────────────────────────
.PHONY: help
.DEFAULT_GOAL := help

help:                           ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}'
