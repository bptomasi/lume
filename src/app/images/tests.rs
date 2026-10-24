use super::*;
use ratatui::{buffer::Buffer, layout::Rect};
use ratatui_image::protocol::{Protocol, sixel::Sixel};

fn native(index: usize) -> (ImageKey, Thumbnail) {
    let area = Rect::new(0, 0, 100, 40);
    (
        ImageKey {
            url: format!("https://memory.example/{index}"),
            width: 100,
            height: 40,
            background: [0; 3],
            detail: None,
        },
        Thumbnail {
            cells: Buffer::empty(area),
            dimensions: (1000, 800),
            cache_warning: None,
            native: Some(Protocol::Sixel(Sixel {
                data: String::with_capacity(8 * 1024 * 1024),
                area,
                is_tmux: false,
            })),
            rows: None,
        },
    )
}

#[test]
fn native_payloads_and_cell_buffers_share_the_budget_without_admission_loops() {
    let mut images = ReaderImages::default();
    let visible = (0..5).map(|index| native(index).0).collect::<Vec<_>>();
    images.viewport(visible.clone());
    for index in 0..5 {
        let (key, thumbnail) = native(index);
        assert!(thumbnail.retained_bytes() > 8 * 1024 * 1024);
        images.remember(key, Ok(thumbnail));
        assert!(images.retained_bytes() <= 32 * 1024 * 1024);
    }
    assert_eq!(images.cache.len(), 3);
    assert_eq!(images.deferred.len(), 2);
    assert!(images.viewport(visible.clone()).is_empty());
    assert!(images.placeholder(&visible[0]).contains("budget"));
    assert!(
        images
            .viewport(vec![visible[0].clone()])
            .contains(&visible[0])
    );
}

#[test]
fn offscreen_payloads_are_evicted_before_visible_ones() {
    let mut images = ReaderImages::default();
    let visible = native(0).0;
    images.viewport(vec![visible.clone()]);
    for index in 0..5 {
        let (key, thumbnail) = native(index);
        images.remember(key, Ok(thumbnail));
    }
    assert!(images.thumbnail(&visible).is_some());
    assert!(images.deferred.is_empty());
    assert!(images.retained_bytes() <= 32 * 1024 * 1024);
}
