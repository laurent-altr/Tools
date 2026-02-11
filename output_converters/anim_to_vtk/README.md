# anim_to_vtk

anim_to_vtk is an external tool to convert OpenRadioss animation files to legacy VTK format (ASCII or binary).

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

Apply anim_to_vtk to each animation file to generate ASCII VTK format (default):

        ./anim_to_vtk_linux64_gf [Deck Rootname]A001 > [Deck Rootname]_001.vtk
        ./anim_to_vtk_linux64_gf [Deck Rootname]A002 > [Deck Rootname]_002.vtk
        ...

To generate binary VTK format (smaller file size, faster I/O), use the `--binary` flag:

        ./anim_to_vtk_linux64_gf [Deck Rootname]A001 --binary > [Deck Rootname]_001.vtk
        ./anim_to_vtk_linux64_gf [Deck Rootname]A002 --binary > [Deck Rootname]_002.vtk
        ...

### Batch Conversion Script

Following Linux bash script can be used to convert all files in a single task:

        #!/bin/bash
        #
        # Script to be launch in Animation file directory
        #
        Rootname=[Deck Rootname]
        OpenRadioss_root=[Path to OpenRadioss installation]
        # Set FORMAT to "--binary" for binary VTK or "" for ASCII VTK
        FORMAT=""
        for file in `ls ${Rootname}A*`
        do
          animation_number=${file#"${Rootname}A"}
          ${OpenRadioss_root}/exec/anim_to_vtk_linuxa64_gf $file ${FORMAT} > ${Rootname}_${animation_number}.vtk
        done

In Paraview, the vtk files are bundled and can be loaded in one step.

### Output Format Options

- **ASCII format** (default): Human-readable text format, larger file size
- **Binary format** (`--binary` or `-b` flag): Compact binary format with approximately 70-80% smaller file size and faster loading times in visualization software
