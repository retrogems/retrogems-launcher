/* RetroGems Launcher : script ajouté à chaque page de retrogems.fr dans le programme.
   Garde l'en-tête et le pied du site, affiche les téléchargements,
   transforme « Installer » en « Jouer » pour les jeux installés et ouvre les liens externes dans le navigateur. */
(function () {
  if (!/(^|\.)retrogems\.fr$/.test(location.hostname)) return;
  if (window.__rglAppScript) return; window.__rglAppScript = 1;   /* v0.1.3 : le programme peut relancer ce script en fin de chargement */
  window.RGL_APP = true;
  /* v0.1.4 : au tout début du chargement, document.documentElement n'existe pas encore : avant, le script plantait ici et rien ne marchait dans le programme */
  try { document.cookie = 'rgapp=1; path=/; max-age=31536000; SameSite=Lax; Secure'; } catch (x) {}
  function langue() { var h = document.documentElement; var l = (h && h.getAttribute('lang')) || (/\/(en|es)\//.exec(location.pathname) || [])[1] || navigator.language || 'fr'; return String(l).slice(0, 2); }
  var L = langue();
  var T = function (fr, en, es) { return L === 'fr' ? fr : (L === 'es' ? es : en); };
  /* v0.1.6 : l'en-tête et le pied du site restent affichés dans le programme ; seuls l'invitation à installer l'appli et les blocs de téléchargement du programme sont cachés */
  var css = '.rg-pwa-feuille,.rgls-dlbox{display:none!important}'
    + '#rgapp{position:fixed;right:18px;bottom:18px;z-index:2147483000;display:flex;flex-direction:column;gap:10px;width:min(360px,calc(100vw - 36px));font-family:Quicksand,system-ui,sans-serif}'
    + '.rgapp-c{background:linear-gradient(160deg,#151b24,#0d1117);border:1px solid rgba(0,255,255,.4);border-radius:14px;padding:14px 16px;color:#e0e0e0;box-shadow:0 0 24px rgba(0,255,255,.18),0 18px 40px rgba(0,0,0,.6)}'
    + '.rgapp-c b{display:block;color:#fff;margin-bottom:4px}.rgapp-c p{margin:4px 0 0;font-size:.85rem;color:#c9d1d9;line-height:1.45}'
    + '.rgapp-b{height:6px;border-radius:4px;background:rgba(255,255,255,.1);overflow:hidden;margin-top:8px}.rgapp-b i{display:block;height:100%;width:0;background:linear-gradient(90deg,#00ffff,#ff00ff);transition:width .3s}'
    + '.rgapp-c.ko{border-color:#ff5c7a}.rgapp-c.ok{border-color:#ff00ff}.rgapp-c{position:relative;padding-right:52px!important}.rgapp-x{position:absolute!important;top:10px;right:10px;width:30px!important;height:30px!important;min-width:0!important;padding:0!important;margin:0!important;display:flex!important;align-items:center!important;justify-content:center!important;border-radius:50%!important;border:1px solid rgba(0,255,255,.6)!important;background:#0d1117!important;color:#fff!important;font-size:0!important;line-height:0!important;cursor:pointer}.rgapp-x svg{width:12px;height:12px;display:block;stroke:#fff}.rgapp-x:hover{background:rgba(0,255,255,.15)!important}'
    + '.rgapp-ok{color:#7dffb0!important;font-weight:700}.rgapp-st{margin-top:10px!important;color:#fff!important;font-weight:700}.rgapp-ol{margin:6px 0 0;padding-left:18px;font-size:.84rem;line-height:1.5;color:#c9d1d9}.rgapp-ol li{margin:0 0 6px}'
    + '.rgapp-btns{display:flex;flex-wrap:wrap;gap:8px;margin-top:12px}.rgapp-btns a{display:inline-flex;align-items:center;justify-content:center;height:36px;padding:0 14px;border-radius:999px;font:700 .85rem Quicksand,sans-serif;text-decoration:none!important}.rgapp-jouer{background:linear-gradient(90deg,#ff00ff,#7000ff);color:#fff!important;box-shadow:0 0 12px rgba(255,0,255,.4)}.rgapp-sec{border:1px solid rgba(0,255,255,.6);color:#0ff!important;background:#0d1117}'
    + '.rgapp-mes{position:fixed;left:18px;bottom:18px;z-index:2147482999;display:none;align-items:center;gap:8px;padding:10px 16px;border-radius:999px;border:1px solid #ff00ff;background:rgba(13,17,23,.92);color:#ffb3ff;font:700 .9rem Quicksand,sans-serif;cursor:pointer;box-shadow:0 0 16px rgba(255,0,255,.35)}'
    + '.rgapp-panneau{position:fixed;left:18px;bottom:70px;z-index:2147482999;width:min(380px,calc(100vw - 36px));max-height:60vh;overflow:auto;display:none;background:#0d1117;border:1px solid rgba(255,0,255,.5);border-radius:14px;padding:12px;box-shadow:0 18px 40px rgba(0,0,0,.6)}'
    + '.rgapp-panneau.on{display:block}.rgapp-l{display:flex;align-items:center;gap:8px;padding:8px;border-bottom:1px solid rgba(255,255,255,.06);color:#e0e0e0}.rgapp-l span{flex:1;min-width:0;font-weight:700;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}'
    + '.rgapp-l a{padding:5px 10px;border-radius:8px;border:1px solid rgba(0,255,255,.5);color:#0ff!important;text-decoration:none!important;font-size:.8rem;font-weight:700}.rgapp-l a.j{background:linear-gradient(90deg,#ff00ff,#7000ff);border-color:transparent;color:#fff!important}';
  function pret(f) { if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', f); else f(); }
  var boite = null, cartes = {}, mes = null, panneau = null;
  pret(function () {
    L = langue();
    try { document.documentElement.classList.add('rg-app'); } catch (x) {}
    var st = document.createElement('style'); st.textContent = css; document.head.appendChild(st);
    boite = document.createElement('div'); boite.id = 'rgapp'; document.body.appendChild(boite);
    mes = document.createElement('button'); mes.className = 'rgapp-mes'; mes.type = 'button'; document.body.appendChild(mes);
    panneau = document.createElement('div'); panneau.className = 'rgapp-panneau'; document.body.appendChild(panneau);
    mes.addEventListener('click', function () { panneau.classList.toggle('on'); });
    window.rglApp.marquer(); window.rglApp.liste();
    new MutationObserver(function () { window.rglApp.marquer(); }).observe(document.body, { childList: true, subtree: true });
  });
  function esc(t) { return String(t).replace(/[&<>"']/g, function (c) { return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]; }); }
  window.rglApp = {
    installes: {},
    etat: function (liste, ap) { var o = {}; (liste || []).forEach(function (x) { o[x.slug] = x; }); this.installes = o; if (ap) this.appareil = ap; this.marquer(); this.liste(); this.sync(); },
    /* v0.1.5 : les jeux installés sont envoyés à « Mes jeux » du compte retrogems.fr (si le joueur est connecté) */
    sync: function () {
      var self = this; if (!this.appareil) return;
      var I = this.installes, jeux = Object.keys(I).map(function (k) { return { slug: k, version: I[k].version || '', installe: 1 }; });
      var corps = JSON.stringify({ appareil: this.appareil, jeux: jeux, complet: true });
      try { if (sessionStorage.getItem('rglSync') === corps) return; } catch (x) {}
      fetch('/wp-admin/admin-ajax.php?action=rest-nonce', { credentials: 'same-origin' }).then(function (r) { return r.ok ? r.text() : ''; }).then(function (n) {
        n = (n || '').trim(); if (!/^[a-f0-9]{6,16}$/.test(n)) return;
        return fetch('/wp-json/retrogems/v1/launcher/jeux-site', { method: 'POST', credentials: 'same-origin', headers: { 'Content-Type': 'application/json', 'X-WP-Nonce': n }, body: corps }).then(function (r) {
          if (!r.ok) return; try { sessionStorage.setItem('rglSync', corps); } catch (x) {}
          if (document.querySelector('.rgd-vide') && /onglet=jeux|mes-jeux|my-games|mis-juegos/.test(location.href)) location.reload();
        });
      }).catch(function () {});
    },
    /* v0.1.5 : fin d'installation : carte qui reste affichée, avec la suite expliquée */
    fini: function (slug, titre, etapes, guide) {
      if (!boite) return;
      var c = cartes[slug]; if (!c) { c = document.createElement('div'); boite.appendChild(c); cartes[slug] = c; }
      c.className = 'rgapp-c ok';
      var li = (etapes || []).map(function (x) { return '<li>' + esc(x) + '</li>'; }).join('');
      c.innerHTML = '<button class="rgapp-x" type="button" aria-label="' + T('Fermer', 'Close', 'Cerrar') + '"><svg viewBox="0 0 24 24" fill="none" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg></button>'
        + '<b>' + esc(titre) + '</b><p class="rgapp-ok">✓ ' + T('Installé sur cet ordinateur', 'Installed on this computer', 'Instalado en este ordenador') + '</p>'
        + (li ? '<p class="rgapp-st">' + T('Avant de jouer :', 'Before you play:', 'Antes de jugar:') + '</p><ol class="rgapp-ol">' + li + '</ol>' : '')
        + '<div class="rgapp-btns"><a class="rgapp-jouer" href="retrogems://jouer/' + encodeURIComponent(slug) + '">▶ ' + T('Jouer', 'Play', 'Jugar') + '</a>'
        + '<a class="rgapp-sec" href="retrogems://dossier/' + encodeURIComponent(slug) + '">' + T('Ouvrir le dossier du jeu', 'Open the game folder', 'Abrir la carpeta del juego') + '</a>'
        + (guide ? '<a class="rgapp-sec" href="' + esc(guide) + '">' + T('Lire le guide', 'Read the guide', 'Leer la guía') + '</a>' : '') + '</div>';
      c.querySelector('.rgapp-x').onclick = function () { c.remove(); delete cartes[slug]; };
    },
    marquer: function () {
      var I = this.installes;
      document.querySelectorAll('a[href^="retrogems://installer/"]:not([data-rgl-maj])').forEach(function (a) {
        var slug = decodeURIComponent(a.getAttribute('href').split('/').pop());
        if (!I[slug]) return;
        a.setAttribute('href', 'retrogems://jouer/' + encodeURIComponent(slug));
        a.removeAttribute('data-rgl-installer'); a.title = T('Jouer', 'Play', 'Jugar');
        if (a.textContent.trim() !== '') { a.textContent = T('Jouer', 'Play', 'Jugar'); }
      });
    },
    liste: function () {
      if (!mes) return;
      var l = Object.keys(this.installes).map(function (k) { return this.installes[k]; }, this);
      mes.style.display = l.length ? 'inline-flex' : 'none';
      mes.textContent = '🎮 ' + T('Mes jeux installés', 'My installed games', 'Mis juegos instalados') + ' (' + l.length + ')';
      panneau.innerHTML = l.map(function (x) {
        var s = encodeURIComponent(x.slug);
        return '<div class="rgapp-l"><span title="' + esc(x.titre) + '">' + esc(x.titre) + '</span><a class="j" href="retrogems://jouer/' + s + '">' + T('Jouer', 'Play', 'Jugar') + '</a>'
          + '<a href="retrogems://dossier/' + s + '" title="' + T('Ouvrir le dossier', 'Open folder', 'Abrir carpeta') + '">📁</a>'
          + '<a href="retrogems://installer/' + s + '" data-rgl-maj title="' + T('Réinstaller ou mettre à jour', 'Reinstall or update', 'Reinstalar o actualizar') + '">↻</a>'
          + '<a href="retrogems://desinstaller/' + s + '" title="' + T('Désinstaller', 'Uninstall', 'Desinstalar') + '" onclick="return confirm(' + esc(JSON.stringify(T('Désinstaller ce jeu ?', 'Uninstall this game?', '¿Desinstalar este juego?'))) + ')">✕</a></div>';
      }).join('');
    },
    progres: function (slug, titre, etat, pct, msg) {
      if (!boite) return;
      var c = cartes[slug];
      if (!c) { c = document.createElement('div'); c.className = 'rgapp-c'; boite.appendChild(c); cartes[slug] = c; }
      var lib = { preparation: T('Préparation', 'Preparing', 'Preparando'), telechargement: T('Téléchargement', 'Downloading', 'Descargando'), installation: T('Installation', 'Installing', 'Instalando'), fini: T('Prêt à jouer', 'Ready to play', 'Listo para jugar'), erreur: T('Problème', 'Problem', 'Problema') }[etat] || etat;
      c.className = 'rgapp-c' + (etat === 'erreur' ? ' ko' : (etat === 'fini' ? ' ok' : ''));
      c.innerHTML = '<button class="rgapp-x" type="button" aria-label="' + T('Fermer', 'Close', 'Cerrar') + '"><svg viewBox="0 0 24 24" fill="none" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg></button><b>' + esc(titre) + '</b><p>' + esc(lib) + (etat === 'telechargement' ? ' · ' + Math.round(pct) + ' %' : '') + (msg ? ' · ' + esc(msg) : '') + '</p>'
        + (etat === 'telechargement' || etat === 'installation' ? '<div class="rgapp-b"><i style="width:' + Math.max(2, pct) + '%"></i></div>' : '')
        + (etat === 'fini' ? '<p><a href="retrogems://jouer/' + encodeURIComponent(slug) + '" style="color:#0ff;font-weight:700">▶ ' + T('Jouer', 'Play', 'Jugar') + '</a></p>' : '');
      c.querySelector('.rgapp-x').onclick = function () { c.remove(); delete cartes[slug]; };

    }
  };
  /* boutons du launcher (Installer, Jouer, Désinstaller…) : envoyés directement au programme */
  document.addEventListener('click', function (e) {
    var a = e.target && e.target.closest ? e.target.closest('a[href^="retrogems://"]') : null; if (!a) return;
    e.preventDefault(); e.stopImmediatePropagation();
    var h = a.getAttribute('href');
    if (/^retrogems:\/\/installer\//.test(h) && window.rglApp) { window.rglApp.progres(decodeURIComponent(h.split('/').pop()), a.getAttribute('data-rgl-titre') || '', 'preparation', 0, ''); }
    /* v0.1.3 : adresse https interceptée par le programme (fiable dans WebView2), sans dépendre de l'IPC */
    location.href = location.origin + '/rgl-action/?u=' + encodeURIComponent(h);
  }, true);
  /* liens externes (GitHub, sites des projets) : dans le navigateur de l'ordinateur ; liens du site : dans le launcher */
  document.addEventListener('click', function (e) {
    var a = e.target && e.target.closest ? e.target.closest('a[href]') : null; if (!a) return;
    var u; try { u = new URL(a.href, location.href); } catch (x) { return; }
    if (u.protocol !== 'http:' && u.protocol !== 'https:') return;
    var site = /(^|\.)retrogems\.fr$/.test(u.hostname);
    var connexion = /(^|\.)(google\.com|googleusercontent\.com|twitch\.tv|gstatic\.com)$/.test(u.hostname);
    if (site || connexion) { if (a.target === '_blank') { e.preventDefault(); location.href = u.href; } return; }
    e.preventDefault(); e.stopPropagation();
    location.href = location.origin + '/rgl-action/?u=' + encodeURIComponent('retrogems://ouvrir/?u=' + encodeURIComponent(u.href));
  }, true);
})();
