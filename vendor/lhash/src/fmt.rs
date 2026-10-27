use core::fmt;

const CHAR_TABLE: &[u8; 16] = b"0123456789abcdef";

///Wrapper to hex format digest
pub struct DigestFmt<T>(pub T);

impl<T: AsRef<[u8]>> fmt::Display for DigestFmt<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf: [u8; 2];

        for byt in self.0.as_ref() {
            buf = [
                CHAR_TABLE[(byt.wrapping_shr(4) & 0xf) as usize],
                CHAR_TABLE[(byt & 0xf) as usize]
            ];
            fmt.write_str(unsafe {
                core::str::from_utf8_unchecked(&buf)
            })?
        }

        Ok(())
    }
}
