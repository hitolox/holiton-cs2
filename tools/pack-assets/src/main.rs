use std::{
    env,
    fs::{self, File},
    io::{self, Read, Write},
    path::PathBuf,
    process::ExitCode,
};

use zip::{
    unstable::write::FileOptionsExt,
    write::{ExtendedFileOptions, FileOptions, ZipWriter},
    CompressionMethod,
};

const PASSWORD: &str = "holiton";

fn usage() {
    eprintln!(
        "Usage: pack-assets <output_zip> <file1> [file2 ...]\n\
         \n\
         Creates a ZipCrypto-encrypted ZIP archive containing the listed files.\n\
         The archive password is hard-coded to \"{}\" — its only purpose is to bypass\n\
         antivirus scanning of the bundled binaries, not to provide real security.",
        PASSWORD
    );
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        usage();
        return ExitCode::from(2);
    }

    let output = PathBuf::from(&args[1]);
    let inputs: Vec<PathBuf> = args[2..].iter().map(PathBuf::from).collect();

    if let Err(e) = pack(&output, &inputs) {
        eprintln!("error: {}", e);
        return ExitCode::FAILURE;
    }

    println!(
        "wrote {} ({} files, password: {})",
        output.display(),
        inputs.len(),
        PASSWORD
    );
    ExitCode::SUCCESS
}

fn pack(output: &PathBuf, inputs: &[PathBuf]) -> io::Result<()> {
    let file = File::create(output)?;
    let mut zip = ZipWriter::new(file);
    let options: FileOptions<'_, ExtendedFileOptions> = FileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(9))
        .with_deprecated_encryption(PASSWORD.as_bytes());

    let mut buf = Vec::new();
    for path in inputs {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "non-utf8 file name"))?;

        zip.start_file(name, options.clone())?;

        let mut f = File::open(path)?;
        buf.clear();
        f.read_to_end(&mut buf)?;
        zip.write_all(&buf)?;

        let meta = fs::metadata(path)?;
        println!("  + {} ({} bytes)", name, meta.len());
    }

    zip.finish()?;
    Ok(())
}
