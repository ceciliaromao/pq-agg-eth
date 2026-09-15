// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console} from "forge-std/Test.sol";
import {SP1Verifier} from "sp1-contracts/v6.1.0/SP1VerifierGroth16.sol";

/// Mede o custo de gas de `SP1Verifier.verifyProof` para as provas Groth16
/// geradas pelo host (M5).
///
/// Casos de batching monolítico (host/verify-mldsa-host/src/bin/bench_m5.rs),
/// para os mesmos valores de k avaliados no M3: fixtures/groth16_k<k>.json.
///
/// Casos de composição recursiva (bin/bench_m5_agg.rs), para os valores de
/// n que o batching monolítico não alcançou no M3 (n=8, 16, 32, ver M4):
/// fixtures/groth16_agg_n<n>.json.
///
/// Casos de composição recursiva com raiz de Merkle em vez de valores
/// públicos brutos (bin/bench_m5_merkle.rs), investigando se isso torna o
/// gas constante em relação a n: fixtures/groth16_merkle_n<n>.json.
///
/// As fixtures precisam existir antes de rodar este teste.
contract GasBenchmarkTest is Test {
    SP1Verifier internal verifier;

    function setUp() public {
        verifier = new SP1Verifier();
    }

    function _runFixture(string memory filename, string memory label, uint256 value) internal {
        string memory path = string.concat("fixtures/", filename);
        string memory json = vm.readFile(path);

        bytes32 vkey = vm.parseJsonBytes32(json, ".vkey");
        bytes memory publicValues = vm.parseJsonBytes(json, ".publicValues");
        bytes memory proof = vm.parseJsonBytes(json, ".proof");

        uint256 gasBefore = gasleft();
        verifier.verifyProof(vkey, publicValues, proof);
        uint256 gasUsed = gasBefore - gasleft();

        console.log(label, value, "gas =", gasUsed);
    }

    function test_GasK1() public {
        _runFixture("groth16_k1.json", "k =", 1);
    }

    function test_GasK2() public {
        _runFixture("groth16_k2.json", "k =", 2);
    }

    function test_GasK4() public {
        _runFixture("groth16_k4.json", "k =", 4);
    }

    function test_GasAggN8() public {
        _runFixture("groth16_agg_n8.json", "n (agregado) =", 8);
    }

    function test_GasAggN16() public {
        _runFixture("groth16_agg_n16.json", "n (agregado) =", 16);
    }

    function test_GasAggN32() public {
        _runFixture("groth16_agg_n32.json", "n (agregado) =", 32);
    }

    function test_GasMerkleN8() public {
        _runFixture("groth16_merkle_n8.json", "n (Merkle) =", 8);
    }

    function test_GasMerkleN32() public {
        _runFixture("groth16_merkle_n32.json", "n (Merkle) =", 32);
    }
}
