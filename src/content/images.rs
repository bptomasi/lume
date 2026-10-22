//! Preserve publisher image references without loading remote assets.

use super::Fragment;

impl Fragment {
    pub(super) fn image(&mut self, tag: &html5gum::StartTag<()>) {
        let address = self.responsive_image(tag).or_else(|| {
            [b"data-src".as_slice(), b"src".as_slice()]
                .into_iter()
                .filter_map(|name| tag.attributes.get(name))
                .find_map(|attribute| self.link_url(&attribute.value))
        });
        let Some(address) = address else {
            return;
        };
        let alt = tag
            .attributes
            .get(b"alt".as_slice())
            .map(|attribute| String::from_utf8_lossy(&attribute.value))
            .unwrap_or_default();
        let alt = super::label(&alt, 300)
            .replace('\\', "\\\\")
            .replace('[', "\\[")
            .replace(']', "\\]");
        let markdown = format!("![{alt}](<{address}>)");
        if let Some(table) = &mut self.table {
            table.text(&markdown);
        } else {
            self.text.push_str(&markdown);
        }
    }

    fn responsive_image(&self, tag: &html5gum::StartTag<()>) -> Option<String> {
        [b"data-srcset".as_slice(), b"srcset".as_slice()]
            .into_iter()
            .filter_map(|name| tag.attributes.get(name))
            .find_map(|attribute| {
                let value = String::from_utf8_lossy(&attribute.value);
                // Commas inside inline data are not candidate separators.
                // Those sets retain the ordinary, independently validated src.
                if value
                    .as_bytes()
                    .windows(5)
                    .any(|window| window.eq_ignore_ascii_case(b"data:"))
                {
                    return None;
                }
                value
                    .split(',')
                    .take(128)
                    .filter_map(|candidate| self.image_candidate(candidate))
                    .max_by_key(|(quality, _)| *quality)
                    .map(|(_, address)| address)
            })
    }

    fn image_candidate(&self, candidate: &str) -> Option<(u32, String)> {
        let mut parts = candidate.split_ascii_whitespace();
        let address = parts.next()?;
        let descriptor = parts.next().unwrap_or("1x");
        if parts.next().is_some() || address.starts_with('#') {
            return None;
        }
        let quality = if let Some(width) = descriptor.strip_suffix('w') {
            let width = width.parse::<u32>().ok()?;
            (1..=4096).contains(&width).then_some(width)?
        } else {
            let density = descriptor.strip_suffix('x')?.parse::<f32>().ok()?;
            if !(0.0..=4.0).contains(&density) || density == 0.0 {
                return None;
            }
            (density * 1000.0) as u32
        };
        Some((quality, self.link_url(address.as_bytes())?))
    }
}
