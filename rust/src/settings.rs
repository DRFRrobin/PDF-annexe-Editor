//! Réglages mémorisés d'une session à l'autre (fichier texte `clé=valeur`, local).

use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub annexe: String,
    pub template: String,
    pub start: String,
    pub total: String,
    pub size: String,
    pub white_fill: bool,
    pub fx: f32,
    pub fy: f32,
    pub margin: f32,
    pub side_w: f32,
    /// 0 = système, 1 = clair, 2 = sombre
    pub theme: u8,
    pub ui_scale: f32,
    pub thumbs: bool,
    /// Vérifier les mises à jour au démarrage.
    pub auto_update: bool,
    // ReeMaker
    pub rm_site: String,
    pub rm_ree: String,
    pub rm_indice: String,
    pub rm_start: String,
    pub rm_arret: String,
    pub rm_cycle: String,
    pub rm_show_cycle: bool,
    /// « 0,1 »
    pub rm_tranches: String,
    pub rm_zoom: f32,
    /// « auto » ou 0/90/180/270
    pub rm_orient: String,
    pub base_size: f32,
    pub keep_pos: bool,
    pub name_tpl: String,
    pub recents: Vec<String>,
}

pub const DEFAULT_NAME_TPL: &str = "[Folio {folios} de ][REE {ree}][ Tr {tranche}][ Ind {indice}]";
/// Ancien modèle par défaut : remplacé automatiquement par le nouveau (folios en tête).
const OLD_NAME_TPL: &str = "[REE {ree}][ Tr {tranche}][ Ind {indice}]";

impl Default for Settings {
    fn default() -> Self {
        Settings {
            annexe: "1".into(),
            template: "Annexe {annexe} Folio {n}/{total}".into(),
            start: "1".into(),
            total: String::new(),
            size: "14".into(),
            white_fill: true,
            fx: 0.04, // bas gauche : ne recouvre pas le cartouche, posé en haut à droite par défaut
            fy: 0.93,
            margin: 20.0,
            side_w: 290.0,
            theme: 0,
            ui_scale: 1.0,
            thumbs: true,
            auto_update: true,
            rm_site: String::new(),
            rm_ree: String::new(),
            rm_indice: String::new(),
            rm_start: "1".into(),
            rm_arret: String::new(),
            rm_cycle: String::new(),
            rm_show_cycle: false,
            rm_tranches: "0".into(),
            rm_zoom: 1.2,
            rm_orient: "auto".into(),
            base_size: 9.0,
            keep_pos: true,
            name_tpl: DEFAULT_NAME_TPL.into(),
            recents: Vec::new(),
        }
    }
}

impl Settings {
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        let mut kv = |k: &str, v: String| {
            s.push_str(k);
            s.push('=');
            s.push_str(&v.replace(['\n', '\r'], " "));
            s.push('\n');
        };
        kv("annexe", self.annexe.clone());
        kv("template", self.template.clone());
        kv("start", self.start.clone());
        kv("total", self.total.clone());
        kv("size", self.size.clone());
        kv("white_fill", self.white_fill.to_string());
        kv("fx", self.fx.to_string());
        kv("fy", self.fy.to_string());
        kv("margin", self.margin.to_string());
        kv("side_w", self.side_w.to_string());
        kv("theme", self.theme.to_string());
        kv("ui_scale", self.ui_scale.to_string());
        kv("thumbs", self.thumbs.to_string());
        kv("auto_update", self.auto_update.to_string());
        kv("rm_site", self.rm_site.clone());
        kv("rm_ree", self.rm_ree.clone());
        kv("rm_indice", self.rm_indice.clone());
        kv("rm_start", self.rm_start.clone());
        kv("rm_arret", self.rm_arret.clone());
        kv("rm_cycle", self.rm_cycle.clone());
        kv("rm_show_cycle", self.rm_show_cycle.to_string());
        kv("rm_tranches", self.rm_tranches.clone());
        kv("rm_zoom", self.rm_zoom.to_string());
        kv("rm_orient", self.rm_orient.clone());
        kv("base_size", self.base_size.to_string());
        kv("keep_pos", self.keep_pos.to_string());
        kv("name_tpl", self.name_tpl.clone());
        for r in &self.recents {
            kv("recent", r.clone());
        }
        s
    }

    pub fn from_text(text: &str) -> Settings {
        let mut s = Settings::default();
        let mut recents = Vec::new();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k {
                "annexe" => s.annexe = v.into(),
                "template" if !v.trim().is_empty() => s.template = v.into(),
                "start" => s.start = v.into(),
                "total" => s.total = v.into(),
                "size" => s.size = v.into(),
                "white_fill" => s.white_fill = v == "true",
                "fx" => s.fx = v.parse().unwrap_or(s.fx).clamp(0.0, 1.0),
                "fy" => s.fy = v.parse().unwrap_or(s.fy).clamp(0.0, 1.0),
                "margin" => s.margin = v.parse().unwrap_or(s.margin).clamp(0.0, 200.0),
                "side_w" => s.side_w = v.parse().unwrap_or(s.side_w).clamp(220.0, 640.0),
                "theme" => s.theme = v.parse().unwrap_or(0).min(2),
                "ui_scale" => s.ui_scale = v.parse::<f32>().unwrap_or(1.0).clamp(0.8, 2.0),
                "thumbs" => s.thumbs = v == "true",
                "auto_update" => s.auto_update = v != "false",
                "rm_site" => s.rm_site = v.into(),
                "rm_ree" => s.rm_ree = v.into(),
                "rm_indice" => s.rm_indice = v.into(),
                "rm_start" => s.rm_start = v.into(),
                "rm_arret" => s.rm_arret = v.into(),
                "rm_cycle" => s.rm_cycle = v.into(),
                "rm_show_cycle" => s.rm_show_cycle = v == "true",
                "rm_tranches" => s.rm_tranches = v.into(),
                "rm_zoom" => s.rm_zoom = v.parse::<f32>().unwrap_or(1.2).clamp(0.5, 3.0),
                "rm_orient" => s.rm_orient = v.into(),
                "base_size" => s.base_size = v.parse::<f32>().unwrap_or(9.0).clamp(5.0, 30.0),
                "keep_pos" => s.keep_pos = v != "false",
                "name_tpl" if !v.trim().is_empty() => s.name_tpl = if v == OLD_NAME_TPL { DEFAULT_NAME_TPL.into() } else { v.into() },
                "recent" if !v.is_empty() => recents.push(v.to_string()),
                _ => {}
            }
        }
        recents.truncate(8);
        s.recents = recents;
        s
    }
}

fn config_file(dir: &str) -> Option<PathBuf> {
    let base = if cfg!(windows) {
        PathBuf::from(std::env::var_os("APPDATA")?)
    } else if cfg!(target_os = "macos") {
        PathBuf::from(std::env::var_os("HOME")?).join("Library/Application Support")
    } else {
        match std::env::var_os("XDG_CONFIG_HOME") {
            Some(x) if !x.is_empty() => PathBuf::from(x),
            _ => PathBuf::from(std::env::var_os("HOME")?).join(".config"),
        }
    };
    Some(base.join(dir).join("settings.txt"))
}

/// Reprend les réglages de l'ancien dossier « AnnexePDF » si « Annexio » n'existe pas encore.
pub fn load() -> Settings {
    ["Annexio", "AnnexePDF"]
        .iter()
        .filter_map(|d| config_file(d))
        .find_map(|p| std::fs::read_to_string(p).ok())
        .map(|t| Settings::from_text(&t))
        .unwrap_or_default()
}

pub fn save(s: &Settings) {
    if let Some(p) = config_file("Annexio") {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, s.to_text());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aller_retour() {
        let mut s = Settings::default();
        s.annexe = "B".into();
        s.template = "Annexe {annexe} = page {n}".into(); // « = » dans la valeur
        s.theme = 2;
        s.auto_update = false;
        s.rm_ree = "JPJ 701".into();
        s.rm_tranches = "1,2".into();
        s.name_tpl = "[REE {ree}] = x".into(); // « = » dans la valeur
        s.ui_scale = 1.25;
        s.recents = vec!["C:\\a b\\c.pdf".into(), "/tmp/d.pdf".into()];
        assert_eq!(Settings::from_text(&s.to_text()), s);
    }

    #[test]
    fn ancien_modele_de_nom_remplace() {
        assert_eq!(Settings::from_text(&format!("name_tpl={OLD_NAME_TPL}\n")).name_tpl, DEFAULT_NAME_TPL);
        assert_eq!(Settings::from_text("name_tpl={nom} perso\n").name_tpl, "{nom} perso");
    }

    #[test]
    fn valeurs_invalides_ignorees() {
        let s = Settings::from_text("fx=abc\ntheme=9\nui_scale=50\nn'importe quoi\nsize=20\n");
        assert_eq!(s.fx, Settings::default().fx);
        assert_eq!(s.theme, 2);
        assert_eq!(s.ui_scale, 2.0);
        assert_eq!(s.size, "20");
    }
}
