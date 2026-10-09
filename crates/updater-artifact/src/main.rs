//! Offline packaging verifier using the same signature library as Tauri updater 2.10.0.
use base64::{Engine, engine::general_purpose::STANDARD};
use minisign_verify::{PublicKey, Signature};
use std::{error::Error, fs, path::Path};

fn verify(archive: &Path, signature: &Path, public_key: &Path) -> Result<(), Box<dyn Error>> {
    let key = STANDARD.decode(fs::read_to_string(public_key)?.trim())?;
    let signature = STANDARD.decode(fs::read_to_string(signature)?.trim())?;
    let key = PublicKey::decode(std::str::from_utf8(&key)?)?;
    let signature = Signature::decode(std::str::from_utf8(&signature)?)?;
    key.verify(&fs::read(archive)?, &signature, true)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: updater-artifact ARCHIVE SIGNATURE PUBLIC_KEY".into());
    }
    verify(
        Path::new(&args[0]),
        Path::new(&args[1]),
        Path::new(&args[2]),
    )?;
    println!("Updater signature verified");
    Ok(())
}
