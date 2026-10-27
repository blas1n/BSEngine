const BLOCK_SIZE: usize = 128;
const RESULT_SIZE: usize = 64;
const STATE_SIZE: usize = 8;
const INIT_STATE: [u64; STATE_SIZE] = [0x6a09e667f3bcc908, 0xbb67ae8584caa73b, 0x3c6ef372fe94f82b, 0xa54ff53a5f1d36f1, 0x510e527fade682d1, 0x9b05688c2b3e6c1f, 0x1f83d9abfb41bd6b, 0x5be0cd19137e2179];
const K512: [u64; 80] = [
	0x428a2f98d728ae22, 0x7137449123ef65cd, 0xb5c0fbcfec4d3b2f, 0xe9b5dba58189dbbc,
	0x3956c25bf348b538, 0x59f111f1b605d019, 0x923f82a4af194f9b, 0xab1c5ed5da6d8118,
	0xd807aa98a3030242, 0x12835b0145706fbe, 0x243185be4ee4b28c, 0x550c7dc3d5ffb4e2,
	0x72be5d74f27b896f, 0x80deb1fe3b1696b1, 0x9bdc06a725c71235, 0xc19bf174cf692694,
	0xe49b69c19ef14ad2, 0xefbe4786384f25e3, 0x0fc19dc68b8cd5b5, 0x240ca1cc77ac9c65,
	0x2de92c6f592b0275, 0x4a7484aa6ea6e483, 0x5cb0a9dcbd41fbd4, 0x76f988da831153b5,
	0x983e5152ee66dfab, 0xa831c66d2db43210, 0xb00327c898fb213f, 0xbf597fc7beef0ee4,
	0xc6e00bf33da88fc2, 0xd5a79147930aa725, 0x06ca6351e003826f, 0x142929670a0e6e70,
	0x27b70a8546d22ffc, 0x2e1b21385c26c926, 0x4d2c6dfc5ac42aed, 0x53380d139d95b3df,
	0x650a73548baf63de, 0x766a0abb3c77b2a8, 0x81c2c92e47edaee6, 0x92722c851482353b,
	0xa2bfe8a14cf10364, 0xa81a664bbc423001, 0xc24b8b70d0f89791, 0xc76c51a30654be30,
	0xd192e819d6ef5218, 0xd69906245565a910, 0xf40e35855771202a, 0x106aa07032bbd1b8,
	0x19a4c116b8d2d0c8, 0x1e376c085141ab53, 0x2748774cdf8eeb99, 0x34b0bcb5e19b48a8,
	0x391c0cb3c5c95a63, 0x4ed8aa4ae3418acb, 0x5b9cca4f7763e373, 0x682e6ff3d6b2b8a3,
	0x748f82ee5defb2fc, 0x78a5636f43172f60, 0x84c87814a1f0ab72, 0x8cc702081a6439ec,
	0x90befffa23631e28, 0xa4506cebde82bde9, 0xbef9a3f7b2c67915, 0xc67178f2e372532b,
	0xca273eceea26619c, 0xd186b8c721c0c207, 0xeada7dd6cde0eb1e, 0xf57d4f7fee6ed178,
	0x06f067aa72176fba, 0x0a637dc5a2c898a6, 0x113f9804bef90dae, 0x1b710b35131c471b,
	0x28db77f523047d84, 0x32caab7b40c72493, 0x3c9ebe0a15c9bebc, 0x431d67c49c100d4c,
	0x4cc5d4becb3e42b6, 0x597f299cfc657e2a, 0x5fcb6fab3ad6faec, 0x6c44198c4a475817
];

const fn sha512_transform(state: [u64; STATE_SIZE], cursor: usize, input: &[u8]) -> [u64; STATE_SIZE] {
    let mut a = state[0];
    let mut b = state[1];
    let mut c = state[2];
    let mut d = state[3];
    let mut e = state[4];
    let mut f = state[5];
    let mut g = state[6];
    let mut h = state[7];

    macro_rules! S0 {
        ($in:expr) => {
            (($in.rotate_right(5) ^ $in).rotate_right(6) ^ $in).rotate_right(28)
        }
    }

    macro_rules! S1 {
        ($in:expr) => {
            (($in.rotate_right(23) ^ $in).rotate_right(4) ^ $in).rotate_right(14)
        }
    }

    macro_rules! s0 {
        ($in:expr) => {
            ($in.rotate_right(7) ^ $in).rotate_right(1) ^ $in.wrapping_shr(7)
        }
    }

    macro_rules! s1 {
        ($in:expr) => {
            ($in.rotate_right(42) ^ $in).rotate_right(19) ^ $in.wrapping_shr(6)
        }
    }

    macro_rules! Ch {
        ($x:expr, $y:expr, $z:expr) => {
            (($z) ^ (($x) & (($y) ^ ($z))))
        }
    }

    macro_rules! Ma {
        ($x:expr, $y:expr, $z:expr) => {
            ((($x) & ($y)) | (($z) & (($x) | ($y))))
        }
    }

    const fn read_u64(input: &[u8], cursor: usize) -> u64 {
        u64::from_be_bytes([input[cursor], input[cursor + 1], input[cursor + 2], input[cursor + 3], input[cursor + 4], input[cursor + 5], input[cursor + 6], input[cursor + 7]])
    }

    let mut x = [0u64; 80];
    let mut idx = 0;
    while idx < 16 {
        x[idx] = read_u64(input, cursor + idx * 8);
        idx += 1;
    }

    while idx < x.len() {
        x[idx] = s1!(x[idx - 2]).wrapping_add(x[idx - 7]).wrapping_add(s0!(x[idx - 15])).wrapping_add(x[idx - 16]);
        idx += 1;
    }

    macro_rules! R {
        ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr, $g:expr, $h:expr, $i:expr) => {
            $h = $h.wrapping_add(S1!($e).wrapping_add(Ch!($e, $f, $g)).wrapping_add(K512[$i]).wrapping_add(x[$i]));
            $d = $d.wrapping_add($h);
            $h = $h.wrapping_add(S0!($a).wrapping_add(Ma!($a, $b, $c)));
        }
    }

    idx = 0;
    while idx < 80 {
        R!(a, b, c, d, e, f, g, h, idx);
        R!(h, a, b, c, d, e, f, g, idx+1);
        R!(g, h, a, b, c, d, e, f, idx+2);
        R!(f, g, h, a, b, c, d, e, idx+3);
        R!(e, f, g, h, a, b, c, d, idx+4);
        R!(d, e, f, g, h, a, b, c, idx+5);
        R!(c, d, e, f, g, h, a, b, idx+6);
        R!(b, c, d, e, f, g, h, a, idx+7);

        idx += 8;
    }

    [
        state[0].wrapping_add(a),
        state[1].wrapping_add(b),
        state[2].wrapping_add(c),
        state[3].wrapping_add(d),
        state[4].wrapping_add(e),
        state[5].wrapping_add(f),
        state[6].wrapping_add(g),
        state[7].wrapping_add(h),
    ]
}

#[inline]
///const `SHA512` algorithm implementation
pub const fn sha512(input: &[u8]) -> [u8; RESULT_SIZE] {
    let mut state = INIT_STATE;
    let mut cursor = 0;

    while cursor + BLOCK_SIZE <= input.len() {
        state = sha512_transform(state, cursor, input);
        cursor += BLOCK_SIZE;
    }

    let mut pos = 0;
    let mut buffer = [0; BLOCK_SIZE];

    while pos < input.len() - cursor {
        buffer[pos] = input[cursor + pos];
        pos += 1;
    }
    buffer[pos] = 0x80;
    pos += 1;

    while pos != (BLOCK_SIZE - (2 * core::mem::size_of::<u64>())) {
        pos &= BLOCK_SIZE - 1;

        if pos == 0 {
            state = sha512_transform(state, 0, &buffer);
        }

        buffer[pos] = 0;
        pos += 1;
    }

    let input_len = input.len() as u64;
    let len_lo = input_len.wrapping_shl(3).to_be_bytes();
    let len_hi = input_len.wrapping_shr(64 - 3).to_be_bytes();
    buffer[pos] = len_hi[0];
    buffer[pos + 1] = len_hi[1];
    buffer[pos + 2] = len_hi[2];
    buffer[pos + 3] = len_hi[3];
    buffer[pos + 4] = len_hi[4];
    buffer[pos + 5] = len_hi[5];
    buffer[pos + 6] = len_hi[6];
    buffer[pos + 7] = len_hi[7];

    buffer[pos + 8] = len_lo[0];
    buffer[pos + 9] = len_lo[1];
    buffer[pos + 10] = len_lo[2];
    buffer[pos + 11] = len_lo[3];
    buffer[pos + 12] = len_lo[4];
    buffer[pos + 13] = len_lo[5];
    buffer[pos + 14] = len_lo[6];
    buffer[pos + 15] = len_lo[7];

    state = sha512_transform(state, 0, &buffer);

    let a = state[0].to_be_bytes();
    let b = state[1].to_be_bytes();
    let c = state[2].to_be_bytes();
    let d = state[3].to_be_bytes();
    let e = state[4].to_be_bytes();
    let f = state[5].to_be_bytes();
    let g = state[6].to_be_bytes();
    let h = state[7].to_be_bytes();
    [
        a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7],
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7],
        d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7],
        e[0], e[1], e[2], e[3], e[4], e[5], e[6], e[7],
        f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7],
        g[0], g[1], g[2], g[3], g[4], g[5], g[6], g[7],
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7],
    ]
}

///`Sha512` algorithm implementation
pub struct Sha512 {
    state: [u64; STATE_SIZE],
    len: u64,
    buffer: [u8; BLOCK_SIZE],
}


impl Sha512 {
    ///Creates new instance
    pub const fn new() -> Self {
        Self {
            state: INIT_STATE,
            len: 0,
            buffer: [0; BLOCK_SIZE]
        }
    }

    ///Resets algorithm's state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    ///Hashes input
    pub const fn const_update(mut self, input: &[u8]) -> Self {
        let num = (self.len & (BLOCK_SIZE as u64 - 1)) as usize;
        self.len += input.len() as u64;

        let mut cursor = 0;

        if num > 0 {
            let block_num = BLOCK_SIZE - num;

            if input.len() < block_num {
                let mut idx = 0;
                while idx < input.len() {
                    self.buffer[num + idx] = input[idx];
                    idx += 1;
                }
                return self;
            }

            let mut idx = 0;
            while idx < block_num {
                self.buffer[num + idx] = input[idx];
                idx += 1;
            }
            self.state = sha512_transform(self.state, 0, &self.buffer);
            cursor += block_num
        }

        while input.len() - cursor >= BLOCK_SIZE {
            self.state = sha512_transform(self.state, cursor, input);
            cursor += BLOCK_SIZE;
        }

        let remains = input.len() - cursor;
        let mut idx = 0;
        while idx < remains {
            self.buffer[idx] = input[cursor + idx];
            idx += 1;
        }

        self
    }

    ///Hashes input
    pub fn update(&mut self, input: &[u8]) {
        let mut num = (self.len & (BLOCK_SIZE as u64 - 1)) as usize;
        self.len += input.len() as u64;

        let mut cursor = 0;

        if num > 0 {
            let buffer = &mut self.buffer[num..];
            num = BLOCK_SIZE - num;

            if input.len() < num {
                buffer[..input.len()].copy_from_slice(input);
                return;
            }

            buffer.copy_from_slice(&input[..num]);
            self.state = sha512_transform(self.state, 0, &self.buffer);
            cursor += num;
        }

        while input.len() - cursor >= BLOCK_SIZE {
            self.state = sha512_transform(self.state, cursor, input);
            cursor += BLOCK_SIZE;
        }

        let remains = input.len() - cursor;
        if remains > 0 {
            self.buffer[..remains].copy_from_slice(&input[cursor..]);
        }
    }

    ///Finalizes algorithm, returning the hash.
    pub const fn const_result(mut self) -> [u8; RESULT_SIZE] {
        let mut pos = (self.len & (BLOCK_SIZE as u64 - 1)) as usize;

        self.buffer[pos] = 0x80;
        pos += 1;

        while pos != (BLOCK_SIZE - (2 * core::mem::size_of::<u64>())) {
            pos &= BLOCK_SIZE - 1;

            if pos == 0 {
                self.state = sha512_transform(self.state, 0, &self.buffer);
            }

            self.buffer[pos] = 0;
            pos += 1;
        }

        let len_lo = self.len.wrapping_shl(3).to_be_bytes();
        let len_hi = self.len.wrapping_shr(64 - 3).to_be_bytes();

        self.buffer[pos] = len_hi[0];
        self.buffer[pos + 1] = len_hi[1];
        self.buffer[pos + 2] = len_hi[2];
        self.buffer[pos + 3] = len_hi[3];
        self.buffer[pos + 4] = len_hi[4];
        self.buffer[pos + 5] = len_hi[5];
        self.buffer[pos + 6] = len_hi[6];
        self.buffer[pos + 7] = len_hi[7];

        self.buffer[pos + 8] = len_lo[0];
        self.buffer[pos + 9] = len_lo[1];
        self.buffer[pos + 10] = len_lo[2];
        self.buffer[pos + 11] = len_lo[3];
        self.buffer[pos + 12] = len_lo[4];
        self.buffer[pos + 13] = len_lo[5];
        self.buffer[pos + 14] = len_lo[6];
        self.buffer[pos + 15] = len_lo[7];

        self.state = sha512_transform(self.state, 0, &self.buffer);

        let a = self.state[0].to_be_bytes();
        let b = self.state[1].to_be_bytes();
        let c = self.state[2].to_be_bytes();
        let d = self.state[3].to_be_bytes();
        let e = self.state[4].to_be_bytes();
        let f = self.state[5].to_be_bytes();
        let g = self.state[6].to_be_bytes();
        let h = self.state[7].to_be_bytes();
        [
            a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7],
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
            c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7],
            d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7],
            e[0], e[1], e[2], e[3], e[4], e[5], e[6], e[7],
            f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7],
            g[0], g[1], g[2], g[3], g[4], g[5], g[6], g[7],
            h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7],
        ]
    }

    ///Finalizes algorithm, returning the hash.
    pub fn result(&mut self) -> [u8; RESULT_SIZE] {
        let mut pos = (self.len & (BLOCK_SIZE as u64 - 1)) as usize;

        self.buffer[pos] = 0x80;
        pos += 1;

        while pos != (BLOCK_SIZE - (2 * core::mem::size_of::<u64>())) {
            pos &= BLOCK_SIZE - 1;

            if pos == 0 {
                self.state = sha512_transform(self.state, 0, &self.buffer);
            }

            self.buffer[pos] = 0;
            pos += 1;
        }

        let len_lo = self.len.wrapping_shl(3).to_be_bytes();
        let len_hi = self.len.wrapping_shr(64 - 3).to_be_bytes();

        self.buffer[pos] = len_hi[0];
        self.buffer[pos + 1] = len_hi[1];
        self.buffer[pos + 2] = len_hi[2];
        self.buffer[pos + 3] = len_hi[3];
        self.buffer[pos + 4] = len_hi[4];
        self.buffer[pos + 5] = len_hi[5];
        self.buffer[pos + 6] = len_hi[6];
        self.buffer[pos + 7] = len_hi[7];

        self.buffer[pos + 8] = len_lo[0];
        self.buffer[pos + 9] = len_lo[1];
        self.buffer[pos + 10] = len_lo[2];
        self.buffer[pos + 11] = len_lo[3];
        self.buffer[pos + 12] = len_lo[4];
        self.buffer[pos + 13] = len_lo[5];
        self.buffer[pos + 14] = len_lo[6];
        self.buffer[pos + 15] = len_lo[7];

        self.state = sha512_transform(self.state, 0, &self.buffer);

        let a = self.state[0].to_be_bytes();
        let b = self.state[1].to_be_bytes();
        let c = self.state[2].to_be_bytes();
        let d = self.state[3].to_be_bytes();
        let e = self.state[4].to_be_bytes();
        let f = self.state[5].to_be_bytes();
        let g = self.state[6].to_be_bytes();
        let h = self.state[7].to_be_bytes();
        [
            a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7],
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
            c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7],
            d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7],
            e[0], e[1], e[2], e[3], e[4], e[5], e[6], e[7],
            f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7],
            g[0], g[1], g[2], g[3], g[4], g[5], g[6], g[7],
            h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7],
        ]
    }
}

impl super::Digest for Sha512 {
    type OutputType = [u8; RESULT_SIZE];
    type BlockType = [u8; BLOCK_SIZE];

    #[inline(always)]
    fn new() -> Self {
        Self::new()
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.reset();
    }

    #[inline(always)]
    fn update(&mut self, input: &[u8]) {
        self.update(input);
    }

    #[inline(always)]
    fn result(&mut self) -> Self::OutputType {
        self.result()
    }
}
