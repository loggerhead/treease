---
summary: "Desktop Workspace contract: platform boundaries, durable constraints, and decision rationale."
read_when:
  - Designing or changing the Tauri host, platform capabilities, sessions, deep links, or desktop distribution
  - Deciding whether a desktop capability may enter the shared Web workspace or Core
---

# Desktop Workspace Architecture Contract

This document is the source of architectural decisions for `Desktop Workspace`. It defines stable boundaries and constraints; it does not prescribe implementation slices, acceptance steps, or test plans.

## Shared Workspace and Platform Boundary

Desktop Workspace and Web Workspace share the same Svelte workspace, primary-document path, and interaction semantics. The desktop app uses one native window to host multiple `Workspace Tab`s and does not build a separate editor. Platform capabilities such as open, save, window events, and deep links may only pass through `Workspace Host`. Shared components and `Document Runtime` must not directly depend on Tauri or browser-specific APIs.

This prevents platform branches from leaking into the shared workspace and avoids duplicating editor, parsing, formatting, graph-building, or snapshot semantics. `packages/core` introduces no desktop dependency or conditional logic.

## Workspace and File Permissions

One window can open multiple independent tabs. Saving is explicit by default; `Hot Exit` restores only unsaved state and does not introduce folder workspaces, file trees, or multi-root workspaces.

Local files use least-privilege `File Access Grant`: read, write, or watch only the specific files that users explicitly give the app through system dialogs, drag and drop, or file associations. The desktop app must not scan directories or request home-directory, full-disk, or other broad permissions.

## Application Identity and Deep Links

The display name is `Treease`, the Bundle / App ID is `com.treease.desktop`, and the deep-link scheme is `treease://`. These values are the common stable identity for system credentials, deep links, file associations, and future signed distribution.

`treease://editor?...` is the only business deep link and reuses `/editor` preset-parameter parsing and validation. Unknown routes or parameters receive no desktop access; `https://treease.com/...` is always a browser link and is never handled by the desktop app.

## Authentication and Sessions

Authentication completes in the system browser and returns to the single-instance desktop app through a registered deep link. Only the desktop host may store the refresh token, using macOS Keychain or Windows Credential Manager; it must not be stored in plaintext in WebView storage or the app directory, and `packages/core` does not participate in this capability.

## Privacy and External Content

The desktop app enables minimized Google Analytics product analytics by default and may report only operation type, format, status, and result. It must not report filenames, paths, text, graph content, session credentials, or local unique identifiers.

To preserve value-preview interactions, HTTPS images may load; remote pages and scripts must not execute in the WebView. Ordinary external links open in the system browser, and host network access is limited to known Treease service, authentication, analytics, and static-resource endpoints.

## Distribution and Updates

The first release distributes Windows x64 and macOS Apple Silicon builds through GitHub Releases. The Tauri updater signs with a separate update key and the app verifies with its embedded public key. The app checks for updates in the background, but the user must confirm download and restart installation; silent updates and downgrades are unsupported, and no paid update service is required.

Before eligibility for the Apple Developer Program, macOS uses ad-hoc signing and is distributed as a development preview. Microsoft Store or paid Developer ID signing/notarization is not a prerequisite for the first release.
