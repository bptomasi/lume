//! Restricted vector rendering: no resource resolver can read publisher paths.

use anyhow::{Context, Result, ensure};
use image::{DynamicImage, RgbaImage};
use resvg::{tiny_skia, usvg};

use super::ImageKey;

#[derive(Default)]
pub(super) struct SvgDecoder {
    options: Option<usvg::Options<'static>>,
}

impl SvgDecoder {
    pub(super) fn render(
        &mut self,
        bytes: &[u8],
        key: &ImageKey,
    ) -> Result<(DynamicImage, (u32, u32))> {
        ensure!(bytes.len() <= 512 * 1024, "SVG exceeds 512 KiB");
        let text =
            std::str::from_utf8(bytes).context("Image is not UTF-8 SVG")?;
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
        let options = self.options.get_or_insert_with(restricted_options);
        if xml.descendants().any(|node| node.has_tag_name("text")) {
            super::fonts::prepare(options)?;
        }
        let tree = usvg::Tree::from_xmltree(&xml, options)?;
        let size = tree.size();
        let scale = (f32::from(key.width) / size.width())
            .min(f32::from(key.height) * 2.0 / size.height());
        let width = (size.width() * scale).ceil().max(1.0) as u32;
        let height = (size.height() * scale).ceil().max(1.0) as u32;
        let mut pixmap = tiny_skia::Pixmap::new(width, height)
            .context("Could not allocate SVG thumbnail")?;
        resvg::render(
            &tree,
            tiny_skia::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );
        let image = unpremultiply(&pixmap)?;
        Ok((image, (size.width() as u32, size.height() as u32)))
    }
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
