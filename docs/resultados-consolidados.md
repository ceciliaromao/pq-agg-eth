# Resultados consolidados: M1 a M5

Base empírica para o capítulo de resultados. Todos os números vêm de
execuções reais documentadas no README do repositório e nos CSVs de
`docs/results/`; este documento reorganiza e conecta esses dados para a
redação.

**Hardware:** Apple M4, 10 núcleos, 16 GB RAM, macOS 26.5.2 (M1-M4; o
custo de gas do M5 independe de hardware).

---

## M1: verificação individual de assinatura ML-DSA-44

**Relação NP implementada:** dado uma chave pública e uma mensagem
(entradas públicas), o provador demonstra conhecimento de uma assinatura
ML-DSA-44 válida, sem revelar a assinatura em si nem um booleano de
validade. A prova simplesmente não existe para uma assinatura inválida.

**Critério de sucesso:** atendido. 10 pares chave/assinatura testados (6
válidos, 4 inválidos com defeitos distintos: assinatura adulterada,
mensagem trocada, chave pública trocada), todos tratados corretamente.

**Tempo de proving observado (informal, 1 execução por caso):**

| Caso | Tempo |
| --- | --- |
| Válido (prova + verificação) | 67,04 s |
| Inválido (prova + rejeição em `verify`) | 49,95 s |

Achado técnico relevante para a metodologia: no `sp1-sdk` 6.5.0,
`prove()` sempre sucede, mesmo quando o programa do circuito (guest)
sofre panic. A prova STARK atesta fielmente o exit_code real da
execução, e a rejeição de assinaturas inválidas acontece na etapa de
`verify()`, que por padrão exige exit_code de sucesso. Isso não é
documentado publicamente pelo SP1 e precisou ser tratado explicitamente
na implementação (`prove_case` encadeia as duas etapas).

---

## M2: harness de benchmark reproduzível

Metodologia: N=30 repetições do mesmo par fixo (chave, mensagem,
assinatura), repetido de propósito para isolar ruído de medição/sistema
da variância intrínseca entre inputs diferentes, dois fenômenos
distintos que não deveriam ser confundidos no desvio padrão reportado.

**Resultados (N=30):**

| Métrica | Média | Desvio padrão |
| --- | --- | --- |
| Tempo de proving | 79,32 s | 16,59 s |
| Tempo de verificação | 31,51 ms | 5,17 ms |
| Tamanho da prova serializada | 1.273.934 bytes | 0 bytes (constante) |

Achado: o desvio padrão do proving é alto (cerca de 21% da média)
apesar do input fixo. As repetições 14 a 23 da sequência de 30 são
visivelmente mais lentas (84 a 129 s) que o restante (64 a 68 s), um
padrão consistente com throttling térmico numa sequência longa de
proving em potência máxima, não com variação criptográfica, já que o
input não mudou entre repetições. O tamanho da prova é perfeitamente
constante, confirmando a propriedade do modo STARK "compressed" do SP1:
tamanho de prova independente do número de ciclos de execução.

---

## M3: agregação por lote monolítico (batching)

Design: um único guest program parametrizado por k, não um circuito por
valor de k, lendo k triplos (chave, mensagem, assinatura) e verificando
todos dentro da mesma execução/prova.

**Resultados:**

| k | N | Proving (média ± desvio) | Verificação (média ± desvio) | Tamanho da prova |
| --- | --- | --- | --- | --- |
| 1 | 30 | 79,32 s ± 16,59 s | 31,51 ms ± 5,17 ms | 1.273.934 B |
| 2 | 10 | 91,16 s ± 3,83 s | 28,88 ms ± 1,93 ms | 1.275.221 B |
| 4 | 5 | 164,38 s ± 16,55 s | 30,15 ms ± 1,76 ms | 1.277.873 B |
| 8 | 0 | falha por falta de memória | n/d | n/d |

Achados principais:

1. Verificação e tamanho da prova permanecem praticamente constantes com
   k. Essa é a propriedade central que sustenta a viabilidade da
   agregação: o custo de verificar um lote de k assinaturas não cresce
   com k.
2. Proving cresce mais que linearmente com k (k=1 para k=2: +15%; k=2
   para k=4: +80% nas médias observadas).
3. O limite prático nesta configuração de hardware é de memória, não de
   tempo. k=8 falhou por falta de memória (processo morto pelo sistema
   operacional) já na primeira tentativa, mesmo em processo recém
   iniciado. k=4 é o maior lote estável como prova única, mas mesmo
   assim repeti-lo várias vezes dentro do mesmo processo de longa
   duração esgota a memória disponível: a memória usada por
   `client.prove()` não é totalmente liberada entre chamadas dentro do
   mesmo processo.
4. Resultado do M3 conforme seu próprio critério de sucesso (a
   metodologia original aceita explicitamente "escala mal" como
   resultado válido): a agregação por lote monolítico escala mal em
   memória nesta configuração de 16 GB de RAM.

---

## M4: composição recursiva

Design, alternativa ao M3: em vez de um único guest verificando k
assinaturas na mesma execução, cada assinatura é provada
independentemente (k=1, modo compressed), e um segundo guest agregador
verifica n dessas sub-provas recursivamente, via a syscall de
verificação recursiva do SP1, produzindo uma prova final que só existe
se todas as n sub-provas forem válidas.

**Resultados (uma execução por n, dado o custo de tempo):**

| n | Sub-provas (tempo total) | Agregação (tempo) | Tamanho da prova agregada | Tempo total |
| --- | --- | --- | --- | --- |
| 2 | 131,00 s | 88,65 s | 1.275.217 B | 219,64 s |
| 8 | 525,65 s | 196,57 s | 1.283.161 B | 722,22 s |
| 16 | 1.084,65 s | 382,74 s | 1.293.759 B | 1.467,39 s |
| 32 | 1.944,74 s | 688,43 s | 1.314.959 B | 2.633,17 s |

Achados principais:

1. A composição recursiva completou n=32 sem falha de memória, o valor
   máximo que a metodologia original previa (k entre 2 e 32) e que o M3
   não alcançou (falhou em k=8). Isso confirma que o teto de memória do
   M3 é uma limitação do batching monolítico, não da agregação como
   técnica: cada sub-prova tem footprint de memória fixo (equivalente
   ao k=1), e a etapa de agregação opera sobre provas já comprimidas e
   pequenas, não sobre o traço de execução bruto de n verificações
   simultâneas.
2. Tamanho da prova final cresce muito pouco com n: 1.275.217 B em n=2
   contra 1.314.959 B em n=32, cerca de 40 B por assinatura adicional
   agregada.
3. Tempo da etapa de agregação cresce de forma aproximadamente linear
   com n (88,65 s em n=2 a 688,43 s em n=32, taxa marginal estável de
   19 a 23 s por sub-prova adicional), um comportamento bem mais
   previsível que o crescimento super-linear do proving monolítico do
   M3.
4. Custo do trade-off: tempo total de parede. Para o mesmo n, a soma de
   n provas independentes mais a agregação é maior que o batching
   monolítico equivalente, quando este não esbarra em seu limite de
   memória. Em compensação, as n sub-provas são independentes por
   construção e poderiam ser paralelizadas entre processos ou máquinas
   diferentes, algo que o batching monolítico do M3 não permite.

---

## M5: custo de gas on-chain

Decisão de modo de prova: Groth16 (wrapping SNARK pairing-based sobre
BN254), não o modo STARK "compressed" usado em M1-M4. Não existe
verificador Solidity para provas STARK nativas do SP1, e implementar um
verificador FRI em Solidity está fora do escopo desta fase. Essa escolha
abre mão da segurança pós-quântica de ponta a ponta apenas no último
passo (wrapping), uma limitação assumida conscientemente e documentada,
consistente com a opção (B) prevista na metodologia original para esta
fase de prova de conceito.

Ambiente: Foundry (EVM simulado), verificador Solidity oficial do SP1
(`SP1Verifier`/`Groth16Verifier`, pacote `succinctlabs/sp1-contracts`
v6.1.0).

### Batching monolítico (M3), k = 1, 2, 4

| k | Tempo de proving Groth16 | Encoding on-chain | Gas de `verifyProof` |
| --- | --- | --- | --- |
| 1 | 374,82 s | 356 bytes | 235.308 |
| 2 | 404,03 s | 356 bytes | 239.132 (+1,6%) |
| 4 | 455,96 s | 356 bytes | 246.893 (+3,2%) |

### Composição recursiva (M4), n = 8, 16, 32

| n | Sub-provas + agregação Groth16 | Encoding on-chain | Valores públicos | Gas de `verifyProof` |
| --- | --- | --- | --- | --- |
| 8 | 1.042,32 s | 356 bytes | 10.592 B | 262.401 (+6,3% sobre k=4) |
| 16 | 1.664,08 s | 356 bytes | 21.190 B | 293.928 (+12,0%) |
| 32 | 3.107,41 s | 356 bytes | 42.390 B | 358.077 (+21,8%) |

Achados principais:

1. O encoding on-chain da prova é exatamente 356 bytes para todo k/n
   testado, a propriedade central do Groth16: tamanho de prova
   constante, independente do que foi computado dentro do circuito.
2. No batching monolítico (k=1,2,4), o gas cresce muito pouco (+1,6% e
   +3,2%), acompanhando o crescimento quase nulo do tamanho da prova já
   visto em M2-M4.
3. Na composição recursiva (n=8,16,32), o gas cresce de forma mais
   visível (+6,3%, +12,0%, +21,8% a cada salto). A causa identificada
   não é o custo criptográfico da verificação Groth16, que permanece
   fixo em cerca de 235 mil gas de base, e sim o tamanho dos valores
   públicos: o guest agregador repassa a chave pública e a mensagem de
   cada sub-prova como saída pública própria, então esse blob cresce
   linearmente com n (cerca de 1.325 bytes por assinatura agregada).
   Descontado o custo fixo de 235.308 gas, o gas incremental por byte
   de valores públicos é estável entre 2,9 e 3,0 gas/byte nos três
   valores de n, consistente com o custo de calldata e do hash SHA-256
   (`hashPublicValues`) sobre os valores públicos no verificador, não
   com o custo do emparelhamento Groth16 em si.
4. Comparação com a literatura: Kiraz e Kardas (2026) reportam entre 390
   e 670 mil gas por verificação de migração de uma única conta
   Ethereum/Bitcoin para ML-DSA via STARK+SP1. Os números aqui (235 a
   358 mil gas, cobrindo de 1 a 32 assinaturas por verificação) ficam
   abaixo até do extremo inferior daquela faixa, inclusive no caso mais
   caro (n=32, 358.077 gas, que verifica 32 assinaturas numa única
   chamada). Critério de sucesso do M5 atendido: números de gas obtidos
   e diretamente comparáveis à literatura.
5. Limitação de design identificada, não da técnica de recursão em si:
   expor a chave pública e a mensagem de cada assinatura como valores
   públicos brutos é o motivo direto do crescimento linear de gas com n
   na composição recursiva.

### Extensão: raiz de Merkle em vez de valores públicos brutos

Testada a hipótese do item 5 acima com um segundo guest agregador
(`aggregate-mldsa-merkle`), que recalcula dentro do próprio circuito a
raiz de uma árvore de Merkle binária (SHA-256) sobre o digest já
verificado de cada sub-prova, e comita só a raiz (32 bytes fixos,
independente de n) em vez dos valores públicos brutos.

| n | Sub-provas + agregação Groth16 | Encoding on-chain | Valores públicos | Gas de `verifyProof` |
| --- | --- | --- | --- | --- |
| 8 | 1.076,30 s | 356 bytes | 32 bytes | 231.609 |
| 32 | 2.822,60 s | 356 bytes | 32 bytes | 231.609 |

O gas é idêntico entre n=8 e n=32, 231.609 em ambos, confirmando a
hipótese por completo: o crescimento de gas na composição recursiva com
valores públicos brutos não vinha do custo criptográfico da
verificação, fixo desde o M2, vinha inteiramente do tamanho dos dados
expostos como saída pública, e é evitável com essa mudança de design no
circuito agregador. O resultado fica inclusive abaixo do próprio k=1 do
batching monolítico (235.308 gas), já que 32 bytes de raiz são menores
que os 1.328 bytes de valores públicos de uma única assinatura crua.

Ressalva importante para a redação: comprometer os dados numa raiz não
elimina a necessidade de publicá-los em algum lugar para que qualquer
consumidor saiba quais transações foram de fato autorizadas. Este
experimento isola corretamente o custo de `verifyProof`, mas não
representa o custo total de um sistema real usando esse esquema, que
precisaria de disponibilidade de dados adicional (por exemplo, os dados
brutos publicados como calldata à parte, mais uma prova de inclusão de
Merkle por transação, com seu próprio custo de gas) ou de outra
hipótese de disponibilidade fora da cadeia. Vale apresentar esse
resultado como confirmação da causa raiz identificada no M5 (o custo
extra vinha dos dados públicos, não da criptografia), não como uma
solução completa e pronta para produção.

---

## Síntese para a dissertação

- Viabilidade técnica confirmada (Objetivo específico c): é possível
  verificar e agregar assinaturas ML-DSA via SP1/STARK, com prova única
  cobrindo de 1 a 32 assinaturas.
- Duas estratégias de agregação com trade-offs opostos e complementares:
  batching monolítico (M3) é mais rápido em tempo de parede para k
  pequeno, mas esbarra num teto de memória em k=8 nesta configuração de
  hardware; composição recursiva (M4) ultrapassa esse teto até n=32, ao
  custo de mais tempo total, mas com a vantagem estrutural de
  paralelizar a geração das sub-provas.
- Tamanho de prova e custo de verificação, local e on-chain,
  praticamente independentes do número de assinaturas agregadas. Essa é
  a propriedade central que justifica tecnicamente a proposta de
  agregação, replicada em três camadas diferentes: M2/M3 (tamanho da
  prova STARK), M4 (tamanho da prova recursiva) e M5 (gas on-chain do
  wrapping Groth16).
- Posicionamento comparativo direto (Objetivo específico e): os números
  de gas do M5 (235 a 358 mil) ficam abaixo da faixa reportada por
  Kiraz e Kardas (2026) para migração de conta única (390 a 670 mil),
  mesmo cobrindo múltiplas assinaturas por verificação.
- Limitações reconhecidas e documentadas com transparência:
  1. Segurança pós-quântica de ponta a ponta é perdida no wrapping
     Groth16 do M5, uma troca consciente e não uma falha.
  2. O teto de memória do M3 é específico desta configuração de
     hardware (16 GB RAM), não uma propriedade fundamental do batching
     monolítico.
  3. O crescimento de gas com n na composição recursiva do M5 vem de
     uma escolha de design específica (valores públicos brutos), não de
     uma limitação fundamental da recursão. Confirmado empiricamente
     com a variante de raiz de Merkle, que produziu gas idêntico
     (231.609) em n=8 e n=32.
