<p align="center"><img src="../assets/logo-256.png" width="96" alt="Logo"></p>

# Annexio — version Rust

**Annexio** — application de bureau (Windows, Linux, macOS) : ajoute dans un rectangle rouge, à l'endroit choisi, le texte « Annexe X Folio n/N » sur les pages d'un PDF. Vos PDF ne quittent jamais l'ordinateur (seule la vérification de mise à jour, désactivable, contacte GitHub).

## Téléchargement

`annexio-windows-x86_64.exe` (ou Linux / macOS) (fichier fourni avec la release). Double-clic pour lancer.

## Utilisation

Barre à chevrons en haut : **Fichier → ReeMaker → Annexio → Export** (`Alt+1` à `Alt+4`). Les deux tampons sont facultatifs et se **cumulent** sur le même document ; rien n'est écrit avant l'export, et le fichier d'origine n'est jamais modifié. Le ⚙ ouvre les paramètres (apparence, taille de l'interface, taille de base du cartouche, modèle du nom du PDF, mises à jour, raccourcis).

1. **Fichier** : ouvrir un PDF (`Ctrl+O`, bouton ou glisser-déposer) ; fichiers récents.
2. **ReeMaker** : site, R.E.E., indice, folio de départ, type d'arrêt, cycle, pages, tranches, zoom et orientation (Auto d'après le sens du texte, ou 0/90/180/270°). « Démarrer le foliotage » : page par page, un clic place le cartouche (ou on le glisse), **Entrée** ou **clic droit** valide et passe à la page suivante ; « Passer », « Précédente », « Pivoter », « Garder la position ». Les folios se suivent sans trou, les pages passées ne consomment pas de numéro. Cartouche : `Site de … Tr 0` · `REE … | Ind. …` · `Folio n | type d'arrêt`.
3. **Annexio** : tampon d'une ligne « Annexe X Folio n/N » (modèle, folio, total, taille, pages, positions rapides, marge, annuler/rétablir).
4. **Export** : récapitulatif et nom automatique d'après ReeMaker, par exemple `REE JPJ 701 Tr 0 Ind A.pdf` (modèle `[REE {ree}][ Tr {tranche}][ Ind {indice}]` réglable ; `[ … ]` = partie facultative). Sans REE ni indice, ou si ReeMaker est ignoré : `<nom>_folioté.pdf`.

Aperçu : miniatures (pastille rouge = page tamponnée), molette = changer de page, `Ctrl+molette` ou − / + = zoom, `Page préc./suiv.`, `Début`/`Fin`. La largeur du panneau de gauche se règle en glissant sa bordure.

| Raccourci | Action |
|---|---|
| `Alt+1` … `Alt+4` | Fichier · ReeMaker · Annexio · Export |
| `Ctrl+O` / `Ctrl+S` / `Ctrl+W` | Ouvrir / Exporter / Fermer |
| `Entrée`, clic droit | Valider la page (foliotage) |
| `Échap` | Annuler le foliotage / quitter un champ |
| Flèches (`Maj` : ×10) | Déplacer le tampon actif |
| `Ctrl+Z` / `Ctrl+Y` | Annuler / rétablir (étape Annexio) |
| `Alt+,` | Paramètres |

Les réglages (textes, position, thème, taille, fichiers récents, ReeMaker) sont mémorisés dans un fichier local (`%APPDATA%\Annexio\settings.txt` sous Windows). Un PDF peut aussi être passé en argument ou « ouvert avec » l'application.

## Mise à jour automatique

Au démarrage, Annexio vérifie en arrière-plan s'il existe une version plus récente (requête vers `api.github.com` ; aucun fichier n'est envoyé). Un bandeau propose **Mettre à jour** ou **Plus tard**. L'installation télécharge le fichier de la release pour votre système, vérifie sa taille et son empreinte SHA-256, remplace l'exécutable (l'ancien est conservé en `.old` jusqu'au lancement suivant) puis propose de redémarrer.

Désactivable dans **Paramètres → Application** (« Vérifier les mises à jour au démarrage »), où un bouton permet aussi de rechercher à la demande. Le dossier de l'exécutable doit être modifiable par l'utilisateur (évitez `C:\Program Files`). La version de `Cargo.toml` doit correspondre au tag de la release (ex. `1.1.17` ↔ `v1.1.17`).

Un PDF peut aussi être passé en argument ou « ouvert avec » l'application.

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
wasm-bindgen --target web --out-dir pkg target/wasm32-unknown-unknown/release/annexio.wasm
```

`stamp_pdf(bytes: Uint8Array, optionsJson: string) -> Uint8Array`
(options : `annexe`, `template`, `start`, `total`, `fontSize`, `pages`, `whiteFill`, `fx`, `fy`).
