CARGO ?= cargo

.PHONY: check check-release check-http check-alacritty fmt lint test bench build demo

# Serialize composition even when a caller uses make -j.
.NOTPARALLEL:

check:
	$(MAKE) fmt
	$(MAKE) lint
	$(MAKE) test
	python3 -m py_compile tools/render_preview.py tools/qualify_alacritty.py
	bash -n lume

fmt:
	$(CARGO) fmt --all -- --check

lint:
	$(CARGO) clippy --locked --all-targets -- -D warnings

test:
	$(CARGO) test --locked --all-targets

# Real HTTP is an explicit release requirement, never silently skipped.
check-release:
	$(MAKE) check
	$(MAKE) check-http
	$(CARGO) build --locked --release

check-http:
	$(CARGO) test --locked --test http -- --ignored

# Requires a real X11 display, or xvfb-run, and the named GUI test tools.
# This is separate from local checks because it opens a desktop window.
check-alacritty:
	python3 tools/qualify_alacritty.py $(ALACRITTY_ARGS)

bench:
	$(CARGO) bench --locked --bench reader
	$(CARGO) bench --locked --bench reader -- --navigation-only
	$(CARGO) bench --locked --bench reader -- --stress
	$(CARGO) bench --locked --bench reader -- --stress --navigation-only

build:
	$(CARGO) build --locked --release

demo:
	$(CARGO) run --locked -- --demo
