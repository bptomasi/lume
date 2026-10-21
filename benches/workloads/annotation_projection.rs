//! Dense annotation projection includes index construction and checks each hit.

use anyhow::{Result, ensure};
use lume::reader::{SourceIndex, SourceMapping};

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let (mappings, quotes) = workload(stress);
    super::measure("dense annotations: linear reference", iterations, || {
        for (quote, expected) in &quotes {
            ensure!(
                lume::reader::to_display(&mappings, quote.clone())
                    == [expected.clone()],
                "Reference quotation projection changed"
            );
        }
        Ok(())
    })?;
    super::measure(
        "dense annotations: build index + all quotes",
        iterations,
        || {
            let index = SourceIndex::new(&mappings);
            for (quote, expected) in &quotes {
                ensure!(
                    index.to_display(quote.clone()) == [expected.clone()],
                    "Indexed quotation projection changed"
                );
            }
            Ok(())
        },
    )
}

type Quote = (std::ops::Range<usize>, std::ops::Range<usize>);

fn workload(stress: bool) -> (Vec<SourceMapping>, Vec<Quote>) {
    let count = if stress { 30_000 } else { 3_000 };
    let notes = if stress { 3_000 } else { 300 };
    let mappings = (0..count)
        .map(|index| SourceMapping {
            source: index * 12..index * 12 + 10,
            display: index * 10..index * 10 + 10,
        })
        .collect::<Vec<_>>();
    let quotes = (0..notes)
        .map(|index| {
            let token = index * (count / notes);
            (
                token * 12 + 2..token * 12 + 7,
                token * 10 + 2..token * 10 + 7,
            )
        })
        .collect::<Vec<_>>();
    (mappings, quotes)
}
