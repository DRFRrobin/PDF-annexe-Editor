//! Mise à jour depuis les releases GitHub : vérification en arrière-plan, installation après
//! confirmation. Aucune donnée du PDF n'est envoyée : une requête GET vers l'API GitHub, puis le
//! téléchargement du fichier de la release (contrôlé par taille et, si fournie, empreinte SHA-256).

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

const REPO: &str = "DRFRrobin/PDF-annexe-Editor";
const DL_PREFIX: &str = "https://github.com/DRFRrobin/PDF-annexe-Editor/releases/download/";
const MAX_BYTES: u64 = 200 * 1024 * 1024;

#[cfg(all(windows, target_arch = "x86_64"))]
pub const ASSET: &str = "annexio-windows-x86_64.exe";
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub const ASSET: &str = "annexio-linux-x86_64";
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub const ASSET: &str = "annexio-macos-arm64";
#[cfg(not(any(
    all(windows, target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64")
)))]
pub const ASSET: &str = "";

#[derive(Debug, Clone)]
pub struct Release {
    pub version: String,
    /// (url, taille, empreinte SHA-256 hexadécimale si connue) du fichier de ce système.
    pub asset: Option<(String, u64, Option<String>)>,
}

/// Version courante ; surchargeable en développement (`ANNEXIO_TEST_VERSION`) pour tester.
pub fn current_version() -> String {
    #[cfg(debug_assertions)]
    if let Ok(v) = std::env::var("ANNEXIO_TEST_VERSION") {
        return v;
    }
    env!("CARGO_PKG_VERSION").to_string()
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
        .timeout_global(Some(std::time::Duration::from_secs(300)))
        .build()
        .into()
}

fn parse_release(v: &serde_json::Value, current: &str) -> Result<Option<Release>, String> {
    let tag = v["tag_name"].as_str().ok_or("Réponse GitHub inattendue")?;
    if !is_newer(tag, current) {
        return Ok(None);
    }
    let asset = v["assets"].as_array().and_then(|a| {
        a.iter().find(|x| x["name"].as_str() == Some(ASSET)).and_then(|x| {
            let u = x["browser_download_url"].as_str()?;
            if !u.starts_with(DL_PREFIX) {
                return None;
            }
            let digest = x["digest"].as_str().and_then(|d| d.strip_prefix("sha256:")).map(str::to_lowercase);
            Some((u.to_string(), x["size"].as_u64().unwrap_or(0), digest))
        })
    });
    Ok(Some(Release { version: tag.trim_start_matches(['v', 'V']).to_string(), asset }))
}

/// Dernière release publiée, ou `None` si elle n'est pas plus récente que la version courante.
pub fn check() -> Result<Option<Release>, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let mut resp = agent()
        .get(&url)
        .header("User-Agent", "annexio-updater")
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Vérification impossible : {e}"))?;
    let body = resp.body_mut().read_to_string().map_err(|e| format!("Réponse illisible : {e}"))?;
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| format!("Réponse illisible : {e}"))?;
    parse_release(&v, &current_version())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Contrôle d'intégrité du fichier téléchargé.
fn verify(data: &[u8], size: u64, digest: Option<&str>) -> Result<(), String> {
    if data.len() < 1_000_000 || (size > 0 && data.len() as u64 != size) {
        return Err("Fichier téléchargé incomplet : mise à jour annulée.".into());
    }
    if let Some(d) = digest {
        if hex(&Sha256::digest(data)) != d {
            return Err("Empreinte du fichier incorrecte : mise à jour annulée.".into());
        }
    }
    Ok(())
}

/// Télécharge le nouvel exécutable et remplace celui en cours ; renvoie son chemin.
pub fn install(rel: &Release) -> Result<PathBuf, String> {
    let (url, size, digest) = rel.asset.clone().ok_or("Aucun fichier pour ce système dans la release.")?;
    let resp = agent()
        .get(&url)
        .header("User-Agent", "annexio-updater")
        .call()
        .map_err(|e| format!("Téléchargement impossible : {e}"))?;
    let mut data = Vec::new();
    resp.into_body().into_reader().take(MAX_BYTES).read_to_end(&mut data).map_err(|e| format!("Téléchargement interrompu : {e}"))?;
    verify(&data, size, digest.as_deref())?;
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
    std::fs::write(&new, data).map_err(|e| format!("Écriture impossible (dossier protégé ?) : {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755));
    }
    let _ = std::fs::remove_file(&old);
    // On ne peut pas écraser un exe en cours sous Windows, mais on peut le renommer.
    if let Err(e) = std::fs::rename(exe, &old) {
        let _ = std::fs::remove_file(&new);
        return Err(format!("Remplacement impossible : {e}"));
    }
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
        assert!(is_newer("v1.1.17", "1.1.16"));
        assert!(is_newer("v1.1.10", "1.1.9")); // comparaison numérique, pas alphabétique
        assert!(is_newer("V1.2", "1.1.9"));
        assert!(!is_newer("V1.1.5", "1.1.7"));
        assert!(!is_newer("1.1.7", "1.1.7"));
        assert!(!is_newer("abc", "1.1.7"));
    }

    #[test]
    fn lecture_de_la_release() {
        let json = serde_json::json!({
            "tag_name": "v9.9.9",
            "assets": [
                {"name": "autre-fichier", "browser_download_url": format!("{DL_PREFIX}v9.9.9/autre-fichier"), "size": 1},
                {"name": ASSET, "browser_download_url": format!("{DL_PREFIX}v9.9.9/{ASSET}"), "size": 123, "digest": "sha256:ABCDEF"},
                ]
        });
        let r = parse_release(&json, "1.0.0").unwrap().unwrap();
        assert_eq!(r.version, "9.9.9");
        if !ASSET.is_empty() {
            assert_eq!(r.asset, Some((format!("{DL_PREFIX}v9.9.9/{ASSET}"), 123, Some("abcdef".into()))));
        }
        assert!(parse_release(&json, "9.9.9").unwrap().is_none());
        // une URL hors du dépôt est refusée
        let faux = serde_json::json!({"tag_name": "v9.9.9", "assets": [
            {"name": ASSET, "browser_download_url": "https://exemple.org/malveillant", "size": 5}]});
        assert!(parse_release(&faux, "1.0.0").unwrap().unwrap().asset.is_none());
    }

    #[test]
    fn controle_d_integrite() {
        let data = vec![7u8; 1_200_000];
        let good = hex(&Sha256::digest(&data));
        assert!(verify(&data, data.len() as u64, Some(&good)).is_ok());
        assert!(verify(&data, data.len() as u64, Some("00")).is_err()); // mauvaise empreinte
        assert!(verify(&data, data.len() as u64 + 1, None).is_err()); // mauvaise taille
        assert!(verify(&data[..10], 0, None).is_err()); // trop petit
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

    /// Test réseau : `cargo test -- --ignored`
    #[test]
    #[ignore]
    fn verification_reseau() {
        std::env::set_var("ANNEXIO_TEST_VERSION", "0.0.1");
        let r = check().unwrap().expect("une release plus récente doit exister");
        println!("{r:?}");
    }
}
