//! Harness de benchmark do M2.
//!
//! N=30 repetições do mesmo par (chave, mensagem, assinatura), fixo entre
//! repetições de propósito, para isolar ruído de medição/sistema
//! (scheduling, throttling térmico) da variância intrínseca entre inputs
//! diferentes, que é um fenômeno distinto e fora de escopo aqui.
//!
//! Mede proving e verificação separadamente (diferente do M1, que mede os
//! dois juntos) e reporta o tamanho da prova serializada em bytes.
//! Resultados por repetição em docs/results/m2_benchmark.csv. Resumo
//! (média, desvio padrão) impresso no fim.
//!
//! Hardware oficial deste benchmark: ver README.
//!
//! Rodar com: cargo run --release -p verify-mldsa-host --bin bench
//! (N=30 vezes ~70s por repetição em modo compressed, cerca de 35 a 50 minutos)

use std::fs::File;
use std::io::Write;
use std::path::Path;

use sp1_sdk::blocking::EnvProver;
use verify_mldsa_host::{bench_case, setup, valid_case, BenchSample};

const N_REPETITIONS: usize = 30;

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn stddev(values: &[f64], mean: f64) -> f64 {
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    variance.sqrt()
}

fn main() {
    let client = EnvProver::new();
    let proving_key = setup(&client);

    // Par fixo, gerado uma única vez e reusado em todas as repetições, para
    // isolar ruído de medição/sistema da variância entre inputs diferentes.
    let case = valid_case("m2-bench", b"transacao de benchmark M2 - par fixo entre repeticoes");

    let results_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/results");
    std::fs::create_dir_all(&results_dir).expect("não foi possível criar docs/results");
    let csv_path = results_dir.join("m2_benchmark.csv");
    let mut csv = File::create(&csv_path).expect("não foi possível criar o CSV de resultados");
    writeln!(csv, "repeticao,proving_time_ms,verification_time_ms,proof_size_bytes")
        .expect("falha ao escrever cabeçalho do CSV");

    let mut samples: Vec<BenchSample> = Vec::with_capacity(N_REPETITIONS);

    for i in 1..=N_REPETITIONS {
        let sample = bench_case(&client, &proving_key, std::slice::from_ref(&case))
            .unwrap_or_else(|e| panic!("repetição {i} falhou: {e}"));

        writeln!(
            csv,
            "{i},{},{},{}",
            sample.proving_time.as_secs_f64() * 1000.0,
            sample.verification_time.as_secs_f64() * 1000.0,
            sample.proof_size_bytes
        )
        .expect("falha ao escrever linha do CSV");
        csv.flush().expect("falha ao gravar CSV em disco");

        println!(
            "[{i}/{N_REPETITIONS}] proving={:.2?} verificacao={:.2?} tamanho={}B",
            sample.proving_time, sample.verification_time, sample.proof_size_bytes
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

    println!("\n=== Resumo (N={N_REPETITIONS}, par fixo) ===");
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
    println!("\nResultados por repetição salvos em {}", csv_path.display());
}
