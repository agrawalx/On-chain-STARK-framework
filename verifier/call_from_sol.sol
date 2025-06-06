// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.0;

interface IRustVerifier {
    function call(bytes calldata input) external returns (bytes memory);
}

contract StarkProofVerifier {
    function verifyProof(
        bytes calldata proof,
        bytes calldata publicInputs,
        address rustContractAddress
    ) external returns (bool success) {
        IRustVerifier rust = IRustVerifier(rustContractAddress);

        uint32 proofLen = uint32(proof.length);
        uint32 inputLen = uint32(publicInputs.length);

        // ABI expects: [proof_len (32 bytes)][input_len (32 bytes)][proof][publicInputs]
        bytes memory inputData = abi.encodePacked(
            bytes32(uint256(proofLen)),         // proof_len padded to 32 bytes
            bytes32(uint256(inputLen)),         // input_len padded to 32 bytes
            proof,
            publicInputs
        );

        // Call the Rust contract's exported `call()` entry
        bytes memory result = rust.call(inputData);

        // Return value is 32 bytes; last byte is the result (0 or 1)
        require(result.length == 32, "Unexpected output length");
        return result[31] != 0;
    }
}

