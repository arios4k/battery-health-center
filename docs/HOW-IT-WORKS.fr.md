# Comment fonctionne Battery Health Center

Les détails derrière l’application, pour les curieux et les contributeurs. Retour au [README](../README.fr.md). Aussi en [English](HOW-IT-WORKS.md), [Español](HOW-IT-WORKS.es.md) et [Deutsch](HOW-IT-WORKS.de.md).

- **Les données de la batterie** sont lues directement dans `/sys/class/power_supply/BAT*`. Les pilotes basés
  sur l’énergie (µWh) comme ceux basés sur la charge (µAh) sont pris en charge.
- **Les limites de charge** sont écrites dans `charge_control_start_threshold` et `charge_control_end_threshold`
  par `bhc-helper`, la seule partie qui s’exécute en tant que root. Il n’utilise que la bibliothèque standard de
  Rust (plus libc pour l’heure locale), n’accepte que des arguments validés, et n’écrit que ces fichiers sysfs,
  ses propres fichiers de configuration dans `/etc` (la limite enregistrée et la programmation de la charge) et,
  pour la charge optimisée, une liste d’heures de débranchement dans `/var/lib/battery-health-center`. Si vous
  activez **Conserver après un redémarrage**, le service systemd réapplique la limite au démarrage et après la
  sortie de veille. Pris en charge par les ThinkPad, de nombreux ordinateurs ASUS, Dell, Huawei, LG, Samsung et
  System76, et d’autres dotés d’un pilote noyau pour les seuils.
- **La programmation de la charge** nécessite une limite de charge enregistrée, car elle lève cette limite
  jusqu’à 100 % à partir du délai d’avance (2 heures par défaut) avant chaque heure choisie, jusqu’à 30 minutes
  après, puis la rétablit. Un service systemd root (`battery-health-center-scheduler`, lancé par une unité
  `.path` uniquement tant qu’une programmation existe) s’en charge sans demander de mot de passe à chaque fois.
  La **charge optimisée** enregistre aussi le moment où vous débranchez après que l’ordinateur est resté branché
  au moins 3 heures. Dès qu’elle a vu au moins 3 débranchements de ce type au cours des 28 derniers jours, en
  semaine (ou le week-end), espacés de moins d’une heure et demie, elle utilise le début de cette plage comme
  heure de disponibilité. Rien ne quitte votre ordinateur.
- **La consommation d’énergie des applications** est une estimation, car Linux ne mesure pas la puissance par
  application. Le temps processeur est échantillonné depuis `/proc`. Les processus sont regroupés en
  applications grâce au scope systemd dans lequel GNOME lance chaque application, ou à défaut par le processus
  parent pour les applications qui déplacent leurs processus ailleurs (comme les navigateurs basés sur
  Chromium). Sur batterie, la puissance mesurée au-dessus du niveau de repos est répartie entre les
  applications selon leur utilisation du processeur.
- **Le mode d’alimentation** utilise l’API D-Bus de power-profiles-daemon, la même que celle des réglages
  rapides de GNOME.
- **L’historique** est enregistré dans `~/.local/share/battery-health-center/history.csv` une fois par minute
  tant que l’application fonctionne. Activez **Enregistrer l’historique en arrière-plan** pour la démarrer
  masquée à l’ouverture de session.
- **Mises à jour et copie masquée.** La copie d’arrière-plan lancée à l’ouverture de session continuerait à
  exécuter l’ancienne version après qu’une mise à jour a remplacé le fichier du programme. Elle s’en aperçoit
  (Linux marque le fichier remplacé comme supprimé) et, dès qu’aucune fenêtre n’est ouverte, elle démarre la
  nouvelle version puis se termine. Elle vérifie une fois par minute : la nouvelle version tourne donc environ
  une minute après la mise à jour.
- **Espaces de travail adaptatifs** : Vue d’ensemble, Santé et Utilisation s’affichent sur deux colonnes dans les
  grandes fenêtres. Charge et Énergie utilisent une seule colonne centrée pour garder les réglages dans l’ordre de
  lecture. En dessous de 820 pixels logiques de largeur d’espace de travail, les colonnes du tableau de bord
  s’empilent ; sous 600 pixels logiques de largeur de fenêtre, la barre latérale devient une navigation de retour.
- **Échecs d’enregistrement** : ils apparaissent dans une bannière persistante avec le chemin concerné et l’erreur.
  Les préférences sont remplacées de façon atomique et restaurées en mémoire si l’enregistrement échoue. Les
  entrées de santé et les heures d’utilisation des applications terminées sont réessayées une fois le stockage
  rétabli. Les échantillons de batterie restent disponibles en mémoire si leur écriture échoue.
- **L’icône de l’application** est livrée depuis `data/icons/hicolor/scalable/apps/io.github.BatteryHealthCenter.svg` ;
  `docs/icon.svg` en est la copie pour la documentation. L’entrée de bureau, les invites PolicyKit, les
  notifications et la boîte « À propos » utilisent le même identifiant d’application. Les compilations depuis les
  sources chargent directement l’icône des sources dans la boîte « À propos », les notifications et la liste
  d’utilisation des applications, afin qu’une ancienne icône installée ne la remplace pas.
