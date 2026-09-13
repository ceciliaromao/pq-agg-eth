//! Guest program SP1 para os Milestones M1 (k=1) e M3 (k>1, agregação por lote).
//!
//! Relação NP: conheço k pares (mensagem, assinatura) tais que cada
//! assinatura ML-DSA-44 é válida sob a respectiva chave pública. Não há
//! saída booleana de validade. Se qualquer uma das k assinaturas do lote
//! for inválida, a asserção falha e nenhuma prova pode ser gerada. Isso é o
//! que torna a prova útil para agregação: ela só existe quando todas as k
//! assinaturas verificam.
//!
//! Entradas lidas via sp1_zkvm::io: primeiro um `u32` `k` (tamanho do lote),
//! depois, k vezes, o triplo (pubkey, mensagem, assinatura) como bytes
//! brutos (read_vec). Arrays de tamanho fixo grandes não têm impl
//! Serialize/Deserialize direta via serde, por isso a conversão para
//! [u8; N] acontece aqui dentro, não no lado do host.
#![no_main]
sp1_zkvm::entrypoint!(main);

// Não existe um tipo Signature nesta crate. O associated type Signature do
// trait Verifier é o próprio [u8; SIG_LEN] (ver fips204 src/lib.rs, macro
// functionality!), então a assinatura não passa por SerDes/try_from_bytes.
use fips204::ml_dsa_44::{PublicKey, PK_LEN, SIG_LEN};
use fips204::traits::{SerDes, Verifier};

pub fn main() {
    let k: u32 = sp1_zkvm::io::read();

    for _ in 0..k {
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

        // Contexto vazio (&[]). FIPS 204 permite um contexto de domínio
        // opcional, não usado nesta fase.
        let is_valid = pk.verify(&message, &sig_bytes, &[]);
        assert!(is_valid, "assinatura ML-DSA inválida no lote, prova não pode ser gerada");

        // Entradas públicas da relação: liga a prova a cada chave pública e
        // mensagem do lote, sem revelar as assinaturas.
        sp1_zkvm::io::commit_slice(&pk_bytes);
        sp1_zkvm::io::commit_slice(&message);
    }
}
