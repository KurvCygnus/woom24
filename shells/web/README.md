# shells/web — woom24's wasm shell

`room-shell-web` is the platform shell that plugs the room engine into the browser (cdylib, `wasm32-unknown-unknown`): wasm-bindgen exports + an in-memory VFS CRT shim (the `woom24-libc` declaration layer under `crt/`) + a Web Audio backend + Canvas2D/WebGL2 presentation. The one-shot build and static-page generation script is `shells/web/scripts/build-www.sh` (landed in Task 8); until then, build with `cargo build -p room-shell-web --target wasm32-unknown-unknown --release` and generate the glue with `wasm-bindgen`.
