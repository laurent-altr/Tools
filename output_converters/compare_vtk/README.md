# compare_vtk

A Rust utility to compare two VTK (Visualization Toolkit) files and report the maximum absolute difference between them.

## Purpose

This tool is designed to validate VTK file converters by comparing their output files. Since different implementations may use different floating-point precision when writing VTK files, this tool focuses on finding the maximum absolute difference rather than requiring exact equality.

## Features

- Supports both ASCII and binary VTK formats
- Compares:
  - Point coordinates (POINTS)
  - Cell connectivity (CELLS)
  - Cell types (CELL_TYPES)
  - Point data (scalars and vectors)
  - Cell data (scalars)
- Reports maximum absolute difference for all floating-point comparisons
- Provides detailed output showing differences per data field

## Building

### Using Cargo

From the `compare_vtk` directory:

```bash
cargo build --release
```

The executable will be in `target/release/compare_vtk` (or `target\release\compare_vtk.exe` on Windows).

## Usage

```bash
compare_vtk <file1.vtk> <file2.vtk>
```

### Example

```bash
./compare_vtk output_rust.vtk output_cpp.vtk
```

### Output

The tool will display:

1. **File information**: Number of points, cells, and data fields in each file
2. **Per-field comparison**: Maximum absolute difference for each data field (coordinates, scalars, vectors)
3. **Overall summary**: The maximum absolute difference across all comparisons
4. **Assessment**: Whether files are essentially identical, very similar, or have noticeable differences

### Example Output

```
Reading file 1: output_rust.vtk
Reading file 2: output_cpp.vtk

=== Comparison Results ===

Number of points: 1000 vs 1000
Number of cells: 500 vs 500
Number of point scalars: 3 vs 3
Number of point vectors: 1 vs 1
Number of cell scalars (int): 2 vs 2
Number of cell scalars (float): 1 vs 1
Points (coordinates): max abs diff = 1.234567e-7 (at index 42)
Point scalar 'NODE_ID': max abs diff = 0.000000e0 (at index 0)
Point scalar 'PRESSURE': max abs diff = 2.345678e-6 (at index 123)
Point vector 'VELOCITY': max abs diff = 3.456789e-7 (at index 89)
Cell scalar (float) 'STRESS': max abs diff = 1.234567e-6 (at index 234)

=== Summary ===
Maximum absolute difference: 2.345678e-6
Files are essentially identical (difference < 1e-6)

Comparison completed successfully.
```

## Exit Codes

- `0`: Comparison completed successfully
- `1`: Error occurred (file not found, format error, or dimension mismatch)

## Limitations

- Only supports legacy VTK format (not XML-based VTK formats like .vtu, .vtp, etc.)
- Integer values in CELLS and CELL_TYPES are expected to match exactly
- Does not currently compare FIELD data or TIME/CYCLE metadata

## Use Cases

1. **Validating Converters**: Compare output from Rust and C++ versions of `anim_to_vtk`
2. **Testing Precision**: Verify that precision differences in floating-point output are acceptable
3. **Regression Testing**: Ensure changes to converters don't introduce significant numerical differences
