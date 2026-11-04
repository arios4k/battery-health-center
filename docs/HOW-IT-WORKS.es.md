# Cómo funciona Battery Health Center

Los detalles detrás de la aplicación, para los curiosos y para quienes contribuyen. Volver al [README](../README.es.md). También en [English](HOW-IT-WORKS.md), [Deutsch](HOW-IT-WORKS.de.md), [Français](HOW-IT-WORKS.fr.md) y [简体中文](HOW-IT-WORKS.zh-CN.md).

- **Los datos de la batería** se leen directamente de `/sys/class/power_supply/BAT*`. Se admiten tanto los
  controladores basados en energía (µWh) como los basados en carga (µAh).
- **Los límites de carga** los escribe `bhc-helper` en `charge_control_start_threshold` y
  `charge_control_end_threshold`; es la única parte que se ejecuta como root. Solo usa la biblioteca estándar de
  Rust (más libc para la hora local), acepta únicamente argumentos validados y escribe solo esos archivos sysfs,
  sus propios archivos de configuración en `/etc` (el límite guardado y la programación de la carga) y, para la
  carga optimizada, una lista de horas de desconexión en `/var/lib/battery-health-center`. Si activas **Mantener
  tras reiniciar**, el servicio de systemd vuelve a aplicar el límite al arrancar y tras reanudar. Lo admiten los
  ThinkPad, muchos ASUS, Dell, Huawei, LG, Samsung y System76, y otros con un controlador del kernel para los
  umbrales.
- **La programación de la carga** necesita un límite de carga guardado, porque levanta ese límite al 100 % desde
  la antelación (2 horas por defecto) antes de cada hora que fijes, hasta 30 minutos después, y luego lo
  restablece. Un servicio root de systemd (`battery-health-center-scheduler`, iniciado por una unidad `.path`
  solo mientras exista una programación) lo hace sin pedir contraseña cada vez. La **carga optimizada**
  también registra cuándo desconectas después de que el portátil haya estado enchufado al menos 3 horas. Cuando
  ha visto al menos 3 desconexiones así en los últimos 28 días, entre semana (o en fin de semana), separadas
  por menos de una hora y media, usa el extremo más temprano de ese intervalo como hora de disponibilidad. Nada
  sale de tu equipo.
- **El uso de energía de las aplicaciones** es una estimación, porque Linux no mide la potencia por aplicación.
  El tiempo de CPU se muestrea de `/proc`. Los procesos se agrupan en aplicaciones mediante el scope de systemd
  en el que GNOME lanza cada aplicación, recurriendo al proceso padre para las aplicaciones que mueven sus
  procesos a otra parte (como los navegadores basados en Chromium). Con batería, la potencia medida por encima
  del nivel de reposo se reparte entre las aplicaciones según su uso de CPU.
- **El modo de energía** usa la API D-Bus de power-profiles-daemon, la misma que usan los ajustes rápidos de
  GNOME.
- **El historial** se guarda en `~/.local/share/battery-health-center/history.csv` una vez por minuto mientras
  la aplicación está en ejecución. Activa **Registrar historial en segundo plano** para iniciarla oculta al
  iniciar sesión.
- **Actualizaciones y la copia oculta.** La copia en segundo plano iniciada al abrir sesión seguiría ejecutando
  la versión antigua después de que una actualización reemplace el archivo del programa. Lo detecta (Linux marca
  el archivo reemplazado como eliminado) y, cuando no hay ninguna ventana abierta, inicia la nueva versión y
  termina. Comprueba una vez por minuto, así que la nueva versión está en marcha en aproximadamente un minuto
  tras la actualización.
- **Espacios de trabajo adaptables**: Resumen, Salud y Uso usan dos columnas en ventanas grandes. Carga y Energía
  usan una sola columna centrada para mantener los ajustes en orden de lectura. Por debajo de 820 píxeles lógicos
  de ancho del espacio de trabajo, las columnas del panel se apilan; por debajo de 600 píxeles lógicos de ancho de
  ventana, la barra lateral pasa a ser navegación de retroceso.
- **Fallos al guardar**: aparecen en un banner persistente con la ruta afectada y el error. Las preferencias se
  reemplazan de forma atómica y se restauran en memoria si falla el guardado. Las entradas de salud y las horas
  de uso de aplicaciones completadas se reintentan cuando el almacenamiento se recupera. Las muestras de batería
  siguen disponibles en memoria si falla su escritura.
- **El icono de la aplicación** se distribuye desde `data/icons/hicolor/scalable/apps/io.github.BatteryHealthCenter.svg`;
  `docs/icon.svg` es la copia equivalente para la documentación. La entrada de escritorio, los avisos de
  PolicyKit, las notificaciones y el diálogo Acerca de usan el mismo ID de aplicación. Las compilaciones desde el
  código fuente cargan directamente el icono del código en el diálogo Acerca de, las notificaciones y la lista de
  uso de aplicaciones, para que un icono instalado más antiguo no lo sustituya.
