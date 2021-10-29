use std::{
    error::Error,
    fs::{read_dir, DirEntry},
    path::PathBuf,
};

use prost_build::Config;

fn main() -> Result<(), Box<dyn Error>> {
    let mut files = Vec::new();

    let paths = read_dir("./proto")?;

    for path in paths {
        files.extend(get_files(path?)?);
    }

    let mut config = Config::default();
    config.protoc_arg("--experimental_allow_proto3_optional");

    tonic_build::configure()
        .build_client(false)
        .compile_with_config(config, &files, &["proto"])?;

    Ok(())
}

fn get_files(path: DirEntry) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    if path.file_type()?.is_file() {
        return Ok(vec![path.path()]);
    }

    let paths = read_dir(path.path())?;
    let mut files = Vec::new();

    for path in paths {
        files.extend(get_files(path?)?);
    }

    Ok(files)
}
