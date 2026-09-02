# Agregação de assinaturas pós-quânticas (ML-DSA) via provas de conhecimento zero, aplicada a contas de usuário (EOA) na Ethereum.

## Contexto

Este repositório contém a implementação associada à pesquisa de mestrado
*"Agregação de Assinaturas Pós-Quânticas via Provas de Conhecimento Zero para
Redução de Overhead em Contas Ethereum"*, desenvolvida no Programa de Pós-Graduação 
em Ciência da Computação da Universidade Federal de Juiz de Fora (UFJF), 
linha de pesquisa em Redes.

**Autora:** Maria Cecília Romão Santos
**Orientação:** Prof. Alex Borges Vieira (UFJF) e Prof. Glauber Dias Gonçalves (UFPI)

## Objetivo

Investigar a viabilidade técnica de um mecanismo de agregação de assinaturas ML-DSA
via zk-SNARK/zk-STARK, aplicado ao modelo de conta da Ethereum (EOA), com o
propósito de mitigar o overhead de propagação e verificação decorrente da adoção de
esquemas de assinatura pós-quântica.

O trabalho se posiciona na **Área 3** do roteiro de resistência quântica da
Ethereum (Buterin, 2026) — assinaturas de conta — buscando um mecanismo análogo ao
já anunciado para a Área 1 (leanXMSS/leanVM, compressão de 250x via agregação
SNARK), ainda inexistente para contas de usuário.

## Status

🚧 Em desenvolvimento — M1 (verificação individual) e M2 (harness de
benchmark) concluídos. Próximo: M3 (agregação por lote).

## Decisões técnicas em vigor

- **Modo de prova (M1-M4):** STARK "core"/"compressed" do SP1, sem wrapping
  Groth16/PLONK sobre BN254 — preserva segurança pós-quântica de ponta a
  ponta enquanto nenhum milestone exige verificação on-chain. Decisão
  reaberta em M5.
- **Referência ML-DSA:** crate Rust pura `no_std`-compatível (ex.: `fips204`),
  não `pqcrypto-dilithium` (inviável no target `riscv32im` do guest SP1 por
  usar FFI sobre C) nem implementação própria do zero.
- **Hardware oficial de benchmark (M1-M4):** Apple M4, 10 cores, 16 GB RAM,
  macOS 26.5.2. Não se aplica a M5 (custo de gas é definido pelo protocolo,
  independente de hardware).

## Estrutura do repositório

```
/circuits                       # guest programs SP1
  verify-mldsa/                 # M1: verificação de 1 assinatura ML-DSA-44
/host                           # host programs (geração/orquestração de provas)
  verify-mldsa-host/
    src/lib.rs                  # geração de casos + prove/verify (M1)
    src/bin/bench.rs            # harness de benchmark (M2)
    tests/verify_mldsa.rs       # suite de testes do M1
/contracts                      # Solidity (a partir de M5)
/docs
  results/                      # CSVs de benchmark por milestone (M2+)
```

Nota: a spec original (Seção 5) sugeria um `/bench` de topo separado para o
harness; na prática ele ficou dentro de `host/verify-mldsa-host/src/bin/`
para reusar a lógica de geração de casos e prove/verify já implementada em
`lib.rs`, sem precisar de uma dependência de path entre crates.

## Milestone M1 — verificação de assinatura ML-DSA-44 individual

Guest program SP1 (`circuits/verify-mldsa`) que recebe `(pubkey, message,
signature)` e só permite gerar prova se a assinatura ML-DSA-44 for válida —
essa escolha de relação NP é intencional: o objetivo é que a prova só exista
quando a assinatura de fato verifica, sem revelar um booleano de
validade/invalidade. Host program (`host/verify-mldsa-host`) gera os pares
de teste e orquestra proving/verificação.

### Pré-requisitos

- Rust (via [rustup](https://rustup.rs/))
- Toolchain do SP1: instalar via [`sp1up`](https://docs.succinct.xyz/docs/sp1/getting-started/install)
  (`curl -L https://sp1.succinct.xyz | bash && sp1up`), que traz o toolchain
  `succinct` usado por `circuits/verify-mldsa/rust-toolchain.toml`.
- `protoc` (Protocol Buffers), exigido pelo build de `sp1-prover-types`:
  `brew install protobuf` no macOS.

### Como rodar

```bash
# Demo rápida (1 caso válido + 1 inválido, imprime tempo de proving observado)
cargo run --release -p verify-mldsa-host

# Suite de testes do critério de sucesso do M1 (10+ pares, válidos e inválidos)
cargo test --release -p verify-mldsa-host --test verify_mldsa -- --nocapture
```

Tempo de proving observado (modo compressed, Apple M4, 26/08/2026, 1 assinatura
ML-DSA-44): **~50-70s por caso** — válido (prova + verificação): 67.04s;
inválido, rejeitado corretamente (prova + rejeição em `verify`): 49.95s.
Informal por design nesta fase — benchmark reproduzível formal é objetivo do M2.

**Nota de implementação:** no `sp1-sdk` 6.5.0, `prove()` sempre sucede —
a prova STARK atesta o que de fato aconteceu na execução, panic do guest
incluso. Quem rejeita é `verify(proof, vk, None)`, que por padrão exige
exit_code de sucesso. Por isso `prove_case` (`host/verify-mldsa-host/src/lib.rs`)
encadeia prove+verify — só descoberto rodando o pipeline de fato, não pela
documentação pública da API.

## Milestone M2 — harness de benchmark

Binário `host/verify-mldsa-host/src/bin/bench.rs`: roda o **mesmo** par
(chave, mensagem, assinatura) fixo em N=30 repetições, medindo proving e
verificação separadamente e o tamanho da prova serializada. Par fixo de
propósito — isola ruído de medição/sistema da variância entre inputs
diferentes. Hardware oficial: Apple M4, 10 cores, 16 GB RAM, macOS 26.5.2.

```bash
# N=30 x ~70-100s/repetição em modo compressed ≈ 35-50 minutos
cargo run --release -p verify-mldsa-host --bin bench
```

Resultados por repetição em [`docs/results/m2_benchmark.csv`](docs/results/m2_benchmark.csv).
Resumo da execução de 26-27/08/2026 (N=30, par fixo):

| Métrica | Média | Desvio padrão |
| --- | --- | --- |
| Proving | 79,32 s | 16,59 s |
| Verificação | 31,51 ms | 5,17 ms |
| Tamanho da prova | 1.273.934 bytes | 0 bytes (constante) |

O desvio padrão do proving é alto (~21% da média) apesar do input fixo —
as repetições 14-23 ficam visivelmente mais lentas (84-129s) que o resto
(64-68s), um padrão consistente com **throttling térmico** numa sequência
longa de proving em potência máxima, não com variação criptográfica (o
input não mudou). Tamanho de prova ficou perfeitamente constante entre as
30 repetições, confirmando a propriedade do modo compressed.

## Licença

MIT — ver [`LICENSE`](LICENSE).
