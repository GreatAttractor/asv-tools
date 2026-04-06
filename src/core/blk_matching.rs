// asv-tools (Astro-Video Tools) - operations on astronomical (and other) videos
// Copyright (C) 2026 Filip Szczerek (ga.software@yahoo.com)
//
// This file is part of asv-tools.
//
// This is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License version 3
// as published by the Free Software Foundation.
//
// This software is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with asv-tools. If not, see <http://www.gnu.org/licenses/>.

use cgmath::{Point2, Vector2, Zero};
use crate::core::{IntPowerOf2, NonNegPowerOf2};
use ser_video::ga_image::{Image, PixelFormat, Rect};
use std::{collections::HashMap, error::Error};

struct HashablePoint2(Point2<f32>);

// we do not need random selection of hasher state on construction; use the default
type SumAbsDiffAtPosition = HashMap<HashablePoint2, u64, std::hash::BuildHasherDefault<std::hash::DefaultHasher>>;

fn create_map(capacity: usize) -> SumAbsDiffAtPosition {
    SumAbsDiffAtPosition::with_capacity_and_hasher(capacity, std::hash::BuildHasherDefault::new())
}

impl std::hash::Hash for HashablePoint2 {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // convert negative zero to positive one, so that we stay consistent with Eq/PartialEq (which equate +/-0.0)
        let x = if self.0.x.signum() == 1.0 { self.0.x } else { -self.0.x };
        let y = if self.0.y.signum() == 1.0 { self.0.y } else { -self.0.y };
        state.write_u32(x.to_bits());
        state.write_u32(y.to_bits());
    }
}

impl std::cmp::PartialEq for HashablePoint2 {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl std::cmp::Eq for HashablePoint2 {}


/// Finds translation of `ref_block` relative to `initial_pos` which best matches underlying pixels of `image`.
/// Requires `image` and `ref_block` to be Mono8; `initial_step` must not be be less than `final_step`.
/// Fails if search would go outside of image.
pub fn determine_translation_by_block_matching(
    image: &Image,
    ref_block: &Image,
    initial_pos: Point2<i32>,
    search_radius: u32,
    initial_step: NonNegPowerOf2,
    final_step: IntPowerOf2
) -> Result<Vector2<f32>, Box<dyn Error + Send + Sync>> {
    assert!(image.pixel_format() == PixelFormat::Mono8);
    assert!(ref_block.pixel_format() == PixelFormat::Mono8);
    assert!(final_step.0 <= initial_step.0 as i32);

    if initial_step.as_int() > search_radius { return Ok(Vector2::zero()); }

    // Initial search proceeds as follows:
    //
    //  (assuming: initial_search_radius / initial_step = 3)
    //  @ - initial position
    //  o - additional positions to check
    //
    //    o o o o o o o
    //    o o o o o o o
    //    o o o o o o o
    //    o o o @ o o o
    //    o o o o o o o
    //    o o o o o o o
    //    o o o o o o o
    //
    let num_initial_search_positions = (2 * search_radius / initial_step.as_int() + 1).pow(2) as usize;

    // During refinement steps, we only search around the previous best position.
    // The number of positions to check is up to 25 (less in practice - skipping the already-checked ones
    // from the previous step).
    //
    //   @ - previous best position
    //   . - positions checked in previous step
    //   x - new positions to check
    //
    //      . x . x .
    //      x x x x x
    //      . x @ x .
    //      x x x x x
    //      . x . x .
    //
    let num_refining_search_positions = 25;

    let mut sum_abs_diffs_at_pos = create_map(num_initial_search_positions.max(num_refining_search_positions));

    let mut pos = initial_pos.cast::<f32>().unwrap();
    let mut step = IntPowerOf2(initial_step.0 as i32);
    let mut search_radius = search_radius as f32;

    while step >= final_step {
        let mut min_sum_abs_diffs = u64::MAX;
        let mut best_pos = pos;

        let mut y = pos.y - search_radius;
        while y <= pos.y + search_radius {
            let mut x = pos.x - search_radius;
            while x <= pos.x + search_radius {
                let pos_to_check = Point2::new(x, y);
                let sum_abs_diffs = sum_abs_diffs_at_pos
                    .entry(HashablePoint2(pos_to_check))
                    .or_insert(
                        if step.0 >= 0 {
                            calc_sum_abs_diff(image, ref_block, pos_to_check.cast::<i32>().unwrap())?
                        } else {
                            calc_sum_abs_diff_subpixel(image, ref_block, pos_to_check)?
                        }
                    );

                if *sum_abs_diffs < min_sum_abs_diffs {
                    min_sum_abs_diffs = *sum_abs_diffs;
                    best_pos = pos_to_check;
                }

                x += step.as_float();
            }

            y += step.as_float();
        }


        search_radius = step.as_float();
        step = step.half();
        pos = best_pos;
    }

    Ok(pos - initial_pos.cast::<f32>().unwrap())
}

/// Fails if `ref_block` at `pos` does not fit inside `image`.
fn calc_sum_abs_diff(image: &Image, ref_block: &Image, pos: Point2<i32>) -> Result<u64, Box<dyn Error + Send + Sync>> {
    let mut sum = 0u64;

    if !image.img_rect().contains_rect(
        &Rect{ x: pos.x, y: pos.y, width: ref_block.width(), height: ref_block.height()}
    ) {
        return Err(format!(
            "ref. block ({}x{}) at ({}, {}) outside of image ({}x{})",
            ref_block.width(), ref_block.height(),
            pos.x, pos.y,
            image.width(),
            image.height()
        ).into());
    }

    let x_min = pos.x;
    let x_max = pos.x + ref_block.width() as i32;
    let y_min = pos.y;
    let y_max = pos.y + ref_block.height() as i32;

    assert!(x_min >= 0);
    assert!(x_max <= image.width() as i32);
    assert!(y_min >= 0);
    assert!(y_max <= image.height() as i32);

    let img_pix = image.mono8_pixels_from(pos.into());
    let img_stride = image.bytes_per_line();
    let blk_pix = ref_block.mono8_pixels_from([0, 0]);
    let blk_stride = ref_block.bytes_per_line();

    let mut img_ofs = 0;
    let mut blk_ofs = 0;

    for _ in y_min..y_max {
        let img_range = img_ofs..img_ofs + (x_max - x_min) as usize;
        let blk_range = blk_ofs..blk_ofs + (x_max - x_min) as usize;

        // TODO make sure it compiled to SIMD w/out bounds checks
        // if not, benchmark unchecked access
        for (img_val, blk_val) in (&img_pix[img_range]).iter().zip(&blk_pix[blk_range]) {
            sum += (*img_val as i32 - *blk_val as i32).abs() as u64;
        }

        img_ofs += img_stride;
        blk_ofs += blk_stride;
    }

    Ok(sum)
}

/// Fails if `ref_block` at `pos` does not fit inside `image`.
fn calc_sum_abs_diff_subpixel(image: &Image, ref_block: &Image, pos: Point2<f32>)
-> Result<u64, Box<dyn Error + Send + Sync>> {
    if !image.img_rect().contains_rect(
        &Rect{
            x: pos.x.floor() as i32,
            y: pos.y.floor() as i32,
            width: ref_block.width() + 1,
            height: ref_block.height() + 1
        }
    ) {
        return Err(format!(
            "ref. block ({}x{}) at ({:.3}, {:.3}) outside of image ({}x{})",
            ref_block.width(), ref_block.height(),
            pos.x, pos.y,
            image.width(),
            image.height()
        ).into());
    }

    let img_pix = image.pixels::<u8>();
    let img_stride = image.bytes_per_line();
    let blk_pix = ref_block.pixels::<u8>();
    let blk_stride = ref_block.bytes_per_line();


    // TODO OPTIMIZE
    let pos_lo = (pos.x.floor() as i32, pos.y.floor() as i32);
    let pos_hi = (pos_lo.0 + 1, pos_lo.1 + 1);

    let xf = pos.x - pos_lo.0 as f32;
    let yf = pos.y - pos_lo.1 as f32;

    let mut sum = 0.0f64;

    for y in 0..ref_block.height() as i32 {
        for x in 0..ref_block.width() as i32 {
            let p00 = img_pix[(y + pos_lo.1) as usize * img_stride + (x + pos_lo.0) as usize] as f32;
            let p10 = img_pix[(y + pos_lo.1) as usize * img_stride + (x + pos_hi.0) as usize] as f32;
            let p11 = img_pix[(y + pos_hi.1) as usize * img_stride + (x + pos_hi.0) as usize] as f32;
            let p01 = img_pix[(y + pos_hi.1) as usize * img_stride + (x + pos_lo.0) as usize] as f32;

            let p_interp_y_lo = (1.0 - xf) * p00 + xf * p10;
            let p_interp_y_hi = (1.0 - xf) * p01 + xf * p11;
            let p_interp = (1.0 - yf) * p_interp_y_lo + yf * p_interp_y_hi;

            sum += (p_interp - blk_pix[(x + y * blk_stride as i32) as usize] as f32).abs() as f64;
        }
    }

    Ok(sum as u64)
}
