<p align="center">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://abchprod.wpengine.com/wp-content/uploads/2024/05/Trident-Github.png?raw=true">
      <img alt="Trident Github" src="https://abchprod.wpengine.com/wp-content/uploads/2024/05/Trident-Github.png?raw=true" width="auto">
    </picture>
  </a>
</p>

<p align="left">
  <img height="100" width="100" src="https://abchprod.wpengine.com/wp-content/uploads/2024/05/Trident-Color.png" alt="Trident"/>


# Trident SVM

An in-process Solana Virtual Machine execution environment built on Agave's `TransactionBatchProcessor`. Trident SVM enables fast, lightweight processing of Solana transactions without running a full validator or test-validator node.

Used by [Trident](https://github.com/Ackee-Blockchain/trident) for fuzz testing and integration testing of Solana programs.

## What It Does

Trident SVM provides a self-contained SVM runtime with:

- **Account Management** — Three-tier account storage (temporary, permanent, sysvars) with automatic state settlement after transaction execution.
- **Full Sysvar Support** — Clock, Rent, EpochSchedule, SlotHashes, SlotHistory, StakeHistory, Fees, RecentBlockhashes, and EpochRewards — initialized at startup with real-time clock tracking.
- **Builtin Programs** — All Agave builtins (System Program, BPF Loader, Compute Budget, etc.) registered and ready to use.
- **Precompile Support** — Ed25519, Secp256k1, and Secp256r1 signature verification precompiles.
- **Embedded SPL Programs** — SPL Token, Token-2022, Associated Token Program, Metaplex Token Metadata, Metaplex Candy Machine V3, SPL Stake Pool, and Chainlink Oracle — loaded from mainnet binaries at initialization.
- **Native Program Entrypoints** — Deploy Rust-native program entrypoints directly into the SVM via the `syscall-v2` feature, bypassing BPF compilation for faster iteration.
- **Builder Pattern** — Configurable initialization with custom accounts, program entrypoints, and logging options.

## Usage

Add this dependency to your `Cargo.toml`:

```toml
[dependencies]
trident-svm = "0.2.0"
```

or from git:

```toml
[dependencies.trident-svm]
git = "https://github.com/Ackee-Blockchain/trident-svm"
```

## License

See [LICENSE](./LICENSE) for details.
