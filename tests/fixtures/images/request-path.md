# One request, three responsibilities

The API accepts work, the queue absorbs bursts, and the database owns durable
state.

![Request path: API → queue → database](https://example.org/request-path.svg)

## Keep ownership explicit

An image can clarify a boundary. Notes and selections keep their positions
while it loads.

Use :image-open to inspect the original diagram in your browser.
