use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process;

#[derive(Debug, Clone)]
struct VtkData {
    points: Vec<f32>,
    cells: Vec<i32>,
    cell_types: Vec<i32>,
    point_scalars: Vec<(String, Vec<f32>)>,
    point_vectors: Vec<(String, Vec<f32>)>,
    cell_scalars: Vec<(String, Vec<i32>)>,
    cell_scalars_float: Vec<(String, Vec<f32>)>,
}

impl VtkData {
    fn new() -> Self {
        VtkData {
            points: Vec::new(),
            cells: Vec::new(),
            cell_types: Vec::new(),
            point_scalars: Vec::new(),
            point_vectors: Vec::new(),
            cell_scalars: Vec::new(),
            cell_scalars_float: Vec::new(),
        }
    }
}

fn read_vtk_file<P: AsRef<Path>>(filename: P) -> Result<VtkData, String> {
    let file = File::open(filename.as_ref())
        .map_err(|e| format!("Failed to open file: {}", e))?;
    
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    
    // Read first line - VTK version
    reader.read_line(&mut line)
        .map_err(|e| format!("Failed to read VTK header: {}", e))?;
    
    if !line.starts_with("# vtk DataFile") {
        return Err("Invalid VTK file format".to_string());
    }
    
    // Read title line
    line.clear();
    reader.read_line(&mut line)
        .map_err(|e| format!("Failed to read title: {}", e))?;
    
    // Read format line (ASCII or BINARY)
    line.clear();
    reader.read_line(&mut line)
        .map_err(|e| format!("Failed to read format: {}", e))?;
    let is_binary = line.trim() == "BINARY";
    
    if is_binary {
        read_binary_vtk(reader)
    } else {
        read_ascii_vtk(reader)
    }
}

fn read_ascii_vtk<R: BufRead>(mut reader: R) -> Result<VtkData, String> {
    let mut data = VtkData::new();
    let mut line = String::new();
    
    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)
            .map_err(|e| format!("Failed to read line: {}", e))?;
        
        if bytes_read == 0 {
            break; // EOF
        }
        
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        
        if trimmed.starts_with("POINTS") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                let num_points: usize = parts[1].parse()
                    .map_err(|_| "Failed to parse number of points".to_string())?;
                data.points = read_float_values(&mut reader, num_points * 3)?;
            }
        } else if trimmed.starts_with("CELLS") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 3 {
                let _num_cells: usize = parts[1].parse()
                    .map_err(|_| "Failed to parse number of cells".to_string())?;
                let cell_size: usize = parts[2].parse()
                    .map_err(|_| "Failed to parse cell size".to_string())?;
                data.cells = read_int_values(&mut reader, cell_size)?;
            }
        } else if trimmed.starts_with("CELL_TYPES") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                let num_types: usize = parts[1].parse()
                    .map_err(|_| "Failed to parse number of cell types".to_string())?;
                data.cell_types = read_int_values(&mut reader, num_types)?;
            }
        } else if trimmed.starts_with("POINT_DATA") {
            read_point_data(&mut reader, &mut data)?;
        } else if trimmed.starts_with("CELL_DATA") {
            read_cell_data(&mut reader, &mut data)?;
        }
    }
    
    Ok(data)
}

fn read_binary_vtk<R: Read>(mut reader: R) -> Result<VtkData, String> {
    let mut data = VtkData::new();
    let mut buffer = Vec::new();
    
    // Read all remaining data
    reader.read_to_end(&mut buffer)
        .map_err(|e| format!("Failed to read binary data: {}", e))?;
    
    let text = String::from_utf8_lossy(&buffer);
    let lines: Vec<&str> = text.lines().collect();
    
    let mut i = 0;
    let mut binary_offset = 0;
    
    // Find where binary data starts (after the last text line before numeric data)
    for (idx, line) in lines.iter().enumerate() {
        if line.trim().starts_with("POINTS") {
            i = idx;
            break;
        }
    }
    
    // Count text bytes to find binary offset
    for j in 0..i {
        binary_offset += lines[j].len() + 1; // +1 for newline
    }
    
    while i < lines.len() {
        let line = lines[i].trim();
        
        if line.starts_with("POINTS") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let num_points: usize = parts[1].parse()
                    .map_err(|_| "Failed to parse number of points".to_string())?;
                binary_offset += line.len() + 1;
                data.points = read_binary_floats(&buffer[binary_offset..], num_points * 3);
                binary_offset += num_points * 3 * 4;
            }
        } else if line.starts_with("CELLS") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let _num_cells: usize = parts[1].parse()
                    .map_err(|_| "Failed to parse number of cells".to_string())?;
                let cell_size: usize = parts[2].parse()
                    .map_err(|_| "Failed to parse cell size".to_string())?;
                binary_offset += line.len() + 1;
                data.cells = read_binary_ints(&buffer[binary_offset..], cell_size);
                binary_offset += cell_size * 4;
            }
        } else if line.starts_with("CELL_TYPES") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let num_types: usize = parts[1].parse()
                    .map_err(|_| "Failed to parse number of cell types".to_string())?;
                binary_offset += line.len() + 1;
                data.cell_types = read_binary_ints(&buffer[binary_offset..], num_types);
                binary_offset += num_types * 4;
            }
        } else if !line.is_empty() {
            binary_offset += line.len() + 1;
        }
        
        i += 1;
    }
    
    Ok(data)
}

fn read_binary_floats(buffer: &[u8], count: usize) -> Vec<f32> {
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let offset = i * 4;
        if offset + 4 <= buffer.len() {
            let bytes = [buffer[offset], buffer[offset + 1], buffer[offset + 2], buffer[offset + 3]];
            result.push(f32::from_be_bytes(bytes));
        }
    }
    result
}

fn read_binary_ints(buffer: &[u8], count: usize) -> Vec<i32> {
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let offset = i * 4;
        if offset + 4 <= buffer.len() {
            let bytes = [buffer[offset], buffer[offset + 1], buffer[offset + 2], buffer[offset + 3]];
            result.push(i32::from_be_bytes(bytes));
        }
    }
    result
}

fn read_float_values<R: BufRead>(reader: &mut R, count: usize) -> Result<Vec<f32>, String> {
    let mut values = Vec::with_capacity(count);
    let mut line = String::new();
    
    while values.len() < count {
        line.clear();
        reader.read_line(&mut line)
            .map_err(|e| format!("Failed to read float values: {}", e))?;
        
        for token in line.split_whitespace() {
            if values.len() >= count {
                break;
            }
            let val: f32 = token.parse()
                .map_err(|_| format!("Failed to parse float: {}", token))?;
            values.push(val);
        }
    }
    
    Ok(values)
}

fn read_int_values<R: BufRead>(reader: &mut R, count: usize) -> Result<Vec<i32>, String> {
    let mut values = Vec::with_capacity(count);
    let mut line = String::new();
    
    while values.len() < count {
        line.clear();
        reader.read_line(&mut line)
            .map_err(|e| format!("Failed to read int values: {}", e))?;
        
        for token in line.split_whitespace() {
            if values.len() >= count {
                break;
            }
            let val: i32 = token.parse()
                .map_err(|_| format!("Failed to parse int: {}", token))?;
            values.push(val);
        }
    }
    
    Ok(values)
}

fn read_point_data<R: BufRead>(reader: &mut R, data: &mut VtkData) -> Result<(), String> {
    let mut line = String::new();
    
    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)
            .map_err(|e| format!("Failed to read line: {}", e))?;
        
        if bytes_read == 0 {
            break;
        }
        
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        
        if trimmed.starts_with("CELL_DATA") {
            // Reached cell data, stop reading point data
            read_cell_data(reader, data)?;
            break;
        } else if trimmed.starts_with("SCALARS") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                let name = parts[1].to_string();
                
                // Read LOOKUP_TABLE line
                line.clear();
                reader.read_line(&mut line)
                    .map_err(|e| format!("Failed to read LOOKUP_TABLE: {}", e))?;
                
                // We need to know the count - use existing points count / 3
                let num_values = data.points.len() / 3;
                let values = read_float_values(reader, num_values)?;
                data.point_scalars.push((name, values));
            }
        } else if trimmed.starts_with("VECTORS") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                let name = parts[1].to_string();
                
                // Vectors have 3 components per point
                let num_values = (data.points.len() / 3) * 3;
                let values = read_float_values(reader, num_values)?;
                data.point_vectors.push((name, values));
            }
        }
    }
    
    Ok(())
}

fn read_cell_data<R: BufRead>(reader: &mut R, data: &mut VtkData) -> Result<(), String> {
    let mut line = String::new();
    
    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)
            .map_err(|e| format!("Failed to read line: {}", e))?;
        
        if bytes_read == 0 {
            break;
        }
        
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        
        if trimmed.starts_with("SCALARS") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 3 {
                let name = parts[1].to_string();
                let data_type = parts[2].to_string();
                
                // Read LOOKUP_TABLE line
                line.clear();
                reader.read_line(&mut line)
                    .map_err(|e| format!("Failed to read LOOKUP_TABLE: {}", e))?;
                
                let num_values = data.cell_types.len();
                
                if data_type == "int" {
                    let values = read_int_values(reader, num_values)?;
                    data.cell_scalars.push((name, values));
                } else {
                    let values = read_float_values(reader, num_values)?;
                    data.cell_scalars_float.push((name, values));
                }
            }
        }
    }
    
    Ok(())
}

fn compare_vtk_files(file1: &str, file2: &str) -> Result<(), String> {
    println!("Reading file 1: {}", file1);
    let data1 = read_vtk_file(file1)?;
    
    println!("Reading file 2: {}", file2);
    let data2 = read_vtk_file(file2)?;
    
    println!("\n=== Comparison Results ===\n");
    
    // Compare dimensions
    println!("Number of points: {} vs {}", data1.points.len() / 3, data2.points.len() / 3);
    println!("Number of cells: {} vs {}", data1.cell_types.len(), data2.cell_types.len());
    println!("Number of point scalars: {} vs {}", data1.point_scalars.len(), data2.point_scalars.len());
    println!("Number of point vectors: {} vs {}", data1.point_vectors.len(), data2.point_vectors.len());
    println!("Number of cell scalars (int): {} vs {}", data1.cell_scalars.len(), data2.cell_scalars.len());
    println!("Number of cell scalars (float): {} vs {}", data1.cell_scalars_float.len(), data2.cell_scalars_float.len());
    
    if data1.points.len() != data2.points.len() {
        return Err("Different number of points".to_string());
    }
    
    if data1.cell_types.len() != data2.cell_types.len() {
        return Err("Different number of cells".to_string());
    }
    
    let mut max_diff_overall = 0.0f32;
    
    // Compare points
    let max_diff_points = compare_float_arrays(&data1.points, &data2.points, "Points (coordinates)");
    max_diff_overall = max_diff_overall.max(max_diff_points);
    
    // Compare point scalars
    for (name1, values1) in &data1.point_scalars {
        if let Some((_, values2)) = data2.point_scalars.iter().find(|(n, _)| n == name1) {
            let max_diff = compare_float_arrays(values1, values2, &format!("Point scalar '{}'", name1));
            max_diff_overall = max_diff_overall.max(max_diff);
        } else {
            println!("Warning: Point scalar '{}' not found in file 2", name1);
        }
    }
    
    // Compare point vectors
    for (name1, values1) in &data1.point_vectors {
        if let Some((_, values2)) = data2.point_vectors.iter().find(|(n, _)| n == name1) {
            let max_diff = compare_float_arrays(values1, values2, &format!("Point vector '{}'", name1));
            max_diff_overall = max_diff_overall.max(max_diff);
        } else {
            println!("Warning: Point vector '{}' not found in file 2", name1);
        }
    }
    
    // Compare cell scalars (float)
    for (name1, values1) in &data1.cell_scalars_float {
        if let Some((_, values2)) = data2.cell_scalars_float.iter().find(|(n, _)| n == name1) {
            let max_diff = compare_float_arrays(values1, values2, &format!("Cell scalar (float) '{}'", name1));
            max_diff_overall = max_diff_overall.max(max_diff);
        } else {
            println!("Warning: Cell scalar (float) '{}' not found in file 2", name1);
        }
    }
    
    println!("\n=== Summary ===");
    println!("Maximum absolute difference: {:.6e}", max_diff_overall);
    
    if max_diff_overall < 1e-6 {
        println!("Files are essentially identical (difference < 1e-6)");
    } else if max_diff_overall < 1e-3 {
        println!("Files are very similar (difference < 1e-3)");
    } else {
        println!("Files have noticeable differences");
    }
    
    Ok(())
}

fn compare_float_arrays(arr1: &[f32], arr2: &[f32], name: &str) -> f32 {
    if arr1.len() != arr2.len() {
        println!("{}: Different array sizes ({} vs {})", name, arr1.len(), arr2.len());
        return f32::INFINITY;
    }
    
    let mut max_diff = 0.0f32;
    let mut max_diff_idx = 0;
    
    for (i, (&v1, &v2)) in arr1.iter().zip(arr2.iter()).enumerate() {
        let diff = (v1 - v2).abs();
        if diff > max_diff {
            max_diff = diff;
            max_diff_idx = i;
        }
    }
    
    println!("{}: max abs diff = {:.6e} (at index {})", name, max_diff, max_diff_idx);
    max_diff
}

fn print_usage(program: &str) {
    eprintln!("Usage: {} <file1.vtk> <file2.vtk>", program);
    eprintln!();
    eprintln!("Compare two VTK files and report the maximum absolute difference.");
    eprintln!("This tool handles both ASCII and binary VTK formats.");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() != 3 {
        print_usage(&args[0]);
        process::exit(1);
    }
    
    let file1 = &args[1];
    let file2 = &args[2];
    
    match compare_vtk_files(file1, file2) {
        Ok(()) => {
            println!("\nComparison completed successfully.");
            process::exit(0);
        }
        Err(e) => {
            eprintln!("\nError: {}", e);
            process::exit(1);
        }
    }
}
