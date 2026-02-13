# anim_to_vtk

anim_to_vtk is an external tool to convert OpenRadioss animation files to legacy VTK format (ASCII or binary) or UNV (Universal File Format).

## How to build

A Rust toolchain installation is required. Install from https://rustup.rs/

### Linux

Enter the platform directory : anim_to_vtk/linux64
Apply the build script : ./build.bash

Executable will be copied in [OpenRadioss]/exec directory

### Linux ARM64

Enter the platform directory : anim_to_vtk/linuxa64
Apply the build script : ./build.bash

Executable will be copied in [OpenRadioss]/exec directory

### Windows

Enter the platform directory : anim_to_vtk/win64
Apply the script : build.bat

Executable is copied in [OpenRadioss]/exec

### Using Cargo directly

From the anim_to_vtk directory:

        cargo build --release

The executable will be in target/release/anim_to_vtk (or target\release\anim_to_vtk.exe on Windows).

## How to use

### Basic Usage

The tool automatically creates output files with `.vtk` or `.unv` extension added to the input filename.

#### Convert a single file

Generate ASCII VTK format (default):

        ./anim_to_vtk_linux64_gf [Deck Rootname]A001

This creates `[Deck Rootname]A001.vtk`

To generate binary VTK format (smaller file size, faster I/O), use the `--binary` flag:

        ./anim_to_vtk_linux64_gf [Deck Rootname]A001 --binary

To generate UNV format, use the `--unv` flag:

        ./anim_to_vtk_linux64_gf [Deck Rootname]A001 --unv

This creates `[Deck Rootname]A001.unv`

#### Convert multiple files

You can convert multiple files in a single command:

        ./anim_to_vtk_linux64_gf [Deck Rootname]A001 [Deck Rootname]A002 [Deck Rootname]A003

Or with binary VTK format:

        ./anim_to_vtk_linux64_gf [Deck Rootname]A001 [Deck Rootname]A002 [Deck Rootname]A003 --binary

Or with UNV format:

        ./anim_to_vtk_linux64_gf [Deck Rootname]A001 [Deck Rootname]A002 [Deck Rootname]A003 --unv

The format flags (`--binary` or `--unv`) can be placed anywhere in the command line arguments.

#### Convert all animation files using wildcards

Using shell wildcards to convert all animation files at once:

        ./anim_to_vtk_linux64_gf [Deck Rootname]A*

Or with binary VTK format:

        ./anim_to_vtk_linux64_gf [Deck Rootname]A* --binary

Or with UNV format:

        ./anim_to_vtk_linux64_gf [Deck Rootname]A* --unv

### Legacy Batch Conversion Script (Optional)

The following Linux bash script can still be used for more complex batch processing:

        #!/bin/bash
        #
        # Script to be launched in Animation file directory
        #
        Rootname=[Deck Rootname]
        OpenRadioss_root=[Path to OpenRadioss installation]
        # Use "--binary" for binary VTK, "--unv" for UNV, or leave empty for ASCII VTK
        FORMAT="--binary"
        ${OpenRadioss_root}/exec/anim_to_vtk_linuxa64_gf ${Rootname}A* ${FORMAT}

In Paraview, the vtk files are bundled and can be loaded in one step.

### Output Format Options

- **ASCII VTK format** (default): Human-readable text format, larger file size
- **Binary VTK format** (`--binary` or `-b` flag): Compact binary format with approximately 70-80% smaller file size and faster loading times in visualization software
- **UNV format** (`--unv` flag): Universal File Format, a text-based format commonly used in FEA applications. Compatible with various pre/post-processing tools.

### UNV Format Details

The UNV (Universal File Format) output includes:

- **Dataset 2411**: Node definitions with coordinates
  - Node labels (IDs)
  - X, Y, Z coordinates in scientific notation
  
- **Dataset 2412**: Element definitions with connectivity
  - 1D elements (beams): Element type 11
  - 2D elements (shells): Element type 44 (quads) or 91 (triangles)
  - 3D elements (solids): Element type 115 (8-node bricks)
  - SPH elements: Element type 136 (point elements)

The UNV format is compatible with many FEA pre/post-processors including:
- Siemens NX
- FEMAP
- ANSA
- Other tools supporting the Universal File Format

**Note**: The current UNV implementation focuses on geometry export (nodes and elements). Result data (stresses, strains, etc.) is not yet included in UNV output but may be added in future versions.

## Performance

The Rust implementation is significantly faster than previous C++ implementations due to:
- Specialized number formatting libraries (ryu, itoa)
- Efficient buffered I/O strategy
- Zero-allocation data processing
- Reusable scratch buffers

For detailed performance analysis and optimization techniques, see [PERFORMANCE.md](PERFORMANCE.md).
