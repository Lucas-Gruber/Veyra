# Design system — foundation

Veyra uses quiet violet accents, neutral charcoal surfaces and a three-pane desktop layout: navigation, searchable item list and a focused detail view. The wordmark and the small V symbol stay consistent in the main window and Quick Access. Typography uses a local system sans-serif stack; icons are bundled SVG components, with no remote favicon or font fetches.

`app/assets/css/main.css` defines semantic surface, text, border and accent tokens. Light mode overrides the same tokens, and System follows `prefers-color-scheme`. Modern is opaque; Glass adds in-window translucency with an opaque fallback. It does not promise compositor-level background blur.

Spacing, restrained corner radii and subtle focus rings are shared by buttons, fields, list rows and dialogs. Hover must not be the only indication of selection. Quick Access opens with search focus, keeps active results visible and provides an action menu. Dialogs contain keyboard focus and restore it when dismissed. Small-screen layouts collapse navigation and item details rather than shrinking every control.

Motion is brief and functional; reduced-motion media queries remove transitions and animations. Every icon-only control has an accessible name. All credential values are masked by default, explicit reveals expire and locking removes visible metadata. Demo notices remain visible in locked, unlocked and quick modes.

The CSS is the source of truth for current tokens. Contrast, assistive technology and HiDPI behavior still need independent testing on KDE and GNOME; browser keyboard tests alone do not establish desktop accessibility conformance.
