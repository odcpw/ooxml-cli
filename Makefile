.PHONY: build test test-race-coverage test-short test-roundtrip fixtures install clean help render-smoke web-smoke-agent web-smoke-nonpptx office-edit-smoke office-edit-smoke-fast office-edit-smoke-windows office-vba-smoke office-vba-smoke-fast check-fast check-local check-ci check-office-schema check-office-com check-office-vba-schema check-office-vba-com check-release-fast check-release-slow fmt-check vet verify verify-strict

# Default target
.DEFAULT_GOAL := help

# Variables
BINARY_NAME := ooxml
MAIN_PATH := ./cmd/ooxml
INSTALL_PATH := $(shell go env GOPATH)/bin
VERSION ?= $(shell git describe --tags --always --dirty 2>/dev/null || echo "dev")

# help: Show this help message
help:
	@echo "ooxml-cli build targets:"
	@echo ""
	@sed -n 's/^# //p' $(MAKEFILE_LIST) | sed -n '/^[a-z]/p' | awk -F': ' '{printf "  %-20s %s\n", $$1, $$2}'

# build: Build the ooxml binary
build:
	@echo "Building $(BINARY_NAME)..."
	@go build -ldflags "-X github.com/ooxml-cli/ooxml-cli/internal/cli.Version=$(VERSION)" -o $(BINARY_NAME) $(MAIN_PATH)
	@echo "✓ Built $(BINARY_NAME)"

# test: Run all tests with race detector and coverage
test: test-race-coverage

# test-race-coverage: Run all tests with race detector and coverage
test-race-coverage:
	@echo "Running tests..."
	@go test -v -race -coverprofile=coverage.txt ./...
	@echo "✓ Tests passed"

# test-short: Run tests in short mode (faster, no integration tests)
test-short:
	@echo "Running tests (short mode)..."
	@go test -short -race ./...
	@echo "✓ Tests passed"

# test-roundtrip: Run no-op roundtrip preservation tests
test-roundtrip:
	@echo "Running roundtrip preservation tests..."
	@go test -v -run TestRoundtrip ./pkg/opc/...
	@echo "✓ Roundtrip tests passed"

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
	@echo "✓ Test fixtures generated"

# render-smoke: Run the Linux render smoke test
render-smoke:
	@echo "Running render smoke test..."
	@go test -v ./pkg/render -run TestRenderSmokeMinimalTitle
	@echo "✓ Render smoke test passed"

# web-smoke-agent: Run the web agent smoke against the freshly built local binary (requires a running web server)
web-smoke-agent: build
	@cd web && OOXML_BIN="$(abspath $(BINARY_NAME))" npm run smoke:agent

# web-smoke-nonpptx: Run the web DOCX/XLSX smoke against the freshly built local binary (requires a running web server)
web-smoke-nonpptx: build
	@cd web && OOXML_BIN="$(abspath $(BINARY_NAME))" npm run smoke:nonpptx

# check-fast: Run the fastest normal Go test loop
check-fast:
	@go test -short ./...

# check-local: Run the normal local Go test gate
check-local:
	@go test ./...

# check-ci: Run the Linux CI-equivalent gate
check-ci:
	@$(MAKE) check-local
	@$(MAKE) render-smoke
	@$(MAKE) vet
	@$(MAKE) build

# office-edit-smoke: Windows only: build, mutate DOCX/XLSX/PPTX fixtures, validate with Open XML SDK, and open outputs in desktop Office
office-edit-smoke:
	@powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -MutationParallelism 4 -OfficeOracleTimeoutSeconds 120 -RequireOpenXmlSdk

# office-edit-smoke-fast: Windows only: run mutation validation plus Open XML SDK schema validation, skipping desktop Office COM
office-edit-smoke-fast:
	@powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -MutationParallelism 4 -RequireOpenXmlSdk -SkipOffice

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
check-release-fast:
	@$(MAKE) verify
	@powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -MutationParallelism 4 -RequireOpenXmlSdk -RunConformance -SkipOffice

# check-release-slow: Release readiness with Office COM: verify + edit smoke + Open XML SDK + conformance + desktop Office open
check-release-slow:
	@$(MAKE) verify
	@powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -MutationParallelism 4 -OfficeOracleTimeoutSeconds 120 -RequireOpenXmlSdk -RunConformance
	@$(MAKE) office-vba-smoke

# install: Install the binary to GOPATH/bin
install: build
	@echo "Installing $(BINARY_NAME) to $(INSTALL_PATH)..."
	@go install -ldflags "-X github.com/ooxml-cli/ooxml-cli/internal/cli.Version=$(VERSION)" $(MAIN_PATH)
	@echo "✓ Installed to $(INSTALL_PATH)/$(BINARY_NAME)"

# clean: Remove build artifacts
clean:
	@echo "Cleaning up..."
	@rm -f $(BINARY_NAME)
	@rm -f coverage.txt
	@go clean ./...
	@echo "✓ Clean complete"

# fmt-check: Check Go formatting without modifying files
fmt-check:
	@echo "Checking gofmt..."
	@files="$$(gofmt -l $$(git ls-files '*.go'))"; if [ -n "$$files" ]; then echo "$$files"; exit 1; fi

# vet: Run go vet
vet:
	@echo "Running go vet..."
	@go vet ./...

# verify: Run the normal local test gate without the repo-wide formatting gate
verify: vet check-local
	@echo "✓ Verification passed"

# verify-strict: Run gofmt check, vet, and the normal local test gate
verify-strict: fmt-check vet check-local
	@echo "✓ Strict verification passed"
