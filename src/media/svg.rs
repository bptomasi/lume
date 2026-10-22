//! Restricted vector rendering: no resource resolver can read publisher paths.

use anyhow::{Context, Result, ensure};
use image::{DynamicImage, RgbaImage};
use resvg::{tiny_skia, usvg};
use sha2::{Digest, Sha256};

use super::ImageKey;

#[derive(Default)]
pub(super) struct SvgDecoder {
    options: Option<usvg::Options<'static>>,
    cached: Option<([u8; 32], usvg::Tree)>,
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
        let image = unpremultiply(&pixmap)?;
        Ok((image, dimensions))
    }

    fn tree(&mut self, bytes: &[u8]) -> Result<&usvg::Tree> {
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        if self.cached.as_ref().is_none_or(|(old, _)| *old != digest) {
            // One admitted source only. Failed input cannot render stale paths.
            self.cached = None;
            let xml = parse(bytes)?;
            let options = self.options.get_or_insert_with(restricted_options);
            if xml.descendants().any(|node| node.has_tag_name("text")) {
                super::fonts::prepare(options)?;
            }
            self.cached =
                Some((digest, usvg::Tree::from_xmltree(&xml, options)?));
        }
        self.cached
            .as_ref()
            .map(|(_, tree)| tree)
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

fn unpremultiply(pixmap: &tiny_skia::Pixmap) -> Result<DynamicImage> {
    let pixels = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();
    let image = RgbaImage::from_raw(pixmap.width(), pixmap.height(), pixels)
        .context("Invalid SVG thumbnail dimensions")?;
    Ok(DynamicImage::ImageRgba8(image))
}
