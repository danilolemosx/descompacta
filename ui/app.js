"use strict";

// ---------- estado (tudo declarado antes de usar) ----------
const T = window.__TAURI__;
const { invoke, Channel } = T.core;

const $ = (s) => document.querySelector(s);
const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
const nomeDe = (c) => String(c).split(/[\\/]/).pop();
const pastaDe = (c) => String(c).slice(0, String(c).length - nomeDe(c).length);
const ehMac = /Mac/.test(navigator.userAgent);

const PADRAO = { onde: "mesma", abrirPasta: true, lixo: false, umZipSo: true, formato: "zip", nivel: "normal", partes: "0" };
const cfg = carregarCfg();
let senhaCompactar = ""; // só na memória: vale até fechar o app

/** @type {any[]} */
const itens = [];
let proximoId = 1;
let rodando = false;
let ultimoResultado = null;
let revelarNoFim = false;
let lote = [];
let loteTimer = null;

const CADEADO = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6"><rect x="5" y="11" width="14" height="10" rx="1.5"/><path d="M8 11V7.5a4 4 0 0 1 8 0V11"/></svg>';

function carregarCfg() {
  try {
    return { ...PADRAO, ...JSON.parse(localStorage.getItem("cfg") || "{}") };
  } catch {
    return { ...PADRAO };
  }
}
function salvarCfg() {
  try { localStorage.setItem("cfg", JSON.stringify(cfg)); } catch {}
}

// ---------- números ----------
function tamanho(b) {
  if (b == null) return "";
  const u = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  while (b >= 1000 && i < u.length - 1) { b /= 1000; i++; }
  const casas = i >= 3 && b < 100 ? 1 : 0;
  return `${b.toFixed(casas).replace(".", ",")} ${u[i]}`;
}
function falta(item) {
  if (!item.inicio || item.pct < 3) return "";
  const s = ((Date.now() - item.inicio) / 1000) * (100 - item.pct) / item.pct;
  if (s < 1) return "";
  if (s < 60) return ` · faltam ${Math.ceil(s)} s`;
  return ` · faltam ${Math.ceil(s / 60)} min`;
}

// ---------- chegada de arquivos ----------
// Junta o que chega quase junto (o Windows manda um arquivo por vez no botão direito)
function chegar(caminhos) {
  if (!caminhos || !caminhos.length) return;
  lote.push(...caminhos);
  clearTimeout(loteTimer);
  loteTimer = setTimeout(() => {
    const l = [...new Set(lote)];
    lote = [];
    processarLote(l);
  }, 350);
}

async function processarLote(caminhos) {
  let pasta = null;
  if (cfg.onde === "perguntar") {
    pasta = await T.dialog.open({ directory: true, title: "Onde salvar?" });
    if (!pasta) return;
  }
  const tarefas = await invoke("planejar", { caminhos, umZipSo: cfg.umZipSo, formato: cfg.formato });
  for (const tarefa of tarefas) {
    const item = { id: proximoId++, tarefa, pasta, estado: "fila", pct: 0, bytes: null, resultado: [], senha: null, erro: "" };
    itens.push(item);
    const fontes = tarefa.tipo === "descompactar" ? [tarefa.arquivo] : tarefa.itens;
    invoke("contar", { caminhos: fontes }).then((c) => { item.bytes = c.bytes; item.arquivosFonte = c.arquivos; atualizar(item); resumo(); });
  }
  render();
  avancar();
}

function nomeItem(item) {
  if (item.estado === "pronto" && item.resultado.length) return nomeDe(item.resultado[0]);
  return item.tarefa.tipo === "descompactar" ? nomeDe(item.tarefa.arquivo) : nomeDe(item.tarefa.destino);
}

// ---------- execução ----------
async function avancar() {
  if (rodando) return;
  const item = itens.find((i) => i.estado === "fila");
  if (!item) {
    if (revelarNoFim && cfg.abrirPasta && ultimoResultado) revelar(ultimoResultado);
    revelarNoFim = false;
    resumo();
    return;
  }
  rodando = true;
  item.estado = "rodando";
  item.pct = 0;
  item.inicio = Date.now();
  render();
  const canal = new Channel();
  canal.onmessage = (p) => { item.pct = p; atualizar(item); };
  const opcoes = {
    formato: cfg.formato, nivel: cfg.nivel, senha: senhaCompactar || null,
    partesMb: Number(cfg.partes) || null, umZipSo: cfg.umZipSo,
  };
  try {
    item.resultado = await invoke("executar", { id: item.id, tarefa: item.tarefa, opcoes, senha: item.senha, pasta: item.pasta, progresso: canal });
    item.estado = "pronto";
    item.pct = 100;
    ultimoResultado = item.resultado[0];
    revelarNoFim = true;
    if (item.tarefa.tipo === "descompactar") {
      if (cfg.lixo) {
        try { await invoke("para_lixeira", { arquivo: item.tarefa.arquivo }); item.lixo = true; }
        catch (e) { item.avisoLixo = String(e); }
      }
    }
    invoke("contar", { caminhos: item.resultado }).then((c) => { item.final = c; atualizar(item); });
  } catch (e) {
    const tipo = e && e.tipo ? e.tipo : "falhou";
    if (tipo === "senhaNecessaria" || tipo === "senhaErrada") {
      item.estado = "senha";
      item.erro = tipo === "senhaErrada" ? "Senha errada · tente de novo" : "Este arquivo tem senha";
    } else if (tipo === "cancelado") {
      item.estado = "cancelado";
    } else {
      item.estado = "erro";
      item.erro = String((e && e.detalhe) || e).split("\n")[0];
    }
  }
  rodando = false;
  render();
  avancar();
}

function revelar(caminho) {
  T.opener.revealItemInDir(caminho).catch(() => {});
}

// ---------- desenho ----------
function detalhe(item) {
  const t = item.tarefa;
  const tam = item.bytes != null ? ` · ${tamanho(item.bytes)}` : "";
  const fmt = (t.tipo === "compactar" ? (t.destino.toLowerCase().endsWith(".7z") ? "7Z" : "ZIP") : "");
  switch (item.estado) {
    case "fila":
      return t.tipo === "descompactar" ? `Na fila${tam}` : `Na fila · vai virar ${fmt}${t.itens.length > 1 ? ` · ${t.itens.length} itens` : ""}${tam}`;
    case "rodando":
      return `${t.tipo === "descompactar" ? "Descompactando" : "Compactando"}${falta(item)}`;
    case "pronto": {
      const f = item.final;
      let s = "Pronto";
      if (f) s += t.tipo === "descompactar" ? ` · ${f.arquivos} arquivo${f.arquivos === 1 ? "" : "s"} · ${tamanho(f.bytes)}` : ` · ${tamanho(f.bytes)}`;
      if (item.resultado.length > 1) s += ` · ${item.resultado.length} partes`;
      if (item.lixo) s += " · original no Lixo";
      if (item.avisoLixo) s += " · não foi para o Lixo";
      return s;
    }
    case "senha":
      return item.erro;
    case "cancelado":
      return "Cancelado";
    default:
      return item.erro || "Erro";
  }
}

function canhoto(item) {
  switch (item.estado) {
    case "fila": return '<span class="v1">—</span><span class="x2">✕</span>';
    case "rodando": return `<span class="v1">${item.pct}</span><span class="x2">✕</span>`;
    case "pronto": return "✓";
    case "senha": return CADEADO;
    case "cancelado": return "✕";
    default: return "!";
  }
}

function podeRenomear(item) {
  if (item.tarefa.tipo !== "compactar") return false;
  return item.estado === "fila" || (item.estado === "pronto" && item.resultado.length === 1);
}

function cartaoHtml(item) {
  const cancelavel = item.estado === "fila" || item.estado === "rodando";
  const titulo = item.estado === "fila" ? "Tirar da fila" : item.estado === "rodando" ? "Cancelar" : item.estado === "pronto" ? "Mostrar no Finder" : "Tirar da lista";
  const pct = item.estado === "pronto" ? 100 : item.estado === "rodando" ? item.pct : 0;
  return `
    <div class="cartao ${item.estado}" data-id="${item.id}">
      <div class="esq">
        <div class="nome ${podeRenomear(item) ? "editavel" : ""}" title="${podeRenomear(item) ? "Clique para renomear" : esc(item.tarefa.arquivo || item.tarefa.destino)}">${esc(nomeItem(item))}</div>
        <div class="det W ${item.estado === "erro" ? "multi" : ""}">${esc(detalhe(item))}</div>
        ${item.estado === "senha" ? `
          <div class="pedeSenha">
            <input type="password" placeholder="Senha do arquivo" autocomplete="off">
            <button class="btn abrirSenha W">Abrir</button>
            <button class="btn claro pularSenha W">Pular</button>
          </div>` : `<div class="trilho"><div style="width:${pct}%"></div></div>`}
      </div>
      <button class="dir ${cancelavel ? "cancelavel" : ""}" title="${titulo}">${canhoto(item)}</button>
    </div>`;
}

function render() {
  const foco = document.activeElement && document.activeElement.closest && document.activeElement.closest(".pedeSenha");
  $("#lista").innerHTML = itens.map(cartaoHtml).join("");
  if (!foco) {
    const campo = $("#lista .pedeSenha input");
    if (campo) campo.focus();
  }
  resumo();
}

function atualizar(item) {
  const c = $(`.cartao[data-id="${item.id}"]`);
  if (!c) return;
  const det = c.querySelector(".det");
  if (det) det.textContent = detalhe(item);
  const barra = c.querySelector(".trilho div");
  if (barra) barra.style.width = `${item.estado === "pronto" ? 100 : item.pct}%`;
  const v1 = c.querySelector(".dir .v1");
  if (v1 && item.estado === "rodando") v1.textContent = item.pct;
}

function resumo() {
  const total = itens.length;
  const feitos = itens.filter((i) => i.estado === "pronto").length;
  const bytes = itens.reduce((s, i) => s + (i.bytes || 0), 0);
  $("#resumo").textContent = total ? `${feitos} de ${total}${bytes ? ` · ${tamanho(bytes)}` : ""}` : "Nada na fila";
  $("#abrirPasta").disabled = !ultimoResultado;
  $("#limpar").hidden = !itens.some((i) => ["pronto", "erro", "cancelado"].includes(i.estado));
  const f = cfg.formato === "7z" ? "7Z" : "ZIP";
  $("#fmtDica").textContent = f;
  $("#fmtDica2").textContent = f;
}

// ---------- ações na fila ----------
$("#lista").addEventListener("click", async (ev) => {
  const c = ev.target.closest(".cartao");
  if (!c) return;
  const item = itens.find((i) => i.id === Number(c.dataset.id));
  if (!item) return;

  if (ev.target.closest(".dir")) {
    if (item.estado === "rodando") invoke("cancelar", { id: item.id });
    else if (item.estado === "pronto") revelar(item.resultado[0]);
    else { itens.splice(itens.indexOf(item), 1); render(); }
    return;
  }
  if (ev.target.closest(".abrirSenha")) {
    const s = c.querySelector(".pedeSenha input").value;
    if (!s) return;
    item.senha = s;
    item.estado = "fila";
    render();
    avancar();
    return;
  }
  if (ev.target.closest(".pularSenha")) {
    itens.splice(itens.indexOf(item), 1);
    render();
    return;
  }
  const nome = ev.target.closest(".nome.editavel");
  if (nome && !nome.querySelector("input")) editarNome(item, nome);
});

$("#lista").addEventListener("keydown", (ev) => {
  if (ev.key === "Enter" && ev.target.closest(".pedeSenha")) ev.target.closest(".cartao").querySelector(".abrirSenha").click();
});

function editarNome(item, el) {
  const atual = nomeItem(item);
  el.innerHTML = `<input type="text" value="${esc(atual)}">`;
  const campo = el.querySelector("input");
  campo.focus();
  const ponto = atual.lastIndexOf(".");
  campo.setSelectionRange(0, ponto > 0 ? ponto : atual.length);
  let feito = false;
  const concluir = async (salvar) => {
    if (feito) return;
    feito = true;
    let novo = campo.value.trim();
    if (salvar && novo && novo !== atual) {
      const ext = atual.slice(atual.lastIndexOf("."));
      if (!novo.toLowerCase().endsWith(ext.toLowerCase())) novo += ext;
      if (item.estado === "fila") {
        item.tarefa.destino = pastaDe(item.tarefa.destino) + novo;
      } else if (item.estado === "pronto") {
        try {
          item.resultado[0] = await invoke("renomear", { caminho: item.resultado[0], nome: novo });
          ultimoResultado = item.resultado[0];
        } catch (e) {
          T.dialog.message(String(e), { title: "Não deu para renomear", kind: "warning" });
        }
      }
    }
    render();
  };
  campo.addEventListener("keydown", (e) => {
    if (e.key === "Enter") concluir(true);
    if (e.key === "Escape") concluir(false);
  });
  campo.addEventListener("blur", () => concluir(true));
}

$("#solte").addEventListener("click", async () => {
  const escolhidos = await T.dialog.open({ multiple: true, title: "Escolha arquivos para abrir ou compactar" });
  if (escolhidos) chegar(Array.isArray(escolhidos) ? escolhidos : [escolhidos]);
});
$("#abrirPasta").addEventListener("click", () => ultimoResultado && revelar(ultimoResultado));
$("#limpar").addEventListener("click", () => {
  for (let k = itens.length - 1; k >= 0; k--) if (["pronto", "erro", "cancelado"].includes(itens[k].estado)) itens.splice(k, 1);
  render();
});

// ---------- arrastar e soltar ----------
T.webview.getCurrentWebview().onDragDropEvent((ev) => {
  const t = ev.payload.type;
  const zona = $("#solte");
  if (t === "enter" || t === "over") zona.classList.add("sobre");
  else zona.classList.remove("sobre");
  if (t === "drop") {
    fecharAjustes();
    chegar(ev.payload.paths);
  }
});
T.event.listen("abrir", (ev) => { fecharAjustes(); chegar(ev.payload); });

// ---------- ajustes ----------
function abrirAjustes() { $("#telaFila").hidden = true; $("#telaAjustes").hidden = false; desenharAjustes(); }
function fecharAjustes() { $("#telaAjustes").hidden = true; $("#telaFila").hidden = false; resumo(); }
$("#abrirAjustes").addEventListener("click", abrirAjustes);
$("#fecharAjustes").addEventListener("click", fecharAjustes);
document.addEventListener("keydown", (e) => { if (e.key === "Escape" && !$("#telaAjustes").hidden) fecharAjustes(); });

function desenharAjustes() {
  document.querySelectorAll(".seg[data-cfg]").forEach((seg) => {
    seg.querySelectorAll("button").forEach((b) => b.classList.toggle("on", String(cfg[seg.dataset.cfg]) === b.dataset.v));
  });
  document.querySelectorAll(".tg[data-cfg]").forEach((b) => b.classList.toggle("on", !!cfg[b.dataset.cfg]));
  validarSenha();
}

document.querySelectorAll(".seg[data-cfg]").forEach((seg) => seg.addEventListener("click", (e) => {
  const b = e.target.closest("button");
  if (!b) return;
  cfg[seg.dataset.cfg] = b.dataset.v;
  salvarCfg();
  desenharAjustes();
}));
document.querySelectorAll(".tg[data-cfg]").forEach((b) => b.addEventListener("click", () => {
  cfg[b.dataset.cfg] = !cfg[b.dataset.cfg];
  salvarCfg();
  desenharAjustes();
}));

$("#senhaCompactar").addEventListener("input", (e) => { senhaCompactar = e.target.value; validarSenha(); });
function validarSenha() {
  // o ZIP não aceita senha com acento ou ç
  const ruim = cfg.formato === "zip" && /[^\x00-\x7f]/.test(senhaCompactar);
  $("#senhaCompactar").classList.toggle("ruim", ruim);
  $("#senhaDica").textContent = ruim ? "No ZIP, sem acento nem ç (ou use 7Z)" : "Vazio = sem senha · vale até fechar o app";
}

// botão direito no Finder (Mac)
let integracao = { disponivel: false, acoesRapidas: false, padrao: false };
function desenharIntegracao() {
  $("#blocoMac").hidden = !integracao.disponivel;
  $("#tgAcoes").classList.toggle("on", integracao.acoesRapidas);
  $("#tgPadrao").classList.toggle("on", integracao.padrao);
}
async function definirIntegracao(acoesRapidas, padrao) {
  try {
    integracao = await invoke("integracao_definir", { acoesRapidas, padrao });
  } catch (e) {
    T.dialog.message(String(e), { title: "Não deu para mudar o botão direito", kind: "warning" });
  }
  desenharIntegracao();
}
$("#tgAcoes").addEventListener("click", () => definirIntegracao(!integracao.acoesRapidas, integracao.padrao));
$("#tgPadrao").addEventListener("click", () => definirIntegracao(integracao.acoesRapidas, !integracao.padrao));

// ---------- início ----------
(async () => {
  render();
  try { $("#versao").textContent = "v" + (await T.app.getVersion()); } catch {}
  try {
    integracao = await invoke("integracao_estado");
    // na primeira vez, liga o botão direito e o clique duplo (como no preview aprovado)
    let primeira = false;
    try { primeira = !localStorage.getItem("integracaoFeita"); localStorage.setItem("integracaoFeita", "1"); } catch {}
    if (primeira && integracao.disponivel && !(integracao.acoesRapidas && integracao.padrao)) await definirIntegracao(true, true);
  } catch {}
  desenharIntegracao();
  chegar(await invoke("arquivos_iniciais"));
})();
