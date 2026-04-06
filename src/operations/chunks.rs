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

use crate::operations::{PROGRESS_CHARS, PROGRESS_BAR_TEMPLATE};
use indicatif::{ProgressBar, ProgressStyle};
use ser_video::{SerVideoReader, SerVideoWriter, WriterParameters};
use std::{error::Error, path::Path};

pub fn split_into_chunks(args: &crate::commands::SplitIntoChunks) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut reader = SerVideoReader::from_path(&args.input_file)
        .map_err(|e| format!("failed to open SER video {}: {}", args.input_file.to_string_lossy(), e))?;
    let src_metadata = reader.metadata();

    let first_src_frame = args.first_frame.get() - 1;
    let last_src_frame = (src_metadata.num_images - 1).min(
        if let Some(n) = args.num_chunks {
            first_src_frame + n * args.num_chunk_frames
        } else {
            src_metadata.num_images - 1
        }
    );

    let pbar = ProgressBar::new((last_src_frame - first_src_frame + 1) as u64);
    pbar.set_style(ProgressStyle::with_template(PROGRESS_BAR_TEMPLATE).unwrap().progress_chars(PROGRESS_CHARS));
    pbar.set_message("Reading frames");

    let mut src_frame_idx = args.first_frame.get() - 1;
    let mut chunk_idx = 0;
    while src_frame_idx < src_metadata.num_images
        && match args.num_chunks { Some(n) => chunk_idx < n, None => true }
    {
        let mut output_fname = args.input_file.file_stem().unwrap().to_os_string();
        output_fname.extend(Path::new(&format!("_{:04}.ser", chunk_idx + 1)));

        let mut writer = SerVideoWriter::from_path(
            args.output_dir.join(output_fname),
            &WriterParameters{
                pixel_fmt: src_metadata.pix_fmt,
                width: src_metadata.width,
                height: src_metadata.height
            }
        )?;

        for frame_idx in src_frame_idx..(src_frame_idx + args.num_chunk_frames).min(src_metadata.num_images) {
            let frame = reader.read_frame(frame_idx)?;
            writer.write_frame(&frame)?;
            if frame_idx % 100 == 0 { pbar.set_position((frame_idx - first_src_frame) as u64); }
        }

        src_frame_idx += args.num_chunk_frames;
        chunk_idx += 1;
    }

    Ok(())
}
