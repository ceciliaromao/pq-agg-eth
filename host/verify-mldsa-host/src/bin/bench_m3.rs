//! Harness de benchmark do M3: mesma lógica do M2, mas para um lote de k
//! assinaturas ML-DSA-44 por prova, com k > 1. Reusa `bench_case` e
//! `valid_batch` de lib.rs, que suportam lote através do mesmo guest
//! program parametrizado usado pelo M1 (k=1).
//!
//! Uso: cargo run --release -p verify-mldsa-host --bin bench_m3 -- <k> <n>
//! (padrão: k=2, n=5 se omitidos). N é menor que os 30 usados no M2 de
//! propósito, já que o custo de proving cresce com k. Ajustar N por
//! invocação conforme o orçamento de tempo disponível para cada valor de k.
//!
//! Resultados em docs/results/m3_benchmark_k<k>.csv, acrescentados a cada
//! execução em vez de sobrescritos. Para k a partir de certo tamanho, a
//! memória de um processo longo não é totalmente liberada entre
//! repetições, então a metodologia recomendada é rodar N=1 várias vezes em
//! processos separados, em vez de um N alto num único processo.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use sp1_sdk::blocking::EnvProver;
use verify_mldsa_host::{bench_case, setup, valid_batch, BenchSample};

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn stddev(values: &[f64], mean: f64) -> f64 {
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    variance.sqrt()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let k: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(2);
    let n: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(5);

    let client = EnvProver::new();
    let proving_key = setup(&client);

    // Lote fixo, gerado uma única vez e reusado em todas as repetições,
    // mesma justificativa metodológica do M2 (isolar ruído de sistema).
    let batch = valid_batch("m3-bench", k);

    let results_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/results");
    std::fs::create_dir_all(&results_dir).expect("não foi possível criar docs/results");
    let csv_path = results_dir.join(format!("m3_benchmark_k{k}.csv"));

    // Repetições já registradas neste arquivo (de invocações anteriores),
    // para continuar a numeração em vez de reiniciar em 1 a cada execução.
    let existing_reps = std::fs::read_to_string(&csv_path).map(|s| s.lines().count().saturating_sub(1)).unwrap_or(0);

    let is_new_file = !csv_path.exists();
    let mut csv = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&csv_path)
        .expect("não foi possível abrir o CSV de resultados");
    if is_new_file {
        writeln!(csv, "k,repeticao,proving_time_ms,verification_time_ms,proof_size_bytes")
            .expect("falha ao escrever cabeçalho do CSV");
    }

    let mut samples: Vec<BenchSample> = Vec::with_capacity(n);

    for i in (existing_reps + 1)..=(existing_reps + n) {
        let sample = bench_case(&client, &proving_key, &batch)
            .unwrap_or_else(|e| panic!("k={k} repetição {i} falhou: {e}"));

        writeln!(
            csv,
            "{k},{i},{},{},{}",
            sample.proving_time.as_secs_f64() * 1000.0,
            sample.verification_time.as_secs_f64() * 1000.0,
            sample.proof_size_bytes
        )
        .expect("falha ao escrever linha do CSV");
        csv.flush().expect("falha ao gravar CSV em disco");

        println!(
            "k={k} [rep. absoluta {i}, {}/{n} desta execução] proving={:.2?} verificacao={:.2?} tamanho={}B",
            i - existing_reps,
            sample.proving_time,
            sample.verification_time,
            sample.proof_size_bytes
        );

        samples.push(sample);
    }

    let proving_ms: Vec<f64> =
        samples.iter().map(|s| s.proving_time.as_secs_f64() * 1000.0).collect();
    let verify_ms: Vec<f64> =
        samples.iter().map(|s| s.verification_time.as_secs_f64() * 1000.0).collect();
    let sizes: Vec<f64> = samples.iter().map(|s| s.proof_size_bytes as f64).collect();

    let proving_mean = mean(&proving_ms);
    let verify_mean = mean(&verify_ms);
    let size_mean = mean(&sizes);

    println!("\n=== Resumo k={k} (N={n}) ===");
    println!(
        "Proving:     média {:.2} ms, desvio padrão {:.2} ms",
        proving_mean,
        stddev(&proving_ms, proving_mean)
    );
    println!(
        "Verificação: média {:.2} ms, desvio padrão {:.2} ms",
        verify_mean,
        stddev(&verify_ms, verify_mean)
    );
    println!(
        "Tamanho da prova: média {:.0} bytes, desvio padrão {:.2} bytes",
        size_mean,
        stddev(&sizes, size_mean)
    );
    println!("\nResultados salvos em {}", csv_path.display());
}
