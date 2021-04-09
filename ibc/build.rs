use std::{
    error::Error,
    fs::{read_dir, DirEntry},
    path::PathBuf,
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut files = Vec::new();

    let paths = read_dir("./proto")?;

    for path in paths {
        files.extend(get_files(path?)?);
    }

    tonic_build::configure()
        .build_server(false)
        .compile(&files, &["proto".into()])?;

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
