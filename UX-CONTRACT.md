# Battery Health Center behavior contract

Sources: the user-requested functionality enhancements; README.md; docs/HOW-IT-WORKS.md; existing settings, storage, helper and UI modules.

## Canonical UI map
| Capability | Owner | Variant and verification |
|---|---|---|
| Select/Listbox | libadwaita ComboRow | Native keyboard/popup; existing Preferences patterns |
| Date | EntryRow + schedule.rs local-calendar validation | Explicit YYYY-MM-DD and HH:MM; validate before privileged submission |
| Form | PreferencesGroup / PreferencesDialog | Bounded native fields, save rollback and persistent error feedback |
| Scrollbar | GTK/libadwaita, Workspace | Native accessible scrollbar and workspace scrolling |
| Toast | ToastOverlay | Successful operations; errors remain visible until resolved |
| CRUD | Controller, history management and privileged helper | Shared ownership/locks; confirmed history replacement/clear; no silent overwrite |

## Data lifecycle and user decisions
- Keep default battery retention at 30 days, app activity at 7 days and health history indefinitely. A user may select different periods; shortening retention requires a confirmation before existing records are pruned.
- Clear actions explicitly select battery, app or health history; default to no selected datasets. Confirmation states that records will be removed. Cancel is the initial/safe action.
- Backups contain local history, including app names and battery identifiers; use a native save picker. They do not contain charge settings, root-helper configuration or credentials.
- Restore validates the entire backup before replacing history and requires confirmation. Coordinate with the current writer and reload all open copies. Reject unsupported or malformed backups without changing history.
- Legacy unidentified health history remains available separately and is never silently attributed to a battery. Different physical identities have separate trends; without a serial, explain that replacement detection is limited.
- One-off plans use the local calendar, expire after their grace window, and preserve recurring rules. Skipping targets one exact upcoming ready-by event, including its lead/grace window; it must not disable the recurring schedule.
- Diagnostics report services, competing managers, changed thresholds and failed automatic switches. They never stop services or override another manager without a user action.

## Flow ledger
| Operation | Pending | Success | Failure |
|---|---|---|---|
| Save preferences | Native controls; prevent duplicate destructive submit | Stay in preferences; persist before applying retention | Restore previous values; show error |
| Set one-off / skip | Disable submit while helper runs | Refresh charging status and show toast | Preserve plan and input; show actionable error |
| Backup | Disable action while writing | Show saved destination / completion | Retain all source history; show error |
| Restore / clear | Native confirmation and writer lock | Reload charts and counters across copies | Keep original data; show rollback/validation error |

All new controls use gettext, accessible names, keyboard operation and narrow-window scrolling. Changes remain on the feature branch until explicitly released.
