# notenwart – brand assets

**The mark:** a crest-shaped shield (the *Wart*, the keeper) with staff lines and an eighth note in brass. Two
weights:

- **Regular** – five staff lines; 64 px and up.
- **Small** – three heavier lines, heavier outline, slightly larger note; 16–48 px (favicon, tabs, small app slots).

Every variant exists **light** (navy on paper) and **dark** (paper on navy). The note is always brass. The gap
between the note and the staff lines is a mask, so the transparent marks work on any background.

Regenerate everything from code: `python3 brand/generate.py` (needs `rsvg-convert` from Homebrew `librsvg`; the
lockup and social PNGs use the Charter font). Edit `generate.py`, not the exported files.

## Colours

| Name | Hex | Use |
|---|---|---|
| Navy (ink) | `#1D2B45` | outline and lines on light, dark backgrounds, text |
| Paper (cream) | `#F5EFE3` | light backgrounds, ink on dark |
| Brass | `#B8862E` | the note on light backgrounds, accents |
| Brass light | `#D9A441` | the note on dark backgrounds, accents on dark |

## Type

**Charter** for the wordmark and headings (bundled with macOS and iOS; free as *Bitstream Charter*), with the stack
`Charter, "Bitstream Charter", "Iowan Old Style", Georgia, serif`. Body text uses the system sans-serif. No web
fonts are loaded from other servers.

## Name

Always written in lowercase – **notenwart**, also at the start of a sentence, in titles and in the wordmark;
the same in handles and domains. Don't write *Notenwart* or *NOTENWART* in running text (environment variables
such as `NOTENWART_PUBLIC_URL` follow their own convention).

## Where to use what

| Use | File |
|---|---|
| Web app and website `<head>` | `web/` – `favicon.ico`, `favicon.svg` (follows light/dark mode), `apple-touch-icon.png`, `icon-192.png`, `icon-512.png`, `icon-maskable-512.png` |
| App icon as an image, rounded tile | `png/icon/notenwart-icon-{light,dark}-{16…1024}.png` (≤ 48 px use the small mark) |
| Edge to edge (platforms that round the icon themselves) | `svg/notenwart-icon-square-{light,dark}.svg` |
| Transparent mark | `svg/notenwart-mark-on-{light,dark}.svg`, `png/mark/` |
| Transparent mark, tiny | `svg/notenwart-mark-small-on-{light,dark}.svg` |
| Monochrome (print, stamps, single colour) | `svg/notenwart-glyph-{black,white}.svg` |
| Mark and wordmark (README, documents) | `svg/notenwart-lockup-on-{light,dark}.svg` (live text, needs Charter or a serif), `png/lockup/` |
| GitHub organisation avatar | `social/github-avatar-500.png` |
| Social preview (GitHub repos, website `og:image`) | `social/notenwart-social-1280x640.png` |

Website head:

```html
<link rel="icon" href="/favicon.ico" sizes="any">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="apple-touch-icon" href="/apple-touch-icon.png">
<link rel="manifest" href="/site.webmanifest">
```

## Rights

notenwart's name and brand belong to falkOSS – Falcon Lake Open Source Software gUG (haftungsbeschränkt) i. G., the
publisher of notenwart. They are **not** covered by the software licence (AGPL-3.0-or-later). The look stays
notenwart's own; project pages say "a falkOSS project" at most. The artwork may be used to refer to notenwart, not
for other products or in a way that suggests endorsement.
