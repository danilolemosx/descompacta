//! Backend Tauri: comandos chamados pela interface (ui/app.js). A lógica fica no crate `motor`.

use std::path::PathBuf;

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Item {
    caminho: String,
    inspecao: Option<motor::Inspecao>,
    erro: Option<String>,
}

/// Arquivos passados na linha de comando (por exemplo, "Abrir com…").
#[tauri::command]
fn arquivos_iniciais() -> Vec<String> {
    std::env::args().skip(1).filter(|a| a.to_lowercase().ends_with(".wav")).collect()
}

/// Inspeciona os arquivos (em outra thread, para não travar a janela).
#[tauri::command]
async fn inspecionar(caminhos: Vec<String>) -> Vec<Item> {
    tauri::async_runtime::spawn_blocking(move || {
        caminhos
            .into_iter()
            .map(|c| match motor::inspecionar(&PathBuf::from(&c)) {
                Ok(i) => Item { caminho: c, inspecao: Some(i), erro: None },
                Err(e) => Item { caminho: c, inspecao: None, erro: Some(format!("{e:#}")) },
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![arquivos_iniciais, inspecionar])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o aplicativo");
}
