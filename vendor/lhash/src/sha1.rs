const BLOCK_SIZE: usize = 64;
const STATE_SIZE: usize = 5;
const RESULT_SIZE: usize = 20;
const INIT_STATE: [u32; STATE_SIZE] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0];

const K1: u32 = 0x5a827999;
const K2: u32 = 0x6ed9eba1;
const K3: u32 = 0x8f1bbcdc;
const K4: u32 = 0xca62c1d6;

#[inline(always)]
const fn f1(x: u32, y: u32, z: u32) -> u32 {
    z ^ (x & (y ^ z))
}

#[inline(always)]
const fn f2(x: u32, y: u32, z: u32) -> u32 {
    x ^ y ^ z
}

#[inline(always)]
const fn f3(x: u32, y: u32, z: u32) -> u32 {
    (x & y) | (z & (x | y))
}

#[inline(always)]
const fn f4(x: u32, y: u32, z: u32) -> u32 {
    x ^ y ^ z
}

macro_rules! M {
    ($x:expr, $idx:expr, $tm:expr) => {{
        $tm = $x[$idx & 0x0f] ^ $x[($idx-14) & 0x0f] ^ $x[($idx-8) & 0x0f] ^ $x[($idx-3) & 0x0f];
        $x[$idx & 0x0f] = $tm.rotate_left(1);
        $x[$idx & 0x0f]
    }}
}

macro_rules! step {
    ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr, $k:expr, $m:expr) => {{
        $e = $e.wrapping_add($a.rotate_left(5).wrapping_add($f($b, $c, $d)).wrapping_add($k).wrapping_add($m));
        $b = $b.rotate_left(30);
    }}
}

const fn sha1_transform(state: [u32; STATE_SIZE], cursor: usize, input: &[u8]) -> [u32; STATE_SIZE] {
    let mut a = state[0];
    let mut b = state[1];
    let mut c = state[2];
    let mut d = state[3];
    let mut e = state[4];

    let mut tm;

    let mut x = [
        u32::from_be_bytes([input[cursor], input[cursor + 1], input[cursor + 2], input[cursor + 3]]),
        u32::from_be_bytes([input[cursor + 4], input[cursor + 5], input[cursor + 6], input[cursor + 7]]),
        u32::from_be_bytes([input[cursor + 8], input[cursor + 9], input[cursor + 10], input[cursor + 11]]),
        u32::from_be_bytes([input[cursor + 12], input[cursor + 13], input[cursor + 14], input[cursor + 15]]),
        u32::from_be_bytes([input[cursor + 16], input[cursor + 17], input[cursor + 18], input[cursor + 19]]),
        u32::from_be_bytes([input[cursor + 20], input[cursor + 21], input[cursor + 22], input[cursor + 23]]),
        u32::from_be_bytes([input[cursor + 24], input[cursor + 25], input[cursor + 26], input[cursor + 27]]),
        u32::from_be_bytes([input[cursor + 28], input[cursor + 29], input[cursor + 30], input[cursor + 31]]),
        u32::from_be_bytes([input[cursor + 32], input[cursor + 33], input[cursor + 34], input[cursor + 35]]),
        u32::from_be_bytes([input[cursor + 36], input[cursor + 37], input[cursor + 38], input[cursor + 39]]),
        u32::from_be_bytes([input[cursor + 40], input[cursor + 41], input[cursor + 42], input[cursor + 43]]),
        u32::from_be_bytes([input[cursor + 44], input[cursor + 45], input[cursor + 46], input[cursor + 47]]),
        u32::from_be_bytes([input[cursor + 48], input[cursor + 49], input[cursor + 50], input[cursor + 51]]),
        u32::from_be_bytes([input[cursor + 52], input[cursor + 53], input[cursor + 54], input[cursor + 55]]),
        u32::from_be_bytes([input[cursor + 56], input[cursor + 57], input[cursor + 58], input[cursor + 59]]),
        u32::from_be_bytes([input[cursor + 60], input[cursor + 61], input[cursor + 62], input[cursor + 63]]),
    ];

    step!(a, b, c, d, e, f1, K1, x[0]);
    step!(e, a, b, c, d, f1, K1, x[1]);
    step!(d, e, a, b, c, f1, K1, x[2]);
    step!(c, d, e, a, b, f1, K1, x[3]);
    step!(b, c, d, e, a, f1, K1, x[4]);
    step!(a, b, c, d, e, f1, K1, x[5]);
    step!(e, a, b, c, d, f1, K1, x[6]);
    step!(d, e, a, b, c, f1, K1, x[7]);
    step!(c, d, e, a, b, f1, K1, x[8]);
    step!(b, c, d, e, a, f1, K1, x[9]);
    step!(a, b, c, d, e, f1, K1, x[10]);
    step!(e, a, b, c, d, f1, K1, x[11]);
    step!(d, e, a, b, c, f1, K1, x[12]);
    step!(c, d, e, a, b, f1, K1, x[13]);
    step!(b, c, d, e, a, f1, K1, x[14]);
    step!(a, b, c, d, e, f1, K1, x[15]);
    step!(e, a, b, c, d, f1, K1, M!(x, 16, tm));
    step!(d, e, a, b, c, f1, K1, M!(x, 17, tm));
    step!(c, d, e, a, b, f1, K1, M!(x, 18, tm));
    step!(b, c, d, e, a, f1, K1, M!(x, 19, tm));
    step!(a, b, c, d, e, f2, K2, M!(x, 20, tm));
    step!(e, a, b, c, d, f2, K2, M!(x, 21, tm));
    step!(d, e, a, b, c, f2, K2, M!(x, 22, tm));
    step!(c, d, e, a, b, f2, K2, M!(x, 23, tm));
    step!(b, c, d, e, a, f2, K2, M!(x, 24, tm));
    step!(a, b, c, d, e, f2, K2, M!(x, 25, tm));
    step!(e, a, b, c, d, f2, K2, M!(x, 26, tm));
    step!(d, e, a, b, c, f2, K2, M!(x, 27, tm));
    step!(c, d, e, a, b, f2, K2, M!(x, 28, tm));
    step!(b, c, d, e, a, f2, K2, M!(x, 29, tm));
    step!(a, b, c, d, e, f2, K2, M!(x, 30, tm));
    step!(e, a, b, c, d, f2, K2, M!(x, 31, tm));
    step!(d, e, a, b, c, f2, K2, M!(x, 32, tm));
    step!(c, d, e, a, b, f2, K2, M!(x, 33, tm));
    step!(b, c, d, e, a, f2, K2, M!(x, 34, tm));
    step!(a, b, c, d, e, f2, K2, M!(x, 35, tm));
    step!(e, a, b, c, d, f2, K2, M!(x, 36, tm));
    step!(d, e, a, b, c, f2, K2, M!(x, 37, tm));
    step!(c, d, e, a, b, f2, K2, M!(x, 38, tm));
    step!(b, c, d, e, a, f2, K2, M!(x, 39, tm));
    step!(a, b, c, d, e, f3, K3, M!(x, 40, tm));
    step!(e, a, b, c, d, f3, K3, M!(x, 41, tm));
    step!(d, e, a, b, c, f3, K3, M!(x, 42, tm));
    step!(c, d, e, a, b, f3, K3, M!(x, 43, tm));
    step!(b, c, d, e, a, f3, K3, M!(x, 44, tm));
    step!(a, b, c, d, e, f3, K3, M!(x, 45, tm));
    step!(e, a, b, c, d, f3, K3, M!(x, 46, tm));
    step!(d, e, a, b, c, f3, K3, M!(x, 47, tm));
    step!(c, d, e, a, b, f3, K3, M!(x, 48, tm));
    step!(b, c, d, e, a, f3, K3, M!(x, 49, tm));
    step!(a, b, c, d, e, f3, K3, M!(x, 50, tm));
    step!(e, a, b, c, d, f3, K3, M!(x, 51, tm));
    step!(d, e, a, b, c, f3, K3, M!(x, 52, tm));
    step!(c, d, e, a, b, f3, K3, M!(x, 53, tm));
    step!(b, c, d, e, a, f3, K3, M!(x, 54, tm));
    step!(a, b, c, d, e, f3, K3, M!(x, 55, tm));
    step!(e, a, b, c, d, f3, K3, M!(x, 56, tm));
    step!(d, e, a, b, c, f3, K3, M!(x, 57, tm));
    step!(c, d, e, a, b, f3, K3, M!(x, 58, tm));
    step!(b, c, d, e, a, f3, K3, M!(x, 59, tm));
    step!(a, b, c, d, e, f4, K4, M!(x, 60, tm));
    step!(e, a, b, c, d, f4, K4, M!(x, 61, tm));
    step!(d, e, a, b, c, f4, K4, M!(x, 62, tm));
    step!(c, d, e, a, b, f4, K4, M!(x, 63, tm));
    step!(b, c, d, e, a, f4, K4, M!(x, 64, tm));
    step!(a, b, c, d, e, f4, K4, M!(x, 65, tm));
    step!(e, a, b, c, d, f4, K4, M!(x, 66, tm));
    step!(d, e, a, b, c, f4, K4, M!(x, 67, tm));
    step!(c, d, e, a, b, f4, K4, M!(x, 68, tm));
    step!(b, c, d, e, a, f4, K4, M!(x, 69, tm));
    step!(a, b, c, d, e, f4, K4, M!(x, 70, tm));
    step!(e, a, b, c, d, f4, K4, M!(x, 71, tm));
    step!(d, e, a, b, c, f4, K4, M!(x, 72, tm));
    step!(c, d, e, a, b, f4, K4, M!(x, 73, tm));
    step!(b, c, d, e, a, f4, K4, M!(x, 74, tm));
    step!(a, b, c, d, e, f4, K4, M!(x, 75, tm));
    step!(e, a, b, c, d, f4, K4, M!(x, 76, tm));
    step!(d, e, a, b, c, f4, K4, M!(x, 77, tm));
    step!(c, d, e, a, b, f4, K4, M!(x, 78, tm));
    step!(b, c, d, e, a, f4, K4, M!(x, 79, tm));

    [
        state[0].wrapping_add(a),
        state[1].wrapping_add(b),
        state[2].wrapping_add(c),
        state[3].wrapping_add(d),
        state[4].wrapping_add(e),
    ]
}

///const `SHA1` algorithm implementation
pub const fn sha1(input: &[u8]) -> [u8; RESULT_SIZE] {
    let mut state = INIT_STATE;
    let mut cursor = 0;

    while cursor + 64 <= input.len() {
        state = sha1_transform(state, cursor, input);
        cursor += 64;
    }

    let mut pos = 0;
    let mut buffer = [0; BLOCK_SIZE];

    while pos < input.len() - cursor {
        buffer[pos] = input[cursor + pos];
        pos += 1;
    }
    buffer[pos] = 0x80;
    pos += 1;

    while pos != (BLOCK_SIZE - core::mem::size_of::<u64>()) {
        pos &= BLOCK_SIZE - 1;

        if pos == 0 {
            state = sha1_transform(state, 0, &buffer);
        }

        buffer[pos] = 0;
        pos += 1;
    }

    let len = (input.len() as u64).wrapping_shl(3).to_be_bytes();
    buffer[pos] = len[0];
    buffer[pos + 1] = len[1];
    buffer[pos + 2] = len[2];
    buffer[pos + 3] = len[3];
    buffer[pos + 4] = len[4];
    buffer[pos + 5] = len[5];
    buffer[pos + 6] = len[6];
    buffer[pos + 7] = len[7];

    state = sha1_transform(state, 0, &buffer);

    let a = state[0].to_be_bytes();
    let b = state[1].to_be_bytes();
    let c = state[2].to_be_bytes();
    let d = state[3].to_be_bytes();
    let e = state[4].to_be_bytes();
    [
        a[0], a[1], a[2], a[3],
        b[0], b[1], b[2], b[3],
        c[0], c[1], c[2], c[3],
        d[0], d[1], d[2], d[3],
        e[0], e[1], e[2], e[3],
    ]
}

///`Sha1` algorithm implementation
pub struct Sha1 {
    state: [u32; STATE_SIZE],
    len: u64,
    buffer: [u8; BLOCK_SIZE],
}

impl Sha1 {
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
            self.state = sha1_transform(self.state, 0, &self.buffer);
            cursor += block_num
        }

        while input.len() - cursor >= BLOCK_SIZE {
            self.state = sha1_transform(self.state, cursor, input);
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
            self.state = sha1_transform(self.state, 0, &self.buffer);
            cursor += num;
        }

        while input.len() - cursor >= BLOCK_SIZE {
            self.state = sha1_transform(self.state, cursor, input);
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

        while pos != (BLOCK_SIZE - core::mem::size_of::<u64>()) {
            pos &= BLOCK_SIZE - 1;

            if pos == 0 {
                self.state = sha1_transform(self.state, 0, &self.buffer);
            }

            self.buffer[pos] = 0;
            pos += 1;
        }

        let len = self.len.wrapping_shl(3).to_be_bytes();
        self.buffer[pos] = len[0];
        self.buffer[pos + 1] = len[1];
        self.buffer[pos + 2] = len[2];
        self.buffer[pos + 3] = len[3];
        self.buffer[pos + 4] = len[4];
        self.buffer[pos + 5] = len[5];
        self.buffer[pos + 6] = len[6];
        self.buffer[pos + 7] = len[7];

        self.state = sha1_transform(self.state, 0, &self.buffer);

        let a = self.state[0].to_be_bytes();
        let b = self.state[1].to_be_bytes();
        let c = self.state[2].to_be_bytes();
        let d = self.state[3].to_be_bytes();
        let e = self.state[4].to_be_bytes();
        [
            a[0], a[1], a[2], a[3],
            b[0], b[1], b[2], b[3],
            c[0], c[1], c[2], c[3],
            d[0], d[1], d[2], d[3],
            e[0], e[1], e[2], e[3],
        ]
    }

    ///Finalizes algorithm, returning the hash.
    pub fn result(&mut self) -> [u8; RESULT_SIZE] {
        let mut pos = (self.len & (BLOCK_SIZE as u64 - 1)) as usize;

        self.buffer[pos] = 0x80;
        pos += 1;

        while pos != (BLOCK_SIZE - core::mem::size_of::<u64>()) {
            pos &= BLOCK_SIZE - 1;

            if pos == 0 {
                self.state = sha1_transform(self.state, 0, &self.buffer);
            }

            self.buffer[pos] = 0;
            pos += 1;
        }

        let len = self.len.wrapping_shl(3).to_be_bytes();
        self.buffer[pos] = len[0];
        self.buffer[pos + 1] = len[1];
        self.buffer[pos + 2] = len[2];
        self.buffer[pos + 3] = len[3];
        self.buffer[pos + 4] = len[4];
        self.buffer[pos + 5] = len[5];
        self.buffer[pos + 6] = len[6];
        self.buffer[pos + 7] = len[7];

        self.state = sha1_transform(self.state, 0, &self.buffer);

        let a = self.state[0].to_be_bytes();
        let b = self.state[1].to_be_bytes();
        let c = self.state[2].to_be_bytes();
        let d = self.state[3].to_be_bytes();
        let e = self.state[4].to_be_bytes();
        [
            a[0], a[1], a[2], a[3],
            b[0], b[1], b[2], b[3],
            c[0], c[1], c[2], c[3],
            d[0], d[1], d[2], d[3],
            e[0], e[1], e[2], e[3],
        ]
    }
}

impl super::Digest for Sha1 {
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
