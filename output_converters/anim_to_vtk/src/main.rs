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

// To build:
//   cargo build --release
//
// To launch conversion:
//   anim_to_vtk animationFile > vtkFile

use std::env;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::process;

use itoa::Buffer as ItoaBuffer;

const FASTMAGI10: i32 = 0x542c;

// ****************************************
// Format floats/doubles to match C++ cout behavior
// C++ cout uses 6 significant digits by default
// ****************************************
fn format_f32_like_cpp(v: f32) -> String {
    if v == 0.0 {
        return "0".to_string();
    }
    
    let abs_v = v.abs();
    
    // Use scientific notation for very large or very small numbers
    if abs_v < 1e-4 || abs_v >= 1e6 {
        let s = format!("{:.5e}", v);
        
        // Clean up exponential notation to match C++ format
        if let Some(e_pos) = s.find('e') {
            let (mantissa, exp_part) = s.split_at(e_pos);
            let mantissa = mantissa.trim_end_matches('0').trim_end_matches('.');
            let exp_str = &exp_part[1..];
            let exp_val: i32 = exp_str.parse().unwrap();
            format!("{}e{:+03}", mantissa, exp_val)
        } else {
            s
        }
    } else {
        // Calculate significant figures needed for 6 total significant digits
        let log = abs_v.log10().floor();
        let decimals = (5.0 - log).max(0.0) as usize;
        let formatted = format!("{:.prec$}", v, prec = decimals);
        
        // Remove trailing zeros after decimal point
        if formatted.contains('.') {
            formatted.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            formatted
        }
    }
}

fn format_f64_like_cpp(v: f64) -> String {
    if v == 0.0 {
        return "0".to_string();
    }
    
    let abs_v = v.abs();
    
    // Use scientific notation for very large or very small numbers
    if abs_v < 1e-4 || abs_v >= 1e6 {
        let s = format!("{:.5e}", v);
        
        // Clean up exponential notation to match C++ format
        if let Some(e_pos) = s.find('e') {
            let (mantissa, exp_part) = s.split_at(e_pos);
            let mantissa = mantissa.trim_end_matches('0').trim_end_matches('.');
            let exp_str = &exp_part[1..];
            let exp_val: i32 = exp_str.parse().unwrap();
            format!("{}e{:+03}", mantissa, exp_val)
        } else {
            s
        }
    } else {
        // Calculate significant figures needed for 6 total significant digits
        let log = abs_v.log10().floor();
        let decimals = (5.0 - log).max(0.0) as usize;
        let formatted = format!("{:.prec$}", v, prec = decimals);
        
        // Remove trailing zeros after decimal point
        if formatted.contains('.') {
            formatted.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            formatted
        }
    }
}

// ****************************************
// read big-endian data from file
// ****************************************
fn read_i32<R: Read>(reader: &mut R) -> i32 {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).expect("Error in reading file");
    i32::from_be_bytes(buf)
}

fn read_f32<R: Read>(reader: &mut R) -> f32 {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).expect("Error in reading file");
    f32::from_be_bytes(buf)
}

fn read_i32_vec<R: Read>(reader: &mut R, count: usize) -> Vec<i32> {
    let mut bytes = vec![0u8; count * 4];
    reader
        .read_exact(&mut bytes)
        .expect("Error in reading file");
    let mut result = Vec::with_capacity(count);
    for chunk in bytes.chunks_exact(4) {
        result.push(i32::from_be_bytes([
            chunk[0], chunk[1], chunk[2], chunk[3],
        ]));
    }
    result
}

fn read_f32_vec<R: Read>(reader: &mut R, count: usize) -> Vec<f32> {
    let mut bytes = vec![0u8; count * 4];
    reader
        .read_exact(&mut bytes)
        .expect("Error in reading file");
    let mut result = Vec::with_capacity(count);
    for chunk in bytes.chunks_exact(4) {
        result.push(f32::from_be_bytes([
            chunk[0], chunk[1], chunk[2], chunk[3],
        ]));
    }
    result
}

fn read_u16_vec<R: Read>(reader: &mut R, count: usize) -> Vec<u16> {
    let mut bytes = vec![0u8; count * 2];
    reader
        .read_exact(&mut bytes)
        .expect("Error in reading file");
    let mut result = Vec::with_capacity(count);
    for chunk in bytes.chunks_exact(2) {
        result.push(u16::from_be_bytes([chunk[0], chunk[1]]));
    }
    result
}

fn read_bytes<R: Read>(reader: &mut R, count: usize) -> Vec<u8> {
    let mut buf = vec![0u8; count];
    reader.read_exact(&mut buf).expect("Error in reading file");
    buf
}

fn read_text<R: Read>(reader: &mut R, count: usize) -> String {
    let buf = read_bytes(reader, count);
    let s = std::str::from_utf8(&buf).unwrap_or("");
    s.trim_end_matches('\0').to_string()
}

// ****************************************
// replace ' ' with '_'
// ****************************************
fn replace_underscore(s: &str) -> String {
    s.replace(' ', "_")
}

// ****************************************
// VtkWriter - abstraction for VTK output in binary or ASCII format
// ****************************************
struct VtkWriter<W: Write> {
    writer: BufWriter<W>,
    binary: bool,
    scratch: Vec<u8>,
    itoa_buf: ItoaBuffer,
}

impl<W: Write> VtkWriter<W> {
    fn new(writer: W, binary: bool) -> Self {
        VtkWriter {
            writer: BufWriter::new(writer),
            binary,
            scratch: Vec::with_capacity(256),
            itoa_buf: ItoaBuffer::new(),
        }
    }

    fn write_i32(&mut self, val: i32) {
        if self.binary {
            self.writer.write_all(&val.to_be_bytes()).unwrap();
        } else {
            self.scratch.clear();
            let s = self.itoa_buf.format(val);
            self.scratch.extend_from_slice(s.as_bytes());
            self.scratch.push(b'\n');
            self.writer.write_all(&self.scratch).unwrap();
        }
    }

    fn write_f32(&mut self, val: f32) {
        if self.binary {
            self.writer.write_all(&val.to_be_bytes()).unwrap();
        } else {
            self.scratch.clear();
            let s = format_f32_like_cpp(val);
            self.scratch.extend_from_slice(s.as_bytes());
            self.scratch.push(b'\n');
            self.writer.write_all(&self.scratch).unwrap();
        }
    }

    // Bulk write f32 values from a slice - more efficient than individual writes
    fn write_f32_slice(&mut self, values: &[f32]) {
        if self.binary {
            for &val in values {
                self.writer.write_all(&val.to_be_bytes()).unwrap();
            }
        } else {
            for &val in values {
                self.scratch.clear();
                let s = format_f32_like_cpp(val);
                self.scratch.extend_from_slice(s.as_bytes());
                self.scratch.push(b'\n');
                self.writer.write_all(&self.scratch).unwrap();
            }
        }
    }

    fn write_f64(&mut self, val: f64) {
        if self.binary {
            self.writer.write_all(&val.to_be_bytes()).unwrap();
        } else {
            self.scratch.clear();
            let s = format_f64_like_cpp(val);
            self.scratch.extend_from_slice(s.as_bytes());
            self.scratch.push(b'\n');
            self.writer.write_all(&self.scratch).unwrap();
        }
    }

    fn write_f32_triple(&mut self, a: f32, b: f32, c: f32) {
        if self.binary {
            self.writer.write_all(&a.to_be_bytes()).unwrap();
            self.writer.write_all(&b.to_be_bytes()).unwrap();
            self.writer.write_all(&c.to_be_bytes()).unwrap();
        } else {
            self.scratch.clear();
            let sa = format_f32_like_cpp(a);
            self.scratch.extend_from_slice(sa.as_bytes());
            self.scratch.push(b' ');
            let sb = format_f32_like_cpp(b);
            self.scratch.extend_from_slice(sb.as_bytes());
            self.scratch.push(b' ');
            let sc = format_f32_like_cpp(c);
            self.scratch.extend_from_slice(sc.as_bytes());
            self.scratch.push(b'\n');
            self.writer.write_all(&self.scratch).unwrap();
        }
    }

    fn write_zeros_f32(&mut self, count: usize) {
        if self.binary {
            let zero_bytes = 0f32.to_be_bytes();
            for _ in 0..count {
                self.writer.write_all(&zero_bytes).unwrap();
            }
        } else {
            for _ in 0..count {
                self.writer.write_all(b"0\n").unwrap();
            }
        }
    }

    fn write_zero_tensor(&mut self) {
        self.write_zeros_f32(9);
    }

    fn write_header(&mut self, text: &str) {
        self.writer.write_all(text.as_bytes()).unwrap();
        self.writer.write_all(b"\n").unwrap();
    }

    fn newline(&mut self) {
        self.writer.write_all(b"\n").unwrap();
    }

    fn flush(&mut self) {
        self.writer.flush().unwrap();
    }

    fn write_i32_line(&mut self, values: &[i32]) {
        if self.binary {
            for &v in values {
                self.writer.write_all(&v.to_be_bytes()).unwrap();
            }
        } else {
            self.scratch.clear();
            for (i, &v) in values.iter().enumerate() {
                if i > 0 {
                    self.scratch.push(b' ');
                }
                let s = self.itoa_buf.format(v);
                self.scratch.extend_from_slice(s.as_bytes());
            }
            self.scratch.push(b'\n');
            self.writer.write_all(&self.scratch).unwrap();
        }
    }
}

// ****************************************
// Small fixed-size dedup helpers
// ****************************************
fn unique_count(nodes: &[i32]) -> usize {
    let mut uniq = [0i32; 8];
    let mut count = 0usize;
    for &n in nodes {
        let mut seen = false;
        for i in 0..count {
            if uniq[i] == n {
                seen = true;
                break;
            }
        }
        if !seen {
            uniq[count] = n;
            count += 1;
        }
    }
    count
}

fn unique_sorted_4(nodes: &[i32]) -> Option<[i32; 4]> {
    let mut uniq = [0i32; 8];
    let mut count = 0usize;
    for &n in nodes {
        let mut seen = false;
        for i in 0..count {
            if uniq[i] == n {
                seen = true;
                break;
            }
        }
        if !seen {
            uniq[count] = n;
            count += 1;
        }
    }
    if count == 4 {
        let mut arr = [uniq[0], uniq[1], uniq[2], uniq[3]];
        arr.sort_unstable();
        Some(arr)
    } else {
        None
    }
}

// ****************************************
// Helper function: resolve part ID for an element
// Advances part_index at part boundaries and parses part ID from text
// ****************************************
fn resolve_part_id(
    iel: usize,           // Element index
    part_index: &mut usize, // Current part index (mutated at boundaries)
    def_part: &[i32],     // Element indices where parts begin
    p_text: &[String],    // Part ID strings (to be parsed as integers)
) -> i32 {
    if *part_index < def_part.len() && iel == def_part[*part_index] as usize {
        *part_index += 1;
    }
    if *part_index < p_text.len() {
        p_text[*part_index].trim().parse().unwrap_or(0)
    } else {
        0
    }
}

// ****************************************
// Helper function: write per-cell i32 values from multiple slices
// ****************************************
fn write_cell_i32_values<W: Write>(
    writer: &mut VtkWriter<W>,
    slices: &[&[i32]],
) {
    for slice in slices {
        for &val in *slice {
            writer.write_i32(val);
        }
    }
    writer.newline();
}

// ****************************************
// Helper function: write elemental scalar field with zero-padding
// ****************************************
fn write_elemental_scalar<W: Write>(
    writer: &mut VtkWriter<W>,
    name: &str,
    counts: &[usize],       // [nb_1d, nb_2d, nb_3d, nb_sph]
    active_idx: usize,      // which element type has actual values
    values: &[f32],         // actual values for active element type
) {
    writer.write_header(&format!("SCALARS {} float 1", name));
    writer.write_header("LOOKUP_TABLE default");
    
    for (idx, &count) in counts.iter().enumerate() {
        if idx == active_idx {
            // Use bulk write for the entire slice - more efficient
            writer.write_f32_slice(&values[0..count]);
        } else {
            writer.write_zeros_f32(count);
        }
    }
    writer.newline();
}

// ****************************************
// Helper function: write elemental scalar from strided data
// For data like torseur values where each element has multiple components
// ****************************************
fn write_elemental_scalar_strided<W: Write>(
    writer: &mut VtkWriter<W>,
    name: &str,
    counts: &[usize],       // [nb_1d, nb_2d, nb_3d, nb_sph]
    active_idx: usize,      // which element type has actual values
    data: &[f32],           // source data array
    stride: usize,          // stride between elements (e.g., 9 for torseur)
    offset: usize,          // offset within stride for this component
    count: usize,           // number of elements
) {
    writer.write_header(&format!("SCALARS {} float 1", name));
    writer.write_header("LOOKUP_TABLE default");
    
    for (idx, &elem_count) in counts.iter().enumerate() {
        if idx == active_idx {
            // Write strided values
            for iel in 0..count {
                writer.write_f32(data[iel * stride + offset]);
            }
        } else {
            writer.write_zeros_f32(elem_count);
        }
    }
    writer.newline();
}

// ****************************************
// Helper function: write symmetric tensor (6-component: 3D/SPH)
// ****************************************
fn write_symmetric_tensor_6<W: Write>(
    writer: &mut VtkWriter<W>,
    name: &str,
    counts: &[usize],
    active_idx: usize,
    values: &[f32],         // [xx, yy, zz, xy, xz, yz] for each element
) {
    writer.write_header(&format!("TENSORS {} float", name));
    
    for (idx, &count) in counts.iter().enumerate() {
        if idx == active_idx {
            for i in 0..count {
                let base = i * 6;
                let xx = values[base];
                let yy = values[base + 1];
                let zz = values[base + 2];
                let xy = values[base + 3];
                let xz = values[base + 4];
                let yz = values[base + 5];
                
                writer.write_f32_triple(xx, xy, xz);
                writer.write_f32_triple(xy, yy, yz);
                writer.write_f32_triple(xz, yz, zz);
            }
        } else {
            for _ in 0..count {
                writer.write_zero_tensor();
            }
        }
    }
    writer.newline();
}

// ****************************************
// Helper function: write symmetric tensor (3-component: 2D)
// ****************************************
fn write_symmetric_tensor_3<W: Write>(
    writer: &mut VtkWriter<W>,
    name: &str,
    counts: &[usize],
    active_idx: usize,
    values: &[f32],         // [xx, yy, xy] for each element
) {
    writer.write_header(&format!("TENSORS {} float", name));
    
    for (idx, &count) in counts.iter().enumerate() {
        if idx == active_idx {
            for i in 0..count {
                let base = i * 3;
                let xx = values[base];
                let yy = values[base + 1];
                let xy = values[base + 2];
                
                writer.write_f32_triple(xx, xy, 0.0);
                writer.write_f32_triple(xy, yy, 0.0);
                writer.write_f32_triple(0.0, 0.0, 0.0);
            }
        } else {
            for _ in 0..count {
                writer.write_zero_tensor();
            }
        }
    }
    writer.newline();
}

// ****************************************
// convert an A-File to vtk format (ASCII or BINARY)
// ****************************************
fn read_radioss_anim<W: Write>(file_name: &str, binary_format: bool, writer: W) {
    let input_file = File::open(file_name).unwrap_or_else(|_| {
        eprintln!("Can't open input file {}", file_name);
        process::exit(1);
    });
    let mut inf = BufReader::new(input_file);

    let mut vtk = VtkWriter::new(writer, binary_format);

    let magic = read_i32(&mut inf);

    match magic {
        FASTMAGI10 => {
            let a_time = read_f32(&mut inf);
            let _time_text = read_text(&mut inf, 81);
            let _mod_anim_text = read_text(&mut inf, 81);
            let _radioss_run_text = read_text(&mut inf, 81);

            let flag_a = read_i32_vec(&mut inf, 10);

            // ********************
            // 2D GEOMETRY
            // ********************
            let nb_nodes = read_i32(&mut inf) as usize;
            let nb_facets = read_i32(&mut inf) as usize;
            let nb_parts = read_i32(&mut inf) as usize;
            let nb_func = read_i32(&mut inf) as usize;
            let nb_efunc = read_i32(&mut inf) as usize;
            let nb_vect = read_i32(&mut inf) as usize;
            let nb_tens = read_i32(&mut inf) as usize;
            let nb_skew = read_i32(&mut inf) as usize;

            if nb_skew > 0 {
                let _skew_short = read_u16_vec(&mut inf, nb_skew * 6);
                // skew values are read but only used internally, not in VTK output
            }

            let coor_a = read_f32_vec(&mut inf, 3 * nb_nodes);

            let mut connect_a: Vec<i32> = Vec::new();
            let mut del_elt_a: Vec<u8> = Vec::new();
            if nb_facets > 0 {
                connect_a = read_i32_vec(&mut inf, nb_facets * 4);
                del_elt_a = read_bytes(&mut inf, nb_facets);
            }

            let mut def_part_a: Vec<i32> = Vec::new();
            let mut p_text_a: Vec<String> = Vec::new();
            if nb_parts > 0 {
                def_part_a = read_i32_vec(&mut inf, nb_parts);
                p_text_a = (0..nb_parts)
                    .map(|_| read_text(&mut inf, 50))
                    .collect();
            }

            let _norm_short_a = read_u16_vec(&mut inf, 3 * nb_nodes);

            let mut f_text_a: Vec<String> = Vec::new();
            let mut func_a: Vec<f32> = Vec::new();
            let mut efunc_a: Vec<f32> = Vec::new();
            if nb_func + nb_efunc > 0 {
                f_text_a = (0..nb_func + nb_efunc)
                    .map(|_| read_text(&mut inf, 81))
                    .collect();
                if nb_func > 0 {
                    func_a = read_f32_vec(&mut inf, nb_nodes * nb_func);
                }
                if nb_efunc > 0 {
                    efunc_a = read_f32_vec(&mut inf, nb_facets * nb_efunc);
                }
            }

            let mut v_text_a: Vec<String> = Vec::new();
            if nb_vect > 0 {
                v_text_a = (0..nb_vect)
                    .map(|_| read_text(&mut inf, 81))
                    .collect();
            }
            let vect_val_a = read_f32_vec(&mut inf, 3 * nb_nodes * nb_vect);

            let mut t_text_a: Vec<String> = Vec::new();
            let mut tens_val_a: Vec<f32> = Vec::new();
            if nb_tens > 0 {
                t_text_a = (0..nb_tens)
                    .map(|_| read_text(&mut inf, 81))
                    .collect();
                tens_val_a = read_f32_vec(&mut inf, nb_facets * 3 * nb_tens);
            }

            if flag_a[0] == 1 {
                let _e_mass_a = read_f32_vec(&mut inf, nb_facets);
                let _n_mass_a = read_f32_vec(&mut inf, nb_nodes);
            }

            let mut nod_num_a: Vec<i32> = Vec::new();
            let mut el_num_a: Vec<i32> = Vec::new();
            if flag_a[1] != 0 {
                nod_num_a = read_i32_vec(&mut inf, nb_nodes);
                el_num_a = read_i32_vec(&mut inf, nb_facets);
            }

            if flag_a[4] != 0 {
                let _part2subset_2d = read_i32_vec(&mut inf, nb_parts);
                let _part_material_2d = read_i32_vec(&mut inf, nb_parts);
                let _part_properties_2d = read_i32_vec(&mut inf, nb_parts);
            }

            // ********************
            // 3D GEOMETRY
            // ********************
            let mut nb_elts_3d: usize = 0;
            let mut nb_efunc_3d: usize = 0;
            let mut nb_tens_3d: usize = 0;
            let mut connect_3d: Vec<i32> = Vec::new();
            let mut del_elt_3d: Vec<u8> = Vec::new();
            let mut def_part_3d: Vec<i32> = Vec::new();
            let mut p_text_3d: Vec<String> = Vec::new();
            let mut f_text_3d: Vec<String> = Vec::new();
            let mut efunc_3d: Vec<f32> = Vec::new();
            let mut t_text_3d: Vec<String> = Vec::new();
            let mut tens_val_3d: Vec<f32> = Vec::new();
            let mut el_num_3d: Vec<i32> = Vec::new();

            if flag_a[2] != 0 {
                nb_elts_3d = read_i32(&mut inf) as usize;
                let nb_parts_3d = read_i32(&mut inf) as usize;
                nb_efunc_3d = read_i32(&mut inf) as usize;
                nb_tens_3d = read_i32(&mut inf) as usize;

                connect_3d = read_i32_vec(&mut inf, nb_elts_3d * 8);
                del_elt_3d = read_bytes(&mut inf, nb_elts_3d);

                def_part_3d = read_i32_vec(&mut inf, nb_parts_3d);
                p_text_3d = (0..nb_parts_3d)
                    .map(|_| read_text(&mut inf, 50))
                    .collect();

                if nb_efunc_3d > 0 {
                    f_text_3d = (0..nb_efunc_3d)
                        .map(|_| read_text(&mut inf, 81))
                        .collect();
                    efunc_3d = read_f32_vec(&mut inf, nb_efunc_3d * nb_elts_3d);
                }

                if nb_tens_3d > 0 {
                    t_text_3d = (0..nb_tens_3d)
                        .map(|_| read_text(&mut inf, 81))
                        .collect();
                    tens_val_3d = read_f32_vec(&mut inf, nb_elts_3d * 6 * nb_tens_3d);
                }

                if flag_a[0] == 1 {
                    let _e_mass_3d = read_f32_vec(&mut inf, nb_elts_3d);
                }
                if flag_a[1] == 1 {
                    el_num_3d = read_i32_vec(&mut inf, nb_elts_3d);
                }
                if flag_a[4] != 0 {
                    let _part2subset_3d = read_i32_vec(&mut inf, nb_parts_3d);
                    let _part_material_3d = read_i32_vec(&mut inf, nb_parts_3d);
                    let _part_properties_3d = read_i32_vec(&mut inf, nb_parts_3d);
                }
            }

            // ********************
            // 1D GEOMETRY
            // ********************
            let mut nb_elts_1d: usize = 0;
            let mut nb_efunc_1d: usize = 0;
            let mut nb_tors_1d: usize = 0;
            let mut connect_1d: Vec<i32> = Vec::new();
            let mut del_elt_1d: Vec<u8> = Vec::new();
            let mut def_part_1d: Vec<i32> = Vec::new();
            let mut p_text_1d: Vec<String> = Vec::new();
            let mut f_text_1d: Vec<String> = Vec::new();
            let mut efunc_1d: Vec<f32> = Vec::new();
            let mut t_text_1d: Vec<String> = Vec::new();
            let mut tors_val_1d: Vec<f32> = Vec::new();
            let mut el_num_1d: Vec<i32> = Vec::new();

            if flag_a[3] != 0 {
                nb_elts_1d = read_i32(&mut inf) as usize;
                let nb_parts_1d = read_i32(&mut inf) as usize;
                nb_efunc_1d = read_i32(&mut inf) as usize;
                nb_tors_1d = read_i32(&mut inf) as usize;
                let is_skew_1d = read_i32(&mut inf);

                connect_1d = read_i32_vec(&mut inf, nb_elts_1d * 2);
                del_elt_1d = read_bytes(&mut inf, nb_elts_1d);

                def_part_1d = read_i32_vec(&mut inf, nb_parts_1d);
                p_text_1d = (0..nb_parts_1d)
                    .map(|_| read_text(&mut inf, 50))
                    .collect();

                if nb_efunc_1d > 0 {
                    f_text_1d = (0..nb_efunc_1d)
                        .map(|_| read_text(&mut inf, 81))
                        .collect();
                    efunc_1d = read_f32_vec(&mut inf, nb_efunc_1d * nb_elts_1d);
                }

                if nb_tors_1d > 0 {
                    t_text_1d = (0..nb_tors_1d)
                        .map(|_| read_text(&mut inf, 81))
                        .collect();
                    tors_val_1d = read_f32_vec(&mut inf, nb_elts_1d * 9 * nb_tors_1d);
                }

                if is_skew_1d != 0 {
                    let _elt2_skew_1d = read_i32_vec(&mut inf, nb_elts_1d);
                }
                if flag_a[0] == 1 {
                    let _e_mass_1d = read_f32_vec(&mut inf, nb_elts_1d);
                }
                if flag_a[1] == 1 {
                    el_num_1d = read_i32_vec(&mut inf, nb_elts_1d);
                }
                if flag_a[4] != 0 {
                    let _part2subset_1d = read_i32_vec(&mut inf, nb_parts_1d);
                    let _part_material_1d = read_i32_vec(&mut inf, nb_parts_1d);
                    let _part_properties_1d = read_i32_vec(&mut inf, nb_parts_1d);
                }
            }

            // hierarchy
            if flag_a[4] != 0 {
                let nb_subsets = read_i32(&mut inf) as usize;
                for _ in 0..nb_subsets {
                    let _subset_text = read_text(&mut inf, 50);
                    let _num_parent = read_i32(&mut inf);
                    let nb_subset_son = read_i32(&mut inf) as usize;
                    if nb_subset_son > 0 {
                        let _subset_son = read_i32_vec(&mut inf, nb_subset_son);
                    }
                    let nb_sub_part_2d = read_i32(&mut inf) as usize;
                    if nb_sub_part_2d > 0 {
                        let _sub_part_2d = read_i32_vec(&mut inf, nb_sub_part_2d);
                    }
                    let nb_sub_part_3d = read_i32(&mut inf) as usize;
                    if nb_sub_part_3d > 0 {
                        let _sub_part_3d = read_i32_vec(&mut inf, nb_sub_part_3d);
                    }
                    let nb_sub_part_1d = read_i32(&mut inf) as usize;
                    if nb_sub_part_1d > 0 {
                        let _sub_part_1d = read_i32_vec(&mut inf, nb_sub_part_1d);
                    }
                }

                let nb_materials = read_i32(&mut inf) as usize;
                let nb_properties = read_i32(&mut inf) as usize;
                let _material_texts: Vec<String> = (0..nb_materials)
                    .map(|_| read_text(&mut inf, 50))
                    .collect();
                let _material_types = read_i32_vec(&mut inf, nb_materials);
                let _properties_texts: Vec<String> = (0..nb_properties)
                    .map(|_| read_text(&mut inf, 50))
                    .collect();
                let _properties_types = read_i32_vec(&mut inf, nb_properties);
            }

            // ********************
            // NODES/ELTS FOR Time History
            // ********************
            if flag_a[5] != 0 {
                let nb_nodes_th = read_i32(&mut inf) as usize;
                let nb_elts_2d_th = read_i32(&mut inf) as usize;
                let nb_elts_3d_th = read_i32(&mut inf) as usize;
                let nb_elts_1d_th = read_i32(&mut inf) as usize;

                let _nodes_2th = read_i32_vec(&mut inf, nb_nodes_th);
                let _n2th_texts: Vec<String> = (0..nb_nodes_th)
                    .map(|_| read_text(&mut inf, 50))
                    .collect();
                let _elt_2d_th = read_i32_vec(&mut inf, nb_elts_2d_th);
                let _elt_2d_th_texts: Vec<String> = (0..nb_elts_2d_th)
                    .map(|_| read_text(&mut inf, 50))
                    .collect();
                let _elt_3d_th = read_i32_vec(&mut inf, nb_elts_3d_th);
                let _elt_3d_th_texts: Vec<String> = (0..nb_elts_3d_th)
                    .map(|_| read_text(&mut inf, 50))
                    .collect();
                let _elt_1d_th = read_i32_vec(&mut inf, nb_elts_1d_th);
                let _elt_1d_th_texts: Vec<String> = (0..nb_elts_1d_th)
                    .map(|_| read_text(&mut inf, 50))
                    .collect();
            }

            // ********************
            // READ SPH PART
            // ********************
            let mut nb_elts_sph: usize = 0;
            let mut nb_efunc_sph: usize = 0;
            let mut nb_tens_sph: usize = 0;
            let mut connec_sph: Vec<i32> = Vec::new();
            let mut del_elt_sph: Vec<u8> = Vec::new();
            let mut def_part_sph: Vec<i32> = Vec::new();
            let mut p_text_sph: Vec<String> = Vec::new();
            let mut scal_text_sph: Vec<String> = Vec::new();
            let mut efunc_sph: Vec<f32> = Vec::new();
            let mut tens_text_sph: Vec<String> = Vec::new();
            let mut tens_val_sph: Vec<f32> = Vec::new();
            let mut nod_num_sph: Vec<i32> = Vec::new();

            if flag_a[7] != 0 {
                nb_elts_sph = read_i32(&mut inf) as usize;
                let nb_parts_sph = read_i32(&mut inf) as usize;
                nb_efunc_sph = read_i32(&mut inf) as usize;
                nb_tens_sph = read_i32(&mut inf) as usize;

                if nb_elts_sph > 0 {
                    connec_sph = read_i32_vec(&mut inf, nb_elts_sph);
                    del_elt_sph = read_bytes(&mut inf, nb_elts_sph);
                }
                if nb_parts_sph > 0 {
                    def_part_sph = read_i32_vec(&mut inf, nb_parts_sph);
                    p_text_sph = (0..nb_parts_sph)
                        .map(|_| read_text(&mut inf, 50))
                        .collect();
                }
                if nb_efunc_sph > 0 {
                    scal_text_sph = (0..nb_efunc_sph)
                        .map(|_| read_text(&mut inf, 81))
                        .collect();
                    efunc_sph = read_f32_vec(&mut inf, nb_efunc_sph * nb_elts_sph);
                }
                if nb_tens_sph > 0 {
                    tens_text_sph = (0..nb_tens_sph)
                        .map(|_| read_text(&mut inf, 81))
                        .collect();
                    tens_val_sph = read_f32_vec(&mut inf, nb_elts_sph * nb_tens_sph * 6);
                }
                if flag_a[0] == 1 {
                    let _e_mass_sph = read_f32_vec(&mut inf, nb_elts_sph);
                }
                if flag_a[1] == 1 {
                    nod_num_sph = read_i32_vec(&mut inf, nb_elts_sph);
                }
                if flag_a[4] != 0 {
                    let _num_parent_sph = read_i32_vec(&mut inf, nb_parts_sph);
                    let _mat_part_sph = read_i32_vec(&mut inf, nb_parts_sph);
                    let _prop_part_sph = read_i32_vec(&mut inf, nb_parts_sph);
                }
            }

            // ********************
            // VTK output
            // ********************
            vtk.write_header("# vtk DataFile Version 3.0");
            vtk.write_header("vtk output");
            if binary_format {
                vtk.write_header("BINARY");
            } else {
                vtk.write_header("ASCII");
            }
            vtk.write_header("DATASET UNSTRUCTURED_GRID");

            vtk.write_header("FIELD FieldData 2");
            vtk.write_header("TIME 1 1 double");
            vtk.write_f64(a_time as f64);
            vtk.newline();
            vtk.write_header("CYCLE 1 1 int");
            vtk.write_i32(0);
            vtk.newline();

            // nodes
            vtk.write_header(&format!("POINTS {} float", nb_nodes));
            for inod in 0..nb_nodes {
                vtk.write_f32_triple(
                    coor_a[3 * inod],
                    coor_a[3 * inod + 1],
                    coor_a[3 * inod + 2],
                );
            }
            vtk.newline();

            // detect tetrahedra in 3D cells
            let mut is_3d_cell_tetrahedron: Vec<bool> = Vec::with_capacity(nb_elts_3d);
            let mut tetra_nodes: Vec<[i32; 4]> = Vec::with_capacity(nb_elts_3d);
            let mut tetrahedron_count: usize = 0;
            for icon in 0..nb_elts_3d {
                let nodes = &connect_3d[icon * 8..icon * 8 + 8];
                if let Some(tet) = unique_sorted_4(nodes) {
                    is_3d_cell_tetrahedron.push(true);
                    tetra_nodes.push(tet);
                    tetrahedron_count += 1;
                } else {
                    is_3d_cell_tetrahedron.push(false);
                    tetra_nodes.push([0; 4]);
                }
            }

            // detect triangles in 2D cells
            let mut is_2d_triangle: Vec<bool> = Vec::with_capacity(nb_facets);
            let mut _triangle_count: usize = 0;
            for icon in 0..nb_facets {
                let nodes = &connect_a[icon * 4..icon * 4 + 4];
                if unique_count(nodes) == 3 {
                    is_2d_triangle.push(true);
                    _triangle_count += 1;
                } else {
                    is_2d_triangle.push(false);
                }
            }

            let total_cells = nb_elts_1d + nb_facets + nb_elts_3d + nb_elts_sph;
            if total_cells > 0 {
                let cells_size = nb_elts_1d * 3
                    + nb_facets * 5
                    + tetrahedron_count * 5
                    + (nb_elts_3d - tetrahedron_count) * 9
                    + nb_elts_sph * 2;
                vtk.write_header(&format!("CELLS {} {}", total_cells, cells_size));

                if binary_format {
                    // 1D elements
                    for icon in 0..nb_elts_1d {
                        vtk.write_i32(2);
                        vtk.write_i32(connect_1d[icon * 2]);
                        vtk.write_i32(connect_1d[icon * 2 + 1]);
                    }
                    // 2D elements
                    for icon in 0..nb_facets {
                        vtk.write_i32(4);
                        vtk.write_i32(connect_a[icon * 4]);
                        vtk.write_i32(connect_a[icon * 4 + 1]);
                        vtk.write_i32(connect_a[icon * 4 + 2]);
                        vtk.write_i32(connect_a[icon * 4 + 3]);
                    }
                    // 3D elements
                    for icon in 0..nb_elts_3d {
                        if is_3d_cell_tetrahedron[icon] {
                            let tet = tetra_nodes[icon];
                            vtk.write_i32(4);
                            vtk.write_i32(tet[0]);
                            vtk.write_i32(tet[1]);
                            vtk.write_i32(tet[2]);
                            vtk.write_i32(tet[3]);
                        } else {
                            vtk.write_i32(8);
                            for i in 0..8 {
                                vtk.write_i32(connect_3d[icon * 8 + i]);
                            }
                        }
                    }
                    // SPH elements
                    for icon in 0..nb_elts_sph {
                        vtk.write_i32(1);
                        vtk.write_i32(connec_sph[icon]);
                    }
                } else {
                    // 1D elements
                    for icon in 0..nb_elts_1d {
                        let vals = [
                            2,
                            connect_1d[icon * 2],
                            connect_1d[icon * 2 + 1],
                        ];
                        vtk.write_i32_line(&vals);
                    }
                    // 2D elements
                    for icon in 0..nb_facets {
                        let vals = [
                            4,
                            connect_a[icon * 4],
                            connect_a[icon * 4 + 1],
                            connect_a[icon * 4 + 2],
                            connect_a[icon * 4 + 3],
                        ];
                        vtk.write_i32_line(&vals);
                    }
                    // 3D elements
                    for icon in 0..nb_elts_3d {
                        if is_3d_cell_tetrahedron[icon] {
                            let tet = tetra_nodes[icon];
                            let vals = [4, tet[0], tet[1], tet[2], tet[3]];
                            vtk.write_i32_line(&vals);
                        } else {
                            let vals = [
                                8,
                                connect_3d[icon * 8],
                                connect_3d[icon * 8 + 1],
                                connect_3d[icon * 8 + 2],
                                connect_3d[icon * 8 + 3],
                                connect_3d[icon * 8 + 4],
                                connect_3d[icon * 8 + 5],
                                connect_3d[icon * 8 + 6],
                                connect_3d[icon * 8 + 7],
                            ];
                            vtk.write_i32_line(&vals);
                        }
                    }
                    // SPH elements
                    for icon in 0..nb_elts_sph {
                        let vals = [1, connec_sph[icon]];
                        vtk.write_i32_line(&vals);
                    }
                }
            }
            vtk.newline();

            // element types
            if total_cells > 0 {
                vtk.write_header(&format!("CELL_TYPES {}", total_cells));
                for _ in 0..nb_elts_1d {
                    vtk.write_i32(3);
                }
                for icon in 0..nb_facets {
                    if is_2d_triangle[icon] {
                        vtk.write_i32(5);
                    } else {
                        vtk.write_i32(9);
                    }
                }
                for icon in 0..nb_elts_3d {
                    if is_3d_cell_tetrahedron[icon] {
                        vtk.write_i32(10);
                    } else {
                        vtk.write_i32(12);
                    }
                }
                for _ in 0..nb_elts_sph {
                    vtk.write_i32(1);
                }
            }
            vtk.newline();

            // nodal scalars & vectors
            vtk.write_header(&format!("POINT_DATA {}", nb_nodes));

            // node id
            vtk.write_header("SCALARS NODE_ID int 1");
            vtk.write_header("LOOKUP_TABLE default");
            for inod in 0..nb_nodes {
                vtk.write_i32(nod_num_a[inod]);
            }
            vtk.newline();

            for ifun in 0..nb_func {
                let name = replace_underscore(&f_text_a[ifun]);
                vtk.write_header(&format!("SCALARS {} float 1", name));
                vtk.write_header("LOOKUP_TABLE default");
                for inod in 0..nb_nodes {
                    vtk.write_f32(func_a[ifun * nb_nodes + inod]);
                }
                vtk.newline();
            }

            for ivect in 0..nb_vect {
                let name = replace_underscore(&v_text_a[ivect]);
                vtk.write_header(&format!("VECTORS {} float", name));
                for inod in 0..nb_nodes {
                    vtk.write_f32_triple(
                        vect_val_a[3 * inod + ivect * 3 * nb_nodes],
                        vect_val_a[3 * inod + 1 + ivect * 3 * nb_nodes],
                        vect_val_a[3 * inod + 2 + ivect * 3 * nb_nodes],
                    );
                }
                vtk.newline();
            }

            vtk.write_header(&format!("CELL_DATA {}", total_cells));

            // element id
            vtk.write_header("SCALARS ELEMENT_ID int 1");
            vtk.write_header("LOOKUP_TABLE default");
            write_cell_i32_values(&mut vtk, &[&el_num_1d, &el_num_a, &el_num_3d, &nod_num_sph]);

            // part id
            vtk.write_header("SCALARS PART_ID int 1");
            vtk.write_header("LOOKUP_TABLE default");

            let mut part_1d_index: usize = 0;
            let mut part_2d_index: usize = 0;
            let mut part_3d_index: usize = 0;
            let mut part_0d_index: usize = 0;

            for iel in 0..nb_elts_1d {
                let part_id = resolve_part_id(iel, &mut part_1d_index, &def_part_1d, &p_text_1d);
                vtk.write_i32(part_id);
            }
            for iel in 0..nb_facets {
                let part_id = resolve_part_id(iel, &mut part_2d_index, &def_part_a, &p_text_a);
                vtk.write_i32(part_id);
            }
            for iel in 0..nb_elts_3d {
                let part_id = resolve_part_id(iel, &mut part_3d_index, &def_part_3d, &p_text_3d);
                vtk.write_i32(part_id);
            }
            for iel in 0..nb_elts_sph {
                let part_id = resolve_part_id(iel, &mut part_0d_index, &def_part_sph, &p_text_sph);
                vtk.write_i32(part_id);
            }
            vtk.newline();

            // element erosion status (0:off, 1:on)
            vtk.write_header("SCALARS EROSION_STATUS int 1");
            vtk.write_header("LOOKUP_TABLE default");
            for iel in 0..nb_elts_1d {
                vtk.write_i32(if del_elt_1d[iel] != 0 { 1 } else { 0 });
            }
            for iel in 0..nb_facets {
                vtk.write_i32(if del_elt_a[iel] != 0 { 1 } else { 0 });
            }
            for iel in 0..nb_elts_3d {
                vtk.write_i32(if del_elt_3d[iel] != 0 { 1 } else { 0 });
            }
            for iel in 0..nb_elts_sph {
                vtk.write_i32(if del_elt_sph[iel] != 0 { 1 } else { 0 });
            }
            vtk.newline();

            // 1D elemental scalars
            let counts = [nb_elts_1d, nb_facets, nb_elts_3d, nb_elts_sph];
            for iefun in 0..nb_efunc_1d {
                let name = replace_underscore(&f_text_1d[iefun]);
                // Direct slice access - no Vec allocation needed
                let start = iefun * nb_elts_1d;
                let end = start + nb_elts_1d;
                write_elemental_scalar(&mut vtk, &format!("1DELEM_{}", name), &counts, 0, &efunc_1d[start..end]);
            }

            // 1D torseur values
            let tors_suffixes = ["F1", "F2", "F3", "M1", "M2", "M3", "M4", "M5", "M6"];
            for iefun in 0..nb_tors_1d {
                let name = replace_underscore(&t_text_1d[iefun]);
                let base_offset = 9 * iefun * nb_elts_1d;
                for j in 0..9usize {
                    // Use strided access - avoids Vec allocation
                    write_elemental_scalar_strided(
                        &mut vtk,
                        &format!("1DELEM_{}{}", name, tors_suffixes[j]),
                        &counts,
                        0,
                        &tors_val_1d[base_offset..],
                        9,  // stride
                        j,  // offset within stride
                        nb_elts_1d,
                    );
                }
            }

            // 2D elemental scalars
            for iefun in 0..nb_efunc {
                let name = replace_underscore(&f_text_a[iefun + nb_func]);
                // Direct slice access - no Vec allocation needed
                let start = iefun * nb_facets;
                let end = start + nb_facets;
                write_elemental_scalar(&mut vtk, &format!("2DELEM_{}", name), &counts, 1, &efunc_a[start..end]);
            }

            // 2D tensors
            for ietens in 0..nb_tens {
                let name = replace_underscore(&t_text_a[ietens]);
                // Direct slice access - tensor values are already contiguous in memory
                let start = ietens * 3 * nb_facets;
                let end = start + 3 * nb_facets;
                write_symmetric_tensor_3(&mut vtk, &format!("2DELEM_{}", name), &counts, 1, &tens_val_a[start..end]);
            }

            // 3D elemental scalars
            for iefun in 0..nb_efunc_3d {
                let name = replace_underscore(&f_text_3d[iefun]);
                // Direct slice access - no Vec allocation needed
                let start = iefun * nb_elts_3d;
                let end = start + nb_elts_3d;
                write_elemental_scalar(&mut vtk, &format!("3DELEM_{}", name), &counts, 2, &efunc_3d[start..end]);
            }

            // 3D tensors
            for ietens in 0..nb_tens_3d {
                let name = replace_underscore(&t_text_3d[ietens]);
                // Direct slice access - tensor values are already contiguous in memory
                let start = ietens * 6 * nb_elts_3d;
                let end = start + 6 * nb_elts_3d;
                write_symmetric_tensor_6(&mut vtk, &format!("3DELEM_{}", name), &counts, 2, &tens_val_3d[start..end]);
            }

            // SPH scalars and tensors
            if flag_a[7] != 0 {
                for iefun in 0..nb_efunc_sph {
                    let name = replace_underscore(&scal_text_sph[iefun]);
                    // Direct slice access - no Vec allocation needed
                    let start = iefun * nb_elts_sph;
                    let end = start + nb_elts_sph;
                    write_elemental_scalar(&mut vtk, &format!("SPHELEM_{}", name), &counts, 3, &efunc_sph[start..end]);
                }

                for ietens in 0..nb_tens_sph {
                    let name = replace_underscore(&tens_text_sph[ietens]);
                    // Direct slice access - tensor values are already contiguous in memory
                    let start = ietens * 6 * nb_elts_sph;
                    let end = start + 6 * nb_elts_sph;
                    write_symmetric_tensor_6(&mut vtk, &format!("SPHELEM_{}", name), &counts, 3, &tens_val_sph[start..end]);
                }
            }

            vtk.flush();
        }

        _ => {
            eprintln!("Error in Anim Files version");
            process::exit(1);
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <filename1> [filename2 ...] [--binary]", args[0]);
        eprintln!("  --binary : Output in binary VTK format (default is ASCII)");
        eprintln!("  Output files will have .vtk extension added automatically");
        process::exit(1);
    }
    
    // Check if --binary flag is present
    let binary_format = args.iter().any(|arg| arg == "--binary" || arg == "-b");
    
    // Collect all input files (skip program name and --binary flag)
    let input_files: Vec<&String> = args[1..]
        .iter()
        .filter(|arg| *arg != "--binary" && *arg != "-b")
        .collect();
    
    if input_files.is_empty() {
        eprintln!("Error: No input files specified");
        process::exit(1);
    }
    
    // Process each input file
    let mut failed_files = Vec::new();
    let mut successful_files = 0;
    
    for file_name in input_files {
        // Always append .vtk extension to create output filename
        let output_file_name = format!("{}.vtk", file_name);
        
        // Verify input file exists before creating output file
        if !std::path::Path::new(file_name.as_str()).exists() {
            eprintln!("Error: Input file {} does not exist", file_name);
            failed_files.push(file_name.clone());
            continue;
        }
        
        let output_file = match File::create(&output_file_name) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error: Can't create output file {}: {}", output_file_name, e);
                failed_files.push(file_name.clone());
                continue;
            }
        };
        
        eprintln!("Converting {} to {}", file_name, output_file_name);
        read_radioss_anim(file_name, binary_format, output_file);
        successful_files += 1;
    }
    
    // Report results
    if !failed_files.is_empty() {
        eprintln!("\nConversion summary: {} succeeded, {} failed", successful_files, failed_files.len());
        eprintln!("Failed files:");
        for file in &failed_files {
            eprintln!("  - {}", file);
        }
        process::exit(1);
    } else if successful_files > 1 {
        eprintln!("\nConversion complete: {} files converted successfully", successful_files);
    }
}
