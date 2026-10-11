//! Motor do Descompacta. Toda a lógica fica aqui, sem nada de interface, para poder ser testada sozinha
//! (`cargo test -p motor`) e usada pela linha de comando (`motor-cli`).
//!
//! O trabalho pesado é do 7-Zip oficial (`7zz`, ou `7z.exe` no Windows), embutido no app como programa
//! externo. O motor decide o que fazer com cada arquivo, monta o comando, lê o progresso e arruma o resultado:
//! - um compactado vira uma pasta (ou o único item de dentro, sem pasta a mais), sem o lixo do Mac;
//! - arquivos e pastas comuns viram um zip (ou 7z), sem `.DS_Store`/`__MACOSX` e com nomes em UTF-8.

mod nomes;
mod sete;

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

pub use nomes::{eh_compactado, nome_base, nome_livre, primeiro_volume, volumes};
pub use sete::caminho_7zz;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Formato {
    #[default]
    Zip,
    #[serde(rename = "7z")]
    SeteZ,
}

impl Formato {
    pub fn extensao(self) -> &'static str {
        match self {
            Formato::Zip => "zip",
            Formato::SeteZ => "7z",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Nivel {
    Rapido,
    #[default]
    Normal,
    Maximo,
}

/// Opções de compactação (as da engrenagem).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Opcoes {
    pub formato: Formato,
    pub nivel: Nivel,
    /// Vazio ou ausente = sem senha.
    pub senha: Option<String>,
    /// Tamanho de cada parte em MB (ausente = não dividir).
    pub partes_mb: Option<u64>,
    /// Vários arquivos soltos viram um zip só (padrão: sim).
    pub um_zip_so: bool,
}

/// O que fazer com o que foi solto na janela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "camelCase")]
pub enum Tarefa {
    Descompactar { arquivo: PathBuf },
    /// `destino` é o nome sugerido; na hora de compactar, se já existir, vira "Nome 2.zip".
    Compactar { itens: Vec<PathBuf>, destino: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "tipo", content = "detalhe", rename_all = "camelCase")]
pub enum Erro {
    /// O arquivo tem senha e nenhuma foi dada.
    SenhaNecessaria,
    SenhaErrada,
    Cancelado,
    Falhou(String),
}

impl std::fmt::Display for Erro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Erro::SenhaNecessaria => write!(f, "o arquivo tem senha"),
            Erro::SenhaErrada => write!(f, "senha errada"),
            Erro::Cancelado => write!(f, "cancelado"),
            Erro::Falhou(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for Erro {}

fn falhou(contexto: &str, e: impl std::fmt::Display) -> Erro {
    Erro::Falhou(format!("{contexto}: {e}"))
}

/// Decide o que fazer com os caminhos soltos na janela (ou vindos do Finder):
/// compactados são abertos (um por vez, partes 2, 3… juntam-se à primeira), o resto é compactado,
/// num arquivo só (`um_zip_so`, com o nome da pasta onde estão) ou um por item.
pub fn planejar(caminhos: &[PathBuf], um_zip_so: bool, formato: Formato) -> Vec<Tarefa> {
    let mut tarefas = Vec::new();
    let mut comuns: Vec<PathBuf> = Vec::new();
    for c in caminhos {
        if c.is_file() && eh_compactado(c) {
            let p = primeiro_volume(c);
            let t = Tarefa::Descompactar { arquivo: p };
            if !tarefas.contains(&t) {
                tarefas.push(t);
            }
        } else if c.exists() && !comuns.contains(c) {
            comuns.push(c.clone());
        }
    }
    if comuns.is_empty() {
        return tarefas;
    }
    if um_zip_so && comuns.len() > 1 {
        let pai = comuns[0].parent().map(Path::to_path_buf).unwrap_or_default();
        let nome = pai.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Arquivos".into());
        let destino = pai.join(format!("{nome}.{}", formato.extensao()));
        tarefas.push(Tarefa::Compactar { itens: comuns, destino });
    } else {
        for c in comuns {
            let pai = c.parent().map(Path::to_path_buf).unwrap_or_default();
            let nome = if c.is_dir() {
                c.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
            } else {
                c.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
            };
            let destino = pai.join(format!("{nome}.{}", formato.extensao()));
            tarefas.push(Tarefa::Compactar { itens: vec![c], destino });
        }
    }
    tarefas
}

/// Abre um compactado. O resultado vai para `pasta` (ou a pasta do próprio arquivo):
/// se dentro houver um único item, ele sai direto; senão, tudo fica numa pasta com o nome do arquivo.
/// Nunca sobrescreve: se o nome existir, vira "Nome 2". Devolve o caminho criado.
pub fn descompactar(
    arquivo: &Path,
    pasta: Option<&Path>,
    senha: Option<&str>,
    progresso: &mut dyn FnMut(u8),
    cancelar: &AtomicBool,
) -> Result<PathBuf, Erro> {
    let pai = match pasta {
        Some(p) => p.to_path_buf(),
        None => arquivo.parent().map(Path::to_path_buf).unwrap_or_default(),
    };
    let tmp = nome_livre(&pai.join(format!(".descompacta-{}", std::process::id())));
    std::fs::create_dir_all(&tmp).map_err(|e| falhou("não foi possível criar a pasta de trabalho", e))?;
    let r = descompactar_em(arquivo, &tmp, senha, progresso, cancelar).and_then(|()| arrumar(&tmp, &pai, arquivo));
    let _ = std::fs::remove_dir_all(&tmp);
    r
}

fn descompactar_em(
    arquivo: &Path,
    tmp: &Path,
    senha: Option<&str>,
    progresso: &mut dyn FnMut(u8),
    cancelar: &AtomicBool,
) -> Result<(), Erro> {
    // .tar.gz e companhia: o 7-Zip tira primeiro o .tar e depois o conteúdo dele
    let duplo = nomes::eh_tar_comprimido(arquivo);
    let mut prog1 = |p: u8| progresso(if duplo { p / 2 } else { p });
    sete::extrair(arquivo, tmp, senha, &mut prog1, cancelar)?;
    if duplo {
        let itens = listar(tmp)?;
        if let [unico] = itens.as_slice() {
            if unico.extension().is_some_and(|e| e.eq_ignore_ascii_case("tar")) {
                let mut prog2 = |p: u8| progresso(50 + p / 2);
                sete::extrair(unico, tmp, None, &mut prog2, cancelar)?;
                std::fs::remove_file(unico).map_err(|e| falhou("não foi possível apagar o .tar intermediário", e))?;
            }
        }
    }
    progresso(100);
    Ok(())
}

/// Tira o lixo do Mac e move o resultado da pasta de trabalho para o lugar final.
fn arrumar(tmp: &Path, pai: &Path, arquivo: &Path) -> Result<PathBuf, Erro> {
    for lixo in ["__MACOSX", ".DS_Store"] {
        let p = tmp.join(lixo);
        if p.is_dir() {
            let _ = std::fs::remove_dir_all(&p);
        } else if p.exists() {
            let _ = std::fs::remove_file(&p);
        }
    }
    let itens = listar(tmp)?;
    let destino = match itens.as_slice() {
        [] => return Err(Erro::Falhou("o arquivo está vazio".into())),
        [unico] => {
            let d = nome_livre(&pai.join(unico.file_name().unwrap_or_default()));
            std::fs::rename(unico, &d).map_err(|e| falhou("não foi possível mover o resultado", e))?;
            d
        }
        _ => {
            let d = nome_livre(&pai.join(nome_base(arquivo)));
            std::fs::create_dir(&d).map_err(|e| falhou("não foi possível criar a pasta", e))?;
            for i in &itens {
                std::fs::rename(i, d.join(i.file_name().unwrap_or_default()))
                    .map_err(|e| falhou("não foi possível mover o resultado", e))?;
            }
            d
        }
    };
    Ok(destino)
}

fn listar(pasta: &Path) -> Result<Vec<PathBuf>, Erro> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(pasta)
        .map_err(|e| falhou("não foi possível ler a pasta de trabalho", e))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    v.sort();
    Ok(v)
}

/// Compacta `itens` em `destino` (que ganha " 2", " 3"… se já existir). Devolve os arquivos criados
/// (mais de um quando dividido em partes).
pub fn compactar(
    itens: &[PathBuf],
    destino: &Path,
    opcoes: &Opcoes,
    progresso: &mut dyn FnMut(u8),
    cancelar: &AtomicBool,
) -> Result<Vec<PathBuf>, Erro> {
    if itens.is_empty() {
        return Err(Erro::Falhou("nada para compactar".into()));
    }
    let destino = if opcoes.partes_mb.is_some() {
        // as partes são "Nome.zip.001", "Nome.zip.002"…: o nome livre tem de valer para a primeira
        let mut d = destino.to_path_buf();
        let mut n = 2;
        while nomes::anexar(&d, ".001").exists() || d.exists() {
            d = nomes::numerado(destino, n);
            n += 1;
        }
        d
    } else {
        nome_livre(destino)
    };
    let r = sete::compactar(itens, &destino, opcoes, progresso, cancelar);
    let criados = criados(&destino);
    match r {
        Ok(()) if !criados.is_empty() => {
            progresso(100);
            Ok(criados)
        }
        Ok(()) => Err(Erro::Falhou("o 7-Zip não criou o arquivo".into())),
        Err(e) => {
            for c in criados {
                let _ = std::fs::remove_file(c);
            }
            Err(e)
        }
    }
}

/// O arquivo criado, ou as partes "destino.001", "destino.002"…
fn criados(destino: &Path) -> Vec<PathBuf> {
    if destino.exists() {
        return vec![destino.to_path_buf()];
    }
    (1..)
        .map(|k| nomes::anexar(destino, &format!(".{k:03}")))
        .take_while(|p| p.exists())
        .collect()
}

#[cfg(test)]
mod testes;
