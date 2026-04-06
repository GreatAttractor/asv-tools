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

mod chunks;
mod quality;
mod stretch_histogram;

pub use chunks::split_into_chunks;
pub use quality::quality_analysis;
pub use stretch_histogram::stretch_histogram;

const PROGRESS_CHARS: &str = "\u{2589}-";
const PROGRESS_BAR_TEMPLATE: &str = "{msg} {wide_bar:.cyan/blue} frame {pos:>4}/{len:4} [{eta_precise}]";
