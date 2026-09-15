use clap::Parser;
use formats::archive::{ArcIndex, Archive};
use std::fs;
use std::io;
use std::path::PathBuf;
use tools::format_size;
#[derive(Parser, Debug)]
#[command(name = "mjo-arc")]
#[command(about = "Extract files from Majiro Engine .arc archives")]
struct Args {
    #[arg(short, long)]
    input: PathBuf,
    #[arg(short, long, default_value = "./output")]
    output: PathBuf,
    #[arg(short, long)]
    list: bool,
    #[arg(short, long)]
    verbose: bool,
    #[arg(long)]
    check: bool,
    #[arg(short = 'j', long)]
    decode_sjis: bool,
}
fn main() -> io::Result<()> {
    let args = Args::parse();
    if args.check {
        let index = ArcIndex::open(&args.input)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        println!("{:?} archive: {} valid entries", index.version(), index.entry_count());
        return Ok(());
    }
    let data = fs::read(&args.input)?;
    let archive = Archive::parse_with(&data, args.decode_sjis)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let arc_name = args.input.file_stem().and_then(|s| s.to_str()).unwrap_or("archive");
    if args.list {
        print_list(&archive, arc_name);
        return Ok(());
    }
    let out_dir = args.output.join(arc_name);
    fs::create_dir_all(&out_dir)?;
    eprintln!("Extracting {} entries to {}", archive.entries.len(), out_dir.display());
    for entry in archive.entries.iter() {
        let relative = safe_relative_path(&entry.name)?;
        let out_path = out_dir.join(relative);
        if out_path.exists() {
            if let Ok(existing) = fs::read(&out_path) {
                if existing == entry.data {
                    continue;
                }
            }
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&out_path, &entry.data)?;
        if args.verbose {
            eprintln!("  write {}  {}", entry.name, format_size(entry.data.len()));
        }
    }
    eprintln!("Done. {} files.", archive.entries.len());
    Ok(())
}
fn safe_relative_path(name: &str) -> io::Result<PathBuf> {
    use std::path::{Component, Path};
    let path = Path::new(name);
    if path
        .components()
        .any(|component| {
            matches!(
                component, Component::Prefix(_) | Component::RootDir |
                Component::ParentDir
            )
        })
    {
        return Err(
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("archive entry escapes output directory: {name:?}"),
            ),
        );
    }
    Ok(path.to_path_buf())
}
fn print_list(archive: &Archive, _arc_name: &str) {
    println!(
        "{:?} archive  {} entries  {} total", archive.version, archive.entries.len(),
        format_size(archive.entries.iter().map(| e | e.data.len()).sum::< usize > ())
    );
    println!("{:-<65}", "");
    for (i, entry) in archive.entries.iter().enumerate() {
        println!("  {:4}  {:>10}  {}", i, format_size(entry.data.len()), entry.name);
    }
}

