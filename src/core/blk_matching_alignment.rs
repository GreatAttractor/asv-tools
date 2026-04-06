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
use crate::{core, core::{blk_matching as blkm, IntPowerOf2, NonNegPowerOf2}};
use crossbeam::channel::Sender;
use ser_video::ga_image::{Image, PixelFormat, Rect};

fn modify_rect(r: &Rect, delta: i32) -> Rect {
    Rect{
        x: r.x - delta,
        y: r.y - delta,
        width: r.width + 2 * delta as u32,
        height: r.height + 2 * delta as u32
    }
}

pub fn worker(
    progress_send: Sender<core::ProgressMsg>,
    result_send: Sender<core::LongTaskResult<core::AlignmentResult>>,
    mut ref_block_rect: Rect,
    search_radius: u32,
    mut img_provider: Box<dyn core::ImageProvider + Send>
) {
    log::info!("starting alignment using block matching");
    let t_start = std::time::Instant::now();

    img_provider.reset();
    let result = || -> core::LongTaskResult<core::AlignmentResult> {
        let num_images = img_provider.num_images();

        let first_img = img_provider.next();
        if first_img.is_none() { return Err("provider has no images".into()); }
        let first_img = first_img.unwrap()?;

        if !first_img.img_rect().contains_rect(&ref_block_rect) {
            return Err("reference block not within first image".into());
        }

        let ref_blk = first_img.convert_pix_fmt_of_subimage(
            PixelFormat::Mono8,
            ref_block_rect.pos(),
            ref_block_rect.width,
            ref_block_rect.height,
            None
        );

        let mut ref_blk_blurred =
            Image::new(ref_block_rect.width, ref_block_rect.height, None, PixelFormat::Mono8, None, true);
        core::apply_box_blur_into(&ref_blk, [0, 0], 3, 3, &mut ref_blk_blurred);
        // best-quality reference block so far
        let mut best_ref_blk: (f32, Image) =
            (core::estimate_quality(&ref_blk.view(), &ref_blk_blurred.view()), ref_blk);

        let mut translations = vec![Vector2::zero()];

        let mut idx = 1;
        while idx < num_images {
            let initial_step = NonNegPowerOf2::from_int(2);

            let roi_expansion_delta = (search_radius + initial_step.as_int()) as i32;

            let image = img_provider.next_roi(
                modify_rect(&ref_block_rect, roi_expansion_delta)
            );
            if image.is_none() { break; }
            let image = image.unwrap()?.into_pix_fmt(PixelFormat::Mono8, None);

            let t = blkm::determine_translation_by_block_matching(
                &image,
                &best_ref_blk.1,
                Point2::new(roi_expansion_delta, roi_expansion_delta),
                search_radius,
                initial_step,
                // WARNING subpixel alignment cannot be used at the moment, because creation of `new_ref_block` below
                // does not take fractional translation parts into account
                IntPowerOf2::new(0)
            )?;

            let t_prev = *translations.last().as_ref().unwrap();
            let t_from_first = *t_prev - t;
            translations.push(t_from_first);
            log::info!(
                "image {} translated by [{:.01}, {:.01}]",
                idx + 1,
                t_from_first.x, t_from_first.y
            );

            let new_ref_block = image.fragment_copy(
                &[roi_expansion_delta + t.x as i32, roi_expansion_delta + t.y as i32],
                ref_block_rect.width,
                ref_block_rect.height,
                true
            );
            core::apply_box_blur_into(&new_ref_block, [0, 0], 3, 3, &mut ref_blk_blurred);
            let new_ref_blk_q = core::estimate_quality(&new_ref_block.view(), &ref_blk_blurred.view());
            if new_ref_blk_q > best_ref_blk.0  {
                best_ref_blk = (new_ref_blk_q, new_ref_block);
            }

            ref_block_rect.x += t.x as i32;
            ref_block_rect.y += t.y as i32;

            let _ = progress_send.try_send(
                core::ProgressMsg{info: None, step: idx + 1, total_steps: num_images }
            );

            idx += 1;
        }

        Ok(translations)
    }();

    if result.is_ok() { log::info!("alignment completed in {:.02} s", t_start.elapsed().as_secs_f32()); }
    let _ = result_send.send(result);
}
