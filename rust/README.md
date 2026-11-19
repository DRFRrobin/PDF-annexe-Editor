# PDF Annexe Editor — version Rust

Même fonction que l'app web : tampon rouge « Annexe X Folio n/N » sur les pages d'un PDF, 100 % local. Gère les pages tournées et les `MediaBox` décalées.

## Téléchargement

Les exécutables (Windows `.exe`, Linux, macOS) sont dans les [Releases](https://github.com/DRFRrobin/PDF-annexe-Editor/releases).

## Programme en ligne de commande

```bash
cd rust
cargo run --release -- entree.pdf sortie.pdf --annexe B --x 0.55 --y 0.03
cargo run --release -- --help
```

Options : `--annexe`, `--template`, `--start`, `--total`, `--size`, `--pages 1-3,5`, `--x`/`--y` (coin haut-gauche, fraction de la page), `--no-fill`.

## Tests

```bash
cargo test
```

## WebAssembly (dans le navigateur)

```bash
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown --no-default-features --features wasm --lib
wasm-bindgen --target web --out-dir pkg target/wasm32-unknown-unknown/release/pdf_annexe.wasm
```

Expose `stamp_pdf(bytes: Uint8Array, optionsJson: string) -> Uint8Array`
(options : `annexe`, `template`, `start`, `total`, `fontSize`, `pages`, `whiteFill`, `fx`, `fy`).
