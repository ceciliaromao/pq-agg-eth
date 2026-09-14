//! Extensão do M5: gera uma prova Groth16 da composição recursiva do M4
//! (guest aggregate-mldsa), para os valores de n que o batching monolítico
//! do M3 não conseguiu alcançar (n=8, 16, 32), e exporta uma fixture JSON
//! consumida pelo teste de gas em contracts/test/GasBenchmark.t.sol.
//!
//! As n sub-provas continuam em modo compressed (exigido para verificação
//! recursiva); só a prova final da agregação é gerada em Groth16.
//!
//! Uso: cargo run --release -p verify-mldsa-host --bin bench_m5_agg -- <n>
//! Fixture escrita em contracts/fixtures/groth16_agg_n<n>.json.

use std::fs;
use std::path::Path;
use std::time::Instant;

use sp1_sdk::blocking::EnvProver;
use sp1_sdk::{HashableKey, ProvingKey};
use verify_mldsa_host::{aggregate_groth16, prove_sub_proofs, setup, setup_aggregate};

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(8);

    let client = EnvProver::new();
    let sub_proving_key = setup(&client);
    let aggregate_proving_key = setup_aggregate(&client);
    let aggregate_vk = aggregate_proving_key.verifying_key();

    let sub_start = Instant::now();
    let sub_proofs = prove_sub_proofs(&client, &sub_proving_key, n);
    let sub_proving_time = sub_start.elapsed();
    println!("n={n}: {n} sub-provas geradas em {sub_proving_time:.2?} no total");

    let outcome = aggregate_groth16(&client, &aggregate_proving_key, sub_proofs);
    let proof = outcome.result.expect("agregação Groth16 deveria ter sucesso");

    let vkey_hex = aggregate_vk.bytes32();
    let public_values_hex = format!("0x{}", hex::encode(proof.public_values.to_vec()));
    let proof_hex = format!("0x{}", hex::encode(proof.bytes()));

    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/fixtures");
    fs::create_dir_all(&fixtures_dir).expect("não foi possível criar contracts/fixtures");
    let fixture_path = fixtures_dir.join(format!("groth16_agg_n{n}.json"));

    let json = format!(
        "{{\n  \"n\": {n},\n  \"vkey\": \"{vkey_hex}\",\n  \"publicValues\": \"{public_values_hex}\",\n  \"proof\": \"{proof_hex}\"\n}}\n"
    );
    fs::write(&fixture_path, json).expect("falha ao escrever a fixture");

    println!("n={n}: agregação Groth16 gerada e verificada em {:.2?}", outcome.proving_time);
    println!("n={n}: encoding on-chain com {} bytes", proof.bytes().len());
    println!(
        "n={n}: tempo total (sub-provas + agregação Groth16) {:.2?}",
        sub_proving_time + outcome.proving_time
    );
    println!("n={n}: fixture salva em {}", fixture_path.display());
}
