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

mod core;
mod operations;

use clap::{Subcommand, Parser};
use std::path::{Path, PathBuf};

pub const COMMIT_HASH: &'static str = include_str!(concat!(env!("OUT_DIR"), "/commit_hash"));
pub const VERSION: &'static str = "0.1.0";

#[derive(Copy, clap::ValueEnum, Clone)]
enum Mode {
    QualityAnalysis,
    SplitIntoChunks,
    // StretchHistogram,
}

mod commands {
    use clap::{Parser};
    use std::path::PathBuf;

    #[derive(Parser)]
    pub struct QualityAnalysis {
        #[arg(short, long)]
        pub input_file: PathBuf,
        #[arg(short, long, default_value = "40")]
        pub area_size: u32,
        #[arg(short, long, default_value = ".")]
        pub output_dir: PathBuf,
    }

    #[derive(Parser)]
    pub struct SplitIntoChunks {
        #[arg(short, long)]
        pub input_file: PathBuf,
        #[arg(long)]
        pub num_chunk_frames: usize,
        #[arg(long)]
        pub num_chunks: Option<usize>,
        #[arg(long, default_value = "1")]
        pub first_frame: std::num::NonZeroUsize, // 1-based
        #[arg(short, long, default_value = ".")]
        pub output_dir: PathBuf,
    }

    #[derive(Parser)]
    pub struct StretchHistogram {
        #[arg(short, long)]
        pub input_file: PathBuf,
        #[arg(short, long)]
        pub output_file: PathBuf,
        #[arg(short, long)]
        pub min_val: f32,
        #[arg(short, long)]
        pub max_val: f32,
    }
}

#[derive(Subcommand)]
enum Command {
    QualityAnalysis(commands::QualityAnalysis),
    SplitIntoChunks(commands::SplitIntoChunks),
    // StretchHistogram(commands::StretchHistogram)
}

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,

    #[arg(long)]
    log_file: Option<PathBuf>,
}

fn main() {
    let initial_msg = format!("Astro-Video Tools v. {} ({})
Copyright (C) Filip Szczerek 2026
This is free software, licensed under GNU GPL version 3, and you are welcome to redistribute \
it under certain conditions (see the LICENSE file for details). This program comes with NO WARRANTY.",
    VERSION, COMMIT_HASH);
    log::info!("{}", &initial_msg);

    println!("{}\n", initial_msg);

    let args = Args::parse();
    if let Some(log_file) = &args.log_file { set_up_logging(log_file); }

    let result = match args.command {
        Command::QualityAnalysis(s) => operations::quality_analysis(&s),
        Command::SplitIntoChunks(s) => operations::split_into_chunks(&s),
        // Command::StretchHistogram(s) => operations::stretch_histogram(&s),
    };

    match result {
        Err(e) => println!("{}", e),
        Ok(()) => println!("\nCompleted successfully.")
    }
}

fn set_up_logging<P: AsRef<Path>>(log_file: P) {
    simplelog::WriteLogger::init(
        simplelog::LevelFilter::max(),
        simplelog::ConfigBuilder::new()
            .set_target_level(simplelog::LevelFilter::Error)
            .set_time_offset_to_local().unwrap()
            .set_time_format_custom(simplelog::format_description!(
                "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:6]"
            ))
            .build(),
        std::fs::File::create(log_file).unwrap()
    ).unwrap();
}
