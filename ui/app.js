"use strict";

const { invoke } = window.__TAURI__.core;
const dialogo = window.__TAURI__.dialog;

const $ = (s) => document.querySelector(s);
const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
const nomeArquivo = (c) => c.split(/[\\/]/).pop();
const virgula = (n, casas = 1) => n.toFixed(casas).replace(".", ",");

/** @type {{caminho: string, inspecao?: any, erro?: string}[]} */
const fila = [];
let selecionado = null;

async function adicionar(caminhos) {
  const novos = caminhos.filter((c) => /\.wav$/i.test(c) && !fila.some((i) => i.caminho === c));
  if (!novos.length) return;
  const res = await invoke("inspecionar", { caminhos: novos });
  fila.push(...res);
  if (!selecionado) selecionado = res[0].caminho;
  render();
}

function render() {
  $("#contagem").textContent = String(fila.length).padStart(2, "0");
  $("#lista").innerHTML = fila.map((i) => `
    <div class="cartao ${i.caminho === selecionado ? "ativo" : ""}" data-c="${esc(i.caminho)}">
      <div class="n">${esc(nomeArquivo(i.caminho))}</div>
      <div class="m">${i.inspecao ? `${virgula(i.inspecao.sr / 1000)} kHz · ${i.inspecao.n_canais} canal(is)` : esc(i.erro || "")}</div>
    </div>`).join("");
  const item = fila.find((i) => i.caminho === selecionado);
  const ins = item && item.inspecao;
  $("#vazio").classList.toggle("oculto", !!ins);
  $("#detalhe").classList.toggle("oculto", !ins);
  if (ins) {
    $("#dNome").textContent = nomeArquivo(item.caminho);
    $("#dTaxa").textContent = `${virgula(ins.sr / 1000)} kHz`;
    $("#dCanais").textContent = ins.n_canais;
    $("#dDuracao").textContent = `${virgula(ins.duracao_s)} s`;
    $("#dPico").textContent = virgula(ins.pico, 3);
    $("#dAvisos").innerHTML = ins.clip
      ? `<span class="etq clip">${ins.clip} amostras clipadas</span>`
      : `<span class="etq ok">sem clip</span>`;
  }
  $("#acao").disabled = !fila.length;
  $("#resumo").textContent = fila.length ? `${fila.length} ARQUIVO(S)` : "NADA NA FILA";
}

$("#lista").addEventListener("click", (ev) => {
  const c = ev.target.closest(".cartao");
  if (c) { selecionado = c.dataset.c; render(); }
});
async function escolher() {
  const sel = await dialogo.open({ multiple: true, filters: [{ name: "WAV", extensions: ["wav", "WAV"] }] });
  if (sel) await adicionar(Array.isArray(sel) ? sel : [sel]);
}
$("#soltar").addEventListener("click", escolher);
$("#adicionarVazio").addEventListener("click", escolher);
$("#acao").addEventListener("click", () => dialogo.message("Aqui entra o processamento do app.", { title: "Modelo" }));

// arrastar e soltar (eventos nativos do Tauri: dão o caminho completo dos arquivos)
window.__TAURI__.webview.getCurrentWebview().onDragDropEvent((ev) => {
  const t = ev.payload.type;
  document.body.classList.toggle("arrastando", t === "enter" || t === "over");
  if (t === "drop") adicionar(ev.payload.paths);
});

render();
invoke("arquivos_iniciais").then((a) => a.length && adicionar(a));
