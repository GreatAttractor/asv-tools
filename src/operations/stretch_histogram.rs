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

use ser_video::ga_image;
use ser_video::{SerVideoReader, SerVideoWriter, WriterParameters};
use std::{error::Error, io::Write};

fn stretch_histogram_mono8(image: &mut ga_image::Image, low: u8, high: u8) {
    use ga_image::PixelFormat;
    assert!(image.pixel_format() == PixelFormat::Mono8);
    assert!(high > low);

    for y in 0..image.height() {
        let line = image.line_mut::<u8>(y);
        for val in line {
            *val = (0xFF as f32 * u8::saturating_sub(*val, low) as f32 / (high - low) as f32) as u8;
        }
    }
}

pub fn stretch_histogram(args: &crate::commands::StretchHistogram) -> Result<(), Box<dyn Error + Send + Sync>> {
    let input = &args.input_file;

    let mut reader = SerVideoReader::from_path(input)
        .map_err(|e| format!("failed to open SER video {}: {}", args.input_file.to_string_lossy(), e))?;
    let metadata = reader.metadata();
    let mut writer = SerVideoWriter::from_path(
        &args.output_file,
        &WriterParameters{
            pixel_fmt: ga_image::PixelFormat::Mono8,
            width: metadata.width,
            height: metadata.height
        }
    )?;

    for frame_idx in 0..metadata.num_images {
        if frame_idx % 100 == 0 {
            print!("{} ", frame_idx); let _ = std::io::stdout().flush();
        }
        let mut frame = reader.read_frame(frame_idx)?;
        // TODO validate min/max gracefully
        stretch_histogram_mono8(
            &mut frame,
            num_traits::NumCast::from(args.min_val).unwrap(),
            num_traits::NumCast::from(args.max_val).unwrap(),
        );
        writer.write_frame(&frame)?;
    }

    Ok(())
}
