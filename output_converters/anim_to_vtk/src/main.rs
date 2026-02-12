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

use std::collections::BTreeSet;
use std::env;
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::process;

const FASTMAGI10: i32 = 0x542c;

// ****************************************
// read big-endian data from file
// ****************************************
fn read_i32(file: &mut File) -> i32 {
    let mut buf = [0u8; 4];
    file.read_exact(&mut buf).expect("Error in reading file");
    i32::from_be_bytes(buf)
}

fn read_f32(file: &mut File) -> f32 {
    let mut buf = [0u8; 4];
    file.read_exact(&mut buf).expect("Error in reading file");
    f32::from_be_bytes(buf)
}

fn read_i32_vec(file: &mut File, count: usize) -> Vec<i32> {
    let mut result = Vec::with_capacity(count);
    for _ in 0..count {
        result.push(read_i32(file));
    }
    result
}

fn read_f32_vec(file: &mut File, count: usize) -> Vec<f32> {
    let mut result = Vec::with_capacity(count);
    for _ in 0..count {
        result.push(read_f32(file));
    }
    result
}

fn read_u16_vec(file: &mut File, count: usize) -> Vec<u16> {
    let mut result = Vec::with_capacity(count);
    let mut buf = [0u8; 2];
    for _ in 0..count {
        file.read_exact(&mut buf).expect("Error in reading file");
        result.push(u16::from_be_bytes(buf));
    }
    result
}

fn read_bytes(file: &mut File, count: usize) -> Vec<u8> {
    let mut buf = vec![0u8; count];
    file.read_exact(&mut buf).expect("Error in reading file");
    buf
}

fn read_text(file: &mut File, count: usize) -> String {
    let buf = read_bytes(file, count);
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
// write binary data to stdout
// ****************************************
fn write_i32_binary<W: Write>(out: &mut BufWriter<W>, val: i32) {
    out.write_all(&val.to_be_bytes()).unwrap();
}

fn write_f32_binary<W: Write>(out: &mut BufWriter<W>, val: f32) {
    out.write_all(&val.to_be_bytes()).unwrap();
}

fn write_f64_binary<W: Write>(out: &mut BufWriter<W>, val: f64) {
    out.write_all(&val.to_be_bytes()).unwrap();
}

// ****************************************
// convert an A-File to vtk format (ASCII or BINARY)
// ****************************************
fn read_radioss_anim<W: Write>(file_name: &str, binary_format: bool, writer: W) {
    let mut inf = File::open(file_name).unwrap_or_else(|_| {
        eprintln!("Can't open input file {}", file_name);
        process::exit(1);
    });

    let mut out = BufWriter::new(writer);

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
            let mut nb_parts_1d: usize = 0;
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
                nb_parts_1d = read_i32(&mut inf) as usize;
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
            writeln!(out, "# vtk DataFile Version 3.0").unwrap();
            writeln!(out, "vtk output").unwrap();
            if binary_format {
                writeln!(out, "BINARY").unwrap();
            } else {
                writeln!(out, "ASCII").unwrap();
            }
            writeln!(out, "DATASET UNSTRUCTURED_GRID").unwrap();

            writeln!(out, "FIELD FieldData 2").unwrap();
            writeln!(out, "TIME 1 1 double").unwrap();
            if binary_format {
                write_f64_binary(&mut out, a_time as f64);
                writeln!(out).unwrap();
            } else {
                writeln!(out, "{}", a_time).unwrap();
            }
            writeln!(out, "CYCLE 1 1 int").unwrap();
            if binary_format {
                write_i32_binary(&mut out, 0);
                writeln!(out).unwrap();
            } else {
                writeln!(out, "0").unwrap();
            }

            // nodes
            writeln!(out, "POINTS {} float", nb_nodes).unwrap();
            if binary_format {
                for inod in 0..nb_nodes {
                    write_f32_binary(&mut out, coor_a[3 * inod]);
                    write_f32_binary(&mut out, coor_a[3 * inod + 1]);
                    write_f32_binary(&mut out, coor_a[3 * inod + 2]);
                }
            } else {
                for inod in 0..nb_nodes {
                    writeln!(
                        out,
                        "{} {} {}",
                        coor_a[3 * inod],
                        coor_a[3 * inod + 1],
                        coor_a[3 * inod + 2]
                    )
                    .unwrap();
                }
            }
            writeln!(out).unwrap();

            // detect tetrahedra in 3D cells
            let mut is_3d_cell_tetrahedron: Vec<bool> = Vec::new();
            let mut tetrahedron_count: usize = 0;
            for icon in 0..nb_elts_3d {
                let mut nodes = BTreeSet::new();
                for i in 0..8 {
                    nodes.insert(connect_3d[icon * 8 + i]);
                }
                if nodes.len() == 4 {
                    is_3d_cell_tetrahedron.push(true);
                    tetrahedron_count += 1;
                } else {
                    is_3d_cell_tetrahedron.push(false);
                }
            }

            // detect triangles in 2D cells
            let mut is_2d_triangle: Vec<bool> = Vec::new();
            let mut _triangle_count: usize = 0;
            for icon in 0..nb_facets {
                let mut nodes = BTreeSet::new();
                for i in 0..4 {
                    nodes.insert(connect_a[icon * 4 + i]);
                }
                if nodes.len() == 3 {
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
                writeln!(out, "CELLS {} {}", total_cells, cells_size).unwrap();

                if binary_format {
                    // 1D elements
                    for icon in 0..nb_elts_1d {
                        write_i32_binary(&mut out, 2);
                        write_i32_binary(&mut out, connect_1d[icon * 2]);
                        write_i32_binary(&mut out, connect_1d[icon * 2 + 1]);
                    }
                    // 2D elements
                    for icon in 0..nb_facets {
                        write_i32_binary(&mut out, 4);
                        write_i32_binary(&mut out, connect_a[icon * 4]);
                        write_i32_binary(&mut out, connect_a[icon * 4 + 1]);
                        write_i32_binary(&mut out, connect_a[icon * 4 + 2]);
                        write_i32_binary(&mut out, connect_a[icon * 4 + 3]);
                    }
                    // 3D elements
                    for icon in 0..nb_elts_3d {
                        if is_3d_cell_tetrahedron[icon] {
                            let mut nodes = BTreeSet::new();
                            for i in 0..8 {
                                nodes.insert(connect_3d[icon * 8 + i]);
                            }
                            write_i32_binary(&mut out, 4);
                            for n in &nodes {
                                write_i32_binary(&mut out, *n);
                            }
                        } else {
                            write_i32_binary(&mut out, 8);
                            for i in 0..8 {
                                write_i32_binary(&mut out, connect_3d[icon * 8 + i]);
                            }
                        }
                    }
                    // SPH elements
                    for icon in 0..nb_elts_sph {
                        write_i32_binary(&mut out, 1);
                        write_i32_binary(&mut out, connec_sph[icon]);
                    }
                } else {
                    // 1D elements
                    for icon in 0..nb_elts_1d {
                        writeln!(
                            out,
                            "2 {} {}",
                            connect_1d[icon * 2],
                            connect_1d[icon * 2 + 1]
                        )
                        .unwrap();
                    }
                    // 2D elements
                    for icon in 0..nb_facets {
                        writeln!(
                            out,
                            "4 {} {} {} {}",
                            connect_a[icon * 4],
                            connect_a[icon * 4 + 1],
                            connect_a[icon * 4 + 2],
                            connect_a[icon * 4 + 3]
                        )
                        .unwrap();
                    }
                    // 3D elements
                    for icon in 0..nb_elts_3d {
                        if is_3d_cell_tetrahedron[icon] {
                            let mut nodes = BTreeSet::new();
                            for i in 0..8 {
                                nodes.insert(connect_3d[icon * 8 + i]);
                            }
                            write!(out, "4").unwrap();
                            for n in &nodes {
                                write!(out, " {}", n).unwrap();
                            }
                            writeln!(out).unwrap();
                        } else {
                            writeln!(
                                out,
                                "8 {}  {}  {}  {}  {}  {}  {}  {}",
                                connect_3d[icon * 8],
                                connect_3d[icon * 8 + 1],
                                connect_3d[icon * 8 + 2],
                                connect_3d[icon * 8 + 3],
                                connect_3d[icon * 8 + 4],
                                connect_3d[icon * 8 + 5],
                                connect_3d[icon * 8 + 6],
                                connect_3d[icon * 8 + 7]
                            )
                            .unwrap();
                        }
                    }
                    // SPH elements
                    for icon in 0..nb_elts_sph {
                        writeln!(out, "1 {}", connec_sph[icon]).unwrap();
                    }
                }
            }
            writeln!(out).unwrap();

            // element types
            if total_cells > 0 {
                writeln!(out, "CELL_TYPES {}", total_cells).unwrap();
                if binary_format {
                    for _ in 0..nb_elts_1d {
                        write_i32_binary(&mut out, 3);
                    }
                    for icon in 0..nb_facets {
                        if is_2d_triangle[icon] {
                            write_i32_binary(&mut out, 5);
                        } else {
                            write_i32_binary(&mut out, 9);
                        }
                    }
                    for icon in 0..nb_elts_3d {
                        if is_3d_cell_tetrahedron[icon] {
                            write_i32_binary(&mut out, 10);
                        } else {
                            write_i32_binary(&mut out, 12);
                        }
                    }
                    for _ in 0..nb_elts_sph {
                        write_i32_binary(&mut out, 1);
                    }
                } else {
                    for _ in 0..nb_elts_1d {
                        writeln!(out, "3").unwrap();
                    }
                    for icon in 0..nb_facets {
                        if is_2d_triangle[icon] {
                            writeln!(out, "5").unwrap();
                        } else {
                            writeln!(out, "9").unwrap();
                        }
                    }
                    for icon in 0..nb_elts_3d {
                        if is_3d_cell_tetrahedron[icon] {
                            writeln!(out, "10").unwrap();
                        } else {
                            writeln!(out, "12").unwrap();
                        }
                    }
                    for _ in 0..nb_elts_sph {
                        writeln!(out, "1").unwrap();
                    }
                }
            }
            writeln!(out).unwrap();

            // nodal scalars & vectors
            writeln!(out, "POINT_DATA {}", nb_nodes).unwrap();

            // node id
            writeln!(out, "SCALARS NODE_ID int 1").unwrap();
            writeln!(out, "LOOKUP_TABLE default").unwrap();
            if binary_format {
                for inod in 0..nb_nodes {
                    write_i32_binary(&mut out, nod_num_a[inod]);
                }
            } else {
                for inod in 0..nb_nodes {
                    writeln!(out, "{}", nod_num_a[inod]).unwrap();
                }
            }
            writeln!(out).unwrap();

            for ifun in 0..nb_func {
                let name = replace_underscore(&f_text_a[ifun]);
                writeln!(out, "SCALARS {} float 1", name).unwrap();
                writeln!(out, "LOOKUP_TABLE default").unwrap();
                if binary_format {
                    for inod in 0..nb_nodes {
                        write_f32_binary(&mut out, func_a[ifun * nb_nodes + inod]);
                    }
                } else {
                    for inod in 0..nb_nodes {
                        writeln!(out, "{}", func_a[ifun * nb_nodes + inod]).unwrap();
                    }
                }
                writeln!(out).unwrap();
            }

            for ivect in 0..nb_vect {
                let name = replace_underscore(&v_text_a[ivect]);
                writeln!(out, "VECTORS {} float", name).unwrap();
                if binary_format {
                    for inod in 0..nb_nodes {
                        write_f32_binary(&mut out, vect_val_a[3 * inod + ivect * 3 * nb_nodes]);
                        write_f32_binary(&mut out, vect_val_a[3 * inod + 1 + ivect * 3 * nb_nodes]);
                        write_f32_binary(&mut out, vect_val_a[3 * inod + 2 + ivect * 3 * nb_nodes]);
                    }
                } else {
                    for inod in 0..nb_nodes {
                        writeln!(
                            out,
                            "{} {} {}",
                            vect_val_a[3 * inod + ivect * 3 * nb_nodes],
                            vect_val_a[3 * inod + 1 + ivect * 3 * nb_nodes],
                            vect_val_a[3 * inod + 2 + ivect * 3 * nb_nodes]
                        )
                        .unwrap();
                    }
                }
                writeln!(out).unwrap();
            }

            writeln!(out, "CELL_DATA {}", total_cells).unwrap();

            // element id
            writeln!(out, "SCALARS ELEMENT_ID int 1").unwrap();
            writeln!(out, "LOOKUP_TABLE default").unwrap();
            if binary_format {
                for iel in 0..nb_elts_1d {
                    write_i32_binary(&mut out, el_num_1d[iel]);
                }
                for iel in 0..nb_facets {
                    write_i32_binary(&mut out, el_num_a[iel]);
                }
                for iel in 0..nb_elts_3d {
                    write_i32_binary(&mut out, el_num_3d[iel]);
                }
                for iel in 0..nb_elts_sph {
                    write_i32_binary(&mut out, nod_num_sph[iel]);
                }
            } else {
                for iel in 0..nb_elts_1d {
                    writeln!(out, "{}", el_num_1d[iel]).unwrap();
                }
                for iel in 0..nb_facets {
                    writeln!(out, "{}", el_num_a[iel]).unwrap();
                }
                for iel in 0..nb_elts_3d {
                    writeln!(out, "{}", el_num_3d[iel]).unwrap();
                }
                for iel in 0..nb_elts_sph {
                    writeln!(out, "{}", nod_num_sph[iel]).unwrap();
                }
            }
            writeln!(out).unwrap();

            // part id
            writeln!(out, "SCALARS PART_ID int 1").unwrap();
            writeln!(out, "LOOKUP_TABLE default").unwrap();

            let mut part_1d_index: usize = 0;
            let mut part_2d_index: usize = 0;
            let mut part_3d_index: usize = 0;
            let mut part_0d_index: usize = 0;

            if binary_format {
                for iel in 0..nb_elts_1d {
                    if part_1d_index < nb_parts_1d && iel == def_part_1d[part_1d_index] as usize {
                        part_1d_index += 1;
                    }
                    if part_1d_index < nb_parts_1d {
                        let val: i32 = p_text_1d[part_1d_index]
                            .trim()
                            .parse()
                            .unwrap_or(0);
                        write_i32_binary(&mut out, val);
                    } else {
                        write_i32_binary(&mut out, 0);
                    }
                }
                for iel in 0..nb_facets {
                    if part_2d_index < nb_parts && iel == def_part_a[part_2d_index] as usize {
                        part_2d_index += 1;
                    }
                    if part_2d_index < nb_parts {
                        let val: i32 = p_text_a[part_2d_index]
                            .trim()
                            .parse()
                            .unwrap_or(0);
                        write_i32_binary(&mut out, val);
                    } else {
                        write_i32_binary(&mut out, 0);
                    }
                }
                for iel in 0..nb_elts_3d {
                    if part_3d_index < p_text_3d.len() && iel == def_part_3d[part_3d_index] as usize {
                        part_3d_index += 1;
                    }
                    if part_3d_index < p_text_3d.len() {
                        let val: i32 = p_text_3d[part_3d_index]
                            .trim()
                            .parse()
                            .unwrap_or(0);
                        write_i32_binary(&mut out, val);
                    } else {
                        write_i32_binary(&mut out, 0);
                    }
                }
                for iel in 0..nb_elts_sph {
                    if part_0d_index < p_text_sph.len() && iel == def_part_sph[part_0d_index] as usize {
                        part_0d_index += 1;
                    }
                    if part_0d_index < p_text_sph.len() {
                        let val: i32 = p_text_sph[part_0d_index]
                            .trim()
                            .parse()
                            .unwrap_or(0);
                        write_i32_binary(&mut out, val);
                    } else {
                        write_i32_binary(&mut out, 0);
                    }
                }
            } else {
                for iel in 0..nb_elts_1d {
                    if part_1d_index < nb_parts_1d && iel == def_part_1d[part_1d_index] as usize {
                        part_1d_index += 1;
                    }
                    if part_1d_index < nb_parts_1d {
                        let val: i32 = p_text_1d[part_1d_index]
                            .trim()
                            .parse()
                            .unwrap_or(0);
                        writeln!(out, "{}", val).unwrap();
                    } else {
                        writeln!(out, "0").unwrap();
                    }
                }
                for iel in 0..nb_facets {
                    if part_2d_index < nb_parts && iel == def_part_a[part_2d_index] as usize {
                        part_2d_index += 1;
                    }
                    if part_2d_index < nb_parts {
                        let val: i32 = p_text_a[part_2d_index]
                            .trim()
                            .parse()
                            .unwrap_or(0);
                        writeln!(out, "{}", val).unwrap();
                    } else {
                        writeln!(out, "0").unwrap();
                    }
                }
                for iel in 0..nb_elts_3d {
                    if part_3d_index < p_text_3d.len() && iel == def_part_3d[part_3d_index] as usize {
                        part_3d_index += 1;
                    }
                    if part_3d_index < p_text_3d.len() {
                        let val: i32 = p_text_3d[part_3d_index]
                            .trim()
                            .parse()
                            .unwrap_or(0);
                        writeln!(out, "{}", val).unwrap();
                    } else {
                        writeln!(out, "0").unwrap();
                    }
                }
                for iel in 0..nb_elts_sph {
                    if part_0d_index < p_text_sph.len() && iel == def_part_sph[part_0d_index] as usize {
                        part_0d_index += 1;
                    }
                    if part_0d_index < p_text_sph.len() {
                        let val: i32 = p_text_sph[part_0d_index]
                            .trim()
                            .parse()
                            .unwrap_or(0);
                        writeln!(out, "{}", val).unwrap();
                    } else {
                        writeln!(out, "0").unwrap();
                    }
                }
            }
            writeln!(out).unwrap();

            // element erosion status (0:off, 1:on)
            writeln!(out, "SCALARS EROSION_STATUS int 1").unwrap();
            writeln!(out, "LOOKUP_TABLE default").unwrap();
            if binary_format {
                for iel in 0..nb_elts_1d {
                    write_i32_binary(&mut out, if del_elt_1d[iel] != 0 { 1 } else { 0 });
                }
                for iel in 0..nb_facets {
                    write_i32_binary(&mut out, if del_elt_a[iel] != 0 { 1 } else { 0 });
                }
                for iel in 0..nb_elts_3d {
                    write_i32_binary(&mut out, if del_elt_3d[iel] != 0 { 1 } else { 0 });
                }
                for iel in 0..nb_elts_sph {
                    write_i32_binary(&mut out, if del_elt_sph[iel] != 0 { 1 } else { 0 });
                }
            } else {
                for iel in 0..nb_elts_1d {
                    writeln!(out, "{}", if del_elt_1d[iel] != 0 { 1 } else { 0 }).unwrap();
                }
                for iel in 0..nb_facets {
                    writeln!(out, "{}", if del_elt_a[iel] != 0 { 1 } else { 0 }).unwrap();
                }
                for iel in 0..nb_elts_3d {
                    writeln!(out, "{}", if del_elt_3d[iel] != 0 { 1 } else { 0 }).unwrap();
                }
                for iel in 0..nb_elts_sph {
                    writeln!(out, "{}", if del_elt_sph[iel] != 0 { 1 } else { 0 }).unwrap();
                }
            }
            writeln!(out).unwrap();

            // 1D elemental scalars
            for iefun in 0..nb_efunc_1d {
                let name = replace_underscore(&f_text_1d[iefun]);
                writeln!(out, "SCALARS 1DELEM_{} float 1", name).unwrap();
                writeln!(out, "LOOKUP_TABLE default").unwrap();
                if binary_format {
                    for iel in 0..nb_elts_1d {
                        write_f32_binary(&mut out, efunc_1d[iefun * nb_elts_1d + iel]);
                    }
                    for _ in 0..nb_facets {
                        write_f32_binary(&mut out, 0.0);
                    }
                    for _ in 0..nb_elts_3d {
                        write_f32_binary(&mut out, 0.0);
                    }
                    for _ in 0..nb_elts_sph {
                        write_f32_binary(&mut out, 0.0);
                    }
                } else {
                    for iel in 0..nb_elts_1d {
                        writeln!(out, "{}", efunc_1d[iefun * nb_elts_1d + iel]).unwrap();
                    }
                    for _ in 0..nb_facets {
                        writeln!(out, "0").unwrap();
                    }
                    for _ in 0..nb_elts_3d {
                        writeln!(out, "0").unwrap();
                    }
                    for _ in 0..nb_elts_sph {
                        writeln!(out, "0").unwrap();
                    }
                }
                writeln!(out).unwrap();
            }

            // 1D torseur values
            let tors_suffixes = ["F1", "F2", "F3", "M1", "M2", "M3", "M4", "M5", "M6"];
            for iefun in 0..nb_tors_1d {
                for j in 0..9usize {
                    let name = replace_underscore(&t_text_1d[iefun]);
                    writeln!(
                        out,
                        "SCALARS 1DELEM_{}{} float 1",
                        name, tors_suffixes[j]
                    )
                    .unwrap();
                    writeln!(out, "LOOKUP_TABLE default").unwrap();
                    if binary_format {
                        for iel in 0..nb_elts_1d {
                            write_f32_binary(
                                &mut out,
                                tors_val_1d[9 * iefun * nb_elts_1d + iel * 9 + j],
                            );
                        }
                        for _ in 0..nb_facets {
                            write_f32_binary(&mut out, 0.0);
                        }
                        for _ in 0..nb_elts_3d {
                            write_f32_binary(&mut out, 0.0);
                        }
                        for _ in 0..nb_elts_sph {
                            write_f32_binary(&mut out, 0.0);
                        }
                    } else {
                        for iel in 0..nb_elts_1d {
                            writeln!(
                                out,
                                "{}",
                                tors_val_1d[9 * iefun * nb_elts_1d + iel * 9 + j]
                            )
                            .unwrap();
                        }
                        for _ in 0..nb_facets {
                            writeln!(out, "0").unwrap();
                        }
                        for _ in 0..nb_elts_3d {
                            writeln!(out, "0").unwrap();
                        }
                        for _ in 0..nb_elts_sph {
                            writeln!(out, "0").unwrap();
                        }
                    }
                    writeln!(out).unwrap();
                }
            }

            // 2D elemental scalars
            for iefun in 0..nb_efunc {
                let name = replace_underscore(&f_text_a[iefun + nb_func]);
                writeln!(out, "SCALARS 2DELEM_{} float 1", name).unwrap();
                writeln!(out, "LOOKUP_TABLE default").unwrap();
                if binary_format {
                    for _ in 0..nb_elts_1d {
                        write_f32_binary(&mut out, 0.0);
                    }
                    for iel in 0..nb_facets {
                        write_f32_binary(&mut out, efunc_a[iefun * nb_facets + iel]);
                    }
                    for _ in 0..nb_elts_3d {
                        write_f32_binary(&mut out, 0.0);
                    }
                    for _ in 0..nb_elts_sph {
                        write_f32_binary(&mut out, 0.0);
                    }
                } else {
                    for _ in 0..nb_elts_1d {
                        writeln!(out, "0").unwrap();
                    }
                    for iel in 0..nb_facets {
                        writeln!(out, "{}", efunc_a[iefun * nb_facets + iel]).unwrap();
                    }
                    for _ in 0..nb_elts_3d {
                        writeln!(out, "0").unwrap();
                    }
                    for _ in 0..nb_elts_sph {
                        writeln!(out, "0").unwrap();
                    }
                }
                writeln!(out).unwrap();
            }

            // 2D tensors
            for ietens in 0..nb_tens {
                let name = replace_underscore(&t_text_a[ietens]);
                writeln!(out, "TENSORS 2DELEM_{} float", name).unwrap();
                if binary_format {
                    for _ in 0..nb_elts_1d {
                        for _ in 0..9 {
                            write_f32_binary(&mut out, 0.0);
                        }
                    }
                    for iel in 0..nb_facets {
                        let base = iel * 3 + ietens * 3 * nb_facets;
                        write_f32_binary(&mut out, tens_val_a[base]);
                        write_f32_binary(&mut out, tens_val_a[base + 2]);
                        write_f32_binary(&mut out, 0.0);
                        write_f32_binary(&mut out, tens_val_a[base + 2]);
                        write_f32_binary(&mut out, tens_val_a[base + 1]);
                        write_f32_binary(&mut out, 0.0);
                        write_f32_binary(&mut out, 0.0);
                        write_f32_binary(&mut out, 0.0);
                        write_f32_binary(&mut out, 0.0);
                    }
                    for _ in 0..nb_elts_3d {
                        for _ in 0..9 {
                            write_f32_binary(&mut out, 0.0);
                        }
                    }
                    for _ in 0..nb_elts_sph {
                        for _ in 0..9 {
                            write_f32_binary(&mut out, 0.0);
                        }
                    }
                } else {
                    for _ in 0..nb_elts_1d {
                        writeln!(out, "0 0 0 ").unwrap();
                        writeln!(out, "0 0 0 ").unwrap();
                        writeln!(out, "0 0 0 ").unwrap();
                    }
                    for iel in 0..nb_facets {
                        let base = iel * 3 + ietens * 3 * nb_facets;
                        writeln!(
                            out,
                            "{} {} 0 ",
                            tens_val_a[base], tens_val_a[base + 2]
                        )
                        .unwrap();
                        writeln!(
                            out,
                            "{} {} 0 ",
                            tens_val_a[base + 2], tens_val_a[base + 1]
                        )
                        .unwrap();
                        writeln!(out, "0 0 0 ").unwrap();
                    }
                    for _ in 0..nb_elts_3d {
                        writeln!(out, "0 0 0 ").unwrap();
                        writeln!(out, "0 0 0 ").unwrap();
                        writeln!(out, "0 0 0 ").unwrap();
                    }
                    for _ in 0..nb_elts_sph {
                        writeln!(out, "0 0 0 ").unwrap();
                        writeln!(out, "0 0 0 ").unwrap();
                        writeln!(out, "0 0 0 ").unwrap();
                    }
                }
                writeln!(out).unwrap();
            }

            // 3D elemental scalars
            for iefun in 0..nb_efunc_3d {
                let name = replace_underscore(&f_text_3d[iefun]);
                writeln!(out, "SCALARS 3DELEM_{} float 1", name).unwrap();
                writeln!(out, "LOOKUP_TABLE default").unwrap();
                if binary_format {
                    for _ in 0..nb_elts_1d {
                        write_f32_binary(&mut out, 0.0);
                    }
                    for _ in 0..nb_facets {
                        write_f32_binary(&mut out, 0.0);
                    }
                    for iel in 0..nb_elts_3d {
                        write_f32_binary(&mut out, efunc_3d[iefun * nb_elts_3d + iel]);
                    }
                    for _ in 0..nb_elts_sph {
                        write_f32_binary(&mut out, 0.0);
                    }
                } else {
                    for _ in 0..nb_elts_1d {
                        writeln!(out, "0").unwrap();
                    }
                    for _ in 0..nb_facets {
                        writeln!(out, "0").unwrap();
                    }
                    for iel in 0..nb_elts_3d {
                        writeln!(out, "{}", efunc_3d[iefun * nb_elts_3d + iel]).unwrap();
                    }
                    for _ in 0..nb_elts_sph {
                        writeln!(out, "0").unwrap();
                    }
                }
                writeln!(out).unwrap();
            }

            // 3D tensors
            for ietens in 0..nb_tens_3d {
                let name = replace_underscore(&t_text_3d[ietens]);
                writeln!(out, "TENSORS 3DELEM_{} float", name).unwrap();
                if binary_format {
                    for _ in 0..nb_elts_1d {
                        for _ in 0..9 {
                            write_f32_binary(&mut out, 0.0);
                        }
                    }
                    for _ in 0..nb_facets {
                        for _ in 0..9 {
                            write_f32_binary(&mut out, 0.0);
                        }
                    }
                    for iel in 0..nb_elts_3d {
                        let base = iel * 6 + ietens * 6 * nb_elts_3d;
                        write_f32_binary(&mut out, tens_val_3d[base]);
                        write_f32_binary(&mut out, tens_val_3d[base + 3]);
                        write_f32_binary(&mut out, tens_val_3d[base + 4]);
                        write_f32_binary(&mut out, tens_val_3d[base + 3]);
                        write_f32_binary(&mut out, tens_val_3d[base + 1]);
                        write_f32_binary(&mut out, tens_val_3d[base + 5]);
                        write_f32_binary(&mut out, tens_val_3d[base + 4]);
                        write_f32_binary(&mut out, tens_val_3d[base + 5]);
                        write_f32_binary(&mut out, tens_val_3d[base + 2]);
                    }
                    for _ in 0..nb_elts_sph {
                        for _ in 0..9 {
                            write_f32_binary(&mut out, 0.0);
                        }
                    }
                } else {
                    for _ in 0..nb_elts_1d {
                        writeln!(out, "0 0 0 ").unwrap();
                        writeln!(out, "0 0 0 ").unwrap();
                        writeln!(out, "0 0 0 ").unwrap();
                    }
                    for _ in 0..nb_facets {
                        writeln!(out, "0 0 0 ").unwrap();
                        writeln!(out, "0 0 0 ").unwrap();
                        writeln!(out, "0 0 0 ").unwrap();
                    }
                    for iel in 0..nb_elts_3d {
                        let base = iel * 6 + ietens * 6 * nb_elts_3d;
                        writeln!(
                            out,
                            "{} {} {}",
                            tens_val_3d[base],
                            tens_val_3d[base + 3],
                            tens_val_3d[base + 4]
                        )
                        .unwrap();
                        writeln!(
                            out,
                            "{} {} {}",
                            tens_val_3d[base + 3],
                            tens_val_3d[base + 1],
                            tens_val_3d[base + 5]
                        )
                        .unwrap();
                        writeln!(
                            out,
                            "{} {} {}",
                            tens_val_3d[base + 4],
                            tens_val_3d[base + 5],
                            tens_val_3d[base + 2]
                        )
                        .unwrap();
                    }
                    for _ in 0..nb_elts_sph {
                        writeln!(out, "0 0 0 ").unwrap();
                        writeln!(out, "0 0 0 ").unwrap();
                        writeln!(out, "0 0 0 ").unwrap();
                    }
                }
                writeln!(out).unwrap();
            }

            // SPH scalars and tensors
            if flag_a[7] != 0 {
                for iefun in 0..nb_efunc_sph {
                    let name = replace_underscore(&scal_text_sph[iefun]);
                    writeln!(out, "SCALARS SPHELEM_{} float 1", name).unwrap();
                    writeln!(out, "LOOKUP_TABLE default").unwrap();
                    if binary_format {
                        for _ in 0..nb_elts_1d {
                            write_f32_binary(&mut out, 0.0);
                        }
                        for _ in 0..nb_facets {
                            write_f32_binary(&mut out, 0.0);
                        }
                        for _ in 0..nb_elts_3d {
                            write_f32_binary(&mut out, 0.0);
                        }
                        for iel in 0..nb_elts_sph {
                            write_f32_binary(&mut out, efunc_sph[iefun * nb_elts_sph + iel]);
                        }
                    } else {
                        for _ in 0..nb_elts_1d {
                            writeln!(out, "0").unwrap();
                        }
                        for _ in 0..nb_facets {
                            writeln!(out, "0").unwrap();
                        }
                        for _ in 0..nb_elts_3d {
                            writeln!(out, "0").unwrap();
                        }
                        for iel in 0..nb_elts_sph {
                            writeln!(out, "{}", efunc_sph[iefun * nb_elts_sph + iel]).unwrap();
                        }
                    }
                    writeln!(out).unwrap();
                }

                for ietens in 0..nb_tens_sph {
                    let name = replace_underscore(&tens_text_sph[ietens]);
                    writeln!(out, "TENSORS SPHELEM_{} float", name).unwrap();
                    if binary_format {
                        for _ in 0..nb_elts_1d {
                            for _ in 0..9 {
                                write_f32_binary(&mut out, 0.0);
                            }
                        }
                        for _ in 0..nb_facets {
                            for _ in 0..9 {
                                write_f32_binary(&mut out, 0.0);
                            }
                        }
                        for _ in 0..nb_elts_3d {
                            for _ in 0..9 {
                                write_f32_binary(&mut out, 0.0);
                            }
                        }
                        for iel in 0..nb_elts_sph {
                            let base = iel * 6 + ietens * 6 * nb_elts_sph;
                            write_f32_binary(&mut out, tens_val_sph[base]);
                            write_f32_binary(&mut out, tens_val_sph[base + 3]);
                            write_f32_binary(&mut out, tens_val_sph[base + 4]);
                            write_f32_binary(&mut out, tens_val_sph[base + 3]);
                            write_f32_binary(&mut out, tens_val_sph[base + 1]);
                            write_f32_binary(&mut out, tens_val_sph[base + 5]);
                            write_f32_binary(&mut out, tens_val_sph[base + 4]);
                            write_f32_binary(&mut out, tens_val_sph[base + 5]);
                            write_f32_binary(&mut out, tens_val_sph[base + 2]);
                        }
                    } else {
                        for _ in 0..nb_elts_1d {
                            writeln!(out, "0 0 0 ").unwrap();
                            writeln!(out, "0 0 0 ").unwrap();
                            writeln!(out, "0 0 0 ").unwrap();
                        }
                        for _ in 0..nb_facets {
                            writeln!(out, "0 0 0 ").unwrap();
                            writeln!(out, "0 0 0 ").unwrap();
                            writeln!(out, "0 0 0 ").unwrap();
                        }
                        for _ in 0..nb_elts_3d {
                            writeln!(out, "0 0 0 ").unwrap();
                            writeln!(out, "0 0 0 ").unwrap();
                            writeln!(out, "0 0 0 ").unwrap();
                        }
                        for iel in 0..nb_elts_sph {
                            let base = iel * 6 + ietens * 6 * nb_elts_sph;
                            writeln!(
                                out,
                                "{} {} {}",
                                tens_val_sph[base],
                                tens_val_sph[base + 3],
                                tens_val_sph[base + 4]
                            )
                            .unwrap();
                            writeln!(
                                out,
                                "{} {} {}",
                                tens_val_sph[base + 3],
                                tens_val_sph[base + 1],
                                tens_val_sph[base + 5]
                            )
                            .unwrap();
                            writeln!(
                                out,
                                "{} {} {}",
                                tens_val_sph[base + 4],
                                tens_val_sph[base + 5],
                                tens_val_sph[base + 2]
                            )
                            .unwrap();
                        }
                    }
                    writeln!(out).unwrap();
                }
            }
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
        // Determine output filename - don't add .vtk if it already exists
        let output_file_name = if file_name.ends_with(".vtk") {
            file_name.clone()
        } else {
            format!("{}.vtk", file_name)
        };
        
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
