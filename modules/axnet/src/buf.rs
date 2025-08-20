use core::convert::Infallible;
use core::mem;

/// Read bytes from a buffer.
pub trait Buf {
    /// Returns the *minimum* number of bytes remaining in the buffer.
    fn remaining(&self) -> usize;

    /// Returns a slice starting at the current position and of length between
    /// `0` and `Buf::remaining()`.
    fn chunk(&self) -> &[u8];

    /// Advances the buffer by `n` bytes.
    fn advance(&mut self, n: usize);
}

/// Extension trait for `Buf`.
pub trait BufExt: Buf {
    /// Reads the buffer with the given function, which is called repeatedly
    /// until it returns `0` or the buffer is exhausted.
    fn read_with<R>(&mut self, mut f: impl FnMut(&[u8]) -> Result<usize, R>) -> Result<usize, R> {
        let mut read = 0;
        loop {
            let d = self.chunk();
            if d.is_empty() {
                break;
            }

            let cnt = f(d)?;
            if cnt == 0 {
                break;
            }

            self.advance(cnt);
            read += cnt;
        }
        Ok(read)
    }
}
impl<T: Buf> BufExt for T {}

/// A trait for values that provide sequential write access to bytes.
pub trait BufMut: Buf {
    /// Returns a mutable slice starting at the current position and of length
    /// between `0` and `Buf::remaining()`.
    fn chunk_mut(&mut self) -> &mut [u8];
}

/// Extension trait for `BufMut`.
pub trait BufMutExt: BufMut {
    /// Fills the buffer with the given function, which is called repeatedly
    /// until it returns `0` or the buffer is exhausted.
    fn fill_with<R>(
        &mut self,
        mut f: impl FnMut(&mut [u8]) -> Result<usize, R>,
    ) -> Result<usize, R> {
        let mut written = 0;
        loop {
            let d = self.chunk_mut();
            if d.is_empty() {
                break;
            }

            let cnt = f(d)?;
            if cnt == 0 {
                break;
            }

            self.advance(cnt);
            written += cnt;
        }
        Ok(written)
    }

    /// Transfer bytes into self from src and advance the cursor by the number of bytes written.
    fn put(&mut self, src: &mut impl Buf) -> usize {
        self.fill_with::<Infallible>(|chunk| {
            let s = src.chunk();
            let cnt = usize::min(s.len(), chunk.len());
            if cnt == 0 {
                return Ok(0);
            }

            chunk[..cnt].copy_from_slice(&s[..cnt]);
            src.advance(cnt);
            Ok(cnt)
        })
        .unwrap_or_else(|err| match err {})
    }
}
impl<T: BufMut> BufMutExt for T {}

impl Buf for &[u8] {
    #[inline]
    fn remaining(&self) -> usize {
        self.len()
    }

    #[inline]
    fn chunk(&self) -> &[u8] {
        self
    }

    #[inline]
    fn advance(&mut self, n: usize) {
        *self = &self[n..];
    }
}
impl Buf for &mut [u8] {
    #[inline]
    fn remaining(&self) -> usize {
        self.len()
    }

    #[inline]
    fn chunk(&self) -> &[u8] {
        self
    }

    #[inline]
    fn advance(&mut self, n: usize) {
        *self = &mut mem::take(self)[n..];
    }
}
impl BufMut for &mut [u8] {
    #[inline]
    fn chunk_mut(&mut self) -> &mut [u8] {
        self
    }
}
