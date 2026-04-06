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

use cgmath::{EuclideanSpace, Vector2, Point2};
use crate::{core, operations::{PROGRESS_CHARS, PROGRESS_BAR_TEMPLATE}};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use ser_video::ga_image::{Image, ImageView, PixelFormat, Rect};
use std::{error::Error, io::Write, path::{Path, PathBuf}};

const QUAL_EST_BOX_BLUR_R: u32 = 3; // TODO make configurable
pub const QUAL_EST_BOX_BLUR_ITERS: usize = 3; // result of 3 iterations is close to a Gaussian blur

struct QualityData {
    whole_frame_quality: Vec<(usize, f32)>,
    best_frags_composite: Image
}

fn make_output_path(input_file: &Path, suffix: &Path, output_dir: &Path) -> PathBuf {
    let mut s = input_file.file_stem().unwrap().to_os_string();
    s.extend(suffix);
    output_dir.join(s)
}

pub fn quality_analysis(args: &crate::commands::QualityAnalysis) -> Result<(), Box<dyn Error + Send + Sync>> {
    let translations = stabilize_video(args)?;

    let q_data = analyze_quality(args, &translations)?;

    save_quality_stats(args, &q_data.whole_frame_quality)?;

    let worst_frame_idx = q_data.whole_frame_quality.iter().min_by(|(_, q1), (_, q2)| q1.total_cmp(q2)).unwrap().0;
    let best_frame_idx = q_data.whole_frame_quality.iter().max_by(|(_, q1), (_, q2)| q1.total_cmp(q2)).unwrap().0;

    log::info!("best frame: {}, worst frame: {}", best_frame_idx + 1, worst_frame_idx + 1);

    let mut reader = open_ser_video(&args.input_file)?;
    reader.read_frame(best_frame_idx)?
        .view().save(
            make_output_path(
                &args.input_file,
                &Path::new(&format!("_best_{:05}.tif", best_frame_idx + 1)),
                &args.output_dir
            ),
            ser_video::ga_image::FileType::Auto
        ).map_err(|e| format!("{:?}", e))?;

    reader.read_frame(worst_frame_idx)?
        .view().save(
            make_output_path(
                &args.input_file,
                &Path::new(&format!("_worst_{:05}.tif", worst_frame_idx + 1)),
                &args.output_dir
            ),
            ser_video::ga_image::FileType::Auto
        ).map_err(|e| format!("{:?}", e))?;

    q_data.best_frags_composite.view().save(
        &make_output_path(
            &args.input_file,
            &Path::new("_best_frag.tif"),
            &args.output_dir
        ),
        ser_video::ga_image::FileType::Auto
    ).map_err(|e| format!("{:?}", e))?;

    Ok(())
}

fn updiv(a: usize, b: usize) -> usize {
    (a + b - 1) / b
}

fn analyze_quality(args: &crate::commands::QualityAnalysis, translations: &[Vector2<f32>])
-> Result<QualityData, Box<dyn Error + Sync + Send>> {
    let mut reader = open_ser_video(&args.input_file)?;
    let mdata = reader.metadata();
    let r_intr = find_intersection((mdata.width, mdata.height), &translations).ok_or("intersection is empty")?;

    log::info!("analyzing quality...");
    let t_start = std::time::Instant::now();

    let mut whole_frame_quality: Vec<(usize, f32)> = vec![];

    let asize = args.area_size as usize;
    let num_areas_horz = updiv(r_intr.width as usize, asize);
    let num_areas_vert = updiv(r_intr.height as usize, asize);

    // Note that the quality estimation areas in the last column and row may be smaller than `asize`,
    // if `width` and/or `height` are not multiples of `asize`.

    #[derive(Clone)]
    struct AreaQuality {
        quality: f32,
        blurred: Option<Image>,
        best: Option<Image>
    }
    // contains frame fragment with best quality for given area
    let mut area_quality = vec![AreaQuality{ quality: 0.0, blurred: None, best: None }; num_areas_horz * num_areas_vert];

    // composite of best-quality areas from the whole video
    let mut best_frags_composite = Image::new(r_intr.width, r_intr.height, None, mdata.pix_fmt, None, true);

    let pbar = ProgressBar::new(mdata.num_images as u64);
    pbar.set_style(ProgressStyle::with_template(PROGRESS_BAR_TEMPLATE).unwrap().progress_chars(PROGRESS_CHARS));
    pbar.set_message("Quality analysis");

    for frame_idx in 0..mdata.num_images {
        let pos = Point2::new(r_intr.x, r_intr.y) - Point2::from_vec(translations[frame_idx].cast::<i32>().unwrap());
        let img = reader.read_frame(frame_idx)?.fragment_copy(&[pos.x, pos.y], r_intr.width, r_intr.height, true);
        let img8 = img.convert_pix_fmt(PixelFormat::Mono8, None);

        let total_q: f32 = area_quality.par_iter_mut().enumerate().map(|(area_idx, entry)| {
            let area_col = area_idx % num_areas_horz;
            let area_row = area_idx / num_areas_horz;
            let area_x = area_col * asize;
            let area_y = area_row * asize;

            let view_rect = Rect{
                x: area_x as i32,
                y: area_y as i32,
                width: (asize as u32).min(r_intr.width - area_x as u32),
                height: (asize as u32).min(r_intr.height - area_y as u32)
            };

            if entry.blurred.is_none() {
                entry.blurred =
                    Some(Image::new(view_rect.width, view_rect.height, None, PixelFormat::Mono8, None, true));
            }

            core::apply_box_blur_into(
                &img8,
                [view_rect.x, view_rect.y],
                QUAL_EST_BOX_BLUR_R,
                QUAL_EST_BOX_BLUR_ITERS,
                entry.blurred.as_mut().unwrap()
            );

            if entry.best.is_none() {
                entry.best = Some(Image::new(view_rect.width, view_rect.height, None, mdata.pix_fmt, None, true));
            }

            let q = core::estimate_quality(
                &ImageView::new(&img8, Some(view_rect)),
                &entry.blurred.as_ref().unwrap().view()
            );
            if q > entry.quality {
                entry.quality = q;
                img.convert_pix_fmt_of_subimage_into(
                    entry.best.as_mut().unwrap(),
                    [view_rect.x, view_rect.y],
                    [0, 0],
                    view_rect.width,
                    view_rect.height,
                    None
                );
            }

            q
        }).sum();

        whole_frame_quality.push((frame_idx + 1, total_q));

        pbar.set_position(frame_idx as u64 + 1);
    }

    for (area_idx, aqual) in area_quality.iter().enumerate() {
        let area_col = area_idx % num_areas_horz;
        let area_row = area_idx / num_areas_horz;
        let area_x = area_col * asize;
        let area_y = area_row * asize;

        let view_rect = Rect{
            x: area_x as i32,
            y: area_y as i32,
            width: (asize as u32).min(r_intr.width - area_x as u32),
            height: (asize as u32).min(r_intr.height - area_y as u32)
        };

        aqual.best.as_ref().unwrap().convert_pix_fmt_of_subimage_into(
            &mut best_frags_composite,
            [0, 0],
            [view_rect.x, view_rect.y],
            view_rect.width,
            view_rect.height,
            None
        );
    }

    log::info!("quality analysis completed in {:.02} s", t_start.elapsed().as_secs_f32());

    Ok(QualityData{ whole_frame_quality, best_frags_composite })
}

/// Takes list of (frame index, quality).
fn save_quality_stats(args: &crate::commands::QualityAnalysis, whole_frame_quality: &[(usize, f32)])
-> Result<(), Box<dyn Error + Sync + Send>> {
    let save_as_csv = |output_path: &Path, data: &[(usize, f32)]| -> Result<(), Box<dyn Error + Sync + Send>> {
        let mut writer = std::io::BufWriter::new(std::fs::File::create(output_path)?);

        writeln!(&mut writer, "frame_index;quality")?;
        for entry in data {
            writeln!(&mut writer, "{};{:.02}", entry.0, entry.1)?;
        }

        Ok(())
    };

    save_as_csv(
        &make_output_path(&args.input_file, &Path::new("_quality.csv"), &args.output_dir),
        whole_frame_quality
    )?;

    let mut sorted = whole_frame_quality.to_vec();
    sorted.sort_by(|&(_, q1), &(_, q2)| { q1.total_cmp(&q2) });

    save_as_csv(
        &make_output_path(&args.input_file, &Path::new("_quality_sorted.csv"), &args.output_dir),
        &sorted
    )?;

    Ok(())
}

/// Returns frame translations (first element is always [0, 0]).
fn stabilize_video(args: &crate::commands::QualityAnalysis) -> Result<Vec<Vector2<f32>>, Box<dyn Error + Sync + Send>> {
    let mut img_provider = core::SerImageProvider::new(open_ser_video(&args.input_file)?);
    let num_images = img_provider.num_images();

    let ref_blk = choose_stabilization_anchor(img_provider.next().ok_or("no frames found")??, 64/*TODO take from args*/);
    log::info!(
        "stabilization anchor: ({}, {}) {}x{}",
        ref_blk.x - ref_blk.width as i32 / 2,
        ref_blk.y - ref_blk.height as i32 / 2,
        ref_blk.width,
        ref_blk.height
    );

    let (progress_send, progress_recv) = crossbeam::channel::bounded::<core::ProgressMsg>(1);
    let (result_send, result_recv) = crossbeam::channel::bounded::<core::LongTaskResult<core::AlignmentResult>>(1);

    let mut join_handle = Some(std::thread::spawn(move || { core::blk_matching_alignment::worker(
        progress_send,
        result_send,
        ref_blk,
        16,
        img_provider
    ); }));

    let pb = ProgressBar::new(num_images as u64);
    pb.set_style(ProgressStyle::with_template(PROGRESS_BAR_TEMPLATE).unwrap().progress_chars(PROGRESS_CHARS));
    pb.set_message("Stabilizing video");

    loop {
        match progress_recv.recv() {
            Err(_) => break,
            Ok(progress) => {
                pb.set_position(progress.step as u64);
            }
        }
    }

    let translations = result_recv.recv()??;
    let _ = join_handle.take().unwrap().join();

    Ok(translations)
}

fn open_ser_video(file_path: &std::path::Path) -> Result<ser_video::SerVideoReader, Box<dyn Error + Send + Sync>> {
    ser_video::SerVideoReader::from_path(file_path)
        .map_err(|e| format!("failed to open SER video {}: {}", file_path.to_string_lossy(), e).into())
}

fn choose_stabilization_anchor(img: Image, blk_size: u32) -> Rect {
    let img = img.into_pix_fmt(PixelFormat::Mono8, None);
    let w = img.width();
    let h = img.height();
    assert!(blk_size <= w / 4 && blk_size <= h / 4);

    struct BestContrast {
        contrast: f32,
        pos: (u32, u32)
    }
    let mut best = BestContrast{ contrast: f32::MAX, pos: (0, 0) };

    let mut y = h / 4;
    while y <= 3 * h / 4 - blk_size {
        let mut x = w / 4;
        while x <= 3 * w / 4 - blk_size {
            let contrast =
                core::estimate_contrast(&img, &Rect{ x: x as i32, y: y as i32, width: blk_size, height: blk_size});

            if contrast < best.contrast { best = BestContrast{ contrast, pos: (x, y) } }
            x += blk_size / 2;
        }
        y += blk_size / 2;
    }

    Rect{ x: best.pos.0 as i32, y: best.pos.1 as i32, width: blk_size, height: blk_size }
}

/// Finds sequence images' intersection given their translation vectors.
fn find_intersection(img_size: (u32, u32), translations: &[Vector2<f32>]) -> Option<Rect> {
    let mut r = Rect{ x: 0, y: 0, width: img_size.0, height: img_size.1 };

    for t in translations.iter().map(|t| t.cast::<i32>().unwrap()) {
        match r.intersection(&Rect{ x: t.x, y: t.y, width: img_size.0, height: img_size.1 }) {
            Some(i) => r = i,
            None => return None
        }
    }

    Some(r)
}
