//! Nomes de arquivo: o que é compactado, partes de um arquivo dividido, nome da pasta de saída e nomes livres.

use std::path::{Path, PathBuf};

/// Extensões que o 7-Zip abre. Ficam de fora as que o próprio sistema trata melhor (.dmg, .pkg, .app).
const EXTENSOES: &[&str] = &[
    "zip", "zipx", "rar", "7z", "tar", "gz", "tgz", "bz2", "tbz", "tbz2", "xz", "txz", "zst", "tzst", "lz", "lzma",
    "z", "taz", "cab", "iso", "arj", "lzh", "lha", "cpio", "xar", "wim", "001", "jar", "apk",
];

/// Compactados que guardam um .tar dentro (abrimos os dois de uma vez).
const TAR_COMPRIMIDO: &[&str] = &[".tar.gz", ".tgz", ".tar.bz2", ".tbz", ".tbz2", ".tar.xz", ".txz", ".tar.zst", ".tzst", ".tar.lz", ".tar.lzma", ".tar.z", ".taz"];

fn nome(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn minusculo(p: &Path) -> String {
    nome(p).to_lowercase()
}

/// Parte numerada: ".002", ".z01", ".r00"… (devolve o número e o prefixo antes do ponto).
fn parte_numerada(n: &str) -> Option<(String, char, u32, usize)> {
    let (antes, ext) = n.rsplit_once('.')?;
    let mut cs = ext.chars();
    let primeira = cs.next()?;
    let resto: String = cs.collect();
    if ext.len() == 3 && ext.chars().all(|c| c.is_ascii_digit()) {
        return Some((antes.to_string(), '#', ext.parse().ok()?, 3));
    }
    if matches!(primeira, 'z' | 'r') && resto.len() == 2 && resto.chars().all(|c| c.is_ascii_digit()) {
        return Some((antes.to_string(), primeira, resto.parse().ok()?, 2));
    }
    None
}

/// "Nome.part3.rar" → ("Nome", 3, 1 dígito)
fn parte_rar(n: &str) -> Option<(String, u32, usize)> {
    let sem = n.strip_suffix(".rar")?;
    let (antes, parte) = sem.rsplit_once(".part")?;
    if parte.is_empty() || !parte.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((antes.to_string(), parte.parse().ok()?, parte.len()))
}

pub fn eh_compactado(p: &Path) -> bool {
    let n = minusculo(p);
    if parte_numerada(&n).is_some() {
        return true;
    }
    n.rsplit_once('.').is_some_and(|(_, ext)| EXTENSOES.contains(&ext))
}

pub(crate) fn eh_tar_comprimido(p: &Path) -> bool {
    let n = minusculo(p);
    TAR_COMPRIMIDO.iter().any(|s| n.ends_with(s))
}

/// Para um arquivo dividido, a parte que o 7-Zip precisa receber (a primeira); para os outros, o próprio arquivo.
/// "X.part2.rar" → "X.part1.rar"; "X.7z.003" → "X.7z.001"; "X.z02" → "X.zip"; "X.r05" → "X.rar".
pub fn primeiro_volume(p: &Path) -> PathBuf {
    let pai = p.parent().unwrap_or(Path::new(""));
    let n = nome(p);
    let l = n.to_lowercase();
    let candidatos: Vec<String> = if let Some((_, k, larg)) = parte_rar(&l) {
        if k <= 1 {
            return p.to_path_buf();
        }
        let base = &n[..n.len() - ".rar".len() - larg - ".part".len()];
        vec![format!("{base}.part{:0larg$}.rar", 1)]
    } else if let Some((_, tipo, k, _)) = parte_numerada(&l) {
        let base = &n[..n.rfind('.').unwrap_or(n.len())];
        match tipo {
            '#' if k > 1 => vec![format!("{base}.001")],
            'z' => vec![format!("{base}.zip"), format!("{base}.ZIP")],
            'r' => vec![format!("{base}.rar"), format!("{base}.RAR")],
            _ => vec![],
        }
    } else {
        vec![]
    };
    candidatos.into_iter().map(|c| pai.join(c)).find(|c| c.exists()).unwrap_or_else(|| p.to_path_buf())
}

/// Todas as partes de um compactado (para mandar todas ao Lixo). Para um arquivo simples, só ele.
pub fn volumes(p: &Path) -> Vec<PathBuf> {
    let primeiro = primeiro_volume(p);
    let pai = primeiro.parent().unwrap_or(Path::new("")).to_path_buf();
    let n = nome(&primeiro);
    let l = n.to_lowercase();
    let mut v = vec![primeiro.clone()];
    if let Some((_, _, larg)) = parte_rar(&l) {
        let base = &n[..n.len() - ".rar".len() - larg - ".part".len()];
        v.extend((2..).map(|k| pai.join(format!("{base}.part{k:0larg$}.rar"))).take_while(|c| c.exists()));
    } else if l.ends_with(".001") {
        let base = &n[..n.len() - 4];
        v.extend((2..).map(|k| pai.join(format!("{base}.{k:03}"))).take_while(|c| c.exists()));
    } else if l.ends_with(".zip") || l.ends_with(".rar") {
        let base = &n[..n.len() - 4];
        let letra = if l.ends_with(".zip") { 'z' } else { 'r' };
        let inicio = if letra == 'z' { 1 } else { 0 };
        v.extend((inicio..).map(|k| pai.join(format!("{base}.{letra}{k:02}"))).take_while(|c| c.exists()));
    }
    v
}

/// Nome da pasta de saída: o nome do arquivo sem as extensões de compactado.
/// "Pack.tar.gz" → "Pack"; "Pack.part1.rar" → "Pack"; "Pack.7z.001" → "Pack".
pub fn nome_base(p: &Path) -> String {
    let mut n = nome(p);
    loop {
        let l = n.to_lowercase();
        if let Some((_, _, larg)) = parte_rar(&l) {
            n.truncate(n.len() - ".rar".len() - larg - ".part".len());
            continue;
        }
        if parte_numerada(&l).is_some() {
            n.truncate(n.rfind('.').unwrap());
            continue;
        }
        match l.rsplit_once('.') {
            Some((antes, ext)) if !antes.is_empty() && (EXTENSOES.contains(&ext)) => n.truncate(antes.len()),
            _ => break,
        }
    }
    if n.is_empty() { "Descompactado".into() } else { n }
}

/// "Nome.zip" + ".001" → "Nome.zip.001"
pub(crate) fn anexar(p: &Path, sufixo: &str) -> PathBuf {
    let mut s = p.as_os_str().to_os_string();
    s.push(sufixo);
    PathBuf::from(s)
}

/// "Pasta/Nome.zip", 2 → "Pasta/Nome 2.zip"; pastas: "Pasta/Nome", 2 → "Pasta/Nome 2".
pub(crate) fn numerado(p: &Path, n: u32) -> PathBuf {
    let pai = p.parent().unwrap_or(Path::new(""));
    let nm = nome(p);
    // ".tar.gz" e afins contam como uma extensão só
    let l = nm.to_lowercase();
    let corte = TAR_COMPRIMIDO
        .iter()
        .find(|s| l.ends_with(*s) && l.len() > s.len())
        .map(|s| nm.len() - s.len())
        .or_else(|| nm.rfind('.').filter(|&i| i > 0));
    match corte {
        Some(i) if !p.is_dir() => pai.join(format!("{} {n}{}", &nm[..i], &nm[i..])),
        _ => pai.join(format!("{nm} {n}")),
    }
}

/// O próprio caminho, se estiver livre; senão "Nome 2", "Nome 3"…
pub fn nome_livre(p: &Path) -> PathBuf {
    if !p.exists() {
        return p.to_path_buf();
    }
    (2..).map(|n| numerado(p, n)).find(|c| !c.exists()).unwrap()
}
