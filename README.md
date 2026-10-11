# Descompacta

Descompactador e compactador gratuito para Mac (Apple Silicon e Intel) e Windows, da suíte do Danilo.

- **Arraste e solte**: compactados (zip, rar, 7z, tar.gz, iso…) são abertos; arquivos e pastas comuns viram um zip.
  Vários arquivos soltos juntos viram **um zip só**, com o nome da pasta onde estão.
- **Fila de bilhetes**: cada arquivo com a sua porcentagem; senha pedida quando o arquivo tiver uma.
- **Sem bagunça**: um arquivo com um único item dentro não cria pasta a mais; nada é sobrescrito ("Nome 2").
- **Zip limpo**: sem `.DS_Store`/`__MACOSX` e com nomes em UTF-8 (acentos chegam certos no Windows).
- **Botão direito**: "Abrir com" e Ações Rápidas no Finder; "Descompactar aqui"/"Compactar em ZIP" no Windows.

Por baixo, o trabalho pesado é do **7-Zip** oficial (versão fixa em `scripts/baixar-7zip.sh`, conferida por
SHA-256), embutido no app. Os previews aprovados da interface estão em `previews/`.

## Desenvolvimento

```bash
scripts/baixar-7zip.sh                                # baixa o 7-Zip para app/binaries (uma vez)
cargo test -p motor                                   # testes do motor
cargo run --release -p motor --bin motor-cli -- X.rar # o mesmo que soltar na janela
cargo tauri dev                                       # (em app/) abre o app
```

No Linux, o app precisa de `libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev`.

## Publicar uma versão

1. Atualize `"version"` em `app/tauri.conf.json`.
2. Confira que os **Testes** passaram na aba Actions.
3. Em **Releases → Draft a new release**, crie a tag `vX.Y.Z` (o alvo é o branch com o código), dê o título e
   clique em **Publish release**.
4. Em 20 a 30 minutos, os instaladores aparecem no fim da página da Release:
   `..._aarch64.dmg` (Mac M1–M4), `..._x64.dmg` (Mac Intel) e `..._x64-setup.exe` (Windows).

## Licenças
- Descompacta: MIT OR Apache-2.0.
- 7-Zip (Igor Pavlov): GNU LGPL, com a restrição do unRAR (o código pode abrir RAR, mas não pode ser usado para
  recriar o compactador RAR). A licença vai junto no app (`7zip-License.txt`). Site: https://www.7-zip.org
- Fonte Archivo (Omnibus-Type): SIL Open Font License 1.1, veja `ui/fonts/OFL.txt`.
