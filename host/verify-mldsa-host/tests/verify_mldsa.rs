//! Teste automatizado do critério de sucesso do M1: prova gerada e
//! verificada corretamente para casos válidos, rejeição correta para casos
//! inválidos, cobrindo pelo menos 10 pares chave/assinatura.
//!
//! A rejeição de casos inválidos acontece em `verify` (exit_code diferente
//! de sucesso é rejeitado por padrão), não em `prove` (ver comentário em
//! host/verify-mldsa-host/src/lib.rs).
//!
//! Proving em modo compressed é lento (dezenas de segundos por caso). Rodar
//! com:
//! cargo test --release -p verify-mldsa-host --test verify_mldsa -- --nocapture

use sp1_sdk::blocking::EnvProver;
use verify_mldsa_host::{
    case_tampered_signature, case_wrong_message, case_wrong_pubkey, prove_case, setup,
    valid_case,
};

#[test]
fn m1_aceita_assinaturas_validas_e_rejeita_invalidas() {
    let client = EnvProver::new();
    let proving_key = setup(&client);

    let cases = vec![
        valid_case("valida-01", b"transacao de teste 01"),
        valid_case("valida-02", b"transacao de teste 02"),
        valid_case("valida-03", b""),
        valid_case("valida-04", &vec![0xAB; 512]),
        valid_case("valida-05", b"mensagem unicode: acao, nao, coracao"),
        valid_case("valida-06", b"transacao de teste 06"),
        case_tampered_signature("invalida-assinatura-01", b"transacao de teste 07"),
        case_tampered_signature("invalida-assinatura-02", b""),
        case_wrong_message(
            "invalida-mensagem-01",
            b"mensagem assinada",
            b"mensagem reivindicada, diferente da assinada",
        ),
        case_wrong_pubkey("invalida-pubkey-01", b"transacao de teste 08"),
    ];
    assert!(cases.len() >= 10, "critério do M1 exige pelo menos 10 pares");

    for case in cases {
        let outcome = prove_case(&client, &proving_key, std::slice::from_ref(&case));
        match (&outcome.result, case.expected_valid) {
            (Ok(_), true) => {
                println!("[{}] OK, prova válida em {:.2?}", case.label, outcome.proving_time);
            }
            (Err(e), false) => {
                println!(
                    "[{}] OK, rejeitado corretamente em {:.2?} ({e})",
                    case.label, outcome.proving_time
                );
            }
            (Ok(_), false) => {
                panic!("[{}] deveria ser rejeitado, mas passou na verificação", case.label)
            }
            (Err(e), true) => {
                panic!("[{}] deveria gerar prova válida, mas falhou: {e}", case.label)
            }
        }
    }
}
