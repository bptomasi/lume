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

#[test]
fn unavailable_source_notifies_a_resized_view_without_installing_stale_cells()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = crate::store::Store::open(&directory.path().join("library"))?;
    let mut app = App::new(store, false)?;
    let old = native(0).0;
    let mut current = old.clone();
    current.width = 80;
    let wanted = vec![current.clone()];
    let completed = crate::media::Completion {
        profile: None,
        key: old.clone(),
        result: Err("SVG unavailable in this build".into()),
        unavailable: true,
    };
    assert!(app.install_thumbnail(completed, &wanted));
    assert!(app.images.error(&current).is_some());
    assert!(app.images.thumbnail(&old).is_none());
    assert!(app.images.thumbnail(&current).is_none());
    assert!(app.images.viewport(wanted).is_empty());
    Ok(())
}

#[test]
fn an_unavailable_completion_for_an_unwanted_source_is_discarded() -> Result<()>
{
    let directory = tempfile::tempdir()?;
    let store = crate::store::Store::open(&directory.path().join("library"))?;
    let mut app = App::new(store, false)?;
    let key = native(0).0;
    let completed = crate::media::Completion {
        profile: None,
        key: key.clone(),
        result: Err("SVG unavailable in this build".into()),
        unavailable: true,
    };
    assert!(!app.install_thumbnail(completed, &[]));
    assert!(app.images.error(&key).is_none());
    assert_eq!(app.images.viewport(vec![key.clone()]), vec![key]);
    Ok(())
}

#[test]
fn unavailable_sources_bound_capacity_and_preserve_the_total_cache_budget() {
    let (key, mut thumbnail) = native(0);
    thumbnail.cells = Buffer::empty(Rect::new(0, 0, 1, 1));
    if let Some(Protocol::Sixel(image)) = &mut thumbnail.native {
        image.data = String::with_capacity(32 * 1024 * 1024 - 2048);
    }
    let mut images = ReaderImages::default();
    images.viewport(vec![key.clone()]);
    images.remember(key.clone(), Ok(thumbnail));
    assert!(images.thumbnail(&key).is_some());
    let mut source = key.clone();
    source.url = String::with_capacity(40 * 1024 * 1024);
    let mut message = String::with_capacity(40 * 1024 * 1024);
    message.push_str(&"€".repeat(1000));
    for index in 0..80 {
        source.url.clear();
        source
            .url
            .push_str(&format!("https://example.org/{index}.svg"));
        images.remember_unavailable(&source, &message);
        assert!(images.unavailable_bytes() <= 65_536);
        assert!(images.retained_bytes() <= 32 * 1024 * 1024);
    }
    assert!(images.error(&source).is_some());
    assert!(images.thumbnail(&key).is_none());
    assert!(images.deferred.contains(&key));
    assert!(images.viewport(vec![key]).is_empty());
    source.width /= 2;
    assert!(images.viewport(vec![source]).is_empty());
}
