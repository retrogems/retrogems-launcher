//! RetroGems Launcher
//!
//! La fenêtre affiche l'interface du launcher de retrogems.fr (la même que sur le site : bibliothèque, Studio,
//! connexion, votes). Le programme ajoute ce qu'un site ne peut pas faire : installer, mettre à jour, lancer et
//! désinstaller les portages, et ouvrir les liens `retrogems://` cliqués dans le navigateur.
//!
//! Liens gérés : retrogems://installer/{slug}, retrogems://jouer/{slug}, retrogems://desinstaller/{slug},
//! retrogems://dossier/{slug}, retrogems://ouvrir/?u={adresse} (lien externe ouvert dans le navigateur).

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_deep_link::DeepLinkExt;

const SITE: &str = "https://retrogems.fr";
const SCRIPT_PAGE: &str = include_str!("app.js");

/* ---------------------------------------------------------------- langue et adresses */

fn langue() -> &'static str {
    let l = sys_locale::get_locale().unwrap_or_default().to_lowercase();
    if l.starts_with("fr") {
        "fr"
    } else if l.starts_with("es") {
        "es"
    } else {
        "en"
    }
}

fn t(fr: &str, en: &str, es: &str) -> String {
    match langue() {
        "fr" => fr.to_string(),
        "es" => es.to_string(),
        _ => en.to_string(),
    }
}

fn accueil() -> String {
    match langue() {
        "fr" => format!("{SITE}/portages-recomp-decomp/?rgapp=1"),
        l => format!("{SITE}/{l}/ports-recomp-decomp/?rgapp=1"),
    }
}

fn systeme() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

/* ---------------------------------------------------------------- dossiers et registre des jeux installés */

fn racine() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("RetroGems Launcher")
}
fn dossier_jeux() -> PathBuf {
    racine().join("jeux")
}
fn chemin_registre() -> PathBuf {
    racine().join("installes.json")
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct Installe {
    slug: String,
    titre: String,
    version: String,
    dossier: String,
    lancer: String,
    args: Vec<String>,
}

fn lire_registre() -> Vec<Installe> {
    fs::read_to_string(chemin_registre())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn ecrire_registre(r: &[Installe]) {
    let _ = fs::create_dir_all(racine());
    if let Ok(s) = serde_json::to_string_pretty(r) {
        let _ = fs::write(chemin_registre(), s);
    }
}

fn nom_sur(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect()
}

fn es<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/* ---------------------------------------------------------------- dialogue avec la page */

fn js_texte(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

fn page(app: &AppHandle, js: String) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.eval(&js);
    }
}

fn progres(app: &AppHandle, slug: &str, titre: &str, etat: &str, pct: f64, msg: &str) {
    page(
        app,
        format!(
            "window.rglApp&&window.rglApp.progres({},{},{},{},{})",
            js_texte(slug),
            js_texte(titre),
            js_texte(etat),
            pct,
            js_texte(msg)
        ),
    );
}

fn pousser_etat(app: &AppHandle) {
    let r = lire_registre();
    let j = serde_json::to_string(&r).unwrap_or_else(|_| "[]".to_string());
    page(app, format!("window.rglApp&&window.rglApp.etat({j})"));
}

/* ---------------------------------------------------------------- liens retrogems:// */

fn gerer_lien(app: &AppHandle, u: &Url) {
    if u.scheme() != "retrogems" {
        return;
    }
    let action = u.host_str().unwrap_or("").to_string();
    let slug = urlencoding::decode(u.path().trim_start_matches('/'))
        .map(|s| s.into_owned())
        .unwrap_or_default();
    let app2 = app.clone();
    match action.as_str() {
        "installer" => {
            tauri::async_runtime::spawn(async move {
                if let Err(e) = installer(&app2, &slug).await {
                    progres(&app2, &slug, &slug, "erreur", 0.0, &e);
                    rapport(&slug, "", false).await;
                }
            });
        }
        "jouer" => match lire_registre().into_iter().find(|i| i.slug == slug) {
            Some(i) => {
                if let Err(e) = lancer_jeu(&i) {
                    progres(app, &i.slug, &i.titre, "erreur", 0.0, &e);
                }
            }
            None => {
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = installer(&app2, &slug).await {
                        progres(&app2, &slug, &slug, "erreur", 0.0, &e);
                    }
                });
            }
        },
        "desinstaller" => {
            let mut r = lire_registre();
            if let Some(i) = r.iter().find(|i| i.slug == slug) {
                let _ = fs::remove_dir_all(&i.dossier);
            }
            r.retain(|i| i.slug != slug);
            ecrire_registre(&r);
            pousser_etat(app);
        }
        "dossier" => {
            if let Some(i) = lire_registre().into_iter().find(|i| i.slug == slug) {
                let _ = open::that(&i.dossier);
            }
        }
        "ouvrir" => {
            if let Some((_, v)) = u.query_pairs().find(|(k, _)| k == "u") {
                let v = v.into_owned();
                if v.starts_with("https://") || v.starts_with("http://") {
                    let _ = open::that(v);
                }
            }
        }
        _ => {}
    }
}

/* ---------------------------------------------------------------- installation */

async fn rapport(slug: &str, version: &str, ok: bool) {
    let corps = serde_json::json!({ "slug": slug, "version": version, "ok": ok, "os": systeme() });
    if let Ok(c) = reqwest::Client::builder().user_agent("RetroGemsLauncher/0.1").build() {
        let _ = c
            .post(format!("{SITE}/wp-json/retrogems/v1/launcher/rapport"))
            .json(&corps)
            .send()
            .await;
    }
}

async fn telecharger(
    app: &AppHandle,
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    slug: &str,
    titre: &str,
) -> Result<(), String> {
    let rep = client.get(url).send().await.map_err(es)?;
    if !rep.status().is_success() {
        return Err(format!("{} ({})", t("Téléchargement refusé", "Download refused", "Descarga rechazada"), rep.status()));
    }
    let total = rep.content_length().unwrap_or(0);
    let mut f = fs::File::create(dest).map_err(es)?;
    let mut recu: u64 = 0;
    let mut dernier: u64 = 101;
    let mut flux = rep.bytes_stream();
    while let Some(morceau) = flux.next().await {
        let m = morceau.map_err(es)?;
        f.write_all(&m).map_err(es)?;
        recu += m.len() as u64;
        if total > 0 {
            let pct = recu * 100 / total;
            if pct != dernier {
                dernier = pct;
                progres(app, slug, titre, "telechargement", pct as f64, "");
            }
        }
    }
    f.flush().map_err(es)?;
    Ok(())
}

fn extraire(fichier: &Path, dest: &Path) -> Result<(), String> {
    let nom = fichier.to_string_lossy().to_lowercase();
    if nom.ends_with(".zip") {
        let f = fs::File::open(fichier).map_err(es)?;
        let mut a = zip::ZipArchive::new(f).map_err(es)?;
        a.extract(dest).map_err(es)
    } else if nom.ends_with(".tar.gz") || nom.ends_with(".tgz") {
        let f = fs::File::open(fichier).map_err(es)?;
        let gz = flate2::read::GzDecoder::new(f);
        tar::Archive::new(gz).unpack(dest).map_err(es)
    } else {
        Err(t(
            "Format d'archive pas encore pris en charge : la page du projet s'ouvre pour l'installer à la main.",
            "Archive format not supported yet: the project page opens so you can install it by hand.",
            "Formato de archivo aún no compatible: se abre la página del proyecto para instalarlo a mano.",
        ))
    }
}

/// Cherche un fichier (ou un dossier .app) par son nom, sans tenir compte de la casse.
fn chercher(dir: &Path, nom: &str, profondeur: u32) -> Option<PathBuf> {
    let lecture = fs::read_dir(dir).ok()?;
    let mut sous = Vec::new();
    for e in lecture.flatten() {
        let p = e.path();
        let n = p
            .file_name()
            .map(|x| x.to_string_lossy().to_string())
            .unwrap_or_default();
        if n.eq_ignore_ascii_case(nom) {
            return Some(p);
        }
        if p.is_dir() && !n.to_lowercase().ends_with(".app") {
            sous.push(p);
        }
    }
    if profondeur == 0 {
        return None;
    }
    for s in sous {
        if let Some(r) = chercher(&s, nom, profondeur - 1) {
            return Some(r);
        }
    }
    None
}

/// Premier dossier .app trouvé (macOS).
fn premier_app(dir: &Path, profondeur: u32) -> Option<PathBuf> {
    let lecture = fs::read_dir(dir).ok()?;
    let mut sous = Vec::new();
    for e in lecture.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.to_string_lossy().to_lowercase().ends_with(".app") {
                return Some(p);
            }
            sous.push(p);
        }
    }
    if profondeur == 0 {
        return None;
    }
    for s in sous {
        if let Some(r) = premier_app(&s, profondeur - 1) {
            return Some(r);
        }
    }
    None
}

#[cfg(unix)]
fn rendre_executable(p: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(p, fs::Permissions::from_mode(0o755));
}
#[cfg(not(unix))]
fn rendre_executable(_p: &Path) {}

#[cfg(target_os = "macos")]
fn installer_dmg(dmg: &Path, dossier: &Path, app_nom: &str) -> Result<PathBuf, String> {
    let mnt = racine().join("montage");
    let _ = fs::remove_dir_all(&mnt);
    fs::create_dir_all(&mnt).map_err(es)?;
    let ok = Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-readonly", "-noautoopen", "-mountpoint"])
        .arg(&mnt)
        .arg(dmg)
        .status()
        .map_err(es)?;
    if !ok.success() {
        return Err(t("Impossible d'ouvrir le fichier .dmg.", "Could not open the .dmg file.", "No se pudo abrir el archivo .dmg."));
    }
    let source = if app_nom.is_empty() { premier_app(&mnt, 1) } else { chercher(&mnt, app_nom, 1).or_else(|| premier_app(&mnt, 1)) };
    let res = match source {
        Some(src) => {
            let cible = dossier.join(src.file_name().unwrap_or_default());
            let _ = fs::remove_dir_all(&cible);
            let st = Command::new("cp").arg("-R").arg(&src).arg(dossier).status().map_err(es)?;
            if st.success() {
                Ok(cible)
            } else {
                Err(t("Copie de l'application impossible.", "Could not copy the app.", "No se pudo copiar la aplicación."))
            }
        }
        None => Err(t("Aucune application trouvée dans le .dmg.", "No app found in the .dmg.", "No se encontró ninguna aplicación en el .dmg.")),
    };
    let _ = Command::new("hdiutil").arg("detach").arg(&mnt).arg("-quiet").status();
    res
}
#[cfg(not(target_os = "macos"))]
fn installer_dmg(_dmg: &Path, _dossier: &Path, _app_nom: &str) -> Result<PathBuf, String> {
    Err("dmg".to_string())
}

async fn installer(app: &AppHandle, slug: &str) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent("RetroGemsLauncher/0.1")
        .build()
        .map_err(es)?;
    progres(app, slug, slug, "preparation", 0.0, &t("Lecture de la recette…", "Reading the recipe…", "Leyendo la receta…"));
    let p: Value = client
        .get(format!(
            "{SITE}/wp-json/retrogems/v1/launcher/projet/{}?lg={}",
            urlencoding::encode(slug),
            langue()
        ))
        .send()
        .await
        .map_err(es)?
        .json()
        .await
        .map_err(es)?;
    let titre = p["titre"].as_str().unwrap_or(slug).to_string();
    let version = p["version"].as_str().unwrap_or("").to_string();
    let c = &p["cibles"][systeme()];
    if !c.is_object() {
        if let Some(u) = p["page"].as_str() {
            page(app, format!("location.href={}", js_texte(u)));
        }
        return Err(t(
            "Pas encore d'installation en 1 clic de ce projet sur ce système : suis le guide de sa page.",
            "No 1-click install of this project on this system yet: follow the guide on its page.",
            "Este proyecto aún no se instala en 1 clic en este sistema: sigue la guía de su página.",
        ));
    }
    let methode = c["methode"].as_str().unwrap_or("").to_string();
    let url = c["url"].as_str().unwrap_or("").to_string();
    let fichier = nom_sur(c["fichier"].as_str().unwrap_or("telechargement"));
    let exe = c["exe"].as_str().unwrap_or("").to_string();
    let app_nom = c["app"].as_str().unwrap_or("").to_string();
    let args: Vec<String> = c["args"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    if url.is_empty() {
        return Err(t("Fichier introuvable dans la recette.", "File missing from the recipe.", "Archivo no encontrado en la receta."));
    }
    if !["archive", "exe", "installeur", "appimage", "dmg"].contains(&methode.as_str()) {
        let _ = open::that(&url);
        progres(app, slug, &titre, "fini", 100.0, &t("Ouvert dans ton navigateur.", "Opened in your browser.", "Abierto en tu navegador."));
        return Ok(());
    }

    let dossier = dossier_jeux().join(nom_sur(slug));
    let temp = racine().join("telechargements");
    fs::create_dir_all(&temp).map_err(es)?;
    let fichier_local = temp.join(&fichier);
    telecharger(app, &client, &url, &fichier_local, slug, &titre).await?;
    progres(app, slug, &titre, "installation", 100.0, &t("Installation…", "Installing…", "Instalando…"));

    let _ = fs::remove_dir_all(&dossier);
    fs::create_dir_all(&dossier).map_err(es)?;
    let mut lancer = PathBuf::new();
    match methode.as_str() {
        "archive" => {
            if let Err(e) = extraire(&fichier_local, &dossier) {
                if let Some(u) = p["page"].as_str() {
                    page(app, format!("location.href={}", js_texte(u)));
                }
                return Err(e);
            }
            let cible = if systeme() == "macos" && !app_nom.is_empty() { app_nom.clone() } else { exe.clone() };
            let base = Path::new(&cible)
                .file_name()
                .map(|x| x.to_string_lossy().to_string())
                .unwrap_or_default();
            let direct = dossier.join(&cible);
            lancer = if !cible.is_empty() && direct.exists() {
                direct
            } else if !base.is_empty() {
                chercher(&dossier, &base, 4).unwrap_or_default()
            } else if systeme() == "macos" {
                premier_app(&dossier, 3).unwrap_or_default()
            } else {
                PathBuf::new()
            };
        }
        "exe" | "appimage" => {
            let d = dossier.join(&fichier);
            fs::rename(&fichier_local, &d).or_else(|_| fs::copy(&fichier_local, &d).map(|_| ())).map_err(es)?;
            lancer = d;
        }
        "installeur" => {
            let d = dossier.join(&fichier);
            fs::rename(&fichier_local, &d).or_else(|_| fs::copy(&fichier_local, &d).map(|_| ())).map_err(es)?;
            let _ = open::that(&d);
        }
        "dmg" => {
            lancer = installer_dmg(&fichier_local, &dossier, &app_nom)?;
        }
        _ => {}
    }
    let _ = fs::remove_file(&fichier_local);

    if lancer.as_os_str().len() > 0 {
        if lancer.is_dir() {
            // .app : rendre exécutables les binaires du paquet
            if let Ok(l) = fs::read_dir(lancer.join("Contents").join("MacOS")) {
                for e in l.flatten() {
                    rendre_executable(&e.path());
                }
            }
        } else {
            rendre_executable(&lancer);
        }
    }

    // Fichier du jeu à fournir par le joueur (sa propre copie)
    let mut note = t("Installé ! Clique sur Jouer.", "Installed! Click Play.", "¡Instalado! Pulsa Jugar.");
    let jr = &p["jeu_requis"];
    if jr["requis"].as_bool().unwrap_or(false) {
        let nom = jr["nom"].as_str().unwrap_or("").to_string();
        let mode = jr["mode"].as_str().unwrap_or("");
        let dest = jr["destination"].as_str().unwrap_or("");
        if mode != "dans_le_jeu" && !dest.is_empty() {
            let d = dossier.join(dest);
            let _ = fs::create_dir_all(&d);
            let _ = open::that(&d);
            note = format!(
                "{} {}",
                t("Installé ! Dépose le fichier de ta copie du jeu dans le dossier qui vient de s'ouvrir :", "Installed! Put the file from your copy of the game in the folder that just opened:", "¡Instalado! Coloca el archivo de tu copia del juego en la carpeta que se acaba de abrir:"),
                nom
            );
        } else if !nom.is_empty() {
            note = format!(
                "{} {}",
                t("Installé ! Au premier lancement, le jeu te demandera le fichier de ta copie :", "Installed! On first launch, the game will ask for the file from your copy:", "¡Instalado! En el primer inicio, el juego te pedirá el archivo de tu copia:"),
                nom
            );
        }
    }

    let mut r = lire_registre();
    r.retain(|i| i.slug != slug);
    r.push(Installe {
        slug: slug.to_string(),
        titre: titre.clone(),
        version: version.clone(),
        dossier: dossier.to_string_lossy().to_string(),
        lancer: lancer.to_string_lossy().to_string(),
        args,
    });
    ecrire_registre(&r);
    pousser_etat(app);
    progres(app, slug, &titre, "fini", 100.0, &note);
    rapport(slug, &version, true).await;
    Ok(())
}

fn lancer_jeu(i: &Installe) -> Result<(), String> {
    let p = PathBuf::from(&i.lancer);
    if i.lancer.is_empty() || !p.exists() {
        let _ = open::that(&i.dossier);
        return Err(t(
            "Ce jeu se lance depuis son propre raccourci : son dossier vient de s'ouvrir.",
            "This game starts from its own shortcut: its folder just opened.",
            "Este juego se inicia desde su propio acceso directo: su carpeta se acaba de abrir.",
        ));
    }
    if cfg!(target_os = "macos") && p.is_dir() {
        let mut c = Command::new("open");
        c.arg(&p);
        if !i.args.is_empty() {
            c.arg("--args").args(&i.args);
        }
        return c.spawn().map(|_| ()).map_err(es);
    }
    let dir = p.parent().map(|x| x.to_path_buf()).unwrap_or_else(|| PathBuf::from(&i.dossier));
    Command::new(&p).args(&i.args).current_dir(dir).spawn().map(|_| ()).map_err(es)
}

/// Appelée par la page (bouton Installer, Jouer…) : plus fiable qu'une navigation vers retrogems:// dans WebView2.
#[tauri::command]
fn lien(app: AppHandle, url: String) {
    if let Ok(u) = url.parse::<Url>() {
        gerer_lien(&app, &u);
    }
}

/* ---------------------------------------------------------------- démarrage */

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .invoke_handler(tauri::generate_handler![lien])
        .setup(|app| {
            let _ = fs::create_dir_all(dossier_jeux());
            let h = app.handle().clone();
            let h_nav = h.clone();
            let h_charge = h.clone();
            let adresse: Url = accueil().parse().expect("adresse");
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(adresse))
                .title("RetroGems Launcher")
                .inner_size(1360.0, 860.0)
                .min_inner_size(960.0, 620.0)
                .initialization_script(SCRIPT_PAGE)
                .on_navigation(move |u| {
                    if u.scheme() == "retrogems" {
                        gerer_lien(&h_nav, u);
                        return false;
                    }
                    true
                })
                .on_page_load(move |_w, charge| {
                    if matches!(charge.event(), PageLoadEvent::Finished) {
                        pousser_etat(&h_charge);
                    }
                })
                .build()?;

            #[cfg(any(target_os = "linux", target_os = "windows"))]
            {
                let _ = app.deep_link().register_all();
            }
            let h_lien = h.clone();
            app.deep_link().on_open_url(move |ev| {
                for u in ev.urls() {
                    gerer_lien(&h_lien, &u);
                }
            });
            if let Ok(Some(liens)) = app.deep_link().get_current() {
                for u in liens {
                    gerer_lien(&h, &u);
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("RetroGems Launcher n'a pas pu démarrer");
}
