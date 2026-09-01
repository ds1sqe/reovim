//! First-party streaming SHA-256 used for evidence identity.

use std::{fs::File, io, io::Read, io::Seek, path::Path};

const INITIAL: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

const ROUND: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

/// Incremental SHA-256 state.
#[derive(Clone)]
#[allow(clippy::redundant_pub_crate)]
pub(crate) struct Sha256 {
    state: [u32; 8],
    block: [u8; 64],
    block_len: usize,
    byte_len: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self { state: INITIAL, block: [0; 64], block_len: 0, byte_len: 0 }
    }
}

impl Sha256 {
    /// Adds bytes to the digest.
    /// The caller must keep the aggregate input below SHA-256's `2^64`-bit
    /// format bound. All checker call sites enforce much smaller committed
    /// byte limits; the assertion keeps future call sites fail-loud.
    pub(crate) fn update(&mut self, mut input: &[u8]) {
        let input_len = u64::try_from(input.len()).expect("slice length fits u64");
        self.byte_len = self.byte_len.checked_add(input_len).expect("SHA-256 byte count overflow");
        assert!(self.byte_len <= u64::MAX / 8, "SHA-256 input exceeds format bit-length bound");
        if self.block_len != 0 {
            let take = (64 - self.block_len).min(input.len());
            self.block[self.block_len..self.block_len + take].copy_from_slice(&input[..take]);
            self.block_len += take;
            input = &input[take..];
            if self.block_len == 64 {
                compress(&mut self.state, &self.block);
                self.block_len = 0;
            }
            if input.is_empty() {
                return;
            }
        }
        while input.len() >= 64 {
            let (head, tail) = input.split_at(64);
            let block: &[u8; 64] = head.try_into().expect("64-byte split");
            compress(&mut self.state, block);
            input = tail;
        }
        self.block[..input.len()].copy_from_slice(input);
        self.block_len = input.len();
    }

    /// Finishes the digest.
    #[must_use]
    pub(crate) fn finish(mut self) -> [u8; 32] {
        let bit_len = self.byte_len.checked_mul(8).expect("bound checked by update");
        self.block[self.block_len] = 0x80;
        self.block_len += 1;
        if self.block_len > 56 {
            self.block[self.block_len..].fill(0);
            compress(&mut self.state, &self.block);
            self.block = [0; 64];
        } else {
            self.block[self.block_len..56].fill(0);
        }
        self.block[56..].copy_from_slice(&bit_len.to_be_bytes());
        compress(&mut self.state, &self.block);

        let mut output = [0; 32];
        for (chunk, word) in output.as_chunks_mut::<4>().0.iter_mut().zip(self.state) {
            chunk.copy_from_slice(&word.to_be_bytes());
        }
        output
    }
}

/// Hashes bytes and returns a stable `sha256:` identity.
#[must_use]
pub fn bytes(input: &[u8]) -> String {
    let mut digest = Sha256::default();
    digest.update(input);
    tagged_hex(digest.finish())
}

/// Hashes at most `byte_limit` bytes and rejects growth beyond that bound.
pub fn file_bounded(path: &Path, byte_limit: u64) -> io::Result<String> {
    let file = File::open(path)?;
    file_handle_bounded(&file, byte_limit)
}

/// Hashes a held file descriptor, preserving inode identity across pathname
/// replacement or unlink.
pub fn file_handle_bounded(file: &File, byte_limit: u64) -> io::Result<String> {
    let before = file.metadata()?;
    if before.len() > byte_limit {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file exceeds digest byte limit"));
    }
    let mut input = file.try_clone()?;
    input.seek(io::SeekFrom::Start(0))?;
    let mut digest = Sha256::default();
    let mut buffer = [0_u8; 16 * 1024];
    let mut total = 0_u64;
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(count).map_err(io::Error::other)?)
            .ok_or_else(|| io::Error::other("file byte count overflow"))?;
        if total > byte_limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "file grew beyond digest byte limit",
            ));
        }
        digest.update(&buffer[..count]);
    }
    let after = file.metadata()?;
    if before.len() != total || after.len() != total {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file changed while hashing"));
    }
    Ok(tagged_hex(digest.finish()))
}

fn tagged_hex(input: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(71);
    result.push_str("sha256:");
    for byte in input {
        result.push(char::from(HEX[usize::from(byte >> 4)]));
        result.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    result
}

pub fn git_blob(bytes: &[u8], object_format: &str) -> io::Result<String> {
    let prefix = format!("blob {}\0", bytes.len());
    match object_format {
        "sha1" => {
            let mut digest = Sha1::default();
            digest.update(prefix.as_bytes());
            digest.update(bytes);
            Ok(plain_hex(&digest.finish()))
        }
        "sha256" => {
            let mut digest = Sha256::default();
            digest.update(prefix.as_bytes());
            digest.update(bytes);
            Ok(plain_hex(&digest.finish()))
        }
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported Git object format {other:?}"),
        )),
    }
}

fn plain_hex(input: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(input.len() * 2);
    for byte in input {
        result.push(char::from(HEX[usize::from(byte >> 4)]));
        result.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    result
}

#[derive(Clone)]
struct Sha1 {
    state: [u32; 5],
    block: [u8; 64],
    block_len: usize,
    byte_len: u64,
}

impl Default for Sha1 {
    fn default() -> Self {
        Self {
            state: [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476, 0xc3d2_e1f0],
            block: [0; 64],
            block_len: 0,
            byte_len: 0,
        }
    }
}

impl Sha1 {
    fn update(&mut self, mut input: &[u8]) {
        let count = u64::try_from(input.len()).expect("slice length fits u64");
        self.byte_len = self.byte_len.checked_add(count).expect("SHA-1 byte count overflow");
        assert!(self.byte_len <= u64::MAX / 8, "SHA-1 input exceeds format bit-length bound");
        if self.block_len != 0 {
            let take = (64 - self.block_len).min(input.len());
            self.block[self.block_len..self.block_len + take].copy_from_slice(&input[..take]);
            self.block_len += take;
            input = &input[take..];
            if self.block_len == 64 {
                compress_sha1(&mut self.state, &self.block);
                self.block_len = 0;
            }
            if input.is_empty() {
                return;
            }
        }
        while input.len() >= 64 {
            let (head, tail) = input.split_at(64);
            compress_sha1(&mut self.state, head.try_into().expect("64-byte split"));
            input = tail;
        }
        self.block[..input.len()].copy_from_slice(input);
        self.block_len = input.len();
    }

    fn finish(mut self) -> [u8; 20] {
        let bit_len = self.byte_len.checked_mul(8).expect("bound checked by update");
        self.block[self.block_len] = 0x80;
        self.block_len += 1;
        if self.block_len > 56 {
            self.block[self.block_len..].fill(0);
            compress_sha1(&mut self.state, &self.block);
            self.block = [0; 64];
        } else {
            self.block[self.block_len..56].fill(0);
        }
        self.block[56..].copy_from_slice(&bit_len.to_be_bytes());
        compress_sha1(&mut self.state, &self.block);
        let mut output = [0; 20];
        for (chunk, word) in output.as_chunks_mut::<4>().0.iter_mut().zip(self.state) {
            chunk.copy_from_slice(&word.to_be_bytes());
        }
        output
    }
}

#[allow(clippy::many_single_char_names)]
fn compress_sha1(state: &mut [u32; 5], block: &[u8; 64]) {
    let mut words = [0_u32; 80];
    for (index, chunk) in block.as_chunks::<4>().0.iter().enumerate() {
        words[index] = u32::from_be_bytes(*chunk);
    }
    for index in 16..80 {
        words[index] =
            (words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16])
                .rotate_left(1);
    }
    let [mut a, mut b, mut c, mut d, mut e] = *state;
    for (index, word) in words.into_iter().enumerate() {
        let (function, constant) = match index {
            0..=19 => ((b & c) | ((!b) & d), 0x5a82_7999),
            20..=39 => (b ^ c ^ d, 0x6ed9_eba1),
            40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc),
            _ => (b ^ c ^ d, 0xca62_c1d6),
        };
        let next = a
            .rotate_left(5)
            .wrapping_add(function)
            .wrapping_add(e)
            .wrapping_add(constant)
            .wrapping_add(word);
        e = d;
        d = c;
        c = b.rotate_left(30);
        b = a;
        a = next;
    }
    for (slot, value) in state.iter_mut().zip([a, b, c, d, e]) {
        *slot = slot.wrapping_add(value);
    }
}

#[allow(clippy::many_single_char_names)]
fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut schedule = [0_u32; 64];
    for (index, chunk) in block.as_chunks::<4>().0.iter().enumerate() {
        schedule[index] = u32::from_be_bytes(*chunk);
    }
    for index in 16..64 {
        let s0 = schedule[index - 15].rotate_right(7)
            ^ schedule[index - 15].rotate_right(18)
            ^ (schedule[index - 15] >> 3);
        let s1 = schedule[index - 2].rotate_right(17)
            ^ schedule[index - 2].rotate_right(19)
            ^ (schedule[index - 2] >> 10);
        schedule[index] = schedule[index - 16]
            .wrapping_add(s0)
            .wrapping_add(schedule[index - 7])
            .wrapping_add(s1);
    }

    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for index in 0..64 {
        let upper = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let choose = (e & f) ^ ((!e) & g);
        let first = h
            .wrapping_add(upper)
            .wrapping_add(choose)
            .wrapping_add(ROUND[index])
            .wrapping_add(schedule[index]);
        let lower = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let majority = (a & b) ^ (a & c) ^ (b & c);
        let second = lower.wrapping_add(majority);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(first);
        d = c;
        c = b;
        b = a;
        a = first.wrapping_add(second);
    }
    for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *slot = slot.wrapping_add(value);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::bytes;

    static TEMP_ID: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn standard_vectors() {
        assert_eq!(
            bytes(b""),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            bytes(b"abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            bytes(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "sha256:248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        assert_eq!(
            bytes(&vec![b'a'; 1_000_000]),
            "sha256:cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn streaming_matches_one_shot() {
        let mut digest = super::Sha256::default();
        digest.update(b"a");
        digest.update(b"bc");
        assert_eq!(super::tagged_hex(digest.finish()), bytes(b"abc"));
    }

    #[test]
    fn padding_boundaries_match_across_chunk_splits() {
        for length in [55, 56, 63, 64, 65] {
            let input = vec![b'x'; length];
            let expected = bytes(&input);
            for split in 0..=length {
                let mut digest = super::Sha256::default();
                digest.update(&input[..split]);
                digest.update(&input[split..]);
                assert_eq!(super::tagged_hex(digest.finish()), expected, "{length}/{split}");
            }
        }
    }

    #[test]
    fn sha1_and_git_blob_known_answers() {
        let sha1 = |input: &[u8]| {
            let mut digest = super::Sha1::default();
            digest.update(input);
            super::plain_hex(&digest.finish())
        };
        assert_eq!(sha1(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(sha1(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            sha1(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
        assert_eq!(sha1(&vec![b'a'; 1_000_000]), "34aa973cd4c4daa4f61eeb2bdbad27316534016f");
        assert_eq!(
            super::git_blob(b"", "sha1").unwrap(),
            "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"
        );
        assert_eq!(
            super::git_blob(b"abc", "sha1").unwrap(),
            "f2ba8f84ab5c1bce84a7b441cb1959cfc7093b7f"
        );
        assert_eq!(
            super::git_blob(b"", "sha256").unwrap(),
            "473a0f4c3be8a93681a267e3b1e9a7dcda1185436fe141f7749120a303721813"
        );
        assert_eq!(
            super::git_blob(b"abc", "sha256").unwrap(),
            "c1cf6e465077930e88dc5136641d402f72a229ddd996f627d60e9639eaba35a6"
        );
    }

    #[test]
    fn sha1_chunk_boundaries_match() {
        for length in [55, 56, 63, 64, 65] {
            let input = vec![b'y'; length];
            let mut one = super::Sha1::default();
            one.update(&input);
            let expected = one.finish();
            for split in 0..=length {
                let mut chunks = super::Sha1::default();
                chunks.update(&input[..split]);
                chunks.update(&input[split..]);
                assert_eq!(chunks.finish(), expected, "{length}/{split}");
            }
        }
    }

    #[test]
    fn bounded_file_accepts_cap_and_rejects_cap_plus_one() {
        let path = std::env::temp_dir().join(format!(
            "reovim-sha256-{}-{}",
            std::process::id(),
            TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, b"1234").unwrap();
        assert_eq!(super::file_bounded(&path, 4).unwrap(), bytes(b"1234"));
        fs::write(&path, b"12345").unwrap();
        assert_eq!(
            super::file_bounded(&path, 4).unwrap_err().kind(),
            std::io::ErrorKind::InvalidData
        );
        fs::remove_file(path).unwrap();
    }
}
