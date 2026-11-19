<p align="center"><img src="../assets/logo-256.png" width="96" alt="Logo"></p>

# PDF Annexe Editor — version Rust

Application de bureau (Windows, Linux, macOS) : ajoute dans un rectangle rouge, à l'endroit choisi, le texte « Annexe X Folio n/N » sur les pages d'un PDF. 100 % local : votre fichier ne quitte jamais l'ordinateur.

## Téléchargement

`pdf-annexe-windows-x86_64.exe` (ou Linux / macOS) (fichier fourni avec la release). Double-clic pour lancer.

## Utilisation

Barre d'onglets en haut :
- **📄 Fichier** : ouvrir un PDF (bouton ou glisser-déposer), naviguer entre les pages, générer le PDF tamponné, fermer le fichier.
- **⚙ Paramètres** : texte du tampon (`{annexe}`, `{n}`, `{total}`), premier folio, total, taille, pages concernées (ex. `1-3,5`), fond blanc ; section **Application** (version).

Le logo est l'icône de la fenêtre et de l'exe Windows.

La largeur du panneau de gauche se règle en glissant sa bordure.

La molette de la souris (ou ↑ ↓, Page préc./suiv.) change de page dans l'aperçu.

Placez le rectangle rouge en le glissant dans l'aperçu, ou en cliquant sur la page. La position est la même (en proportion) sur toutes les pages. Un PDF peut aussi être passé en argument ou « ouvert avec » l'application.

## Développement

```bash
cd rust
cargo run --release [fichier.pdf]
cargo test
```

La bibliothèque (`src/lib.rs`, tamponnage avec `lopdf`) expose aussi une liaison WebAssembly :

```bash
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown --no-default-features --features wasm --lib
wasm-bindgen --target web --out-dir pkg target/wasm32-unknown-unknown/release/pdf_annexe.wasm
```

`stamp_pdf(bytes: Uint8Array, optionsJson: string) -> Uint8Array`
(options : `annexe`, `template`, `start`, `total`, `fontSize`, `pages`, `whiteFill`, `fx`, `fy`).
