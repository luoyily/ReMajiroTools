use clap::Parser;
use formats::image::{parse_majiro_image_with_name, ImageError, MajiroImage};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
fn img_err(e: ImageError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e.to_string())
}
#[derive(Parser, Debug)]
#[command(name = "rct2png", about = "Convert Majiro Engine .rct / .rc8 images to PNG")]
struct Args {
    #[arg(short, long)]
    input: PathBuf,
    #[arg(short, long)]
    output: Option<PathBuf>,
    #[arg(short = 'I', long)]
    info: bool,
    #[arg(short, long)]
    batch: bool,
    #[arg(short, long)]
    force: bool,
    #[arg(short, long)]
    verbose: bool,
    #[arg(long)]
    raw: bool,
    #[arg(long)]
    check: bool,
}
fn main() -> io::Result<()> {
    let args = Args::parse();
    if args.batch || args.input.is_dir() {
        batch_convert(&args)
    } else {
        convert_single(&args)
    }
}
fn convert_single(args: &Args) -> io::Result<()> {
    let data = fs::read(&args.input)?;
    let fname = args.input.file_name().and_then(|s| s.to_str()).unwrap_or("unknown");
    let (img, _alpha_used) = load_with_alpha(&data, fname, &args.input, args.raw)
        .map_err(img_err)?;
    if args.info || args.check {
        print_info(&args.input, &img);
        return Ok(());
    }
    let out_path = match &args.output {
        Some(p) => p.clone(),
        None => {
            let stem = args
                .input
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            PathBuf::from(format!("{}.png", stem))
        }
    };
    write_png(&img, &out_path, args.force)?;
    if args.verbose {
        eprintln!(
            "  {} → {}  ({}x{}, {})", args.input.display(), out_path.display(), img
            .width, img.height, img.subtype
        );
    }
    Ok(())
}
fn batch_convert(args: &Args) -> io::Result<()> {
    let out_dir = match &args.output {
        Some(p) => p.clone(),
        None => PathBuf::from("./png_output"),
    };
    fs::create_dir_all(&out_dir)?;
    let mut count = 0u64;
    let mut errors = 0u64;
    let mut total_pixels = 0u64;
    for entry in fs::read_dir(&args.input)? {
        let entry = entry?;
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext != "rct" && ext != "rc8" {
            continue;
        }
        let stem_str = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if !args.raw && stem_str.ends_with('_') {
            continue;
        }
        let out_path = out_dir.join(format!("{}.png", stem_str));
        if !args.check && out_path.exists() && !args.force {
            if args.verbose {
                eprintln!("  skip {} (output exists)", path.display());
            }
            continue;
        }
        let data = match fs::read(&path) {
            Ok(d) => d,
            Err(e) => {
                errors += 1;
                eprintln!("  Error reading {}: {}", path.display(), e);
                continue;
            }
        };
        let fname = path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown");
        let (img, alpha_used) = match load_with_alpha(&data, fname, &path, args.raw) {
            Ok(v) => v,
            Err(e) => {
                errors += 1;
                eprintln!("  Error parsing {}: {}", path.display(), e);
                continue;
            }
        };
        if args.info {
            print_info(&path, &img);
        }
        if args.check {
            count += 1;
            total_pixels += (img.width as u64) * (img.height as u64);
            continue;
        }
        match write_png(&img, &out_path, args.force) {
            Ok(()) => {
                count += 1;
                total_pixels += (img.width as u64) * (img.height as u64);
                if args.verbose {
                    let alpha_mark = if alpha_used { " α" } else { "" };
                    eprintln!(
                        "  {:>4} {} → {}  ({}x{}, {}{})", count, path.file_name()
                        .unwrap_or_default().to_string_lossy(), out_path.display(), img
                        .width, img.height, img.subtype, alpha_mark
                    );
                }
            }
            Err(e) => {
                errors += 1;
                eprintln!("  Error writing {}: {}", out_path.display(), e);
            }
        }
    }
    let action = if args.check { "Validated" } else { "Converted" };
    eprintln!(
        "{} {} images ({:.1} MP total), {} errors", action, count, total_pixels as f64 /
        1_000_000.0, errors
    );
    if args.check && errors > 0 {
        Err(
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{errors} image(s) failed validation"),
            ),
        )
    } else {
        Ok(())
    }
}
fn load_with_alpha(
    data: &[u8],
    fname: &str,
    path: &Path,
    raw: bool,
) -> Result<(MajiroImage, bool), ImageError> {
    let mut img = parse_majiro_image_with_name(data, Some(fname))?;
    let alpha_used = if raw {
        false
    } else {
        apply_colorkey(&mut img, path);
        try_apply_alpha(&mut img, path)
    };
    Ok((img, alpha_used))
}
fn apply_colorkey(img: &mut MajiroImage, main_path: &Path) -> bool {
    let class_name = match img.class_name.as_ref() {
        Some(s) => s.trim_end_matches('\0').trim(),
        None => return false,
    };
    if class_name.is_empty() {
        return false;
    }
    let src_path = main_path.parent().unwrap_or(Path::new(".")).join(class_name);
    let src_data = match fs::read(&src_path) {
        Ok(d) => d,
        Err(_) => return false,
    };
    let src_fname = src_path.file_name().and_then(|s| s.to_str()).unwrap_or(class_name);
    let src = match parse_majiro_image_with_name(&src_data, Some(src_fname)) {
        Ok(i) => i,
        Err(_) => return false,
    };
    if src.width != img.width || src.height != img.height {
        eprintln!(
            "    Note: color-key source {} ({}x{}) size mismatch with {} ({}x{})",
            class_name, src.width, src.height, main_path.file_name().unwrap_or_default()
            .to_string_lossy(), img.width, img.height
        );
    }
    let pixel_count = (img.width as usize) * (img.height as usize);
    let src_count = (src.width as usize) * (src.height as usize);
    let mut replaced = 0u64;
    for i in 0..pixel_count {
        let o = i * 4;
        if o + 2 >= img.pixels.len() {
            break;
        }
        if img.pixels[o] == 0xFF && img.pixels[o + 1] == 0 && img.pixels[o + 2] == 0
            && i < src_count
        {
            let so = i * 4;
            if so + 2 < src.pixels.len() {
                img.pixels[o] = src.pixels[so];
                img.pixels[o + 1] = src.pixels[so + 1];
                img.pixels[o + 2] = src.pixels[so + 2];
                replaced += 1;
            }
        }
    }
    replaced > 0
}
fn try_apply_alpha(img: &mut MajiroImage, main_path: &Path) -> bool {
    let stem = main_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if stem.is_empty() {
        return false;
    }
    for suffix in &["_", "_alfa", "_alpha"] {
        for ext in &["rc8", "rct"] {
            let alpha_path = main_path
                .parent()
                .unwrap_or(Path::new("."))
                .join(format!("{}{}.{}", stem, suffix, ext));
            if let Some(alpha_data) = try_load_alpha_file(&alpha_path) {
                apply_alpha_pixels(img, &alpha_data);
                return true;
            }
        }
    }
    false
}
fn try_load_alpha_file(path: &Path) -> Option<Vec<u8>> {
    let data = fs::read(path).ok()?;
    let fname = path.file_name()?.to_str()?;
    let alpha_img = parse_majiro_image_with_name(&data, Some(fname)).ok()?;
    let gray: Vec<u8> = alpha_img
        .pixels
        .chunks(4)
        .map(|rgba| {
            let r = rgba[0] as f32;
            let g = rgba[1] as f32;
            let b = rgba[2] as f32;
            (0.299f32 * r + 0.587f32 * g + 0.114f32 * b) as u8
        })
        .collect();
    Some(gray)
}
fn apply_alpha_pixels(img: &mut MajiroImage, alpha: &[u8]) {
    let pixel_count = (img.width as usize) * (img.height as usize);
    let alpha_count = alpha.len();
    if alpha_count == 0 {
        return;
    }
    if alpha_count != pixel_count {
        eprintln!(
            "    Note: alpha size mismatch (alpha={}, image={}), applying best effort",
            alpha_count, pixel_count
        );
    }
    for (i, alpha_value) in alpha.iter().take(pixel_count).enumerate() {
        let out_off = i * 4;
        if out_off + 3 < img.pixels.len() {
            img.pixels[out_off + 3] = 255 - alpha_value;
        }
    }
}
fn print_info(path: &Path, img: &MajiroImage) {
    println!("File:      {}", path.display());
    println!("Format:    {}", img.subtype);
    println!(
        "Dimensions: {} × {}  ({:.1} MP)", img.width, img.height, (img.width as f64 *
        img.height as f64) / 1_000_000.0
    );
    if let Some(ref class) = img.class_name {
        println!("Class:     {} (color-key source)", class);
    }
    println!();
}
fn write_png(img: &MajiroImage, path: &Path, force: bool) -> io::Result<()> {
    if path.exists() && !force {
        return Err(
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{} exists (use --force to overwrite)", path.display()),
            ),
        );
    }
    let file = fs::File::create(path)?;
    let w = &mut io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(w, img.width, img.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| io::Error::other(format!("PNG header: {}", e)))?;
    writer
        .write_image_data(&img.pixels)
        .map_err(|e| io::Error::other(format!("PNG write: {}", e)))?;
    Ok(())
}
