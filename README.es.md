<div align="center">

[English](README.md) · **Español** · [Deutsch](README.de.md) · [Français](README.fr.md)

<img src="docs/icon.svg" width="128" alt="Icono de Battery Health Center: una batería con el nivel de carga en verde">

# Battery Health Center

**Todo sobre la batería de tu portátil, en un solo lugar.**
Salud, ciclos, límites y programación de la carga, y qué aplicaciones la gastan, para GNOME.

[![Última versión](https://img.shields.io/github/v/release/arios4k/battery-health-center?label=versi%C3%B3n&color=26a269)](https://github.com/arios4k/battery-health-center/releases/latest)
[![CI](https://github.com/arios4k/battery-health-center/actions/workflows/ci.yml/badge.svg)](https://github.com/arios4k/battery-health-center/actions/workflows/ci.yml)
[![Instalar con apt](https://img.shields.io/badge/install-apt-orange)](#instalación-ubuntu-debian-y-derivadas)
[![Instalar en Fedora](https://img.shields.io/badge/install-Fedora%20COPR-51a2da)](#instalación-fedora)
[![Licencia: GPL-3.0-or-later](https://img.shields.io/badge/licencia-GPL--3.0--or--later-blue)](LICENSE)
[![Idiomas](https://img.shields.io/badge/idiomas-EN%20%C2%B7%20ES%20%C2%B7%20DE%20%C2%B7%20FR-lightgrey)](#traducciones)
[![Hecho con Rust y GTK4](https://img.shields.io/badge/made%20with-Rust%20%2B%20GTK4-informational)](#compilar-desde-el-código-fuente)

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/overview-dark.png">
  <img src="docs/screenshots/overview-light.png" alt="La página Resumen de Battery Health Center: un anillo de carga al 77 % con el tiempo restante, el resumen de la salud de la batería y lo ocurrido desde la última carga">
</picture>

</div>

## Una sola aplicación en lugar de un montón de scripts

Linux te da los datos de la batería, pero dispersos: una terminal llena de `upower` y archivos de `/sys`, una
extensión del shell para el límite de carga, `powertop` para adivinar qué aplicaciones gastan. Battery Health
Center los reúne en una aplicación nativa de GNOME, parecida a los ajustes de batería de macOS:

| En lugar de… | Battery Health Center te ofrece |
|---|---|
| leer `/sys/class/power_supply` y `upower -i` a mano | **Salud**: capacidad máxima frente a la capacidad de diseño, ciclos, estado y una tendencia con la fecha prevista para llegar al 80 % |
| un script o una extensión para fijar el límite de carga | **Carga**: perfiles de un toque, un límite personalizado, una programación de carga y la carga optimizada |
| adivinar qué aplicación gasta la batería | **Uso**: aplicaciones ordenadas por energía, con vatios estimados y una vista hora a hora de cada una |
| rebuscar en Ajustes, Retoques y extensiones las opciones de energía | **Energía**: modo de energía, modo automático y los ajustes de ahorro de energía de GNOME en una sola página |
| una hoja de cálculo con mediciones | **Informes**: un informe de batería en HTML y exportaciones CSV de tu historial |

## Características

### 🔋 Conoce de verdad la salud de tu batería

La capacidad máxima comparada con la de la batería nueva, su estado (Normal, Desgastada o Se recomienda
revisión), los ciclos, el voltaje, el fabricante, el modelo y la química. La capacidad se registra una vez al
día, así que tras unas dos semanas obtienes una tendencia y la fecha prevista para llegar al 80 %. Si el
porcentaje da saltos o el portátil se apaga antes de tiempo, una calibración guiada (cargar, descargar casi por
completo, cargar de nuevo) corrige la lectura. Se admiten portátiles con dos baterías.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/health-dark.png">
  <img src="docs/screenshots/health-light.png" alt="La página Salud: capacidad máxima 85 %, estado, capacidad a carga completa y capacidad de diseño, y capacidad a lo largo del tiempo">
</picture>

### 🛡️ Haz que la batería dure años

Las baterías de litio se desgastan más rápido cuando permanecen al 100 %. Elige un perfil, **Máxima duración
(60 %)**, **Equilibrado (80 %)** o **Capacidad completa (100 %)**, o fija exactamente dónde se detiene y se
reanuda la carga. El límite se mantiene tras reiniciar y tras suspender. ¿Necesitas la batería llena para un
viaje? **Cargar al 100 % ahora** carga por completo una sola vez y luego tu límite vuelve solo.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/charging-dark.png">
  <img src="docs/screenshots/charging-light.png" alt="La página Carga con el perfil Equilibrado (80 %), los puntos de parada y reanudación, y el interruptor Mantener tras reiniciar">
</picture>

### 🗓️ Llena cuando la necesitas, protegida el resto del tiempo

Dile cuándo necesitas el portátil, por ejemplo **de lunes a viernes a las 07:30**, y el límite se levanta poco
antes para que la batería esté llena a tiempo y luego se restablece. O activa la **carga optimizada** y aprende
tu rutina según cuándo sueles desenchufar. Introduces tu contraseña una sola vez, al guardar la programación;
un pequeño servicio hace el resto.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/schedule-dark.png">
  <img src="docs/screenshots/schedule-light.png" alt="La sección Programación de carga con la carga optimizada activada, lo que ha aprendido hasta ahora y dos horas de carga completa">
</picture>

### 📊 Mira qué la está gastando

Nivel de batería y consumo del último día o de la última semana, uso de la batería mientras el portátil estaba
suspendido (con un aviso cuando es inusualmente alto) y las aplicaciones que más energía usan ahora mismo, desde
que abriste la aplicación, o en 24 horas y 7 días. Haz clic en una aplicación para ver su impacto energético y su
uso estimado hora a hora. Linux no mide la potencia por aplicación, así que las cifras son estimaciones basadas
en el tiempo de CPU y el consumo medido por encima del reposo.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/usage-dark.png">
  <img src="docs/screenshots/usage-light.png" alt="La página Uso: historial del nivel de batería con el límite de carga, y uso de la batería durante la suspensión">
</picture>

<div align="center">
<img src="docs/screenshots/app-trend-dark.png" width="360" alt="El uso de energía de una aplicación hora a hora, que se abre al hacer clic en ella en la página Uso">
</div>

### ⚡ Modos de energía sin buscarlos

Ahorro de energía, Equilibrado o Rendimiento, con un **modo de energía automático** que cambia al conectar o
desconectar el cargador, además de los ajustes de ahorro de energía de GNOME (atenuar la pantalla, apagar la
pantalla, suspensión automática, botón de encendido y tapa) y el porcentaje de batería en la barra superior. Solo
se muestra lo que admite tu hardware.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/power-dark.png">
  <img src="docs/screenshots/power-light.png" alt="La página Energía: modo de energía actual, modo de energía automático y ajustes de ahorro de energía de GNOME">
</picture>

### 💡 Pequeños empujones, sin agobiar

Mientras funcionas con batería, el Resumen puede sugerirte bajar una pantalla muy brillante, apagar la
retroiluminación del teclado o desactivar el Bluetooth cuando no hay nada conectado, cada cosa con un botón. Las
notificaciones te avisan cuando se alcanza el límite de carga, cuando está completamente cargada sin límite,
cuando la batería está baja, cuando una aplicación la gasta y cuando el portátil gastó mucha batería suspendido.
Puedes desactivarlas en Preferencias (salvo los pasos de la calibración).

### 📤 Llévate tus datos

Exporta un informe de batería en HTML (para una reclamación de garantía o cuando vendas el portátil) desde el
menú, o ejecuta `battery-health-center --export-report report.html`. **Exportar datos como CSV** (o
`battery-health-center --export-csv <carpeta>`) escribe el historial de la batería, la capacidad a lo largo del
tiempo, las sesiones de suspensión y el uso de energía de cada aplicación por hora como archivos CSV para una
hoja de cálculo.

## Por qué Battery Health Center

- **Nativa y ligera.** Rust con GTK 4 y libadwaita: un programa de 1,2 MB (un paquete de 0,5 MB), modo claro y
  oscuro, y sigue tu tema de GNOME.
- **Privada por diseño.** Todo se queda en tu equipo: la aplicación nunca se conecta a internet y no tiene
  telemetría. El historial son unos pocos archivos CSV en tu carpeta personal.
- **Cuidadosa con root.** Solo un pequeño ayudante se ejecuta como root. Acepta únicamente valores validados y
  escribe solo los archivos de límite de carga de la batería y su propia configuración.
- **Habla tu idioma.** Inglés, español, alemán y francés, según el idioma de tu sistema.
- **Se actualiza con tu sistema.** Instálala desde un repositorio apt firmado (o el COPR de Fedora) y tus
  actualizaciones habituales la mantienen al día.

## Preguntas frecuentes

**¿Funcionará el límite de carga en mi portátil?**
La información de la batería funciona en cualquier portátil con un controlador de batería estándar de Linux. Los
límites de carga necesitan que el firmware lo permita y que el kernel lo exponga como `charge_control_end_threshold`
(o `charge_stop_threshold`), como en los ThinkPad y muchos portátiles ASUS, Dell, Huawei, LG, Samsung y System76.
Si el tuyo no lo tiene, la página Carga lo indica y todo lo demás sigue funcionando.

**¿Qué escritorios y distribuciones?**
Se desarrolla y prueba en GNOME: Ubuntu 24.04, Zorin OS 18 y Linux Mint 22. El paquete necesita GTK 4.14 y
libadwaita 1.5 o posteriores, así que Ubuntu 22.04, Debian 12 y Mint 21 son demasiado antiguos. La página Energía
usa los ajustes propios de GNOME, por lo que es la parte más ligada a GNOME.

**¿Es seguro cambiar el límite de carga?**
Fija los mismos umbrales del firmware que usan otras herramientas y opciones de la BIOS, y **Capacidad completa
(100 %)** vuelve a quitar el límite. Cambiarlo pide tu contraseña.

**¿Tengo que dejar la aplicación abierta?**
El historial, el consumo en suspensión y las notificaciones se registran mientras la aplicación está en
ejecución. La primera vez que la abres pregunta si quieres **registrar el historial en segundo plano**; di que
sí y se inicia oculta al iniciar sesión. Puedes cambiar de opinión cuando quieras en la página Uso.

<sub>Capturas realizadas en Zorin OS 18 con su tema ZorinBlue. Regénéralas con
`docs/screenshots/capture.sh`.</sub>

## Instalación (Ubuntu, Debian y derivadas)

Para Ubuntu 24.04+, Debian 13+, Linux Mint 22, Zorin OS 18 y otras derivadas, en Intel/AMD de 64 bits:

```bash
sudo install -d -m 0755 /etc/apt/keyrings
curl -fsSL https://arios4k.github.io/apt/key.asc | sudo tee /etc/apt/keyrings/battery-health-center.asc > /dev/null
echo "deb [signed-by=/etc/apt/keyrings/battery-health-center.asc] https://arios4k.github.io/apt stable main" | sudo tee /etc/apt/sources.list.d/battery-health-center.list
sudo apt update && sudo apt install battery-health-center
```

Después abre **Battery Health Center** desde tus aplicaciones. Las actualizaciones llegan con
`sudo apt upgrade`. La huella de la clave de firma es `F090 D01D 74F7 14C8 5A95 23CA 4773 367E BD97 6CF5`, y el
repositorio (con consejos para resolver problemas) está en [arios4k/apt](https://github.com/arios4k/apt).

## Instalación (Fedora)

Para Fedora 43 y posteriores, en Intel/AMD de 64 bits, desde el [repositorio COPR](https://copr.fedorainfracloud.org/coprs/arios/battery-health-center/):

```bash
sudo dnf copr enable arios/battery-health-center
sudo dnf install battery-health-center
```

Después abre **Battery Health Center** desde tus aplicaciones. Las actualizaciones llegan con
`sudo dnf upgrade`. La instalación activa los servicios de límite de carga y programación, así que no hay que
configurar nada más.

## Si vienes del snap

Si instalaste una versión anterior como snap, quítala primero con `sudo snap remove battery-health-center`. Eso
borra su historial, así que copia `~/snap/battery-health-center/current/.local/share/battery-health-center/*.csv`
a `~/.local/share/battery-health-center/` antes de hacerlo.

## Compilar desde el código fuente

Requisitos: Rust, GTK ≥ 4.14, libadwaita ≥ 1.5 y `gettext` para las traducciones.

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev pkg-config gettext   # Debian/Ubuntu
cargo run --release
```

Cambiar el límite de carga usa `pkexec` para ejecutar `bhc-helper` desde la carpeta de compilación, así que pide
tu contraseña cada vez hasta que la aplicación esté instalada. Para instalarla en todo el sistema, ejecuta
`./install.sh` (quítala con `./install.sh --uninstall`), o construye un paquete con
`scripts/build-locale.sh && cargo deb`.

Útil durante el desarrollo:

```bash
# probar una traducción
scripts/build-locale.sh && LANGUAGE=es BHC_LOCALEDIR=$PWD/locale cargo run --release

# probar las pantallas de la programación de la carga sin tocar tus ajustes reales: apunta la aplicación a una
# carpeta con un battery-health-center.conf de prueba (una línea como "BAT0 75 80") y, opcionalmente, un
# battery-health-center.schedule y un unplug-log
BHC_HELPER_FILES=/path/to/folder cargo run --release

# guardar una captura de cada página
BHC_SCREENSHOTS=/path/to/folder cargo run --release
```

## Traducciones

La aplicación está disponible en inglés, español (`es`), alemán (`de`) y francés (`fr`). Usa el idioma de tu
sistema y recurre al inglés para lo que no esté traducido. El informe de batería HTML exportado sigue estando
solo en inglés. Este README y [Cómo funciona](docs/HOW-IT-WORKS.es.md) también están disponibles en
[English](README.md), [Deutsch](README.de.md) y [Français](README.fr.md).

Para añadir un idioma, por ejemplo italiano:

```bash
msginit --locale=it --input=po/battery-health-center.pot --output=po/it.po
```

Traduce las cadenas de `po/it.po` (ayuda un editor de PO como Poedit o Gtranslator de GNOME), luego añade `it` a
`po/LINGUAS` y una línea para él junto a las demás en `[package.metadata.deb]` de `Cargo.toml` (los archivos
`.mo`), y compruébalo con el comando anterior. Los pull requests con idiomas nuevos son muy bienvenidos.
`Comment[it]` y `Keywords[it]` en `data/io.github.BatteryHealthCenter.desktop` y un `<summary xml:lang="it">` en
el archivo metainfo traducen cómo se presenta la aplicación.

Cuando cambia el texto de la aplicación, actualiza la plantilla y las traducciones (las cadenas sin traducir
aparecen vacías y se muestran en inglés):

```bash
python3 scripts/update-pot.py
msgmerge --update --backup=none po/es.po po/battery-health-center.pot   # y los demás idiomas
```

Las cadenas del código pasan por `tr("…")`, `trf("… {name}", &[("name", value)])` para texto con valores, y
`trn`/`trnf` para plurales; consulta `src/i18n.rs`.

## Más documentación

- [Cómo funciona](docs/HOW-IT-WORKS.es.md): de dónde salen los datos, cómo funcionan el límite de carga, la
  programación y las estimaciones de energía de las aplicaciones.
- [Mantenimiento y empaquetado](docs/MAINTAINING.md) (en inglés): publicar una versión y notas para las
  distribuciones que quieran incluir la aplicación. Serán bienvenidas y me sentiría honrado.
- [El repositorio apt](https://github.com/arios4k/apt): pasos de instalación, solución de problemas y cómo se
  construye.

## Contribuir

```bash
cargo test
cargo clippy
```

Los informes de errores, las traducciones y las pruebas en otros portátiles y distribuciones son especialmente
bienvenidos: por favor [abre una incidencia](https://github.com/arios4k/battery-health-center/issues) con el
modelo de tu portátil y tu distribución.

## Autor

Creado por **Alexis Rios-Negron**.

## Licencia

GPL-3.0-or-later © Alexis Rios-Negron. Consulta [LICENSE](LICENSE).
