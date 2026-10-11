//! Botão direito no Finder (Mac): Ações Rápidas e app padrão para compactados.
//! No Windows o menu do botão direito é instalado pelo instalador (windows/hooks.nsh).

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Situacao {
    /// Só no Mac: no Windows a interface esconde estes ajustes.
    pub disponivel: bool,
    pub acoes_rapidas: bool,
    pub padrao: bool,
}

#[cfg(not(target_os = "macos"))]
pub fn situacao() -> Situacao {
    Situacao { disponivel: false, acoes_rapidas: false, padrao: false }
}

#[cfg(not(target_os = "macos"))]
pub fn definir(_acoes_rapidas: bool, _padrao: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
pub use mac::{definir, situacao};

#[cfg(target_os = "macos")]
mod mac {
    use std::path::PathBuf;

    use core_foundation::base::TCFType;
    use core_foundation::string::{CFString, CFStringRef};
    use core_foundation::url::{CFURL, CFURLRef};

    use super::Situacao;

    const ID: &str = "com.danilolemos.descompacta";
    const UTILITARIO: &str = "com.apple.archiveutility";

    /// Tipos que passam a abrir no Descompacta com clique duplo (.iso fica com o Mac, que o monta como disco).
    const TIPOS_PADRAO: &[&str] = &[
        "public.zip-archive",
        "com.rarlab.rar-archive",
        "org.7-zip.7-zip-archive",
        "public.tar-archive",
        "org.gnu.gnu-zip-archive",
        "org.gnu.gnu-zip-tar-archive",
        "public.bzip2-archive",
        "org.tukaani.xz-archive",
    ];

    /// Para mostrar "Descompactar aqui" só em compactados.
    const TIPOS_COMPACTADOS: &[&str] = &[
        "public.archive",
        "public.zip-archive",
        "com.rarlab.rar-archive",
        "org.7-zip.7-zip-archive",
        "public.tar-archive",
        "org.gnu.gnu-zip-archive",
        "org.gnu.gnu-zip-tar-archive",
        "public.bzip2-archive",
        "org.tukaani.xz-archive",
        "com.microsoft.cab",
    ];

    #[link(name = "CoreServices", kind = "framework")]
    extern "C" {
        fn LSSetDefaultRoleHandlerForContentType(tipo: CFStringRef, papel: u32, app: CFStringRef) -> i32;
        fn LSCopyDefaultRoleHandlerForContentType(tipo: CFStringRef, papel: u32) -> CFStringRef;
        fn LSRegisterURL(app: CFURLRef, atualizar: u8) -> i32;
    }

    /// Garante que o Mac já conhece o app (logo depois de instalado ele pode ainda não ter registrado).
    fn registrar_app() {
        let Some(app) = std::env::current_exe().ok().and_then(|e| e.ancestors().find(|p| p.extension().is_some_and(|x| x == "app")).map(|p| p.to_path_buf())) else {
            return;
        };
        if let Some(url) = CFURL::from_path(&app, true) {
            unsafe { LSRegisterURL(url.as_concrete_TypeRef(), 1) };
        }
    }
    const TODOS_OS_PAPEIS: u32 = 0xFFFF_FFFF;
    const VISUALIZADOR: u32 = 0x0000_0002;

    fn app_padrao(tipo: &str) -> Option<String> {
        let t = CFString::new(tipo);
        let r = unsafe { LSCopyDefaultRoleHandlerForContentType(t.as_concrete_TypeRef(), VISUALIZADOR) };
        if r.is_null() {
            return None;
        }
        Some(unsafe { CFString::wrap_under_create_rule(r) }.to_string())
    }

    fn mudar_padrao(tipo: &str, app: &str) {
        let t = CFString::new(tipo);
        let a = CFString::new(app);
        unsafe { LSSetDefaultRoleHandlerForContentType(t.as_concrete_TypeRef(), TODOS_OS_PAPEIS, a.as_concrete_TypeRef()) };
    }

    fn pasta_servicos() -> Option<PathBuf> {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Services"))
    }

    /// (nome do pacote, item do menu, tipos aceitos)
    fn acoes() -> [(&'static str, &'static str, Vec<&'static str>); 2] {
        [
            ("Descompacta - Descompactar aqui.workflow", "Descompactar aqui", TIPOS_COMPACTADOS.to_vec()),
            ("Descompacta - Compactar em ZIP.workflow", "Compactar em ZIP", vec!["public.item"]),
        ]
    }

    pub fn situacao() -> Situacao {
        let acoes_rapidas = pasta_servicos().is_some_and(|s| acoes().iter().all(|(n, _, _)| s.join(n).exists()));
        let padrao = app_padrao("public.zip-archive").is_some_and(|a| a.eq_ignore_ascii_case(ID));
        Situacao { disponivel: true, acoes_rapidas, padrao }
    }

    pub fn definir(acoes_rapidas: bool, padrao: bool) -> Result<(), String> {
        let servicos = pasta_servicos().ok_or("pasta pessoal não encontrada")?;
        for (pacote, item, tipos) in acoes() {
            let p = servicos.join(pacote);
            if acoes_rapidas {
                let c = p.join("Contents");
                std::fs::create_dir_all(&c).map_err(|e| e.to_string())?;
                std::fs::write(c.join("Info.plist"), info_plist(item, &tipos)).map_err(|e| e.to_string())?;
                std::fs::write(c.join("document.wflow"), DOCUMENTO.replace("{COMANDO}", &format!("open -b {ID} \"$@\"")))
                    .map_err(|e| e.to_string())?;
            } else if p.exists() {
                std::fs::remove_dir_all(&p).map_err(|e| e.to_string())?;
            }
        }
        // o Finder relê a lista de Ações Rápidas
        let _ = std::process::Command::new("/System/Library/CoreServices/pbs").arg("-update").status();

        if padrao {
            registrar_app();
        }
        // às vezes o Mac ignora a primeira tentativa (app recém-instalado): conferimos e repetimos
        for tentativa in 0..3 {
            let mut faltou = false;
            for tipo in TIPOS_PADRAO {
                let atual = app_padrao(tipo).unwrap_or_default();
                let somos = atual.eq_ignore_ascii_case(ID);
                if padrao && !somos {
                    mudar_padrao(tipo, ID);
                    faltou |= !app_padrao(tipo).is_some_and(|a| a.eq_ignore_ascii_case(ID));
                } else if !padrao && somos {
                    mudar_padrao(tipo, UTILITARIO);
                }
            }
            if !faltou {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(300 * (tentativa + 1)));
        }
        Ok(())
    }

    fn info_plist(item: &str, tipos: &[&str]) -> String {
        let tipos: String = tipos.iter().map(|t| format!("\t\t\t\t<string>{t}</string>\n")).collect();
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>NSServices</key>
	<array>
		<dict>
			<key>NSBackgroundColorName</key>
			<string>background</string>
			<key>NSIconName</key>
			<string>NSActionTemplate</string>
			<key>NSMenuItem</key>
			<dict>
				<key>default</key>
				<string>{item}</string>
			</dict>
			<key>NSMessage</key>
			<string>runWorkflowAsService</string>
			<key>NSRequiredContext</key>
			<dict>
				<key>NSApplicationIdentifier</key>
				<string>com.apple.finder</string>
			</dict>
			<key>NSSendFileTypes</key>
			<array>
{tipos}			</array>
		</dict>
	</array>
</dict>
</plist>
"#
        )
    }

    /// Ação Rápida do Automator: "Executar Script de Shell" com os arquivos como argumentos.
    const DOCUMENTO: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>AMApplicationBuild</key>
	<string>534</string>
	<key>AMApplicationVersion</key>
	<string>2.10</string>
	<key>AMDocumentVersion</key>
	<string>2</string>
	<key>actions</key>
	<array>
		<dict>
			<key>action</key>
			<dict>
				<key>AMAccepts</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Optional</key>
					<true/>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.string</string>
					</array>
				</dict>
				<key>AMActionVersion</key>
				<string>2.0.3</string>
				<key>AMApplication</key>
				<array>
					<string>Automator</string>
				</array>
				<key>AMParameterProperties</key>
				<dict>
					<key>COMMAND_STRING</key>
					<dict/>
					<key>CheckedForUserDefaultShell</key>
					<dict/>
					<key>inputMethod</key>
					<dict/>
					<key>shell</key>
					<dict/>
					<key>source</key>
					<dict/>
				</dict>
				<key>AMProvides</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.string</string>
					</array>
				</dict>
				<key>ActionBundlePath</key>
				<string>/System/Library/Automator/Run Shell Script.action</string>
				<key>ActionName</key>
				<string>Run Shell Script</string>
				<key>ActionParameters</key>
				<dict>
					<key>COMMAND_STRING</key>
					<string>{COMANDO}</string>
					<key>CheckedForUserDefaultShell</key>
					<true/>
					<key>inputMethod</key>
					<integer>1</integer>
					<key>shell</key>
					<string>/bin/zsh</string>
					<key>source</key>
					<string></string>
				</dict>
				<key>BundleIdentifier</key>
				<string>com.apple.RunShellScript</string>
				<key>CFBundleVersion</key>
				<string>2.0.3</string>
				<key>CanShowSelectedItemsWhenRun</key>
				<false/>
				<key>CanShowWhenRun</key>
				<true/>
				<key>Category</key>
				<array>
					<string>AMCategoryUtilities</string>
				</array>
				<key>Class Name</key>
				<string>RunShellScriptAction</string>
				<key>InputUUID</key>
				<string>5B1D3E9A-7C2F-4A61-9E0B-3F8D2C6A1B47</string>
				<key>Keywords</key>
				<array>
					<string>Shell</string>
					<string>Script</string>
				</array>
				<key>OutputUUID</key>
				<string>8E4A2C71-0D3B-4F5E-A69C-1B7D9E2F3A58</string>
				<key>UUID</key>
				<string>C3F7A1D2-6B8E-4C09-B5A4-9D2E1F0C7B36</string>
				<key>UnlocalizedApplications</key>
				<array>
					<string>Automator</string>
				</array>
				<key>arguments</key>
				<dict>
					<key>0</key>
					<dict>
						<key>default value</key>
						<integer>0</integer>
						<key>name</key>
						<string>inputMethod</string>
						<key>required</key>
						<string>0</string>
						<key>type</key>
						<string>0</string>
						<key>uuid</key>
						<string>0</string>
					</dict>
					<key>1</key>
					<dict>
						<key>default value</key>
						<false/>
						<key>name</key>
						<string>CheckedForUserDefaultShell</string>
						<key>required</key>
						<string>0</string>
						<key>type</key>
						<string>0</string>
						<key>uuid</key>
						<string>1</string>
					</dict>
					<key>2</key>
					<dict>
						<key>default value</key>
						<string></string>
						<key>name</key>
						<string>source</string>
						<key>required</key>
						<string>0</string>
						<key>type</key>
						<string>0</string>
						<key>uuid</key>
						<string>2</string>
					</dict>
					<key>3</key>
					<dict>
						<key>default value</key>
						<string></string>
						<key>name</key>
						<string>COMMAND_STRING</string>
						<key>required</key>
						<string>0</string>
						<key>type</key>
						<string>0</string>
						<key>uuid</key>
						<string>3</string>
					</dict>
					<key>4</key>
					<dict>
						<key>default value</key>
						<string>/bin/sh</string>
						<key>name</key>
						<string>shell</string>
						<key>required</key>
						<string>0</string>
						<key>type</key>
						<string>0</string>
						<key>uuid</key>
						<string>4</string>
					</dict>
				</dict>
				<key>conversionLabel</key>
				<integer>0</integer>
				<key>isViewVisible</key>
				<integer>1</integer>
				<key>location</key>
				<string>309.000000:305.000000</string>
				<key>nibPath</key>
				<string>/System/Library/Automator/Run Shell Script.action/Contents/Resources/Base.lproj/main.nib</string>
			</dict>
			<key>isViewVisible</key>
			<integer>1</integer>
		</dict>
	</array>
	<key>connectors</key>
	<dict/>
	<key>workflowMetaData</key>
	<dict>
		<key>applicationBundleID</key>
		<string>com.apple.finder</string>
		<key>applicationBundleIDsByPath</key>
		<dict>
			<key>/System/Library/CoreServices/Finder.app</key>
			<string>com.apple.finder</string>
		</dict>
		<key>applicationPath</key>
		<string>/System/Library/CoreServices/Finder.app</string>
		<key>applicationPaths</key>
		<array>
			<string>/System/Library/CoreServices/Finder.app</string>
		</array>
		<key>inputTypeIdentifier</key>
		<string>com.apple.Automator.fileSystemObject</string>
		<key>outputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>presentationMode</key>
		<integer>15</integer>
		<key>processesInput</key>
		<false/>
		<key>serviceApplicationBundleID</key>
		<string>com.apple.finder</string>
		<key>serviceApplicationPath</key>
		<string>/System/Library/CoreServices/Finder.app</string>
		<key>serviceInputTypeIdentifier</key>
		<string>com.apple.Automator.fileSystemObject</string>
		<key>serviceOutputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>serviceProcessesInput</key>
		<false/>
		<key>systemImageName</key>
		<string>NSActionTemplate</string>
		<key>useAutomaticInputType</key>
		<false/>
		<key>workflowTypeIdentifier</key>
		<string>com.apple.Automator.servicesMenu</string>
	</dict>
</dict>
</plist>
"#;
}
