//! Harness comparativo do M4: agregação recursiva de n sub-provas do guest
//! verify-mldsa, como alternativa ao batching monolítico do M3.
//!
//! Uso: cargo run --release -p verify-mldsa-host --bin bench_m4 -- <n>
//! (padrão: n=2 se omitido)

use std::time::Instant;

use sp1_sdk::blocking::EnvProver;
use verify_mldsa_host::{aggregate, prove_sub_proofs, setup, setup_aggregate};

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(2);

    let client = EnvProver::new();
    let sub_proving_key = setup(&client);
    let aggregate_proving_key = setup_aggregate(&client);

    let sub_start = Instant::now();
    let sub_proofs = prove_sub_proofs(&client, &sub_proving_key, n);
    let sub_proving_time = sub_start.elapsed();
    println!("n={n}: {n} sub-provas geradas em {sub_proving_time:.2?} no total");

    let outcome = aggregate(&client, &aggregate_proving_key, sub_proofs);
    match outcome.result {
        Ok(proof) => {
            let proof_size = bincode::serialize(&proof).expect("falha ao serializar a prova").len();
            println!(
                "n={n}: agregação gerada e verificada em {:.2?}, tamanho {proof_size}B",
                outcome.proving_time
            );
            println!(
                "n={n}: tempo total (sub-provas + agregação) {:.2?}",
                sub_proving_time + outcome.proving_time
            );
        }
        Err(e) => panic!("agregação falhou: {e}"),
    }
}
