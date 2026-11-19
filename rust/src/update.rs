//! Mise à jour manuelle depuis les releases GitHub. Aucune donnée du PDF n'est envoyée :
//! une requête GET vers l'API GitHub, puis le téléchargement du fichier de la release.

use std::io::Read;
use std::path::{Path, PathBuf};

const REPO: &str = "DRFRrobin/PDF-annexe-Editor";
const DL_PREFIX: &str = "https://github.com/DRFRrobin/PDF-annexe-Editor/releases/download/";
const MAX_BYTES: u64 = 200 * 1024 * 1024;

#[cfg(all(windows, target_arch = "x86_64"))]
pub const ASSET: &str = "pdf-annexe-windows-x86_64.exe";
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub const ASSET: &str = "pdf-annexe-linux-x86_64";
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub const ASSET: &str = "pdf-annexe-macos-arm64";
#[cfg(not(any(
    all(windows, target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64")
)))]
pub const ASSET: &str = "";

#[derive(Debug, Clone)]
pub struct Release {
    pub version: String,
    pub page: String,
    /// (url, taille) du fichier correspondant à ce système, s'il existe.
    pub asset: Option<(String, u64)>,
}

/// « V1.2.3 » / « v1.2.3 » -> [1, 2, 3]
pub fn parse_version(s: &str) -> Option<Vec<u64>> {
    let t = s.trim().trim_start_matches(['v', 'V']);
    let parts: Option<Vec<u64>> = t.split('.').map(|p| p.parse().ok()).collect();
    parts.filter(|p| !p.is_empty())
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(mut a), Some(mut b)) => {
            let n = a.len().max(b.len());
            a.resize(n, 0);
            b.resize(n, 0);
            a > b
        }
        _ => false,
    }
}

/// Client HTTP qui s'appuie sur les certificats du système (proxy d'entreprise, antivirus…).
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build())
        .build()
        .into()
}

/// Dernière release publiée, ou `None` si elle n'est pas plus récente que `current`.
pub fn check(current: &str) -> Result<Option<Release>, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let mut resp = agent()
        .get(&url)
        .header("User-Agent", "pdf-annexe-updater")
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Vérification impossible : {e}"))?;
    let body = resp.body_mut().read_to_string().map_err(|e| format!("Réponse illisible : {e}"))?;
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| format!("Réponse illisible : {e}"))?;
    let tag = v["tag_name"].as_str().ok_or("Réponse GitHub inattendue")?;
    if !is_newer(tag, current) {
        return Ok(None);
    }
    let asset = v["assets"].as_array().and_then(|a| {
        a.iter().find(|x| x["name"].as_str() == Some(ASSET)).and_then(|x| {
            let u = x["browser_download_url"].as_str()?;
            u.starts_with(DL_PREFIX).then(|| (u.to_string(), x["size"].as_u64().unwrap_or(0)))
        })
    });
    Ok(Some(Release {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        page: v["html_url"].as_str().unwrap_or("").to_string(),
        asset,
    }))
}

/// Télécharge le nouvel exécutable et remplace celui en cours ; renvoie son chemin.
pub fn install(rel: &Release) -> Result<PathBuf, String> {
    let (url, size) = rel.asset.clone().ok_or("Aucun fichier pour ce système dans la release.")?;
    let resp = agent().get(&url).header("User-Agent", "pdf-annexe-updater").call().map_err(|e| format!("Téléchargement impossible : {e}"))?;
    let mut data = Vec::new();
    resp.into_body().into_reader().take(MAX_BYTES).read_to_end(&mut data).map_err(|e| format!("Téléchargement interrompu : {e}"))?;
    if data.len() < 1_000_000 || (size > 0 && data.len() as u64 != size) {
        return Err("Fichier téléchargé incomplet : mise à jour annulée.".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    replace_exe(&exe, &data)?;
    Ok(exe)
}

fn side(exe: &Path, ext: &str) -> PathBuf {
    let mut s = exe.as_os_str().to_owned();
    s.push(ext);
    PathBuf::from(s)
}

fn replace_exe(exe: &Path, data: &[u8]) -> Result<(), String> {
    let (new, old) = (side(exe, ".new"), side(exe, ".old"));
    std::fs::write(&new, data).map_err(|e| format!("Écriture impossible : {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755));
    }
    let _ = std::fs::remove_file(&old);
    // On ne peut pas écraser un exe en cours sous Windows, mais on peut le renommer.
    std::fs::rename(exe, &old).map_err(|e| format!("Remplacement impossible : {e}"))?;
    if let Err(e) = std::fs::rename(&new, exe) {
        let _ = std::fs::rename(&old, exe); // retour arrière
        return Err(format!("Remplacement impossible : {e}"));
    }
    Ok(())
}

/// À appeler au démarrage : supprime l'ancien exécutable laissé par une mise à jour.
pub fn cleanup() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(side(&exe, ".old"));
        let _ = std::fs::remove_file(side(&exe, ".new"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(parse_version("V1.1.5"), Some(vec![1, 1, 5]));
        assert!(is_newer("v1.1.8", "1.1.7"));
        assert!(is_newer("V1.2", "1.1.9"));
        assert!(!is_newer("V1.1.5", "1.1.7"));
        assert!(!is_newer("1.1.7", "1.1.7"));
        assert!(!is_newer("abc", "1.1.7"));
    }

    /// Test réseau : `cargo test --features gui -- --ignored`
    #[test]
    #[ignore]
    fn verification_reseau() {
        let r = check("0.0.1").unwrap().expect("une release plus récente doit exister");
        println!("{r:?}");
    }

    #[test]
    fn remplacement() {
        let d = std::env::temp_dir().join(format!("anx_upd_{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let exe = d.join("app.exe");
        std::fs::write(&exe, b"ancien").unwrap();
        replace_exe(&exe, b"nouveau").unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"nouveau");
        assert_eq!(std::fs::read(side(&exe, ".old")).unwrap(), b"ancien");
        std::fs::remove_dir_all(&d).unwrap();
    }
}
