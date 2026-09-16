use std::io;

pub struct Reader<'a> {
    bytes: &'a [u8],
}
impl<'a> Reader<'a> {
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }
    pub fn take(&mut self, length: usize) -> io::Result<&'a [u8]> {
        let (taken, rest) = self
            .bytes
            .split_at_checked(length)
            .ok_or_else(|| io::Error::other("truncated common value"))?;
        self.bytes = rest;
        Ok(taken)
    }
    pub fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(io::Error::other)?,
        ))
    }
    pub fn u64(&mut self) -> io::Result<u64> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().map_err(io::Error::other)?,
        ))
    }
    pub fn bytes(&mut self) -> io::Result<&'a [u8]> {
        let length = usize::try_from(self.u32()?).map_err(io::Error::other)?;
        self.take(length)
    }
    pub fn end(self) -> io::Result<()> {
        if self.bytes.is_empty() {
            Ok(())
        } else {
            Err(io::Error::other("extra common value fields"))
        }
    }
}
