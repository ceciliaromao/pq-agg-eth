//! Guest program SP1 — Milestone M1.
//!
//! Relação NP demonstrada: "conheço uma mensagem e uma assinatura ML-DSA-44
//! tais que a assinatura é válida sob a chave pública dada". Não há saída
//! booleana de validade — se a assinatura for inválida, a asserção abaixo
//! falha e nenhuma prova pode ser gerada para essa entrada (ver
//! docs/decisions.md, D2). Isso é o que torna a prova útil para agregação
//! futura: uma prova só existe quando a assinatura de fato verifica.
//!
//! Entradas lidas via sp1_zkvm::io (nessa ordem): pubkey, mensagem, assinatura,
//! todas como bytes brutos (read_vec) — arrays de tamanho fixo grandes não têm
//! impl Serialize/Deserialize direta via serde, por isso a conversão para
//! [u8; N] acontece aqui dentro, não no lado do host.
#![no_main]
sp1_zkvm::entrypoint!(main);

// Não existe um tipo `Signature` nesta crate — o associated type `Signature`
// do trait `Verifier` é o próprio `[u8; SIG_LEN]` (ver fips204 src/lib.rs,
// macro `functionality!`), então a assinatura não passa por SerDes/try_from_bytes.
use fips204::ml_dsa_44::{PublicKey, PK_LEN, SIG_LEN};
use fips204::traits::{SerDes, Verifier};

pub fn main() {
    let pk_bytes: Vec<u8> = sp1_zkvm::io::read_vec();
    let message: Vec<u8> = sp1_zkvm::io::read_vec();
    let sig_bytes: Vec<u8> = sp1_zkvm::io::read_vec();

    let pk_bytes: [u8; PK_LEN] = pk_bytes
        .try_into()
        .expect("chave pública com tamanho inesperado para ML-DSA-44");
    let sig_bytes: [u8; SIG_LEN] = sig_bytes
        .try_into()
        .expect("assinatura com tamanho inesperado para ML-DSA-44");

    let pk = PublicKey::try_from_bytes(pk_bytes).expect("chave pública ML-DSA malformada");

    // Contexto vazio (&[]) — FIPS 204 permite um contexto de domínio opcional;
    // M1 não usa (ver spec, Seção 2 — fora do escopo desta fase).
    let is_valid = pk.verify(&message, &sig_bytes, &[]);
    assert!(is_valid, "assinatura ML-DSA inválida — prova não pode ser gerada");

    // Entradas públicas da relação: liga a prova à chave pública e à mensagem
    // específicas (sem revelar nada além disso, e sem revelar a assinatura).
    sp1_zkvm::io::commit_slice(&pk_bytes);
    sp1_zkvm::io::commit_slice(&message);
}
