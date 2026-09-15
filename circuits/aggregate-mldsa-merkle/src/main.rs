//! Guest program SP1, variante experimental do agregador do M4: em vez de
//! repassar os valores públicos brutos de cada sub-prova como saída
//! pública própria, calcula a raiz de uma árvore de Merkle binária sobre
//! esses valores e comita só a raiz (32 bytes, fixo independente de n).
//! Investiga se isso torna o custo de gas de verificação on-chain
//! constante em relação a n (extensão do M5).
//!
//! Mesma relação de verificação recursiva do guest aggregate-mldsa:
//! verifica n sub-provas do guest verify-mldsa via a syscall de
//! verificação recursiva do SP1. A raiz é recalculada aqui dentro a partir
//! do hash de cada sub-prova, não recebida como entrada do host, então a
//! prova só existe se a raiz corresponder às sub-provas de fato
//! verificadas.
//!
//! Assume n potência de 2 (sem padding para folhas ímpares), suficiente
//! para os valores de n testados (8 e 32).

#![no_main]
sp1_zkvm::entrypoint!(main);

use sha2::{Digest, Sha256};

pub fn main() {
    let n: u32 = sp1_zkvm::io::read();
    assert!(n.is_power_of_two() && n > 0, "n precisa ser potência de 2 nesta variante");

    let mut level: Vec<[u8; 32]> = Vec::with_capacity(n as usize);

    for _ in 0..n {
        let vk_digest: [u32; 8] = sp1_zkvm::io::read();
        let public_values: Vec<u8> = sp1_zkvm::io::read_vec();

        let pv_digest: [u8; 32] = Sha256::digest(&public_values).into();
        sp1_zkvm::lib::verify::verify_sp1_proof(&vk_digest, &pv_digest);

        // A folha é o mesmo digest já verificado pela syscall, sem hash
        // adicional: cada folha corresponde exatamente à sub-prova que
        // acabou de ser verificada.
        level.push(pv_digest);
    }

    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len() / 2);
        for pair in level.chunks_exact(2) {
            let mut hasher = Sha256::new();
            hasher.update(pair[0]);
            hasher.update(pair[1]);
            next.push(hasher.finalize().into());
        }
        level = next;
    }
    let root = level[0];

    sp1_zkvm::io::commit_slice(&root);
}
