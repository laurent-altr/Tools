// Copyright 1986-2026 Altair Engineering Inc.
// SPDX-License-Identifier: MIT

/// All FE data parsed from an Abaqus .inp file.
#[derive(Default)]
pub struct Model {
    pub nodes: Vec<Node>,
    pub elem_blocks: Vec<ElemBlock>,
    /// Node sets: ordered (name, node-id list)
    pub nsets: Vec<(String, Vec<u32>)>,
    /// Element sets: ordered (name, elem-id list)
    pub elsets: Vec<(String, Vec<u32>)>,
    /// Materials: ordered by first occurrence
    pub materials: Vec<(String, Material)>,
    pub sections: Vec<Section>,
    pub boundaries: Vec<Boundary>,
    pub amplitudes: Vec<(String, Vec<(f64, f64)>)>,
    pub contacts: Vec<Contact>,
    pub ties: Vec<Tie>,
    pub surfaces: Vec<Surface>,
    /// Engine control parameters
    pub run_time: f64,
    pub dt: f64,
}

pub struct Node {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

pub struct ElemBlock {
    pub kind: ElemKind,
    pub elset: String,
    pub elems: Vec<Elem>,
}

pub struct Elem {
    pub id: u32,
    pub nodes: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ElemKind {
    Sh3n,
    Shell,
    Penta,
    Tetra4,
    Brick,
    Tetra10,
    Spring,
    Cohesive,
    Mass,
}

impl ElemKind {
    pub fn rad_kw(self) -> &'static str {
        match self {
            ElemKind::Sh3n => "SH3N",
            ElemKind::Shell => "SHELL",
            ElemKind::Penta => "PENTA6",
            ElemKind::Tetra4 => "TETRA",
            ElemKind::Brick | ElemKind::Cohesive => "BRICK",
            ElemKind::Tetra10 => "TETRA10",
            ElemKind::Spring => "SPRING",
            ElemKind::Mass => "SPRING", // written as SPRING with one node
        }
    }
}

#[derive(Default)]
pub struct Material {
    pub rho: f64,
    pub e: f64,
    pub nu: f64,
    /// Isotropic plasticity: (yield_stress, plastic_strain) pairs
    pub plastic: Vec<(f64, f64)>,
    /// Neo-Hookean: C10 parameter
    pub neo_hooke_c10: Option<f64>,
}

pub struct Section {
    pub elset: String,
    pub material: String,
    pub kind: SectionKind,
}

#[derive(Clone)]
pub enum SectionKind {
    /// Shell with thickness
    Shell(f64),
    /// Membrane with thickness (no bending stiffness)
    Membrane(f64),
    Solid,
    Cohesive,
}

pub struct Boundary {
    pub nset: String,
    /// Radioss constraint mask: [TX, TY, TZ, RX, RY, RZ]
    pub mask: [u8; 6],
    pub value: f64,
    pub amplitude: String,
}

pub struct Contact {
    pub name: String,
    pub slave: String,
    pub master: String,
    pub friction: f64,
}

pub struct Tie {
    pub name: String,
    pub slave: String,
    pub master: String,
}

pub struct Surface {
    pub name: String,
    pub entries: Vec<(String, String)>, // (elset_or_node, face_label)
    pub node_type: bool,
}
