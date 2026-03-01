//Copyright>
//Copyright> Copyright (C) 1986-2026 Altair Engineering Inc.
//Copyright>
//Copyright> Permission is hereby granted, free of charge, to any person obtaining
//Copyright> a copy of this software and associated documentation files (the "Software"),
//Copyright> to deal in the Software without restriction, including without limitation
//Copyright> the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
//Copyright> sell copies of the Software, and to permit persons to whom the Software is
//Copyright> furnished to do so, subject to the following conditions:
//Copyright>
//Copyright> The above copyright notice and this permission notice shall be included in all
//Copyright> copies or substantial portions of the Software.
//Copyright>
//Copyright> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//Copyright> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//Copyright> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//Copyright> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
//Copyright> WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
//Copyright> IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
//Copyright>

// Rust port of th_to_csv.c
// Converts OpenRadioss time history (T01) files to CSV format.
//
// Usage:
//   th_to_csv <T01File>           => writes <T01File>.csv
//   th_to_csv <T01File> <Output>  => writes <Output>.csv

use std::env;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::process;

// ── low-level binary helpers ──────────────────────────────────────────────────

/// Read exactly `n` bytes from `reader`, returning an error on short read / EOF.
fn read_exact_bytes(reader: &mut impl Read, n: usize) -> io::Result<Vec<u8>> {
    let mut buf = vec![0u8; n];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

/// Read a 4-byte big-endian integer (Fortran record marker).
fn read_eor(reader: &mut impl Read) -> io::Result<i32> {
    let buf = read_exact_bytes(reader, 4)?;
    Ok(ieee_ascii_to_integer(&buf))
}

/// Decode 4 big-endian bytes → i32 (matches IEEE_ASCII_to_integer in C).
fn ieee_ascii_to_integer(b: &[u8]) -> i32 {
    let mut result = (b[0] as i32) << 24
        | (b[1] as i32) << 16
        | (b[2] as i32) << 8
        | (b[3] as i32);
    // handle sign extension for values with high bit set (64-bit compat in C)
    if (result as u32) & 0x8000_0000 != 0 {
        result = result.wrapping_add(-1).wrapping_sub(0xFFFF_FFFFu32 as i32);
    }
    result
}

/// Decode 4 big-endian bytes → f32 (matches IEEE_ASCII_to_real in C).
fn ieee_ascii_to_real(b: &[u8]) -> f32 {
    let sign: f64 = if b[0] & 0x80 != 0 { -1.0 } else { 1.0 };

    let exponent = (((b[0] & 0x7f) as i32) << 1) + (((b[1] & 0x80) as i32) >> 7);
    if exponent == 0 {
        return 0.0_f32;
    }
    let exponent = exponent - 126;

    let shift = 256.0_f64;
    let mut mantissa = (b[1] & 0x7f) as f64;
    mantissa = mantissa * shift + b[2] as f64;
    mantissa = mantissa * shift + b[3] as f64;
    mantissa /= f64::powi(2.0, 24);
    mantissa += 0.5;

    (sign * mantissa * f64::powi(2.0, exponent)) as f32
}

/// Read one integer from the stream.
fn read_i(reader: &mut impl Read) -> io::Result<i32> {
    let buf = read_exact_bytes(reader, 4)?;
    Ok(ieee_ascii_to_integer(&buf))
}

/// Read one f32 from the stream; returns None on EOF/short read.
fn read_r(reader: &mut impl Read) -> Option<f32> {
    let mut buf = [0u8; 4];
    match reader.read_exact(&mut buf) {
        Ok(_) => Some(ieee_ascii_to_real(&buf)),
        Err(_) => None,
    }
}

/// Read `len` characters (stored as individual bytes) into a String,
/// trimming trailing ASCII whitespace/nulls.
fn read_chars(reader: &mut impl Read, len: usize) -> io::Result<String> {
    let buf = read_exact_bytes(reader, len)?;
    let s: String = buf.iter().map(|&c| c as char).collect();
    Ok(s.trim_end().to_string())
}

// ── variable-code → name mapping ─────────────────────────────────────────────

fn var_code_name(code: i32) -> &'static str {
    match code {
        1 => "IE",
        2 => "KE",
        3 => "XMOM",
        4 => "YMOM",
        5 => "ZMOM",
        6 => "MASS",
        7 => "HE",
        8 => "TURBKE",
        9 => "XCG",
        10 => "YCG",
        11 => "ZCG",
        12 => "XXMOM",
        13 => "YYMOM",
        14 => "ZZMOM",
        15 => "IXX",
        16 => "IYY",
        17 => "IZZ",
        18 => "IXY",
        19 => "IYZ",
        20 => "IZX",
        21 => "RIE",
        22 => "KERB",
        23 => "RKERB",
        24 => "RKE",
        25 => "ERODED",
        28 => "HEAT",
        29 => "VX",
        30 => "VY",
        31 => "VZ",
        32 => "PW",
        _ => "empty",
    }
}

// ── file-structure reader ─────────────────────────────────────────────────────

/// Holds all dimension/count information gathered during the pre-read pass.
struct Dimensions {
    thicode: i32,
    title_length: usize,
    nb_glob_var: usize,
    nb_part_var: usize,
    nb_subs_var: usize,
    nb_time_step: usize,
    cpt_data: usize,
    cpt_th_group_names: usize,
    npart_nthpart: usize,
    nthgrp2: usize,
    nummat: usize,
    numgeo: usize,
    nsubs: usize,
    nvar_part: Vec<usize>,
    nbelem_thgrp: Vec<usize>,
    nvar_thgrp: Vec<usize>,
}

/// Skip the file header (TITRE + ivers + optional ADDITIONAL RECORDS).
/// Returns (THICODE, title_length).
fn skip_header(reader: &mut impl Read, thicode: i32) -> io::Result<()> {
    // ivers / date record
    read_eor(reader)?;
    read_chars(reader, 80)?;
    read_eor(reader)?;

    // optional ADDITIONAL RECORDS
    if thicode > 3050 {
        read_eor(reader)?;
        read_i(reader)?;
        read_eor(reader)?;

        read_eor(reader)?;
        read_i(reader)?;
        read_eor(reader)?;

        read_eor(reader)?;
        read_r(reader); // FAC_MASS
        read_r(reader); // FAC_LENGTH
        read_r(reader); // FAC_TIME
        read_eor(reader)?;
    }
    Ok(())
}

/// Read TITRE record; returns (thicode, title_length).
fn read_titre(reader: &mut impl Read) -> io::Result<(i32, usize)> {
    read_eor(reader)?;
    let thicode = read_i(reader)?;
    let title_length = if thicode >= 4021 {
        100
    } else if thicode >= 3041 {
        80
    } else {
        40
    };
    read_chars(reader, 80)?; // model title (fixed 80 chars)
    read_eor(reader)?;
    Ok((thicode, title_length))
}

/// Read HIERARCHY INFO record; returns (npart_nthpart, nummat, numgeo, nsubs, nthgrp2, nglob).
fn read_hierarchy_info(reader: &mut impl Read) -> io::Result<(usize, usize, usize, usize, usize, usize)> {
    read_eor(reader)?;
    let npart_nthpart = read_i(reader)? as usize;
    let nummat       = read_i(reader)? as usize;
    let numgeo       = read_i(reader)? as usize;
    let nsubs        = read_i(reader)? as usize;
    let nthgrp2      = read_i(reader)? as usize;
    let nglob        = read_i(reader)? as usize;
    read_eor(reader)?;
    Ok((npart_nthpart, nummat, numgeo, nsubs, nthgrp2, nglob))
}

// ── PRE-READ PASS ─────────────────────────────────────────────────────────────

fn t01_pre_read(filename: &str) -> io::Result<Dimensions> {
    // ── first mini-pass: just get thicode ────────────────────────────────────
    let f = File::open(filename)?;
    let mut r = BufReader::new(f);

    let (thicode, title_length) = read_titre(&mut r)?;
    skip_header(&mut r, thicode)?;
    let (npart_nthpart, _nummat, _numgeo, _nsubs, _nthgrp2, nglob) =
        read_hierarchy_info(&mut r)?;
    drop(r);

    // ── second pass: count variables ─────────────────────────────────────────
    let f = File::open(filename)?;
    let mut r = BufReader::new(f);

    read_titre(&mut r)?;
    skip_header(&mut r, thicode)?;
    let (npart_nthpart2, nummat, numgeo, nsubs, nthgrp22, _nglob2) =
        read_hierarchy_info(&mut r)?;

    // GLOBAL VAR IDs
    if nglob > 0 {
        read_eor(&mut r)?;
        for _ in 0..nglob {
            read_i(&mut r)?;
        }
        read_eor(&mut r)?;
    }

    // PART DESCRIPTIONS
    let mut nvar_part = Vec::with_capacity(npart_nthpart2);
    let mut nb_part_var = 0usize;
    for _ in 0..npart_nthpart2 {
        read_eor(&mut r)?;
        read_i(&mut r)?;                         // part ID
        read_chars(&mut r, title_length)?;       // name
        read_i(&mut r)?;
        read_i(&mut r)?;
        read_i(&mut r)?;
        let nvar = read_i(&mut r)? as usize;
        read_eor(&mut r)?;
        nvar_part.push(nvar);
        nb_part_var += nvar;
        for j in 0..nvar {
            if j == 0 { read_eor(&mut r)?; }
            read_i(&mut r)?;
            if j == nvar - 1 { read_eor(&mut r)?; }
        }
    }

    // MATER DESCRIPTIONS
    for _ in 0..nummat {
        read_eor(&mut r)?;
        read_i(&mut r)?;
        read_chars(&mut r, title_length)?;
        read_eor(&mut r)?;
    }

    // GEO DESCRIPTIONS
    for _ in 0..numgeo {
        read_eor(&mut r)?;
        read_i(&mut r)?;
        read_chars(&mut r, title_length)?;
        read_eor(&mut r)?;
    }

    // HIERARCHY (SUBSET) DESCRIPTIONS
    let mut nb_subs_var = 0usize;
    for _ in 0..nsubs {
        read_eor(&mut r)?;
        read_i(&mut r)?; // subs ID
        read_i(&mut r)?;
        let nbsubsf = read_i(&mut r)? as usize;
        let nbpartf = read_i(&mut r)? as usize;
        let nvar    = read_i(&mut r)? as usize;
        read_chars(&mut r, title_length)?;
        read_eor(&mut r)?;
        for j in 0..nbsubsf {
            if j == 0 { read_eor(&mut r)?; }
            read_i(&mut r)?;
            if j == nbsubsf - 1 { read_eor(&mut r)?; }
        }
        for j in 0..nbpartf {
            if j == 0 { read_eor(&mut r)?; }
            read_i(&mut r)?;
            if j == nbpartf - 1 { read_eor(&mut r)?; }
        }
        for j in 0..nvar {
            if j == 0 { read_eor(&mut r)?; }
            read_i(&mut r)?;
            if j == nvar - 1 { read_eor(&mut r)?; }
            nb_subs_var += 1;
        }
    }

    // TH GROUP DESCRIPTIONS
    let mut nbelem_thgrp = Vec::with_capacity(nthgrp22);
    let mut nvar_thgrp   = Vec::with_capacity(nthgrp22);
    let mut cpt_th_group_names = 0usize;
    for _ in 0..nthgrp22 {
        read_eor(&mut r)?;
        read_i(&mut r)?;
        read_i(&mut r)?;
        read_i(&mut r)?;
        let nbelem = read_i(&mut r)? as usize;
        let nvar   = read_i(&mut r)? as usize;
        read_chars(&mut r, title_length)?;
        read_eor(&mut r)?;
        nbelem_thgrp.push(nbelem);
        nvar_thgrp.push(nvar);
        for _ in 0..nbelem {
            read_eor(&mut r)?;
            read_i(&mut r)?;
            read_chars(&mut r, title_length)?;
            read_eor(&mut r)?;
            cpt_th_group_names += nvar;
        }
        for j in 0..nvar {
            if j == 0 { read_eor(&mut r)?; }
            read_i(&mut r)?;
            if j == nvar - 1 { read_eor(&mut r)?; }
        }
    }

    // Count time steps and data points
    let mut nb_time_step = 0usize;
    let mut cpt_data = 0usize;
    loop {
        // TIME
        match read_eor(&mut r) {
            Err(_) => break,
            Ok(_) => {}
        }
        match read_r(&mut r) {
            None => break,
            Some(_) => { cpt_data += 1; }
        }
        if read_eor(&mut r).is_err() { break; }
        nb_time_step += 1;

        // GLOBAL VARS
        if nglob > 0 {
            read_eor(&mut r)?;
            for _ in 0..nglob {
                if read_r(&mut r).is_none() { break; }
                cpt_data += 1;
            }
            read_eor(&mut r)?;
        }

        // PART VARS
        if npart_nthpart > 0 {
            let tot: usize = nvar_part.iter().sum();
            if tot > 0 { read_eor(&mut r)?; }
            for &nv in &nvar_part {
                for _ in 0..nv {
                    if read_r(&mut r).is_none() { break; }
                    cpt_data += 1;
                }
            }
            if tot > 0 { read_eor(&mut r)?; }
        }

        // SUBSET VARS
        if nb_subs_var > 0 {
            read_eor(&mut r)?;
            for _ in 0..nb_subs_var {
                if read_r(&mut r).is_none() { break; }
                cpt_data += 1;
            }
            read_eor(&mut r)?;
        }

        // TH GROUP VARS
        for i in 0..nthgrp22 {
            read_eor(&mut r)?;
            for _ in 0..nbelem_thgrp[i] {
                for _ in 0..nvar_thgrp[i] {
                    if read_r(&mut r).is_none() { break; }
                    cpt_data += 1;
                }
            }
            read_eor(&mut r)?;
        }
    }

    Ok(Dimensions {
        thicode,
        title_length,
        nb_glob_var: nglob,
        nb_part_var,
        nb_subs_var,
        nb_time_step,
        cpt_data,
        cpt_th_group_names,
        npart_nthpart: npart_nthpart2,
        nthgrp2: nthgrp22,
        nummat,
        numgeo,
        nsubs,
        nvar_part,
        nbelem_thgrp,
        nvar_thgrp,
    })
}

// ── FULL READ PASS ────────────────────────────────────────────────────────────

struct T01Data {
    all_data: Vec<f32>,
    th_part_names: Vec<String>,
    th_subs_names: Vec<String>,
    th_group_names: Vec<String>,
}

fn t01_read(filename: &str, dims: &Dimensions) -> io::Result<T01Data> {
    let f = File::open(filename)?;
    let mut r = BufReader::new(f);

    let thicode      = dims.thicode;
    let title_length = dims.title_length;

    read_titre(&mut r)?;
    skip_header(&mut r, thicode)?;
    let (_, _, _, _, _, _) = read_hierarchy_info(&mut r)?;

    // GLOBAL VAR IDs (skip)
    if dims.nb_glob_var > 0 {
        read_eor(&mut r)?;
        for _ in 0..dims.nb_glob_var { read_i(&mut r)?; }
        read_eor(&mut r)?;
    }

    // PART DESCRIPTIONS → collect names
    let mut th_part_names: Vec<String> = Vec::with_capacity(dims.nb_part_var);
    for _i in 0..dims.npart_nthpart {
        read_eor(&mut r)?;
        let _id   = read_i(&mut r)?;
        let name  = read_chars(&mut r, title_length)?;
        read_i(&mut r)?;
        read_i(&mut r)?;
        read_i(&mut r)?;
        let nvar  = read_i(&mut r)? as usize;
        read_eor(&mut r)?;
        let title = format!("{} ", name.trim_end());
        for j in 0..nvar {
            if j == 0 { read_eor(&mut r)?; }
            let code = read_i(&mut r)?;
            if j == nvar - 1 { read_eor(&mut r)?; }
            th_part_names.push(format!("{}{}", title, var_code_name(code)));
        }
    }

    // MATER DESCRIPTIONS (skip)
    for _ in 0..dims.nummat {
        read_eor(&mut r)?;
        read_i(&mut r)?;
        read_chars(&mut r, title_length)?;
        read_eor(&mut r)?;
    }

    // GEO DESCRIPTIONS (skip)
    for _ in 0..dims.numgeo {
        read_eor(&mut r)?;
        read_i(&mut r)?;
        read_chars(&mut r, title_length)?;
        read_eor(&mut r)?;
    }

    // HIERARCHY (SUBSET) DESCRIPTIONS → collect names
    let mut th_subs_names: Vec<String> = Vec::with_capacity(dims.nb_subs_var);
    for _ in 0..dims.nsubs {
        read_eor(&mut r)?;
        read_i(&mut r)?; // subs ID
        read_i(&mut r)?;
        let nbsubsf = read_i(&mut r)? as usize;
        let nbpartf = read_i(&mut r)? as usize;
        let nvar    = read_i(&mut r)? as usize;
        let name    = read_chars(&mut r, title_length)?;
        let title   = format!("{} ", name.trim_end());
        read_eor(&mut r)?;
        for j in 0..nbsubsf {
            if j == 0 { read_eor(&mut r)?; }
            read_i(&mut r)?;
            if j == nbsubsf - 1 { read_eor(&mut r)?; }
        }
        for j in 0..nbpartf {
            if j == 0 { read_eor(&mut r)?; }
            read_i(&mut r)?;
            if j == nbpartf - 1 { read_eor(&mut r)?; }
        }
        for j in 0..nvar {
            if j == 0 { read_eor(&mut r)?; }
            let code = read_i(&mut r)?;
            if j == nvar - 1 { read_eor(&mut r)?; }
            th_subs_names.push(format!("{}{}", title, var_code_name(code)));
        }
    }

    // TH GROUP DESCRIPTIONS → collect names
    let mut th_group_names: Vec<String> = Vec::with_capacity(dims.cpt_th_group_names);
    for _i in 0..dims.nthgrp2 {
        read_eor(&mut r)?;
        read_i(&mut r)?;
        read_i(&mut r)?;
        read_i(&mut r)?;
        let nbelem = read_i(&mut r)? as usize;
        let nvar   = read_i(&mut r)? as usize;
        let grp_name = read_chars(&mut r, title_length)?;
        read_eor(&mut r)?;
        for _ in 0..nbelem {
            read_eor(&mut r)?;
            let elem_id = read_i(&mut r)?;
            let elem_name = read_chars(&mut r, title_length)?;
            read_eor(&mut r)?;
            let title = format!("{} {} {}", grp_name.trim_end(), elem_id, elem_name.trim_end());
            for _ in 0..nvar {
                th_group_names.push(title.clone());
            }
        }
        for j in 0..nvar {
            if j == 0 { read_eor(&mut r)?; }
            read_i(&mut r)?;
            if j == nvar - 1 { read_eor(&mut r)?; }
        }
    }

    // TIME-STEP DATA
    let mut all_data: Vec<f32> = Vec::with_capacity(dims.cpt_data);
    loop {
        // TIME
        if read_eor(&mut r).is_err() { break; }
        match read_r(&mut r) {
            None => break,
            Some(v) => all_data.push(v),
        }
        if read_eor(&mut r).is_err() { break; }

        // GLOBAL VARS
        if dims.nb_glob_var > 0 {
            read_eor(&mut r)?;
            for _ in 0..dims.nb_glob_var {
                match read_r(&mut r) {
                    None => break,
                    Some(v) => all_data.push(v),
                }
            }
            read_eor(&mut r)?;
        }

        // PART VARS
        if dims.npart_nthpart > 0 {
            let tot: usize = dims.nvar_part.iter().sum();
            if tot > 0 { read_eor(&mut r)?; }
            for &nv in &dims.nvar_part {
                for _ in 0..nv {
                    match read_r(&mut r) {
                        None => break,
                        Some(v) => all_data.push(v),
                    }
                }
            }
            if tot > 0 { read_eor(&mut r)?; }
        }

        // SUBSET VARS
        if dims.nb_subs_var > 0 {
            read_eor(&mut r)?;
            for _ in 0..dims.nb_subs_var {
                match read_r(&mut r) {
                    None => break,
                    Some(v) => all_data.push(v),
                }
            }
            read_eor(&mut r)?;
        }

        // TH GROUP VARS
        for i in 0..dims.nthgrp2 {
            read_eor(&mut r)?;
            for _ in 0..dims.nbelem_thgrp[i] {
                for _ in 0..dims.nvar_thgrp[i] {
                    match read_r(&mut r) {
                        None => break,
                        Some(v) => all_data.push(v),
                    }
                }
            }
            read_eor(&mut r)?;
        }
    }

    Ok(T01Data { all_data, th_part_names, th_subs_names, th_group_names })
}

// ── force-name detection ──────────────────────────────────────────────────────

fn is_force_name(name: &str, output_type: i32) -> bool {
    let force_names = [
        "FNX", "FNY", "FNZ", "FTX", "FTY", "FTZ",
        "MX", "MY", "MZ",
        "REACX", "REACY", "REACZ",
        "REACXX", "REACYY", "REACZZ",
        "|FNX|", "|FNY|", "|FNZ|",
        "|FX|", "|FY|", "|FZ|",
        "||FN||", "||F||",
        "FXI", "FYI", "FZI",
        "MXI", "MYI", "MZI",
    ];
    let name_trimmed = name.trim();
    for &f in &force_names {
        if name_trimmed == f { return true; }
    }
    if (name_trimmed == "FX" || name_trimmed == "FY" || name_trimmed == "FZ") && output_type != 6 {
        return true;
    }
    if (name_trimmed == "F1" || name_trimmed == "F2" || name_trimmed == "F3"
        || name_trimmed == "M1" || name_trimmed == "M2" || name_trimmed == "M3")
        && output_type == 104
    {
        return true;
    }
    false
}

// ── CSV WRITE ─────────────────────────────────────────────────────────────────

fn csv_file_write(
    csv_filename: &str,
    title_filename: &str,
    dims: &Dimensions,
    data: &mut T01Data,
) -> io::Result<()> {
    let nb_time_step = dims.nb_time_step;
    let nb_data = if nb_time_step > 0 { data.all_data.len() / nb_time_step } else { 0 };

    let csv_file = File::create(csv_filename)?;
    let mut w = BufWriter::new(csv_file);

    // ── global variable headers (always present) ──────────────────────────────
    write!(w, "\"time\",")?;
    let global_headers = [
        "INTERNAL ENERGY", "KINETIC ENERGY", "X-MOMENTUM", "Y-MOMENTUM", "Z-MOMENTUM",
        "MASS", "TIME STEP", "ROTATION ENERGY", "EXTERNAL WORK", "SPRING ENERGY",
        "CONTACT ENERGY", "HOURGLASS ENERGY", "ELASTIC CONTACT ENERGY",
        "FRICTIONAL CONTACT ENERGY", "DAMPING CONTACT ENERGY ",
    ];
    for h in &global_headers {
        write!(w, "\"{}\",", h)?;
    }
    if dims.nb_glob_var >= 16 { write!(w, "\"PLASTIC WORK\",")?; }
    if dims.nb_glob_var >= 17 { write!(w, "\"ADDED MASS\",")?; }
    if dims.nb_glob_var >= 18 { write!(w, "\"PERCENTAGE ADDED MASS\",")?; }
    if dims.nb_glob_var >= 19 { write!(w, "\"INLET MASS\",")?; }
    if dims.nb_glob_var >= 20 { write!(w, "\"OUTLET MASS\",")?; }
    if dims.nb_glob_var >= 21 { write!(w, "\"INLET ENERGY\",")?; }
    if dims.nb_glob_var >= 22 { write!(w, "\"OUTLET ENERGY\",")?; }
    if dims.nb_glob_var >= 23 {
        for _ in 23..=dims.nb_glob_var {
            write!(w, "\"NO NAME\",")?;
        }
    }

    // ── part and subset headers ───────────────────────────────────────────────
    for name in &data.th_part_names {
        write!(w, "\"{} \",", name)?;
    }
    for name in &data.th_subs_names {
        write!(w, "\"{} \",", name)?;
    }

    // ── TH group headers + optional impulse→force derivation ─────────────────
    let titles_file = File::open(title_filename).ok().map(BufReader::new);
    let group_start = 1 + dims.nb_glob_var + dims.nb_part_var + dims.nb_subs_var;
    let mut is_impulse: Vec<bool> = vec![false; nb_data];

    if let Some(mut tf) = titles_file {
        let mut cpt = 0usize;
        for i in group_start..nb_data {
            let mut type_buf = [0u8; 10];
            tf.read_exact(&mut type_buf)?;
            let output_type: i32 = std::str::from_utf8(&type_buf)
                .unwrap_or("0").trim().parse().unwrap_or(0);
            let mut sep = [0u8; 1];
            tf.read_exact(&mut sep)?;
            let mut name_buf = [0u8; 10];
            tf.read_exact(&mut name_buf)?;
            let var_name = std::str::from_utf8(&name_buf).unwrap_or("").trim().to_string();

            let group_name = data.th_group_names.get(cpt).map(|s| s.as_str()).unwrap_or("");
            write!(w, "\"{}  {}\"", group_name, var_name)?;
            if i < nb_data - 1 { write!(w, ",")?; }
            cpt += 1;

            is_impulse[i] = is_force_name(&var_name, output_type);
            tf.read_exact(&mut sep)?; // newline separator
        }
    } else {
        let mut cpt = 0usize;
        for i in group_start..nb_data {
            let group_name = data.th_group_names.get(cpt).map(|s| s.as_str()).unwrap_or("");
            write!(w, "\"{}  var {}\"", group_name, i)?;
            if i < nb_data - 1 { write!(w, ",")?; }
            cpt += 1;
        }
    }

    // ── impulse → force derivation ────────────────────────────────────────────
    if nb_time_step > 1 {
        for i in group_start..nb_data {
            if !is_impulse[i] { continue; }
            let mut tmp = vec![0.0f32; nb_time_step];
            // forward difference for first step
            tmp[0] = (data.all_data[nb_data + i] - data.all_data[i])
                / (data.all_data[nb_data] - data.all_data[0]);
            // central difference for interior
            for j in 1..nb_time_step - 1 {
                tmp[j] = (data.all_data[nb_data * (j + 1) + i]
                    - data.all_data[nb_data * (j - 1) + i])
                    / (data.all_data[nb_data * (j + 1)]
                        - data.all_data[nb_data * (j - 1)]);
            }
            // backward difference for last step
            let jl = nb_time_step - 1;
            tmp[jl] = (data.all_data[nb_data * jl + i]
                - data.all_data[nb_data * (jl - 1) + i])
                / (data.all_data[nb_data * jl] - data.all_data[nb_data * (jl - 1)]);
            for j in 0..nb_time_step {
                data.all_data[nb_data * j + i] = tmp[j];
            }
        }
    }

    // ── data rows ─────────────────────────────────────────────────────────────
    writeln!(w)?;
    let total = data.all_data.len();
    for (i, &val) in data.all_data[..total.saturating_sub(1)].iter().enumerate() {
        write!(w, "{:e}", val as f64)?;
        if nb_data > 0 && (i + 1) % nb_data == 0 {
            writeln!(w)?;
        } else {
            write!(w, ",")?;
        }
    }

    Ok(())
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!(" ** ERROR: MISSING INPUT ARGUMENT: TH-FILE");
        process::exit(1);
    }

    let t01_filename = &args[1];
    let csv_filename = if args.len() >= 3 {
        format!("{}.csv", args[2])
    } else {
        format!("{}.csv", t01_filename)
    };
    let title_filename = format!("{}_TITLES", t01_filename);

    println!("\n T01 TO CSV CONVERTER\n");
    println!("FILE        = {}", t01_filename);
    println!("OUTPUT FILE = {}", csv_filename);

    // PRE-READ
    let dims = match t01_pre_read(t01_filename) {
        Ok(d) => d,
        Err(e) => {
            eprintln!(" ** ERROR: {}", e);
            process::exit(1);
        }
    };

    // READ
    let mut data = match t01_read(t01_filename, &dims) {
        Ok(d) => d,
        Err(e) => {
            eprintln!(" ** ERROR: {}", e);
            process::exit(1);
        }
    };

    // WRITE
    if let Err(e) = csv_file_write(&csv_filename, &title_filename, &dims, &mut data) {
        eprintln!(" ** ERROR writing CSV: {}", e);
        process::exit(1);
    }

    println!(" ** CONVERSION COMPLETED");
}
