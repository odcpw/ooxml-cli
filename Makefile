.PHONY: build test test-unit test-smoke fixtures install clean help web-smoke-agent web-smoke-nonpptx office-edit-smoke office-edit-smoke-fast office-edit-smoke-windows office-vba-smoke office-vba-smoke-fast check-fast check-local check-ci check-office-schema check-office-com check-office-vba-schema check-office-vba-com check-release-fast check-release-slow fmt-check clippy verify verify-strict go-reference-build go-reference-test go-reference-test-short go-reference-contract go-reference-fmt-check go-reference-vet go-reference-render-smoke

# Default target
.DEFAULT_GOAL := help

# Variables
BINARY_NAME := ooxml
CARGO ?= cargo
GO ?= go
GO_REFERENCE_DIR ?= $(if $(wildcard go/go.mod),go,.)

ifeq ($(OS),Windows_NT)
EXE := .exe
else
EXE :=
endif

RUST_DEBUG_BIN := target/debug/$(BINARY_NAME)$(EXE)
GO_REFERENCE_BIN := target/go-reference/$(BINARY_NAME)$(EXE)

# help: Show this help message
help:
	@echo "ooxml-cli build targets:"
	@echo ""
	@sed -n 's/^# //p' $(MAKEFILE_LIST) | sed -n '/^[a-z]/p' | awk -F': ' '{printf "  %-28s %s\n", $$1, $$2}'

# build: Build the Rust ooxml binary
build:
	@echo "Building Rust $(BINARY_NAME)..."
	@$(CARGO) build --bin $(BINARY_NAME)
	@echo "Built $(RUST_DEBUG_BIN)"

# test: Run the normal Rust test gate without live Go oracle calls
test: test-unit test-smoke

# test-unit: Run Rust unit tests for the CLI binary
test-unit:
	@$(CARGO) test --bin $(BINARY_NAME)

# test-smoke: Run the frozen Rust integration smoke that does not invoke Go
test-smoke:
	@$(CARGO) test --test rust_contract_smoke frozen_cli_slice_matches_go_baseline -- --exact

# fixtures: Generate test fixtures using python-pptx
fixtures:
	@echo "Generating test fixtures..."
	@if [ ! -d .venv ]; then python -m venv .venv; fi && \
	. .venv/bin/activate && \
	pip install -q python-pptx==0.6.23 lxml && \
	cd testdata/generate/python && \
	cd ../../.. && \
	python testdata/generate/python/minimal_title.py && \
	python testdata/generate/python/title_content.py && \
	python testdata/generate/python/picture_placeholder.py && \
	python testdata/generate/python/table_slide.py && \
	python testdata/generate/python/chart_simple.py && \
	python testdata/generate/python/table_simple.py && \
	python testdata/generate/python/table_merged.py && \
	python testdata/generate/python/table_styled.py && \
	python testdata/generate/python/notes_slide.py && \
	python testdata/generate/python/multi_layout.py && \
	python testdata/generate/python/notes_handout.py && \
	python testdata/generate/python/corrupted_missing_media.py && \
	python testdata/generate/python/corrupted_dangling_layout.py && \
	python testdata/generate/python/edge_empty_paragraphs.py && \
	python testdata/generate/python/edge_mixed_bullets.py && \
	python testdata/generate/python/edge_nested_groups.py && \
	python testdata/generate/python/edge_large_deck.py && \
	python testdata/generate/python/producer_rich_text_powerpoint.py && \
	python testdata/generate/python/producer_rich_text_libreoffice.py && \
	python testdata/generate/python/slide_assembly_multi.py && \
	python testdata/generate/python/slide_assembly_import_source.py && \
	python testdata/generate/python/slide_assembly_notes_media.py && \
	python testdata/generate/python/template_branded.py && \
	python testdata/generate/python/theme_custom_colors.py && \
	python testdata/generate/python/theme_custom_fonts.py && \
	python testdata/generate/python/layout_qa_text_overflow.py && \
	python testdata/generate/python/layout_qa_shape_collision.py && \
	python testdata/generate/python/layout_qa_dense_slide.py && \
	python testdata/generate/python/create_geometry_fixtures.py && \
	python testdata/generate/python/create_rich_text_fixtures.py && \
	python testdata/generate/python/create_producer_fixtures.py && \
	python testdata/generate/python/create_xlsx_fixtures.py && \
	python testdata/generate/python/create_docx_fixtures.py
	@echo "Test fixtures generated"

# web-smoke-agent: Run the web agent smoke against the freshly built local Rust binary
web-smoke-agent: build
	@cd web && OOXML_BIN="$(abspath $(RUST_DEBUG_BIN))" npm run smoke:agent

# web-smoke-nonpptx: Run the web DOCX/XLSX smoke against the freshly built local Rust binary
web-smoke-nonpptx: build
	@cd web && OOXML_BIN="$(abspath $(RUST_DEBUG_BIN))" npm run smoke:nonpptx

# check-fast: Compile all Rust targets without running live Go oracle tests
check-fast:
	@$(CARGO) check --all-targets

# check-local: Run the normal local Rust gate
check-local: fmt-check check-fast clippy test build

# check-ci: Run the Rust CI-equivalent gate
check-ci: verify

# office-edit-smoke: Windows only: build Rust, mutate DOCX/XLSX/PPTX fixtures, validate with Open XML SDK, and open outputs in desktop Office
office-edit-smoke: build
	@powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -BinaryPath "$(abspath $(RUST_DEBUG_BIN))" -SkipBuild -MutationParallelism 4 -OfficeOracleTimeoutSeconds 120 -RequireOpenXmlSdk

# office-edit-smoke-fast: Windows only: run Rust mutation validation plus Open XML SDK schema validation, skipping desktop Office COM
office-edit-smoke-fast: build
	@powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -BinaryPath "$(abspath $(RUST_DEBUG_BIN))" -SkipBuild -MutationParallelism 4 -RequireOpenXmlSdk -SkipOffice

# office-edit-smoke-windows: Alias for office-edit-smoke
office-edit-smoke-windows: office-edit-smoke

# office-vba-smoke: Windows only: generate real XLSM/PPTM macro seeds from .bas/.cls sources, validate with Open XML SDK, and open outputs in desktop Office
office-vba-smoke:
	@powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -RequireOpenXmlSdk -EnableVbaObjectModelAccess -OfficeOracleTimeoutSeconds 120

# office-vba-smoke-fast: Windows only: generate Office-native VBA seeds, run strict/Open XML SDK validation, and skip the final desktop Office open oracle
office-vba-smoke-fast:
	@powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -RequireOpenXmlSdk -SkipOffice -EnableVbaObjectModelAccess

# check-office-schema: Windows only: run edit smoke with strict validation and Open XML SDK, skipping desktop Office COM
check-office-schema: office-edit-smoke-fast

# check-office-com: Windows only: run the full edit smoke through desktop Office COM
check-office-com: office-edit-smoke

# check-office-vba-schema: Windows only: run the VBA macro smoke with strict/Open XML SDK validation, skipping the final desktop Office open oracle
check-office-vba-schema: office-vba-smoke-fast

# check-office-vba-com: Windows only: run the full VBA macro smoke through desktop Office COM
check-office-vba-com: office-vba-smoke

# check-release-fast: Release readiness without Office COM: verify + edit smoke + Open XML SDK + conformance
check-release-fast: build
	@$(MAKE) verify
	@powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -BinaryPath "$(abspath $(RUST_DEBUG_BIN))" -SkipBuild -MutationParallelism 4 -RequireOpenXmlSdk -RunConformance -SkipOffice

# check-release-slow: Release readiness with Office COM: verify + edit smoke + Open XML SDK + conformance + desktop Office open
check-release-slow: build
	@$(MAKE) verify
	@powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -BinaryPath "$(abspath $(RUST_DEBUG_BIN))" -SkipBuild -MutationParallelism 4 -OfficeOracleTimeoutSeconds 120 -RequireOpenXmlSdk -RunConformance
	@$(MAKE) office-vba-smoke

# install: Install the Rust binary with cargo
install: build
	@$(CARGO) install --path . --bin $(BINARY_NAME)

# clean: Remove Rust build artifacts
clean:
	@$(CARGO) clean

# fmt-check: Check Rust formatting without modifying files
fmt-check:
	@$(CARGO) fmt --all -- --check

# clippy: Run Rust clippy with warnings denied
clippy:
	@$(CARGO) clippy --all-targets -- -D warnings

# verify: Run the normal Rust verification gate
verify: check-local
	@echo "Verification passed"

# verify-strict: Run the normal Rust gate plus doc tests
verify-strict: verify
	@$(CARGO) test --doc
	@echo "Strict verification passed"

# go-reference-build: Optional legacy Go reference build; not part of normal CI
go-reference-build:
	@mkdir -p target/go-reference
	@$(GO) -C "$(GO_REFERENCE_DIR)" build -buildvcs=false -o "$(abspath $(GO_REFERENCE_BIN))" ./cmd/ooxml

# go-reference-test: Optional legacy Go reference test suite; not part of normal CI
go-reference-test:
	@$(GO) -C "$(GO_REFERENCE_DIR)" test ./...

# go-reference-test-short: Optional fast legacy Go reference tests; not part of normal CI
go-reference-test-short:
	@$(GO) -C "$(GO_REFERENCE_DIR)" test -short ./...

# go-reference-contract: Optional live Go-vs-Rust parity contract; not part of normal CI
go-reference-contract:
	@$(CARGO) test --test rust_contract_smoke

# go-reference-fmt-check: Optional legacy Go formatting check; not part of normal CI
go-reference-fmt-check:
	@cd "$(GO_REFERENCE_DIR)" && files="$$(gofmt -l $$(git ls-files '*.go'))"; if [ -n "$$files" ]; then echo "$$files"; exit 1; fi

# go-reference-vet: Optional legacy Go vet check; not part of normal CI
go-reference-vet:
	@$(GO) -C "$(GO_REFERENCE_DIR)" vet ./...

# go-reference-render-smoke: Optional legacy Go render smoke; not part of normal CI
go-reference-render-smoke:
	@$(GO) -C "$(GO_REFERENCE_DIR)" test -v ./pkg/render -run TestRenderSmokeMinimalTitle
