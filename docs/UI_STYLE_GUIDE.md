# UI Style Guide

## Goal

BakkesSwap should feel like a compact desktop utility for local mod-style configuration work.

The shell should read closer to a BakkesMod-style settings window or dark ImGui tool than to a website, dashboard hero, or marketing page.

## Core Direction

- dark utility-panel shell
- fixed left sidebar on desktop
- compact top status bar
- dense but readable controls
- compact panels and tables
- clear status badges for runtime, path posture, and workflow state
- smaller typography and tighter spacing than a web app landing page

## Layout Rules

- keep the left sidebar dedicated to navigation, runtime state, and non-negotiable guardrails
- keep the current `CookedPCConsole` visible in the top status area during risky workflows
- avoid large empty hero regions and oversized card spacing
- prefer stacked panels, split utility columns, and compact tables
- allow mobile collapse, but optimize first for desktop utility use

## Visual Language

- favor dark gray, near-black, and muted steel tones over bright product-marketing colors
- use orange sparingly as the main action accent
- use green, yellow, and red for status only
- keep panel borders subtle and consistent
- use small radii instead of large rounded web cards

## Typography

- use compact headings
- use uppercase micro-labels for panel headers, table columns, and badges
- keep normal body copy tight and factual
- use monospace for paths, commands, confirmation phrases, and logs

## Component Rules

### Panels

- use `.panel` for the base shell unit
- use `.panel-header` for consistent title and action alignment
- keep panels compact and information-dense

### Status

- use `.status-badge` for short runtime or state indicators
- badge text should stay short and avoid long profile names when possible
- use `.success`, `.warning`, and `.danger` only for meaningful state, not decoration

### Actions

- buttons should look like tool actions, not primary marketing CTAs
- keep button labels short and operational
- disabled buttons must remain visually obvious
- focus states must stay clear

### Tables And Rows

- use `.compact-table` for dense key/value and list layouts
- truncate or scroll long path values instead of letting them explode row height
- keep row actions visible and aligned

### Paths And Logs

- use `.path-text` for long Windows paths
- long paths should prefer monospace, truncation, or horizontal scroll
- use `.monospace-log` for session activity output
- keep logs dark, high-contrast, and terminal-like

## Page-Specific Guidance

### Home

- read like a utility overview, not a welcome page
- keep the path, counts, and recent activity visible quickly

### Quick Swap

- always present TARGET on the left and SOURCE on the right
- keep selected item cards compact and factual
- keep compatibility and plan/build actions in one dense panel below the split columns

### Install Preview

- present the page like a confirmation tool window
- keep file changes and backup paths in compact tables
- keep blockers and warnings visible without turning the page into giant banners

### Active Swaps

- use a compact table first
- keep restore actions visible per row
- put the restore tool panel directly after the table

### Diagnostics

- prefer compact key/value tables
- keep the page factual and operational

### Logs

- keep the visual language close to a terminal panel
- show timestamps, command names, and details in a stable monospace grid

## Hard Boundaries

- do not move backend logic into the frontend for style reasons
- do not add overlay behavior, runtime hooks, or online-facing affordances
- keep all smoke validation sandboxed and local-only
