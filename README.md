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
Ethereum (Buterin, 2026), assinaturas de conta, buscando um mecanismo análogo
ao já anunciado para a Área 1 (leanXMSS/leanVM, compressão de 250x via
agregação SNARK), ainda inexistente para contas de usuário.

## Status

🚧 Em desenvolvimento. M1 (verificação individual), M2 (harness de
benchmark) e M3 (agregação por lote) concluídos. Próximo: M4 (composição
recursiva, opcional) ou M5 (custo de gas on-chain).

## Decisões técnicas em vigor

- **Modo de prova (M1-M4):** STARK "core"/"compressed" do SP1, sem wrapping
  para Groth16/PLONK sobre BN254. Preserva segurança pós-quântica de ponta a
  ponta enquanto nenhum milestone exige verificação on-chain. Decisão
  reaberta em M5.
- **Referência ML-DSA:** crate Rust pura `no_std`-compatível (`fips204`), não
  `pqcrypto-dilithium` (inviável no target `riscv32im` do guest SP1 por usar
  FFI sobre C) nem implementação própria do zero.
- **Hardware oficial de benchmark (M1-M4):** Apple M4, 10 cores, 16 GB RAM,
  macOS 26.5.2. Não se aplica ao M5, cujo custo de gas é definido pelo
  protocolo e independe de hardware.

## Estrutura do repositório

```
/circuits                       # guest programs SP1
  verify-mldsa/                 # M1+M3: verificação de k assinaturas ML-DSA-44 (k=1..N)
/host                           # host programs (geração/orquestração de provas)
  verify-mldsa-host/
    src/lib.rs                  # geração de casos/lotes + prove/verify (M1, M3)
    src/bin/bench.rs            # harness de benchmark k=1 (M2)
    src/bin/bench_m3.rs         # harness de benchmark parametrizado por k (M3)
    tests/verify_mldsa.rs       # suite de testes do M1
/contracts                      # Solidity (a partir de M5)
/docs
  results/                      # CSVs de benchmark por milestone (M2+)
```

O harness de benchmark fica dentro de `host/verify-mldsa-host/src/bin/`, e
não num diretório `/bench` separado, para reusar a lógica de geração de
casos e prove/verify já implementada em `lib.rs`, sem precisar de uma
dependência de path entre crates.

## Milestone M1: verificação de assinatura ML-DSA-44 individual

Guest program SP1 (`circuits/verify-mldsa`) que recebe `(pubkey, message,
signature)` e só permite gerar prova se a assinatura ML-DSA-44 for válida.
Essa escolha de relação NP é intencional: o objetivo é que a prova só
exista quando a assinatura de fato verifica, sem revelar um booleano de
validade. Host program (`host/verify-mldsa-host`) gera os pares de teste e
orquestra proving/verificação.

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

Tempo de proving observado (modo compressed, Apple M4, 26/08/2026, 1
assinatura ML-DSA-44): entre 50 e 70 segundos por caso. Caso válido (prova
mais verificação): 67,04s. Caso inválido, rejeitado corretamente (prova
mais rejeição em `verify`): 49,95s. Números informais nesta fase; o
benchmark reproduzível formal é objetivo do M2.

**Nota de implementação:** no `sp1-sdk` 6.5.0, `prove()` sempre sucede. A
prova STARK atesta o que de fato aconteceu na execução, incluindo um
eventual panic do guest. Quem rejeita é `verify(proof, vk, None)`, que por
padrão exige exit_code de sucesso. Por isso `prove_case`
(`host/verify-mldsa-host/src/lib.rs`) encadeia prove e verify: esse
comportamento não é evidente na documentação pública da API e precisa ser
tratado explicitamente no código.

## Milestone M2: harness de benchmark

Binário `host/verify-mldsa-host/src/bin/bench.rs`: roda o mesmo par (chave,
mensagem, assinatura) fixo em N=30 repetições, medindo proving e
verificação separadamente e o tamanho da prova serializada. O par é fixo de
propósito, para isolar ruído de medição/sistema da variância entre inputs
diferentes. Hardware oficial: Apple M4, 10 cores, 16 GB RAM, macOS 26.5.2.

```bash
# N=30 vezes ~70-100s por repetição em modo compressed, cerca de 35 a 50 minutos
cargo run --release -p verify-mldsa-host --bin bench
```

Resultados por repetição em [`docs/results/m2_benchmark.csv`](docs/results/m2_benchmark.csv).
Resumo da execução de 26-27/08/2026 (N=30, par fixo):

| Métrica | Média | Desvio padrão |
| --- | --- | --- |
| Proving | 79,32 s | 16,59 s |
| Verificação | 31,51 ms | 5,17 ms |
| Tamanho da prova | 1.273.934 bytes | 0 bytes (constante) |

O desvio padrão do proving é alto (cerca de 21% da média) apesar do input
fixo. As repetições 14 a 23 ficam visivelmente mais lentas (84 a 129s) que
o resto (64 a 68s), um padrão consistente com throttling térmico numa
sequência longa de proving em potência máxima, e não com variação
criptográfica, já que o input não mudou entre repetições. O tamanho da
prova ficou perfeitamente constante entre as 30 repetições, confirmando a
propriedade do modo compressed.

## Milestone M3: agregação por lote (batching)

O guest program (`circuits/verify-mldsa`) foi generalizado do M1 (k=1
fixo) para receber um `k` (`u32`) seguido de k triplos (pubkey, message,
signature), verificando todos dentro da mesma prova. O caso k=1 continua
funcionando de forma idêntica ao M1, confirmado por teste de regressão.
Decisão de design: um guest program parametrizado por k em tempo de
execução, em vez de um crate por valor de k, para evitar duplicação e
manter uma única relação NP para qualquer tamanho de lote. No host
(`lib.rs`), `valid_batch(k)` gera k pares chave/mensagem independentes
(contas diferentes assinando transações diferentes, o cenário real de
agregação); `prove_case` e `bench_case` passaram a receber `&[MlDsaCase]`
em vez de um caso único.

```bash
# cargo run --release -p verify-mldsa-host --bin bench_m3 -- <k> <n>
# Resultados acrescentados em docs/results/m3_benchmark_k<k>.csv
cargo run --release -p verify-mldsa-host --bin bench_m3 -- 2 10
```

### Resultados (Apple M4, 16 GB RAM, 12/09/2026)

| k | N | Proving (média ± desvio) | Verificação (média ± desvio) | Tamanho da prova |
| --- | --- | --- | --- | --- |
| 1 | 30 | 79,32 s ± 16,59 s | 31,51 ms ± 5,17 ms | 1.273.934 B |
| 2 | 10 | 91,16 s ± 3,83 s | 28,88 ms ± 1,93 ms | 1.275.221 B |
| 4 | 5 | 164,38 s ± 16,55 s | 30,15 ms ± 1,76 ms | 1.277.873 B |
| 8 | 0 | falha por falta de memória, nenhuma repetição completada | n/d | n/d |

Dados por repetição em [`docs/results/`](docs/results/) (`m3_benchmark_k2.csv`,
`m3_benchmark_k4.csv`; `m3_benchmark_k8.csv` só tem o cabeçalho, sem linhas).

### Achados

Verificação e tamanho da prova permanecem praticamente constantes com k,
que é a propriedade esperada do modo compressed e um resultado favorável à
tese de agregação: o custo de verificar um lote de k assinaturas não cresce
com k. Proving, por outro lado, cresce mais que linearmente com k (de k=1
para k=2, +15%; de k=2 para k=4, +80% nas médias acima).

O limite observado nesta máquina (16 GB de RAM) é de memória, não só de
tempo. k=8 falhou por falta de memória (processo morto pelo sistema
operacional) já na primeira tentativa, mesmo em processo recém-iniciado.
k=4 é o maior lote estável como prova única, mas repeti-lo várias vezes
dentro do mesmo processo de longa duração também esgota a memória
disponível: rodando N=10 no mesmo processo, as duas primeiras repetições
completaram normalmente (155,79s e 192,47s) e a terceira derrubou o
processo; rodando k=4 em processos novos e separados (N=1 de cada vez),
três execuções adicionais completaram sem problema (164,93s, 142,01s e
166,71s). Isso indica que a memória usada por `client.prove()` não é
totalmente liberada entre chamadas dentro do mesmo processo. Os cinco
valores de k=4 na tabela acima vêm dessa combinação de execuções (duas do
processo de longa duração, três de processos novos), registradas em
`m3_benchmark_k4.csv`. Por esse motivo, `bench_m3` passou a acrescentar ao
CSV em vez de sobrescrevê-lo a cada execução, permitindo compor resultados
de múltiplas invocações sem perder dados de execuções anteriores.

O comportamento assintótico do M3 está documentado: a agregação, do jeito
como está implementada, escala mal em memória nesta configuração de
hardware. Rodar k em {8, 16, 32} exigiria mais RAM do que os 16 GB
disponíveis nesta máquina; ficaria para um hardware com mais memória, caso
isso seja revisitado.

## Licença

MIT. Ver [`LICENSE`](LICENSE).
