#!/usr/bin/env python3
"""Builds the static user manual (docs/index.html and docs/manual/*.html).

Run from anywhere: python3 scripts/build-manual.py
Screenshots live in docs/manual/img (regenerate them with scripts/capture-manual.sh).
The published site is GitHub Pages serving the docs/ folder of the main branch.
"""
import html
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
DOCS = ROOT / "docs"
SITE = "https://arios4k.github.io/battery-health-center/"
REPO = "https://github.com/arios4k/battery-health-center"
VERSION = next(
    line.split('"')[1] for line in (ROOT / "Cargo.toml").read_text().splitlines() if line.startswith("version")
)

# (slug, nav title, one-line summary)
PAGES = [
    ("getting-started", "Getting started", "Install the app, open it for the first time and find your way around."),
    ("overview", "Overview", "Your battery, charger and recommendations at a glance."),
    ("health", "Health", "Capacity, wear, cycle count, the long-term trend and calibration."),
    ("charging", "Charging", "Charge limits, profiles, one-time full charges and pausing charging."),
    ("schedule", "Charging schedule", "Full when you need it, protected the rest of the time."),
    ("power", "Power", "Power mode, automatic power mode and GNOME's power-saving settings."),
    ("usage", "Usage", "Battery history, sleep drain and which apps use the most energy."),
    ("preferences", "Notifications and preferences", "Choose which notifications you get and when."),
    ("data", "Reports and your data", "HTML battery report, CSV export and where everything is stored."),
    ("reference", "Reference", "Keyboard shortcuts, command line, files and a glossary."),
    ("troubleshooting", "Troubleshooting and FAQ", "Answers to the most common questions and problems."),
    ("privacy", "Privacy and security", "What stays on your computer and what the root helper can do."),
    ("whats-new", "What's new", "What changed in every release, with links to the full explanation."),
]
TITLES = {slug: title for slug, title, _ in PAGES}


def e(text):
    return html.escape(text, quote=True)


def fig(name, alt, caption, narrow=False):
    cls = ' class="narrow"' if narrow else ""
    return (
        f"<figure{cls}><picture>"
        f'<source media="(prefers-color-scheme: dark)" srcset="@IMG@{name}-dark.png">'
        f'<img src="@IMG@{name}-light.png" alt="{e(alt)}" loading="lazy">'
        f"</picture><figcaption>{caption}</figcaption></figure>"
    )


def note(kind, text):
    label = {"tip": "Tip", "note": "Note", "warn": "Careful", "hw": "Hardware"}[kind]
    return f'<aside class="callout {kind}"><strong>{label}.</strong> {text}</aside>'


def steps(*items):
    return "<ol class=\"steps\">" + "".join(f"<li>{i}</li>" for i in items) + "</ol>"


def table(head, rows):
    out = "<div class=\"table-wrap\"><table><thead><tr>" + "".join(f"<th scope=\"col\">{h}</th>" for h in head) + "</tr></thead><tbody>"
    for row in rows:
        out += "<tr>" + "".join(f"<td>{c}</td>" for c in row) + "</tr>"
    return out + "</tbody></table></div>"


# --------------------------------------------------------------------------- content

# What's new: one entry per release, newest first. Add the new release at the top when you publish it.
# (version, date, [items as HTML: bold lead-in, then what changed and where to read more])
WHATS_NEW = [
    ("0.6.5", "November 9, 2026", [
        "<strong>Help opens the right page.</strong> The help button, <kbd>F1</kbd> and <em>User Manual</em> now open the manual page that matches the page you are on: on Charging they open the Charging page. See <a href=\"getting-started.html#layout\">Finding your way around</a>.",
        "<strong>What's new.</strong> This page lists what changed in every release.",
        "<strong>A friendlier manual on phones.</strong> The mobile menu keeps the icon and title in a header with a Close button, and the manual uses the same design as the Voltkit and ThermalPrint manuals.",
    ]),
    ("0.6.4", "November 8, 2026", [
        "<strong>Help button and user manual.</strong> A question-mark button at the top of the sidebar, <kbd>F1</kbd> and <em>User Manual</em> in the main menu open this manual in your browser. See <a href=\"getting-started.html#layout\">Finding your way around</a>.",
    ]),
    ("0.6.3", "November 8, 2026", [
        "<strong>About dialog fits without scrolling.</strong> It is more compact, and its Voltkit link now goes to the Voltkit source page, where the README is.",
    ]),
    ("0.6.2", "November 7, 2026", [
        "<strong>More Linux Apps.</strong> The About dialog links to Voltkit and ThermalPrint, with their official icons.",
    ]),
    ("0.6.1", "November 6, 2026", [
        "<strong>Screen reader support.</strong> The charge ring and the charts are named and described (the latest value and what the colors mean), level bars announce their value, icon-only buttons have names and decorative icons are hidden. See <a href=\"getting-started.html#screen-readers\">Accessibility</a>.",
        "<strong>Clearer charts.</strong> Chart colors keep at least 3:1 contrast on light and dark cards, and periods on the charger get a strip along the bottom of the chart so they do not rely on color alone. The dark theme's accent keeps 4.5:1 with white button text. See <a href=\"usage.html#chart\">Battery level and power</a>.",
    ]),
    ("0.6.0", "November 4, 2026", [
        "<strong>French and Simplified Chinese.</strong> The app is now available in English, Spanish, German, French and Simplified Chinese, following your system language.",
        "<strong>Translated documentation.</strong> The README and <em>How it works</em> are available in Spanish, German, French and Simplified Chinese.",
    ]),
    ("0.5.1", "November 3, 2026", [
        "<strong>Cleaner sidebar.</strong> The duplicate app name row at the top of the sidebar is gone.",
    ]),
    ("0.5.0", "November 3, 2026", [
        "<strong>A refreshed app.</strong> New navigation with a sidebar, adaptive workspaces, a modern battery icon and a branded About dialog. See <a href=\"getting-started.html#layout\">Finding your way around</a>.",
        "<strong>Dashboard columns.</strong> Overview, Health and Usage use two columns in a large window, while Charging and Power use one centered column so settings stay in reading order. The columns stack in a narrow window.",
        "<strong>Charging status at a glance.</strong> Cards show the active charge limit, the next scheduled full charge and any temporary override. See <a href=\"charging.html#status\">Charging status</a>.",
        "<strong>Clearer energy numbers.</strong> The app explains how the app energy estimates work, and empty charts say what to do. See <a href=\"usage.html#apps\">Apps using the most energy</a>.",
        "<strong>Failed saves are reported.</strong> A banner names the file and the error, preferences are kept if saving fails, and health and app-usage history are retried once storage recovers. See <a href=\"data.html#stored\">Where your data is stored</a>.",
    ]),
    ("0.4.5", "November 1, 2026", [
        "<strong>Links in About.</strong> The About dialog links to the project website and the issue tracker.",
    ]),
    ("0.4.4", "November 1, 2026", [
        "<strong>First-launch question.</strong> The first time you open the app it asks whether to record history in the background, so history is not lost when the window is closed. The answer is remembered, and people who already had settings are not asked. See <a href=\"getting-started.html#first-launch\">Your first launch</a>.",
    ]),
    ("0.4.3", "October 28, 2026", [
        "<strong>Updates reach the hidden copy.</strong> The background copy started at login used to keep running the old version after an update. Now, once the program file has been replaced and no window is open, it starts the new version and exits. See <a href=\"usage.html#background\">Record history in the background</a>.",
    ]),
    ("0.4.2", "October 28, 2026", [
        "<strong>Housekeeping.</strong> Code that only existed for the discontinued snap package was removed. Nothing changes for the apt and Fedora packages.",
    ]),
    ("0.4.1", "October 27, 2026", [
        "<strong>Clearer labels.</strong> <em>Current Mode</em> on the Power page, <em>Notify At</em> in Preferences, and <em>Charging Behavior</em> with a <em>While Plugged In</em> row on the Charging page. See <a href=\"charging.html#behavior\">Charging behavior</a>.",
    ]),
    ("0.4.0", "October 27, 2026", [
        "<strong>Charge profiles.</strong> Maximum Lifespan (60%), Balanced (80%) and Full Capacity (100%). See <a href=\"charging.html#limit\">Charge limit</a>.",
        "<strong>Charging schedule and optimized charging.</strong> The limit is lifted shortly before the times you choose, or before the times learned from when you usually unplug, and put back afterwards, without asking for a password each time. See <a href=\"schedule.html\">Charging schedule</a>.",
        "<strong>App detail.</strong> Click an app on the Usage page to see its energy impact and estimated energy use hour by hour over a day or a week. See <a href=\"usage.html#detail\">App detail</a>.",
        "<strong>CSV export.</strong> <em>Export Data as CSV</em> in the menu and the <code>--export-csv</code> option. See <a href=\"data.html#csv\">Export data as CSV</a>.",
        "<strong>Smarter recommendations.</strong> While on battery, the Overview suggests turning down a bright screen, turning off the keyboard backlight, or turning off Bluetooth when nothing is connected. See <a href=\"overview.html#recommendations\">Recommendations</a>.",
    ]),
    ("0.3.3", "October 26, 2026", [
        "<strong>Spanish and German.</strong> The app follows the system language and shows English for anything not yet translated. The application menu entry and its description are translated too.",
    ]),
    ("0.3.2", "October 26, 2026", [
        "<strong>Crash fix.</strong> Changing Screen Blank or Automatic Suspend no longer crashes the app.",
        "<strong>Fits the ring.</strong> The charge ring says \"Not Charging\" instead of the wider \"Plugged In, Not Charging\".",
    ]),
    ("0.3.1", "October 26, 2026", [
        "<strong>Power Mode fix.</strong> Choosing a mode in the dropdown is no longer undone a moment later.",
    ]),
    ("0.3.0", "October 26, 2026", [
        "<strong>The Power page.</strong> Power mode, automatic power mode, GNOME's power-saving settings, and the power button and lid actions. See <a href=\"power.html\">Power</a>.",
        "<strong>Debian and Ubuntu packages.</strong> Charge limits ask for a password through PolicyKit, and a system service re-applies a saved limit at boot and after sleep. See <a href=\"getting-started.html#install\">Install</a>.",
    ]),
]

CONTENT = {}

CONTENT["getting-started"] = f"""
<p>Battery Health Center is a native GNOME app for the battery in your laptop. It shows how healthy the
battery is, lets you protect it with a charge limit and a schedule, and shows which apps are draining it.
This page gets you from zero to a running app.</p>

<h2 id="requirements">What you need</h2>
<ul>
<li><strong>A laptop with a battery</strong> that Linux can read. Battery information works with any standard Linux
battery driver. Without a battery the app shows a "No Battery Found" message.</li>
<li><strong>GNOME</strong> with GTK 4.14 and libadwaita 1.5 or newer. It is developed and tested on Ubuntu 24.04,
Zorin OS 18 and Linux Mint 22, and on Fedora 43 and newer. Ubuntu 22.04, Debian 12 and Mint 21 are too old.</li>
<li><strong>Charge-limit support (optional).</strong> Setting a charge limit needs firmware that the Linux kernel exposes as
<code>charge_control_end_threshold</code> (or <code>charge_stop_threshold</code>). This is common on ThinkPads and
many ASUS, Dell, Huawei, LG, Samsung and System76 laptops. When your laptop does not have it, the Charging page says
"Not supported on this laptop" and everything else keeps working.</li>
</ul>

<h2 id="install">Install</h2>
<h3>Ubuntu, Debian and derivatives (apt)</h3>
<p>For Ubuntu 24.04+, Debian 13+, Linux Mint 22, Zorin OS 18 and other derivatives on 64-bit Intel/AMD:</p>
<pre><code>sudo install -d -m 0755 /etc/apt/keyrings
curl -fsSL https://arios4k.github.io/apt/key.asc | sudo tee /etc/apt/keyrings/battery-health-center.asc &gt; /dev/null
echo "deb [signed-by=/etc/apt/keyrings/battery-health-center.asc] https://arios4k.github.io/apt stable main" | sudo tee /etc/apt/sources.list.d/battery-health-center.list
sudo apt update &amp;&amp; sudo apt install battery-health-center</code></pre>
<p>Updates arrive with <code>sudo apt upgrade</code>. The signing key fingerprint is
<code>F090 D01D 74F7 14C8 5A95 23CA 4773 367E BD97 6CF5</code>.</p>
<h3>Fedora (COPR)</h3>
<pre><code>sudo dnf copr enable arios/battery-health-center
sudo dnf install battery-health-center</code></pre>
<p>Updates arrive with <code>sudo dnf upgrade</code>. The charge-limit and schedule services are turned on by the
installation, so nothing else needs setting up.</p>
<h3>From source</h3>
<p>See <a href="{REPO}#build-from-source">Build from source</a> in the README.</p>
{note("note", "If you installed an older version as a snap, remove it first with <code>sudo snap remove battery-health-center</code>. That deletes its history, so copy <code>~/snap/battery-health-center/current/.local/share/battery-health-center/*.csv</code> to <code>~/.local/share/battery-health-center/</code> before you do.")}

<h2 id="first-launch">Your first launch</h2>
<p>Open <strong>Battery Health Center</strong> from your applications. The first time, the app asks whether it should
<strong>record history in the background</strong>.</p>
{steps("Choose <strong>Turn On</strong> if you want battery history, sleep drain and notifications to keep working with the window closed. The app then starts hidden when you log in.",
       "Choose <strong>Not Now</strong> to skip it. History is then only recorded while the window is open. You can change your mind any time with the <em>Record History in the Background</em> switch on the <a href=\"usage.html\">Usage</a> page.")}
<p>Some pages need time to fill with data. Nothing is wrong when a chart is empty at first:</p>
{table(["What", "How long until it appears"], [
    ["Battery level and power charts", "A sample is saved once a minute while the app runs. The chart starts the moment you open the app."],
    ["Apps using significant energy", "A few seconds after you open the app."],
    ["Capacity trend and the date the battery reaches 80%", "At least 5 daily readings spread over 14 days. The capacity is logged once a day, so open the app on different days or turn on background recording."],
    ["Optimized charging", "At least 3 unplugs after a long charge on weekdays (and again on weekends) in the last 28 days."],
    ["Sleep drain", "After the laptop sleeps on battery for at least 15 minutes while the app was running before and after."],
])}

<h2 id="layout">Finding your way around</h2>
{fig("overview", "The Overview page: a sidebar with Overview, Health, Charging, Power and Usage, the help and main menu buttons at the top of the sidebar, and a two-column dashboard.", "The main window. The sidebar on the left switches between the five pages; the question-mark button opens this manual and the menu button opens the main menu.")}
<ul>
<li><strong>The sidebar</strong> has the five pages: <a href="overview.html">Overview</a>, <a href="health.html">Health</a>,
<a href="charging.html">Charging</a>, <a href="power.html">Power</a> and <a href="usage.html">Usage</a>. Hover a page to read
a one-line description of it.</li>
<li><strong>The help button</strong> (a question mark, at the top of the sidebar) opens this manual in your browser, at the page that matches the page you are on: on Charging it opens the Charging page. You can also
press <kbd>F1</kbd> or choose <em>User Manual</em> from the main menu.</li>
<li><strong>The main menu</strong> (three lines) has <em>Refresh</em>, <em>Export Battery Report…</em>, <em>Export Data as CSV…</em>,
<em>Preferences</em>, <em>User Manual</em>, <em>About Battery Health Center</em> and <em>Quit</em>.</li>
<li><strong>Adaptive layout.</strong> Overview, Health and Usage use two columns in a large window. Charging and Power use one
centered column so the settings stay in reading order. Below 820 logical pixels of width the columns stack, and below
600 pixels the sidebar becomes a back-navigation list, so the app also works in a narrow window.</li>
<li><strong>Light and dark.</strong> The app follows your GNOME appearance and accent color. In high-contrast mode it keeps the
system palette.</li>
<li><strong>Save problems.</strong> If a preference or the history cannot be saved (for example, the disk is full), a banner at
the top says so. Choose <em>Details</em> to see which file and why. The app retries history writes when storage recovers.</li>
</ul>
{fig("about", "The About dialog with the app icon, version, project links, links to Voltkit and ThermalPrint, and the author and license.", "The About dialog: copy the version number for a bug report, open the project page, report an issue, or read the license.", narrow=True)}

<h2 id="screen-readers">Accessibility</h2>
<p>The charge ring and the charts are exposed to screen readers as images with a name and a description (the latest value
and what the colors mean). Level bars announce their value, icon-only buttons have names, and decorative icons are hidden.
Chart colors keep at least 3:1 contrast on light and dark cards, periods on the charger are marked by a strip along the bottom of the
chart so color is never the only signal, and the dark theme's accent keeps 4.5:1 with white button text.</p>
"""

CONTENT["overview"] = f"""
<p>Overview is the page the app opens on. It answers "how is my battery doing right now?" without making you hunt for it.</p>
{fig("overview", "The Overview page showing 100 percent fully charged on the left, and on the right the battery health and two recommendations.", "The Overview page. The left column is about right now; the right column is about the battery's long-term health and what you could do better.")}

<h2 id="ring">The charge ring</h2>
{fig("overview-ring", "A green ring showing 100 percent and Full, with the headline Fully Charged and the line Connected to power.", "The ring shows the charge level. The headline and the line under it say what the battery is doing.", narrow=True)}
<p>The percentage is in the middle. The headline underneath is one of:</p>
{table(["Headline", "Meaning"], [
    ["<strong>Charging</strong>", "The charger is connected and the battery is filling up. Below it you see the time left until full (or until your limit), for example \"1 h 10 min until 80%\", or the charging power in watts."],
    ["<strong>On Battery</strong>", "The laptop is running from the battery. The line below estimates the time remaining at the current usage; while it is still measuring it says \"Estimating time remaining…\"."],
    ["<strong>Plugged In, Not Charging</strong> / <strong>Charging On Hold</strong>", "The charger is connected but charging is held on purpose, usually because the charge limit has been reached. The line below says \"Charge limit of 80% protects your battery\"."],
    ["<strong>Fully Charged</strong>", "The battery is at 100% and the charger is connected."],
    ["<strong>Charging to 100%</strong>", "A one-time or scheduled full charge is running. Your limit comes back automatically."],
])}
<p>The time estimates are based on how fast the battery has been charging or draining recently, so they settle after a minute or two.</p>

<h2 id="power-source">Power source</h2>
{fig("overview-power-source", "A card named Power Source showing USB-C Power Delivery at 65 W with 20 V times 3.25 A.", "The Power Source card names the charger and its rating.", narrow=True)}
<p>When a charger is connected, this card names it (<em>AC adapter</em> or <em>USB-C Power Delivery</em>) and shows its
rating in watts, with volts and amps when the laptop reports them. Chargers that do not report a rating say "This charger
doesn't report its power rating". The card also appears on battery and shows <strong>Last Seen Plugged In</strong>.</p>
<p>Three more groups can show up on this page depending on the situation:</p>
<ul>
<li><strong>Since Last Charge</strong> (on battery): how long ago you unplugged, how much of the battery you used, and the average draw in watts.</li>
<li><strong>Other Devices</strong>: the battery level of connected wireless devices such as a mouse or a keyboard, when Linux reports it.</li>
<li><strong>Two batteries</strong>: laptops with two batteries are supported.</li>
</ul>

<h2 id="health">Battery health summary</h2>
{fig("overview-health", "A card named Battery Health with Maximum Capacity 87 percent with a bar, Condition Normal, and Cycle Count 214.", "A short health summary. The full picture is on the Health page.", narrow=True)}
<p>Maximum capacity compared with when the battery was new, its condition, and its cycle count. Open the
<a href="health.html">Health</a> page for explanations, the trend and calibration.</p>

<h2 id="recommendations">Recommendations</h2>
{fig("overview-recommendations", "Two recommendations: Limit charging to 80 percent with a Set Up button, and High drain during sleep with a Details button.", "Each recommendation says what it noticed and, where it can, has a button that fixes it.", narrow=True)}
<p>Recommendations are small, practical suggestions based on the current state. They only appear while they are true, and
when there is nothing to suggest the card says <strong>Everything looks good</strong>.</p>
{table(["Recommendation", "When it appears", "Button"], [
    ["Battery capacity is down to N%", "Maximum capacity is below 80%. Calibrating can correct a wrong reading; if it stays low, consider a replacement.", "<em>Calibrate</em> opens the calibration on the Health page."],
    ["Limit charging to 80%", "The laptop is plugged in and no charge limit is set, and your battery supports one.", "<em>Set Up</em> opens the Charging page."],
    ["Draining while plugged in", "The charger cannot keep up with the laptop's power use. Use a more powerful charger.", "None"],
    ["Low-power charger", "The charger is under 45 W, so charging can be slow, especially while you work.", "None"],
    ["High drain during sleep", "The laptop used a lot of battery while asleep (more than 2% per hour).", "<em>Details</em> opens the sleep list on the Usage page."],
    ["An app is using significant energy", "An app is keeping the CPU busy on battery. Quitting apps you are not using makes the battery last longer.", "<em>Details</em> opens the app on the Usage page."],
    ["Performance mode is on", "You are on battery in Performance mode, which drains faster. Balanced is usually enough.", "<em>Use Balanced</em>"],
    ["Screen brightness is at N%", "On battery with the brightness at 80% or more. The screen is usually the biggest power user.", "<em>Lower to 50%</em>"],
    ["Keyboard backlight is on", "On battery with the keyboard backlight on.", "<em>Turn Off</em>"],
    ["Bluetooth is on but nothing is connected", "On battery with Bluetooth enabled and no device connected.", "<em>Turn Off</em>"],
    ["Battery is low", "The battery is low and not charging. Regularly running it flat wears it faster.", "None"],
])}
{note("tip", "The brightness, keyboard backlight and Bluetooth suggestions only appear on battery, and only when your desktop exposes those controls.")}
"""

CONTENT["health"] = f"""
<p>Every battery loses a little capacity with age. The Health page measures how much, shows how it has changed over time,
and tells you when it is worth doing something about it.</p>
{fig("health", "The Health page with the Capacity card on the left and the Capacity Over Time chart on the right.", "The Health page: capacity and usage on the left, the long-term trend and battery details on the right.")}

<h2 id="capacity">Capacity</h2>
{fig("health-capacity", "The Capacity card: Maximum Capacity 86.8 percent, Condition Normal, Full Charge Capacity 44.2 Wh, Design Capacity 51.0 Wh and Capacity Lost 6.8 Wh (13.2 percent).", "The Capacity card compares what the battery holds now with what it held when new.", narrow=True)}
<ul>
<li><strong>Maximum Capacity</strong> is the energy the battery holds when full now, as a percentage of what it held when it was new,
shown with a level bar.</li>
<li><strong>Full Charge Capacity</strong> is that energy in watt-hours (Wh). <strong>Design Capacity</strong> is what the manufacturer
rated the battery for when new. <strong>Capacity Lost</strong> is the difference in Wh and in percent.</li>
<li><strong>Condition</strong> turns the number into a plain word:</li>
</ul>
{table(["Condition", "Maximum capacity", "What it means"], [
    ["Normal", "80% or more", "The battery is functioning normally."],
    ["Worn", "60% to 80%", "The battery holds less charge than when new but works normally."],
    ["Service Recommended", "Under 60%", "The battery holds significantly less charge; consider replacing it."],
])}
{note("note", "Some drivers report charge in µAh (charge-based) and others in µWh (energy-based). The app supports both.")}

<h2 id="trend">Capacity over time and the trend</h2>
{fig("health-trend", "A chart of capacity from 70 to 100 percent over three weeks with a dashed line at 80 percent, and a Trend row saying 14 of 5 readings and 12 of 14 days.", "Capacity is recorded once a day. The dashed line marks 80%, the usual point where a battery is considered worn.", narrow=True)}
<p>The app records the maximum capacity once a day (the first time it runs that day) and plots it. Readings wobble a little from
day to day, so the line is the direction that matters, not any single point.</p>
<p>After <strong>at least 5 daily readings spanning at least 14 days</strong> the <em>Trend</em> row switches from progress
("3/5 readings · 6/14 days") to a result:</p>
<ul>
<li><strong>N% per month</strong>, the rate at which capacity is falling, and <strong>Reaches 80%</strong>: the month and year the
battery is projected to cross 80% ("Around March 2028").</li>
<li><strong>Not declining</strong> when the line is flat or rising.</li>
<li><strong>Already below 80%</strong> when it has already crossed.</li>
</ul>
<p>A battery that does not report capacity data shows "This battery does not report capacity data, so a health trend cannot be calculated."</p>
{note("tip", "Turn on <em>Record History in the Background</em> on the Usage page so the daily reading is saved even on days you never open the window.")}

<h2 id="usage">Usage</h2>
{fig("health-usage", "A Usage card with Cycle Count 214, Current Charge 44.2 Wh, Average Draw on Battery over 24 hours 13.8 W, Voltage 12.85 V and Minimum Design Voltage 11.52 V.", "Cycle count and live electrical readings.", narrow=True)}
<ul>
<li><strong>Cycle Count</strong> is how many full charge-and-discharge equivalents the battery has been through. Most laptop
batteries are rated for 300–1000 cycles.</li>
<li><strong>Current Charge</strong> is the energy in the battery now, in Wh.</li>
<li><strong>Average Draw on Battery (24 h)</strong> is the average power the laptop used while on battery over the last day. It
appears once there is enough history.</li>
<li><strong>Voltage</strong> and <strong>Minimum Design Voltage</strong>: the present voltage and the lowest voltage the battery is designed to reach.</li>
</ul>
<p>Rows the battery does not report are shown as "Not reported", and rows with no history yet say "No data yet".</p>

<h2 id="information">Battery information</h2>
{fig("health-information", "A Battery Information card listing the manufacturer, model, serial number, chemistry and device name.", "The details of the battery itself. The serial number is hidden in this screenshot.", narrow=True)}
<p>Manufacturer, model, serial number, chemistry (for example Li-ion or Li-poly) and the device name Linux uses
(<code>BAT0</code>). It is handy for warranty claims and when you shop for a replacement.</p>

<h2 id="calibration">Calibration</h2>
{fig("health-calibration", "A Calibration card with the text Not running, You'll get a notification at each step, and a Start button.", "Start a guided calibration from the Calibration card.", narrow=True)}
<p>Battery gauges estimate the charge. When the percentage jumps around, or the laptop shuts down before it reaches 0%, the
estimate has drifted. Calibration lets the battery relearn its true capacity. It takes a full charge, a full discharge and another full
charge, usually a few hours in total, and you can use the laptop normally in between.</p>
{steps("Choose <strong>Start</strong> and confirm <em>Start Calibration?</em>. If the laptop is not full, plug it in; the card shows \"Step 1 of 3: Charge to 100%\".",
       "When the battery is full you get the notification <em>Calibration: Unplug Your Laptop</em>. Unplug the charger and use the laptop until the battery is nearly empty (5%). The card shows \"Step 2 of 3\" with the current percentage.",
       "At 5% you get <em>Calibration: Plug In Now</em>. Connect the charger and let it charge to 100% again.",
       "You get <em>Calibration Complete</em>. The battery's capacity reading has been recalibrated, and your charge limit is back on.")}
{fig("calibration", "The Start Calibration confirmation dialog on top of the Health page.", "The confirmation explains the three steps before anything starts.")}
{note("warn", "During calibration the charge limit is lifted so the battery can reach 100%. Choose <strong>Cancel</strong> on the card at any time to stop; your limit returns.")}
{note("hw", "Calibration needs a battery that reports its charge accurately. It is a fix for a wrong reading, not for a battery that has genuinely worn out: if the maximum capacity stays low afterwards, that number is the truth.")}
"""

CONTENT["charging"] = f"""
<p>Lithium batteries wear faster when they sit at full charge. If your laptop is plugged in most of the day, limiting
the charge to about 80% can noticeably extend the battery's lifespan. This page sets that limit.</p>
{fig("charging", "The Charging page showing the Charging Status cards, then the Charge Limit settings.", "The Charging page is one centered column: status first, then the limit, the schedule and the behavior controls.")}
{note("hw", "Charge limits need firmware support that Linux exposes as <code>charge_control_end_threshold</code>. If your laptop does not have it, this page shows <strong>Not supported on this laptop</strong> with the reason, and the rest of the app works normally.")}

<h2 id="status">Charging status</h2>
{fig("charging-status", "Three small cards: Active Charge Limit BAT0 100 percent, Next Scheduled Full Charge Full by Sun 10:00 starting Sun 08:00, and Temporary Override None, charging is unrestricted.", "The three cards at the top tell you what is in force right now.")}
<ul>
<li><strong>Active Charge Limit</strong>: where charging stops for each battery. "Charging will stop at 80%", or "Charge limit removed" when there is no limit.</li>
<li><strong>Next Scheduled Full Charge</strong>: when your <a href="schedule.html">schedule</a> will next lift the limit, as "Full by Mon 07:30 · starts Mon 05:30". It says "Save a charge limit below 100% to use scheduled charging" when a schedule would have no limit to lift, and "Learning your routine" while optimized charging is still gathering data.</li>
<li><strong>Temporary Override</strong>: "None" normally. During a one-time full charge it says "One-time full charge", with the limit that comes back; during a scheduled full charge it says "Scheduled full charge".</li>
</ul>

<h2 id="limit">Charge limit</h2>
{fig("charging-limit", "The Charge Limit card with Current Setting Charges to 100 percent, a Profile menu, the Limit Maximum Charge switch, Stop Charging At and Resume Charging Below spin buttons, Keep After Restart, and an Apply button.", "Pick a profile, or switch on Limit Maximum Charge and set your own numbers, then choose Apply.")}
<h3>Profiles</h3>
<p>The <strong>Profile</strong> menu sets both numbers at once:</p>
{table(["Profile", "Stops charging at", "Resumes below", "Best for"], [
    ["<strong>Maximum Lifespan (60%)</strong>", "60%", "55%", "Plugged in most of the time. The least wear."],
    ["<strong>Balanced (80%)</strong>", "80%", "75%", "A good everyday balance."],
    ["<strong>Full Capacity (100%)</strong>", "100%", "No limit", "The longest time away from the charger."],
    ["<strong>Custom</strong>", "Your number", "Your number", "Your own settings below."],
])}
<h3>Your own numbers</h3>
<ul>
<li><strong>Limit Maximum Charge</strong> turns the limit on or off. When it is off, the battery charges to 100%.</li>
<li><strong>Stop Charging At</strong> (a percentage) is the level where charging stops.</li>
<li><strong>Resume Charging Below</strong> is the level where charging starts again. Keeping a gap avoids topping up after every small drop. It has to be below the stop level.</li>
<li><strong>Keep After Restart</strong> re-applies the limit when the computer starts and after it wakes from sleep. Without it the firmware may forget the limit when it restarts. A saved limit is also required for the schedule.</li>
</ul>
<h3>Applying the limit</h3>
{steps("Pick a profile, or change the numbers.", "Choose <strong>Apply</strong>.", "Enter your password in the system prompt. Changing a charge limit writes to the battery's firmware settings, so it needs administrator permission. The prompt is the standard PolicyKit one.", "A message confirms \"Charging will stop at 80%\" (or \"Charge limit removed\").")}
{note("tip", "Not every laptop has a separate resume level. When yours does not, <em>Resume Charging Below</em> is not shown. The page only offers the controls your laptop supports.")}

<h2 id="now">Charge to 100% now</h2>
<p>Need a full battery for a trip or a long day away from the charger? While a limit below 100% is in force, a <strong>Charge to 100% Now</strong> row
appears with a <strong>Charge Now</strong> button.</p>
{steps("Connect the charger and choose <strong>Charge Now</strong>.", "The Overview says \"Charging to 100%\" and the Temporary Override card says \"One-time full charge\".", "When the battery is full, or when you unplug, your limit comes back by itself. Choose <strong>Back to your charge limit</strong> if you change your mind earlier.")}

<h2 id="behavior">Charging behavior</h2>
{fig("charging-behavior", "The Charging Behavior card with a While Plugged In menu set to Normal.", "Temporarily pause charging or run from the battery while plugged in.")}
<p>The <strong>While Plugged In</strong> menu is a temporary override for special situations:</p>
{table(["Choice", "What it does"], [
    ["<strong>Normal</strong>", "Charges as usual, following your limit."],
    ["<strong>Pause Charging</strong>", "The laptop stays plugged in and runs from the charger without charging the battery."],
    ["<strong>Run on Battery</strong>", "The laptop runs from the battery even though the charger is connected. Useful for testing how long the battery lasts."],
])}
<p>Choose <strong>Normal</strong> to go back. This control only appears when your laptop supports it.</p>
"""

CONTENT["schedule"] = f"""
<p>A charge limit is great until the morning you need the laptop to last all day. The charging schedule gets you both: the
limit protects the battery, and shortly before you need the laptop the limit is lifted so the battery is
full on time, then put back.</p>
{fig("charging-schedule", "The Charging Schedule card with Optimized Charging on, learned unplug times, Start Charging Before 2.0 hours, two ready-by rules and an Add a Time row.", "The Charging Schedule card: optimized charging, the ready-by times, and the lead time.")}
{note("warn", "The schedule needs a saved charge limit below 100%. Turn on <em>Limit Maximum Charge</em> and <em>Keep After Restart</em> on the Charging page, then choose <strong>Apply</strong>. Until then the schedule card tells you so.")}

<h2 id="rules">Ready-by times</h2>
<p>A <strong>ready-by time</strong> is a day or group of days with the time you want the battery to be full by, for example
<em>Weekdays, 07:30</em>.</p>
{steps("Choose <strong>Add a Time…</strong>.", "Pick the days (<em>Mon</em> to <em>Sun</em>) and the time. At least one day must be selected.", "Choose <strong>Add</strong>. The time appears as a row such as \"Full by 07:30 · Weekdays\". Use the trash button on a row to remove it.", "Choose <strong>Save Schedule</strong> and enter your password once.")}
{fig("add-time", "The Add a Time dialog with day buttons Monday to Sunday and hour and minute spin buttons.", "Pick days and the time you want the battery to be full by.")}
<ul>
<li>Days that share a time are shown together: <em>Every day</em>, <em>Weekdays</em>, <em>Weekends</em>, or a list of days.</li>
<li>You can have up to 14 rules.</li>
<li>Rules are saved only when you choose <strong>Save Schedule</strong>, and "Schedule saved" or "Schedule removed" confirms it.</li>
</ul>

<h2 id="lead">Start charging before</h2>
<p><strong>Start Charging Before</strong> is the lead time, in hours before each ready-by time, when the limit is lifted. The default is
2 hours; you can choose from half an hour to six hours. Pick a longer lead if your charger is slow or your battery is large.</p>
<p>The limit stays lifted until the battery is full, you unplug, or 30 minutes after the ready-by time, whichever comes first, so a battery that is a bit
slow still finishes. Then your limit is restored.</p>
<p>The Charging Status card <em>Next Scheduled Full Charge</em> shows what will happen next, such as "Full by Mon 07:30 · starts Mon 05:30".</p>

<h2 id="optimized">Optimized charging</h2>
<p>Do not want to enter times? Turn on <strong>Optimized Charging</strong> and the app learns your routine.</p>
<ul>
<li>Whenever you unplug after the laptop was plugged in for <strong>at least 3 hours</strong>, the app records the time.</li>
<li>When it has seen <strong>at least 3 such unplugs</strong> in the last <strong>28 days</strong> on weekdays (or on weekends) that are within
<strong>an hour and a half</strong> of each other, it uses the <em>earlier end</em> of that range as the ready-by time for those days.</li>
<li>The <strong>Learned So Far</strong> row shows the result: "Weekdays: charged by about 08:15", or "Weekends: still learning (2 of 3 unplugs)".</li>
</ul>
<p>Weekday and weekend habits are learned separately, so a lie-in on Saturday does not disturb your Monday. Everything stays on your computer, in a small file of unplug times.</p>

<h2 id="how">How it works</h2>
<p>When you choose <strong>Save Schedule</strong>, the app asks for your password once and installs the schedule where a small
systemd service can read it. That service runs as root only while a schedule exists (a systemd <code>.path</code> unit starts it), lifts and restores the limit
at the right moments, and goes back to sleep. It needs no password each time and does not
need the app to be running. The details are on <a href="privacy.html">Privacy and security</a>.</p>
"""

CONTENT["power"] = f"""
<p>The Power page puts the most useful power options in one place: the power mode, a way to switch it
automatically, and the GNOME settings that save the most battery. Everything here is shown only if your system supports it.</p>
{fig("power", "The Power page with Power Mode, Automatic Power Mode and Power Saving sections.", "The Power page is one centered column, like Charging.")}

<h2 id="mode">Power mode</h2>
{fig("power-mode", "A Power Mode card with Current Mode set to Balanced.", "Choose how the laptop trades performance for battery life.")}
<p>Power Saver, Balanced or Performance, using the same system service as GNOME's quick settings. <strong>Power Saver</strong> reduces
performance to extend battery life. <strong>Performance</strong> is only offered on some laptops and drains the battery faster.
The setting is shared with the GNOME panel, so a change made there shows up here and the other way around.</p>
{note("note", "If the Power Profiles Daemon is not running, the page says \"Power Mode and Automatic Power Mode Aren't Available\", because the mode can neither be read nor changed.")}

<h2 id="automatic">Automatic power mode</h2>
{fig("power-automatic", "The Automatic Power Mode card with Switch Automatically off, plugged-in and on-battery modes, and a low battery override.", "Switch the mode by itself when you plug in or unplug.")}
<p>Turn on <strong>Switch Automatically</strong> and the app changes the power mode when you plug in or unplug the charger.</p>
<ul>
<li><strong>When Plugged In</strong> and <strong>On Battery</strong>: choose the mode for each state. A typical setup is Performance on the charger and Power Saver on battery.</li>
<li><strong>Use Power Saver When Battery Is Low</strong> overrides the on-battery mode once the charge falls to the <strong>Low Battery Level</strong> (20% by default).</li>
<li>You can still change the mode by hand at any time. It is only switched again the next time something changes (the charger is connected or removed).</li>
</ul>

<h2 id="saving">Power saving</h2>
{fig("power-saving", "The Power Saving card with switches for Power Saver When Battery Is Low and Dim Screen When Inactive, and menus for Screen Blank, Automatic Suspend on Battery and Automatic Suspend When Plugged In.", "GNOME's own battery-saving settings.")}
<p>These are the same settings as <em>Settings → Power</em>, collected in one place:</p>
{table(["Setting", "What it does"], [
    ["Power Saver When Battery Is Low", "GNOME switches to Power Saver mode when the battery gets low."],
    ["Dim Screen When Inactive", "Dims the screen when you stop using the laptop. The screen is usually the biggest power user, so this saves a lot."],
    ["Automatic Screen Brightness", "Adjusts the brightness to the light around you (needs an ambient light sensor)."],
    ["Screen Blank", "Turns the screen off after you stop using the laptop. Choose from 1 to 15 minutes, or Never."],
    ["Automatic Suspend on Battery", "Sleeps after a time of inactivity on battery (5 minutes to 2 hours, or Never). Sleeping uses far less power than staying on."],
    ["Automatic Suspend When Plugged In", "The same, on the charger."],
])}

<h2 id="buttons">Power button and lid</h2>
{fig("power-buttons", "The Power Button and Lid card with menus for Power Button, Close the Lid on Battery and Close the Lid When Plugged In, each set to Suspend.", "What the power button and the lid do.")}
<p>Choose what happens when you press the power button and when you close the lid, separately on battery and on the charger. The options
include <em>Suspend</em>, <em>Hibernate</em>, <em>Turn Off the Screen</em>, <em>Shut Down</em>, <em>Log Out</em>, <em>Ask What to Do</em> and <em>Do Nothing</em>;
you only see the ones your system supports.</p>

<h2 id="top-bar">Top bar</h2>
{fig("power-top-bar", "A Top Bar card with the switch Show Battery Percentage.", "Show the percentage next to the battery icon.")}
<p><strong>Show Battery Percentage</strong> shows the percentage next to the battery icon in the GNOME top bar.</p>
{note("note", "If GNOME's power settings cannot be found, this part of the page says \"Power Saving and Power Button Settings Aren't Available\". They need GNOME Settings Daemon. The page is the part of the app that is most tied to GNOME.")}
"""

CONTENT["usage"] = f"""
<p>The Usage page answers "where did my battery go?" It has the history of the battery level and power draw, how much the laptop used
while asleep, and the apps that use the most energy.</p>
{fig("usage", "The Usage page with the battery level chart, the background recording switch, sleep sessions, and lists of apps by energy use.", "The Usage page: history on the left, apps on the right.")}

<h2 id="chart">Battery level and power</h2>
{fig("usage-battery-level", "A chart of the battery level over 24 hours with green areas for plugged in and blue for on battery, with buttons Level, Power, 24 Hours and 7 Days.", "The history chart. Choose what to plot and how far back to look.")}
<p>Use the two button groups above the chart:</p>
<ul>
<li><strong>Level</strong> plots the battery percentage. <strong>Power</strong> plots the power draw in watts: blue is power used on battery, green is charging power.</li>
<li><strong>24 Hours</strong> or <strong>7 Days</strong> sets the range. History is kept for 30 days.</li>
</ul>
<p>In the level chart, <strong>green is plugged in</strong>, <strong>blue is on battery</strong>, and a <strong>dashed line</strong> marks your charge limit. Periods on the charger also
get a thick strip along the bottom, so the chart does not depend on color alone. Gaps are times when the app was not running.</p>
<p>A new install says "Your history starts here" and "Not enough samples in this view yet" until the app has recorded a few minutes. A sample is
saved every minute while the app runs.</p>

<h2 id="background">Record history in the background</h2>
{fig("usage-background", "The Record History in the Background switch with its explanation.", "One switch keeps history, sleep drain and notifications running with the window closed.", narrow=True)}
<p>History, sleep drain and notifications only work while Battery Health Center is running. This switch adds a login item that starts
the app <strong>hidden</strong> when you log in, so everything keeps working with no window. Open the app from your applications at any time
to bring the window up; the hidden copy hands over to it.</p>
<p>After an update the hidden copy notices that its program file was replaced and restarts itself in the new version, within about a minute, once no window is open.</p>

<h2 id="sleep">Battery use during sleep</h2>
{fig("usage-sleep", "A list of sleep sessions with start times, durations, percent used, and High or Moderate labels, followed by a Sleep Mode row saying Modern Standby (s2idle).", "How much charge your laptop used while suspended.")}
<p>When the app is running before and after the laptop sleeps <em>on battery</em> for at least 15 minutes, it records the session: when it
started, how long it slept, and how much battery it used, in percent and in percent per hour. The five most recent sessions are listed.</p>
<ul>
<li><strong>High</strong> means more than 2% per hour. That is worth investigating, and you also get a notification and an Overview recommendation.</li>
<li><strong>Moderate</strong> is 1% to 2% per hour, and <strong>Normal</strong> is under 1% per hour.</li>
</ul>
<p>The <strong>Sleep Mode</strong> row says how your laptop sleeps:</p>
{table(["Mode", "Meaning"], [
    ["Modern Standby (s2idle)", "Wakes quickly, but some laptops drain more while asleep. If your laptop also supports deep sleep, the row says so and mentions the kernel option <code>mem_sleep_default=deep</code>."],
    ["Deep Sleep (S3)", "The lowest-power sleep mode."],
    ["Shallow Sleep (S1)", "A light sleep mode."],
])}
{note("tip", "A laptop that loses a lot of charge asleep is a common complaint. Modern Standby is usually the cause; deep sleep, when available, typically uses much less battery.")}

<h2 id="apps">Apps using the most energy</h2>
{fig("usage-significant", "A list of apps using significant energy: Claude High, GNOME Shell Medium, Display Server Medium and Linux Kernel Low.", "Apps ranked by their current energy impact.")}
<p>The <strong>Using Significant Energy</strong> list at the top right is what to look at when the battery is draining fast right now. It ranks the apps by their current energy impact; when none stands out, it says "No apps are using significant energy". Click an app to see its detail.</p>
{fig("usage-apps", "Energy Use by App with the buttons Since Opened, 24 Hours and 7 Days and a ranked list of apps with their CPU use and an impact label.", "Energy Use by App: pick the period with the buttons.")}
<p>The <strong>Energy Use by App</strong> list covers a longer period: <strong>Since Opened</strong> (since the app started), <strong>24 Hours</strong> or <strong>7 Days</strong>. It shows the eight biggest. Each row has:</p>
<ul>
<li>the app's name and icon, and how many <em>processes</em> it has (a browser may have dozens);</li>
<li>its <strong>CPU</strong> use in percent of one core, and the estimated energy used on battery;</li>
<li>an <strong>impact</strong> label: <strong>High</strong> (40% or more), <strong>Medium</strong> (10% or more) or <strong>Low</strong>. <em>System</em> marks background services such as the Linux kernel.</li>
</ul>
{note("warn", "Linux does not measure power per app. Energy impact is estimated from CPU time, and watts are the measured battery drain above idle, shared between apps by CPU use. GPU, display and device activity are not measured per app. Treat the numbers as a guide, not as a measurement. The (i) button next to each list explains this in the app.")}
<p>Apps are grouped using the systemd scope that GNOME starts each app in. Apps that move their processes elsewhere (such as Chromium-based browsers) are grouped by their parent process, so a browser is one row, not fifty.</p>

<h2 id="detail">App detail</h2>
{fig("app-detail", "The detail dialog for an app with its energy impact and energy used, and charts for the last 24 hours and 7 days.", "Click an app to see its energy impact and estimated use hour by hour.", narrow=True)}
<p>Click any app to open its detail. It shows the <strong>Energy Impact</strong> and the <strong>Energy Used</strong> (an estimate in Wh on battery), with the period set
by the <strong>24 Hours / 7 Days</strong> buttons, and an hour-by-hour chart of CPU use and energy. Only hours when the app was running are shown, and completed hours are saved for the last 7 days. A new install says "No hourly history yet. Keep the app running to collect activity."</p>
"""

CONTENT["preferences"] = f"""
<p>Open <strong>Preferences</strong> from the main menu or with <kbd>Ctrl</kbd>+<kbd>,</kbd> to choose which notifications you
get. Everything else in the app is on its own page.</p>
{fig("preferences", "The Preferences dialog with switches for Charge Limit Reached, Fully Charged Without a Limit, Low Battery with a Notify At level, Apps Using Significant Energy and High Drain During Sleep.", "The Preferences dialog.")}

<h2 id="notifications">Notifications</h2>
<p>Notifications are sent while Battery Health Center is running. Turn on <em>Record History in the Background</em> on the
<a href="usage.html">Usage</a> page to get them even when the window is closed. All of them are on by default; turn off any you do not want.</p>
{table(["Notification", "When you get it"], [
    ["<strong>Charge Limit Reached</strong>", "Charging stops at your limit: \"Charged to 80%. Charging has stopped at your limit to protect the battery.\""],
    ["<strong>Fully Charged Without a Limit</strong>", "The battery is full and no limit is set: a reminder to unplug, or to set a charge limit to slow wear."],
    ["<strong>Low Battery</strong>", "The battery drops to the <em>Notify At</em> level (20% by default) and you are not charging. It adds the time remaining when it knows it."],
    ["<strong>Apps Using Significant Energy</strong>", "An app keeps the CPU very busy for a couple of minutes while you are on battery. Quit it if you are not using it."],
    ["<strong>High Drain During Sleep</strong>", "The laptop used a lot of battery while asleep (over 2% per hour): \"Your laptop used 7% in 51 min asleep (8.3% per hour).\""],
])}
<p>Clicking a notification opens, or raises, the app window.</p>
{note("note", "The calibration step notifications (<em>Unplug Your Laptop</em>, <em>Plug In Now</em>, <em>Calibration Complete</em>) are always sent, because you need them to complete the calibration.")}

<h2 id="other">Other settings, found elsewhere</h2>
{table(["Setting", "Where"], [
    ["Record history in the background", "<a href=\"usage.html#background\">Usage</a> page"],
    ["Charge limit, schedule and charging behavior", "<a href=\"charging.html\">Charging</a> page"],
    ["Power mode, automatic power mode, power saving", "<a href=\"power.html\">Power</a> page"],
    ["Where the settings are saved", "<code>~/.config/battery-health-center/settings.ini</code>"],
])}
"""

CONTENT["data"] = f"""
<p>Your battery data belongs to you. Battery Health Center can export it as a readable report or as spreadsheet-friendly files, and
it stores everything in plain files in your home folder.</p>

<h2 id="report">Battery report (HTML)</h2>
<p>Choose <strong>Export Battery Report…</strong> from the main menu, pick a file name, and the app saves a single self-contained
HTML page you can open in any browser, print, email or keep. It is useful for a warranty claim, or to show a buyer when you sell the laptop. A <strong>Open</strong> button in the confirmation opens it straight away.</p>
<p>The report has these sections:</p>
<ul>
<li><strong>Battery</strong>: a table for each battery with the manufacturer, model, chemistry, design and full-charge capacity, maximum capacity, condition, cycle count and more.</li>
<li><strong>Capacity Over Time</strong>: the daily capacity chart.</li>
<li><strong>Battery Level, Last 7 Days</strong>: the level chart with charger periods.</li>
<li><strong>Battery Use During Sleep</strong>: the recorded sleep sessions.</li>
<li><strong>Apps by Energy Use, Last 7 Days</strong>: the top apps with estimated use.</li>
</ul>
<p>The report is in English whatever your system language.</p>

<h2 id="csv">Export data as CSV</h2>
<p>Choose <strong>Export Data as CSV…</strong>, pick a folder, and choose <strong>Save Here</strong>. The app writes four files you can open in a spreadsheet
or load into your own tools (the confirmation says how many files were saved):</p>
{table(["File", "Contents"], [
    ["<code>battery-history.csv</code>", "Battery level and power samples, one per minute (kept for 30 days)."],
    ["<code>capacity-history.csv</code>", "The daily capacity readings used for the trend."],
    ["<code>sleep-sessions.csv</code>", "Each recorded sleep session and how much battery it used."],
    ["<code>app-energy.csv</code>", "Each app's CPU use and estimated energy, per hour (kept for 7 days)."],
])}
<p>Timestamps use the ISO 8601 format with your time zone, for example <code>2026-11-07T08:15:00-05:00</code>.</p>

<h2 id="cli">From the command line</h2>
<p>Both exports also work without opening the window, which is handy for scripts and scheduled backups:</p>
<pre><code>battery-health-center --export-report report.html
battery-health-center --export-csv ~/battery-data</code></pre>
<p>The CSV command prints the path of each file it wrote.</p>

<h2 id="stored">Where your data is stored</h2>
{table(["Location", "What it holds"], [
    ["<code>~/.local/share/battery-health-center/history.csv</code>", "Battery samples, once a minute, for the last 30 days."],
    ["<code>~/.local/share/battery-health-center/health.csv</code>", "The daily capacity readings used for the trend."],
    ["<code>~/.local/share/battery-health-center/app-history.csv</code>", "Per-app energy by the hour, for the last 7 days."],
    ["<code>~/.config/battery-health-center/settings.ini</code>", "Your preferences."],
    ["<code>~/.config/autostart/io.github.BatteryHealthCenter.desktop</code>", "Present only when background recording is on."],
])}
<p>The charge limit and the schedule are stored by the root helper, outside your home folder (see <a href="reference.html#files">Reference</a>).
To start over, quit the app and delete the files you want to reset.</p>
<p>Writes are made safely: preferences are replaced atomically, and if a save fails the app keeps the previous file, shows a banner with the path and the error, and retries the history when storage recovers.</p>
"""

CONTENT["reference"] = f"""
<h2 id="shortcuts">Keyboard shortcuts</h2>
{table(["Shortcut", "Action"], [
    ["<kbd>F1</kbd>", "Open this manual at the page for the one you are on"],
    ["<kbd>Ctrl</kbd>+<kbd>R</kbd>", "Refresh the readings now"],
    ["<kbd>Ctrl</kbd>+<kbd>,</kbd>", "Open Preferences"],
    ["<kbd>Ctrl</kbd>+<kbd>Q</kbd>", "Quit"],
    ["<kbd>Tab</kbd>, <kbd>Shift</kbd>+<kbd>Tab</kbd>, <kbd>Enter</kbd>", "Move between controls and activate them; the sidebar rows are keyboard-accessible too"],
])}

<h2 id="menu">The main menu</h2>
{table(["Item", "What it does"], [
    ["Refresh", "Reads the battery and everything else again right now."],
    ["Export Battery Report…", "Saves the <a href=\"data.html#report\">HTML report</a>."],
    ["Export Data as CSV…", "Saves the <a href=\"data.html#csv\">CSV files</a>."],
    ["Preferences", "Opens the <a href=\"preferences.html\">notification settings</a>."],
    ["User Manual", "Opens this manual at the page for the one you are on."],
    ["About Battery Health Center", "Version, links, credits and license."],
    ["Quit", "Closes the app."],
])}

<h2 id="cli">Command line</h2>
{table(["Command", "What it does"], [
    ["<code>battery-health-center</code>", "Open the window."],
    ["<code>battery-health-center --background</code>", "Start hidden and keep recording history (this is what the login item runs)."],
    ["<code>battery-health-center --export-report FILE.html</code>", "Write the battery report and exit."],
    ["<code>battery-health-center --export-csv FOLDER</code>", "Write the four CSV files into the folder and exit."],
])}

<h2 id="files">Files and services</h2>
{table(["Path or unit", "Purpose"], [
    ["<code>/usr/bin/battery-health-center</code>", "The app."],
    ["<code>bhc-helper</code>", "The only part that runs as root. It validates its arguments and writes the charge-limit files and its own configuration. Started through PolicyKit."],
    ["<code>/etc/battery-health-center.conf</code>", "The saved charge limit that <em>Keep After Restart</em> re-applies."],
    ["<code>/etc/battery-health-center.schedule</code>", "The saved ready-by times and lead time."],
    ["<code>/var/lib/battery-health-center/unplug-log</code>", "The unplug times learned by optimized charging."],
    ["<code>battery-health-center-restore.service</code>", "Re-applies your limit at startup and after sleep."],
    ["<code>battery-health-center-scheduler.service</code> and <code>.path</code>", "Run the schedule; the <code>.path</code> unit starts the service only while a schedule exists."],
    ["<code>/sys/class/power_supply/BAT*</code>", "Where the battery readings come from (read directly; no root needed)."],
])}
<p>The exact configuration file locations can differ slightly between packages; the <a href="{REPO}/blob/main/docs/HOW-IT-WORKS.md">How it works</a> notes describe what each piece does.</p>

<h2 id="glossary">Glossary</h2>
<dl>
<dt>Capacity (maximum capacity)</dt><dd>How much energy the battery holds when full now, compared with when it was new.</dd>
<dt>Design capacity</dt><dd>What the manufacturer rated the battery for when new, in Wh.</dd>
<dt>Cycle count</dt><dd>The number of full charge-and-discharge equivalents. Two half charges make one cycle.</dd>
<dt>Wh (watt-hour)</dt><dd>A unit of stored energy. A 50 Wh battery can power a 10 W laptop for about five hours.</dd>
<dt>Charge threshold</dt><dd>A firmware setting for where charging stops (end threshold) and resumes (start threshold).</dd>
<dt>Power profile</dt><dd>The system's performance setting: Power Saver, Balanced or Performance.</dd>
<dt>Energy impact</dt><dd>An estimate of how much battery an app uses, based on its CPU time.</dd>
<dt>Sleep drain</dt><dd>Battery used while the laptop is suspended, as percent and percent per hour.</dd>
<dt>s2idle, S3 and S1</dt><dd>Sleep states. <em>s2idle</em> (Modern Standby) wakes quickly, <em>S3</em> (deep) uses the least power, <em>S1</em> is a light sleep.</dd>
<dt>Calibration</dt><dd>A full charge, a full discharge and a full charge that lets the battery gauge relearn the real capacity.</dd>
<dt>PolicyKit</dt><dd>The standard Linux service that asks for your password before an app makes an administrator-level change.</dd>
</dl>
"""

CONTENT["troubleshooting"] = f"""
<h2 id="faq">Frequently asked questions</h2>

<details open><summary>Will the charge limit work on my laptop?</summary>
<p>Battery information works on any laptop with a standard Linux battery driver. Charge limits need firmware support that the kernel exposes as
<code>charge_control_end_threshold</code> (or <code>charge_stop_threshold</code>), as on ThinkPads and many ASUS, Dell, Huawei, LG, Samsung and System76
laptops. You can check from a terminal:</p>
<pre><code>ls /sys/class/power_supply/BAT*/charge_control_end_threshold</code></pre>
<p>If the file exists, the limit should work. If not, the Charging page says "Not supported on this laptop" and everything else still works.</p></details>

<details><summary>Which desktops and distributions are supported?</summary>
<p>It is developed and tested on GNOME: Ubuntu 24.04, Zorin OS 18 and Linux Mint 22, and Fedora 43 and newer. It needs GTK 4.14 and libadwaita 1.5 or newer, so Ubuntu 22.04, Debian 12 and Mint 21 are too old. The Power page uses GNOME's own settings, so it is the part most tied to GNOME.</p></details>

<details><summary>Is it safe to change the charge limit?</summary>
<p>It sets the same firmware thresholds that other tools and BIOS options use. <strong>Full Capacity (100%)</strong> removes the limit again. Changing it asks for your password, and only a small helper that accepts nothing but validated values makes the change.</p></details>

<details><summary>Do I have to keep the app open?</summary>
<p>History, sleep drain and notifications are recorded while the app runs. Turn on <em>Record History in the Background</em> on the <a href="usage.html#background">Usage</a> page and it starts hidden when you log in.
The charge limit itself is applied by the system and does not need the app.</p></details>

<details><summary>Why do the charts and the trend start empty?</summary>
<p>Samples are taken once a minute while the app runs, and the capacity once a day. The trend needs 5 readings over 14 days. See
<a href="getting-started.html#first-launch">your first launch</a> for how long each thing takes.</p></details>

<details><summary>Why are the energy numbers per app only estimates?</summary>
<p>Linux does not measure power per app. The app shares the measured battery drain above idle between apps by their CPU use. GPU, display and device activity are not measured per app, so treat the numbers as a guide.</p></details>

<details><summary>Does it work on a desktop PC?</summary>
<p>No. It needs a battery. Without one the app says "No Battery Found".</p></details>

<details><summary>Can I use it in my language?</summary>
<p>The app is available in English, Spanish, German, French and Simplified Chinese, following your system language. The exported HTML report and this manual are in English.</p></details>

<h2 id="problems">Problems and fixes</h2>
{table(["Problem", "What to try"], [
    ["The charge limit does not stop charging at my number", "Choose <strong>Apply</strong> after changing it, and keep <em>Keep After Restart</em> on. Some laptops round the value or accept only certain ones. Check that the Charging Status card shows the limit you expect. Charging is held at the limit and resumes only below the <em>Resume Charging Below</em> level."],
    ["It asks for my password every time I change a limit", "That is the PolicyKit prompt, which appears each time you apply a new limit. You enter your password once more when you save a schedule, and the schedule itself then runs without asking. A copy run from the build folder (not installed) asks every time."],
    ["\"The bhc-helper program was not found. Is the app installed?\"", "The helper that makes root changes is missing. Install the app from the packages instead of running a loose build, or run <code>./install.sh</code> from the source."],
    ["\"Authorization was cancelled\" / \"The change could not be applied\"", "The password prompt was closed or the firmware refused the value. Try again, and pick a value your laptop accepts."],
    ["The schedule does not seem to run", "A schedule needs a <em>saved</em> limit below 100% (<em>Limit Maximum Charge</em> and <em>Keep After Restart</em>, then <strong>Apply</strong>) and a saved schedule (<strong>Save Schedule</strong>). Check the <em>Next Scheduled Full Charge</em> card."],
    ["Optimized charging says it is still learning", "It needs at least 3 unplugs on weekdays (and 3 on weekends) within 28 days, each after at least 3 hours on the charger and within 90 minutes of one another."],
    ["The Power page says the mode is not available", "The Power Profiles Daemon is not running. Install and start <code>power-profiles-daemon</code>, then reopen the page."],
    ["No notifications when the window is closed", "Turn on <em>Record History in the Background</em> on the Usage page. Check GNOME's notification settings for the app."],
    ["The battery percentage jumps or the laptop dies before 0%", "Run the <a href=\"health.html#calibration\">calibration</a>. If the maximum capacity is low afterwards, that is the true state of the battery."],
    ["A banner says \"Some changes could not be saved\"", "Choose <strong>Details</strong>. Check disk space and the permissions of the folder named there. The app retries when storage recovers."],
    ["High drain during sleep", "Check the sleep list on the Usage page. If the Sleep Mode row says Modern Standby and your laptop supports deep sleep, you can try the kernel option <code>mem_sleep_default=deep</code>. Disconnect USB devices before sleeping."],
])}

<h2 id="report">Reporting a bug</h2>
<p>Please <a href="{REPO}/issues">open an issue</a> with your laptop model, your distribution, the app version (copy it from the About dialog with one click) and what you expected to see. Translations and testing on other laptops are especially welcome.</p>
"""

CONTENT["whats-new"] = (
    "<p>This page lists what changed in every release and links to the full explanation of each feature. Your version is shown in the "
    "<strong>About Battery Health Center</strong> dialog (main menu).</p>"
    + "".join(
        f'<h2 id="version-{v.replace(".", "-")}">Version {v}</h2><p class="date">{d}</p><ul>' + "".join(f"<li>{i}</li>" for i in items) + "</ul>"
        for v, d, items in WHATS_NEW
    )
    + f'<p>Every release is also listed, with its download, on the <a href="{REPO}/releases">releases page</a>.</p>'
)

CONTENT["privacy"] = f"""
<h2 id="local">Everything stays on your computer</h2>
<ul>
<li>The app <strong>never connects to the internet</strong> and has <strong>no telemetry</strong>, no accounts and no ads.</li>
<li>Your history is a few CSV files in your home folder. Delete them whenever you like.</li>
<li>Optimized charging stores a list of unplug times on your computer. Nothing leaves it.</li>
<li>The only network activity involved is opening a link in your browser when <em>you</em> click one, such as this manual.</li>
</ul>

<h2 id="root">The root helper</h2>
<p>Changing the battery's charge limit requires administrator rights, because the firmware settings are protected. The app itself never runs as root. Instead:</p>
<ul>
<li>A small program, <code>bhc-helper</code>, is the <strong>only part that runs as root</strong>. It is started through PolicyKit, so you approve it with your password.</li>
<li>It uses only Rust's standard library (plus libc for the local time), <strong>accepts nothing but validated arguments</strong>, and refuses anything else.</li>
<li>It writes only: the battery's <code>charge_control_start_threshold</code> and <code>charge_control_end_threshold</code> files; its own configuration files in <code>/etc</code> (the saved limit and the charging schedule); and a list of unplug times in <code>/var/lib/battery-health-center</code> for optimized charging.</li>
<li>With <em>Keep After Restart</em> on, a small systemd service re-applies the limit at startup and after sleep. With a schedule saved, a second service (started by a <code>.path</code> unit only while a schedule exists) lifts and restores the limit at the right times without asking for a password each time.</li>
</ul>
<p>Everything else (reading battery data, drawing charts, power modes, GNOME settings) is done as you, with no extra permissions.</p>

<h2 id="open">Open source</h2>
<p>The app is free software under the <strong>GPL-3.0-or-later</strong>. You can read exactly what it does in the
<a href="{REPO}">source code</a>. Packages are published from a signed apt repository and Fedora's COPR, so your usual updates keep it current.</p>
"""


# --------------------------------------------------------------------------- site (same design as the Voltkit manual)

# (slug, group) in reading order; "index" is the manual's front page.
GROUPS = [
    ("Start here", ["index", "getting-started"]),
    ("The app", ["overview", "health", "charging", "schedule", "power", "usage"]),
    ("Settings and data", ["preferences", "data"]),
    ("Reference", ["reference", "troubleshooting", "privacy", "whats-new"]),
]
TITLES["index"] = "Battery Health Center Manual"
ORDER = [slug for _, slugs in GROUPS for slug in slugs]
SITE_TITLE = "Battery Health Center Manual"
AUTHOR = "https://github.com/arios4k"

PAGE = """<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>@TITLE@ · @SITE@</title>
<meta name="description" content="@DESC@">
<link rel="icon" href="img/icon.svg"><link rel="stylesheet" href="style.css"></head>
<body>
<header class="top"><button class="menu" aria-label="Open the menu" aria-expanded="false" aria-controls="nav">☰</button>
<a class="brand" href="index.html"><img src="img/icon.svg" alt="" width="28" height="28"> <span>@SITE@</span></a>
<button class="find" aria-label="Search the manual" aria-expanded="false">⌕</button>
<input id="q" type="search" placeholder="Search the manual…" aria-label="Search the manual" autocomplete="off">
<a class="ghost" href="@REPO@">GitHub</a></header>
<div id="results" class="results" hidden></div>
<div class="layout">
<aside class="side" id="nav" aria-label="Manual pages"><div class="drawer-head"><a class="brand" href="index.html"><img src="img/icon.svg" alt="" width="28" height="28"> <span>@SITE@</span></a><button class="close" aria-label="Close the menu">✕ Close</button></div>@NAV@</aside>
<main><details class="toc-m"><summary>On this page</summary><ul>@TOC@</ul></details><article>@BODY@</article>@PAGER@
<footer>Written by Alexis Rios-Negron · Battery Health Center @VERSION@ · <a href="@REPO@/issues">Report a problem with this manual</a> · GPL-3.0-or-later</footer></main>
<aside class="toc" aria-label="On this page"><div class="group-title">On this page</div><ul>@TOC@</ul></aside>
</div><script src="manual.js"></script></body></html>
"""

REDIRECT = """<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Battery Health Center Manual</title>
<meta http-equiv="refresh" content="0; url=manual/">
<link rel="canonical" href="@SITE@manual/"></head>
<body><p>The manual is at <a href="manual/">manual/</a>.</p></body></html>
"""


def slugify(text):
    import re
    s = re.sub(r"<[^>]+>", "", text).lower()
    return re.sub(r"[^a-z0-9]+", "-", s).strip("-") or "section"


def convert(body):
    """Turns the page markup into the Voltkit manual's conventions and returns (html, headings)."""
    import re
    body = re.sub(r'<aside class="callout (\w+)"><strong>(.*?)</strong> (.*?)</aside>',
                  lambda m: f'<div class="callout {"warning" if m.group(1) == "warn" else "note" if m.group(1) == "hw" else m.group(1)}"><p><strong>{m.group(2)}</strong> {m.group(3)}</p></div>',
                  body, flags=re.S)
    body = body.replace('<div class="table-wrap">', '<div class="table">').replace('<ol class="steps">', "<ol>").replace('<figure class="narrow">', "<figure>")
    body = re.sub(r"<details( open)?><summary>(.*?)</summary>(.*?)</details>", r"<h3>\2</h3>\3", body, flags=re.S)
    heads, seen = [], set()

    def head(m):
        level, attrs, text = m.group(1), m.group(2), m.group(3)
        found = re.search(r'id="([^"]+)"', attrs)
        ident = found.group(1) if found else slugify(text)
        while ident in seen:
            ident += "-2"
        seen.add(ident)
        if level in "23":
            heads.append((int(level), ident, re.sub(r"<[^>]+>", "", text)))
        anchor = f'<a class="anchor" href="#{ident}" aria-label="Link to this section">#</a>' if level != "1" else ""
        return f'<h{level} id="{ident}">{text}{anchor}</h{level}>'

    body = re.sub(r"<h([1-3])([^>]*)>(.*?)</h\1>", head, body, flags=re.S)
    return body, heads


def build():
    import json, re, shutil
    manual = DOCS / "manual"
    manual.mkdir(parents=True, exist_ok=True)
    (manual / "img").mkdir(exist_ok=True)
    shutil.copy(DOCS / "icon.svg", manual / "img" / "icon.svg")

    intro_rows = "".join(
        f'<tr><td><a href="{slug}.html">{e(title)}</a></td><td>{e(summary)}</td></tr>' for slug, title, summary in PAGES
    )
    CONTENT["index"] = f"""
<p><strong>Battery Health Center</strong> is a native GNOME app for the battery in your laptop. It shows how healthy the battery is, lets you protect it with a charge limit and a schedule, and shows which apps are draining it. This manual explains every feature of the app, page by page, with screenshots.</p>
{fig("overview", "Battery Health Center's Overview page, the page the app opens on", "Battery Health Center's Overview page, the page the app opens on")}
<h2 id="how-to-read-this-manual">How to read this manual</h2>
<ul>
<li><strong>New here?</strong> Start with <a href="getting-started.html">Getting started</a>.</li>
<li><strong>Looking for one page of the app?</strong> Pick it in the left menu. Each page of this manual follows the app's own pages, with every control explained.</li>
<li><strong>Looking for a setting, a file or a command?</strong> See <a href="reference.html">Reference</a>.</li>
<li><strong>Something is not working?</strong> See <a href="troubleshooting.html">Troubleshooting and FAQ</a>.</li>
<li>Press <kbd>/</kbd> anywhere in the manual to search it.</li>
</ul>
<h2 id="the-pages">The pages of this manual</h2>
<div class="table"><table><thead><tr><th>Page</th><th>What it covers</th></tr></thead><tbody>{intro_rows}</tbody></table></div>
<h2 id="principles">Principles</h2>
<ul>
<li><strong>Native and light.</strong> Rust with GTK 4 and libadwaita. It follows your GNOME theme, light and dark.</li>
<li><strong>Private by design.</strong> Everything runs on your computer. The app makes no network requests and sends no telemetry. The only things that leave your computer are the links you click yourself, such as this manual.</li>
<li><strong>Careful with root.</strong> Only a small helper runs as root. It accepts nothing but validated values and writes only the battery's charge-limit files and its own configuration.</li>
<li><strong>Accessible.</strong> Screen reader names and descriptions, and charts that never rely on color alone.</li>
<li><strong>In your language.</strong> The app follows your system language: English, Spanish, German, French and Simplified Chinese. This manual is in English.</li>
</ul>
<h2 id="getting-help">Getting help</h2>
<ul>
<li>In the app, press <kbd>F1</kbd> or click the <strong>?</strong> button at the top of the sidebar to open this manual.</li>
<li>Found a bug or have an idea? <a href="{REPO}/issues">Open an issue</a>.</li>
</ul>
"""
    CONTENT["getting-started"] = CONTENT["getting-started"]

    pages = []
    for slug in ORDER:
        if slug == "index":
            title, summary = TITLES["index"], ""
        else:
            title = TITLES[slug]
            summary = next(s for sl, _, s in PAGES if sl == slug)
        body, heads = convert(f"<h1>{e(title)}</h1>" + CONTENT[slug])
        if slug != "index":
            body = body.replace("</h1>", "</h1>", 1)
        text = re.sub(r"\s+", " ", re.sub(r"<[^>]+>", " ", body))
        group = next(g for g, slugs in GROUPS if slug in slugs)
        pages.append(dict(slug=slug, title=title, summary=summary, body=body, heads=heads, text=text, group=group))

    def nav(current):
        out = ""
        for group, slugs in GROUPS:
            out += f'<div class="group"><div class="group-title">{e(group)}</div><ul>'
            for slug in slugs:
                cur = ' aria-current="page"' if slug == current else ""
                out += f'<li><a href="{slug}.html"{cur}>{e(TITLES[slug])}</a></li>'
            out += "</ul></div>"
        return out

    (manual / "search.json").write_text(json.dumps([
        dict(url=f'{p["slug"]}.html', title=p["title"], group=p["group"], text=p["text"][:6000],
             heads=[dict(id=i, t=t) for _, i, t in p["heads"]]) for p in pages], ensure_ascii=False))

    byline = f'<p class="byline">By <a href="{AUTHOR}">Alexis Rios-Negron</a> · Battery Health Center {VERSION}</p>'
    for n, p in enumerate(pages):
        toc = "".join(f'<li class="l{lvl}"><a href="#{i}">{e(t)}</a></li>' for lvl, i, t in p["heads"])
        prev = pages[n - 1] if n else None
        nxt = pages[n + 1] if n + 1 < len(pages) else None
        pager = '<nav class="pager" aria-label="Previous and next page">'
        pager += f'<a class="prev" href="{prev["slug"]}.html"><span>Previous</span>{e(prev["title"])}</a>' if prev else "<span></span>"
        pager += f'<a class="next" href="{nxt["slug"]}.html"><span>Next</span>{e(nxt["title"])}</a>' if nxt else "<span></span>"
        pager += "</nav>"
        desc = p["summary"] or "Battery Health Center is a native GNOME app that shows battery health, protects it with charge limits and schedules, and shows which apps drain it."
        page = PAGE
        for key, value in {
            "@TITLE@": e(p["title"]), "@SITE@": SITE_TITLE, "@DESC@": e(desc), "@REPO@": REPO, "@NAV@": nav(p["slug"]),
            "@TOC@": toc, "@PAGER@": pager, "@VERSION@": VERSION,
        }.items():
            page = page.replace(key, value)
        body = p["body"].replace("</h1>", "</h1>" + byline, 1).replace("@IMG@", "img/")
        (manual / f'{p["slug"]}.html').write_text(page.replace("@BODY@", body))

    # The site root only forwards to the manual, so the address the app opens keeps working.
    (DOCS / "index.html").write_text(REDIRECT.replace("@SITE@", SITE))
    (DOCS / ".nojekyll").write_text("")

    # A broken link or image fails the build.
    ids = {f'{p["slug"]}.html': set(re.findall(r'id="([^"]+)"', (manual / f'{p["slug"]}.html').read_text())) for p in pages}
    bad = []
    for p in pages:
        text = (manual / f'{p["slug"]}.html').read_text()
        for href in re.findall(r'href="([^"]+)"', text):
            if href.startswith(("http://", "https://", "mailto:")):
                continue
            target, _, frag = href.partition("#")
            target = target or f'{p["slug"]}.html'
            if target not in ids and not (manual / target).exists():
                bad.append(f'{p["slug"]}: no page {href}')
            elif frag and target in ids and frag not in ids[target]:
                bad.append(f'{p["slug"]}: no section {href}')
        bad += [f'{p["slug"]}: missing {src}' for src in re.findall(r'(?:src|srcset)="([^"]+)"', text) if not (manual / src).exists()]
    if bad:
        raise SystemExit("Broken links in the manual:\n  " + "\n  ".join(bad))
    print(f"built {len(pages)} pages")


if __name__ == "__main__":
    build()
