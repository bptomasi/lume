"""Render an exported Ratatui buffer for human visual review (requires Pillow)."""

import argparse
import json
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


def render(source: Path, destination: Path) -> None:
    snapshot = json.loads(source.read_text())
    font = ImageFont.truetype("DejaVuSansMono.ttf", 15)
    cell_width = 10
    cell_height = 21
    padding = 24
    image = Image.new(
        "RGB",
        (
            snapshot["width"] * cell_width + padding * 2,
            snapshot["height"] * cell_height + padding * 2,
        ),
        snapshot["cells"][0][2],
    )
    draw = ImageDraw.Draw(image)
    for index, (symbol, foreground, background) in enumerate(snapshot["cells"]):
        column = index % snapshot["width"]
        row = index // snapshot["width"]
        left = padding + column * cell_width
        top = padding + row * cell_height
        draw.rectangle(
            (left, top, left + cell_width - 1, top + cell_height - 1),
            fill=background,
        )
        # Terminal block cells fill their pixel regions without font bearings.
        # Drawing them as font glyphs introduces seams absent from the buffer.
        if symbol == "▀":
            draw.rectangle(
                (left, top, left + cell_width - 1, top + cell_height // 2 - 1),
                fill=foreground,
            )
        elif symbol == "▄":
            draw.rectangle(
                (left, top + cell_height // 2,
                 left + cell_width - 1, top + cell_height - 1),
                fill=foreground,
            )
        else:
            draw.text((left, top), symbol, font=font, fill=foreground)
    destination.parent.mkdir(parents=True, exist_ok=True)
    image.save(destination)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    render(args.source, args.destination)


if __name__ == "__main__":
    main()
