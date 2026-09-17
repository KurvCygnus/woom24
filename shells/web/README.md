# shells/web — woom24 的 wasm shell

`room-shell-web` 是把 room 引擎接入浏览器的平台外壳 (cdylib, `wasm32-unknown-unknown`): wasm-bindgen 导出 + 内存 VFS CRT 垫片 (`crt/` 下的 `woom24-libc` 声明层) + Web Audio 后端 + Canvas2D/WebGL2 呈现. 一键构建与静态页生成脚本为 `shells/web/scripts/build-www.sh` (Task 8 落地); 在此之前用 `cargo build -p room-shell-web --target wasm32-unknown-unknown --release` 加 `wasm-bindgen` 生成胶水.
