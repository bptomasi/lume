use super::*;
use anyhow::Result;

#[test]
fn aggregated_runs_preserve_writer_state_and_both_gri_modes() -> Result<()> {
    for limited in [true, false] {
        for mask in 0..64 {
            for length in [1, 2, 3, 4, 254, 255, 256, 257, 510, 511, 10_240] {
                compare_run(mask, length, limited)?;
            }
        }
    }
    Ok(())
}

fn compare_run(mask: u8, length: usize, limited: bool) -> Result<()> {
    let mut expected_bytes = Vec::new();
    let mut expected = sixel_output::new(&mut expected_bytes);
    expected.set_gri_arg_limit(limited);
    for _ in 0..length {
        expected.put_pixel(mask).map_err(error)?;
    }
    expected.put_flash().map_err(error)?;
    let mut bytes = Vec::new();
    let mut actual = Output::new(&mut bytes, 10_240, 6, &[0; 3], usize::MAX)
        .map_err(error)?;
    actual.writer.buffer.clear();
    actual.writer.set_gri_arg_limit(limited);
    actual.run(mask, length).map_err(error)?;
    actual.flush().map_err(error)?;
    assert_eq!(actual.writer.save_count, 0);
    assert_eq!(actual.writer.save_pixel, 0);
    assert_eq!(actual.writer.buffer, expected.buffer);
    assert_eq!(actual.writer.fn_write, expected.fn_write);
    Ok(())
}

#[test]
fn palette_switches_require_flushed_runs_and_invalid_runs_are_rejected()
-> Result<()> {
    let mut bytes = Vec::new();
    let mut output =
        Output::new(&mut bytes, 8, 6, &[0; 6], usize::MAX).map_err(error)?;
    output.color(0, false).map_err(error)?;
    output.run(63, 8).map_err(error)?;
    assert!(output.color(1, true).is_err());
    output.flush().map_err(error)?;
    output.color(1, true).map_err(error)?;
    assert_eq!(output.writer.active_palette, 1);
    assert!(output.run(64, 1).is_err());
    assert!(output.run(0, 0).is_err());
    assert!(output.run(0, 9).is_err());
    assert!(output.color(2, true).is_err());
    Ok(())
}

#[test]
fn packet_and_footer_accounting_counts_pending_bytes_once() -> Result<()> {
    for length in [16_380, 16_383, 16_384, 16_385, 32_768] {
        let mut bytes = Vec::new();
        let mut output =
            Output::new(&mut bytes, 10_240, 6, &[0; 3], usize::MAX)
                .map_err(error)?;
        // Both flushed Vec and pending String participate before the footer.
        output.writer.fn_write.resize(length / 2, b'?');
        output.writer.buffer = "?".repeat(length - length / 2);
        output.limit = length + 1;
        assert!(output.check_size().is_err());
        output.limit = length + 2;
        output.finish().map_err(error)?;
        assert_eq!(bytes.len(), length + 2);
    }
    Ok(())
}

fn error(error: Box<dyn std::error::Error>) -> anyhow::Error {
    anyhow::anyhow!("{error}")
}

#[test]
fn production_default_limits_repeats_and_footer_requires_flushed_state()
-> Result<()> {
    let mut bytes = Vec::new();
    let mut output =
        Output::new(&mut bytes, 511, 6, &[0; 3], usize::MAX).map_err(error)?;
    output.run(63, 511).map_err(error)?;
    output.flush().map_err(error)?;
    assert!(output.writer.buffer.ends_with("!255~!255~~"));
    output.run(1, 1).map_err(error)?;
    assert!(output.finish().is_err());
    assert!(!bytes.ends_with(b"\x1b\\"));
    Ok(())
}
