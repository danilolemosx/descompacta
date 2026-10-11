//! Backend Tauri: comandos chamados pela interface (ui/app.js). A lógica fica no crate `motor`.

use std::path::PathBuf;

/// Arquivos passados na linha de comando (por exemplo, o menu do botão direito no Windows).
#[tauri::command]
fn arquivos_iniciais() -> Vec<String> {
    std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect()
}

/// O que fazer com o que foi solto na janela.
#[tauri::command]
fn planejar(caminhos: Vec<String>, um_zip_so: bool, formato: motor::Formato) -> Vec<motor::Tarefa> {
    let caminhos: Vec<PathBuf> = caminhos.into_iter().map(PathBuf::from).collect();
    motor::planejar(&caminhos, um_zip_so, formato)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![arquivos_iniciais, planejar])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o aplicativo");
}
