---
version: alpha
name: Battery Health Center
description: Native GNOME battery monitoring and charging controls, visually aligned with Voltkit.
colors:
  primary: '#2167d5'
  background: '#f3f5f8'
  surface: '#ffffff'
  foreground: '#202631'
  dark-background: '#1b2026'
  dark-surface: '#222930'
typography:
  sans:
    fontFamily: system-ui, sans-serif
  mono:
    fontFamily: monospace
rounded:
  DEFAULT: 16px
  control: 8px
spacing:
  workspace: 20px
  card: 18px
components:
  workspace: {}
  preferences: {}
  dialog: {}
  feedback: {}
---

# Battery Health Center Design System

## Overview
The reference is the existing Voltkit-aligned native GNOME application, not a web dashboard. Laptop users monitor health, understand energy use and explicitly control supported firmware settings. Preserve the five workspaces and the charge ring. Product register; global audience, with English, Spanish, German, French and Simplified Chinese. No Japan-specific market or data contract is in scope. Use the desktop language and local calendar/time. Evidence: README.md, docs/HOW-IT-WORKS.md, src/ui and src/i18n.rs.

Runtime tokens are owned by src/ui/appearance.rs and appearance.css; this document mirrors their existing values. Use libadwaita/system typography and high-contrast palette, with numeric text for measurements. Do not introduce a second token system.

## Colors
Use the existing blue accent, native semantic error/warning/success colors, and theme-relative card surfaces. Charts and warnings also carry text or shapes. High contrast remains desktop-owned.

## Typography
System fonts cover all supported scripts. Existing title, heading, caption and numeric CSS roles remain canonical. Localized labels may wrap; values and actions must remain reachable.

## Layout
Workspace owns scrolling, responsive columns and centered settings. Overview, Health and Usage can use two columns; Charging and Power remain a single readable column. Settings and diagnostics extend these existing workspaces rather than adding top-level navigation.

## Elevation & Depth
Keep the current restrained card borders and shadows. Native dialogs own modal elevation and scrolling.

## Shapes
Use existing rounded cards, native controls and symbolic icons. The application battery icon is a brand asset, not an action icon.

## Components
PreferencesDialog and PreferencesGroup own settings; ComboRow owns single choice; SpinRow owns bounded numbers; EntryRow owns typed date/time; FileDialog owns local file selection. AlertDialog owns explicit destructive confirmations. ToastOverlay reports completed actions; persistent banners report unresolved errors. No duplicated screen-local confirmation framework.

GTK owns keyboard behavior, accessible control names, focus restoration, reduced motion, native scrollbars and popup geometry. Date/time entry explicitly names the local format and validates before applying. Use the existing gettext helpers for all product strings.

## Do's and Don'ts
- Preserve the established theme and accessible native controls.
- Explain scope and consequences before replacing or clearing history.
- Show loading, disabled, empty and failure states honestly.
- Never hide a failed save, fabricate a measurement, or automatically disable another power manager.
