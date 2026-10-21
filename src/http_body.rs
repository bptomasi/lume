//! Bound decoded HTTP bytes before handing them to domain-specific parsers.
//!
//! ureq's response configuration limits the wire stream before gzip decoding.
//! Reading one extra decoded byte distinguishes an exact fit from overflow.
//! Status, retries, cancellation and cache publication belong to each caller.

use std::io::Read;

use anyhow::{Context, Result};

#[derive(Debug)]
pub(crate) struct LimitExceeded {
    limit: usize,
}

impl std::fmt::Display for LimitExceeded {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Decoded response exceeds {} bytes", self.limit)
    }
}

impl std::error::Error for LimitExceeded {}

pub(crate) fn read(body: &mut ureq::Body, limit: usize) -> Result<Vec<u8>> {
    read_decoded(body.as_reader(), limit)
}

fn read_decoded(reader: impl Read, limit: usize) -> Result<Vec<u8>> {
    let probe = u64::try_from(limit)?.saturating_add(1);
    let mut bytes = Vec::new();
    reader
        .take(probe)
        .read_to_end(&mut bytes)
        .context("Could not read decoded HTTP response")?;
    if bytes.len() > limit {
        return Err(LimitExceeded { limit }.into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_limit_is_accepted_and_overflow_reads_only_one_extra_byte()
    -> Result<()> {
        assert_eq!(read_decoded(&b"abcd"[..], 4)?, b"abcd");
        let mut input = &b"abcdefghi"[..];
        assert!(read_decoded(&mut input, 4).is_err());
        assert_eq!(input, b"fghi");
        assert!(read_decoded(&b"x"[..], 0).is_err());
        assert!(read_decoded(&b""[..], 0)?.is_empty());
        Ok(())
    }

    #[test]
    fn read_errors_never_return_partial_success() -> Result<()> {
        let failure = std::io::Error::other("truncated response");
        let input = std::io::Cursor::new(b"prefix")
            .chain(std::io::Cursor::new(Vec::<u8>::new()));
        assert_eq!(read_decoded(input, 6)?, b"prefix");
        assert!(read_decoded(FailingReader(failure), 6).is_err());
        Ok(())
    }

    struct FailingReader(std::io::Error);

    impl Read for FailingReader {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(self.0.kind(), self.0.to_string()))
        }
    }
}
