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

mod blk_matching;
mod filters;
mod ser_img_provider;

pub mod blk_matching_alignment;

use cgmath::Vector2;
use ser_video::ga_image::{Image, Rect};
use std::error::Error;

pub use filters::{apply_box_blur_into, estimate_contrast, estimate_quality};
pub use ser_img_provider::SerImageProvider;

pub type LongTaskResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct IntPowerOf2(pub i32);

impl IntPowerOf2 {
    pub fn new(power: i32) -> IntPowerOf2 { IntPowerOf2(power) }

    pub fn from_int(value: u32) -> IntPowerOf2 {
        assert!(is_power_of_2(value as usize));
        IntPowerOf2(value.ilog2() as i32)
    }

    pub fn as_float(&self) -> f32 {
        if self.0 >= 0 { 2i32.pow(self.0 as u32) as f32 } else { 2.0f32.powi(self.0) }
    }

    pub fn half(&self) -> IntPowerOf2 {
        IntPowerOf2(self.0 - 1)
    }
}

#[derive(Copy, Clone)]
pub struct NonNegPowerOf2(pub u32);

impl NonNegPowerOf2 {
    pub fn new(power: u32) -> NonNegPowerOf2 { NonNegPowerOf2(power) }

    pub fn from_int(value: u32) -> NonNegPowerOf2 {
        assert!(is_power_of_2(value as usize));
        NonNegPowerOf2(value.ilog2())
    }

    pub fn as_int(&self) -> u32 { 2u32.pow(self.0) }

    pub fn half(&self) -> NonNegPowerOf2 {
        if self.0 == 0 { self.clone() } else { NonNegPowerOf2(self.0 - 1) }
    }
}

pub trait ImageProvider {
    /// Resets to the first image.
    fn reset(&mut self);

    /// Returns `None` if no more images.
    fn next(&mut self) -> Option<Result<Image, Box<dyn Error + Send + Sync>>>;

    /// Returns `None` if no more images.
    fn next_roi(&mut self, roi: Rect) -> Option<Result<Image, Box<dyn Error + Send + Sync>>>;

    fn num_images(&self) -> usize;
}

pub struct ProgressMsg {
    pub info: Option<String>,
    pub step: usize, // 1-based
    pub total_steps: usize
}

pub type Cancel = ();

/// List of image translations rel. to the first one; first element is always [0, 0].
pub type AlignmentResult = Vec<Vector2<f32>>;

pub(in crate::core) fn is_power_of_2(n: usize) -> bool {
    n.count_ones() == 1
}
