//! Harness do M5: gera uma prova Groth16 (compatível com verificação
//! on-chain) para um lote de k assinaturas ML-DSA-44, nos mesmos valores de
//! k avaliados no M3, e exporta uma fixture JSON consumida pelo teste de
//! gas em contracts/test/GasBenchmark.t.sol.
//!
//! O modo Groth16 preserva o formato de saída que o `SP1Verifier.sol`
//! oficial do SP1 espera (`.bytes()`), ao custo de perder a segurança
//! pós-quântica de ponta a ponta: o wrapping final é uma prova SNARK
//! pairing-based sobre BN254. Essa ressalva está documentada no README.
//!
//! Uso: cargo run --release -p verify-mldsa-host --bin bench_m5 -- <k>
//! Fixture escrita em contracts/fixtures/groth16_k<k>.json.

use std::fs;
use std::path::Path;
use std::time::Instant;

use sp1_sdk::blocking::{EnvProver, ProveRequest, Prover};
use sp1_sdk::{HashableKey, ProvingKey, SP1Stdin};
use verify_mldsa_host::{setup, valid_batch, write_batch};

fn main() {
    let k: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(1);

    let client = EnvProver::new();
    let proving_key = setup(&client);
    let vk = proving_key.verifying_key();

    let batch = valid_batch("m5-groth16", k);
    let mut stdin = SP1Stdin::new();
    write_batch(&mut stdin, &batch);

    let start = Instant::now();
    let proof = client
        .prove(&proving_key, stdin)
        .groth16()
        .run()
        .expect("prova Groth16 deveria ter sucesso");
    let proving_time = start.elapsed();

    client.verify(&proof, vk, None).expect("verificação Groth16 deveria ter sucesso");

    let vkey_hex = vk.bytes32();
    let public_values_hex = format!("0x{}", hex::encode(proof.public_values.to_vec()));
    let proof_hex = format!("0x{}", hex::encode(proof.bytes()));

    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/fixtures");
    fs::create_dir_all(&fixtures_dir).expect("não foi possível criar contracts/fixtures");
    let fixture_path = fixtures_dir.join(format!("groth16_k{k}.json"));

    let json = format!(
        "{{\n  \"k\": {k},\n  \"vkey\": \"{vkey_hex}\",\n  \"publicValues\": \"{public_values_hex}\",\n  \"proof\": \"{proof_hex}\"\n}}\n"
    );
    fs::write(&fixture_path, json).expect("falha ao escrever a fixture");

    println!("k={k}: prova Groth16 gerada e verificada em {proving_time:.2?}");
    println!("k={k}: encoding on-chain com {} bytes", proof.bytes().len());
    println!("k={k}: fixture salva em {}", fixture_path.display());
}
