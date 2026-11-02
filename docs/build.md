# Build chain: europass-xml (Rust -> wasm-bindgen)

Verified on 2026-11-02 with a throwaway crate (serde 1, serde_json 1, quick-xml 0.37, wasm-bindgen 0.2),
rustc/cargo 1.96.1, wasm-pack 0.15.0, node 24, vitest 3.2.4. The throwaway crate is NOT in the repo.

## Findings

| Question | Result |
|---|---|
| crates.io reachable? | Yes (index.crates.io returns 200). |
| Works offline? | Yes. `~/.cargo/registry` already holds ~1050 crates; `cargo build --offline --release --target wasm32-unknown-unknown` succeeded for the probe. Use `--locked` and commit `Cargo.lock` so CI/offline is reproducible. |
| `wasm-pack build` targets web / nodejs / bundler | All three compile and run. `nodejs` output ran fine under plain node (`<SkillsPassport>Ni</SkillsPassport>`). |
| Hand-rolled XML writer needed? | No. quick-xml compiles to wasm without extra features. A hand-rolled escaping writer remains an acceptable fallback (about 30 lines). |
| Wasm size (probe: serde_json + quick-xml, opt-level s, lto) | 96 KB raw, 88 KB after `wasm-opt -Os`, about 40 KB gzipped. Real crate with full CV model will likely be 150-300 KB raw; far below oxigraph (3.5 MB) and typst (28 MB). |

### Gotcha 1: wasm-pack's wasm-opt step FAILS with the system wasm-opt
wasm-pack 0.15 runs `/usr/bin/wasm-opt` (binaryen v108), which rejects what rustc 1.96 emits
(`Bulk memory operation (bulk memory is disabled)`), so `wasm-pack build --release` exits 1 (the `.wasm` is still
written, but the command fails). Fix in Cargo.toml (preferred, keeps optimisation):

    [package.metadata.wasm-pack.profile.release]
    wasm-opt = ["-Os", "--all-features"]

Verified manually: `wasm-opt -Os --all-features` works. If it ever breaks, use `wasm-opt = false`
(costs ~8% size). Dev profile: `[package.metadata.wasm-pack.profile.dev] wasm-opt = false`.

### Gotcha 2: nodejs target is unusable from this repo
Root `package.json` has `"type": "module"`. `--target nodejs` emits CommonJS in a `.js` file, so importing it from
inside the repo fails (ESM/CJS error). Do not use `--target nodejs`.

### Gotcha 3: vitest uses `environment: 'jsdom'` (vitest.config.ts)
Under jsdom `import.meta.url` is not a `file:` URL, so `new URL('x.wasm', import.meta.url)` and the
generated default `init()` (which fetches relative to `import.meta.url`) fail ("The URL must be of scheme file").
Solution verified: `--target web` output + `initSync` fed with bytes read via `fs` from a `process.cwd()`-based path.

## Decision: ONE target, `--target web`, for browser, SSR/prerender and tests

Same pattern as oxigraph in `src/lib/semcv/semantic_cv_store.ts` (`import wasmUrl from 'oxigraph/web_bg.wasm?url'`
then `await init(wasmUrl)`): vite emits the wasm as a hashed asset, adapter-static copies it to `dist/`. No vite
plugin (vite-plugin-wasm / top-level-await) is needed, and vite.config.ts needs no change.

Browser (client only; guard with `typeof window !== 'undefined'` like initOxigraph, since SSG prerender runs in node):

```ts
import wasmUrl from '$lib/semcv/europass/wasm/europass_xml_bg.wasm?url';
import init, { cv_to_europass_xml } from '$lib/semcv/europass/wasm/europass_xml.js';
let ready: Promise<unknown> | undefined;
export async function toEuropassXml(cvJson: string, opts?: string) {
  ready ??= init({ module_or_path: wasmUrl });   // wasm-bindgen >=0.2.93 object form
  await ready;
  return cv_to_europass_xml(cvJson, opts);
}
```

Tests (node/jsdom; mirror it in a `loadWasmForTests()` helper):

```ts
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { initSync } from '$lib/semcv/europass/wasm/europass_xml.js';
initSync({ module: readFileSync(path.resolve(process.cwd(), 'src/lib/semcv/europass/wasm/europass_xml_bg.wasm')) });
```
(Verified passing in vitest 3.2.4 / jsdom. Always use `process.cwd()`, never `import.meta.url`.)
Existing oxigraph rule for tests still applies (`oxigraph/node.js`).

## Commands

Crate lives at `crates/europass-xml/` (Cargo.toml there; no workspace needed unless a second crate appears).

```bash
# one-off tooling check
rustup target add wasm32-unknown-unknown
# build (release) into the committed location
wasm-pack build crates/europass-xml --release --target web \
  --out-dir ../../src/lib/semcv/europass/wasm --out-name europass_xml --no-pack
# native tests / XSD validation harness
cargo test --manifest-path crates/europass-xml/Cargo.toml --locked
```
`--no-pack` skips the generated package.json/README (not needed for a relative import; wasm-pack 0.15 supports it).
wasm-pack also writes `.gitignore` containing `*` into out-dir: delete it (script below), otherwise the artifacts
are silently ignored.

## package.json changes

Add (npm only):

```json
"build:wasm": "wasm-pack build crates/europass-xml --release --target web --out-dir ../../src/lib/semcv/europass/wasm --out-name europass_xml --no-pack && rm -f src/lib/semcv/europass/wasm/.gitignore",
"test:rust": "cargo test --manifest-path crates/europass-xml/Cargo.toml --locked"
```

- Do NOT add `build:wasm` to `postinstall` or to `build`. The existing postinstall only copies prebuilt npm wasm;
  the GitHub Pages workflow (`.github/workflows/pages.yml`: setup-node 20, `npm install`, `npm run build`) has no Rust
  toolchain and installing one in CI would add minutes and a failure mode.
- `npm run build` therefore consumes the committed artifacts and starts with `npm run check:wasm`
  (`scripts/check-europass-wasm.mjs`), which fails the build when `src/lib/semcv/europass/wasm/` is missing or
  empty, so a checkout without the generated files cannot deploy a site with a dead export button. Run `npm run build:wasm` manually whenever
  `crates/europass-xml/src` or the schema changes.
- Optional safeguard: a vitest test that asserts the committed `.wasm` exists, plus a golden-XML test through the wasm
  so a stale binary is caught.

## What gets committed

**Decision (explicit): the generated wasm package IS committed.** The deploy (GitHub Pages) has no Rust toolchain,
so the Europass XML button would break if the binary were ignored. The cost is a ~300 KB binary diff each time the
crate changes. Rules that keep this tolerable: rebuild with `npm run build:wasm` only when `crates/europass-xml/`
changed, commit the rebuilt files in the same commit as the Rust change, and never hand-edit them. `build:wasm`
deletes the `.gitignore` that wasm-pack writes (it contains `*`) on purpose, otherwise the files could not be tracked.
`crates/europass-xml/.gitignore` already ignores `target/` and `pkg/`.

- Commit: `crates/europass-xml/{Cargo.toml,Cargo.lock,src/,tests/,schema/,docs/}` and the build output
  `src/lib/semcv/europass/wasm/` (`europass_xml.js`, `.d.ts`, `europass_xml_bg.wasm`, `_bg.wasm.d.ts`; <300 KB).
  Rationale: CI and Pages have no Rust; the artifact is small; builds stay reproducible from the checkout.
- Ignore: add to the root `.gitignore`:

      # Rust build output
      crates/**/target/
      crates/**/pkg/

  (Currently `crates/europass-xml/target/` is NOT ignored; verified with `git check-ignore`.)
- Out-dir under `src/lib` (not `static/`) is deliberate: vite hashes and tree-shakes the asset via `?url`, and
  `$lib` imports work in components, vitest and prerender. The `static/wasm/` folder is gitignored (typst postinstall)
  and must not be used.

## Cargo.toml skeleton (recommendation)

```toml
[package]
name = "europass-xml"
version = "0.1.0"
edition = "2021"
license = "MIT"
publish = false
rust-version = "1.80"

[lib]
crate-type = ["cdylib", "rlib"]   # cdylib for wasm-pack, rlib for cargo test / integration tests

[dependencies]
wasm-bindgen = "0.2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
quick-xml = "0.37"

[dev-dependencies]
# wasm-bindgen-test only if browser tests are wanted; XSD validation is done natively via xmllint (see below)

[profile.release]
opt-level = "s"        # "z" saves ~5% more; measure
lto = true
codegen-units = 1
panic = "abort"
strip = true

[package.metadata.wasm-pack.profile.release]
wasm-opt = ["-Os", "--all-features"]   # required: system wasm-opt v108 rejects bulk-memory otherwise

[package.metadata.wasm-pack.profile.dev]
wasm-opt = false
```

Notes: keep the exported API string-in/string-out (`fn cv_to_europass_xml(json: &str, opts_json: Option<String>)
-> Result<String, JsError>`) to avoid serde-wasm-bindgen and keep the binary small; avoid `chrono`/`regex`/`getrandom`
(getrandom needs the `js` feature on wasm32 and bloats). Pin `wasm-bindgen` version consistent with the installed
wasm-bindgen CLI that wasm-pack downloads/uses; if wasm-pack reports a schema mismatch, `cargo update -p wasm-bindgen`.

## XSD validation harness (note for the schema task)

Validate with `xmllint --noout --schema schema/v3.4.1/EuropassSchema.xsd out.xml` from a Rust integration test
(`std::process::Command`, skipped if xmllint is missing). The legacy `xsd:import` hosts no longer resolve, so use
local copies plus either an XML catalog (`XML_CATALOG_FILES`, `<rewriteURI>` mapping
`http://europass.cedefop.europa.eu/xml/imported/` to the local dir) or rewritten schemaLocations; also pass
`xmllint --nonet`.

## Throwaway probe (not committed)

`<scratch>/probe/`
