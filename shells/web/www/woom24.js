// woom24 static loader: rAF drives woom24_tick, DOM keyboard -> woom24_push_key.
// Self-contained: imports only the wasm-bindgen glue generated into ./pkg; zero
// runtime network requests (the wasm loads from the same directory via a
// relative path; file:// direct open is handled by the later single-file work).
import init, {
  woom24_version,
  woom24_minimal_start,
  woom24_standard_start,
  woom24_register_file,
  woom24_attach_canvas,
  woom24_push_key,
  woom24_tick,
} from "./pkg/room_shell_web.js";

const status = document.getElementById("status");
const canvas = document.getElementById("screen");

// Keyboard map: DOM code -> Doom key byte (this table is the wasm shell's
// platform mapping, the peer of the native shell's keys table; the D2 contract
// requires already-mapped bytes to be pushed).
const DOM2DOOM = {
  Enter: 13, NumpadEnter: 13, Escape: 27, ArrowLeft: 0xac, ArrowRight: 0xae,
  ArrowUp: 0xad, ArrowDown: 0xaf, ControlLeft: 0xa3, ControlRight: 0xa3,
  Space: 0xa2, ShiftLeft: 0x80 + 0x36, ShiftRight: 0x80 + 0x36,
  AltLeft: 0x80 + 0x38, AltRight: 0x80 + 0x38, Tab: 9, Backspace: 0x7f,
  Pause: 0xff, Equal: 61, Minus: 45,
  F1: 0x80 + 0x3b, F2: 0x80 + 0x3c, F3: 0x80 + 0x3d, F4: 0x80 + 0x3e,
  F5: 0x80 + 0x3f, F6: 0x80 + 0x40, F7: 0x80 + 0x41, F8: 0x80 + 0x42,
  F9: 0x80 + 0x43, F10: 0x80 + 0x44, F11: 0x80 + 0x57, F12: 0x80 + 0x58,
};

function doomKeyFor(code) {
  if (DOM2DOOM[code] !== undefined) return DOM2DOOM[code];
  if (code.startsWith("Key") && code.length === 4) {
    return code.charCodeAt(3) | 0x20; // lowercase ASCII
  }
  if (code.startsWith("Digit") && code.length === 6) {
    return code.charCodeAt(5);
  }
  return null;
}

// One rAF chain only: a refused second start (the engine's CREATED latch
// reports to the console) must never start a second loop, or woom24_tick
// would run ~2x per frame and the simulation would run at double speed.
let loopRunning = false;
function startLoop() {
  if (loopRunning) return;
  loopRunning = true;
  requestAnimationFrame(loop);
}

function loop() {
  woom24_tick();
  requestAnimationFrame(loop);
}

try {
  await init();
  status.textContent = "wasm ok (v" + woom24_version() + ")";
  woom24_attach_canvas(canvas);

  // Keyboard listeners are registered only once the module is instantiated:
  // the handlers call into wasm, so a keypress during or after a failed
  // init must not throw inside the listener.
  window.addEventListener("keydown", (e) => {
    const k = doomKeyFor(e.code);
    if (k !== null) { e.preventDefault(); woom24_push_key(true, k); }
  });
  window.addEventListener("keyup", (e) => {
    const k = doomKeyFor(e.code);
    if (k !== null) { e.preventDefault(); woom24_push_key(false, k); }
  });

  // Demo flow A (minimal entry): pick a local IWAD -> config panel.
  document.addEventListener("iwad-picked", async (ev) => {
    const bytes = new Uint8Array(await ev.detail.arrayBuffer());
    woom24_minimal_start(1080, ev.detail.name, bytes);
    startLoop();
  });

  // Demo flow B (standard entry): console call --
  //   woom24_demo_standard(<File object of a .wad>)
  window.woom24_demo_standard = async (file) => {
    const bytes = new Uint8Array(await file.arrayBuffer());
    woom24_register_file(file.name, bytes);
    woom24_standard_start(JSON.stringify({
      iwad: file.name, pwads: [], sf2: null, maxRenderRes: 1080, engineArgs: [],
    }));
    startLoop();
  };

  // The page gets an IWAD picker (flow A's trigger).
  const pick = document.createElement("input");
  pick.type = "file"; pick.accept = ".wad";
  pick.onchange = () => {
    if (pick.files[0]) {
      document.dispatchEvent(new CustomEvent("iwad-picked", { detail: pick.files[0] }));
    }
  };
  document.body.appendChild(pick);
  status.textContent = "Select a local IWAD file to start (or call woom24_demo_standard(file) from the console)";
} catch (e) {
  status.textContent = "wasm load failed: " + e;
  console.error(e);
}
