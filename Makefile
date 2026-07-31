.PHONY: help version-bump release build test clean clippy fmt-check lint

# Auto-generate version from today's date with auto-incrementing patch.
# Format: YYYYMMDD.0.X where X increments for multiple releases on one day.
define get_next_version
$(shell \
	TODAY=$$(date +%Y%m%d); \
	LATEST=$$(git tag -l "v$$TODAY.*" 2>/dev/null | sort -V | tail -1); \
	if [ -z "$$LATEST" ]; then \
		echo "$$TODAY.0.0"; \
	else \
		PATCH=$$(echo "$$LATEST" | sed 's/.*\.0\.\([0-9]*\)/\1/'); \
		echo "$$TODAY.0.$$((PATCH + 1))"; \
	fi \
)
endef

VERSION := $(get_next_version)

help:
	@echo "Schrodinger's Life Makefile"
	@echo ""
	@echo "Usage:"
	@echo "  make release                       - Auto-version and release (recommended)"
	@echo "  make release VERSION=20260730.0.0  - Release with a specific version"
	@echo "  make build                         - Build release binary"
	@echo "  make test                          - Run tests"
	@echo "  make lint                          - Run formatting, Clippy, and tests"
	@echo "  make clean                         - Clean build artifacts"
	@echo ""
	@echo "Next version will be: $(VERSION)"

version-bump:
	@echo "Creating release branch for version $(VERSION)..."
	@git checkout -b release/v$(VERSION)
	@sed -i 's/^version = .*/version = "$(VERSION)"/' Cargo.toml
	@cargo check --quiet
	@git add Cargo.toml Cargo.lock
	@git commit -m "chore: bump version to $(VERSION)"

release: version-bump
	@git checkout main
	@git merge --no-ff release/v$(VERSION) -m "Merge branch 'release/v$(VERSION)'"
	@git tag -a v$(VERSION) -m "Release v$(VERSION)"
	@git push origin main
	@git push origin v$(VERSION)
	@echo "Released v$(VERSION); GitHub Actions will publish the Linux binary."

build:
	cargo build --release

test:
	cargo test --all-targets

clippy:
	cargo clippy --all-targets -- -D warnings

fmt-check:
	cargo fmt -- --check

lint: fmt-check clippy test

clean:
	cargo clean
