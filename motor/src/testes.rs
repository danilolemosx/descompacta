//! Testes do motor. Os que rodam o 7-Zip precisam dele em app/binaries (scripts/baixar-7zip.sh).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use super::*;

/// Pasta temporária própria de cada teste, apagada no fim.
struct Tmp(PathBuf);

impl Tmp {
    fn novo() -> Tmp {
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!("descompacta-teste-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        Tmp(p)
    }
    fn p(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
    fn escrever(&self, rel: &str, conteudo: &[u8]) -> PathBuf {
        let p = self.p(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, conteudo).unwrap();
        p
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn nao() -> AtomicBool {
    AtomicBool::new(false)
}

fn sem_prog() -> impl FnMut(u8) {
    |_| {}
}

/// Bytes pseudoaleatórios (não comprimem: bons para testar partes).
fn ruido(n: usize, semente: u64) -> Vec<u8> {
    let mut x = semente | 1;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect()
}

fn nomes_em(p: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(p).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

fn opcoes(formato: Formato) -> Opcoes {
    Opcoes { formato, um_zip_so: true, ..Default::default() }
}

// ---------- nomes (sem 7-Zip) ----------

#[test]
fn reconhece_compactados() {
    for n in ["a.zip", "a.RAR", "a.7z", "a.tar.gz", "a.tgz", "a.iso", "a.7z.001", "a.z01", "a.r00", "a.part2.rar"] {
        assert!(eh_compactado(Path::new(n)), "{n}");
    }
    for n in ["a.wav", "a.mov", "a.dmg", "Pasta", "a.zip.txt", "a.mxf"] {
        assert!(!eh_compactado(Path::new(n)), "{n}");
    }
}

#[test]
fn nome_da_pasta_de_saida() {
    let casos = [
        ("Pack.zip", "Pack"),
        ("Pack.tar.gz", "Pack"),
        ("Pack.part01.rar", "Pack"),
        ("Pack.7z.001", "Pack"),
        ("foto.jpg.zip", "foto.jpg"),
        ("Ep18 Cena 1.rar", "Ep18 Cena 1"),
        (".zip", ".zip"),
    ];
    for (n, esperado) in casos {
        assert_eq!(nome_base(Path::new(n)), esperado, "{n}");
    }
}

#[test]
fn partes_apontam_para_a_primeira() {
    let t = Tmp::novo();
    for n in ["S.part01.rar", "S.part02.rar", "S.part03.rar", "V.7z.001", "V.7z.002", "Z.zip", "Z.z01", "Z.z02", "R.rar", "R.r00"] {
        t.escrever(n, b"x");
    }
    assert_eq!(primeiro_volume(&t.p("S.part03.rar")), t.p("S.part01.rar"));
    assert_eq!(primeiro_volume(&t.p("V.7z.002")), t.p("V.7z.001"));
    assert_eq!(primeiro_volume(&t.p("Z.z02")), t.p("Z.zip"));
    assert_eq!(primeiro_volume(&t.p("R.r00")), t.p("R.rar"));
    assert_eq!(volumes(&t.p("S.part02.rar")).len(), 3);
    assert_eq!(volumes(&t.p("V.7z.001")).len(), 2);
    assert_eq!(volumes(&t.p("Z.zip")), vec![t.p("Z.zip"), t.p("Z.z01"), t.p("Z.z02")]);
    assert_eq!(volumes(&t.p("R.rar")), vec![t.p("R.rar"), t.p("R.r00")]);
}

#[test]
fn planeja_o_que_foi_solto() {
    let t = Tmp::novo();
    let pasta = t.p("Entrega_Final");
    let a = t.escrever("Entrega_Final/a.wav", b"a");
    let b = t.escrever("Entrega_Final/b.mov", b"b");
    let sub = t.p("Entrega_Final/Sons");
    fs::create_dir_all(&sub).unwrap();
    let z1 = t.escrever("Entrega_Final/S.part01.rar", b"x");
    let z2 = t.escrever("Entrega_Final/S.part02.rar", b"x");

    // um zip só, com o nome da pasta onde estão; as duas partes viram uma tarefa só
    let p = planejar(&[a.clone(), z2.clone(), b.clone(), z1.clone(), sub.clone()], true, Formato::Zip);
    assert_eq!(
        p,
        vec![
            Tarefa::Descompactar { arquivo: z1.clone() },
            Tarefa::Compactar { itens: vec![a.clone(), b.clone(), sub.clone()], destino: pasta.join("Entrega_Final.zip") },
        ]
    );

    // um por item
    let p = planejar(&[a.clone(), sub.clone()], false, Formato::SeteZ);
    assert_eq!(
        p,
        vec![
            Tarefa::Compactar { itens: vec![a.clone()], destino: pasta.join("a.7z") },
            Tarefa::Compactar { itens: vec![sub.clone()], destino: pasta.join("Sons.7z") },
        ]
    );

    // um item só não precisa do nome da pasta
    let p = planejar(&[b.clone()], true, Formato::Zip);
    assert_eq!(p, vec![Tarefa::Compactar { itens: vec![b], destino: pasta.join("b.zip") }]);
}

#[test]
fn nomes_livres() {
    let t = Tmp::novo();
    t.escrever("X.zip", b"");
    t.escrever("X 2.zip", b"");
    t.escrever("Y.tar.gz", b"");
    fs::create_dir_all(t.p("Pasta.v2")).unwrap();
    assert_eq!(nome_livre(&t.p("X.zip")), t.p("X 3.zip"));
    assert_eq!(nome_livre(&t.p("Y.tar.gz")), t.p("Y 2.tar.gz"));
    assert_eq!(nome_livre(&t.p("Pasta.v2")), t.p("Pasta.v2 2"));
    assert_eq!(nome_livre(&t.p("Nova")), t.p("Nova"));
}

#[test]
fn opcoes_vindas_da_interface() {
    let o: Opcoes = serde_json::from_str(r#"{"formato":"7z","nivel":"maximo","senha":"x","partesMb":2048,"umZipSo":true}"#).unwrap();
    assert_eq!((o.formato, o.nivel, o.partes_mb, o.um_zip_so), (Formato::SeteZ, Nivel::Maximo, Some(2048), true));
    let o: Opcoes = serde_json::from_str("{}").unwrap();
    assert_eq!((o.formato, o.nivel, o.senha), (Formato::Zip, Nivel::Normal, None));
}

// ---------- com o 7-Zip ----------

/// Uma pasta com acento, lixo do Mac e um arquivo solto.
fn material(t: &Tmp) -> (PathBuf, PathBuf) {
    t.escrever("orig/Sons/Descrição ação.wav", &ruido(300_000, 1));
    t.escrever("orig/Sons/sub/b.txt", b"oi");
    t.escrever("orig/Sons/.DS_Store", b"lixo");
    t.escrever("orig/Sons/._Descrição ação.wav", b"lixo");
    t.escrever("orig/Sons/__MACOSX/x", b"lixo");
    let solto = t.escrever("orig/leia.txt", b"leia");
    (t.p("orig/Sons"), solto)
}

#[test]
fn zip_ida_e_volta_sem_lixo_e_com_utf8() {
    let t = Tmp::novo();
    let (pasta, _) = material(&t);
    let mut ultimo = 0;
    let criados = compactar(&[pasta.clone()], &t.p("Sons.zip"), &opcoes(Formato::Zip), &mut |p| ultimo = p, &nao()).unwrap();
    assert_eq!(criados, vec![t.p("Sons.zip")]);
    assert_eq!(ultimo, 100);

    // a marca de UTF-8 (bit 11) em todos os cabeçalhos com nome acentuado
    let bytes = fs::read(t.p("Sons.zip")).unwrap();
    let mut achou = 0;
    for i in 0..bytes.len().saturating_sub(30) {
        if &bytes[i..i + 4] == b"PK\x03\x04" {
            let flags = u16::from_le_bytes([bytes[i + 6], bytes[i + 7]]);
            let n = u16::from_le_bytes([bytes[i + 26], bytes[i + 27]]) as usize;
            let nome = &bytes[i + 30..i + 30 + n];
            if !nome.is_ascii() {
                assert!(flags & 0x0800 != 0, "nome acentuado sem marca UTF-8");
                achou += 1;
            }
            assert!(!nome.windows(9).any(|w| w == b".DS_Store"), "levou .DS_Store");
            assert!(!nome.windows(8).any(|w| w == b"__MACOSX"), "levou __MACOSX");
            assert!(!nome.starts_with(b"Sons/._"), "levou ._");
        }
    }
    assert_eq!(achou, 1);

    // um único item dentro (a pasta Sons): sai direto, sem pasta a mais; da segunda vez, "Sons 2"
    let saida = t.p("saida");
    fs::create_dir_all(&saida).unwrap();
    let r = descompactar(&t.p("Sons.zip"), Some(&saida), None, &mut sem_prog(), &nao()).unwrap();
    assert_eq!(r, saida.join("Sons"));
    assert_eq!(nomes_em(&r), vec!["Descrição ação.wav", "sub"]);
    assert_eq!(fs::read(r.join("Descrição ação.wav")).unwrap(), ruido(300_000, 1));
    let r2 = descompactar(&t.p("Sons.zip"), Some(&saida), None, &mut sem_prog(), &nao()).unwrap();
    assert_eq!(r2, saida.join("Sons 2"));
    // nada de pasta de trabalho esquecida
    assert_eq!(nomes_em(&saida), vec!["Sons", "Sons 2"]);
}

#[test]
fn varios_itens_viram_pasta_com_o_nome_do_arquivo() {
    let t = Tmp::novo();
    let (pasta, solto) = material(&t);
    compactar(&[pasta, solto], &t.p("Pacote.7z"), &opcoes(Formato::SeteZ), &mut sem_prog(), &nao()).unwrap();
    let r = descompactar(&t.p("Pacote.7z"), None, None, &mut sem_prog(), &nao()).unwrap();
    assert_eq!(r, t.p("Pacote"));
    assert_eq!(nomes_em(&r), vec!["Sons", "leia.txt"]);
}

#[test]
fn senha() {
    for formato in [Formato::Zip, Formato::SeteZ] {
        let t = Tmp::novo();
        let (pasta, _) = material(&t);
        // o ZIP só aceita senha sem acento; o 7Z aceita qualquer uma
        let senha = if formato == Formato::Zip { "segredo 123" } else { "segredo ç" };
        let o = Opcoes { senha: Some(senha.into()), ..opcoes(formato) };
        let arq = t.p(&format!("S.{}", formato.extensao()));
        if formato == Formato::Zip {
            let acento = Opcoes { senha: Some("ç".into()), ..opcoes(formato) };
            assert!(matches!(compactar(&[pasta.clone()], &arq, &acento, &mut sem_prog(), &nao()), Err(Erro::Falhou(_))));
        }
        compactar(&[pasta], &arq, &o, &mut sem_prog(), &nao()).unwrap();
        assert_eq!(descompactar(&arq, None, None, &mut sem_prog(), &nao()), Err(Erro::SenhaNecessaria), "{formato:?}");
        assert_eq!(descompactar(&arq, None, Some("errada"), &mut sem_prog(), &nao()), Err(Erro::SenhaErrada), "{formato:?}");
        let r = descompactar(&arq, None, Some(senha), &mut sem_prog(), &nao()).unwrap();
        assert_eq!(fs::read(r.join("Descrição ação.wav")).unwrap(), ruido(300_000, 1));
        // as tentativas erradas não deixaram nada para trás
        assert_eq!(nomes_em(&t.0), vec![format!("S.{}", formato.extensao()), "Sons".into(), "orig".into()]);
    }
}

#[test]
fn partes() {
    let t = Tmp::novo();
    let grande = t.escrever("orig/grande.wav", &ruido(2_500_000, 7));
    let o = Opcoes { partes_mb: Some(1), ..opcoes(Formato::SeteZ) };
    let criados = compactar(&[grande], &t.p("G.7z"), &o, &mut sem_prog(), &nao()).unwrap();
    assert_eq!(criados, vec![t.p("G.7z.001"), t.p("G.7z.002"), t.p("G.7z.003")]);
    // soltar a parte do meio funciona
    let tarefas = planejar(&[t.p("G.7z.002")], true, Formato::Zip);
    assert_eq!(tarefas, vec![Tarefa::Descompactar { arquivo: t.p("G.7z.001") }]);
    let saida = t.p("saida");
    fs::create_dir_all(&saida).unwrap();
    let r = descompactar(&t.p("G.7z.001"), Some(&saida), None, &mut sem_prog(), &nao()).unwrap();
    assert_eq!(fs::read(r).unwrap(), ruido(2_500_000, 7));
    // a segunda vez não sobrescreve as partes
    let criados = compactar(&[t.p("orig/grande.wav")], &t.p("G.7z"), &o, &mut sem_prog(), &nao()).unwrap();
    assert_eq!(criados[0], t.p("G 2.7z.001"));
}

#[test]
fn tar_gz_abre_de_uma_vez() {
    let t = Tmp::novo();
    let (pasta, solto) = material(&t);
    // .tar com o 7-Zip e depois .gz
    let mut c = std::process::Command::new(caminho_7zz().unwrap());
    let st = c.arg("a").arg("-ttar").arg("-bso0").arg("-bsp0").arg(t.p("P.tar")).arg(&pasta).arg(&solto).status().unwrap();
    assert!(st.success());
    let st = std::process::Command::new(caminho_7zz().unwrap())
        .arg("a").arg("-tgzip").arg("-bso0").arg("-bsp0").arg(t.p("P.tar.gz")).arg(t.p("P.tar")).status().unwrap();
    assert!(st.success());
    fs::remove_file(t.p("P.tar")).unwrap();
    let mut passos = Vec::new();
    let r = descompactar(&t.p("P.tar.gz"), None, None, &mut |p| passos.push(p), &nao()).unwrap();
    assert_eq!(r, t.p("P"));
    assert_eq!(nomes_em(&r), vec!["Sons", "leia.txt"]);
    assert!(passos.windows(2).all(|w| w[0] <= w[1]), "progresso voltou: {passos:?}");
}

#[test]
fn cancelar_nao_deixa_nada() {
    let t = Tmp::novo();
    let (pasta, _) = material(&t);
    let sim = AtomicBool::new(true);
    assert_eq!(compactar(&[pasta.clone()], &t.p("C.zip"), &opcoes(Formato::Zip), &mut sem_prog(), &sim), Err(Erro::Cancelado));
    compactar(&[pasta], &t.p("C.zip"), &opcoes(Formato::Zip), &mut sem_prog(), &nao()).unwrap();
    assert_eq!(descompactar(&t.p("C.zip"), None, None, &mut sem_prog(), &sim), Err(Erro::Cancelado));
    assert_eq!(nomes_em(&t.0), vec!["C.zip", "orig"]);
}

#[test]
fn arquivo_corrompido() {
    let t = Tmp::novo();
    let falso = t.escrever("falso.zip", b"isto nao e um zip");
    match descompactar(&falso, None, None, &mut sem_prog(), &nao()) {
        Err(Erro::Falhou(m)) => assert!(m.starts_with("o arquivo está corrompido"), "{m}"),
        outro => panic!("{outro:?}"),
    }
    assert_eq!(nomes_em(&t.0), vec!["falso.zip"]);
}
