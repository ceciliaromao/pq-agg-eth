//! Guest program SP1 para o Milestone M4: composição recursiva de sub-provas
//! do guest verify-mldsa, como alternativa ao batching simples do M3.
//!
//! Cada sub-prova (gerada com o guest verify-mldsa em modo compressed,
//! k=1) atesta a validade de uma assinatura ML-DSA-44. Este programa
//! verifica n dessas sub-provas dentro da própria execução, usando a
//! syscall de verificação recursiva do SP1, e produz uma única prova final
//! que só existe se todas as n sub-provas forem válidas.
//!
//! Entradas lidas via sp1_zkvm::io: um `u32` `n` (quantidade de
//! sub-provas), seguido, para cada sub-prova, do digest da sua chave de
//! verificação (`[u32; 8]`) e dos seus valores públicos brutos (bytes). O
//! conteúdo opaco da sub-prova em si não é lido pelo buffer normal: é
//! fornecido separadamente pelo host (via SP1Stdin::write_proof) e casado
//! automaticamente com cada chamada de verificação, na ordem em que
//! acontecem.
#![no_main]
sp1_zkvm::entrypoint!(main);

use sha2::{Digest, Sha256};

pub fn main() {
    let n: u32 = sp1_zkvm::io::read();

    for _ in 0..n {
        let vk_digest: [u32; 8] = sp1_zkvm::io::read();
        let public_values: Vec<u8> = sp1_zkvm::io::read_vec();

        let pv_digest: [u8; 32] = Sha256::digest(&public_values).into();
        sp1_zkvm::lib::verify::verify_sp1_proof(&vk_digest, &pv_digest);

        // Repassa os valores públicos da sub-prova (pubkey e mensagem,
        // definidos pelo guest verify-mldsa) como saída pública da
        // agregação.
        sp1_zkvm::io::commit_slice(&public_values);
    }
}
