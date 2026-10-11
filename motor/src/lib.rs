//! Motor do app. Toda a lógica (ler, processar, gravar arquivos) fica aqui, sem nada de interface,
//! para poder ser testada sozinha (`cargo test -p motor`) e usada pela linha de comando (`motor-cli`).
//!
//! Exemplo: inspeção de um WAV (taxa, canais, duração, pico e amostras clipadas).
//! Troque pelo processamento do seu app.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

/// Resultado da inspeção de um WAV.
#[derive(Debug, Clone, Serialize)]
pub struct Inspecao {
    pub sr: u32,
    pub n_canais: u16,
    pub bits: u16,
    pub duracao_s: f64,
    /// Pico absoluto (0..1).
    pub pico: f32,
    /// Amostras com |x| ≥ 0,999.
    pub clip: usize,
}

/// Lê um WAV e devolve as amostras normalizadas (-1..1), intercaladas.
pub fn ler_wav(caminho: &Path) -> Result<(hound::WavSpec, Vec<f32>)> {
    let mut r = hound::WavReader::open(caminho).with_context(|| format!("não foi possível abrir {}", caminho.display()))?;
    let spec = r.spec();
    let amostras = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().collect::<Result<Vec<_>, _>>()?,
        hound::SampleFormat::Int => {
            let escala = (1i64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>().map(|s| s.map(|v| v as f32 / escala)).collect::<Result<Vec<_>, _>>()?
        }
    };
    Ok((spec, amostras))
}

/// Inspeciona amostras já lidas.
pub fn inspecionar_amostras(spec: &hound::WavSpec, amostras: &[f32]) -> Inspecao {
    let pico = amostras.iter().fold(0f32, |m, &x| m.max(x.abs()));
    let clip = amostras.iter().filter(|x| x.abs() >= 0.999).count();
    let quadros = amostras.len() / spec.channels.max(1) as usize;
    Inspecao {
        sr: spec.sample_rate,
        n_canais: spec.channels,
        bits: spec.bits_per_sample,
        duracao_s: quadros as f64 / spec.sample_rate as f64,
        pico,
        clip,
    }
}

/// Lê e inspeciona um WAV.
pub fn inspecionar(caminho: &Path) -> Result<Inspecao> {
    let (spec, amostras) = ler_wav(caminho)?;
    Ok(inspecionar_amostras(&spec, &amostras))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn pico_e_clip() {
        let spec = hound::WavSpec { channels: 2, sample_rate: 48_000, bits_per_sample: 24, sample_format: hound::SampleFormat::Int };
        let mut x = vec![0.1f32; 96_000];
        x[10] = -1.0;
        x[11] = 0.5;
        let i = inspecionar_amostras(&spec, &x);
        assert_eq!(i.pico, 1.0);
        assert_eq!(i.clip, 1);
        assert_eq!(i.duracao_s, 1.0);
    }

    #[test]
    fn grava_e_le() {
        let caminho = std::env::temp_dir().join(format!("motor-teste-{}.wav", std::process::id()));
        let spec = hound::WavSpec { channels: 1, sample_rate: 44_100, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut w = hound::WavWriter::create(&caminho, spec).unwrap();
        for k in 0..44_100 {
            w.write_sample(((k % 100) as i16 - 50) * 100).unwrap();
        }
        w.finalize().unwrap();
        let i = inspecionar(&caminho).unwrap();
        let _ = std::fs::remove_file(&caminho);
        assert_eq!((i.sr, i.n_canais, i.bits), (44_100, 1, 16));
        assert_eq!(i.duracao_s, 1.0);
        assert_eq!(i.clip, 0);
    }
}
