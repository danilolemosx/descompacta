//! Conversa com o 7-Zip: acha o programa, monta o comando, lê o progresso e traduz os erros.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use crate::{Erro, Formato, Nivel, Opcoes};

const NOME: &str = if cfg!(windows) { "7zz.exe" } else { "7zz" };

/// Onde está o 7-Zip: `DESCOMPACTA_7ZZ`, ao lado do app (instalado), em `app/binaries/` (desenvolvimento)
/// ou no PATH.
pub fn caminho_7zz() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("DESCOMPACTA_7ZZ") {
        return Some(PathBuf::from(p));
    }
    if let Some(pasta) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        let p = pasta.join(NOME);
        if p.is_file() {
            return Some(p);
        }
    }
    let binarios = Path::new(env!("CARGO_MANIFEST_DIR")).join("../app/binaries");
    if let Ok(ls) = std::fs::read_dir(&binarios) {
        let mut achados: Vec<PathBuf> = ls
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                let n = p.file_name().unwrap_or_default().to_string_lossy();
                n.starts_with("7zz-") && (cfg!(windows) == n.ends_with(".exe"))
            })
            .collect();
        achados.sort();
        if let Some(p) = achados.into_iter().next() {
            return Some(p);
        }
    }
    ["7zz", "7z"].iter().find_map(|n| {
        std::env::var_os("PATH").and_then(|caminhos| {
            std::env::split_paths(&caminhos)
                .map(|d| d.join(if cfg!(windows) { format!("{n}.exe") } else { n.to_string() }))
                .find(|p| p.is_file())
        })
    })
}

fn comando() -> Result<Command, Erro> {
    let p = caminho_7zz().ok_or_else(|| Erro::Falhou("7-Zip não encontrado (rode scripts/baixar-7zip.sh)".into()))?;
    let mut c = Command::new(p);
    c.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    Ok(c)
}

pub(crate) fn extrair(
    arquivo: &Path,
    destino: &Path,
    senha: Option<&str>,
    progresso: &mut dyn FnMut(u8),
    cancelar: &AtomicBool,
) -> Result<(), Erro> {
    let mut c = comando()?;
    // -p sempre presente: sem senha, o 7-Zip responde "Wrong password" em vez de esperar o teclado
    c.arg("x").arg("-y").arg("-bso0").arg("-bsp1").arg("-bse2");
    c.arg(format!("-p{}", senha.unwrap_or("")));
    c.arg(format!("-o{}", destino.display()));
    c.arg("--").arg(arquivo);
    rodar(c, progresso, cancelar).map_err(|e| match e {
        Erro::Falhou(m) if m.contains("Wrong password") || m.contains("encrypted") => {
            if senha.is_some_and(|s| !s.is_empty()) { Erro::SenhaErrada } else { Erro::SenhaNecessaria }
        }
        outro => outro,
    })
}

pub(crate) fn compactar(
    itens: &[PathBuf],
    destino: &Path,
    o: &Opcoes,
    progresso: &mut dyn FnMut(u8),
    cancelar: &AtomicBool,
) -> Result<(), Erro> {
    if o.formato == Formato::Zip && o.senha.as_deref().is_some_and(|s| !s.is_ascii()) {
        return Err(Erro::Falhou("no ZIP, a senha não pode ter acento nem ç (tire os acentos ou use 7Z)".into()));
    }
    let mut c = comando()?;
    c.arg("a").arg("-bso0").arg("-bsp1").arg("-bse2").arg("-ssw");
    c.arg(match o.formato {
        Formato::Zip => "-tzip",
        Formato::SeteZ => "-t7z",
    });
    c.arg(match o.nivel {
        Nivel::Rapido => "-mx1",
        Nivel::Normal => "-mx5",
        Nivel::Maximo => "-mx9",
    });
    if o.formato == Formato::Zip {
        // nomes em UTF-8 marcados no zip: acentos chegam certos no Windows
        c.arg("-mcu=on");
    }
    if let Some(s) = o.senha.as_deref().filter(|s| !s.is_empty()) {
        c.arg(format!("-p{s}"));
        c.arg(if o.formato == Formato::Zip { "-mem=AES256" } else { "-mhe=on" });
    }
    if let Some(mb) = o.partes_mb.filter(|&mb| mb > 0) {
        c.arg(format!("-v{mb}m"));
    }
    // lixo do Mac (e do Windows) não entra
    for x in ["-xr!.DS_Store", "-xr!__MACOSX", "-xr!._*", "-xr!Thumbs.db", "-xr!desktop.ini"] {
        c.arg(x);
    }
    c.arg("--").arg(destino);
    // caminhos completos: o 7-Zip guarda só o último nome ("Pasta/…", "arquivo.wav")
    for i in itens {
        c.arg(std::path::absolute(i).unwrap_or_else(|_| i.clone()));
    }
    rodar(c, progresso, cancelar)
}

/// Roda o 7-Zip, repassando a porcentagem e matando o processo se `cancelar` virar verdadeiro.
fn rodar(mut c: Command, progresso: &mut dyn FnMut(u8), cancelar: &AtomicBool) -> Result<(), Erro> {
    if cancelar.load(Ordering::Relaxed) {
        return Err(Erro::Cancelado);
    }
    let mut filho = c.spawn().map_err(|e| Erro::Falhou(format!("não foi possível rodar o 7-Zip: {e}")))?;
    let mut saida = filho.stdout.take().unwrap();
    let mut erros = filho.stderr.take().unwrap();
    let (tx, rx) = mpsc::channel::<u8>();
    let leitor = std::thread::spawn(move || {
        // o 7-Zip escreve " 45% 12 - nome" e volta com \b: pegamos os dígitos antes de cada '%'
        let mut buf = [0u8; 4096];
        let mut num: u32 = 0;
        let mut tem = false;
        while let Ok(n) = saida.read(&mut buf) {
            if n == 0 {
                break;
            }
            for &b in &buf[..n] {
                match b {
                    b'0'..=b'9' => {
                        num = (num * 10 + (b - b'0') as u32).min(1000);
                        tem = true;
                    }
                    b'%' if tem => {
                        let _ = tx.send(num.min(100) as u8);
                        tem = false;
                        num = 0;
                    }
                    _ => {
                        tem = false;
                        num = 0;
                    }
                }
            }
        }
    });
    let leitor_erros = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = erros.read_to_string(&mut s);
        s
    });
    let mut ultimo = 0u8;
    let status = loop {
        while let Ok(p) = rx.try_recv() {
            if p != ultimo {
                ultimo = p;
                progresso(p);
            }
        }
        if cancelar.load(Ordering::Relaxed) {
            let _ = filho.kill();
            let _ = filho.wait();
            let _ = leitor.join();
            return Err(Erro::Cancelado);
        }
        match filho.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(Erro::Falhou(format!("erro esperando o 7-Zip: {e}"))),
        }
    };
    let _ = leitor.join();
    let msg = leitor_erros.join().unwrap_or_default();
    // 0 = tudo certo; 1 = só avisos (por exemplo, dados sobrando no fim do arquivo)
    match status.code() {
        Some(0) | Some(1) => Ok(()),
        Some(255) => Err(Erro::Cancelado),
        _ => {
            let msg = msg.trim();
            let msg = if msg.is_empty() { format!("o 7-Zip parou com o código {:?}", status.code()) } else { traduzir(msg) };
            Err(Erro::Falhou(msg))
        }
    }
}

/// Os erros mais comuns em português; o resto segue como o 7-Zip escreveu.
fn traduzir(msg: &str) -> String {
    // "Wrong password" segue em inglês: extrair() o transforma em SenhaNecessaria/SenhaErrada
    let tabela = [
        ("open the file as", "o arquivo está corrompido ou não é um compactado"),
        ("Is not archive", "o arquivo está corrompido ou não é um compactado"),
        ("Unexpected end of archive", "o arquivo está incompleto (download pela metade?)"),
        ("Missing volume", "falta uma das partes do arquivo"),
        ("not enough space", "o disco está cheio"),
        ("No space left", "o disco está cheio"),
        ("Data Error", "o arquivo está corrompido"),
        ("CRC Failed", "o arquivo está corrompido"),
    ];
    match tabela.iter().find(|(chave, _)| msg.contains(chave)) {
        Some((_, pt)) if !msg.contains("Wrong password") => format!("{pt}\n\n{msg}"),
        _ => msg.to_string(),
    }
}
