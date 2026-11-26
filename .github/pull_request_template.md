## What and why

## Checked
- [ ] `pnpm check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` (launcher, aod-nup)
- [ ] `python tools/check_content.py --staged` (no game material, keys or build output)
- [ ] Engine changes are made in `engine/source/` (generator or runtime), never in generated C
- [ ] Tested in game (say how), or not needed
