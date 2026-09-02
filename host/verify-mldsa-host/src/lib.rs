//! Host program — Milestone M1.
//!
//! Gera pares (chave pública, mensagem, assinatura) ML-DSA-44 — válidos e
//! inválidos — e roda cada um pelo guest program `verify-mldsa` via SP1,
//! usando o modo de prova STARK "compressed" (ver docs/decisions.md, D1).
//!
//! API do sp1-sdk 6.5.0 (blocking, feature `blocking`) — conferida lendo o
//! source vendorizado da crate, não só docs: `EnvProver::setup(elf: Elf)`
//! retorna só a proving key (`EnvProvingKey`), não um par (pk, vk); a
//! verifying key vem de `proving_key.verifying_key()` via o trait
//! `ProvingKey`. `Elf` é um tipo próprio (`include_elf!` retorna `Elf`, não
//! `&[u8]`).
//!
//! Descoberta empírica importante (via `cargo run`, não só leitura): `prove()`
//! SEMPRE sucede em `SP1` 6.5.0, mesmo quando o guest dá panic (`assert!`
//! falho) — a prova STARK simplesmente atesta "o programa rodou e terminou
//! com exit_code X", panic incluso. Quem de fato rejeita é `verify(proof, vk,
//! None)`: com `status_code: None`, `verify_proof` (sp1-sdk `src/prover.rs`)
//! usa `StatusCode::SUCCESS` como default e retorna
//! `Err(UnexpectedExitCode)` se o exit_code commitado não for de sucesso.
//! Por isso `prove_case` abaixo encadeia prove+verify — a relação NP do M1
//! (D2) só "aceita" um caso quando as duas etapas sucedem.

use fips204::ml_dsa_44::{self};
use fips204::traits::{KeyGen, SerDes, Signer};
use sp1_sdk::blocking::{EnvProver, EnvProvingKey, ProveRequest, Prover};
use sp1_sdk::{include_elf, Elf, ProvingKey, SP1ProofWithPublicValues, SP1Stdin, SP1VerifyingKey};
use std::time::{Duration, Instant};

/// ELF do guest program `verify-mldsa`, buildado pelo build.rs via sp1-build.
pub const ELF: Elf = include_elf!("verify-mldsa");

/// Um caso de teste: chave pública, mensagem e assinatura, todos como bytes brutos
/// (o formato que o guest espera via `sp1_zkvm::io::read_vec`).
pub struct MlDsaCase {
    pub label: &'static str,
    pub pk_bytes: Vec<u8>,
    pub message: Vec<u8>,
    pub sig_bytes: Vec<u8>,
    /// Se `false`, espera-se que a geração de prova falhe (ver docs/decisions.md, D2).
    pub expected_valid: bool,
}

fn keygen_and_sign(message: &[u8]) -> (Vec<u8>, Vec<u8>) {
    // KG::try_keygen (não ml_dsa_44::try_keygen direto) e a assinatura já
    // sai como [u8; SIG_LEN] — o associated type `Signature` do trait
    // `Signer` É o array bruto nesta crate, sem SerDes (ver circuits/verify-mldsa/src/main.rs).
    let (pk, sk) = ml_dsa_44::KG::try_keygen().expect("keygen ML-DSA-44 falhou");
    let sig: [u8; ml_dsa_44::SIG_LEN] =
        sk.try_sign(message, &[]).expect("assinatura ML-DSA-44 falhou");
    (pk.into_bytes().to_vec(), sig.to_vec())
}

/// Caso positivo: assinatura genuína sobre `message`.
pub fn valid_case(label: &'static str, message: &[u8]) -> MlDsaCase {
    let (pk_bytes, sig_bytes) = keygen_and_sign(message);
    MlDsaCase {
        label,
        pk_bytes,
        message: message.to_vec(),
        sig_bytes,
        expected_valid: true,
    }
}

/// Caso negativo: um byte da assinatura é invertido após uma assinatura genuína.
pub fn case_tampered_signature(label: &'static str, message: &[u8]) -> MlDsaCase {
    let mut case = valid_case(label, message);
    let last = case.sig_bytes.len() - 1;
    case.sig_bytes[last] ^= 0x01;
    case.expected_valid = false;
    case
}

/// Caso negativo: assinatura genuína, mas a mensagem verificada é outra.
pub fn case_wrong_message(label: &'static str, signed: &[u8], claimed: &[u8]) -> MlDsaCase {
    let mut case = valid_case(label, signed);
    case.message = claimed.to_vec();
    case.expected_valid = false;
    case
}

/// Caso negativo: assinatura genuína, mas verificada contra uma chave pública diferente.
pub fn case_wrong_pubkey(label: &'static str, message: &[u8]) -> MlDsaCase {
    let mut case = valid_case(label, message);
    let (other_pk, _) = ml_dsa_44::KG::try_keygen().expect("keygen ML-DSA-44 falhou");
    case.pk_bytes = other_pk.into_bytes().to_vec();
    case.expected_valid = false;
    case
}

pub struct ProveOutcome {
    /// `Ok` só quando prove() E verify() sucedem — ver nota de API acima (D2).
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

/// Roda um caso pelo guest program: gera a prova (modo compressed, D1) e a
/// verifica. Só retorna `Ok` se ambas as etapas sucederem — ver a nota de
/// API no topo do arquivo sobre por que a rejeição acontece em `verify`,
/// não em `prove`.
pub fn prove_case(
    client: &EnvProver,
    proving_key: &EnvProvingKey,
    case: &MlDsaCase,
) -> ProveOutcome {
    let mut stdin = SP1Stdin::new();
    stdin.write_vec(case.pk_bytes.clone());
    stdin.write_vec(case.message.clone());
    stdin.write_vec(case.sig_bytes.clone());

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

/// Uma amostra do harness de benchmark do M2: proving e verificação medidos
/// separadamente (diferente de `ProveOutcome::proving_time`, que no M1 mede
/// os dois juntos), mais o tamanho da prova serializada em bytes.
pub struct BenchSample {
    pub proving_time: Duration,
    pub verification_time: Duration,
    /// Serialização via bincode — o mesmo formato usado por `SP1ProofWithPublicValues::save`.
    pub proof_size_bytes: usize,
}

/// Como `prove_case`, mas para o M2 (spec, Seção 4): mede proving e
/// verificação em etapas separadas e reporta o tamanho da prova.
pub fn bench_case(
    client: &EnvProver,
    proving_key: &EnvProvingKey,
    case: &MlDsaCase,
) -> Result<BenchSample, String> {
    let mut stdin = SP1Stdin::new();
    stdin.write_vec(case.pk_bytes.clone());
    stdin.write_vec(case.message.clone());
    stdin.write_vec(case.sig_bytes.clone());

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
