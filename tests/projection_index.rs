//! Indexed quote projection agrees with an independent interval calculation.

use lume::reader::{SourceIndex, SourceMapping};

#[test]
fn interval_index_handles_reordering_overlap_entities_and_empty_ranges() {
    let mappings = reordered_tokens();
    let index = SourceIndex::new(&mappings);
    for start in 0..110 {
        for length in 0..20 {
            let selected = start..start + length;
            let expected = mappings
                .iter()
                .filter_map(|mapping| {
                    let from = mapping.source.start.max(selected.start);
                    let to = mapping.source.end.min(selected.end);
                    if from >= to {
                        return None;
                    }
                    if mapping.source.len() != mapping.display.len() {
                        return Some(mapping.display.clone());
                    }
                    Some(
                        mapping.display.start + from - mapping.source.start
                            ..mapping.display.start + to - mapping.source.start,
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(index.to_display(selected), expected);
        }
    }
}

fn reordered_tokens() -> Vec<SourceMapping> {
    vec![
        SourceMapping {
            source: 50..60,
            display: 0..10,
        },
        SourceMapping {
            source: 0..100,
            display: 10..13,
        },
        SourceMapping {
            source: 10..14,
            display: 13..17,
        },
        SourceMapping {
            source: 10..15,
            display: 17..18,
        },
        SourceMapping {
            source: 100..104,
            display: 18..22,
        },
    ]
}
