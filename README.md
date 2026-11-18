# PDF Annexe Editor

**👉 [Ouvrir l'outil](https://drfrrobin.github.io/PDF-annexe-Editor/)**

Application web 100 % cliente (aucune donnée n'est envoyée) : ajoute dans un rectangle rouge, à l'endroit choisi, le texte « Annexe X Folio n/N » sur les pages d'un PDF.

## Utilisation en local

Servir le dossier (`python3 -m http.server`) puis ouvrir `http://localhost:8000/`. Les librairies (pdf-lib, pdf.js) sont dans `vendor/`.

## Publication

Le lien ci-dessus nécessite l'activation de GitHub Pages : Settings → Pages → Source « Deploy from a branch », branche `main`, dossier `/ (root)`.

## Versions

Version actuelle : **1.1.3** (affichée dans l'en-tête de l'app). Chaque mise à jour ajoute 0.0.1 ; voir [CHANGELOG.md](CHANGELOG.md).
