
# 🧠 STARK Verification on Polkadot using PolkaVM

## Overview

This project proves that it's now possible to build **end-to-end DApps** that:

>🧩 **Put complex computation in Rust contracts** (e.g., ZK proof verification, cryptography)
>⚙️ **Use Solidity to handle business logic, user interactions, and token flow**

Thanks to __PolkaVM__, which enables high-performance Rust smart contracts running in a `no_std` **RISC-V** environment, and **AssetHub**, which lets these contracts interoperate directly with Solidity.

As a practical demo, this project verifies a **STARK proof for a linear regression computation**, generated with **Winterfell**, directly on-chain.

## 🔥 What This Project Enables
- ✅ Complex logic like STARK verification in no_std Rust contracts

- ✅ Business logic, user access control, and payments via Solidity

- ✅ A fully integrated dual-runtime DApp using AssetHub

- ✅ On-chain **Merkle verification, Blake3 hashing, and custom finite field math**

- ✅ Real-world demo: verifying a linear regression STARK proof

## 📐 Use Case: Linear Regression STARK Proof
As a proof-of-concept, this project shows how you can:

- Train a linear regression model off-chain

- Prove that computation using a STARK proof

- Verify that proof entirely on-chain, in a RISC-V smart contract

- Let Solidity contracts trigger the verification and handle outcomes

- This approach is ideal for verifiable compute, ML inference, or ZK analytics.

```📦 Architecture
+---------------------------+           +------------------------------+
|   Solidity Contract       |  calls    |   Rust STARK Verifier        |
| (Business Logic Layer)    +---------->+ (RISC-V no_std Smart Contract)|
+---------------------------+           +------------------------------+
            |                                        |
            |              Runs on PolkaVM           |
            +----------------------------------------+
                       Deployed via AssetHub
```
## This architecture allows a clean separation of roles:

- Rust handles cryptographic computation and ZK logic

- Solidity manages DApp logic, users, tokens, and payouts

## 🧠 Using Winterfell
We use Winterfell off-chain to:

- Define AIR constraints for linear regression

- Generate a STARK proof

- Serialize proof and public inputs with bincode

- Winterfell's verifier cannot run on-chain due to std dependencies, so we re-implement the verifier manually in Rust inside a no_std contract.

## ⚠️ For this demo:

The public inputs are hardcoded into the Rust verifier contract(there were problems in deserializing using bincode as it did not support no_std environment. we have planned to implement a custom deserializer to fix this)

The proof is deserialized on-chain using Winterfell-compatible layout 

## 🧩 From Hardcoded to General-Purpose STARK Verifier
While this demo uses hardcoded public inputs for simplicity, once generalized deserialization is implemented in the Rust verifier contract (handling input formats):
✅ This framework can become a universal on-chain STARK verifier for any AIR program.

## That means you’ll be able to:

- Upload and verify any kind of STARK proof (ML, fraud proofs, off-chain analytics, etc.)

- Keep all business logic in Solidity — rewards, access, asset transfers, etc.

- Build full DApps where Rust handles heavy computation, and Solidity handles everything else

- This unlocks a huge range of use cases, including ZK gaming, verifiable oracles, proof-of-compute systems, and modular AI inference on-chain — all powered by PolkaVM + AssetHub.

## How to run: 
- clone the git repository 
```git clone https://github.com/agrawalx/On-chain-STARK-framework.git```

**🧾 1. Deploy the Rust Verifier Contract**
```
cd verifier 
RUST_ADDRESS=$(cast send --account dev-account --create "$(xxd -p -c 99999 contract.polkavm)" --json | jq -r .contractAddress)
```

**🛠️ 2. Generate the STARK Proof**
- Navigate to the generate_proof directory and add Winterfell as a dependency:
`cargo add winterfell`
- Then run the proof generation script in release mode:
`cargo run -release `
> This will print the serialized STARK proof and public inputs to the terminal. 


**✅ 3. deploy and Verify the Proof via Solidity Contract**
- deploy the solidity contract on a testnet 
- Use the generated proofBytes,publicInputBytes and rust contract address as arguments to the Solidity contract’s verify() function:

**the solidity contract internally calls rust contract and outputs if the proof is valid or not**

## Challenges we faced:
- The contract.polkavm file size comes to about 183.7 Kb. First we tried deploying it through cast but we faced Arguement too long. Then we tried deploying it       using JS script in which we faced  error: { code: -32003, message: 'max initcode size exceeded' }

