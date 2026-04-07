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

use ser_video::ga_image::{Image, ImageView, PixelFormat, Rect};
use std::cmp::min;
use std::convert::{From};
use std::mem::{swap};

/// Produces a range of specified length.
macro_rules! range { ($start:expr, $len:expr) => { $start .. $start + $len } }

/// Writes blurred version of fragment of `img` (starting at `src_pos` and sized as `dest`) into `dest`.
pub fn apply_box_blur_into(img: &Image, src_pos: [i32; 2], box_radius: u32, iterations: usize, dest: &mut Image) {
    assert!(img.pixel_format() == PixelFormat::Mono8);
    assert!(dest.pixel_format() == PixelFormat::Mono8);
    assert!(img.width() >= dest.width() && img.height() >= dest.height());

    let [src_x, src_y] = src_pos;

    let w = dest.width();
    let h = dest.height();

    box_blur(
        &img.pixels::<u8>()[src_x as usize + src_y as usize * img.bytes_per_line()..],
        dest.pixels_mut::<u8>(),
        w,
        h,
        img.bytes_per_line(),
        box_radius,
        iterations
    );
}

/// Returns a copy of `img` with box blur applied; `img` has to be `Mono8`.
pub fn apply_box_blur(img: &Image, box_radius: u32, iterations: usize) -> Image {
    assert!(img.pixel_format() == PixelFormat::Mono8);
    let mut blurred_img = Image::new(img.width(), img.height(), None, img.pixel_format(), None, false);
    apply_box_blur_into(img, [0, 0], box_radius, iterations, &mut blurred_img);
    blurred_img
}

/// Performs a blurring pass of a range of `length` elements.
///
/// `T` is `u8` or `u32`, `src` points to the range's beginning,
/// `step` is the distance between subsequent elements.
///
fn box_blur_pass<T>(src: &[T], pix_sum: &mut [u32], box_radius: u32, length: usize, step: usize)
    where T: Copy, u32: From<T>
{
    // TODO rewrite loops with iterators to avoid indexed access

    // sum for the first pixel in the current line/row (count the last pixel multiple times if needed)
    let pix_sum_0 = &mut pix_sum[0];
    *pix_sum_0 = (box_radius as u32 + 1) * u32::from(src[0]);

    let mut i = step;
    while i <= box_radius as usize * step {
        *pix_sum_0 += u32::from(src[min(i,  (length - 1) * step)]);
        i += step;
    }

    // starting region
    let mut i = step;
    while i <= min((length - 1) * step, box_radius as usize * step) {
        pix_sum[i] = pix_sum[i - step] - u32::from(src[0]) +
            u32::from(src[min((length - 1) * step, i + box_radius as usize * step)]);
        i += step;
    }

    if length > box_radius as usize {
        // middle region
        i = (box_radius as usize + 1) * step;
        while i < (length - box_radius as usize) * step {
            pix_sum[i] = pix_sum[i - step] - u32::from(src[i - (box_radius as usize + 1) * step]) +
                u32::from(src[i + box_radius as usize * step]);
            i += step;
        }

        // end region
        i = (length - box_radius as usize) * step;
        while i < length * step {
             pix_sum[i] = pix_sum[i - step] -
                u32::from(src[
                    if i > (box_radius as usize + 1) * step { i - (box_radius as usize + 1) * step } else { 0 }
                ]) +
                u32::from(src[min(i + box_radius as usize * step, (length - 1) * step)]);
             i += step;
        }
    }
}

/// Fills `blurred` with box-blurred contents of `src`.
///
/// Both `src` and `blurred` have `width`*`height` elements (8-bit grayscale); `src_line_stride` is the distance between
/// lines in `src` (which may be a part of a larger image). Line stride in `blurred` equals `width`.
///
fn box_blur(
    src: &[u8],
    blurred: &mut [u8],
    width: u32,
    height: u32,
    src_line_stride: usize,
    box_radius: u32,
    iterations: usize
) {
    assert!(iterations > 0);
    assert!(box_radius > 0);

    if width == 0 || height == 0 { return; }

    // First, the 32-bit unsigned sums of neighborhoods are calculated horizontally
    // and (incrementally) vertically. The max value of a (unsigned) sum is:
    //
    //      (2^8-1) * (box_radius*2 + 1)^2
    //
    // In order for it to fit in 32 bits, box_radius must be below ca. 2^11 - 1.

    assert!((box_radius as u32) < (1u32 << 11) - 1);

    // we need 2 summation buffers to act as source/destination (and then vice versa)
    let mut pix_sum_1 = vec![0u32; (width * height) as usize].into_boxed_slice();
    let mut pix_sum_2 = vec![0u32; (width * height) as usize].into_boxed_slice();

    let divisor = (2 * box_radius + 1).pow(2) as u32;

    // For pixels less than 'box_radius' away from image border, assume the off-image neighborhood consists of copies
    // of the border pixel.

    let mut src_array = &mut pix_sum_1[..];
    let mut dest_array = &mut pix_sum_2[..];

    for n in 0..iterations {
        swap(&mut src_array, &mut dest_array);

        // calculate horizontal neighborhood sums
        if n == 0 {
            // special case: in iteration 0 the source is the 8-bit 'src'
            let mut s_offs = 0usize;
            let mut d_offs = 0usize;

            for _ in 0..height {
                box_blur_pass(
                    &src[range!(s_offs, width as usize)],
                    &mut dest_array[range!(d_offs, width as usize)],
                    box_radius, width as usize, 1
                );
                s_offs += src_line_stride;
                d_offs += width as usize;
            }
        } else {
            let mut offs = 0usize;

            for _ in 0..height {
                box_blur_pass(
                    &src_array[range!(offs, width as usize)],
                    &mut dest_array[range!(offs, width as usize)],
                    box_radius, width as usize, 1
                );
                offs += width as usize;
            }
        }

        swap(&mut src_array, &mut dest_array);

        // Calculate vertical neighborhood sums
        for offs in 0..width as usize {
            box_blur_pass(
                &src_array[offs..],
                &mut dest_array[offs..],
                box_radius, height as usize, width as usize
            );
        }

        // Divide to obtain normalized result. We choose not to divide just once after completing all iterations,
        // because the 32-bit intermediate values would overflow in as little as 3 iterations with 8-pixel box radius
        // for an all-white input image. In such case the final sums would be:
        //
        //   255 * ((2*8+1)^2)^3 = 6'155'080'095
        //
        // (where the exponent 3 = number of iterations)

        for i in dest_array.iter_mut() {
            *i /= divisor;
        }
    }

    // 'dest_array' is where the last summation results were stored,
    // now use it as source for producing the final 8-bit image in 'blurred'

    for i in 0 .. (width*height) as usize {
        blurred[i] = dest_array[i] as u8;
    }
}

/// Estimates quality of the specified image (must be mono 8-bit).
///
/// Quality is the sum of differences between input image and its blurred version.
/// In other words, sum of values of the high-frequency component.
/// The sum is normalized by dividing by the number of pixels.
///
/// `pixels` starts at the beginning of a `width`x`height` area
/// in an image with `line_stride` distance between lines.
///
pub fn estimate_quality(original: &ImageView, blurred: &ImageView) -> f32 {
    assert!(original.pixel_format() == PixelFormat::Mono8 && blurred.pixel_format() == PixelFormat::Mono8);
    assert!(original.width() == blurred.width() && original.height() == blurred.height());

    let w = original.width();
    let h = original.height();

    let mut quality = 0.0;

    for y in 0..h {
        let orig_line = original.line_raw(y);
        let blur_line = blurred.line_raw(y);

        for (orig_val, blur_val) in orig_line.iter().zip(blur_line.iter()) {
            quality += i32::abs(*orig_val as i32 - *blur_val as i32) as f32;
        }
    }

    quality / ((w * h) as f32)
}

/// Calculates contrast based on histogram variance and its width; lower value means better contrast.
/// TODO check on more videos if Sobel-based gradient summation is better.
pub fn estimate_contrast(img: &Image, area: &Rect) -> f32 {
    assert!(img.pixel_format() == PixelFormat::Mono8);

    let mut histogram: [usize; 256] = [0; 256];
    let pixels = img.mono8_pixels_from([area.x, area.y]);
    for y in 0..area.height as usize {
        let row_ofs = y * img.bytes_per_line();
        for val in &pixels[row_ofs..row_ofs + area.width as usize] {
            *unsafe { histogram.get_unchecked_mut(*val as usize) } += 1;
        }
    }

    let num_non_zero: usize = histogram.iter().fold(0, |acc, &x| if x > 0 { acc + 1} else { acc });
    let first_non_zero = *histogram.iter().find(|&&x| x > 0).unwrap() as f64;

    // use the variance formula with offset (`first_non_zero_val`) to avoid catastrophic cancellation
    let variance = (
        histogram.iter().filter(|&&x| x > 0).fold(0.0f64, |acc, &x| acc + (x as f64 - first_non_zero).powi(2))
        -
        histogram.iter().filter(|&&x| x > 0).fold(0.0f64, |acc, &x| acc + (x as f64 - first_non_zero)).powi(2) / num_non_zero as f64
    ) / num_non_zero as f64;

    let first_non_zero_idx = histogram.iter().enumerate().find(|(_, &x)| x > 0).unwrap().0;
    let last_non_zero_idx = histogram.len() - 1 - histogram.iter().rev().enumerate().find(|(_, &x)| x > 0).unwrap().0;

    let histogram_width = last_non_zero_idx - first_non_zero_idx + 1;

    variance.sqrt() as f32 / histogram_width as f32 // TODO Use sum with fine-tuned weights?
}
