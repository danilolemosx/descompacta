//! Linha de comando do motor, para testar sem a interface:
//!   cargo run --release -p motor --bin motor-cli -- arquivo.wav...

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arquivos: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if arquivos.is_empty() {
        eprintln!("uso: motor-cli arquivo.wav...");
        return ExitCode::from(2);
    }
    let mut ok = true;
    for a in &arquivos {
        match motor::inspecionar(a) {
            Ok(i) => println!(
                "{}: {} Hz, {} canal(is), {} bits, {:.2} s, pico {:.3}, {} amostras clipadas",
                a.display(), i.sr, i.n_canais, i.bits, i.duracao_s, i.pico, i.clip
            ),
            Err(e) => {
                eprintln!("ERRO {}: {e:#}", a.display());
                ok = false;
            }
        }
    }
    if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}
