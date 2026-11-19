use pdf_annexe::{stamp, Options};
use std::process::exit;

const AIDE: &str = "pdf-annexe <entree.pdf> <sortie.pdf> [options]
  --annexe X        numéro/lettre d'annexe (défaut 1)
  --template T      modèle (défaut « Annexe {annexe} Folio {n}/{total} »)
  --start N         premier folio (défaut 1)
  --total N         total N (défaut : nombre de pages)
  --size PT         taille du texte (défaut 14)
  --pages R         pages à tamponner, ex. 1-3,5 (défaut toutes)
  --x F --y F       coin haut-gauche, fraction de la page 0..1 (défaut 0.55 0.03)
  --no-fill         pas de fond blanc";

fn main() {
    if let Err(e) = run() {
        eprintln!("Erreur : {e}");
        exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut files, mut o) = (Vec::new(), Options::default());
    while let Some(a) = args.next() {
        let mut val = |n: &str| args.next().ok_or(format!("{n} attend une valeur"));
        match a.as_str() {
            "-h" | "--help" => return Err(AIDE.into()),
            "--annexe" => o.annexe = val(&a)?,
            "--template" => o.template = val(&a)?,
            "--start" => o.start = val(&a)?.parse().map_err(|_| "--start invalide")?,
            "--total" => o.total = Some(val(&a)?.parse().map_err(|_| "--total invalide")?),
            "--size" => o.font_size = val(&a)?.parse().map_err(|_| "--size invalide")?,
            "--pages" => o.pages = val(&a)?,
            "--x" => o.fx = val(&a)?.parse().map_err(|_| "--x invalide")?,
            "--y" => o.fy = val(&a)?.parse().map_err(|_| "--y invalide")?,
            "--no-fill" => o.white_fill = false,
            _ if a.starts_with("--") => return Err(format!("option inconnue {a}\n{AIDE}")),
            _ => files.push(a),
        }
    }
    let [input, output] = files.as_slice() else { return Err(AIDE.into()) };
    let bytes = std::fs::read(input).map_err(|e| format!("{input} : {e}"))?;
    let (out, n) = stamp(&bytes, &o)?;
    std::fs::write(output, out).map_err(|e| format!("{output} : {e}"))?;
    println!("Terminé : {n} page(s) tamponnée(s) -> {output}");
    Ok(())
}
