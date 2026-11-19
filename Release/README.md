# AUTOCOMP Release v1.0.0

> 让编译器学会思考

This directory is the in-repository fallback release artifact for platforms where GitHub Releases cannot be created. It contains the release manifest, checksums, and source snapshot instructions for AUTOCOMP v1.0.0.

## Release contents

- `RELEASE_MANIFEST.json` — version, author, features, verification summary, and source snapshot metadata
- `SOURCE_ARCHIVE_SHA256.txt` — SHA-256 of the source archive included in this release folder
- `autocomp-v1.0.0-source.tar.gz` — reproducible source snapshot of the release commit
- `README.md` — this release directory guide

## Build from source

```bash
tar -xzf autocomp-v1.0.0-source.tar.gz
cd autocomp-v1.0.0-source
cargo test --workspace
cargo build --release -p autocomp-cli
./target/release/autocomp help
```

## Verify archive integrity

```bash
sha256sum -c SOURCE_ARCHIVE_SHA256.txt
```

## Notes

The current snapshot is an M1 runnable compiler skeleton. It provides IR parsing and validation, profile text input/output, pass pipeline, automated PGO flow, HTML/JSON reports, verification checklist, and CLI commands. It does not yet implement LLVM-backed native machine-code generation; report percentages are heuristic confidence values, not measured performance gains.
