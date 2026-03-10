// Copyright 1986-2026 Altair Engineering Inc.
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::path::Path;
use crate::model::*;

// ---------------------------------------------------------------------------
// Keyword → element kind + node count
// ---------------------------------------------------------------------------
fn elem_kind(etype: &str) -> Option<(ElemKind, usize)> {
    match etype {
        "S3" | "S3R" | "M3D3" | "R3D3" => Some((ElemKind::Sh3n, 3)),
        "S4" | "S4R" | "R3D4" | "M3D4R" => Some((ElemKind::Shell, 4)),
        "C3D6" | "SC6R" => Some((ElemKind::Penta, 6)),
        "COH3D6" => Some((ElemKind::Cohesive, 6)),
        "C3D4" => Some((ElemKind::Tetra4, 4)),
        "C3D8" | "C3D8R" | "C3D8I" | "SC8R" | "COH3D8" => Some((ElemKind::Brick, 8)),
        "C3D10" | "C3D10M" => Some((ElemKind::Tetra10, 10)),
        "SPRINGA" | "CONN3D2" => Some((ElemKind::Spring, 2)),
        "MASS" | "DCOUP3D" => Some((ElemKind::Mass, 1)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Abaqus named boundary conditions → DOF mask [TX,TY,TZ,RX,RY,RZ]
// ---------------------------------------------------------------------------
fn named_bc(name: &str) -> Option<[u8; 6]> {
    match name {
        "ENCASTRE" | "ENCASTR" => Some([1, 1, 1, 1, 1, 1]),
        "PINNED"               => Some([1, 1, 1, 0, 0, 0]),
        "XSYMM"               => Some([1, 0, 0, 0, 1, 1]),
        "YSYMM"               => Some([0, 1, 0, 1, 0, 1]),
        "ZSYMM"               => Some([0, 0, 1, 1, 1, 0]),
        _                     => None,
    }
}

// ---------------------------------------------------------------------------
// Parse "KEY=VALUE, KEY2=VALUE2" fragment into a map (keys uppercased)
// ---------------------------------------------------------------------------
fn params(after_kw: &str) -> HashMap<String, String> {
    after_kw
        .split(',')
        .filter_map(|p| {
            let (k, v) = p.split_once('=')?;
            Some((k.trim().to_ascii_uppercase(), v.trim().to_string()))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Expand a GENERATE range or plain id list from data lines
// ---------------------------------------------------------------------------
fn expand_ids(data: &[String], generate: bool) -> Vec<u32> {
    let flat: Vec<u32> = data
        .iter()
        .flat_map(|l| l.split(',').filter_map(|v| v.trim().parse::<u32>().ok()))
        .collect();
    if generate && flat.len() >= 2 {
        let step = flat.get(2).copied().unwrap_or(1).max(1);
        (flat[0]..=flat[1]).step_by(step as usize).collect()
    } else {
        flat
    }
}

// ---------------------------------------------------------------------------
// Resolve nested elset/nset references (one level deep is enough)
// ---------------------------------------------------------------------------
fn resolve_sets(sets: &mut Vec<(String, Vec<u32>)>) {
    // Build snapshot: name → ids (numeric only; string references are not yet supported)
    let snapshot: HashMap<String, Vec<u32>> = sets
        .iter()
        .map(|(n, v)| (n.clone(), v.clone()))
        .collect();
    // TODO: a full implementation would do a second pass to expand named references.
    // For now numeric ids are expanded at parse time and string refs are silently skipped.
    let _ = snapshot;
}

// ---------------------------------------------------------------------------
// Load raw lines, honouring *INCLUDE directives
// ---------------------------------------------------------------------------
fn load_lines(path: &Path) -> Vec<String> {
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => { eprintln!("Cannot read {}: {e}", path.display()); return vec![]; }
    };
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut out = Vec::new();
    for line in src.lines() {
        let trimmed = line.trim();
        // Abaqus comment
        if trimmed.starts_with("**") { continue; }
        // *INCLUDE, INPUT=path
        if trimmed.to_ascii_uppercase().starts_with("*INCLUDE") {
            if let Some((_, rhs)) = trimmed.split_once('=') {
                let inc = dir.join(rhs.trim());
                out.extend(load_lines(&inc));
                continue;
            }
        }
        out.push(line.to_string());
    }
    out
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------
pub fn parse(path: &Path) -> Model {
    let raw = load_lines(path);
    let mut model = Model { run_time: 1.0, dt: 5e-7, ..Default::default() };

    // Track mutable state across blocks
    let mut cur_mat = String::new();
    let mut cur_friction = 0.0f64;

    let mut i = 0usize;
    while i < raw.len() {
        let line = raw[i].trim();
        if line.is_empty() { i += 1; continue; }
        if !line.starts_with('*') { i += 1; continue; }

        // Split into keyword part and parameter part
        let (kw_part, param_part) = line[1..].split_once(',').unwrap_or((&line[1..], ""));
        let kw = kw_part.trim().to_ascii_uppercase();
        let param_up = param_part.to_ascii_uppercase(); // raw upper-case params for flag checks
        let p = params(param_part);

        // Collect data lines (non-keyword, non-empty)
        i += 1;
        let data_start = i;
        while i < raw.len() && !raw[i].trim().starts_with('*') {
            i += 1;
        }
        let data: Vec<String> = raw[data_start..i]
            .iter()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();

        // -------------------------------------------------------------------
        match kw.as_str() {
            // ---- Nodes ----------------------------------------------------
            kw if kw.starts_with("NODE")
                && !matches!(kw, "NODE OUTPUT" | "NODE PRINT" | "NODE FILE" | "NODE RESPONSE") =>
            {
                for l in &data {
                    let f: Vec<&str> = l.splitn(6, ',').collect();
                    if f.len() < 4 { continue; }
                    let Ok(id) = f[0].trim().parse::<u32>() else { continue };
                    let Ok(x) = f[1].trim().parse::<f64>() else { continue };
                    let Ok(y) = f[2].trim().parse::<f64>() else { continue };
                    let Ok(z) = f[3].trim().parse::<f64>() else { continue };
                    model.nodes.push(Node { id, x, y, z });
                }
            }

            // ---- Elements -------------------------------------------------
            "ELEMENT" => {
                let etype = p.get("TYPE").map(|s| s.to_ascii_uppercase()).unwrap_or_default();
                let elset = p.get("ELSET").cloned().unwrap_or_else(|| etype.clone());
                if let Some((kind, nn)) = elem_kind(&etype) {
                    // Flatten multi-line element data (e.g. C3D10 spans 2 lines)
                    let flat: Vec<u32> = data
                        .iter()
                        .flat_map(|l| l.split(',').filter_map(|v| v.trim().parse::<u32>().ok()))
                        .collect();
                    let stride = nn + 1;
                    let elems = flat
                        .chunks(stride)
                        .filter(|c| c.len() == stride)
                        .map(|c| Elem { id: c[0], nodes: c[1..].to_vec() })
                        .collect();
                    let _ = nn; // node count used only during element parsing above
                    model.elem_blocks.push(ElemBlock { kind, elset, elems });
                } else {
                    eprintln!("Warning: unsupported element type '{etype}'");
                }
            }

            // ---- Node sets ------------------------------------------------
            "NSET" => {
                let name = p.get("NSET").cloned().unwrap_or_default();
                // GENERATE is a flag keyword (no = value), check the raw param string
                let gen = p.contains_key("GENERATE") || param_up.contains("GENERATE");
                let ids = expand_ids(&data, gen);
                if let Some(pos) = model.nsets.iter().position(|(n, _)| n == &name) {
                    model.nsets[pos].1.extend(ids);
                } else {
                    model.nsets.push((name, ids));
                }
            }

            // ---- Element sets ---------------------------------------------
            "ELSET" => {
                let name = p.get("ELSET").cloned().unwrap_or_default();
                let gen = p.contains_key("GENERATE") || param_up.contains("GENERATE");
                let ids = expand_ids(&data, gen);
                if let Some(pos) = model.elsets.iter().position(|(n, _)| n == &name) {
                    model.elsets[pos].1.extend(ids);
                } else {
                    model.elsets.push((name, ids));
                }
            }

            // ---- Material definition -------------------------------------
            "MATERIAL" => {
                cur_mat = p.get("NAME").cloned().unwrap_or_default();
                if !model.materials.iter().any(|(n, _)| n == &cur_mat) {
                    model.materials.push((cur_mat.clone(), Material::default()));
                }
            }
            "DENSITY" => {
                let v: f64 = data.first()
                    .and_then(|l| l.split(',').next())
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0.0);
                if let Some((_, m)) = model.materials.iter_mut().find(|(n, _)| n == &cur_mat) {
                    m.rho = v;
                }
            }
            kw if kw.starts_with("ELASTIC") && !kw.contains("TRACTION") => {
                let vals: Vec<f64> = data.first()
                    .map(|l| l.split(',').filter_map(|v| v.trim().parse().ok()).collect())
                    .unwrap_or_default();
                if vals.len() >= 2 {
                    if let Some((_, m)) = model.materials.iter_mut().find(|(n, _)| n == &cur_mat) {
                        m.e = vals[0]; m.nu = vals[1];
                    }
                }
            }
            "PLASTIC" => {
                if let Some((_, m)) = model.materials.iter_mut().find(|(n, _)| n == &cur_mat) {
                    for l in &data {
                        let pts: Vec<f64> = l.split(',').filter_map(|v| v.trim().parse().ok()).collect();
                        if pts.len() >= 2 { m.plastic.push((pts[0], pts[1])); }
                    }
                }
            }
            kw if kw.starts_with("HYPERELASTIC") && param_up.contains("NEO HOOKE") => {
                let vals: Vec<f64> = data.first()
                    .map(|l| l.split(',').filter_map(|v| v.trim().parse().ok()).collect())
                    .unwrap_or_default();
                if let Some(c10) = vals.first() {
                    if let Some((_, m)) = model.materials.iter_mut().find(|(n, _)| n == &cur_mat) {
                        m.neo_hooke_c10 = Some(*c10);
                    }
                }
            }

            // ---- Sections → properties ------------------------------------
            kw if kw.starts_with("SHELL SECTION") => {
                let thk = data.first()
                    .and_then(|l| l.split(',').next())
                    .and_then(|s| s.trim().parse::<f64>().ok())
                    .unwrap_or(1.0);
                model.sections.push(Section {
                    elset: p.get("ELSET").cloned().unwrap_or_default(),
                    material: p.get("MATERIAL").cloned().unwrap_or_default(),
                    kind: SectionKind::Shell(thk),
                });
            }
            kw if kw.starts_with("MEMBRANE SECTION") => {
                let thk = data.first()
                    .and_then(|l| l.split(',').next())
                    .and_then(|s| s.trim().parse::<f64>().ok())
                    .unwrap_or(1.0);
                model.sections.push(Section {
                    elset: p.get("ELSET").cloned().unwrap_or_default(),
                    material: p.get("MATERIAL").cloned().unwrap_or_default(),
                    kind: SectionKind::Membrane(thk),
                });
            }
            kw if kw.starts_with("SOLID SECTION") => {
                model.sections.push(Section {
                    elset: p.get("ELSET").cloned().unwrap_or_default(),
                    material: p.get("MATERIAL").cloned().unwrap_or_default(),
                    kind: SectionKind::Solid,
                });
            }
            kw if kw.starts_with("COHESIVE SECTION") => {
                model.sections.push(Section {
                    elset: p.get("ELSET").cloned().unwrap_or_default(),
                    material: p.get("MATERIAL").cloned().unwrap_or_default(),
                    kind: SectionKind::Cohesive,
                });
            }

            // ---- Boundary conditions --------------------------------------
            "BOUNDARY" => {
                let amp = p.get("AMPLITUDE").cloned().unwrap_or_default();
                for l in &data {
                    let f: Vec<&str> = l.split(',').map(str::trim).collect();
                    if f.is_empty() { continue; }
                    let nset = f[0].to_string();
                    // Named shortcut (ENCASTRE, PINNED…)
                    if let Some(mask) = f.get(1).and_then(|s| named_bc(&s.to_ascii_uppercase())) {
                        model.boundaries.push(Boundary { nset, mask, value: 0.0, amplitude: amp.clone() });
                        continue;
                    }
                    let dof1 = f.get(1).and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
                    let dof2 = f.get(2).and_then(|s| s.parse::<u32>().ok()).unwrap_or(dof1);
                    let value = f.get(3).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                    if dof1 == 0 { continue; }
                    let mut mask = [0u8; 6];
                    for d in dof1..=dof2.min(6) { mask[(d - 1) as usize] = 1; }
                    model.boundaries.push(Boundary { nset, mask, value, amplitude: amp.clone() });
                }
            }

            // ---- Amplitude (tabular) → /FUNCT ----------------------------
            "AMPLITUDE" => {
                let name = p.get("NAME").cloned().unwrap_or_default();
                let flat: Vec<f64> = data.iter()
                    .flat_map(|l| l.split(',').filter_map(|s| s.trim().parse::<f64>().ok()))
                    .collect();
                let pts: Vec<(f64, f64)> = flat.chunks(2)
                    .filter(|c| c.len() == 2)
                    .map(|c| (c[0], c[1]))
                    .collect();
                model.amplitudes.push((name, pts));
            }

            // ---- Surfaces ------------------------------------------------
            "SURFACE" => {
                let name = p.get("NAME").cloned().unwrap_or_default();
                let node_type = p.get("TYPE").map(|t| t.to_ascii_uppercase() == "NODE").unwrap_or(false);
                let entries: Vec<(String, String)> = data.iter().filter_map(|l| {
                    let (a, b) = l.split_once(',')?;
                    Some((a.trim().to_string(), b.trim().to_string()))
                }).collect();
                model.surfaces.push(Surface { name, entries, node_type });
            }

            // ---- Contact pair ---------------------------------------------
            "FRICTION" => {
                cur_friction = data.first()
                    .and_then(|l| l.split(',').next())
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(cur_friction);
                if p.get("ROUGH").is_some() || param_up.contains("ROUGH") {
                    cur_friction = 0.57; // Rough ≈ high friction coefficient
                }
            }
            "SURFACE INTERACTION" => {
                // Reset friction for the new interaction; FRICTION block follows
                cur_friction = 0.0;
            }
            "CONTACT PAIR" => {
                let contact_name = p.get("INTERACTION").cloned().unwrap_or_default();
                for l in &data {
                    let f: Vec<&str> = l.split(',').map(str::trim).collect();
                    if f.len() >= 2 {
                        model.contacts.push(Contact {
                            name: contact_name.clone(),
                            slave: f[0].to_string(),
                            master: f[1].to_string(),
                            friction: cur_friction,
                        });
                    }
                }
            }

            // ---- Tie constraints ------------------------------------------
            "TIE" => {
                let name = p.get("NAME").cloned().unwrap_or_default();
                for l in &data {
                    let f: Vec<&str> = l.split(',').map(str::trim).collect();
                    if f.len() >= 2 {
                        model.ties.push(Tie {
                            name: name.clone(),
                            slave: f[0].to_string(),
                            master: f[1].to_string(),
                        });
                    }
                }
            }

            // ---- Engine: time step from *DYNAMIC, EXPLICIT ---------------
            kw if kw.contains("DYNAMIC") && (kw.contains("EXPLICIT") || param_up.contains("EXPLICIT")) => {
                if let Some(l) = data.first() {
                    let f: Vec<f64> = l.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                    if let Some(&dt) = f.get(0) { if dt > 0.0 { model.dt = dt; } }
                    if let Some(&rt) = f.get(1) { if rt > 0.0 { model.run_time = rt; } }
                }
            }

            _ => {} // ignore unrecognised keywords
        }
    }

    resolve_sets(&mut model.nsets);
    resolve_sets(&mut model.elsets);
    model
}
