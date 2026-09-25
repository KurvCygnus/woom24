// div-probe tool: scan the wasm code section for i32.const constants that fall
// inside ticdup's .bss neighborhood window, and report the enclosing function
// names via the wasm "name" custom section.
// Usage: node tools/scan-addr-refs.mjs <wasm> <loHex> <hiHex>
import fs from "node:fs";

const [file, loHex, hiHex] = process.argv.slice(2);
const buf = fs.readFileSync(file);
const lo = parseInt(loHex, 16), hi = parseInt(hiHex, 16);

let p = 8;
function u32() { let r = 0, s = 0, b; do { b = buf[p++]; r |= (b & 0x7f) << s; s += 7; } while (b & 0x80); return r >>> 0; }
function peekU32(at) { let r = 0, s = 0, b, q = at; do { b = buf[q++]; r |= (b & 0x7f) << s; s += 7; } while (b & 0x80); return r >>> 0; }

const sections = [];
while (p < buf.length)
{
    const id = buf[p++]; const size = u32(); const start = p;
    sections.push({ id, start, size });
    p = start + size;
}

// name section: custom (id 0), name "name"
let funcNames = new Map();
for (const s of sections)
{
    if (s.id !== 0) continue;
    const nameLen = peekU32(s.start);
    // read name
    let q = s.start; const nl = u32.call ? 0 : 0; // placeholder
    // re-parse properly
    q = s.start;
    let n = 0, sh = 0, b;
    do { b = buf[q++]; n |= (b & 0x7f) << sh; sh += 7; } while (b & 0x80);
    const name = buf.slice(q, q + n).toString("latin1");
    if (name !== "name") continue;
    const end = s.start + s.size;
    while (q < end)
    {
        const sub = buf[q++]; const ssize = (() => { let r = 0, sh2 = 0, b2; do { b2 = buf[q++]; r |= (b2 & 0x7f) << sh2; sh2 += 7; } while (b2 & 0x80); return r >>> 0; })();
        const subEnd = q + ssize;
        if (sub === 1) // function names
        {
            let count = 0; { let r = 0, sh2 = 0, b2; do { b2 = buf[q++]; r |= (b2 & 0x7f) << sh2; sh2 += 7; } while (b2 & 0x80); count = r; }
            for (let i = 0; i < count; i++)
            {
                let idx = 0; { let r = 0, sh2 = 0, b2; do { b2 = buf[q++]; r |= (b2 & 0x7f) << sh2; sh2 += 7; } while (b2 & 0x80); idx = r; }
                let len = 0; { let r = 0, sh2 = 0, b2; do { b2 = buf[q++]; r |= (b2 & 0x7f) << sh2; sh2 += 7; } while (b2 & 0x80); len = r; }
                funcNames.set(idx, buf.slice(q, q + len).toString("latin1"));
                q += len;
            }
        }
        q = subEnd;
    }
}

// code section: id 10
const code = sections.find(s => s.id === 10);
let q = code.start;
let fcount = 0; { let r = 0, sh = 0, b; do { b = buf[q++]; r |= (b & 0x7f) << sh; sh += 7; } while (b & 0x80); fcount = r; }

// build function byte-offset -> index map
const funcs = []; // {start, end, index}
{
    let q2 = q;
    for (let i = 0; i < fcount; i++)
    {
        const size = (() => { let r = 0, sh = 0, b; do { b = buf[q2++]; r |= (b & 0x7f) << sh; sh += 7; } while (b & 0x80); return r >>> 0; })();
        funcs.push({ start: q2, end: q2 + size, index: i });
        q2 += size;
    }
}

// scan every i32.const (0x41) whose LEB value lands in [lo, hi]
function decodeSignedLEB(at) { let r = 0, sh = 0, b, q3 = at; do { b = buf[q3++]; r |= (b & 0x7f) << sh; sh += 7; } while (b & 0x80); if (sh < 32 && (b & 0x40)) r |= (~0 << sh); return { val: r | 0, len: q3 - at }; }

const hits = new Map(); // funcIndex -> [addrs]
for (let i = 0; i < buf.length - 5; i++)
{
    if (buf[i] !== 0x41) continue;
    const { val, len } = decodeSignedLEB(i + 1);
    if (val >= lo && val <= hi)
    {
        // find enclosing function (binary search)
        let a = 0, b2 = funcs.length - 1, mid;
        while (a <= b2) { mid = (a + b2) >> 1; if (funcs[mid].start <= i) a = mid + 1; else b2 = mid - 1; }
        const f = funcs[b2];
        if (f && i >= f.start && i < f.end)
        {
            if (!hits.has(f.index)) hits.set(f.index, []);
            hits.get(f.index).push(val);
        }
    }
}

const sorted = [...hits.entries()].sort((x, y2) => x[0] - y2[0]);
for (const [idx, addrs] of sorted)
{
    const name = funcNames.get(idx) ?? `func#${idx}`;
    const uniq = [...new Set(addrs)].map(a => "0x" + a.toString(16)).join(",");
    console.log(`${name}\n    idx=${idx} consts=${uniq} (${addrs.length} refs)`);
}
console.log(`total functions with refs in window: ${sorted.length}`);
