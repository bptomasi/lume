//! Restricted vector rendering: no resource resolver can read publisher paths.

use anyhow::{Context, Result, ensure};
use image::{DynamicImage, RgbImage};
use resvg::{tiny_skia, usvg};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;

use super::ImageKey;

#[derive(Default)]
pub(super) struct SvgDecoder {
    options: Option<usvg::Options<'static>>,
    cached: VecDeque<Source>,
}

struct Source {
    digest: [u8; 32],
    bytes: usize,
    tree: usvg::Tree,
}

impl SvgDecoder {
    pub(super) fn render(
        &mut self,
        bytes: &[u8],
        key: &ImageKey,
    ) -> Result<(DynamicImage, (u32, u32))> {
        ensure!(bytes.len() <= 512 * 1024, "SVG exceeds 512 KiB");
        let tree = self.tree(bytes)?;
        let size = tree.size();
        let dimensions =
            (size.width().ceil() as u32, size.height().ceil() as u32);
        let geometry =
            super::geometry::Geometry::new((size.width(), size.height()), key);
        let mut pixmap =
            tiny_skia::Pixmap::new(geometry.width, geometry.height)
                .context("Could not allocate SVG thumbnail")?;
        let [red, green, blue] = key.background;
        pixmap.fill(tiny_skia::Color::from_rgba8(red, green, blue, 255));
        resvg::render(
            tree,
            tiny_skia::Transform::from_row(
                geometry.scale,
                0.0,
                0.0,
                geometry.scale,
                -geometry.offset[0],
                -geometry.offset[1],
            ),
            &mut pixmap.as_mut(),
        );
        let image = opaque_pixels(&pixmap)?;
        Ok((image, dimensions))
    }

    fn tree(&mut self, bytes: &[u8]) -> Result<&usvg::Tree> {
        let digest = Sha256::digest(bytes).into();
        if let Some(index) = self
            .cached
            .iter()
            .position(|source| source.digest == digest)
        {
            let source = self
                .cached
                .remove(index)
                .context("SVG cache entry disappeared")?;
            self.cached.push_front(source);
        } else {
            while self.cached.len() >= 2
                || self.cached.iter().map(|source| source.bytes).sum::<usize>()
                    + bytes.len()
                    > 512 * 1024
            {
                if self.cached.pop_back().is_none() {
                    break;
                }
            }
            let xml = parse(bytes)?;
            let options = self.options.get_or_insert_with(restricted_options);
            if xml.descendants().any(|node| node.has_tag_name("text")) {
                super::fonts::prepare(options)?;
            }
            let tree = usvg::Tree::from_xmltree(&xml, options)?;
            self.cached.push_front(Source {
                digest,
                bytes: bytes.len(),
                tree,
            });
        }
        self.cached
            .front()
            .map(|source| &source.tree)
            .context("SVG source was not prepared")
    }
}

fn parse(bytes: &[u8]) -> Result<roxmltree::Document<'_>> {
    let text = std::str::from_utf8(bytes).context("Image is not UTF-8 SVG")?;
    let xml = roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 5000,
            ..Default::default()
        },
    )
    .context("Invalid or overly complex SVG")?;
    ensure!(
        xml.root_element().tag_name().name() == "svg",
        "Supported images: PNG, JPEG, WebP, GIF and SVG"
    );
    Ok(xml)
}

fn restricted_options() -> usvg::Options<'static> {
    usvg::Options {
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..Default::default()
    }
}

fn opaque_pixels(pixmap: &tiny_skia::Pixmap) -> Result<DynamicImage> {
    let pixels = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| [pixel.red(), pixel.green(), pixel.blue()])
        .collect();
    let image = RgbImage::from_raw(pixmap.width(), pixmap.height(), pixels)
        .context("Invalid SVG thumbnail dimensions")?;
    Ok(DynamicImage::ImageRgb8(image))
}
