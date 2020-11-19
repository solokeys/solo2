
# Running tests

```bash
cargo test --features logging/std,clients-1 --target $(rustc -Vv | awk 'NR==5{print $2}')
```
