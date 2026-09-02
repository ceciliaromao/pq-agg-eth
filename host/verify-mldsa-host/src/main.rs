//! Demo do pipeline do M1: gera um par válido e um inválido, prova+verifica
//! cada um (rejeição acontece em verify, não em prove — ver nota de API em
//! src/lib.rs), e reporta o tempo observado. Não é o harness de benchmark
//! formal (isso é M2) — só uma checagem manual rápida de que o pipeline
//! guest+host está funcionando.
//!
//! Rodar com: cargo run --release -p verify-mldsa-host

use sp1_sdk::blocking::EnvProver;
use verify_mldsa_host::{case_tampered_signature, prove_case, setup, valid_case};

fn main() {
    let client = EnvProver::new();
    let proving_key = setup(&client);

    let valid = valid_case("demo-valida", b"mensagem de teste M1");
    let outcome = prove_case(&client, &proving_key, &valid);
    match outcome.result {
        Ok(_) => println!(
            "[{}] prova gerada e verificada em {:.2?}",
            valid.label, outcome.proving_time
        ),
        Err(e) => panic!("caso válido deveria gerar+verificar prova, mas falhou: {e}"),
    }

    let invalid = case_tampered_signature("demo-invalida", b"mensagem de teste M1");
    let outcome = prove_case(&client, &proving_key, &invalid);
    match outcome.result {
        Ok(_) => panic!("caso inválido não deveria passar na verificação, mas passou"),
        Err(e) => println!(
            "[{}] rejeitado corretamente em {:.2?} ({e})",
            invalid.label, outcome.proving_time
        ),
    }
}
