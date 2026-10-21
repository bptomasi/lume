# Decoded HTTP boundary fixtures

These deliberately small, deterministic gzip streams expand to exact domain
limits and one byte beyond them. The gzip timestamp is zero. They are wire
fixtures, not files loaded by production code or arbitrary publisher content.

- article-exact.gz: 2,097,152 bytes of valid HTML, a short #article main element
  and a padding comment. article-over.gz adds one comment byte.
- image-exact.gz: a 100x40 solid orange RGB PNG followed by zero padding to
  8,388,608 bytes. image-over.gz adds one padding byte. PNG trailing padding
  keeps the valid raster contract independent of SVG's stricter 512 KiB limit.

Native real-HTTP tests assert acceptance at the inclusive limit, rejection
above it, invalid-gzip failure and preservation/nonpublication of cache data.
Feed boundary streams remain alongside the existing feed fixtures.
