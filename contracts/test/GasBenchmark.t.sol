// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console} from "forge-std/Test.sol";
import {SP1Verifier} from "sp1-contracts/v6.1.0/SP1VerifierGroth16.sol";

/// Mede o custo de gas de `SP1Verifier.verifyProof` para as provas Groth16
/// geradas pelo host (host/verify-mldsa-host/src/bin/bench_m5.rs), nos
/// mesmos valores de k avaliados no M3.
///
/// As fixtures em fixtures/groth16_k<k>.json precisam existir antes de
/// rodar este teste (gerar com `cargo run -p verify-mldsa-host --bin
/// bench_m5 -- <k>`).
contract GasBenchmarkTest is Test {
    SP1Verifier internal verifier;

    function setUp() public {
        verifier = new SP1Verifier();
    }

    function _runFixture(uint256 k) internal {
        string memory path = string.concat("fixtures/groth16_k", vm.toString(k), ".json");
        string memory json = vm.readFile(path);

        bytes32 vkey = vm.parseJsonBytes32(json, ".vkey");
        bytes memory publicValues = vm.parseJsonBytes(json, ".publicValues");
        bytes memory proof = vm.parseJsonBytes(json, ".proof");

        uint256 gasBefore = gasleft();
        verifier.verifyProof(vkey, publicValues, proof);
        uint256 gasUsed = gasBefore - gasleft();

        console.log("k =", k, "gas =", gasUsed);
    }

    function test_GasK1() public {
        _runFixture(1);
    }

    function test_GasK2() public {
        _runFixture(2);
    }

    function test_GasK4() public {
        _runFixture(4);
    }
}
