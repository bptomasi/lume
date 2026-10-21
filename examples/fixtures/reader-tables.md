# Clear comparisons

Compare the tradeoffs, then keep the code close to the explanation.

| Strategy | Coordination | Reads | Writes |
| :--- | :---: | ---: | ---: |
| **Leader** | Consensus | 1 hop | 2 hops |
| Quorum | Majority | 2 hops | 2 hops |
| Local cache | None | 0 hops | Async |

## One continuous code surface

```rust
fn commit(value: &Record) -> Result<()> {
    journal.append(value)?;

    Ok(())
}
```

Short rows and blank lines share the same background. Inline `journal.append`
stays compact inside an ordinary paragraph.

Use **zh / zl** to pan a wide table, or **:tables** to open its inspector.
