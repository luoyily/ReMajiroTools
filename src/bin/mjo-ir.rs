use clap::{Parser, Subcommand};
use formats::script::MjoFile;
use std::collections::BTreeMap;
use std::path::PathBuf;
use vm::ir::{from_text, to_mjo_bytes, to_text, IrItem, IrModule};
#[derive(Parser, Debug)]
#[command(name = "mjo-ir")]
#[command(about = "Convert Majiro .mjo scripts to/from the lossless text IR")]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand, Debug)]
enum Command {
    Export {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long)]
        v2: bool,
    },
    Import { #[arg(short, long)] input: PathBuf, #[arg(short, long)] output: PathBuf },
    Check { #[arg(short, long)] input: PathBuf },
    Localize {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        catalog: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    match args.command {
        Command::Export { input, output, v2 } => {
            let data = std::fs::read(&input)?;
            let mjo = MjoFile::parse(&data)?;
            let module = if v2 {
                IrModule::from_mjo_v2(&mjo)
            } else {
                IrModule::from_mjo(&mjo)
            };
            let text = to_text(&module);
            match output {
                Some(path) => std::fs::write(path, text)?,
                None => print!("{text}"),
            }
        }
        Command::Import { input, output } => {
            let text = std::fs::read_to_string(&input)?;
            let module = from_text(&text)?;
            std::fs::write(output, to_mjo_bytes(&module)?)?;
        }
        Command::Check { input } => {
            let text = std::fs::read_to_string(&input)?;
            let module = from_text(&text)
                .map_err(|e| format!("{}: {e}", input.display()))?;
            let (mut errors, notes) = vm::ir::check_module(&module);
            for note in &notes {
                println!("[check] {} :: {note}", input.display());
            }
            match to_mjo_bytes(&module) {
                Ok(_) => println!("[check] {} :: assembles cleanly", input.display()),
                Err(e) => errors.push(format!("assembly: {e}")),
            }
            if errors.is_empty() {
                println!("[check] {} :: OK", input.display());
            } else {
                for e in &errors {
                    eprintln!("[check] {} :: ERROR {e}", input.display());
                }
                return Err(
                    format!(
                        "{}: check failed with {} error(s)", input.display(), errors
                        .len()
                    )
                        .into(),
                );
            }
        }
        Command::Localize { input, catalog, output } => {
            let data = std::fs::read(&input)?;
            let mjo = MjoFile::parse(&data)?;
            let mut module = IrModule::from_mjo(&mjo);
            let raw: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(&catalog)?,
            )?;
            let mut messages: BTreeMap<u32, (Option<String>, String)> = BTreeMap::new();
            if let Some(entries) = raw.get("messages").and_then(|m| m.as_object()) {
                for (key, value) in entries {
                    let offset = u32::from_str_radix(key.trim_start_matches("0x"), 16)
                        .map_err(|e| format!("bad catalog offset {key:?}: {e}"))?;
                    let source = value
                        .get("source")
                        .and_then(|v| v.as_str())
                        .map(str::to_owned);
                    let text = value
                        .get("text")
                        .and_then(|v| v.as_str())
                        .map(str::to_owned)
                        .ok_or_else(|| format!("catalog entry {key} lacks \"text\""))?;
                    messages.insert(offset, (source, text));
                }
            }
            let mut attached = 0usize;
            let skipped_not_text_line = 0usize;
            let mut body = std::mem::take(&mut module.body);
            let mut with_tr: Vec<IrItem> = Vec::with_capacity(
                body.len() + messages.len(),
            );
            let mut offset = 0usize;
            for item in body.drain(..) {
                let mut pending_message = None;
                if let IrItem::Insn { opcode: 0x841, .. } = &item {
                    if let Some(message) = messages.get(&(offset as u32)) {
                        pending_message = Some(message.clone());
                    }
                }
                let item_size = match &item {
                    IrItem::Insn { args, .. } => 2 + vm::ir::args_len(args),
                    IrItem::RawBytes(bytes) => bytes.len(),
                    _ => 0,
                };
                with_tr.push(item);
                if let Some((source, text)) = pending_message {
                    with_tr.push(IrItem::Tr { source, text });
                    attached += 1;
                }
                offset += item_size;
            }
            let orphans = messages.len() - attached;
            let _ = skipped_not_text_line;
            module.body = with_tr;
            std::fs::write(&output, to_text(&module))?;
            eprintln!(
                "[LOCALIZE] {} -> {} (attached {attached}, unmatched {orphans})", input
                .display(), output.display()
            );
        }
    }
    Ok(())
}
