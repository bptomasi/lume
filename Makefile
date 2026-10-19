CARGO ?= cargo
PREFIX ?= $(HOME)/.local
ifeq ($(origin LUME_DATA_DIR), undefined)
DATA_DIR ?= $(HOME)/.local/lume
else
DATA_DIR ?= $(LUME_DATA_DIR)
endif
TARGET_DIR ?= $(if $(CARGO_TARGET_DIR),$(CARGO_TARGET_DIR),target)
BINARY ?= $(TARGET_DIR)/release/lume
MIGRATE_FROM ?=

.PHONY: check check-release check-http check-alacritty fmt lint test bench build demo
.PHONY: install test-tools bench-tools bench-notes bench-recommendations
.PHONY: bench-images

# Serialize composition even when a caller uses make -j.
.NOTPARALLEL:

check:
	$(MAKE) fmt
	$(MAKE) lint
	$(MAKE) test
	$(MAKE) test-tools
	python3 -m py_compile tools/render_preview.py tools/qualify_alacritty.py \
		tools/install.py tools/bench_install.py tools/bench_notes.py \
		tools/bench_support.py tools/bench_recommendations.py tools/bench_images.py

fmt:
	$(CARGO) fmt --all -- --check

lint:
	$(CARGO) clippy --locked --all-targets -- -D warnings

# Native harnesses run all tests; the custom reader bench verifies every
# workload with its --test scenario. Full measurements belong to make bench.
test:
	$(CARGO) test --locked --all-targets -- --test

test-tools:
	python3 -m unittest discover -s tools/tests -p 'test_*.py'

# Real HTTP is an explicit release requirement, never silently skipped.
check-release:
	$(MAKE) check
	$(MAKE) check-http
	$(CARGO) build --locked --release

check-http:
	$(CARGO) test --locked --test http --test image_http -- --ignored

# Requires a real X11 display, or xvfb-run, and the named GUI test tools.
# This is separate from local checks because it opens a desktop window.
check-alacritty:
	python3 tools/qualify_alacritty.py $(ALACRITTY_ARGS)

bench:
	$(CARGO) bench --locked --bench reader
	$(CARGO) bench --locked --bench reader -- --navigation-only
	$(CARGO) bench --locked --bench reader -- --stress
	$(CARGO) bench --locked --bench reader -- --stress --navigation-only
	$(MAKE) bench-tools

# Repeated resource measurements; ordinary checks exercise the Rust workloads.
bench-notes:
	python3 tools/bench_notes.py

bench-recommendations:
	python3 tools/bench_recommendations.py

bench-images:
	python3 tools/bench_images.py

bench-tools:
	python3 tools/bench_install.py

build:
	$(CARGO) build --locked --release

install: build
	python3 tools/install.py --binary "$(BINARY)" --prefix "$(PREFIX)" \
		--data-dir "$(DATA_DIR)" \
		$(if $(MIGRATE_FROM),--migrate-from "$(MIGRATE_FROM)")

demo:
	$(CARGO) run --locked -- --demo
