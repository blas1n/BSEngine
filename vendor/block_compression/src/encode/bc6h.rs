use super::common::*;
use crate::BC6HSettings;

pub(crate) struct BlockCompressorBC6H<'a> {
    block: [f32; 64],
    data: [u32; 5],
    best_err: f32,

    rgb_bounds: [f32; 6],
    max_span: f32,
    max_span_idx: usize,

    mode: usize,
    epb: u32,
    qbounds: [i32; 8],
    settings: &'a BC6HSettings,
}

#[inline(always)]
pub fn srgb_to_linear(srgb: u8) -> f32 {
    let v = (srgb as f32) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

impl<'a> BlockCompressorBC6H<'a> {
    pub(crate) fn new(settings: &'a BC6HSettings) -> Self {
        Self {
            block: [0.0; 64],
            data: [0; 5],
            best_err: f32::INFINITY,
            rgb_bounds: [0.0; 6],
            max_span: 0.0,
            max_span_idx: 0,
            mode: 0,
            epb: 0,
            qbounds: [0; 8],
            settings,
        }
    }

    pub(crate) fn load_block_interleaved_8bit(
        &mut self,
        rgba_data: &[u8],
        xx: usize,
        yy: usize,
        stride: usize,
    ) {
        for y in 0..4 {
            for x in 0..4 {
                let pixel_x = xx * 4 + x;
                let pixel_y = yy * 4 + y;

                let offset = pixel_y * stride + pixel_x * 4;

                let red = half::f16::from_f32(srgb_to_linear(rgba_data[offset])).to_bits() as f32;
                let green =
                    half::f16::from_f32(srgb_to_linear(rgba_data[offset + 1])).to_bits() as f32;
                let blue =
                    half::f16::from_f32(srgb_to_linear(rgba_data[offset + 2])).to_bits() as f32;

                self.block[y * 4 + x] = red;
                self.block[16 + y * 4 + x] = green;
                self.block[32 + y * 4 + x] = blue;
                self.block[48 + y * 4 + x] = 0.0;
            }
        }
    }

    pub(crate) fn load_block_interleaved_16bit(
        &mut self,
        rgba_data: &[half::f16],
        xx: usize,
        yy: usize,
        stride: usize,
    ) {
        for y in 0..4 {
            for x in 0..4 {
                let pixel_x = xx * 4 + x;
                let pixel_y = yy * 4 + y;

                let offset = pixel_y * stride + pixel_x * 4;

                let red = rgba_data[offset].to_bits() as f32;
                let green = rgba_data[offset + 1].to_bits() as f32;
                let blue = rgba_data[offset + 2].to_bits() as f32;

                self.block[y * 4 + x] = red;
                self.block[16 + y * 4 + x] = green;
                self.block[32 + y * 4 + x] = blue;
                self.block[48 + y * 4 + x] = 0.0;
            }
        }
    }

    pub(crate) fn store_data(
        &self,
        blocks_buffer: &mut [u8],
        block_width: usize,
        xx: usize,
        yy: usize,
    ) {
        let offset = (yy * block_width + xx) * 16;

        for (index, &value) in self.data[..4].iter().enumerate() {
            let byte_offset = offset + index * 4;
            blocks_buffer[byte_offset] = value as u8;
            blocks_buffer[byte_offset + 1] = (value >> 8) as u8;
            blocks_buffer[byte_offset + 2] = (value >> 16) as u8;
            blocks_buffer[byte_offset + 3] = (value >> 24) as u8;
        }
    }

    fn get_mode_prefix(mode: usize) -> u32 {
        const MODE_PREFIX_TABLE: [u32; 14] = [0, 1, 2, 6, 10, 14, 18, 22, 26, 30, 3, 7, 11, 15];

        MODE_PREFIX_TABLE[mode]
    }

    fn get_span(mode: usize) -> f32 {
        const SPAN_TABLE: [f32; 14] = [
            0.9 * 65535.0 / 64.0,  // (0) 4 / 10
            0.9 * 65535.0 / 4.0,   // (1) 5 / 7
            0.8 * 65535.0 / 256.0, // (2) 3 / 11
            -1.0,
            -1.0,
            0.9 * 65535.0 / 32.0, // (5) 4 / 9
            0.9 * 65535.0 / 16.0, // (6) 4 / 8
            -1.0,
            -1.0,
            65535.0,               // (9) absolute
            65535.0,               // (10) absolute
            0.95 * 65535.0 / 8.0,  // (11) 8 / 11
            0.95 * 65535.0 / 32.0, // (12) 7 / 12
            6.0,                   // (13) 3 / 16
        ];

        SPAN_TABLE[mode]
    }

    fn get_mode_bits(mode: usize) -> u32 {
        const MODE_BITS_TABLE: [u32; 14] = [
            10, 7, 11, 0xFFFFFFFF, 0xFFFFFFFF, 9, 8, 0xFFFFFFFF, 0xFFFFFFFF, 6, 10, 11, 12, 16,
        ];

        MODE_BITS_TABLE[mode]
    }

    fn ep_quant_bc6h_8(&mut self, ep: &[f32; 8], bits: u32, pairs: u32) {
        let levels = 1 << bits;

        for i in 0..8 * pairs as usize {
            let v = (ep[i] / (256.0 * 256.0 - 1.0) * (levels - 1) as f32 + 0.5) as i32;
            self.qbounds[i] = i32::clamp(v, 0, levels - 1);
        }
    }

    fn compute_qbounds_core(&mut self, rgb_span: [f32; 3]) {
        let mut bounds = [0.0; 8];

        for p in 0..3 {
            let middle = (self.rgb_bounds[p] + self.rgb_bounds[3 + p]) / 2.0;
            bounds[p] = middle - rgb_span[p] / 2.0;
            bounds[4 + p] = middle + rgb_span[p] / 2.0;
        }

        self.ep_quant_bc6h_8(&bounds, self.epb, 1);
    }

    fn compute_qbounds(&mut self, span: f32) {
        self.compute_qbounds_core([span, span, span]);
    }

    fn compute_qbounds2(&mut self, span: f32, max_span_idx: usize) {
        let mut rgb_span = [span, span, span];
        if max_span_idx < 3 {
            rgb_span[max_span_idx] *= 2.0;
        }
        self.compute_qbounds_core(rgb_span);
    }

    fn unpack_to_uf16(v: u32, bits: u32) -> u32 {
        if bits >= 15 {
            return v;
        }
        if v == 0 {
            return 0;
        }
        if v == (1 << bits) - 1 {
            return 0xFFFF;
        }

        (v * 2 + 1) << (15 - bits)
    }

    fn ep_quant_bc6h(qep: &mut [i32; 24], ep: &[f32; 24], bits: u32, pairs: usize) {
        let levels = 1 << bits;

        for i in 0..8 * pairs {
            let v = (ep[i] / (256.0 * 256.0 - 1.0) * (levels - 1) as f32 + 0.5) as i32;
            qep[i] = i32::clamp(v, 0, levels - 1);
        }
    }

    fn ep_dequant_bc6h(ep: &mut [f32; 24], qep: &[i32; 24], bits: u32, pairs: usize) {
        for i in 0..8 * pairs {
            ep[i] = Self::unpack_to_uf16(qep[i] as u32, bits) as f32;
        }
    }

    fn ep_quant_dequant_bc6h(&self, qep: &mut [i32; 24], ep: &mut [f32; 24], pairs: usize) {
        let bits = self.epb;
        Self::ep_quant_bc6h(qep, ep, bits, pairs);

        for i in 0..2 * pairs {
            for p in 0..3 {
                qep[i * 4 + p] = i32::clamp(qep[i * 4 + p], self.qbounds[p], self.qbounds[4 + p]);
            }
        }

        Self::ep_dequant_bc6h(ep, qep, bits, pairs);
    }

    fn bc6h_code_2p(&mut self, qep: &mut [i32; 24], qblock: [u32; 2], part_id: i32, mode: usize) {
        let bits = 3;

        let flips = bc7_code_apply_swap_mode01237(qep, qblock, 1, part_id);

        self.data = [0; 5];
        let mut pos = 0;

        let mut packed = [0; 4];
        Self::bc6h_pack(&mut packed, qep, mode);

        // Mode
        put_bits(&mut self.data, &mut pos, 5, packed[0]);

        // Endpoints
        put_bits(&mut self.data, &mut pos, 30, packed[1]);
        put_bits(&mut self.data, &mut pos, 30, packed[2]);
        put_bits(&mut self.data, &mut pos, 12, packed[3]);

        // Partition
        put_bits(&mut self.data, &mut pos, 5, part_id as u32);

        // Quantized values
        bc7_code_qblock(&mut self.data, &mut pos, qblock, bits, flips);
        bc7_code_adjust_skip_mode01237(&mut self.data, 1, part_id);
    }

    fn bc6h_code_1p(&mut self, qep: &mut [i32; 24], qblock: &mut [u32; 2], mode: usize) {
        bc7_code_apply_swap_mode456(qep, 4, qblock, 4);

        self.data = [0; 5];
        let mut pos = 0;

        let mut packed = [0; 4];
        Self::bc6h_pack(&mut packed, qep, mode);

        // Mode
        put_bits(&mut self.data, &mut pos, 5, packed[0]);

        // Endpoints
        put_bits(&mut self.data, &mut pos, 30, packed[1]);
        put_bits(&mut self.data, &mut pos, 30, packed[2]);

        // Quantized values
        bc7_code_qblock(&mut self.data, &mut pos, *qblock, 4, 0);
    }

    fn bc6h_enc_2p(&mut self) {
        let mut full_stats = [0.0; 15];
        compute_stats_masked(&mut full_stats, &self.block, 0xFFFFFFFF, 3);

        let mut part_list = [0; 32];
        for part in 0..32 {
            let mask = get_pattern_mask(part, 0);
            let bound12 = block_pca_bound_split(&self.block, mask, full_stats, 3);
            let bound = bound12 as i32;
            part_list[part as usize] = part + bound * 64;
        }

        partial_sort_list(&mut part_list, 32, self.settings.fast_skip_threshold);
        self.bc6h_enc_2p_list(&part_list, self.settings.fast_skip_threshold);
    }

    fn bc6h_enc_2p_part_fast(
        &self,
        qep: &mut [i32; 24],
        qblock: &mut [u32; 2],
        part_id: i32,
    ) -> f32 {
        let pattern = get_pattern(part_id);
        let bits = 3;
        let pairs = 2;
        let channels = 3;

        let mut ep = [0.0; 24];
        for j in 0..pairs as usize {
            let mask = get_pattern_mask(part_id, j as u32);
            block_segment_core(&mut ep[j * 8..], &self.block, mask, channels);
        }

        self.ep_quant_dequant_bc6h(qep, &mut ep, 2);

        block_quant(qblock, &self.block, bits, &ep, pattern, channels)
    }

    fn bc6h_enc_2p_list(&mut self, part_list: &[i32; 32], part_count: u32) {
        if part_count == 0 {
            return;
        }

        let bits = 3;
        let pairs = 2;
        let channels = 3;

        let mut best_qep = [0; 24];
        let mut best_qblock = [0; 2];
        let mut best_part_id = -1;
        let mut best_err = f32::INFINITY;

        for part in 0..part_count as usize {
            let part_id = (*part_list)[part] & 31;

            let mut qep = [0; 24];
            let mut qblock = [0; 2];
            let err = self.bc6h_enc_2p_part_fast(&mut qep, &mut qblock, part_id);

            if err < best_err {
                best_qep[..(8 * pairs)].copy_from_slice(&qep[..(8 * pairs)]);
                best_qblock.copy_from_slice(&qblock);
                best_part_id = part_id;
                best_err = err;
            }
        }

        // Refine
        for _ in 0..self.settings.refine_iterations_2p {
            let mut ep = [0.0; 24];
            for j in 0..pairs {
                let mask = get_pattern_mask(best_part_id, j as u32);
                opt_endpoints(
                    &mut ep[j * 8..],
                    &self.block,
                    bits,
                    best_qblock,
                    mask,
                    channels,
                );
            }

            let mut qep = [0; 24];
            let mut qblock = [0; 2];
            self.ep_quant_dequant_bc6h(&mut qep, &mut ep, 2);

            let pattern = get_pattern(best_part_id);
            let err = block_quant(&mut qblock, &self.block, bits, &ep, pattern, channels);

            if err < best_err {
                best_qep[..(8 * pairs)].copy_from_slice(&qep[..(8 * pairs)]);
                best_qblock.copy_from_slice(&qblock);
                best_err = err;
            }
        }

        if best_err < self.best_err {
            self.best_err = best_err;
            self.bc6h_code_2p(&mut best_qep, best_qblock, best_part_id, self.mode);
        }
    }

    fn bc6h_enc_1p(&mut self) {
        let mut ep = [0.0; 24];
        block_segment_core(&mut ep, &self.block, 0xFFFFFFFF, 3);

        let mut qep = [0; 24];
        self.ep_quant_dequant_bc6h(&mut qep, &mut ep, 1);

        let mut qblock = [0; 2];
        let mut err = block_quant(&mut qblock, &self.block, 4, &ep, 0, 3);

        // Refine
        let refine_iterations = self.settings.refine_iterations_1p;
        for _ in 0..refine_iterations {
            opt_endpoints(&mut ep, &self.block, 4, qblock, 0xFFFFFFFF, 3);
            self.ep_quant_dequant_bc6h(&mut qep, &mut ep, 1);
            err = block_quant(&mut qblock, &self.block, 4, &ep, 0, 3);
        }

        if err < self.best_err {
            self.best_err = err;
            self.bc6h_code_1p(&mut qep, &mut qblock, self.mode);
        }
    }

    fn bc6h_test_mode(&mut self, mode: usize, enc: bool, margin: f32) {
        let mode_bits = Self::get_mode_bits(mode);
        let span = Self::get_span(mode);
        let max_span = self.max_span;
        let max_span_idx = self.max_span_idx;

        if max_span * margin > span {
            return;
        }

        if mode >= 10 {
            self.epb = mode_bits;
            self.mode = mode;
            self.compute_qbounds(span);
            if enc {
                self.bc6h_enc_1p();
            }
        } else if mode <= 1 || mode == 5 || mode == 9 {
            self.epb = mode_bits;
            self.mode = mode;
            self.compute_qbounds(span);
            if enc {
                self.bc6h_enc_2p();
            }
        } else {
            self.epb = mode_bits;
            self.mode = mode + max_span_idx;
            self.compute_qbounds2(span, max_span_idx);
            if enc {
                self.bc6h_enc_2p();
            }
        }
    }

    fn bit_at(v: i32, pos: u32) -> u32 {
        ((v >> pos) & 1) as u32
    }

    fn reverse_bits(v: u32, bits: u32) -> u32 {
        if bits == 2 {
            return (v >> 1) + (v & 1) * 2;
        }

        if bits == 6 {
            let vv = (v & 0x5555) * 2 + ((v >> 1) & 0x5555);
            return (vv >> 4) + ((vv >> 2) & 3) * 4 + (vv & 3) * 16;
        }

        panic!("should never happen")
    }

    fn bc6h_pack(packed: &mut [u32; 4], qep: &[i32; 24], mode: usize) {
        if mode == 0 {
            let mut pred_qep = [0; 16];
            for p in 0..3 {
                pred_qep[p] = qep[p];
                pred_qep[4 + p] = (qep[4 + p] - qep[p]) & 31;
                pred_qep[8 + p] = (qep[8 + p] - qep[p]) & 31;
                pred_qep[12 + p] = (qep[12 + p] - qep[p]) & 31;
            }

            let mut pqep = [0; 10];

            pqep[4] = pred_qep[4] as u32 + (pred_qep[8 + 1] & 15) as u32 * 64;
            pqep[5] = pred_qep[5] as u32 + (pred_qep[12 + 1] & 15) as u32 * 64;
            pqep[6] = pred_qep[6] as u32 + (pred_qep[8 + 2] & 15) as u32 * 64;

            pqep[4] += Self::bit_at(pred_qep[12 + 1], 4) << 5;
            pqep[5] += Self::bit_at(pred_qep[12 + 2], 0) << 5;
            pqep[6] += Self::bit_at(pred_qep[12 + 2], 1) << 5;

            pqep[8] = pred_qep[8] as u32 + Self::bit_at(pred_qep[12 + 2], 2) * 32;
            pqep[9] = pred_qep[12] as u32 + Self::bit_at(pred_qep[12 + 2], 3) * 32;

            packed[0] = Self::get_mode_prefix(0);
            packed[0] += Self::bit_at(pred_qep[8 + 1], 4) << 2;
            packed[0] += Self::bit_at(pred_qep[8 + 2], 4) << 3;
            packed[0] += Self::bit_at(pred_qep[12 + 2], 4) << 4;

            packed[1] =
                ((pred_qep[2] as u32) << 20) + ((pred_qep[1] as u32) << 10) + pred_qep[0] as u32;
            packed[2] = (pqep[6] << 20) + (pqep[5] << 10) + pqep[4];
            packed[3] = (pqep[9] << 6) + pqep[8];
        } else if mode == 1 {
            let mut pred_qep = [0; 16];
            for p in 0..3 {
                pred_qep[p] = qep[p];
                pred_qep[4 + p] = (qep[4 + p] - qep[p]) & 63;
                pred_qep[8 + p] = (qep[8 + p] - qep[p]) & 63;
                pred_qep[12 + p] = (qep[12 + p] - qep[p]) & 63;
            }

            let mut pqep = [0; 8];

            pqep[0] = pred_qep[0] as u32;
            pqep[0] += Self::bit_at(pred_qep[12 + 2], 0) << 7;
            pqep[0] += Self::bit_at(pred_qep[12 + 2], 1) << 8;
            pqep[0] += Self::bit_at(pred_qep[8 + 2], 4) << 9;

            pqep[1] = pred_qep[1] as u32;
            pqep[1] += Self::bit_at(pred_qep[8 + 2], 5) << 7;
            pqep[1] += Self::bit_at(pred_qep[12 + 2], 2) << 8;
            pqep[1] += Self::bit_at(pred_qep[8 + 1], 4) << 9;

            pqep[2] = pred_qep[2] as u32;
            pqep[2] += Self::bit_at(pred_qep[12 + 2], 3) << 7;
            pqep[2] += Self::bit_at(pred_qep[12 + 2], 5) << 8;
            pqep[2] += Self::bit_at(pred_qep[12 + 2], 4) << 9;

            pqep[4] = pred_qep[4] as u32 + ((pred_qep[8 + 1] & 15) as u32 * 64);
            pqep[5] = pred_qep[5] as u32 + ((pred_qep[12 + 1] & 15) as u32 * 64);
            pqep[6] = pred_qep[6] as u32 + ((pred_qep[8 + 2] & 15) as u32 * 64);

            packed[0] = Self::get_mode_prefix(1);
            packed[0] += Self::bit_at(pred_qep[8 + 1], 5) << 2;
            packed[0] += Self::bit_at(pred_qep[12 + 1], 4) << 3;
            packed[0] += Self::bit_at(pred_qep[12 + 1], 5) << 4;

            packed[1] = (pqep[2] << 20) + (pqep[1] << 10) + pqep[0];
            packed[2] = (pqep[6] << 20) + (pqep[5] << 10) + pqep[4];
            packed[3] = ((pred_qep[12] as u32) << 6) + pred_qep[8] as u32;
        } else if mode == 2 || mode == 3 || mode == 4 {
            let mut dqep = [0; 16];
            for p in 0..3 {
                let mask = if p == mode - 2 { 31 } else { 15 };
                dqep[p] = qep[p];
                dqep[4 + p] = (qep[4 + p] - qep[p]) & mask;
                dqep[8 + p] = (qep[8 + p] - qep[p]) & mask;
                dqep[12 + p] = (qep[12 + p] - qep[p]) & mask;
            }

            let mut pqep = [0; 10];

            pqep[0] = (dqep[0] & 1023) as u32;
            pqep[1] = (dqep[1] & 1023) as u32;
            pqep[2] = (dqep[2] & 1023) as u32;

            pqep[4] = dqep[4] as u32 + ((dqep[8 + 1] & 15) as u32 * 64);
            pqep[5] = dqep[5] as u32 + ((dqep[12 + 1] & 15) as u32 * 64);
            pqep[6] = dqep[6] as u32 + ((dqep[8 + 2] & 15) as u32 * 64);

            pqep[8] = dqep[8] as u32;
            pqep[9] = dqep[12] as u32;

            if mode == 2 {
                packed[0] = Self::get_mode_prefix(2);

                pqep[5] += Self::bit_at(dqep[1], 10) << 4;
                pqep[6] += Self::bit_at(dqep[2], 10) << 4;

                pqep[4] += Self::bit_at(dqep[0], 10) << 5;
                pqep[5] += Self::bit_at(dqep[12 + 2], 0) << 5;
                pqep[6] += Self::bit_at(dqep[12 + 2], 1) << 5;
                pqep[8] += Self::bit_at(dqep[12 + 2], 2) << 5;
                pqep[9] += Self::bit_at(dqep[12 + 2], 3) << 5;
            } else if mode == 3 {
                packed[0] = Self::get_mode_prefix(3);

                pqep[4] += Self::bit_at(dqep[0], 10) << 4;
                pqep[6] += Self::bit_at(dqep[2], 10) << 4;
                pqep[8] += Self::bit_at(dqep[12 + 2], 0) << 4;
                pqep[9] += Self::bit_at(dqep[8 + 1], 4) << 4;

                pqep[4] += Self::bit_at(dqep[12 + 1], 4) << 5;
                pqep[5] += Self::bit_at(dqep[1], 10) << 5;
                pqep[6] += Self::bit_at(dqep[12 + 2], 1) << 5;
                pqep[8] += Self::bit_at(dqep[12 + 2], 2) << 5;
                pqep[9] += Self::bit_at(dqep[12 + 2], 3) << 5;
            } else if mode == 4 {
                packed[0] = Self::get_mode_prefix(4);

                pqep[4] += Self::bit_at(dqep[0], 10) << 4;
                pqep[5] += Self::bit_at(dqep[1], 10) << 4;
                pqep[8] += Self::bit_at(dqep[12 + 2], 1) << 4;
                pqep[9] += Self::bit_at(dqep[12 + 2], 4) << 4;

                pqep[4] += Self::bit_at(dqep[8 + 2], 4) << 5;
                pqep[5] += Self::bit_at(dqep[12 + 2], 0) << 5;
                pqep[6] += Self::bit_at(dqep[2], 10) << 5;
                pqep[8] += Self::bit_at(dqep[12 + 2], 2) << 5;
                pqep[9] += Self::bit_at(dqep[12 + 2], 3) << 5;
            }

            packed[1] = (pqep[2] << 20) + (pqep[1] << 10) + pqep[0];
            packed[2] = (pqep[6] << 20) + (pqep[5] << 10) + pqep[4];
            packed[3] = (pqep[9] << 6) + pqep[8];
        } else if mode == 5 {
            let mut dqep = [0; 16];
            for p in 0..3 {
                dqep[p] = qep[p];
                dqep[4 + p] = (qep[4 + p] - qep[p]) & 31;
                dqep[8 + p] = (qep[8 + p] - qep[p]) & 31;
                dqep[12 + p] = (qep[12 + p] - qep[p]) & 31;
            }

            let mut pqep = [0; 10];

            pqep[0] = dqep[0] as u32;
            pqep[1] = dqep[1] as u32;
            pqep[2] = dqep[2] as u32;
            pqep[4] = dqep[4] as u32 + (dqep[8 + 1] & 15) as u32 * 64;
            pqep[5] = dqep[5] as u32 + (dqep[12 + 1] & 15) as u32 * 64;
            pqep[6] = dqep[6] as u32 + (dqep[8 + 2] & 15) as u32 * 64;
            pqep[8] = dqep[8] as u32;
            pqep[9] = dqep[12] as u32;

            pqep[0] += Self::bit_at(dqep[8 + 2], 4) << 9;
            pqep[1] += Self::bit_at(dqep[8 + 1], 4) << 9;
            pqep[2] += Self::bit_at(dqep[12 + 2], 4) << 9;

            pqep[4] += Self::bit_at(dqep[12 + 1], 4) << 5;
            pqep[5] += Self::bit_at(dqep[12 + 2], 0) << 5;
            pqep[6] += Self::bit_at(dqep[12 + 2], 1) << 5;

            pqep[8] += Self::bit_at(dqep[12 + 2], 2) << 5;
            pqep[9] += Self::bit_at(dqep[12 + 2], 3) << 5;

            packed[0] = Self::get_mode_prefix(5);

            packed[1] = (pqep[2] << 20) + (pqep[1] << 10) + pqep[0];
            packed[2] = (pqep[6] << 20) + (pqep[5] << 10) + pqep[4];
            packed[3] = (pqep[9] << 6) + pqep[8];
        } else if mode == 6 || mode == 7 || mode == 8 {
            let mut dqep = [0; 16];
            for p in 0..3 {
                let mask = if p == mode - 6 { 63 } else { 31 };
                dqep[p] = qep[p];
                dqep[4 + p] = (qep[4 + p] - qep[p]) & mask;
                dqep[8 + p] = (qep[8 + p] - qep[p]) & mask;
                dqep[12 + p] = (qep[12 + p] - qep[p]) & mask;
            }

            let mut pqep = [0; 10];

            pqep[0] = dqep[0] as u32;
            pqep[0] += Self::bit_at(dqep[8 + 2], 4) << 9;

            pqep[1] = dqep[1] as u32;
            pqep[1] += Self::bit_at(dqep[8 + 1], 4) << 9;

            pqep[2] = dqep[2] as u32;
            pqep[2] += Self::bit_at(dqep[12 + 2], 4) << 9;

            pqep[4] = dqep[4] as u32 + (dqep[8 + 1] & 15) as u32 * 64;
            pqep[5] = dqep[5] as u32 + (dqep[12 + 1] & 15) as u32 * 64;
            pqep[6] = dqep[6] as u32 + (dqep[8 + 2] & 15) as u32 * 64;

            pqep[8] = dqep[8] as u32;
            pqep[9] = dqep[12] as u32;

            if mode == 6 {
                packed[0] = Self::get_mode_prefix(6);

                pqep[0] += Self::bit_at(dqep[12 + 1], 4) << 8;
                pqep[1] += Self::bit_at(dqep[12 + 2], 2) << 8;
                pqep[2] += Self::bit_at(dqep[12 + 2], 3) << 8;
                pqep[5] += Self::bit_at(dqep[12 + 2], 0) << 5;
                pqep[6] += Self::bit_at(dqep[12 + 2], 1) << 5;
            } else if mode == 7 {
                packed[0] = Self::get_mode_prefix(7);

                pqep[0] += Self::bit_at(dqep[12 + 2], 0) << 8;
                pqep[1] += Self::bit_at(dqep[8 + 1], 5) << 8;
                pqep[2] += Self::bit_at(dqep[12 + 1], 5) << 8;
                pqep[4] += Self::bit_at(dqep[12 + 1], 4) << 5;
                pqep[6] += Self::bit_at(dqep[12 + 2], 1) << 5;
                pqep[8] += Self::bit_at(dqep[12 + 2], 2) << 5;
                pqep[9] += Self::bit_at(dqep[12 + 2], 3) << 5;
            } else if mode == 8 {
                packed[0] = Self::get_mode_prefix(8);

                pqep[0] += Self::bit_at(dqep[12 + 2], 1) << 8;
                pqep[1] += Self::bit_at(dqep[8 + 2], 5) << 8;
                pqep[2] += Self::bit_at(dqep[12 + 2], 5) << 8;
                pqep[4] += Self::bit_at(dqep[12 + 1], 4) << 5;
                pqep[5] += Self::bit_at(dqep[12 + 2], 0) << 5;
                pqep[8] += Self::bit_at(dqep[12 + 2], 2) << 5;
                pqep[9] += Self::bit_at(dqep[12 + 2], 3) << 5;
            }

            packed[1] = (pqep[2] << 20) + (pqep[1] << 10) + pqep[0];
            packed[2] = (pqep[6] << 20) + (pqep[5] << 10) + pqep[4];
            packed[3] = (pqep[9] << 6) + pqep[8];
        } else if mode == 9 {
            let mut pqep = [0; 10];

            pqep[0] = qep[0] as u32;
            pqep[0] += Self::bit_at(qep[12 + 1], 4) << 6;
            pqep[0] += Self::bit_at(qep[12 + 2], 0) << 7;
            pqep[0] += Self::bit_at(qep[12 + 2], 1) << 8;
            pqep[0] += Self::bit_at(qep[8 + 2], 4) << 9;

            pqep[1] = qep[1] as u32;
            pqep[1] += Self::bit_at(qep[8 + 1], 5) << 6;
            pqep[1] += Self::bit_at(qep[8 + 2], 5) << 7;
            pqep[1] += Self::bit_at(qep[12 + 2], 2) << 8;
            pqep[1] += Self::bit_at(qep[8 + 1], 4) << 9;

            pqep[2] = qep[2] as u32;
            pqep[2] += Self::bit_at(qep[12 + 1], 5) << 6;
            pqep[2] += Self::bit_at(qep[12 + 2], 3) << 7;
            pqep[2] += Self::bit_at(qep[12 + 2], 5) << 8;
            pqep[2] += Self::bit_at(qep[12 + 2], 4) << 9;

            pqep[4] = qep[4] as u32 + (qep[8 + 1] & 15) as u32 * 64;
            pqep[5] = qep[5] as u32 + (qep[12 + 1] & 15) as u32 * 64;
            pqep[6] = qep[6] as u32 + (qep[8 + 2] & 15) as u32 * 64;

            packed[0] = Self::get_mode_prefix(9);
            packed[1] = (pqep[2] << 20) + (pqep[1] << 10) + pqep[0];
            packed[2] = (pqep[6] << 20) + (pqep[5] << 10) + pqep[4];
            packed[3] = (qep[12] << 6) as u32 + qep[8] as u32;
        } else if mode == 10 {
            packed[0] = Self::get_mode_prefix(10);
            packed[1] = (qep[2] << 20) as u32 + (qep[1] << 10) as u32 + qep[0] as u32;
            packed[2] = (qep[6] << 20) as u32 + (qep[5] << 10) as u32 + qep[4] as u32;
        } else if mode == 11 {
            let mut dqep = [0; 8];
            for p in 0..3 {
                dqep[p] = qep[p];
                dqep[4 + p] = (qep[4 + p] - qep[p]) & 511;
            }

            let mut pqep = [0; 10];

            pqep[0] = (dqep[0] & 1023) as u32;
            pqep[1] = (dqep[1] & 1023) as u32;
            pqep[2] = (dqep[2] & 1023) as u32;

            pqep[4] = dqep[4] as u32 + (dqep[0] >> 10) as u32 * 512;
            pqep[5] = dqep[5] as u32 + (dqep[1] >> 10) as u32 * 512;
            pqep[6] = dqep[6] as u32 + (dqep[2] >> 10) as u32 * 512;

            packed[0] = Self::get_mode_prefix(11);
            packed[1] = (pqep[2] << 20) + (pqep[1] << 10) + pqep[0];
            packed[2] = (pqep[6] << 20) + (pqep[5] << 10) + pqep[4];
        } else if mode == 12 {
            let mut dqep = [0; 8];
            for p in 0..3 {
                dqep[p] = qep[p];
                dqep[4 + p] = (qep[4 + p] - qep[p]) & 255;
            }

            let mut pqep = [0; 8];

            pqep[0] = (dqep[0] & 1023) as u32;
            pqep[1] = (dqep[1] & 1023) as u32;
            pqep[2] = (dqep[2] & 1023) as u32;

            pqep[4] = dqep[4] as u32 + Self::reverse_bits((dqep[0] >> 10) as u32, 2) * 256;
            pqep[5] = dqep[5] as u32 + Self::reverse_bits((dqep[1] >> 10) as u32, 2) * 256;
            pqep[6] = dqep[6] as u32 + Self::reverse_bits((dqep[2] >> 10) as u32, 2) * 256;

            packed[0] = Self::get_mode_prefix(12);
            packed[1] = (pqep[2] << 20) + (pqep[1] << 10) + pqep[0];
            packed[2] = (pqep[6] << 20) + (pqep[5] << 10) + pqep[4];
        } else if mode == 13 {
            let mut dqep = [0; 8];
            for p in 0..3 {
                dqep[p] = qep[p];
                dqep[4 + p] = (qep[4 + p] - qep[p]) & 15;
            }

            let mut pqep = [0; 8];

            pqep[0] = (dqep[0] & 1023) as u32;
            pqep[1] = (dqep[1] & 1023) as u32;
            pqep[2] = (dqep[2] & 1023) as u32;

            pqep[4] = dqep[4] as u32 + Self::reverse_bits((dqep[0] >> 10) as u32, 6) * 16;
            pqep[5] = dqep[5] as u32 + Self::reverse_bits((dqep[1] >> 10) as u32, 6) * 16;
            pqep[6] = dqep[6] as u32 + Self::reverse_bits((dqep[2] >> 10) as u32, 6) * 16;

            packed[0] = Self::get_mode_prefix(13);
            packed[1] = (pqep[2] << 20) + (pqep[1] << 10) + pqep[0];
            packed[2] = (pqep[6] << 20) + (pqep[5] << 10) + pqep[4];
        }
    }

    fn bc6h_setup(&mut self) {
        for p in 0..3 {
            self.rgb_bounds[p] = 0xFFFF as f32;
            self.rgb_bounds[3 + p] = 0.0;
        }

        // Find min/max bounds
        for p in 0..3 {
            for k in 0..16 {
                let value = (self.block[p * 16 + k] / 31.0) * 64.0;
                self.block[p * 16 + k] = value;
                self.rgb_bounds[p] = f32::min(self.rgb_bounds[p], value);
                self.rgb_bounds[3 + p] = f32::max(self.rgb_bounds[3 + p], value);
            }
        }

        self.max_span = 0.0;
        self.max_span_idx = 0;

        for p in 0..3 {
            let span = self.rgb_bounds[3 + p] - self.rgb_bounds[p];
            if span > self.max_span {
                self.max_span_idx = p;
                self.max_span = span;
            }
        }
    }

    pub(crate) fn compress_bc6h_core(&mut self) {
        self.bc6h_setup();

        if self.settings.slow_mode != 0 {
            self.bc6h_test_mode(0, true, 0.0);
            self.bc6h_test_mode(1, true, 0.0);
            self.bc6h_test_mode(2, true, 0.0);
            self.bc6h_test_mode(5, true, 0.0);
            self.bc6h_test_mode(6, true, 0.0);
            self.bc6h_test_mode(9, true, 0.0);
            self.bc6h_test_mode(10, true, 0.0);
            self.bc6h_test_mode(11, true, 0.0);
            self.bc6h_test_mode(12, true, 0.0);
            self.bc6h_test_mode(13, true, 0.0);
        } else {
            if self.settings.fast_skip_threshold > 0 {
                self.bc6h_test_mode(9, false, 0.0);

                if self.settings.fast_mode != 0 {
                    self.bc6h_test_mode(1, false, 1.0);
                }

                self.bc6h_test_mode(6, false, 1.0 / 1.2);
                self.bc6h_test_mode(5, false, 1.0 / 1.2);
                self.bc6h_test_mode(0, false, 1.0 / 1.2);
                self.bc6h_test_mode(2, false, 1.0);
                self.bc6h_enc_2p();

                if self.settings.fast_mode == 0 {
                    self.bc6h_test_mode(1, true, 0.0);
                }
            }

            self.bc6h_test_mode(10, false, 0.0);
            self.bc6h_test_mode(11, false, 1.0);
            self.bc6h_test_mode(12, false, 1.0);
            self.bc6h_test_mode(13, false, 1.0);
            self.bc6h_enc_1p();
        }
    }
}
