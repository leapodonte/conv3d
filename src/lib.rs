pub mod gltf_builder;
pub use gltf_builder::*;

pub use gltf::json;
pub use json::validation::Checked::Valid;

use clap::clap_derive::ValueEnum;
use std::path::Path;
use stl_io::IndexedMesh;

#[derive(Debug, Clone, ValueEnum, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileFormat {
    Stl,
    Gltf,
    Glb,
}

pub fn get_extension(format: FileFormat) -> &'static str {
    match format {
        FileFormat::Stl => "stl",
        FileFormat::Gltf => "gltf",
        FileFormat::Glb => "glb",
    }
}

/// Calculate bounding coordinates of a list of vertices, used for the clipping distance of the model
pub fn bounding_coords(points: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::MAX, f32::MAX, f32::MAX];
    let mut max = [f32::MIN, f32::MIN, f32::MIN];

    for point in points {
        for i in 0..3 {
            min[i] = f32::min(min[i], point[i]);
            max[i] = f32::max(max[i], point[i]);
        }
    }
    (min, max)
}

pub fn convert_stl_to_gltf(
    stl: IndexedMesh,
    input_filename: impl AsRef<Path>,
) -> Result<GltfBuilder, String> {
    let mesh_name = input_filename
        .as_ref()
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut gltf = GltfBuilder::new();
    let with_indices = true;

    let (positions, mut normals) = stl
        .vertices
        .iter()
        .map(|it| ([it[0], it[1], it[2]], [0.0, 0.0, 0.0]))
        .collect::<(Vec<[f32; 3]>, Vec<[f32; 3]>)>();

    let mut normals_count = vec![0; normals.len()];
    for face in &stl.faces {
        for vi in face.vertices {
            normals[vi][0] += face.normal[0];
            normals[vi][1] += face.normal[1];
            normals[vi][2] += face.normal[2];
            normals_count[vi] += 1;
        }
    }

    // normalize
    for i in 0..normals.len() {
        let n = normals[i];
        let count = normals_count[i] as f32;
        normals[i] = [n[0] / count, n[1] / count, n[2] / count];
    }

    let (min, max) = bounding_coords(&positions);
    println!("min: {min:?} max: {max:?}");
    let vcount = positions.len();

    let positions_view =
        gltf.push_buffer_with_view(Some("positions".to_string()), positions, None, None);

    let normals_view = gltf.push_buffer_with_view(Some("normals".to_string()), normals, None, None);

    let positions = gltf.push_accessor_vec3(
        Some("positions".to_string()),
        positions_view,
        0,
        vcount,
        Some(min),
        Some(max),
    );
    let normals = gltf.push_accessor_vec3(
        Some("normals".to_string()),
        normals_view,
        3,
        vcount,
        None,
        None,
    );

    let indices = stl
        .faces
        .iter()
        .flat_map(|it| {
            [
                it.vertices[0] as u32,
                it.vertices[1] as u32,
                it.vertices[2] as u32,
            ]
        })
        .collect::<Vec<_>>();
    let nb_indices = indices.len();
    let indices_view =
        gltf.push_buffer_with_view(Some("indices".to_string()), indices, Some(1), None);
    let indices = if with_indices {
        Some(gltf.push_accessor_u32(Some("indices".to_string()), indices_view, 0, nb_indices))
    } else {
        None
    };

    let primitive = json::mesh::Primitive {
        attributes: {
            let mut map = std::collections::BTreeMap::new();
            map.insert(Valid(json::mesh::Semantic::Positions), positions);
            map.insert(Valid(json::mesh::Semantic::Normals), normals);
            map
        },
        extensions: Default::default(),
        extras: Default::default(),
        indices,
        material: None,
        mode: Valid(json::mesh::Mode::Triangles),
        targets: None,
    };

    let mesh = gltf.push_mesh(Some(mesh_name.clone()), vec![primitive], None);
    let node = gltf.push_node(Some(mesh_name), Some(mesh), None, None);
    let scene = gltf.push_scene(vec![node]);
    gltf.set_default_scene(Some(scene));

    Ok(gltf)
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{File, OpenOptions},
        io::BufWriter,
        path::Path,
    };

    use crate::convert_stl_to_gltf;

    #[test]
    fn test_glb() {
        let path = std::env::current_dir().unwrap().join("data/torus.stl");
        let mut file = OpenOptions::new()
            .read(true)
            .open(path.clone())
            .unwrap_or_else(|_| panic!("Unable to open {}", path.clone().display()));
        let stl = stl_io::read_stl(&mut file)
            .unwrap_or_else(|_| panic!("Unable to parse {}", path.display()));
        println!("Parsed {}", path.display());
        let mut outpath = path.clone();
        outpath.set_extension("glb");
        if outpath != *path {
            let gltf = convert_stl_to_gltf(stl, path).unwrap();
            let file = File::create(outpath.clone()).unwrap();
            let writer = BufWriter::new(file);
            let glb = gltf.to_glb().unwrap();
            glb.to_writer(writer).unwrap();

            println!("Output: {}", outpath.display());
        }
    }

    #[test]
    fn test_gltf() {
        let path = std::env::current_dir().unwrap().join("data/torus.stl");
        let mut file = OpenOptions::new()
            .read(true)
            .open(path.clone())
            .unwrap_or_else(|_| panic!("Unable to open {}", path.clone().display()));
        let stl = stl_io::read_stl(&mut file)
            .unwrap_or_else(|_| panic!("Unable to parse {}", path.display()));
        println!("Parsed {}", path.display());
        let mut outpath = path.clone();
        outpath.set_extension("gltf");
        if outpath != *path {
            let gltf = convert_stl_to_gltf(stl, path).unwrap();
            let file = File::create(outpath.clone()).unwrap();
            let writer = BufWriter::new(file);
            let mut gltf = gltf.merge_gltf_buffers().unwrap();
            gltf.set_buffer_uri(
                0,
                Some(format!(
                    "{}.bin",
                    outpath
                        .file_stem()
                        .unwrap_or_default()
                        .to_str()
                        .unwrap_or_default()
                )),
            )
            .unwrap();
            gltf.write_to_gltf(writer).unwrap();
            gltf.write_all_buffers(outpath.parent().unwrap_or(Path::new(".")))
                .unwrap();

            println!("Output: {}", outpath.display());
        }
    }
}
