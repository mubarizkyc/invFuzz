## To run 

```bash
# Install Crucible CLI 
git clone https://github.com/asymmetric-research/crucible ~/src/crucible
cargo install --path ~/src/crucible/crates/crucible-fuzz-cli
# Build the program 
cd examples/escrow-demo
cargo build-sbf programs/escrow/Cargo.toml
#Fuzz for all invariants
crucible run escrow_fuzz invariant_acceptance --release --stop-on-crash
```

