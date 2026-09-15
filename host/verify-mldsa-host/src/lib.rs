//! Host program para os Milestones M1 (k=1) e M3 (k>1, agregação por lote).
//!
//! Gera pares (chave pública, mensagem, assinatura) ML-DSA-44, válidos e
//! inválidos, individuais ou em lote, e roda cada lote pelo guest program
//! `verify-mldsa` via SP1, usando o modo de prova STARK "compressed" (sem
//! wrapping para Groth16/PLONK, ver README).
//!
//! Notas sobre a API do sp1-sdk 6.5.0 (feature `blocking`):
//!
//! `EnvProver::setup(elf: Elf)` retorna só a proving key (`EnvProvingKey`).
//! A verifying key vem de `proving_key.verifying_key()`, do trait
//! `ProvingKey`. `Elf` é um tipo próprio; `include_elf!` retorna `Elf`, não
//! `&[u8]`.
//!
//! `prove()` sempre retorna `Ok`, mesmo quando o guest dá panic: a prova
//! STARK atesta que o programa rodou e terminou com um dado exit_code,
//! panic incluso. A rejeição acontece em `verify(proof, vk, None)`: com
//! `status_code: None`, a verificação usa `StatusCode::SUCCESS` como padrão
//! e retorna `Err(UnexpectedExitCode)` se o exit_code não indicar sucesso.
//! Por isso `prove_case` encadeia prove e verify: a relação NP só aceita um
//! lote quando as duas etapas sucedem para todos os seus casos.

use fips204::ml_dsa_44::{self};
use fips204::traits::{KeyGen, SerDes, Signer};
use sp1_sdk::blocking::{EnvProver, EnvProvingKey, ProveRequest, Prover};
use sp1_sdk::{
    include_elf, Elf, HashableKey, ProvingKey, SP1Proof, SP1ProofWithPublicValues, SP1Stdin,
    SP1VerifyingKey,
};
use std::time::{Duration, Instant};

/// ELF do guest program `verify-mldsa`, buildado pelo build.rs via sp1-build.
pub const ELF: Elf = include_elf!("verify-mldsa");

/// ELF do guest program `aggregate-mldsa` (M4), buildado pelo build.rs via sp1-build.
pub const AGGREGATE_ELF: Elf = include_elf!("aggregate-mldsa");

/// ELF do guest program `aggregate-mldsa-merkle` (extensão do M5): mesma
/// verificação recursiva do aggregate-mldsa, mas comita só a raiz de
/// Merkle das sub-provas em vez dos valores públicos brutos de cada uma.
pub const AGGREGATE_MERKLE_ELF: Elf = include_elf!("aggregate-mldsa-merkle");

/// Um caso de teste: chave pública, mensagem e assinatura, todos como bytes brutos
/// (o formato que o guest espera via `sp1_zkvm::io::read_vec`).
pub struct MlDsaCase {
    pub label: String,
    pub pk_bytes: Vec<u8>,
    pub message: Vec<u8>,
    pub sig_bytes: Vec<u8>,
    /// Se `false`, espera-se que a geração de prova falhe (a assinatura em
    /// questão faz o lote inteiro ser rejeitado em `verify`).
    pub expected_valid: bool,
}

fn keygen_and_sign(message: &[u8]) -> (Vec<u8>, Vec<u8>) {
    // KG::try_keygen (não ml_dsa_44::try_keygen direto). A assinatura já sai
    // como [u8; SIG_LEN]: o associated type Signature do trait Signer é o
    // array bruto nesta crate, sem SerDes.
    let (pk, sk) = ml_dsa_44::KG::try_keygen().expect("keygen ML-DSA-44 falhou");
    let sig: [u8; ml_dsa_44::SIG_LEN] =
        sk.try_sign(message, &[]).expect("assinatura ML-DSA-44 falhou");
    (pk.into_bytes().to_vec(), sig.to_vec())
}

/// Caso positivo: assinatura genuína sobre `message`.
pub fn valid_case(label: impl Into<String>, message: &[u8]) -> MlDsaCase {
    let (pk_bytes, sig_bytes) = keygen_and_sign(message);
    MlDsaCase {
        label: label.into(),
        pk_bytes,
        message: message.to_vec(),
        sig_bytes,
        expected_valid: true,
    }
}

/// Caso negativo: um byte da assinatura é invertido após uma assinatura genuína.
pub fn case_tampered_signature(label: impl Into<String>, message: &[u8]) -> MlDsaCase {
    let mut case = valid_case(label, message);
    let last = case.sig_bytes.len() - 1;
    case.sig_bytes[last] ^= 0x01;
    case.expected_valid = false;
    case
}

/// Caso negativo: assinatura genuína, mas a mensagem verificada é outra.
pub fn case_wrong_message(label: impl Into<String>, signed: &[u8], claimed: &[u8]) -> MlDsaCase {
    let mut case = valid_case(label, signed);
    case.message = claimed.to_vec();
    case.expected_valid = false;
    case
}

/// Caso negativo: assinatura genuína, mas verificada contra uma chave pública diferente.
pub fn case_wrong_pubkey(label: impl Into<String>, message: &[u8]) -> MlDsaCase {
    let mut case = valid_case(label, message);
    let (other_pk, _) = ml_dsa_44::KG::try_keygen().expect("keygen ML-DSA-44 falhou");
    case.pk_bytes = other_pk.into_bytes().to_vec();
    case.expected_valid = false;
    case
}

/// Gera k pares válidos independentes, com chave e mensagem distintas por
/// índice. Simula k contas diferentes assinando k transações diferentes,
/// o cenário de uso real de agregação.
pub fn valid_batch(label_prefix: &str, k: usize) -> Vec<MlDsaCase> {
    (0..k)
        .map(|i| {
            let message = format!("{label_prefix}-msg-{i}");
            valid_case(format!("{label_prefix}-{i}"), message.as_bytes())
        })
        .collect()
}

pub struct ProveOutcome {
    /// `Ok` só quando prove() e verify() sucedem para todo o lote (ver
    /// comentário no início do arquivo).
    pub result: Result<SP1ProofWithPublicValues, String>,
    pub proving_time: Duration,
}

/// Faz o setup do guest program uma única vez (é caro, não repetir por caso).
/// A verifying key vem de `proving_key.verifying_key()` (trait `ProvingKey`).
pub fn setup(client: &EnvProver) -> EnvProvingKey {
    client.setup(ELF).expect("setup do guest program verify-mldsa falhou")
}

/// Chave de verificação correspondente à proving key retornada por `setup`.
pub fn verifying_key(proving_key: &EnvProvingKey) -> &SP1VerifyingKey {
    proving_key.verifying_key()
}

/// Escreve o lote no stdin do guest: `k` (u32) seguido de k triplos
/// (pubkey, mensagem, assinatura). k=1 (slice de um elemento) é o formato
/// usado pelo M1.
pub fn write_batch(stdin: &mut SP1Stdin, cases: &[MlDsaCase]) {
    stdin.write(&(cases.len() as u32));
    for case in cases {
        stdin.write_vec(case.pk_bytes.clone());
        stdin.write_vec(case.message.clone());
        stdin.write_vec(case.sig_bytes.clone());
    }
}

/// Roda um lote de k casos pelo guest program (k=1 é o cenário do M1): gera
/// a prova em modo compressed e a verifica. Só retorna `Ok` se ambas as
/// etapas sucederem para todo o lote (a rejeição acontece em `verify`, não
/// em `prove`; ver comentário no início do arquivo).
pub fn prove_case(
    client: &EnvProver,
    proving_key: &EnvProvingKey,
    cases: &[MlDsaCase],
) -> ProveOutcome {
    let mut stdin = SP1Stdin::new();
    write_batch(&mut stdin, cases);

    let start = Instant::now();
    let result = client
        .prove(proving_key, stdin)
        .compressed()
        .run()
        .map_err(|e| e.to_string())
        .and_then(|proof| {
            client
                .verify(&proof, proving_key.verifying_key(), None)
                .map(|()| proof)
                .map_err(|e| e.to_string())
        });
    let proving_time = start.elapsed();

    ProveOutcome { result, proving_time }
}

/// Uma amostra do harness de benchmark do M2/M3: proving e verificação
/// medidos separadamente (diferente de `ProveOutcome::proving_time`, que
/// mede os dois juntos), mais o tamanho da prova serializada em bytes.
pub struct BenchSample {
    pub proving_time: Duration,
    pub verification_time: Duration,
    /// Serialização via bincode (o mesmo formato usado por `SP1ProofWithPublicValues::save`).
    pub proof_size_bytes: usize,
}

/// Como `prove_case`, mas para os harnesses de benchmark (M2 para k=1, M3
/// para k>1): mede proving e verificação em etapas separadas e reporta o
/// tamanho da prova. `cases` deve conter só casos válidos, já que o
/// benchmark caracteriza o caminho normal, não rejeição (rejeição é coberta
/// pelos testes automatizados do M1).
pub fn bench_case(
    client: &EnvProver,
    proving_key: &EnvProvingKey,
    cases: &[MlDsaCase],
) -> Result<BenchSample, String> {
    let mut stdin = SP1Stdin::new();
    write_batch(&mut stdin, cases);

    let prove_start = Instant::now();
    let proof = client.prove(proving_key, stdin).compressed().run().map_err(|e| e.to_string())?;
    let proving_time = prove_start.elapsed();

    let proof_size_bytes = bincode::serialize(&proof).map_err(|e| e.to_string())?.len();

    let verify_start = Instant::now();
    client
        .verify(&proof, proving_key.verifying_key(), None)
        .map_err(|e| e.to_string())?;
    let verification_time = verify_start.elapsed();

    Ok(BenchSample { proving_time, verification_time, proof_size_bytes })
}

/// Uma sub-prova para o Milestone M4 (composição recursiva): uma prova
/// compressed do guest verify-mldsa (k=1), junto da sua verifying key.
pub struct SubProof {
    pub proof: SP1ProofWithPublicValues,
    pub vk: SP1VerifyingKey,
}

/// Gera n sub-provas independentes, cada uma verificando uma assinatura
/// ML-DSA-44 diferente, usando o mesmo guest program e proving key do M1/M3.
pub fn prove_sub_proofs(client: &EnvProver, sub_proving_key: &EnvProvingKey, n: usize) -> Vec<SubProof> {
    (0..n)
        .map(|i| {
            let message = format!("m4-sub-msg-{i}");
            let case = valid_case(format!("m4-sub-{i}"), message.as_bytes());

            let mut stdin = SP1Stdin::new();
            write_batch(&mut stdin, std::slice::from_ref(&case));

            let proof = client
                .prove(sub_proving_key, stdin)
                .compressed()
                .run()
                .expect("sub-prova do M4 deveria ter sucesso");

            SubProof { proof, vk: sub_proving_key.verifying_key().clone() }
        })
        .collect()
}

/// Faz o setup do guest program agregador (M4).
pub fn setup_aggregate(client: &EnvProver) -> EnvProvingKey {
    client.setup(AGGREGATE_ELF).expect("setup do guest program aggregate-mldsa falhou")
}

/// Faz o setup do guest program agregador com raiz de Merkle (extensão do
/// M5). `aggregate_groth16` funciona com essa proving key sem alterações,
/// já que o formato do stdin (n, digests e sub-provas) é idêntico ao do
/// aggregate-mldsa: só a saída pública do guest muda.
pub fn setup_aggregate_merkle(client: &EnvProver) -> EnvProvingKey {
    client
        .setup(AGGREGATE_MERKLE_ELF)
        .expect("setup do guest program aggregate-mldsa-merkle falhou")
}

/// Monta o stdin do guest agregador a partir de n sub-provas: escreve o
/// digest da verifying key e os valores públicos brutos de cada uma no
/// stdin normal, e a prova em si via `SP1Stdin::write_proof` (consumida
/// pela syscall de verificação recursiva dentro do guest, na mesma ordem).
fn build_aggregate_stdin(sub_proofs: Vec<SubProof>) -> SP1Stdin {
    let mut stdin = SP1Stdin::new();
    stdin.write(&(sub_proofs.len() as u32));
    for sub in &sub_proofs {
        stdin.write(&sub.vk.hash_u32());
        stdin.write_vec(sub.proof.public_values.to_vec());
    }
    for sub in sub_proofs {
        let SP1Proof::Compressed(recursion_proof) = sub.proof.proof else {
            panic!("sub-prova do M4 não está em modo compressed");
        };
        stdin.write_proof(*recursion_proof, sub.vk.vk.clone());
    }
    stdin
}

/// Compõe n sub-provas já geradas numa única prova recursiva, em modo
/// compressed (M4).
pub fn aggregate(
    client: &EnvProver,
    aggregate_proving_key: &EnvProvingKey,
    sub_proofs: Vec<SubProof>,
) -> ProveOutcome {
    let stdin = build_aggregate_stdin(sub_proofs);

    let start = Instant::now();
    let result = client
        .prove(aggregate_proving_key, stdin)
        .compressed()
        .run()
        .map_err(|e| e.to_string())
        .and_then(|proof| {
            client
                .verify(&proof, aggregate_proving_key.verifying_key(), None)
                .map(|()| proof)
                .map_err(|e| e.to_string())
        });
    let proving_time = start.elapsed();

    ProveOutcome { result, proving_time }
}

/// Como `aggregate`, mas em modo Groth16 (M5): compõe n sub-provas numa
/// única prova recursiva já no formato aceito por um verificador on-chain.
pub fn aggregate_groth16(
    client: &EnvProver,
    aggregate_proving_key: &EnvProvingKey,
    sub_proofs: Vec<SubProof>,
) -> ProveOutcome {
    let stdin = build_aggregate_stdin(sub_proofs);

    let start = Instant::now();
    let result = client
        .prove(aggregate_proving_key, stdin)
        .groth16()
        .run()
        .map_err(|e| e.to_string())
        .and_then(|proof| {
            client
                .verify(&proof, aggregate_proving_key.verifying_key(), None)
                .map(|()| proof)
                .map_err(|e| e.to_string())
        });
    let proving_time = start.elapsed();

    ProveOutcome { result, proving_time }
}
