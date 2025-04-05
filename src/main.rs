#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[cfg(unix)]
use std::fs::File;
#[cfg(unix)]
use std::os::unix::io::FromRawFd;

// in unix, line separate is "\n" we need to trim 1 character.
#[cfg(unix)]
pub const TRIM: usize = 1;
// in windows, line separate is "\r\n" we need to trim 2 characters.
#[cfg(windows)]
pub const TRIM: usize = 2;

use std::io::{self, BufRead, Read, Write};

const ONES: *const u8 = b"1111111111111111__T_H_O_R_I_U_M_".as_ptr();

type SimdFn = unsafe fn(*const u8, *const u8) -> u32;

#[inline]
#[target_feature(enable = "sse2")]
unsafe fn hash_256bit_sse2(ptr: *const u8, b: *const u8) -> u32 {
    let n1 = hash_128bit_sse2(ptr, ONES);
    let n2 = hash_128bit_sse2(ptr.add(16), b);
    let num = n1 + n2.rotate_left(16);
    num
}

#[inline]
#[target_feature(enable = "sse2")]
unsafe fn hash_128bit_sse2(ptr: *const u8, b_ptr: *const u8) -> u32 {
    let a = _mm_loadu_si128(ptr as *const __m128i);
    let b = _mm_loadu_si128(b_ptr as *const __m128i);
    let eq = _mm_cmpeq_epi8(a, b);
    let mask = _mm_movemask_epi8(eq);
    mask as u32
}

struct Table256bit {
    key: Vec<u32>,
    val: Vec<u32>,
    size: usize,
}

impl Table256bit {
    const MAXS: [u32; 4] = [u32::MAX; 4];

    #[inline]
    fn new(size: usize) -> Self {
        let pow = (32 - (size as u32).leading_zeros()) + 2;
        let size = (1 << pow) - 1;
        Self {
            key: vec![u32::MAX; size + 5],
            val: vec![u32::MIN; size + 5],
            size,
        }
    }

    #[inline]
    fn insert(&mut self, key: u32, val: u32) {
        let mut mask = key as usize & self.size;
        unsafe {
            if *self.key.get_unchecked(mask) == u32::MAX {
                *self.key.get_unchecked_mut(mask) = key;
                *self.val.get_unchecked_mut(mask) = val;
                return;
            }
            let ptr = self.key.as_ptr();
            let maxs = _mm_loadu_si128(Self::MAXS.as_ptr() as *const __m128i);
            let b = _mm_set1_epi32(key as i32);
            loop {
                let a = _mm_loadu_si128(ptr.add(mask) as *const __m128i);
                let eqk = _mm_cmpeq_epi32(a, b);
                let mask_key = _mm_movemask_epi8(eqk);
                if mask_key > 0 {
                    let pos = mask + (mask_key.trailing_zeros() as usize >> 2);
                    *self.key.get_unchecked_mut(pos) = key;
                    *self.val.get_unchecked_mut(pos) = val;
                    return;
                }
                let eqx = _mm_cmpeq_epi32(a, maxs);
                let mask_max = _mm_movemask_epi8(eqx);
                if mask_max > 0 {
                    let pos = mask + (mask_max.trailing_zeros() as usize >> 2);
                    *self.key.get_unchecked_mut(pos) = key;
                    *self.val.get_unchecked_mut(pos) = val;
                    return;
                }
                mask = (mask + 4) & self.size;
            }
        }
    }

    #[inline]
    fn search(&mut self, key: u32) -> Option<&u32> {
        let mut mask = key as usize & self.size;
        unsafe {
            let ptr = self.key.as_ptr();
            let maxs = _mm_loadu_si128(Self::MAXS.as_ptr() as *const __m128i);
            let b = _mm_set1_epi32(key as i32);
            loop {
                let a = _mm_loadu_si128(ptr.add(mask) as *const __m128i);
                let eqk = _mm_cmpeq_epi32(a, b);
                let mask_key = _mm_movemask_epi8(eqk);
                if mask_key > 0 {
                    let pos = mask + (mask_key.trailing_zeros() as usize >> 2);
                    return Some(self.val.get_unchecked(pos));
                }
                let eqx = _mm_cmpeq_epi32(a, maxs);
                let mask_max = _mm_movemask_epi8(eqx);
                if mask_max > 0 {
                    return None;
                }
                mask = (mask + 4) & self.size;
            }
        }
    }
}

struct SequenceReader<R: BufRead + Read> {
    inner: R,
    haystack: Vec<u8>,
    k: usize,
    m: usize,
}

impl<R: BufRead + Read> SequenceReader<R> {
    #[inline]
    fn new(mut inner: R) -> Self {
        let mut buf = String::with_capacity(10);
        inner.read_line(&mut buf).unwrap();
        let init: Vec<usize> = buf.trim().split(' ').map(|x| x.parse().unwrap()).collect();
        Self {
            inner,
            haystack: Vec::with_capacity(1_000_012),
            k: init[0],
            m: init[1],
        }
    }

    #[inline]
    fn get_m(&self) -> usize {
        self.m
    }

    #[inline]
    fn get_k(&self) -> usize {
        self.k
    }

    #[inline]
    fn read_needles(
        &mut self,
        needles: &mut Table256bit,
        hash: SimdFn,
        b: *const u8,
    ) -> io::Result<()> {
        let end = self.m + TRIM;
        let mut buf = vec![0u8; end];
        for i in 0..self.k {
            self.inner.read_exact(&mut buf)?;
            let num = unsafe { hash(buf.as_ptr(), b) };
            needles.insert(num, (i + 1) as u32);
        }
        Ok(())
    }

    #[inline]
    fn read_n(&mut self) -> io::Result<usize> {
        let mut buf = String::with_capacity(4);
        self.inner.read_line(&mut buf)?;
        Ok(buf.trim().parse().unwrap())
    }

    #[inline]
    fn next(&mut self) -> io::Result<&[u8]> {
        let mut buf = String::with_capacity(8);
        self.inner.read_line(&mut buf)?;
        let end: usize = buf.trim().parse().unwrap();
        unsafe { self.haystack.set_len(end + TRIM) };
        self.inner.read_exact(&mut self.haystack)?;
        unsafe { self.haystack.set_len(end + 12) };
        self.haystack[end..end + 12].copy_from_slice(&[0u8; 12]);
        Ok(&self.haystack[..end])
    }
}

fn main() -> io::Result<()> {
    // Solved by Thorium

    let stdin = io::stdin();
    let stdin = stdin.lock();

    #[cfg(unix)]
    let mut stdout = unsafe { File::from_raw_fd(1) };
    #[cfg(not(unix))]
    let mut stdout = io::stdout();

    let mut reader = SequenceReader::new(stdin);

    let m = reader.get_m();
    let k = reader.get_k();

    let (hash, b): (SimdFn, _) = if m > 16 {
        let pos = 16 - (m - 16);
        (hash_256bit_sse2, unsafe { ONES.add(pos) })
    } else {
        let pos = 16 - m;
        (hash_128bit_sse2, unsafe { ONES.add(pos) })
    };
    let bitset = 1 << (m - 1);

    let mut needles = Table256bit::new(k);
    reader.read_needles(&mut needles, hash, b)?;

    let n = reader.read_n()?;

    let mut result = vec![0u8; k + 1];
    let mut out = String::with_capacity(2048);

    let mut _empty: bool;

    for _ in 0..n {
        _empty = true;
        let haystack = reader.next()?;
        let ptr = haystack.as_ptr();
        let len = haystack.len();
        if haystack.len() < m {
            stdout.write_all(b"OK\n")?;
            continue;
        }
        let mut num = unsafe { hash(ptr, b) };
        for i in m..len {
            if let Some(index) = needles.search(num) {
                unsafe {
                    *result.get_unchecked_mut(*index as usize) = 1;
                }
                _empty = false;
            }
            num >>= 1;
            if unsafe { *haystack.get_unchecked(i) } == b'1' {
                num |= bitset;
            }
        }
        if _empty {
            stdout.write_all(b"OK\n")?;
        } else {
            out.clear();
            unsafe {
                _empty = true;
                for i in 1..=k {
                    if *result.get_unchecked(i) == 1 {
                        out.push_str(&i.to_string());
                        out.push(' ');
                        *result.get_unchecked_mut(i) = 0;
                    }
                }
            }
            out.pop();
            stdout.write_all(out.as_bytes())?;
            stdout.write_all(b"\n")?;
        }
    }
    Ok(())
}
