use super::common::*;
use crate::BC7Settings;

#[derive(Default)]
struct Mode45Parameters {
    qep: [i32; 8],
    qblock: [u32; 2],
    aqep: [i32; 2],
    aqblock: [u32; 2],
    rotation: u32,
    swap: u32,
}

pub(crate) struct BlockCompressorBC7<'a> {
    block: [f32; 64],
    data: [u32; 5],
    best_err: f32,
    opaque_err: f32,
    settings: &'a BC7Settings,
}

#[inline(always)]
const fn sq(x: f32) -> f32 {
    x * x
}

impl<'a> BlockCompressorBC7<'a> {
    pub(crate) fn new(settings: &'a BC7Settings) -> Self {
        Self {
            block: [0.0; 64],
            data: [0; 5],
            best_err: f32::INFINITY,
            opaque_err: 0.0,
            settings,
        }
    }

    pub(crate) fn load_block_interleaved_rgba(
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

                let red = rgba_data[offset] as f32;
                let green = rgba_data[offset + 1] as f32;
                let blue = rgba_data[offset + 2] as f32;
                let alpha = rgba_data[offset + 3] as f32;

                self.block[y * 4 + x] = red;
                self.block[16 + y * 4 + x] = green;
                self.block[32 + y * 4 + x] = blue;
                self.block[48 + y * 4 + x] = alpha;
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

    fn unpack_to_byte(v: i32, bits: u32) -> i32 {
        let vv = v << (8 - bits);
        vv + (vv >> bits)
    }

    fn ep_quant0367(qep: &mut [i32], ep: &[f32], mode: usize, channels: usize) {
        let bits = if mode == 0 {
            4
        } else if mode == 7 {
            5
        } else {
            7
        };
        let levels = 1 << bits;
        let levels2 = levels * 2 - 1;

        for i in 0..2 {
            let mut qep_b = [0; 8];

            for b in 0..2 {
                for p in 0..4 {
                    let v = ((ep[i * 4 + p] / 255.0 * levels2 as f32 - b as f32) / 2.0 + 0.5)
                        as i32
                        * 2
                        + b as i32;
                    qep_b[b * 4 + p] = i32::clamp(v, b as i32, levels2 - 1 + b as i32);
                }
            }

            let mut ep_b = [0.0; 8];
            for j in 0..8 {
                ep_b[j] = qep_b[j] as f32;
            }

            if mode == 0 {
                for j in 0..8 {
                    ep_b[j] = Self::unpack_to_byte(qep_b[j], 5) as f32;
                }
            }

            let mut err0 = 0.0;
            let mut err1 = 0.0;
            for p in 0..channels {
                err0 += sq(ep[i * 4 + p] - ep_b[p]);
                err1 += sq(ep[i * 4 + p] - ep_b[4 + p]);
            }

            for p in 0..4 {
                qep[i * 4 + p] = if err0 < err1 { qep_b[p] } else { qep_b[4 + p] };
            }
        }
    }

    fn ep_quant1(qep: &mut [i32], ep: &mut [f32]) {
        let mut qep_b = [0; 16];

        for b in 0..2 {
            for i in 0..8 {
                let v = ((ep[i] / 255.0 * 127.0 - b as f32) / 2.0 + 0.5) as i32 * 2 + b as i32;
                qep_b[b * 8 + i] = i32::clamp(v, b as i32, 126 + b as i32);
            }
        }

        // dequant
        let mut ep_b = [0.0; 16];
        for k in 0..16 {
            ep_b[k] = Self::unpack_to_byte(qep_b[k], 7) as f32;
        }

        let mut err0 = 0.0;
        let mut err1 = 0.0;
        for j in 0..2 {
            for p in 0..3 {
                err0 += sq(ep[j * 4 + p] - ep_b[j * 4 + p]);
                err1 += sq(ep[j * 4 + p] - ep_b[8 + j * 4 + p]);
            }
        }

        for i in 0..8 {
            qep[i] = if err0 < err1 { qep_b[i] } else { qep_b[8 + i] };
        }
    }

    fn ep_quant245(qep: &mut [i32], ep: &[f32], mode: usize) {
        let bits = if mode == 5 { 7 } else { 5 };

        let levels = 1 << bits;

        for i in 0..8 {
            let v = (ep[i] / 255.0 * (levels - 1) as f32 + 0.5) as i32;
            qep[i] = i32::clamp(v, 0, levels - 1);
        }
    }

    fn ep_quant(qep: &mut [i32], ep: &mut [f32], mode: usize, channels: usize) {
        const PAIRS_TABLE: [usize; 8] = [3, 2, 3, 2, 1, 1, 1, 2];
        let pairs = PAIRS_TABLE[mode];

        if mode == 0 || mode == 3 || mode == 6 || mode == 7 {
            for i in 0..pairs {
                Self::ep_quant0367(&mut qep[i * 8..], &ep[i * 8..], mode, channels);
            }
        } else if mode == 1 {
            for i in 0..pairs {
                Self::ep_quant1(&mut qep[i * 8..], &mut ep[i * 8..]);
            }
        } else if mode == 2 || mode == 4 || mode == 5 {
            for i in 0..pairs {
                Self::ep_quant245(&mut qep[i * 8..], &ep[i * 8..], mode);
            }
        }
    }

    fn ep_dequant(ep: &mut [f32], qep: &[i32], mode: usize) {
        const PAIRS_TABLE: [usize; 8] = [3, 2, 3, 2, 1, 1, 1, 2];
        let pairs = PAIRS_TABLE[mode];

        // mode 3, 6 are 8-bit
        if mode == 3 || mode == 6 {
            for i in 0..8 * pairs {
                ep[i] = qep[i] as f32;
            }
        } else if mode == 1 || mode == 5 {
            for i in 0..8 * pairs {
                ep[i] = Self::unpack_to_byte(qep[i], 7) as f32;
            }
        } else if mode == 0 || mode == 2 || mode == 4 {
            for i in 0..8 * pairs {
                ep[i] = Self::unpack_to_byte(qep[i], 5) as f32;
            }
        } else if mode == 7 {
            for i in 0..8 * pairs {
                ep[i] = Self::unpack_to_byte(qep[i], 6) as f32;
            }
        }
    }

    fn ep_quant_dequant(qep: &mut [i32], ep: &mut [f32], mode: usize, channels: usize) {
        Self::ep_quant(qep, ep, mode, channels);
        Self::ep_dequant(ep, qep, mode);
    }

    fn opt_channel(
        &self,
        qblock: &mut [u32; 2],
        qep: &mut [i32; 2],
        channel_block: &[f32; 16],
        bits: u32,
        epbits: u32,
    ) -> f32 {
        let mut ep = [255.0, 0.0];

        for k in 0..16 {
            ep[0] = f32::min(ep[0], channel_block[k]);
            ep[1] = f32::max(ep[1], channel_block[k]);
        }

        Self::channel_quant_dequant(qep, &mut ep, epbits);
        let mut err = Self::channel_opt_quant(qblock, channel_block, bits, &ep);

        // Refine
        let refine_iterations = self.settings.refine_iterations_channel;
        for _ in 0..refine_iterations {
            Self::channel_opt_endpoints(&mut ep, channel_block, bits, *qblock);
            Self::channel_quant_dequant(qep, &mut ep, epbits);
            err = Self::channel_opt_quant(qblock, channel_block, bits, &ep);
        }

        err
    }

    fn channel_quant_dequant(qep: &mut [i32; 2], ep: &mut [f32; 2], epbits: u32) {
        let elevels = 1 << epbits;

        for i in 0..2 {
            let v = (ep[i] / 255.0 * (elevels - 1) as f32 + 0.5) as i32;
            qep[i] = i32::clamp(v, 0, elevels - 1);
            ep[i] = Self::unpack_to_byte(qep[i], epbits) as f32;
        }
    }

    fn channel_opt_quant(
        qblock: &mut [u32; 2],
        channel_block: &[f32; 16],
        bits: u32,
        ep: &[f32; 2],
    ) -> f32 {
        let levels = 1 << bits;

        qblock[0] = 0;
        qblock[1] = 0;

        let mut total_err = 0.0;

        for k in 0..16 {
            let proj = (channel_block[k] - ep[0]) / (ep[1] - ep[0] + 0.001);

            let q1 = (proj * levels as f32 + 0.5) as i32;
            let q1_clamped = i32::clamp(q1, 1, levels - 1);

            let mut err0 = 0.0;
            let mut err1 = 0.0;
            let w0 = get_unquant_value(bits, q1_clamped - 1);
            let w1 = get_unquant_value(bits, q1_clamped);

            let dec_v0 = (((64 - w0) * ep[0] as i32 + w0 * ep[1] as i32 + 32) / 64) as f32;
            let dec_v1 = (((64 - w1) * ep[0] as i32 + w1 * ep[1] as i32 + 32) / 64) as f32;
            err0 += sq(dec_v0 - channel_block[k]);
            err1 += sq(dec_v1 - channel_block[k]);

            let best_err = if err0 < err1 { err0 } else { err1 };

            let best_q = if err0 < err1 {
                q1_clamped - 1
            } else {
                q1_clamped
            };

            qblock[k / 8] |= (best_q as u32) << (4 * (k % 8));
            total_err += best_err;
        }

        total_err
    }

    fn channel_opt_endpoints(
        ep: &mut [f32; 2],
        channel_block: &[f32; 16],
        bits: u32,
        qblock: [u32; 2],
    ) {
        let levels = 1 << bits;

        let mut atb1 = 0.0;
        let mut sum_q = 0.0;
        let mut sum_qq = 0.0;
        let mut sum = 0.0;

        for k1 in 0..2 {
            let mut qbits_shifted = qblock[k1];
            for k2 in 0..8 {
                let k = k1 * 8 + k2;
                let q = (qbits_shifted & 15) as f32;
                qbits_shifted >>= 4;

                let x = (levels - 1) as f32 - q;

                sum_q += q;
                sum_qq += q * q;

                sum += channel_block[k];
                atb1 += x * channel_block[k];
            }
        }

        let atb2 = (levels - 1) as f32 * sum - atb1;

        let cxx = 16.0 * sq((levels - 1) as f32) - 2.0 * (levels - 1) as f32 * sum_q + sum_qq;
        let cyy = sum_qq;
        let cxy = (levels - 1) as f32 * sum_q - sum_qq;
        let scale = (levels - 1) as f32 / (cxx * cyy - cxy * cxy);

        ep[0] = (atb1 * cyy - atb2 * cxy) * scale;
        ep[1] = (atb2 * cxx - atb1 * cxy) * scale;

        ep[0] = f32::clamp(ep[0], 0.0, 255.0);
        ep[1] = f32::clamp(ep[1], 0.0, 255.0);

        if f32::abs(cxx * cyy - cxy * cxy) < 0.001 {
            ep[0] = sum / 16.0;
            ep[1] = ep[0];
        }
    }

    pub(crate) fn block_segment(ep: &mut [f32], block: &[f32; 64], mask: u32, channels: usize) {
        block_segment_core(ep, block, mask, channels);

        for i in 0..2 {
            for p in 0..channels {
                ep[4 * i + p] = f32::clamp(ep[4 * i + p], 0.0, 255.0);
            }
        }
    }

    fn bc7_code_mode01237(
        &mut self,
        qep: &mut [i32; 24],
        qblock: [u32; 2],
        part_id: i32,
        mode: usize,
    ) {
        let bits = if mode == 0 || mode == 1 { 3 } else { 2 };
        let pairs = if mode == 0 || mode == 2 { 3 } else { 2 };
        let channels = if mode == 7 { 4 } else { 3 };

        let flips = bc7_code_apply_swap_mode01237(qep, qblock, mode, part_id);

        self.data = [0; 5];
        let mut pos = 0;

        // Mode 0-3, 7
        put_bits(&mut self.data, &mut pos, (mode + 1) as u32, 1 << mode);

        // Partition
        if mode == 0 {
            put_bits(&mut self.data, &mut pos, 4, (part_id & 15) as u32);
        } else {
            put_bits(&mut self.data, &mut pos, 6, (part_id & 63) as u32);
        }

        // Endpoints
        for p in 0..channels {
            for j in 0..pairs * 2 {
                if mode == 0 {
                    put_bits(&mut self.data, &mut pos, 4, (qep[j * 4 + p] as u32) >> 1);
                } else if mode == 1 {
                    put_bits(&mut self.data, &mut pos, 6, (qep[j * 4 + p] as u32) >> 1);
                } else if mode == 2 {
                    put_bits(&mut self.data, &mut pos, 5, qep[j * 4 + p] as u32);
                } else if mode == 3 {
                    put_bits(&mut self.data, &mut pos, 7, (qep[j * 4 + p] as u32) >> 1);
                } else if mode == 7 {
                    put_bits(&mut self.data, &mut pos, 5, (qep[j * 4 + p] as u32) >> 1);
                }
            }
        }

        // P bits
        if mode == 1 {
            for j in 0..2 {
                put_bits(&mut self.data, &mut pos, 1, (qep[j * 8] as u32) & 1);
            }
        }

        if mode == 0 || mode == 3 || mode == 7 {
            for j in 0..pairs * 2 {
                put_bits(&mut self.data, &mut pos, 1, (qep[j * 4] as u32) & 1);
            }
        }

        // Quantized values
        bc7_code_qblock(&mut self.data, &mut pos, qblock, bits, flips);
        bc7_code_adjust_skip_mode01237(&mut self.data, mode, part_id);
    }

    fn bc7_code_mode45(&mut self, params: &Mode45Parameters, mode: usize) {
        let mut qep = params.qep;
        let mut qblock = params.qblock;
        let mut aqep = params.aqep;
        let mut aqblock = params.aqblock;
        let rotation = params.rotation;
        let swap = params.swap;

        let bits = 2;
        let abits = if mode == 4 { 3 } else { 2 };
        let epbits = if mode == 4 { 5 } else { 7 };
        let aepbits = if mode == 4 { 6 } else { 8 };

        if swap == 0 {
            bc7_code_apply_swap_mode456(&mut qep, 4, &mut qblock, bits);
            bc7_code_apply_swap_mode456(&mut aqep, 1, &mut aqblock, abits);
        } else {
            std::mem::swap(&mut qblock, &mut aqblock);

            bc7_code_apply_swap_mode456(&mut aqep, 1, &mut qblock, bits);
            bc7_code_apply_swap_mode456(&mut qep, 4, &mut aqblock, abits);
        }

        // Clear state data
        self.data = [0; 5];
        let mut pos = 0;

        // Mode 4-5
        put_bits(&mut self.data, &mut pos, (mode + 1) as u32, 1 << mode);

        // Rotation
        put_bits(&mut self.data, &mut pos, 2, (rotation + 1) & 3);

        if mode == 4 {
            put_bits(&mut self.data, &mut pos, 1, swap);
        }

        // Endpoints
        for p in 0..3 {
            put_bits(&mut self.data, &mut pos, epbits, qep[p] as u32);
            put_bits(&mut self.data, &mut pos, epbits, qep[4 + p] as u32);
        }

        // Alpha endpoints
        put_bits(&mut self.data, &mut pos, aepbits, aqep[0] as u32);
        put_bits(&mut self.data, &mut pos, aepbits, aqep[1] as u32);

        // Quantized values
        bc7_code_qblock(&mut self.data, &mut pos, qblock, bits, 0);
        bc7_code_qblock(&mut self.data, &mut pos, aqblock, abits, 0);
    }

    fn bc7_code_mode6(&mut self, qep: &mut [i32], qblock: &mut [u32; 2]) {
        bc7_code_apply_swap_mode456(qep, 4, qblock, 4);

        self.data = [0; 5];
        let mut pos = 0;

        // Mode 6
        put_bits(&mut self.data, &mut pos, 7, 64);

        // Endpoints
        for p in 0..4 {
            put_bits(&mut self.data, &mut pos, 7, (qep[p] as u32) >> 1);
            put_bits(&mut self.data, &mut pos, 7, (qep[4 + p] as u32) >> 1);
        }

        // P bits
        put_bits(&mut self.data, &mut pos, 1, (qep[0] as u32) & 1);
        put_bits(&mut self.data, &mut pos, 1, (qep[4] as u32) & 1);

        // Quantized values
        bc7_code_qblock(&mut self.data, &mut pos, *qblock, 4, 0);
    }

    fn bc7_enc_mode01237_part_fast(
        &self,
        qep: &mut [i32; 24],
        qblock: &mut [u32; 2],
        part_id: i32,
        mode: usize,
    ) -> f32 {
        let pattern = get_pattern(part_id);
        let bits = if mode == 0 || mode == 1 { 3 } else { 2 };
        let pairs = if mode == 0 || mode == 2 { 3 } else { 2 };
        let channels = if mode == 7 { 4 } else { 3 };

        let mut ep = [0.0; 24];
        for j in 0..pairs {
            let mask = get_pattern_mask(part_id, j as u32);
            Self::block_segment(&mut ep[j * 8..], &self.block, mask, channels);
        }

        Self::ep_quant_dequant(qep, &mut ep, mode, channels);

        block_quant(qblock, &self.block, bits, &ep, pattern, channels)
    }

    fn bc7_enc_mode01237(&mut self, mode: usize, part_list: &[i32; 64], part_count: usize) {
        if part_count == 0 {
            return;
        }

        let bits = if mode == 0 || mode == 1 { 3 } else { 2 };
        let pairs = if mode == 0 || mode == 2 { 3 } else { 2 };
        let channels = if mode == 7 { 4 } else { 3 };

        let mut best_qep = [0; 24];
        let mut best_qblock = [0; 2];
        let mut best_part_id = -1;
        let mut best_err = f32::INFINITY;

        for &part in part_list[..part_count].iter() {
            let mut part_id = part & 63;
            part_id = if pairs == 3 { part_id + 64 } else { part_id };

            let mut qep = [0; 24];
            let mut qblock = [0; 2];
            let err = self.bc7_enc_mode01237_part_fast(&mut qep, &mut qblock, part_id, mode);

            if err < best_err {
                best_qep[..(8 * pairs)].copy_from_slice(&qep[..(8 * pairs)]);
                best_qblock.copy_from_slice(&qblock);

                best_part_id = part_id;
                best_err = err;
            }
        }

        let refine_iterations = self.settings.refine_iterations[mode];
        for _ in 0..refine_iterations {
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

            Self::ep_quant_dequant(&mut qep, &mut ep, mode, channels);

            let pattern = get_pattern(best_part_id);
            let err = block_quant(&mut qblock, &self.block, bits, &ep, pattern, channels);

            if err < best_err {
                best_qep[..(8 * pairs)].copy_from_slice(&qep[..(8 * pairs)]);
                best_qblock.copy_from_slice(&qblock);

                best_err = err;
            }
        }

        if mode != 7 {
            best_err += self.opaque_err;
        }

        if best_err < self.best_err {
            self.best_err = best_err;
            self.bc7_code_mode01237(&mut best_qep, best_qblock, best_part_id, mode);
        }
    }

    fn bc7_enc_mode02(&mut self) {
        let part_list: [i32; 64] = std::array::from_fn(|part| part as i32);

        self.bc7_enc_mode01237(0, &part_list, 16);

        if self.settings.skip_mode2 == 0 {
            self.bc7_enc_mode01237(2, &part_list, 64);
        }
    }

    fn bc7_enc_mode13(&mut self) {
        if self.settings.fast_skip_threshold_mode1 == 0
            && self.settings.fast_skip_threshold_mode3 == 0
        {
            return;
        }

        let mut full_stats = [0.0; 15];
        compute_stats_masked(&mut full_stats, &self.block, 0xFFFFFFFF, 3);

        let mut part_list = [0; 64];
        for part in 0..64 {
            let mask = get_pattern_mask(part, 0);
            let bound12 = block_pca_bound_split(&self.block, mask, full_stats, 3);
            let bound = bound12 as i32;
            part_list[part as usize] = part + bound * 64;
        }

        let partial_count = u32::max(
            self.settings.fast_skip_threshold_mode1,
            self.settings.fast_skip_threshold_mode3,
        );
        partial_sort_list(&mut part_list, 64, partial_count);
        self.bc7_enc_mode01237(
            1,
            &part_list,
            self.settings.fast_skip_threshold_mode1 as usize,
        );
        self.bc7_enc_mode01237(
            3,
            &part_list,
            self.settings.fast_skip_threshold_mode3 as usize,
        );
    }

    fn bc7_enc_mode45_candidate(
        &self,
        best_candidate: &mut Mode45Parameters,
        best_err: &mut f32,
        mode: usize,
        rotation: u32,
        swap: u32,
    ) {
        let mut bits = 2;
        let mut abits = 2;
        let mut aepbits = 8;

        if mode == 4 {
            abits = 3;
            aepbits = 6;
        }

        // (mode 4)
        if swap == 1 {
            bits = 3;
            abits = 2;
        }

        let mut candidate_block = [0.0; 64];

        for k in 0..16 {
            for p in 0..3 {
                candidate_block[k + p * 16] = self.block[k + p * 16];
            }

            if rotation < 3 {
                // Apply channel rotation
                if self.settings.channels == 4 {
                    candidate_block[k + rotation as usize * 16] = self.block[k + 3 * 16];
                }
                if self.settings.channels == 3 {
                    candidate_block[k + rotation as usize * 16] = 255.0;
                }
            }
        }

        let mut ep = [0.0; 8];
        Self::block_segment(&mut ep, &candidate_block, 0xFFFFFFFF, 3);

        let mut qep = [0; 8];
        Self::ep_quant_dequant(&mut qep, &mut ep, mode, 3);

        let mut qblock = [0; 2];
        let mut err = block_quant(&mut qblock, &candidate_block, bits, &ep, 0, 3);

        // Refine
        let refine_iterations = self.settings.refine_iterations[mode];
        for _ in 0..refine_iterations {
            opt_endpoints(&mut ep, &candidate_block, bits, qblock, 0xFFFFFFFF, 3);
            Self::ep_quant_dequant(&mut qep, &mut ep, mode, 3);
            err = block_quant(&mut qblock, &candidate_block, bits, &ep, 0, 3);
        }

        let channel_data: [f32; 16] =
            std::array::from_fn(|k| self.block[k + rotation as usize * 16]);

        // Encoding selected channel
        let mut aqep = [0; 2];
        let mut aqblock = [0; 2];

        err += self.opt_channel(&mut aqblock, &mut aqep, &channel_data, abits, aepbits);

        if err < *best_err {
            best_candidate.qep.copy_from_slice(&qep[..8]);
            best_candidate.qblock.copy_from_slice(&qblock);
            best_candidate.aqblock.copy_from_slice(&aqblock);
            best_candidate.aqep.copy_from_slice(&aqep);
            best_candidate.rotation = rotation;
            best_candidate.swap = swap;
            *best_err = err;
        }
    }

    fn bc7_enc_mode45(&mut self) {
        let mut best_candidate = Mode45Parameters::default();
        let mut best_err = self.best_err;

        let channel0 = self.settings.mode45_channel0;
        for p in channel0..self.settings.channels {
            self.bc7_enc_mode45_candidate(&mut best_candidate, &mut best_err, 4, p, 0);
            self.bc7_enc_mode45_candidate(&mut best_candidate, &mut best_err, 4, p, 1);
        }

        // Mode 4
        if best_err < self.best_err {
            self.best_err = best_err;
            self.bc7_code_mode45(&best_candidate, 4);
        }

        for p in channel0..self.settings.channels {
            self.bc7_enc_mode45_candidate(&mut best_candidate, &mut best_err, 5, p, 0);
        }

        // Mode 5
        if best_err < self.best_err {
            self.best_err = best_err;
            self.bc7_code_mode45(&best_candidate, 5);
        }
    }

    fn bc7_enc_mode6(&mut self) {
        const MODE: usize = 6;
        const BITS: u32 = 4;

        let mut ep = [0.0; 8];
        Self::block_segment(
            &mut ep,
            &self.block,
            0xFFFFFFFF,
            self.settings.channels as usize,
        );

        if self.settings.channels == 3 {
            ep[3] = 255.0;
            ep[7] = 255.0;
        }

        let mut qep = [0; 8];
        Self::ep_quant_dequant(&mut qep, &mut ep, MODE, self.settings.channels as usize);

        let mut qblock = [0; 2];
        let mut err = block_quant(
            &mut qblock,
            &self.block,
            BITS,
            &ep,
            0,
            self.settings.channels as usize,
        );

        let refine_iterations = self.settings.refine_iterations[MODE];
        for _ in 0..refine_iterations {
            opt_endpoints(
                &mut ep,
                &self.block,
                BITS,
                qblock,
                0xFFFFFFFF,
                self.settings.channels as usize,
            );
            Self::ep_quant_dequant(&mut qep, &mut ep, MODE, self.settings.channels as usize);
            err = block_quant(
                &mut qblock,
                &self.block,
                BITS,
                &ep,
                0,
                self.settings.channels as usize,
            );
        }

        if err < self.best_err {
            self.best_err = err;
            self.bc7_code_mode6(&mut qep, &mut qblock);
        }
    }

    fn bc7_enc_mode7(&mut self) {
        if self.settings.fast_skip_threshold_mode7 == 0 {
            return;
        }

        let mut full_stats = [0.0; 15];
        compute_stats_masked(
            &mut full_stats,
            &self.block,
            0xFFFFFFFF,
            self.settings.channels as usize,
        );

        let mut part_list = [0; 64];
        for part in 0..64 {
            let mask = get_pattern_mask(part, 0);
            let bound12 = block_pca_bound_split(
                &self.block,
                mask,
                full_stats,
                self.settings.channels as usize,
            );
            let bound = bound12 as i32;
            part_list[part as usize] = part + bound * 64;
        }

        partial_sort_list(&mut part_list, 64, self.settings.fast_skip_threshold_mode7);
        self.bc7_enc_mode01237(
            7,
            &part_list,
            self.settings.fast_skip_threshold_mode7 as usize,
        );
    }

    pub(crate) fn compress_block_bc7_core(&mut self) {
        if self.settings.mode_selection[0] != 0 {
            self.bc7_enc_mode02();
        }
        if self.settings.mode_selection[1] != 0 {
            self.bc7_enc_mode13();
            self.bc7_enc_mode7();
        }
        if self.settings.mode_selection[2] != 0 {
            self.bc7_enc_mode45();
        }
        if self.settings.mode_selection[3] != 0 {
            self.bc7_enc_mode6();
        }
    }

    pub(crate) fn compute_opaque_err(&mut self) {
        self.opaque_err = if self.settings.channels == 3 {
            0.0
        } else {
            let mut err = 0.0;
            for k in 0..16 {
                err += sq(self.block[48 + k] - 255.0);
            }
            err
        };
    }
}
