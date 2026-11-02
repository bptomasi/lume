//! Narrow adapter for icy_sixel 0.1.3, revision 80038c275a9a2075.
//! Palette selection requires flushed state. Only save_count is extended;
//! put_pixel owns ASCII conversion and put_flash owns RLE/255 semantics.
//! Dependency updates must re-review these invariants and exact-stream tests.
use icy_sixel::{
    SixelResult,
    output::{sixel_node, sixel_output},
};

pub(super) struct Output<'a> {
    writer: sixel_output<&'a mut Vec<u8>>,
    colors: usize,
    width: usize,
    limit: usize,
    scratch: usize,
}

impl<'a> Output<'a> {
    pub(super) fn new(
        bytes: &'a mut Vec<u8>,
        width: u32,
        height: u32,
        palette: &[u8],
        limit: usize,
    ) -> SixelResult<Self> {
        let mut writer = sixel_output::new(bytes);
        writer.encode_header(width as i32, height as i32)?;
        let colors = palette.len() / 3;
        for color in 0..colors {
            writer.output_rgb_palette_definition(palette, color as i32, -1)?;
        }
        let output = Self {
            writer,
            colors,
            width: width as usize,
            limit,
            scratch: 0,
        };
        output.check_size()?;
        Ok(output)
    }

    pub(super) fn scratch(&mut self, bytes: usize) {
        self.scratch = bytes;
    }

    fn observe(&self) {
        // The upstream String never shrinks; its final capacity is its peak.
        crate::media::profiling::encoder_scratch(
            self.scratch + self.writer.buffer.capacity(),
        );
    }

    pub(super) fn band(&mut self, band: usize) {
        if band != 0 {
            self.writer.putc('-');
        }
    }

    pub(super) fn color(
        &mut self,
        color: usize,
        painted: bool,
    ) -> SixelResult<()> {
        if self.writer.save_count != 0 || color >= self.colors {
            return Err("Sixel palette selection has pending pixels".into());
        }
        if painted {
            self.writer.putc('$');
        }
        let node = sixel_node {
            pal: color as i32,
            ..sixel_node::default()
        };
        self.writer.put_node(&mut 0, node, self.colors as i32, -1)
    }

    pub(super) fn run(&mut self, mask: u8, length: usize) -> SixelResult<()> {
        if mask > 63 || length == 0 || length > self.width {
            return Err("Invalid Sixel mask run".into());
        }
        self.writer.put_pixel(mask)?;
        let extra = i32::try_from(length - 1)?;
        self.writer.save_count = self
            .writer
            .save_count
            .checked_add(extra)
            .ok_or("Sixel repeat overflow")?;
        Ok(())
    }

    pub(super) fn flush(&mut self) -> SixelResult<()> {
        // Check at the old natural plane boundary: forcing earlier flushes
        // would change runs. At most W literal pixels plus a color selector
        // and separator can overshoot the cap; callers discard that failure.
        self.writer.put_flash()?;
        self.check_size()
    }

    fn check_size(&self) -> SixelResult<()> {
        // Upstream ignores Write errors. Its pending String has not yet been
        // copied to the Vec; reserve the unchanged two-byte 7-bit footer.
        let bytes = self
            .writer
            .fn_write
            .len()
            .checked_add(self.writer.buffer.len())
            .and_then(|bytes| bytes.checked_add(2));
        if bytes.is_none_or(|bytes| bytes > self.limit) {
            return Err("Native pixels exceed encoded output limit".into());
        }
        Ok(())
    }

    pub(super) fn finish(mut self) -> SixelResult<()> {
        if self.writer.save_count != 0 {
            return Err("Sixel footer has pending pixels".into());
        }
        self.check_size()?;
        self.writer.encode_footer()?;
        self.observe();
        // encode_footer retains its copied String: count the Vec only now.
        if self.writer.fn_write.len() > self.limit {
            return Err("Native pixels exceed encoded output limit".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
