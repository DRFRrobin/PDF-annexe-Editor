# Annexio

<p align="center"><img src="assets/logo-256.png" width="96" alt="Logo Annexio"></p>

**👉 [Ouvrir l'outil](https://drfrrobin.github.io/PDF-annexe-Editor/)**

**Annexio** : application web 100 % cliente (aucune donnée n'est envoyée) pour ajouter des tampons rouges sur les pages d'un PDF, en 4 étapes :

| Étape | Rôle |
|---|---|
| **1 · Fichier** | ouvrir le PDF (bouton, `Ctrl+O` ou glisser-déposer) |
| **2 · ReeMaker** | cartouche à 3 lignes (« Site de … » · REE | indice · Folio | type d'arrêt), posé page par page, folios continus, orientation automatique selon le sens du texte |
| **3 · Annexio** | tampon d'une ligne « Annexe X Folio n/N » |
| **4 · Export** | un seul PDF avec les deux tampons ; le fichier d'origine n'est jamais modifié ; **nom automatique** d'après ReeMaker (ex. `Folio 36 à 54 de REE JPJ 701 Tr 0 Ind A.pdf`, modèle réglable dans ⚙) |

Les étapes ReeMaker et Annexio sont facultatives et se cumulent ; rien n'est écrit avant l'export. Clavier : `Alt+1…4` étapes, `Entrée` ou clic droit valider une page, flèches ajuster le tampon (`Maj` : ×10), `Page préc./suiv.` changer de page, `Ctrl+molette` zoom, `Alt+,` paramètres.

## Version Rust

Une version Rust (application de bureau, sans ligne de commande) est dans [`rust/`](rust/README.md).

## Utilisation en local

Servir le dossier (`python3 -m http.server`) puis ouvrir `http://localhost:8000/`. Les librairies (pdf-lib, pdf.js) sont dans `vendor/`. Le dessin des tampons est dans `stamps.js`, testé avec `node tests/stamps.test.js`.

## Publication

Le lien ci-dessus nécessite l'activation de GitHub Pages : Settings → Pages → Source « Deploy from a branch », branche `main`, dossier `/ (root)`.

## Versions

Version actuelle : **1.1.24** (affichée dans l'en-tête de l'app). Chaque mise à jour ajoute 0.0.1 ; voir [CHANGELOG.md](CHANGELOG.md).
