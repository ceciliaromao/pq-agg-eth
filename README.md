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
benchmark), M3 (agregação por lote), M4 (composição recursiva) e M5
(custo de gas on-chain) concluídos.

## Decisões técnicas em vigor

- **Modo de prova (M1-M4):** STARK "core"/"compressed" do SP1, sem wrapping
  para Groth16/PLONK sobre BN254. Preserva segurança pós-quântica de ponta a
  ponta enquanto nenhum milestone exige verificação on-chain.
- **Modo de prova (M5, verificação on-chain):** Groth16, com wrapping SNARK
  pairing-based sobre BN254. Não existe verificador Solidity para provas
  STARK "compressed" nativas, e implementar um verificador FRI em Solidity
  está fora do escopo desta fase. Essa escolha abre mão da segurança
  pós-quântica de ponta a ponta no último passo (o wrapping em si seria
  quebrado por um adversário quântico capaz de resolver o logaritmo
  discreto em curvas elípticas), uma limitação assumida conscientemente e
  documentada aqui, não descoberta a posteriori.
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
  aggregate-mldsa/              # M4: composição recursiva de n sub-provas
/host                           # host programs (geração/orquestração de provas)
  verify-mldsa-host/
    src/lib.rs                  # geração de casos/lotes + prove/verify + agregação (M1, M3, M4)
    src/bin/bench.rs            # harness de benchmark k=1 (M2)
    src/bin/bench_m3.rs         # harness de benchmark parametrizado por k (M3)
    src/bin/bench_m4.rs         # harness comparativo de composição recursiva (M4)
    src/bin/bench_m5.rs         # fixtures Groth16 de batching monolítico (M5)
    src/bin/bench_m5_agg.rs     # fixtures Groth16 de composição recursiva (M5)
    tests/verify_mldsa.rs       # suite de testes do M1
/contracts                      # projeto Foundry (M5)
  fixtures/                     # fixtures Groth16 geradas por bench_m5 (JSON)
  test/GasBenchmark.t.sol       # mede o gas de verifyProof por valor de k
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

O comportamento assintótico do M3 está documentado: a agregação por lote
monolítico, do jeito como está implementada, escala mal em memória nesta
configuração de hardware. Rodar k em {8, 16, 32} exigiria mais RAM do que
os 16 GB disponíveis nesta máquina, a não ser que se troque a abordagem.
O M4 investiga exatamente essa alternativa.

## Milestone M4: composição recursiva

Alternativa ao batching monolítico do M3: em vez de um único guest
verificando k assinaturas na mesma execução, cada assinatura é provada
independentemente (guest `verify-mldsa`, k=1, modo compressed), e um
segundo guest (`circuits/aggregate-mldsa`) verifica n dessas sub-provas
dentro da própria execução, produzindo uma prova final que só existe se
todas as n sub-provas forem válidas.

O mecanismo usa a verificação recursiva de provas do SP1: o host gera cada
sub-prova em modo compressed (obrigatório, é o único modo que pode ser
verificado recursivamente), e passa cada uma para o guest agregador via
`SP1Stdin::write_proof`, junto do digest da sua verifying key
(`HashableKey::hash_u32`) e dos seus valores públicos brutos. Dentro do
guest agregador, `sp1_zkvm::lib::verify::verify_sp1_proof(vk_digest,
pv_digest)` verifica cada sub-prova, recebendo o conteúdo da prova em si
automaticamente (casado por ordem de chamada com o que foi escrito via
`write_proof`), sem precisar lê-la explicitamente como entrada. O
`pv_digest` é o SHA-256 dos valores públicos brutos, calculado pelo
próprio guest agregador.

```bash
# cargo run --release -p verify-mldsa-host --bin bench_m4 -- <n>
cargo run --release -p verify-mldsa-host --bin bench_m4 -- 8
```

### Resultados (Apple M4, 16 GB RAM, 13/09/2026, uma execução por n)

| n | Sub-provas (tempo total) | Agregação (tempo) | Tamanho da prova agregada | Tempo total |
| --- | --- | --- | --- | --- |
| 2 | 131,00 s | 88,65 s | 1.275.217 B | 219,64 s |
| 8 | 525,65 s | 196,57 s | 1.283.161 B | 722,22 s |
| 16 | 1.084,65 s | 382,74 s | 1.293.759 B | 1.467,39 s |
| 32 | 1.944,74 s | 688,43 s | 1.314.959 B | 2.633,17 s |

Dados em [`docs/results/m4_benchmark.csv`](docs/results/m4_benchmark.csv).

### Achados

A composição recursiva completou n=32 sem falha de memória, o valor mais
alto que a spec original previa e que o M3 não conseguiu alcançar (M3
falhou em k=8). Isso confirma a hipótese que motivou o M4: como cada
sub-prova é gerada num processo independente com footprint de memória
fixo (equivalente ao k=1 do M3), a etapa de agregação em si opera sobre
provas já comprimidas e de tamanho pequeno, não sobre o traço de execução
bruto de n verificações ML-DSA simultâneas. O teto de memória do M3 é uma
limitação do batching monolítico, não da agregação em si.

O tamanho da prova final cresce muito pouco com n (de 1.275.217 B em n=2
para 1.314.959 B em n=32, cerca de 40 B por assinatura adicional
agregada), reforçando a propriedade de tamanho quase constante já
observada no M2 e no M3. O tempo da etapa de agregação cresce de forma
aproximadamente linear com n (de 88,65 s em n=2 a 688,43 s em n=32, uma
taxa marginal estável de 19 a 23 s por sub-prova adicional), um
comportamento bem mais previsível que o crescimento super-linear do
proving monolítico observado no M3.

O custo é tempo total de parede: para o mesmo n, a soma de n provas
independentes mais a agregação é significativamente maior que o batching
monolítico equivalente (quando este último não esbarra no limite de
memória). Em compensação, a geração das n sub-provas é independente por
natureza: pode ser paralelizada entre processos ou máquinas diferentes,
o que o batching monolítico do M3 não permite.

Resultados de uma única execução por valor de n, dado o custo de tempo
(quase 44 minutos só para n=32). Consistente com o critério de sucesso do
M4: resultado comparativo documentado, neste caso favorável à composição
recursiva como forma de contornar o limite de memória do M3, ao custo de
mais tempo total de execução.

## Milestone M5: custo de gas on-chain

Estimativa do custo de verificação em ambiente EVM simulado (Foundry),
para os mesmos valores de k avaliados no M3 (k=1, 2, 4). Ver a decisão de
modo de prova acima: este milestone usa Groth16 em vez de STARK
compressed, a única opção com verificador Solidity pronto no SP1.

### Pré-requisitos adicionais

- [Foundry](https://getfoundry.sh/) (`forge`, `anvil`, `cast`).
- Go (`go build`), para compilar o wrapper Groth16 (gnark) localmente via
  a feature `native-gnark` do `sp1-sdk`. Necessário porque a imagem Docker
  oficial do SP1 para o wrapping Groth16 só publica build para amd64 e
  falha em Apple Silicon (arm64).
- Dependências do Foundry, instaladas uma vez:
  ```bash
  cd contracts
  forge install foundry-rs/forge-std --no-git
  forge install succinctlabs/sp1-contracts --no-git
  ```

### Como rodar

```bash
# Gera a fixture Groth16 para um valor de k (cada um demora de 6 a 8 minutos)
cargo run --release -p verify-mldsa-host --bin bench_m5 -- 1
cargo run --release -p verify-mldsa-host --bin bench_m5 -- 2
cargo run --release -p verify-mldsa-host --bin bench_m5 -- 4

# Extensão: Groth16 sobre a composição recursiva do M4, para n=8/16/32
# (17 a 52 minutos cada, a maior parte gerando as n sub-provas)
cargo run --release -p verify-mldsa-host --bin bench_m5_agg -- 8
cargo run --release -p verify-mldsa-host --bin bench_m5_agg -- 16
cargo run --release -p verify-mldsa-host --bin bench_m5_agg -- 32

# Mede o gas de verifyProof para cada fixture gerada
cd contracts && forge test -vv
```

### Resultados (Apple M4, 16 GB RAM, 14/09/2026)

| k | Tempo de proving Groth16 | Encoding on-chain | Gas de `verifyProof` |
| --- | --- | --- | --- |
| 1 | 374,82 s | 356 bytes | 235.308 |
| 2 | 404,03 s | 356 bytes | 239.132 |
| 4 | 455,96 s | 356 bytes | 246.893 |

Fixtures em [`contracts/fixtures/`](contracts/fixtures/).

### Achados

O encoding on-chain da prova é exatamente 356 bytes para todo k testado,
a propriedade central do Groth16: tamanho de prova constante,
independente do que foi computado dentro do circuito. O custo de gas
segue o mesmo padrão, crescendo muito pouco com k (k=1 para k=2: +1,6%;
k=2 para k=4: +3,2%), consistente com o crescimento quase nulo do
tamanho da prova já observado no M2, M3 e M4. O aumento residual vem do
tamanho dos valores públicos (pubkey e mensagem de cada assinatura do
lote, que crescem com k) e não do custo criptográfico da verificação em
si, que é fixo.

Comparado a Kiraz e Kardas (2026), que reportam entre 390 e 670 mil gas
por verificação de migração de uma única conta, os números aqui (235 a
247 mil gas para lotes de 1 a 4 assinaturas numa única prova) ficam
abaixo mesmo do extremo inferior daquela faixa, apesar de cobrirem mais
de uma assinatura por verificação. Critério de sucesso do M5 atendido:
números de gas obtidos e comparáveis à literatura.

O tempo de proving Groth16 (375 a 456 segundos) é maior que o modo
compressed usado em M1-M4 para o mesmo k, porque inclui uma etapa
adicional de wrapping (redução da prova STARK para uma prova SNARK sobre
BN254) além da prova STARK de base.

### Extensão: gas para composição recursiva (n=8, 16, 32)

O batching monolítico do M3 não alcançou k=8 por falta de memória (ver
M3). A composição recursiva do M4 alcançou n=32 sem esse problema, então
faz sentido perguntar qual o custo de gas de verificar on-chain uma
dessas provas agregadas. Reaproveitando `prove_sub_proofs` e o guest
`aggregate-mldsa` do M4, `bench_m5_agg` gera as n sub-provas em modo
compressed e envolve só a prova final da agregação em Groth16.

| n (agregado) | Sub-provas + agregação | Encoding on-chain | Tamanho dos valores públicos | Gas de `verifyProof` |
| --- | --- | --- | --- | --- |
| 8 | 1.042,32 s | 356 bytes | 10.592 bytes | 262.401 |
| 16 | 1.664,08 s | 356 bytes | 21.190 bytes | 293.928 |
| 32 | 3.107,41 s | 356 bytes | 42.390 bytes | 358.077 |

O encoding da prova continua fixo em 356 bytes, mas o gas cresce mais
que nos casos de batching monolítico (k=4 para n=8: +6,3%; n=8 para
n=16: +12,0%; n=16 para n=32: +21,8%). A causa não é a verificação
criptográfica em si, que continua com custo fixo, e sim o tamanho dos
valores públicos: o guest agregador repassa a chave pública e a
mensagem de cada uma das n sub-provas como saída pública própria (ver
M4), então esse blob cresce linearmente com n (cerca de 1.325 bytes por
assinatura agregada). Descontando o custo fixo de 235.308 gas (o mesmo
de k=1), o gas incremental por byte de valores públicos fica estável
entre 2,9 e 3,0 gas/byte nos três valores de n testados, consistente
com o custo de calldata e do hash SHA-256 dos valores públicos feito
por `hashPublicValues` no verificador, não com o custo do emparelhamento
Groth16.

Isso aponta um limite prático do design atual, não da técnica de
recursão em si. Repassar a chave pública e a mensagem de cada assinatura
como valores públicos brutos é o motivo direto do crescimento linear de
gas com n. Um próximo passo natural seria comprometer esses dados dentro
do próprio circuito agregador (por exemplo, numa raiz de Merkle), em vez
de expô-los diretamente, o que tornaria o custo de gas verdadeiramente
constante em relação a n. Fica registrado aqui como observação para
trabalho futuro, fora do escopo desta fase.

## Licença

MIT. Ver [`LICENSE`](LICENSE).
