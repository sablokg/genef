//! Headless export: `cargo run --example export_svg -- <results_dir> <prefix> <out_dir>`
use cafe_gui::{parser::CafeData, svg};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    if a.len() != 4 {
        anyhow::bail!("usage: export_svg <results_dir> <prefix> <out_dir>");
    }
    let d = CafeData::load(Path::new(&a[1]), &a[2])?;
    std::fs::create_dir_all(&a[3])?;
    let n = svg::export_all(&d, Path::new(&a[3]), &svg::ExportOptions::default())?;
    println!("loaded [{}]; wrote {n} SVG files to {}", d.loaded.join(", "), a[3]);
    Ok(())
}
