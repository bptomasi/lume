.DEFAULT_GOAL := help

CARGO ?= cargo
PYTHON ?= python3
PREFIX ?= $(HOME)/.local
ifeq ($(origin LUME_DATA_DIR), undefined)
DATA_DIR ?= $(HOME)/.local/lume
else
DATA_DIR ?= $(LUME_DATA_DIR)
endif
TARGET_DIR ?= $(if $(CARGO_TARGET_DIR),$(CARGO_TARGET_DIR),target)
BINARY ?= $(TARGET_DIR)/release/lume
CARGO_ABOUT ?= cargo-about
CARGO_AUDIT ?= cargo-audit
REPORT_DIR ?= reports
MIGRATE_FROM ?=

.PHONY: help paths check check-release check-http check-alacritty fmt format
.PHONY: lint test bench build run demo clean
.PHONY: install upgrade test-tools bench-tools bench-notes bench-recommendations
.PHONY: bench-reading bench-images bench-interaction bench-code check-image-display check-foot
.PHONY: check-latency bench-configuration check-licenses check-security licenses package

# Serialize composition even when a caller uses make -j.
.NOTPARALLEL:

help:
	@printf '%s\n' \
		'Lume — build, install and development commands' \
		'' \
		'Usage: make [command] [VARIABLE=value]' \
		'' \
		'Application:' \
		'  build                 Build the locked release binary' \
		'  install               Install; preserve data and detect legacy libraries' \
		'  upgrade               Update an installed binary from this checkout' \
		'  run                   Run the local application' \
		'  demo                  Explore an isolated demonstration library' \
		'  paths                 Show build and installation paths' \
		'  clean                 Remove Cargo build output; keep reading data' \
		'' \
		'Checks:' \
		'  check                 Run all local checks; no services required' \
		'  check-release         Local, current security, HTTP and release build' \
		'  check-security        Current RustSec gate; requires cargo-audit 0.22.2' \
		'  licenses              Refresh complete notices with cargo-about 0.9.2' \
		'  package               Verify and package standalone Linux executable' \
		'  fmt / format          Check / apply Rust formatting' \
		'  lint                  Run Clippy for every target' \
		'  test / test-tools     Rust behavior and benchmark checks / Python tests' \
		'  check-http            Qualify real loopback HTTP behavior' \
		'  check-foot            Qualify images and input in a real Wayland Foot' \
		'  check-image-display   Alias for check-foot' \
		'  check-alacritty       Qualify Alacritty input and cleanup on X11' \
		'  check-latency         Measure real Foot input/frame acknowledgement' \
		'' \
		'Correctness-checked measurements:' \
		'  bench                 Full representative and stress workloads' \
		'  bench-configuration   TOML publication and revision checks' \
		'  bench-code            Long code rows and selected frames' \
		'  bench-interaction     Input, navigation and resize workloads' \
		'  bench-reading         Plans, offline/cache, selectors and linked pages' \
		'  bench-images          Image decode, encoding, zoom and scroll' \
		'  bench-notes           Annotations and large notebooks' \
		'  bench-recommendations Local relevance and cached navigation' \
		'  bench-tools           Installer migration and upgrade workloads' \
		'' \
		'Options: PREFIX, DATA_DIR, CARGO_TARGET_DIR, BINARY, CARGO, PYTHON' \
		'Legacy import: make install MIGRATE_FROM=/absolute/library.sqlite3' \
		'Upgrade rebuilds local source; it never pulls Git or changes config.'

paths:
	@printf '%s\n' \
		'Build directory: $(TARGET_DIR)' \
		'Release binary:  $(BINARY)' \
		'Installed binary: $(PREFIX)/bin/lume' \
		'Library: $(DATA_DIR)/library.sqlite3' \
		'Use lume config path for the runtime configuration location.'

check:
	$(MAKE) check-licenses
	$(MAKE) fmt
	$(MAKE) lint
	$(MAKE) test
	$(MAKE) test-tools
	$(PYTHON) -m py_compile tools/render_preview.py tools/qualify_alacritty.py \
		tools/install.py tools/bench_install.py tools/bench_notes.py \
		tools/bench_support.py tools/bench_recommendations.py tools/bench_images.py \
		tools/qualify_foot.py tools/foot_terminal.py tools/bench_interaction.py \
		tools/terminal_latency.py tools/latency_fixture.py tools/qualify_latency.py \
		tools/bench_code.py tools/bench_configuration.py tools/bench_reading.py \
		tools/advisories.py tools/dependency_inventory.py tools/visual_dependencies.py \
		tools/package_release.py

fmt:
	$(CARGO) fmt --all -- --check

format:
	$(CARGO) fmt --all

lint:
	$(CARGO) clippy --locked --all-targets -- -D warnings

# Native harnesses run all tests; the custom reader bench verifies every
# workload with its --test scenario. Full measurements belong to make bench.
test:
	$(CARGO) test --locked --all-targets -- --test

test-tools:
	$(PYTHON) -m unittest discover -s tools/tests -p 'test_*.py'

# Real HTTP is an explicit release requirement, never silently skipped.
check-release:
	$(MAKE) check
	$(MAKE) check-security
	$(MAKE) check-http
	$(MAKE) build

check-http:
	$(CARGO) test --locked --test http --test image_http -- --ignored

# Requires a real X11 display, or xvfb-run, and the named GUI test tools.
# This is separate from local checks because it opens a desktop window.
check-alacritty:
	$(PYTHON) tools/qualify_alacritty.py --binary "$(BINARY)" $(ALACRITTY_ARGS)

# Requires Foot and native Wayland; opens only an owned qualification window.
check-image-display: check-foot

check-latency:
	$(PYTHON) tools/qualify_latency.py --binary "$(BINARY)" $(LATENCY_ARGS)

check-foot:
	$(PYTHON) tools/qualify_foot.py --binary "$(BINARY)" $(FOOT_ARGS)

bench:
	$(CARGO) bench --locked --bench reader
	$(CARGO) bench --locked --bench reader -- --navigation-only
	$(CARGO) bench --locked --bench reader -- --stress
	$(CARGO) bench --locked --bench reader -- --stress --navigation-only
	$(MAKE) bench-tools

# Repeated resource measurements; ordinary checks exercise the Rust workloads.
bench-configuration:
	$(PYTHON) tools/bench_configuration.py

bench-notes:
	$(PYTHON) tools/bench_notes.py

bench-recommendations:
	$(PYTHON) tools/bench_recommendations.py

bench-reading:
	$(PYTHON) tools/bench_reading.py

bench-images:
	$(PYTHON) tools/bench_images.py

bench-code:
	$(PYTHON) tools/bench_code.py --output .local/benchmarks/code-viewport.json

bench-interaction:
	$(PYTHON) tools/bench_interaction.py --output .local/benchmarks/interaction.json

bench-tools:
	$(PYTHON) tools/bench_install.py

build:
	$(CARGO) build --locked --release

install: build
	$(PYTHON) tools/install.py --binary "$(BINARY)" --prefix "$(PREFIX)" \
		--data-dir "$(DATA_DIR)" \
		$(if $(MIGRATE_FROM),--migrate-from "$(MIGRATE_FROM)")

upgrade: build
	$(PYTHON) tools/install.py --upgrade \
		--binary "$(BINARY)" --prefix "$(PREFIX)" --data-dir "$(DATA_DIR)" \
		$(if $(MIGRATE_FROM),--migrate-from "$(MIGRATE_FROM)")

run:
	$(CARGO) run --locked --

demo:
	$(CARGO) run --locked -- --demo

clean:
	$(CARGO) clean --target-dir "$(TARGET_DIR)"

# Verification uses committed evidence and never downloads tools.
check-licenses:
	$(PYTHON) tools/dependency_inventory.py

licenses:
	$(PYTHON) tools/dependency_inventory.py --generate --cargo-about "$(CARGO_ABOUT)"

check-security:
	mkdir -p "$(REPORT_DIR)"
	$(PYTHON) tools/advisories.py --cargo-audit "$(CARGO_AUDIT)" \
		--output "$(REPORT_DIR)/advisories.json"

package: build check-licenses
	$(PYTHON) tools/package_release.py --binary "$(BINARY)" $(PACKAGE_ARGS)
