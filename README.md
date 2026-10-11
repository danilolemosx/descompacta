# Modelo de app desktop (Mac e Windows)

Ponto de partida para os apps do Danilo, com o que funcionou no **Decanta**:

- **Tauri v2**: um app leve para Mac (Apple Silicon e Intel) e Windows, com a lógica em Rust.
- **`motor/`**: a lógica do app, testável sozinha, com uma linha de comando (`motor-cli`).
- **`app/`**: a janela, que liga a interface ao motor.
- **`ui/`**: a interface em HTML/CSS/JS, no estilo "bilhete" (papel, tinta e vermelho, fonte Archivo).
- **GitHub Actions**: testes a cada envio e instaladores ao publicar uma Release.
- **Sem assinatura paga**: Mac com assinatura ad-hoc e Windows com instalador comum. Os avisos de segurança e
  como contorná-los estão em `docs/LEIA-ME-modelo.txt`.

O exemplo que vem pronto lê WAVs arrastados para a janela e mostra taxa, canais, duração, pico e clip.

## Começar um app novo

1. No GitHub, clique em **Use this template → Create a new repository**.
2. Abra uma sessão do Claude Code no repositório novo e descreva o app. O `CLAUDE.md` explica o jeito de
   trabalhar, e `docs/processo.md` traz o passo a passo e as lições do Decanta.

## Desenvolvimento

```bash
cargo test -p motor                                   # testes do motor
cargo run --release -p motor --bin motor-cli -- a.wav # motor pela linha de comando
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
- Este modelo: MIT OR Apache-2.0.
- Fonte Archivo (Omnibus-Type): SIL Open Font License 1.1, veja `ui/fonts/OFL.txt`.
