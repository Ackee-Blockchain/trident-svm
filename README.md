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

Implementation of Trident SVM allowing for fast processing of Solana Transactions.

Used by [Trident](https://github.com/Ackee-Blockchain/trident) to process Solana Transactions.


## Usage

Add this dependency to your `Cargo.toml`:


```toml
[dependencies]
trident-svm = "0.0.1"
```

or

```toml
[dependencies.trident-svm]
git = "https://github.com/Ackee-Blockchain/trident-svm"
```

> [!NOTE]
> Trident SVM optionally sets syscall stubs for solana 1.18 and 2.0:
> - [StubsV1](https://github.com/Ackee-Blockchain/trident-syscall-stubs)
> - [StubsV2](https://github.com/Ackee-Blockchain/trident-syscall-stubs)
