# RetroGems Launcher

Programme PC du launcher RetroGems (Windows, Linux / Steam Deck, macOS), fait avec Tauri 2.

- La fenêtre affiche l'interface du launcher de retrogems.fr : bibliothèque, RetroGems Studio, connexion, votes.
- Le programme installe, met à jour, lance et désinstalle les portages dont la recette est vérifiée et activée dans Lewis.
- Les boutons « Installer » de retrogems.fr ouvrent le programme (liens retrogems://).
- Jeux installés dans : Windows `%LOCALAPPDATA%\RetroGems Launcher\jeux`, Linux `~/.local/share/RetroGems Launcher/jeux`, macOS `~/Library/Application Support/RetroGems Launcher/jeux`.

## Fabrication

Tout est automatique : à chaque envoi sur la branche `main`, GitHub fabrique gratuitement les installateurs
(fichier `.github/workflows/fabriquer.yml`) et les publie dans « Releases » sous le numéro de `version`
(`src-tauri/tauri.conf.json`). Pour une nouvelle version, augmente ce numéro (0.1.0 → 0.1.1) avant l'envoi.

retrogems.fr copie ensuite les installateurs sur son propre serveur (Lewis › 🚀 Launcher › Réglages › Dépôt du launcher)
et les propose sur https://retrogems.fr/launcher/.
