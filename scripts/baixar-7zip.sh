#!/usr/bin/env bash
# Baixa o 7-Zip oficial (versão fixa, conferida por SHA-256) para app/binaries/,
# onde o app o embute como programa externo (sidecar).
#
#   scripts/baixar-7zip.sh [alvo]
#
# alvo: aarch64-apple-darwin | x86_64-apple-darwin | x86_64-pc-windows-msvc | x86_64-unknown-linux-gnu
# (sem alvo: o do computador atual, segundo `rustc -vV`).
set -euo pipefail

VERSAO=26.03
V=2603
sha() { # funciona no bash 3 do Mac (sem arrays associativos)
  case "$1" in
    mac) echo 5ca87677072c59f5602e5c49baa27d4694bacd2259b4e507f0094249d4281480 ;;
    linux-x64) echo dc99eff5008f1ab79bd7084c68513701547a808a89502bf4133683535ab3c695 ;;
    x64) echo 0859c524b8a63551848f0c246abddcb1d0b7b656b0fbfe879f8d85e61a9e6edd ;;
  esac
}

alvo="${1:-$(rustc -vV | tr -d '\r' | sed -n 's/^host: //p')}"
raiz="$(cd "$(dirname "$0")/.." && pwd)"
destino="$raiz/app/binaries"
mkdir -p "$destino"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

baixar() { # $1 = sufixo do arquivo (mac, linux-x64, x64), $2 = extensão
  local arq="7z${V}-$1.$2"
  curl -fsSL -o "$tmp/$arq" "https://github.com/ip7z/7zip/releases/download/${VERSAO}/$arq"
  local soma
  soma="$( (sha256sum "$tmp/$arq" 2>/dev/null || shasum -a 256 "$tmp/$arq") | cut -d' ' -f1)"
  if [ "$soma" != "$(sha "$1")" ]; then
    echo "SHA-256 não confere para $arq: $soma" >&2
    exit 1
  fi
  echo "$tmp/$arq"
}

case "$alvo" in
  *-apple-darwin)
    # binário universal (Apple Silicon e Intel)
    arq="$(baixar mac tar.xz)"
    tar -xJf "$arq" -C "$tmp" 7zz License.txt
    install -m 755 "$tmp/7zz" "$destino/7zz-$alvo"
    ;;
  x86_64-unknown-linux-gnu)
    arq="$(baixar linux-x64 tar.xz)"
    tar -xJf "$arq" -C "$tmp" 7zz License.txt
    install -m 755 "$tmp/7zz" "$destino/7zz-$alvo"
    ;;
  x86_64-pc-windows-msvc)
    # o instalador é um arquivo 7z: tiramos só o 7z.exe, o 7z.dll (formatos, inclusive RAR) e a licença.
    # Precisa de um 7z já instalado (os runners Windows do GitHub têm).
    arq="$(baixar x64 exe)"
    7z e -y -o"$tmp" "$arq" 7z.exe 7z.dll License.txt > /dev/null
    cp "$tmp/7z.exe" "$destino/7zz-$alvo.exe"
    cp "$tmp/7z.dll" "$destino/7z.dll"
    ;;
  *)
    echo "alvo não suportado: $alvo" >&2
    exit 1
    ;;
esac
cp "$tmp/License.txt" "$destino/7zip-License.txt"
echo "7-Zip $VERSAO pronto em $destino para $alvo"
