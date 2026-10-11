//! Backend Tauri: comandos chamados pela interface (ui/app.js). A lógica fica no crate `motor`.

mod integracao;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{Emitter, Manager, RunEvent, State};

#[derive(Default)]
struct Estado {
    /// Um sinal de cancelar por tarefa em andamento.
    cancelar: Mutex<HashMap<u32, Arc<AtomicBool>>>,
    /// Arquivos que chegaram (Finder, outra instância) antes de a interface estar pronta.
    pendentes: Mutex<(bool, Vec<String>)>,
}

/// Entrega arquivos à interface; se ela ainda não carregou, guarda para `arquivos_iniciais`.
fn entregar(app: &tauri::AppHandle, caminhos: Vec<String>) {
    if caminhos.is_empty() {
        return;
    }
    let estado = app.state::<Estado>();
    let mut p = estado.pendentes.lock().unwrap();
    if p.0 {
        let _ = app.emit("abrir", caminhos);
        if let Some(j) = app.get_webview_window("main") {
            let _ = j.unminimize();
            let _ = j.set_focus();
        }
    } else {
        p.1.extend(caminhos);
    }
}

fn caminhos_dos_argumentos(args: impl Iterator<Item = String>) -> Vec<String> {
    args.filter(|a| !a.starts_with('-') && Path::new(a).exists()).collect()
}

/// Arquivos com que o app foi aberto (linha de comando no Windows, "Abrir com" no Mac).
#[tauri::command]
fn arquivos_iniciais(estado: State<'_, Estado>) -> Vec<String> {
    let mut p = estado.pendentes.lock().unwrap();
    p.0 = true;
    let mut v = caminhos_dos_argumentos(std::env::args().skip(1));
    v.append(&mut p.1);
    v
}

/// O que fazer com o que foi solto na janela.
#[tauri::command]
fn planejar(caminhos: Vec<String>, um_zip_so: bool, formato: motor::Formato) -> Vec<motor::Tarefa> {
    let caminhos: Vec<PathBuf> = caminhos.into_iter().map(PathBuf::from).collect();
    motor::planejar(&caminhos, um_zip_so, formato)
}

#[derive(Serialize)]
struct Contagem {
    arquivos: u64,
    bytes: u64,
}

#[tauri::command]
async fn contar(caminhos: Vec<String>) -> Contagem {
    tauri::async_runtime::spawn_blocking(move || {
        let mut c = Contagem { arquivos: 0, bytes: 0 };
        for p in caminhos {
            let (a, b) = motor::contar(Path::new(&p));
            c.arquivos += a;
            c.bytes += b;
        }
        c
    })
    .await
    .unwrap_or(Contagem { arquivos: 0, bytes: 0 })
}

/// Roda uma tarefa (descompactar ou compactar), mandando a porcentagem pelo canal.
/// Devolve os caminhos criados.
#[tauri::command]
async fn executar(
    estado: State<'_, Estado>,
    id: u32,
    tarefa: motor::Tarefa,
    opcoes: motor::Opcoes,
    senha: Option<String>,
    pasta: Option<String>,
    progresso: Channel<u8>,
) -> Result<Vec<String>, motor::Erro> {
    let sinal = Arc::new(AtomicBool::new(false));
    estado.cancelar.lock().unwrap().insert(id, sinal.clone());
    let r = tauri::async_runtime::spawn_blocking(move || {
        let mut prog = |p: u8| {
            let _ = progresso.send(p);
        };
        let pasta = pasta.map(PathBuf::from);
        match tarefa {
            motor::Tarefa::Descompactar { arquivo } => {
                motor::descompactar(&arquivo, pasta.as_deref(), senha.as_deref(), &mut prog, &sinal).map(|p| vec![p])
            }
            motor::Tarefa::Compactar { itens, destino } => {
                let destino = match (&pasta, destino.file_name()) {
                    (Some(p), Some(n)) => p.join(n),
                    _ => destino,
                };
                motor::compactar(&itens, &destino, &opcoes, &mut prog, &sinal)
            }
        }
    })
    .await
    .unwrap_or_else(|e| Err(motor::Erro::Falhou(format!("erro interno: {e}"))));
    estado.cancelar.lock().unwrap().remove(&id);
    r.map(|v| v.into_iter().map(|p| p.to_string_lossy().into_owned()).collect())
}

#[tauri::command]
fn cancelar(estado: State<'_, Estado>, id: u32) {
    if let Some(s) = estado.cancelar.lock().unwrap().get(&id) {
        s.store(true, Ordering::Relaxed);
    }
}

/// Manda o compactado (com todas as partes) para o Lixo.
#[tauri::command]
fn para_lixeira(arquivo: String) -> Result<(), String> {
    trash::delete_all(motor::volumes(Path::new(&arquivo))).map_err(|e| e.to_string())
}

/// Renomeia um resultado na mesma pasta. Devolve o caminho novo.
#[tauri::command]
fn renomear(caminho: String, nome: String) -> Result<String, String> {
    let nome = nome.trim();
    if nome.is_empty() || nome.contains(['/', '\\', ':']) {
        return Err("nome inválido".into());
    }
    let de = PathBuf::from(&caminho);
    let para = de.with_file_name(nome);
    if para == de {
        return Ok(caminho);
    }
    if para.exists() {
        return Err("já existe um arquivo com esse nome".into());
    }
    std::fs::rename(&de, &para).map_err(|e| e.to_string())?;
    Ok(para.to_string_lossy().into_owned())
}

#[tauri::command]
fn integracao_estado() -> integracao::Situacao {
    integracao::situacao()
}

#[tauri::command]
async fn integracao_definir(acoes_rapidas: bool, padrao: bool) -> Result<integracao::Situacao, String> {
    tauri::async_runtime::spawn_blocking(move || {
        integracao::definir(acoes_rapidas, padrao)?;
        Ok(integracao::situacao())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        // Windows: o menu do botão direito abre uma instância por arquivo; elas entregam tudo à primeira
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            entregar(app, caminhos_dos_argumentos(args.into_iter().skip(1)));
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Estado::default())
        .invoke_handler(tauri::generate_handler![
            arquivos_iniciais,
            planejar,
            contar,
            executar,
            cancelar,
            para_lixeira,
            renomear,
            integracao_estado,
            integracao_definir
        ])
        .build(tauri::generate_context!())
        .expect("erro ao iniciar o aplicativo");
    app.run(|app, evento| {
        // Mac: "Abrir com", clique duplo e Ações Rápidas chegam por aqui
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        if let RunEvent::Opened { urls } = &evento {
            let caminhos = urls
                .iter()
                .filter_map(|u| u.to_file_path().ok())
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            entregar(app, caminhos);
        }
        let _ = (app, &evento);
    });
}
