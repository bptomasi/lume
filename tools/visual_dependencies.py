"""Actionable preflight for optional visual qualification dependencies."""

import importlib


def pillow():
    try:
        return importlib.import_module("PIL.Image")
    except ImportError as error:
        raise RuntimeError(
            "Pillow is required for visual tooling. Create a virtualenv and run "
            "python -m pip install -r tools/requirements-visual.txt; "
            "Lume itself does not require Python."
        ) from error
