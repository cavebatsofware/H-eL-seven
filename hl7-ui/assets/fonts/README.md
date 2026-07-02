# Bundled fonts

Vendored so the app never depends on a webfont CDN or system-installed fonts
(webkitgtk desktop and offline browsers both lack them; missing text fonts let
Noto Color Emoji hijack digit glyphs).

Both are the **latin** subset, variable weight 400–700, from Google Fonts.

| File | Family | Upstream | License |
|------|--------|----------|---------|
| `Inter-latin.woff2` | Inter | https://fonts.google.com/specimen/Inter | SIL OFL 1.1 |
| `JetBrainsMono-latin.woff2` | JetBrains Mono | https://fonts.google.com/specimen/JetBrains+Mono | SIL OFL 1.1 |
