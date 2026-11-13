//! RetroGems (application pour ordinateur : le site retrogems.fr + installation des portages)
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
        "fr" => format!("{SITE}/"),   /* v0.2.0 : l'application RetroGems s'ouvre sur l'accueil du site */
        l => format!("{SITE}/{l}/"),
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

/// Identité de cet ordinateur pour « Mes jeux » sur retrogems.fr : clé tirée au hasard une fois, nom de l'ordinateur, système.
fn appareil() -> Value {
    let f = racine().join("appareil.txt");
    let mut cle = fs::read_to_string(&f).unwrap_or_default().trim().to_string();
    if cle.len() < 32 || !cle.chars().all(|c| c.is_ascii_hexdigit()) {
        use std::hash::{BuildHasher, Hasher};
        let mut c = String::new();
        for i in 0..4u64 {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u64(i);
            h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
            h.write_u32(std::process::id());
            c.push_str(&format!("{:016x}", h.finish()));
        }
        cle = c;
        let _ = fs::create_dir_all(racine());
        let _ = fs::write(&f, &cle);
    }
    let nom = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "PC".to_string());
    serde_json::json!({ "cle": cle, "nom": nom, "os": systeme() })
}

fn pousser_etat(app: &AppHandle) {
    let r = lire_registre();
    let j = serde_json::to_string(&r).unwrap_or_else(|_| "[]".to_string());
    page(app, format!("window.rglApp&&window.rglApp.etat({j},{})", appareil()));
}

/// Agent du navigateur intégré : celui d'Edge, suivi de RetroGemsLauncher (le site reconnaît ainsi le programme).
fn agent() -> String {
    let base = if cfg!(target_os = "windows") {
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0"
    } else if cfg!(target_os = "macos") {
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15"
    } else {
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15"
    };
    format!("{base} RetroGemsLauncher/{}", env!("CARGO_PKG_VERSION"))
}

/// https://retrogems.fr/rgl-action/?u=retrogems://installer/slug → retrogems://installer/slug
fn action_https(u: &Url) -> Option<Url> {
    let h = u.host_str().unwrap_or("");
    if (u.scheme() != "https" && u.scheme() != "http") || !(h == "retrogems.fr" || h.ends_with(".retrogems.fr")) {
        return None;
    }
    if !u.path().trim_end_matches('/').ends_with("/rgl-action") {
        return None;
    }
    let v = u.query_pairs().find(|(k, _)| k == "u").map(|(_, v)| v.into_owned())?;
    let l: Url = v.parse().ok()?;
    if l.scheme() == "retrogems" { Some(l) } else { None }
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
    if let Ok(c) = reqwest::Client::builder().user_agent(agent()).build() {
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
    /* v0.1.4 : même identité que le navigateur intégré (Cloudflare refuse les programmes inconnus) */
    let client = reqwest::Client::builder()
        .user_agent(agent())
        .build()
        .map_err(es)?;
    progres(app, slug, slug, "preparation", 0.0, &t("Lecture de la recette…", "Reading the recipe…", "Leyendo la receta…"));
    let rep_p = client
        .get(format!(
            "{SITE}/wp-json/retrogems/v1/launcher/projet/{}?lg={}",
            urlencoding::encode(slug),
            langue()
        ))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("{} : {e}", t("Site injoignable", "Site unreachable", "Sitio inaccesible")))?;
    let statut = rep_p.status();
    let texte = rep_p.text().await.map_err(es)?;
    if !statut.is_success() {
        return Err(format!("{} ({statut})", t("Le site a refusé la demande", "The site refused the request", "El sitio rechazó la solicitud")));
    }
    let p: Value = serde_json::from_str(&texte).map_err(|_| t("Réponse du site illisible.", "Unreadable site response.", "Respuesta del sitio ilegible."))?;
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

    // v0.1.5 : explication claire de la suite (fichier du jeu à fournir, premier lancement)
    let mut etapes: Vec<String> = Vec::new();
    let jr = &p["jeu_requis"];
    if jr["requis"].as_bool().unwrap_or(false) {
        let nom = jr["nom"].as_str().unwrap_or("").to_string();
        let formats: Vec<String> = jr["formats"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
        let fmt = if formats.is_empty() { String::new() } else { format!(" ({})", formats.join(", ")) };
        let mode = jr["mode"].as_str().unwrap_or("");
        let dest = jr["destination"].as_str().unwrap_or("");
        if mode != "dans_le_jeu" && !dest.is_empty() {
            let d = dossier.join(dest);
            let _ = fs::create_dir_all(&d);
            etapes.push(format!("{} {nom}{fmt} {}", t("Ce portage a besoin de ta propre copie du jeu :", "This port needs your own copy of the game:", "Este port necesita tu propia copia del juego:"), t("— clique sur « Ouvrir le dossier du jeu » et dépose le fichier dans le dossier qui s'ouvre.", "— click \"Open the game folder\" and put the file in the folder that opens.", "— pulsa «Abrir la carpeta del juego» y coloca el archivo en la carpeta que se abre.")));
        } else if !nom.is_empty() {
            etapes.push(format!("{} {nom}{fmt}.", t("Ce portage a besoin de ta propre copie du jeu : au premier lancement, il te demandera le fichier", "This port needs your own copy of the game: on first launch it will ask for the file", "Este port necesita tu propia copia del juego: en el primer inicio te pedirá el archivo")));
        }
        if let Some(aide) = jr["aide"][langue()].as_str().or_else(|| jr["aide"]["fr"].as_str()) {
            if !aide.trim().is_empty() { etapes.push(aide.trim().to_string()); }
        }
    }
    if let Some(n) = p["notes"].as_str() {
        if !n.trim().is_empty() { etapes.push(n.trim().to_string()); }
    }
    let guide = p["guide"].as_str().unwrap_or("").to_string();

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
    progres(app, slug, &titre, "fini", 100.0, "");
    page(app, format!("window.rglApp&&window.rglApp.fini({},{},{},{})", js_texte(slug), js_texte(&titre), serde_json::to_string(&etapes).unwrap_or_else(|_| "[]".to_string()), js_texte(&guide)));
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

/* ---------------------------------------------------------------- mise à jour automatique (v0.2.0) */

/// a > b (versions 1.2.3)
fn plus_recente(a: &str, b: &str) -> bool {
    let p = |s: &str| s.trim_start_matches(['v', 'V']).split('.').map(|x| x.trim().parse::<u64>().unwrap_or(0)).collect::<Vec<u64>>();
    let (x, y) = (p(a), p(b));
    for i in 0..3 {
        let (m, n) = (x.get(i).copied().unwrap_or(0), y.get(i).copied().unwrap_or(0));
        if m != n {
            return m > n;
        }
    }
    false
}

/// Cherche une nouvelle version sur retrogems.fr ; si elle existe, la télécharge, l'installe et relance l'application.
async fn verifier_maj(app: AppHandle) -> Result<(), String> {
    let client = reqwest::Client::builder().user_agent(agent()).build().map_err(es)?;
    /* pas de réseau ou site indisponible : on réessaiera plus tard, sans rien afficher */
    let rep = match client.get(format!("{SITE}/wp-json/retrogems/v1/launcher/appli")).header("Accept", "application/json").send().await {
        Ok(r) => r,
        Err(_) => return Ok(()),
    };
    let j: Value = match rep.json().await {
        Ok(j) => j,
        Err(_) => return Ok(()),
    };
    let v = j["version"].as_str().unwrap_or("").to_string();
    if v.is_empty() || !plus_recente(&v, env!("CARGO_PKG_VERSION")) {
        return Ok(());
    }
    let url = j["fichiers"][systeme()].as_str().unwrap_or("").to_string();
    if !url.starts_with("https://retrogems.fr/") {
        return Ok(());
    }
    let titre = format!("RetroGems {v}");
    let nom = nom_sur(url.rsplit('/').next().unwrap_or("maj"));
    let dossier = racine().join("maj");
    let _ = fs::remove_dir_all(&dossier);
    fs::create_dir_all(&dossier).map_err(es)?;
    let fichier = dossier.join(&nom);
    progres(&app, "__maj", &titre, "telechargement", 0.0, &t("Nouvelle version de l'application", "New version of the app", "Nueva versión de la aplicación"));
    telecharger(&app, &client, &url, &fichier, "__maj", &titre).await?;
    progres(&app, "__maj", &titre, "installation", 100.0, &t("Installation puis redémarrage…", "Installing, then restarting…", "Instalando y reiniciando…"));
    appliquer_maj(&app, &fichier)
}

#[cfg(target_os = "windows")]
fn appliquer_maj(app: &AppHandle, f: &Path) -> Result<(), String> {
    /* installateur NSIS : /P sans fenêtres, /UPDATE garde réglages et raccourcis, /R relance l'application */
    Command::new(f).args(["/P", "/UPDATE", "/R"]).spawn().map_err(es)?;
    app.exit(0);
    Ok(())
}
#[cfg(target_os = "linux")]
fn appliquer_maj(app: &AppHandle, f: &Path) -> Result<(), String> {
    let actuel = match std::env::var("APPIMAGE") {
        Ok(a) if !a.is_empty() => PathBuf::from(a),
        _ => {
            /* installé en .deb / .rpm : la mise à jour se fait par le paquet */
            let _ = open::that(format!("{SITE}/portages-recomp-decomp/#telecharger"));
            return Err(t("Nouvelle version disponible : installe le nouveau paquet.", "New version available: install the new package.", "Nueva versión disponible: instala el nuevo paquete."));
        }
    };
    let neuf = actuel.with_extension("nouveau");
    fs::copy(f, &neuf).map_err(es)?;
    rendre_executable(&neuf);
    fs::rename(&neuf, &actuel).map_err(es)?;
    Command::new(&actuel).spawn().map_err(es)?;
    app.exit(0);
    Ok(())
}
#[cfg(target_os = "macos")]
fn appliquer_maj(app: &AppHandle, f: &Path) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(es)?;
    let paquet = exe.ancestors().find(|p| p.extension().map(|e| e == "app").unwrap_or(false)).map(|p| p.to_path_buf()).ok_or_else(|| "app".to_string())?;
    let temp = racine().join("maj").join("app");
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&temp).map_err(es)?;
    let neuf = installer_dmg(f, &temp, "")?;
    let _ = fs::remove_dir_all(&paquet);
    let ok = Command::new("ditto").arg(&neuf).arg(&paquet).status().map(|s| s.success()).unwrap_or(false);
    if !ok {
        return Err(t("Mise à jour impossible : télécharge la nouvelle version sur retrogems.fr.", "Update failed: download the new version from retrogems.fr.", "Actualización imposible: descarga la nueva versión en retrogems.fr."));
    }
    Command::new("open").arg("-n").arg(&paquet).spawn().map_err(es)?;
    app.exit(0);
    Ok(())
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
                .title("RetroGems")
                .inner_size(1360.0, 860.0)
                .min_inner_size(960.0, 620.0)
                .initialization_script(SCRIPT_PAGE)
                .user_agent(&agent())
                .on_navigation(move |u| {
                    if u.scheme() == "retrogems" {
                        gerer_lien(&h_nav, u);
                        return false;
                    }
                    /* v0.1.3 : action du launcher envoyée par la page sous forme d'adresse https (WebView2 signale toujours ces navigations) */
                    if let Some(l) = action_https(u) {
                        gerer_lien(&h_nav, &l);
                        return false;
                    }
                    true
                })
                .on_page_load(move |w, charge| {
                    if matches!(charge.event(), PageLoadEvent::Finished) {
                        /* le script du launcher est remis à la fin du chargement, au cas où le script d'ouverture n'aurait pas été lancé */
                        let _ = w.eval(SCRIPT_PAGE);
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
            /* v0.2.0 : mise à jour automatique, 20 s après l'ouverture puis toutes les 6 heures */
            let h_maj = h.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(20));
                loop {
                    if let Err(e) = tauri::async_runtime::block_on(verifier_maj(h_maj.clone())) {
                        if !e.is_empty() {
                            progres(&h_maj, "__maj", "RetroGems", "erreur", 0.0, &e);
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_secs(6 * 3600));
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
        .expect("RetroGems n'a pas pu démarrer");
}
