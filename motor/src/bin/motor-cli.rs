//! Linha de comando do motor, para testar sem a interface. Faz o mesmo que soltar os arquivos na janela:
//!   cargo run --release -p motor --bin motor-cli -- [--7z] [--nivel rapido|normal|maximo] [--senha S]
//!       [--partes MB] [--separado] caminho...

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

use motor::{Formato, Nivel, Opcoes, Tarefa};

fn main() -> ExitCode {
    let mut o = Opcoes { um_zip_so: true, ..Default::default() };
    let mut caminhos: Vec<PathBuf> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--7z" => o.formato = Formato::SeteZ,
            "--separado" => o.um_zip_so = false,
            "--senha" => o.senha = args.next(),
            "--partes" => o.partes_mb = args.next().and_then(|s| s.parse().ok()),
            "--nivel" => {
                o.nivel = match args.next().as_deref() {
                    Some("rapido") => Nivel::Rapido,
                    Some("maximo") => Nivel::Maximo,
                    _ => Nivel::Normal,
                }
            }
            _ => caminhos.push(PathBuf::from(a)),
        }
    }
    if caminhos.is_empty() {
        eprintln!("uso: motor-cli [--7z] [--nivel rapido|normal|maximo] [--senha S] [--partes MB] [--separado] caminho...");
        return ExitCode::from(2);
    }
    let cancelar = AtomicBool::new(false);
    let mut ok = true;
    for t in motor::planejar(&caminhos, o.um_zip_so, o.formato) {
        let mut prog = |p: u8| eprint!("\r{p:3}%");
        let r = match &t {
            Tarefa::Descompactar { arquivo } => {
                eprintln!("descompactando {}", arquivo.display());
                motor::descompactar(arquivo, None, o.senha.as_deref(), &mut prog, &cancelar).map(|p| vec![p])
            }
            Tarefa::Compactar { itens, destino } => {
                eprintln!("compactando {} item(ns) em {}", itens.len(), destino.display());
                motor::compactar(itens, destino, &o, &mut prog, &cancelar)
            }
        };
        eprintln!();
        match r {
            Ok(feitos) => feitos.iter().for_each(|f| println!("{}", f.display())),
            Err(e) => {
                eprintln!("ERRO: {e}");
                ok = false;
            }
        }
    }
    if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}
