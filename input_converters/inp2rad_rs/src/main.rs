// Copyright 1986-2026 Altair Engineering Inc.
// SPDX-License-Identifier: MIT
//
// inp2rad — Abaqus .inp → OpenRadioss .rad converter (Rust edition)
//
// Usage: inp2rad <model.inp>
//
// Produces <model>_0000.rad (starter) and <model>_0001.rad (engine).

mod model;
mod parse;
mod write;

use std::{env, fs, path::Path, process};

fn main() {
    let args: Vec<String> = env::args().collect();
    let input_path = args.get(1).unwrap_or_else(|| {
        eprintln!("Usage: inp2rad <model.inp>");
        process::exit(1);
    });

    let path = Path::new(input_path);
    if !path.exists() {
        eprintln!("File not found: {input_path}");
        process::exit(1);
    }

    let stem = path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("model");

    let dir = path.parent().unwrap_or(Path::new("."));
    let starter_path = dir.join(format!("{stem}_0000.rad"));
    let engine_path  = dir.join(format!("{stem}_0001.rad"));

    eprintln!("inp2rad: reading {input_path}");
    let model = parse::parse(path);
    eprintln!(
        "inp2rad: {} nodes, {} element blocks, {} materials, {} sections",
        model.nodes.len(),
        model.elem_blocks.len(),
        model.materials.len(),
        model.sections.len(),
    );

    let starter = write::write_starter(&model, stem);
    let engine  = write::write_engine(&model, stem);

    fs::write(&starter_path, &starter).unwrap_or_else(|e| {
        eprintln!("Cannot write {}: {e}", starter_path.display());
        process::exit(1);
    });
    fs::write(&engine_path, &engine).unwrap_or_else(|e| {
        eprintln!("Cannot write {}: {e}", engine_path.display());
        process::exit(1);
    });

    eprintln!("inp2rad: wrote {}", starter_path.display());
    eprintln!("inp2rad: wrote {}", engine_path.display());
}
