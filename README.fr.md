<div align="center">

[English](README.md) · [Español](README.es.md) · [Deutsch](README.de.md) · **Français** · [简体中文](README.zh-CN.md)

<img src="docs/icon.svg" width="128" alt="Icône de Battery Health Center : une batterie bleue avec une cellule turquoise et une pulsation blanche de santé">

# Battery Health Center

**Tout sur la batterie de votre ordinateur portable, au même endroit.**
Santé, nombre de cycles, limites et programmation de la charge, et applications qui la vident, pour GNOME.

[![Dernière version](https://img.shields.io/github/v/release/arios4k/battery-health-center?label=version&color=26a269)](https://github.com/arios4k/battery-health-center/releases/latest)
[![CI](https://github.com/arios4k/battery-health-center/actions/workflows/ci.yml/badge.svg)](https://github.com/arios4k/battery-health-center/actions/workflows/ci.yml)
[![Manuel](https://img.shields.io/badge/manuel-lire-2ea44f)](https://arios4k.github.io/battery-health-center/manual/)
[![Téléchargements](https://img.shields.io/github/downloads/arios4k/battery-health-center/total?color=26a269)](https://github.com/arios4k/battery-health-center/releases)
[![Installer avec apt](https://img.shields.io/badge/install-apt-orange)](#installation-ubuntu-debian-et-dérivées)
[![Installer sur Fedora](https://img.shields.io/badge/install-Fedora%20COPR-51a2da)](#installation-fedora)
[![Compilation COPR](https://copr.fedorainfracloud.org/coprs/arios/battery-health-center/package/battery-health-center/status_image/last_build.png)](https://copr.fedorainfracloud.org/coprs/arios/battery-health-center/builds/)
[![Licence : GPL-3.0-or-later](https://img.shields.io/badge/licence-GPL--3.0--or--later-blue)](LICENSE)
[![Langues](https://img.shields.io/badge/langues-EN%20%C2%B7%20ES%20%C2%B7%20DE%20%C2%B7%20FR%20%C2%B7%20ZH-lightgrey)](#traductions)
[![Fait avec Rust et GTK4](https://img.shields.io/badge/made%20with-Rust%20%2B%20GTK4-informational)](#compiler-depuis-les-sources)
[![Soutenir sur PayPal](https://img.shields.io/badge/PayPal-Support-00457C?logo=paypal&logoColor=white)](https://paypal.me/AlexisRios)

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/v0.5.0/overview-dark.png">
  <img src="docs/screenshots/v0.5.0/overview-light.png" alt="Battery Health Center 0.5.0 : navigation latérale, état et santé de la batterie dans un tableau de bord adaptatif">
</picture>

</div>

## Une seule application au lieu d’un tas de scripts

Linux fournit les chiffres de la batterie, mais éparpillés : un terminal plein de `upower` et de fichiers
`/sys`, une extension du shell pour la limite de charge, `powertop` pour deviner quelles applications
consomment. Battery Health Center les réunit dans une application GNOME native, un peu comme les réglages de
batterie de macOS :

| Au lieu de… | Battery Health Center vous offre |
|---|---|
| lire `/sys/class/power_supply` et `upower -i` à la main | **Santé** : capacité maximale par rapport à la capacité d’origine, nombre de cycles, état, et une tendance avec la date prévue pour atteindre 80 % |
| un script ou une extension pour fixer la limite de charge | **Charge** : des profils en un geste, une limite personnalisée, une programmation de la charge et la charge optimisée |
| deviner quelle application vide la batterie | **Utilisation** : les applications classées par énergie, avec la puissance estimée en watts et une vue heure par heure pour chacune |
| fouiller Paramètres, Ajustements et les extensions pour les options d’énergie | **Énergie** : mode d’alimentation, mode automatique et réglages d’économie d’énergie de GNOME sur une seule page |
| un tableur de mesures | **Rapports** : un rapport de batterie HTML et des exports CSV de votre historique |

## Manuel

**Pour apprendre à utiliser Battery Health Center, lisez le [manuel de Battery Health Center](https://arios4k.github.io/battery-health-center/manual/)** (en anglais). Il explique chaque page pas à pas : santé et calibrage, limites de charge et programmation de la charge, utilisation des applications, modes d’alimentation, notifications, rapports et vos données, avec le dépannage et la liste des nouveautés de chaque version. Il s’ouvre aussi depuis l’application avec **F1** ou le bouton d’aide.

## Pourquoi Battery Health Center

- **Natif et léger.** Rust avec GTK 4 et libadwaita : un programme de 1,2 Mo (un paquet de 0,5 Mo), mode clair et
  sombre, et il suit votre thème GNOME.
- **Respectueux de la vie privée.** Tout reste sur votre ordinateur : l’application ne se connecte jamais à
  internet et n’a aucune télémétrie. L’historique tient dans quelques fichiers CSV de votre dossier personnel.
- **Prudent avec root.** Seul un petit assistant s’exécute en tant que root. Il n’accepte que des valeurs
  validées et n’écrit que les fichiers de limite de charge de la batterie et sa propre configuration.
- **Parle votre langue.** Anglais, espagnol, allemand, français et chinois simplifié, selon la langue de votre système.
- **Se met à jour avec votre système.** Installez-le depuis un dépôt apt signé (ou le COPR de Fedora) et vos
  mises à jour habituelles le tiennent à jour.

D’autres réponses, par exemple pour savoir si la limite de charge fonctionne sur votre ordinateur, se trouvent dans la page [Troubleshooting and FAQ](https://arios4k.github.io/battery-health-center/manual/troubleshooting.html#faq) du manuel.

<sub>Captures d’écran réalisées sur Zorin OS 18 avec son thème ZorinBlue. Régénérez-les avec
`docs/screenshots/capture.sh`.</sub>

## Installation (Ubuntu, Debian et dérivées)

Pour Ubuntu 24.04+, Debian 13+, Linux Mint 22, Zorin OS 18 et autres dérivées, sur Intel/AMD 64 bits :

```bash
sudo install -d -m 0755 /etc/apt/keyrings
curl -fsSL https://arios4k.github.io/apt/key.asc | sudo tee /etc/apt/keyrings/battery-health-center.asc > /dev/null
echo "deb [signed-by=/etc/apt/keyrings/battery-health-center.asc] https://arios4k.github.io/apt stable main" | sudo tee /etc/apt/sources.list.d/battery-health-center.list
sudo apt update && sudo apt install battery-health-center
```

Ouvrez ensuite **Battery Health Center** depuis vos applications. Les mises à jour arrivent avec
`sudo apt upgrade`. L’empreinte de la clé de signature est `F090 D01D 74F7 14C8 5A95 23CA 4773 367E BD97 6CF5`,
et le dépôt (avec des conseils de dépannage) se trouve sur [arios4k/apt](https://github.com/arios4k/apt).

## Installation (Fedora)

Pour Fedora 43 et suivantes, sur Intel/AMD 64 bits, depuis le [dépôt COPR](https://copr.fedorainfracloud.org/coprs/arios/battery-health-center/) :

```bash
sudo dnf copr enable arios/battery-health-center
sudo dnf install battery-health-center
```

Ouvrez ensuite **Battery Health Center** depuis vos applications. Les mises à jour arrivent avec
`sudo dnf upgrade`. Les services de limite de charge et de programmation sont activés par l’installation : il n’y
a rien d’autre à configurer.

## Passer depuis le snap

Si vous avez installé une ancienne version sous forme de snap, supprimez-la d’abord avec
`sudo snap remove battery-health-center`. Cela efface son historique : copiez donc
`~/snap/battery-health-center/current/.local/share/battery-health-center/*.csv` vers
`~/.local/share/battery-health-center/` avant de le faire.

## Compiler depuis les sources

Prérequis : Rust, GTK ≥ 4.14, libadwaita ≥ 1.5, et `gettext` pour les traductions.

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev pkg-config gettext   # Debian/Ubuntu
cargo run --release
```

Changer la limite de charge utilise `pkexec` pour lancer `bhc-helper` depuis le dossier de compilation : il
demande donc votre mot de passe à chaque fois tant que l’application n’est pas installée. Pour l’installer pour
tout le système, lancez `./install.sh` (supprimez-la avec `./install.sh --uninstall`), ou construisez un paquet
avec `scripts/build-locale.sh && cargo deb`.

Pratique pendant le développement :

```bash
# essayer une traduction
scripts/build-locale.sh && LANGUAGE=fr BHC_LOCALEDIR=$PWD/locale cargo run --release

# essayer les écrans de programmation de la charge sans toucher à vos vrais réglages : pointez l’application vers
# un dossier contenant un battery-health-center.conf factice (une ligne comme « BAT0 75 80 »), et
# éventuellement un battery-health-center.schedule et un unplug-log
BHC_HELPER_FILES=/path/to/folder cargo run --release

# enregistrer une capture d’écran de chaque page
BHC_SCREENSHOTS=/path/to/folder cargo run --release
```

## Traductions

L’application est disponible en anglais, espagnol (`es`), allemand (`de`), français (`fr`) et chinois simplifié (`zh_CN`). Elle utilise la
langue de votre système et revient à l’anglais pour tout ce qui n’est pas traduit. Le rapport de batterie HTML
exporté reste uniquement en anglais. Ce README et [Comment ça marche](docs/HOW-IT-WORKS.fr.md) existent aussi en
[English](README.md), [Español](README.es.md), [Deutsch](README.de.md) et [简体中文](README.zh-CN.md).

Pour ajouter une langue, par exemple l’italien :

```bash
msginit --locale=it --input=po/battery-health-center.pot --output=po/it.po
```

Traduisez les chaînes de `po/it.po` (un éditeur PO comme Poedit ou Gtranslator de GNOME aide), puis ajoutez `it`
à `po/LINGUAS` et une ligne pour cette langue à côté des autres dans `[package.metadata.deb]` de `Cargo.toml`
(les fichiers `.mo`), et vérifiez avec la commande ci-dessus. Les pull requests ajoutant de nouvelles langues
sont les bienvenues. `Comment[it]` et `Keywords[it]` dans `data/io.github.BatteryHealthCenter.desktop` et un
`<summary xml:lang="it">` dans le fichier metainfo traduisent la façon dont l’application est présentée.

Quand le texte de l’application change, mettez à jour le modèle et les traductions (les chaînes pas encore
traduites apparaissent vides et s’affichent en anglais) :

```bash
python3 scripts/update-pot.py
msgmerge --update --backup=none po/fr.po po/battery-health-center.pot   # et les autres langues
```

Les chaînes du code passent par `tr("…")`, `trf("… {name}", &[("name", value)])` pour le texte avec des valeurs,
et `trn`/`trnf` pour les pluriels ; voir `src/i18n.rs`.

## Plus de documentation

- [Manuel d’utilisation](https://arios4k.github.io/battery-health-center/manual/) (en anglais) : un guide détaillé de chaque fonction, avec des captures d’écran. Ouvrez-le
  depuis l’application avec le bouton d’aide ou `F1`.
- [Comment ça marche](docs/HOW-IT-WORKS.fr.md) : d’où viennent les chiffres, comment fonctionnent la limite de
  charge, la programmation et les estimations d’énergie des applications.
- [Maintenance et empaquetage](docs/MAINTAINING.md) (en anglais) : publier une version, et notes pour les
  distributions qui veulent inclure l’application. Vous y êtes les bienvenus, et j’en serais honoré.
- [Le dépôt apt](https://github.com/arios4k/apt) : étapes d’installation, dépannage et fonctionnement.

## Contribuer

```bash
cargo test
cargo clippy
```

Les rapports de bogues, les traductions et les tests sur d’autres ordinateurs et distributions sont
particulièrement bienvenus : [ouvrez un ticket](https://github.com/arios4k/battery-health-center/issues) en
indiquant le modèle de votre ordinateur et votre distribution.

## Auteur

Créé par **Alexis Rios-Negron**.

## Licence

GPL-3.0-or-later © Alexis Rios-Negron. Voir [LICENSE](LICENSE).
