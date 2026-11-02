#[inline(always)]
pub(crate) const fn sq(x: f32) -> f32 {
    x * x
}

pub(crate) fn get_unquant_value(bits: u32, index: i32) -> i32 {
    match bits {
        2 => {
            const TABLE: [i32; 16] = [0, 21, 43, 64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
            TABLE[index as usize]
        }
        3 => {
            const TABLE: [i32; 16] = [0, 9, 18, 27, 37, 46, 55, 64, 0, 0, 0, 0, 0, 0, 0, 0];
            TABLE[index as usize]
        }
        _ => {
            const TABLE: [i32; 16] = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];
            TABLE[index as usize]
        }
    }
}

pub(crate) fn get_pattern(part_id: i32) -> u32 {
    const PATTERN_TABLE: [u32; 128] = [
        0x50505050, 0x40404040, 0x54545454, 0x54505040, 0x50404000, 0x55545450, 0x55545040,
        0x54504000, 0x50400000, 0x55555450, 0x55544000, 0x54400000, 0x55555440, 0x55550000,
        0x55555500, 0x55000000, 0x55150100, 0x00004054, 0x15010000, 0x00405054, 0x00004050,
        0x15050100, 0x05010000, 0x40505054, 0x00404050, 0x05010100, 0x14141414, 0x05141450,
        0x01155440, 0x00555500, 0x15014054, 0x05414150, 0x44444444, 0x55005500, 0x11441144,
        0x05055050, 0x05500550, 0x11114444, 0x41144114, 0x44111144, 0x15055054, 0x01055040,
        0x05041050, 0x05455150, 0x14414114, 0x50050550, 0x41411414, 0x00141400, 0x00041504,
        0x00105410, 0x10541000, 0x04150400, 0x50410514, 0x41051450, 0x05415014, 0x14054150,
        0x41050514, 0x41505014, 0x40011554, 0x54150140, 0x50505500, 0x00555050, 0x15151010,
        0x54540404, 0xAA685050, 0x6A5A5040, 0x5A5A4200, 0x5450A0A8, 0xA5A50000, 0xA0A05050,
        0x5555A0A0, 0x5A5A5050, 0xAA550000, 0xAA555500, 0xAAAA5500, 0x90909090, 0x94949494,
        0xA4A4A4A4, 0xA9A59450, 0x2A0A4250, 0xA5945040, 0x0A425054, 0xA5A5A500, 0x55A0A0A0,
        0xA8A85454, 0x6A6A4040, 0xA4A45000, 0x1A1A0500, 0x0050A4A4, 0xAAA59090, 0x14696914,
        0x69691400, 0xA08585A0, 0xAA821414, 0x50A4A450, 0x6A5A0200, 0xA9A58000, 0x5090A0A8,
        0xA8A09050, 0x24242424, 0x00AA5500, 0x24924924, 0x24499224, 0x50A50A50, 0x500AA550,
        0xAAAA4444, 0x66660000, 0xA5A0A5A0, 0x50A050A0, 0x69286928, 0x44AAAA44, 0x66666600,
        0xAA444444, 0x54A854A8, 0x95809580, 0x96969600, 0xA85454A8, 0x80959580, 0xAA141414,
        0x96960000, 0xAAAA1414, 0xA05050A0, 0xA0A5A5A0, 0x96000000, 0x40804080, 0xA9A8A9A8,
        0xAAAAAA44, 0x2A4A5254,
    ];

    PATTERN_TABLE[part_id as usize]
}

pub(crate) fn get_pattern_mask(part_id: i32, j: u32) -> u32 {
    const PATTERN_MASK_TABLE: [u32; 128] = [
        0xCCCC3333, 0x88887777, 0xEEEE1111, 0xECC81337, 0xC880377F, 0xFEEC0113, 0xFEC80137,
        0xEC80137F, 0xC80037FF, 0xFFEC0013, 0xFE80017F, 0xE80017FF, 0xFFE80017, 0xFF0000FF,
        0xFFF0000F, 0xF0000FFF, 0xF71008EF, 0x008EFF71, 0x71008EFF, 0x08CEF731, 0x008CFF73,
        0x73108CEF, 0x3100CEFF, 0x8CCE7331, 0x088CF773, 0x3110CEEF, 0x66669999, 0x366CC993,
        0x17E8E817, 0x0FF0F00F, 0x718E8E71, 0x399CC663, 0xAAAA5555, 0xF0F00F0F, 0x5A5AA5A5,
        0x33CCCC33, 0x3C3CC3C3, 0x55AAAA55, 0x96966969, 0xA55A5AA5, 0x73CE8C31, 0x13C8EC37,
        0x324CCDB3, 0x3BDCC423, 0x69969669, 0xC33C3CC3, 0x99666699, 0x0660F99F, 0x0272FD8D,
        0x04E4FB1B, 0x4E40B1BF, 0x2720D8DF, 0xC93636C9, 0x936C6C93, 0x39C6C639, 0x639C9C63,
        0x93366CC9, 0x9CC66339, 0x817E7E81, 0xE71818E7, 0xCCF0330F, 0x0FCCF033, 0x774488BB,
        0xEE2211DD, 0x08CC0133, 0x8CC80037, 0xCC80006F, 0xEC001331, 0x330000FF, 0x00CC3333,
        0xFF000033, 0xCCCC0033, 0x0F0000FF, 0x0FF0000F, 0x00F0000F, 0x44443333, 0x66661111,
        0x22221111, 0x136C0013, 0x008C8C63, 0x36C80137, 0x08CEC631, 0x3330000F, 0xF0000333,
        0x00EE1111, 0x88880077, 0x22C0113F, 0x443088CF, 0x0C22F311, 0x03440033, 0x69969009,
        0x9960009F, 0x03303443, 0x00660699, 0xC22C3113, 0x8C0000EF, 0x1300007F, 0xC4003331,
        0x004C1333, 0x22229999, 0x00F0F00F, 0x24929249, 0x29429429, 0xC30C30C3, 0xC03C3C03,
        0x00AA0055, 0xAA0000FF, 0x30300303, 0xC0C03333, 0x90900909, 0xA00A5005, 0xAAA0000F,
        0x0AAA0555, 0xE0E01111, 0x70700707, 0x6660000F, 0x0EE01111, 0x07707007, 0x06660999,
        0x660000FF, 0x00660099, 0x0CC03333, 0x03303003, 0x60000FFF, 0x80807777, 0x10100101,
        0x000A0005, 0x08CE8421,
    ];

    let mask_packed = PATTERN_MASK_TABLE[part_id as usize];
    let mask0 = mask_packed & 0xFFFF;
    let mask1 = mask_packed >> 16;

    if j == 2 {
        !mask0 & !mask1
    } else if j == 0 {
        mask0
    } else {
        mask1
    }
}

pub(crate) fn get_skips(part_id: i32) -> [u32; 3] {
    const SKIP_TABLE: [u32; 128] = [
        0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0,
        0xF0, 0xF0, 0x20, 0x80, 0x20, 0x20, 0x80, 0x80, 0xF0, 0x20, 0x80, 0x20, 0x20, 0x80, 0x80,
        0x20, 0x20, 0xF0, 0xF0, 0x60, 0x80, 0x20, 0x80, 0xF0, 0xF0, 0x20, 0x80, 0x20, 0x20, 0x20,
        0xF0, 0xF0, 0x60, 0x60, 0x20, 0x60, 0x80, 0xF0, 0xF0, 0x20, 0x20, 0xF0, 0xF0, 0xF0, 0xF0,
        0xF0, 0x20, 0x20, 0xF0, 0x3F, 0x38, 0xF8, 0xF3, 0x8F, 0x3F, 0xF3, 0xF8, 0x8F, 0x8F, 0x6F,
        0x6F, 0x6F, 0x5F, 0x3F, 0x38, 0x3F, 0x38, 0x8F, 0xF3, 0x3F, 0x38, 0x6F, 0xA8, 0x53, 0x8F,
        0x86, 0x6A, 0x8F, 0x5F, 0xFA, 0xF8, 0x8F, 0xF3, 0x3F, 0x5A, 0x6A, 0xA8, 0x89, 0xFA, 0xF6,
        0x3F, 0xF8, 0x5F, 0xF3, 0xF6, 0xF6, 0xF8, 0x3F, 0xF3, 0x5F, 0x5F, 0x5F, 0x8F, 0x5F, 0xAF,
        0x5F, 0xAF, 0x8F, 0xDF, 0xF3, 0xCF, 0x3F, 0x38,
    ];

    let skip_packed = SKIP_TABLE[part_id as usize];

    [0, skip_packed >> 4, skip_packed & 15]
}

pub(crate) fn put_bits(data: &mut [u32; 5], pos: &mut u32, bits: u32, v: u32) {
    data[(*pos / 32) as usize] |= v << (*pos % 32);
    if *pos % 32 + bits > 32 {
        data[(*pos / 32 + 1) as usize] |= v >> (32 - *pos % 32);
    }
    *pos += bits;
}

pub(crate) fn data_shl_1bit_from(data: &mut [u32; 5], from_bits: usize) {
    if from_bits < 96 {
        let shifted = (data[2] >> 1) | (data[3] << 31);
        let mask = ((1 << (from_bits - 64)) - 1) >> 1;
        data[2] = (mask & data[2]) | (!mask & shifted);
        data[3] = (data[3] >> 1) | (data[4] << 31);
        data[4] >>= 1;
    } else if from_bits < 128 {
        let shifted = (data[3] >> 1) | (data[4] << 31);
        let mask = ((1 << (from_bits - 96)) - 1) >> 1;
        data[3] = (mask & data[3]) | (!mask & shifted);
        data[4] >>= 1;
    }
}

pub(crate) fn partial_sort_list(list: &mut [i32], length: usize, partial_count: u32) {
    for k in 0..partial_count as usize {
        let mut best_idx = k;
        let mut best_value = list[k];

        for i in k + 1..length {
            if best_value > list[i] {
                best_value = list[i];
                best_idx = i;
            }
        }

        list.swap(k, best_idx);
    }
}

pub(crate) fn opt_endpoints(
    ep: &mut [f32],
    block: &[f32; 64],
    bits: u32,
    qblock: [u32; 2],
    mask: u32,
    channels: usize,
) {
    let levels = 1 << bits;

    let mut atb1 = [0.0; 4];
    let mut sum_q = 0.0;
    let mut sum_qq = 0.0;
    let mut sum = [0.0; 5];

    let mut mask_shifted = mask << 1;
    for k1 in 0..2 {
        let mut qbits_shifted = qblock[k1];
        for k2 in 0..8 {
            let k = k1 * 8 + k2;
            let q = (qbits_shifted & 15) as f32;
            qbits_shifted >>= 4;

            mask_shifted >>= 1;
            if (mask_shifted & 1) == 0 {
                continue;
            }

            let x = (levels - 1) as f32 - q;

            sum_q += q;
            sum_qq += q * q;

            sum[4] += 1.0;
            for p in 0..channels {
                sum[p] += block[k + p * 16];
                atb1[p] += x * block[k + p * 16];
            }
        }
    }

    let mut atb2 = [0.0; 4];
    for p in 0..channels {
        atb2[p] = (levels - 1) as f32 * sum[p] - atb1[p];
    }

    let cxx = sum[4] * sq((levels - 1) as f32) - 2.0 * (levels - 1) as f32 * sum_q + sum_qq;
    let cyy = sum_qq;
    let cxy = (levels - 1) as f32 * sum_q - sum_qq;
    let scale = (levels - 1) as f32 / (cxx * cyy - cxy * cxy);

    for p in 0..channels {
        ep[p] = (atb1[p] * cyy - atb2[p] * cxy) * scale;
        ep[4 + p] = (atb2[p] * cxx - atb1[p] * cxy) * scale;
    }

    if f32::abs(cxx * cyy - cxy * cxy) < 0.001 {
        // flatten
        for p in 0..channels {
            ep[p] = sum[p] / sum[4];
            ep[4 + p] = ep[p];
        }
    }
}

// Principal Component Analysis (PCA) bound
pub(crate) fn get_pca_bound(covar: &[f32; 10], channels: usize) -> f32 {
    const POWER_ITERATIONS: u32 = 4; // Quite approximative, but enough for bounding

    let mut covar_scaled = *covar;
    let inv_var = 1.0 / (256.0 * 256.0);
    for covar_scaled in covar_scaled.iter_mut() {
        *covar_scaled *= inv_var;
    }

    const EPS: f32 = sq(0.001);
    covar_scaled[0] += EPS;
    covar_scaled[4] += EPS;
    covar_scaled[7] += EPS;

    let mut axis = [0.0; 4];
    compute_axis(&mut axis, &covar_scaled, POWER_ITERATIONS, channels);

    let mut a_vec = [0.0; 4];
    if channels == 3 {
        ssymv3(&mut a_vec, &covar_scaled, &axis);
    } else if channels == 4 {
        ssymv4(&mut a_vec, &covar_scaled, &axis);
    }

    let mut sq_sum = 0.0;
    for &value in a_vec[..channels].iter() {
        sq_sum += sq(value);
    }
    let lambda = sq_sum.sqrt();

    let mut bound = covar_scaled[0] + covar_scaled[4] + covar_scaled[7];
    if channels == 4 {
        bound += covar_scaled[9];
    }
    bound -= lambda;

    f32::max(bound, 0.0)
}

pub(crate) fn ssymv3(a: &mut [f32; 4], covar: &[f32; 10], b: &[f32; 4]) {
    a[0] = covar[0] * b[0] + covar[1] * b[1] + covar[2] * b[2];
    a[1] = covar[1] * b[0] + covar[4] * b[1] + covar[5] * b[2];
    a[2] = covar[2] * b[0] + covar[5] * b[1] + covar[7] * b[2];
}

pub(crate) fn ssymv4(a: &mut [f32; 4], covar: &[f32; 10], b: &[f32; 4]) {
    a[0] = covar[0] * b[0] + covar[1] * b[1] + covar[2] * b[2] + covar[3] * b[3];
    a[1] = covar[1] * b[0] + covar[4] * b[1] + covar[5] * b[2] + covar[6] * b[3];
    a[2] = covar[2] * b[0] + covar[5] * b[1] + covar[7] * b[2] + covar[8] * b[3];
    a[3] = covar[3] * b[0] + covar[6] * b[1] + covar[8] * b[2] + covar[9] * b[3];
}

pub(crate) fn compute_axis(
    axis: &mut [f32; 4],
    covar: &[f32; 10],
    power_iterations: u32,
    channels: usize,
) {
    let mut a_vec = [1.0, 1.0, 1.0, 1.0];

    for i in 0..power_iterations {
        if channels == 3 {
            ssymv3(axis, covar, &a_vec);
        } else if channels == 4 {
            ssymv4(axis, covar, &a_vec);
        }

        a_vec[..channels].copy_from_slice(&axis[..channels]);

        // Renormalize every other iteration
        if i % 2 == 1 {
            let mut norm_sq = 0.0;
            for p in 0..channels {
                norm_sq += sq(axis[p]);
            }

            let rnorm = 1.0 / norm_sq.sqrt();
            for value in a_vec[..channels].iter_mut() {
                *value *= rnorm;
            }
        }
    }

    axis[..channels].copy_from_slice(&a_vec[..channels]);
}

pub(crate) fn compute_stats_masked(
    stats: &mut [f32; 15],
    block: &[f32; 64],
    mask: u32,
    channels: usize,
) {
    let mut mask_shifted = mask << 1;
    for k in 0..16 {
        mask_shifted >>= 1;
        let flag = (mask_shifted & 1) as f32;

        let mut rgba = [0.0; 4];
        for p in 0..channels {
            rgba[p] = block[k + p * 16] * flag;
        }
        stats[14] += flag;

        stats[10] += rgba[0];
        stats[11] += rgba[1];
        stats[12] += rgba[2];

        stats[0] += rgba[0] * rgba[0];
        stats[1] += rgba[0] * rgba[1];
        stats[2] += rgba[0] * rgba[2];

        stats[4] += rgba[1] * rgba[1];
        stats[5] += rgba[1] * rgba[2];

        stats[7] += rgba[2] * rgba[2];

        if channels == 4 {
            stats[13] += rgba[3];
            stats[3] += rgba[0] * rgba[3];
            stats[6] += rgba[1] * rgba[3];
            stats[8] += rgba[2] * rgba[3];
            stats[9] += rgba[3] * rgba[3];
        }
    }
}

pub(crate) fn covar_from_stats(covar: &mut [f32; 10], stats: [f32; 15], channels: usize) {
    covar[0] = stats[0] - stats[10] * stats[10] / stats[14];
    covar[1] = stats[1] - stats[10] * stats[11] / stats[14];
    covar[2] = stats[2] - stats[10] * stats[12] / stats[14];

    covar[4] = stats[4] - stats[11] * stats[11] / stats[14];
    covar[5] = stats[5] - stats[11] * stats[12] / stats[14];

    covar[7] = stats[7] - stats[12] * stats[12] / stats[14];

    if channels == 4 {
        covar[3] = stats[3] - stats[10] * stats[13] / stats[14];
        covar[6] = stats[6] - stats[11] * stats[13] / stats[14];
        covar[8] = stats[8] - stats[12] * stats[13] / stats[14];
        covar[9] = stats[9] - stats[13] * stats[13] / stats[14];
    }
}

pub(crate) fn compute_covar_dc_masked(
    covar: &mut [f32; 10],
    dc: &mut [f32; 4],
    block: &[f32; 64],
    mask: u32,
    channels: usize,
) {
    let mut stats = [0.0; 15];
    compute_stats_masked(&mut stats, block, mask, channels);

    // Calculate dc values from stats
    for p in 0..channels {
        dc[p] = stats[10 + p] / stats[14];
    }

    covar_from_stats(covar, stats, channels);
}

pub(crate) fn block_pca_axis(
    axis: &mut [f32; 4],
    dc: &mut [f32; 4],
    block: &[f32; 64],
    mask: u32,
    channels: usize,
) {
    const POWER_ITERATIONS: u32 = 8; // 4 not enough for HQ

    let mut covar = [0.0; 10];
    compute_covar_dc_masked(&mut covar, dc, block, mask, channels);

    const INV_VAR: f32 = 1.0 / (256.0 * 256.0);
    for covar in covar.iter_mut() {
        *covar *= INV_VAR;
    }

    const EPS: f32 = sq(0.001);
    covar[0] += EPS;
    covar[4] += EPS;
    covar[7] += EPS;
    covar[9] += EPS;

    compute_axis(axis, &covar, POWER_ITERATIONS, channels);
}

pub(crate) fn block_pca_bound_split(
    block: &[f32; 64],
    mask: u32,
    full_stats: [f32; 15],
    channels: usize,
) -> f32 {
    let mut stats = [0.0; 15];
    compute_stats_masked(&mut stats, block, mask, channels);

    let mut covar1 = [0.0; 10];
    covar_from_stats(&mut covar1, stats, channels);

    for i in 0..15 {
        stats[i] = full_stats[i] - stats[i];
    }

    let mut covar2 = [0.0; 10];
    covar_from_stats(&mut covar2, stats, channels);

    let mut bound = 0.0;
    bound += get_pca_bound(&covar1, channels);
    bound += get_pca_bound(&covar2, channels);

    bound.sqrt() * 256.0
}

pub(crate) fn block_quant(
    qblock: &mut [u32; 2],
    block: &[f32; 64],
    bits: u32,
    ep: &[f32],
    pattern: u32,
    channels: usize,
) -> f32 {
    let mut total_err = 0.0;
    let levels = 1 << bits;

    qblock[0] = 0;
    qblock[1] = 0;

    let mut pattern_shifted = pattern;
    for k in 0..16 {
        let j = (pattern_shifted & 3) as usize;
        pattern_shifted >>= 2;

        let mut proj = 0.0;
        let mut div = 0.0;
        for p in 0..channels {
            let ep_a = ep[8 * j + p];
            let ep_b = ep[8 * j + 4 + p];
            proj += (block[k + p * 16] - ep_a) * (ep_b - ep_a);
            div += sq(ep_b - ep_a);
        }

        proj /= div;

        let q1 = (proj * levels as f32 + 0.5) as i32;
        let q1_clamped = i32::clamp(q1, 1, levels - 1);

        let mut err0 = 0.0;
        let mut err1 = 0.0;
        let w0 = get_unquant_value(bits, q1_clamped - 1);
        let w1 = get_unquant_value(bits, q1_clamped);

        for p in 0..channels {
            let ep_a = ep[8 * j + p];
            let ep_b = ep[8 * j + 4 + p];
            let dec_v0 = (((64 - w0) * ep_a as i32 + w0 * ep_b as i32 + 32) / 64) as f32;
            let dec_v1 = (((64 - w1) * ep_a as i32 + w1 * ep_b as i32 + 32) / 64) as f32;
            err0 += sq(dec_v0 - block[k + p * 16]);
            err1 += sq(dec_v1 - block[k + p * 16]);
        }

        let mut best_err = err1;
        let mut best_q = q1_clamped;
        if err0 < err1 {
            best_err = err0;
            best_q = q1_clamped - 1;
        }

        qblock[k / 8] |= (best_q as u32) << (4 * (k % 8));
        total_err += best_err;
    }

    total_err
}

pub(crate) fn block_segment_core(ep: &mut [f32], block: &[f32; 64], mask: u32, channels: usize) {
    let mut axis = [0.0; 4];
    let mut dc = [0.0; 4];
    block_pca_axis(&mut axis, &mut dc, block, mask, channels);

    let mut ext = [f32::INFINITY, f32::NEG_INFINITY];

    // Find min/max
    let mut mask_shifted = mask << 1;
    for k in 0..16 {
        mask_shifted >>= 1;
        if (mask_shifted & 1) == 0 {
            continue;
        }

        let mut dot = 0.0;
        for p in 0..channels {
            dot += axis[p] * (block[16 * p + k] - dc[p]);
        }

        ext[0] = f32::min(ext[0], dot);
        ext[1] = f32::max(ext[1], dot);
    }

    // Create some distance if the endpoints collapse
    if ext[1] - ext[0] < 1.0 {
        ext[0] -= 0.5;
        ext[1] += 0.5;
    }

    for i in 0..2 {
        for p in 0..channels {
            ep[4 * i + p] = ext[i] * axis[p] + dc[p];
        }
    }
}

pub(crate) fn bc7_code_qblock(
    data: &mut [u32; 5],
    qpos: &mut u32,
    qblock: [u32; 2],
    bits: u32,
    flips: u32,
) {
    let levels = 1 << bits;
    let mut flips_shifted = flips;

    for k1 in 0..2 {
        let mut qbits_shifted = qblock[k1];
        for k2 in 0..8 {
            let mut q = qbits_shifted & 15;
            if (flips_shifted & 1) > 0 {
                q = (levels - 1) - q;
            }

            if k1 == 0 && k2 == 0 {
                put_bits(data, qpos, bits - 1, q);
            } else {
                put_bits(data, qpos, bits, q);
            }
            qbits_shifted >>= 4;
            flips_shifted >>= 1;
        }
    }
}

pub(crate) fn bc7_code_adjust_skip_mode01237(data: &mut [u32; 5], mode: usize, part_id: i32) {
    let pairs = if mode == 0 || mode == 2 { 3 } else { 2 };
    let bits = if mode == 0 || mode == 1 { 3 } else { 2 };

    let mut skips = get_skips(part_id);

    if pairs > 2 && skips[1] < skips[2] {
        skips.swap(1, 2);
    }

    for &k in skips[1..pairs].iter() {
        data_shl_1bit_from(data, 128 + (pairs - 1) - (15 - k as usize) * bits);
    }
}

pub(crate) fn bc7_code_apply_swap_mode456(
    qep: &mut [i32],
    channels: usize,
    qblock: &mut [u32; 2],
    bits: u32,
) {
    let levels = 1 << bits;

    if (qblock[0] & 15) >= levels / 2 {
        for p in 0..channels {
            qep.swap(p, channels + p);
        }

        for value in qblock.iter_mut() {
            *value = (0x11111111 * (levels - 1)) - *value;
        }
    }
}

pub(crate) fn bc7_code_apply_swap_mode01237(
    qep: &mut [i32; 24],
    qblock: [u32; 2],
    mode: usize,
    part_id: i32,
) -> u32 {
    let bits = if mode == 0 || mode == 1 { 3 } else { 2 };
    let pairs = if mode == 0 || mode == 2 { 3 } else { 2 };

    let mut flips = 0;
    let levels = 1 << bits;

    let skips = get_skips(part_id);

    for j in 0..pairs {
        let k0 = skips[j] as usize;
        // Extract 4 bits from qblock at position k0
        let q = (qblock[k0 >> 3] << (28 - (k0 & 7) * 4)) >> 28;

        if q >= levels / 2 {
            for p in 0..4 {
                qep.swap(8 * j + p, 8 * j + 4 + p);
            }

            let pmask = get_pattern_mask(part_id, j as u32);
            flips |= pmask;
        }
    }

    flips
}
