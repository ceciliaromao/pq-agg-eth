//! Extensão do M5: mesma coisa que bench_m5_agg, mas usando o guest
//! agregador com raiz de Merkle (aggregate-mldsa-merkle) em vez do
//! aggregate-mldsa original, para verificar se isso torna o custo de gas
//! de `verifyProof` constante em relação a n (a hipótese registrada como
//! trabalho futuro na primeira rodada do M5).
//!
//! `aggregate_groth16` é reusado sem alterações: o formato do stdin é
//! idêntico entre os dois guests agregadores, só a proving key muda.
//!
//! Uso: cargo run --release -p verify-mldsa-host --bin bench_m5_merkle -- <n>
//! (n precisa ser potência de 2). Fixture escrita em
//! contracts/fixtures/groth16_merkle_n<n>.json.

use std::fs;
use std::path::Path;
use std::time::Instant;

use sp1_sdk::blocking::EnvProver;
use sp1_sdk::{HashableKey, ProvingKey};
use verify_mldsa_host::{aggregate_groth16, prove_sub_proofs, setup, setup_aggregate_merkle};

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(8);
    assert!(n.is_power_of_two(), "n precisa ser potência de 2 para o guest com raiz de Merkle");

    let client = EnvProver::new();
    let sub_proving_key = setup(&client);
    let aggregate_proving_key = setup_aggregate_merkle(&client);
    let aggregate_vk = aggregate_proving_key.verifying_key();

    let sub_start = Instant::now();
    let sub_proofs = prove_sub_proofs(&client, &sub_proving_key, n);
    let sub_proving_time = sub_start.elapsed();
    println!("n={n}: {n} sub-provas geradas em {sub_proving_time:.2?} no total");

    let outcome = aggregate_groth16(&client, &aggregate_proving_key, sub_proofs);
    let proof = outcome.result.expect("agregação Groth16 com raiz de Merkle deveria ter sucesso");

    let vkey_hex = aggregate_vk.bytes32();
    let public_values_hex = format!("0x{}", hex::encode(proof.public_values.to_vec()));
    let proof_hex = format!("0x{}", hex::encode(proof.bytes()));

    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/fixtures");
    fs::create_dir_all(&fixtures_dir).expect("não foi possível criar contracts/fixtures");
    let fixture_path = fixtures_dir.join(format!("groth16_merkle_n{n}.json"));

    let json = format!(
        "{{\n  \"n\": {n},\n  \"vkey\": \"{vkey_hex}\",\n  \"publicValues\": \"{public_values_hex}\",\n  \"proof\": \"{proof_hex}\"\n}}\n"
    );
    fs::write(&fixture_path, json).expect("falha ao escrever a fixture");

    println!(
        "n={n}: agregação Groth16 com raiz de Merkle gerada e verificada em {:.2?}",
        outcome.proving_time
    );
    println!("n={n}: encoding on-chain com {} bytes", proof.bytes().len());
    println!("n={n}: valores públicos com {} bytes", proof.public_values.to_vec().len());
    println!(
        "n={n}: tempo total (sub-provas + agregação Groth16) {:.2?}",
        sub_proving_time + outcome.proving_time
    );
    println!("n={n}: fixture salva em {}", fixture_path.display());
}
