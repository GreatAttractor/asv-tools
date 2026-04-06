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

use crate::core::ImageProvider;
use ser_video::{ga_image::{Image, Rect}, SerVideoReader};
use std::{error::Error, path::Path};

pub struct SerImageProvider {
    reader: SerVideoReader,
    img_idx: usize,
    metadata: ser_video::SerMetadata
}

impl SerImageProvider {
    pub fn from_path<P: AsRef<Path>>(file_path: P)
    -> Result<Box<dyn ImageProvider + Send>, Box<dyn Error + Send + Sync>> {
        let reader = SerVideoReader::from_path(file_path)?;
        Ok(SerImageProvider::new(reader))
    }

    pub fn new(reader: SerVideoReader) -> Box<dyn ImageProvider + Send> {
        let metadata = reader.metadata();
        Box::new(SerImageProvider{ reader, img_idx: 0, metadata })
    }
}

impl ImageProvider for SerImageProvider {
    fn reset(&mut self) {
        self.img_idx = 0;
    }

    fn next(&mut self) -> Option<Result<Image, Box<dyn Error + Send + Sync>>> {
        if self.img_idx >= self.metadata.num_images { return None; }

        let img_rect = Rect{ x: 0, y: 0, width: self.metadata.width, height: self.metadata.height };
        self.next_roi(img_rect)
    }

    fn next_roi(&mut self, roi: Rect) -> Option<Result<Image, Box<dyn Error + Send + Sync>>> {
        if self.img_idx >= self.metadata.num_images { return None; }

        let idx_to_read = self.img_idx;

        let img = self.reader.read_frame(idx_to_read);
        if img.is_err() { return Some(img); }
        let img = img.unwrap();

        if !img.img_rect().contains_rect(&roi) {
            return Some(Err(
                format!("ROI {:?} outside of image [{}] ({}x{})", roi, idx_to_read, img.width(), img.height()).into()
            ));
        }

        self.img_idx += 1;
        Some(Ok(img.fragment_copy(&roi.pos(), roi.width, roi.height, true)))
    }

    fn num_images(&self) -> usize {
        self.metadata.num_images
    }
}
