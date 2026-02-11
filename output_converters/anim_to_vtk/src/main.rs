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
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::process;

const FASTMAGI10: i32 = 0x542c;

// ****************************************
// read big-endian data from a buffered reader
// ****************************************
fn read_i32(reader: &mut impl Read) -> i32 {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).expect("Error in reading file");
    i32::from_be_bytes(buf)
}

fn read_f32(reader: &mut impl Read) -> f32 {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).expect("Error in reading file");
    f32::from_be_bytes(buf)
}

fn read_i32_vec(reader: &mut impl Read, count: usize) -> Vec<i32> {
    let mut buf = vec![0u8; count * 4];
    reader.read_exact(&mut buf).expect("Error in reading file");
    buf.chunks_exact(4)
        .map(|c| i32::from_be_bytes(c.try_into().unwrap()))
        .collect()
}

fn read_f32_vec(reader: &mut impl Read, count: usize) -> Vec<f32> {
    let mut buf = vec![0u8; count * 4];
    reader.read_exact(&mut buf).expect("Error in reading file");
    buf.chunks_exact(4)
        .map(|c| f32::from_be_bytes(c.try_into().unwrap()))
        .collect()
}

fn read_u16_vec(reader: &mut impl Read, count: usize) -> Vec<u16> {
    let mut buf = vec![0u8; count * 2];
    reader.read_exact(&mut buf).expect("Error in reading file");
    buf.chunks_exact(2)
        .map(|c| u16::from_be_bytes(c.try_into().unwrap()))
        .collect()
}

fn read_bytes(reader: &mut impl Read, count: usize) -> Vec<u8> {
    let mut buf = vec![0u8; count];
    reader.read_exact(&mut buf).expect("Error in reading file");
    buf
}

fn read_text(reader: &mut impl Read, count: usize) -> String {
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
// VTK output helpers
// ****************************************
fn write_zero_scalar_rows(out: &mut impl Write, count: usize) {
    for _ in 0..count {
        writeln!(out, "0").unwrap();
    }
}

fn write_zero_tensor_rows(out: &mut impl Write, count: usize) {
    for _ in 0..count {
        writeln!(out, "0 0 0 ").unwrap();
        writeln!(out, "0 0 0 ").unwrap();
        writeln!(out, "0 0 0 ").unwrap();
    }
}

fn write_symmetric_tensor(out: &mut impl Write, vals: &[f32], base: usize) {
    writeln!(out, "{} {} {}", vals[base], vals[base + 3], vals[base + 4]).unwrap();
    writeln!(out, "{} {} {}", vals[base + 3], vals[base + 1], vals[base + 5]).unwrap();
    writeln!(out, "{} {} {}", vals[base + 4], vals[base + 5], vals[base + 2]).unwrap();
}

// ****************************************
// convert an A-File to ascii vtk format
// ****************************************
fn read_radioss_anim(file_name: &str) {
    let file = File::open(file_name).unwrap_or_else(|_| {
        eprintln!("Can't open input file {}", file_name);
        process::exit(1);
    });
    let mut inf = BufReader::new(file);

    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());

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
            // VTK ASCII output
            // ********************
            writeln!(out, "# vtk DataFile Version 3.0").unwrap();
            writeln!(out, "vtk output").unwrap();
            writeln!(out, "ASCII").unwrap();
            writeln!(out, "DATASET UNSTRUCTURED_GRID").unwrap();

            writeln!(out, "FIELD FieldData 2").unwrap();
            writeln!(out, "TIME 1 1 double").unwrap();
            writeln!(out, "{}", a_time).unwrap();
            writeln!(out, "CYCLE 1 1 int").unwrap();
            writeln!(out, "0").unwrap();

            // nodes
            writeln!(out, "POINTS {} float", nb_nodes).unwrap();
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
            writeln!(out).unwrap();

            // detect tetrahedra in 3D cells and cache unique node sets
            let mut tetrahedron_count: usize = 0;
            let tet_unique_nodes: Vec<Option<Vec<i32>>> = (0..nb_elts_3d)
                .map(|icon| {
                    let mut nodes = BTreeSet::new();
                    for i in 0..8 {
                        nodes.insert(connect_3d[icon * 8 + i]);
                    }
                    if nodes.len() == 4 {
                        tetrahedron_count += 1;
                        Some(nodes.into_iter().collect())
                    } else {
                        None
                    }
                })
                .collect();

            // detect triangles in 2D cells
            let is_2d_triangle: Vec<bool> = (0..nb_facets)
                .map(|icon| {
                    let mut nodes = BTreeSet::new();
                    for i in 0..4 {
                        nodes.insert(connect_a[icon * 4 + i]);
                    }
                    nodes.len() == 3
                })
                .collect();

            let total_cells = nb_elts_1d + nb_facets + nb_elts_3d + nb_elts_sph;
            if total_cells > 0 {
                let cells_size = nb_elts_1d * 3
                    + nb_facets * 5
                    + tetrahedron_count * 5
                    + (nb_elts_3d - tetrahedron_count) * 9
                    + nb_elts_sph * 2;
                writeln!(out, "CELLS {} {}", total_cells, cells_size).unwrap();

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
                for (icon, tet_nodes) in tet_unique_nodes.iter().enumerate() {
                    if let Some(nodes) = tet_nodes {
                        write!(out, "4").unwrap();
                        for n in nodes {
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
                for sph_node in connec_sph.iter().take(nb_elts_sph) {
                    writeln!(out, "1 {}", sph_node).unwrap();
                }
            }
            writeln!(out).unwrap();

            // element types
            if total_cells > 0 {
                writeln!(out, "CELL_TYPES {}", total_cells).unwrap();
                for _ in 0..nb_elts_1d {
                    writeln!(out, "3").unwrap();
                }
                for &is_tri in &is_2d_triangle {
                    if is_tri {
                        writeln!(out, "5").unwrap();
                    } else {
                        writeln!(out, "9").unwrap();
                    }
                }
                for tet_nodes in &tet_unique_nodes {
                    if tet_nodes.is_some() {
                        writeln!(out, "10").unwrap();
                    } else {
                        writeln!(out, "12").unwrap();
                    }
                }
                for _ in 0..nb_elts_sph {
                    writeln!(out, "1").unwrap();
                }
            }
            writeln!(out).unwrap();

            // nodal scalars & vectors
            writeln!(out, "POINT_DATA {}", nb_nodes).unwrap();

            // node id
            writeln!(out, "SCALARS NODE_ID int 1").unwrap();
            writeln!(out, "LOOKUP_TABLE default").unwrap();
            for id in &nod_num_a {
                writeln!(out, "{}", id).unwrap();
            }
            writeln!(out).unwrap();

            for ifun in 0..nb_func {
                let name = replace_underscore(&f_text_a[ifun]);
                writeln!(out, "SCALARS {} float 1", name).unwrap();
                writeln!(out, "LOOKUP_TABLE default").unwrap();
                for inod in 0..nb_nodes {
                    writeln!(out, "{}", func_a[ifun * nb_nodes + inod]).unwrap();
                }
                writeln!(out).unwrap();
            }

            for ivect in 0..nb_vect {
                let name = replace_underscore(&v_text_a[ivect]);
                writeln!(out, "VECTORS {} float", name).unwrap();
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
                writeln!(out).unwrap();
            }

            writeln!(out, "CELL_DATA {}", total_cells).unwrap();

            // element id
            writeln!(out, "SCALARS ELEMENT_ID int 1").unwrap();
            writeln!(out, "LOOKUP_TABLE default").unwrap();
            for id in &el_num_1d {
                writeln!(out, "{}", id).unwrap();
            }
            for id in &el_num_a {
                writeln!(out, "{}", id).unwrap();
            }
            for id in &el_num_3d {
                writeln!(out, "{}", id).unwrap();
            }
            for id in &nod_num_sph {
                writeln!(out, "{}", id).unwrap();
            }
            writeln!(out).unwrap();

            // part id
            writeln!(out, "SCALARS PART_ID int 1").unwrap();
            writeln!(out, "LOOKUP_TABLE default").unwrap();

            let mut part_1d_index: usize = 0;
            let mut part_2d_index: usize = 0;
            let mut part_3d_index: usize = 0;
            let mut part_0d_index: usize = 0;

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
            writeln!(out).unwrap();

            // element erosion status (0:off, 1:on)
            writeln!(out, "SCALARS EROSION_STATUS int 1").unwrap();
            writeln!(out, "LOOKUP_TABLE default").unwrap();
            for &d in &del_elt_1d {
                writeln!(out, "{}", if d != 0 { 1 } else { 0 }).unwrap();
            }
            for &d in &del_elt_a {
                writeln!(out, "{}", if d != 0 { 1 } else { 0 }).unwrap();
            }
            for &d in &del_elt_3d {
                writeln!(out, "{}", if d != 0 { 1 } else { 0 }).unwrap();
            }
            for &d in &del_elt_sph {
                writeln!(out, "{}", if d != 0 { 1 } else { 0 }).unwrap();
            }
            writeln!(out).unwrap();

            // 1D elemental scalars
            for iefun in 0..nb_efunc_1d {
                let name = replace_underscore(&f_text_1d[iefun]);
                writeln!(out, "SCALARS 1DELEM_{} float 1", name).unwrap();
                writeln!(out, "LOOKUP_TABLE default").unwrap();
                for iel in 0..nb_elts_1d {
                    writeln!(out, "{}", efunc_1d[iefun * nb_elts_1d + iel]).unwrap();
                }
                write_zero_scalar_rows(&mut out, nb_facets + nb_elts_3d + nb_elts_sph);
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
                    for iel in 0..nb_elts_1d {
                        writeln!(
                            out,
                            "{}",
                            tors_val_1d[9 * iefun * nb_elts_1d + iel * 9 + j]
                        )
                        .unwrap();
                    }
                    write_zero_scalar_rows(&mut out, nb_facets + nb_elts_3d + nb_elts_sph);
                    writeln!(out).unwrap();
                }
            }

            // 2D elemental scalars
            for iefun in 0..nb_efunc {
                let name = replace_underscore(&f_text_a[iefun + nb_func]);
                writeln!(out, "SCALARS 2DELEM_{} float 1", name).unwrap();
                writeln!(out, "LOOKUP_TABLE default").unwrap();
                write_zero_scalar_rows(&mut out, nb_elts_1d);
                for iel in 0..nb_facets {
                    writeln!(out, "{}", efunc_a[iefun * nb_facets + iel]).unwrap();
                }
                write_zero_scalar_rows(&mut out, nb_elts_3d + nb_elts_sph);
                writeln!(out).unwrap();
            }

            // 2D tensors
            for (ietens, t_name) in t_text_a.iter().enumerate().take(nb_tens) {
                let name = replace_underscore(t_name);
                writeln!(out, "TENSORS 2DELEM_{} float", name).unwrap();
                write_zero_tensor_rows(&mut out, nb_elts_1d);
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
                write_zero_tensor_rows(&mut out, nb_elts_3d + nb_elts_sph);
                writeln!(out).unwrap();
            }

            // 3D elemental scalars
            for iefun in 0..nb_efunc_3d {
                let name = replace_underscore(&f_text_3d[iefun]);
                writeln!(out, "SCALARS 3DELEM_{} float 1", name).unwrap();
                writeln!(out, "LOOKUP_TABLE default").unwrap();
                write_zero_scalar_rows(&mut out, nb_elts_1d + nb_facets);
                for iel in 0..nb_elts_3d {
                    writeln!(out, "{}", efunc_3d[iefun * nb_elts_3d + iel]).unwrap();
                }
                write_zero_scalar_rows(&mut out, nb_elts_sph);
                writeln!(out).unwrap();
            }

            // 3D tensors
            for (ietens, t_name) in t_text_3d.iter().enumerate().take(nb_tens_3d) {
                let name = replace_underscore(t_name);
                writeln!(out, "TENSORS 3DELEM_{} float", name).unwrap();
                write_zero_tensor_rows(&mut out, nb_elts_1d + nb_facets);
                for iel in 0..nb_elts_3d {
                    let base = iel * 6 + ietens * 6 * nb_elts_3d;
                    write_symmetric_tensor(&mut out, &tens_val_3d, base);
                }
                write_zero_tensor_rows(&mut out, nb_elts_sph);
                writeln!(out).unwrap();
            }

            // SPH scalars and tensors
            if flag_a[7] != 0 {
                for iefun in 0..nb_efunc_sph {
                    let name = replace_underscore(&scal_text_sph[iefun]);
                    writeln!(out, "SCALARS SPHELEM_{} float 1", name).unwrap();
                    writeln!(out, "LOOKUP_TABLE default").unwrap();
                    write_zero_scalar_rows(&mut out, nb_elts_1d + nb_facets + nb_elts_3d);
                    for iel in 0..nb_elts_sph {
                        writeln!(out, "{}", efunc_sph[iefun * nb_elts_sph + iel]).unwrap();
                    }
                    writeln!(out).unwrap();
                }

                for (ietens, t_name) in tens_text_sph.iter().enumerate().take(nb_tens_sph) {
                    let name = replace_underscore(t_name);
                    writeln!(out, "TENSORS SPHELEM_{} float", name).unwrap();
                    write_zero_tensor_rows(&mut out, nb_elts_1d + nb_facets + nb_elts_3d);
                    for iel in 0..nb_elts_sph {
                        let base = iel * 6 + ietens * 6 * nb_elts_sph;
                        write_symmetric_tensor(&mut out, &tens_val_sph, base);
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
        eprintln!("Call a filename");
        process::exit(1);
    }
    read_radioss_anim(&args[1]);
}
