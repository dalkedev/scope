# Snurpo identity

**A nosy little HTTP debugger.**

Snurpo is an inquisitive mint creature with an enormous lavender nose, cream eyes,
small hands and feet, and a slightly suspicious expression. Its silhouette and
nose are the identifying features. Keep the entire nose visible and leave clear
space around the character.

## Source assets

| Asset | Purpose |
| --- | --- |
| [snurpo-logo.png](../assets/brand/snurpo-logo.png) | Original 1254 × 1254 transparent mascot; desktop icon source. |
| [snurpo-ui.png](../assets/brand/snurpo-ui.png) | 128 × 128 transparent export for the desktop interface and favicon. |
| [snurpo-banner.png](../assets/brand/snurpo-banner.png) | 1774 × 887 repository/portfolio illustration; not an application screenshot. |
| [snurpo-tray-source.png](../assets/brand/snurpo-tray-source.png) | Original transparent, monochrome head for the macOS menu bar. |
| [provenance.json](../assets/brand/provenance.json) | Generation details, exact prompt references, dimensions and SHA-256 hashes. |

The supporting palette uses lavender, mint, cream and charcoal. The desktop uses
lavender for interactive selections/focus and mint for small brand details; HTTP
status, errors, warnings and capture states retain their semantic meaning in both
light and dark appearances. Do not stretch
the character or use the banner as an icon source. Keep product information in
text so that the README remains understandable without its images.

Desktop PNG, ICO and ICNS variants live in `src-tauri/icons/`. Regenerate with the
locked Tauri CLI into a temporary directory and replace only existing desktop
files; generated Android/iOS assets are not used by this application:

```sh
npm ci
npm run tauri -- icon assets/brand/snurpo-logo.png --output .runtime/snurpo-icons
```

The macOS status item uses `src-tauri/icons/snurpo-tray-template.png`: a 36 × 36
transparent template with the eye and face holes preserved. Its native renderer
normalizes the image to 18 logical points and macOS supplies the appropriate
light/dark tint. The [tray prompt](../assets/brand/tray-prompt.md) and technical
trim/resize export are recorded in provenance. The menu offers **Show Snurpo**
and **Quit Snurpo**; it does not change capture or launch-at-login settings.

## Voice and provenance

Be curious, playful and concise in welcome or promotional copy. Controls, errors
and technical documentation should remain direct: capture, inspect, edit, replay,
save and compare. Do not imply a hosted service, universal pinning bypass,
independent security audit or features the application does not implement.

The mascot and banner were generated with OpenAI image generation on 12 November
2026. The banner uses the mascot as an identity reference. Exact prompts are
preserved in [mascot-prompt.md](../assets/brand/mascot-prompt.md) and
[banner-prompt.md](../assets/brand/banner-prompt.md). These original raster assets
are unchanged; desktop icons and the small UI image are technical Tauri exports.
The menu-bar template is a technical sharp export of its separate generated source.
No private conversations, account content, third-party photographs or application
captures were supplied for the artwork. Existing real screenshots and their
[capture provenance](images/README.md) remain historical evidence.

Snurpo artwork is distributed under **AGPL-3.0-or-later**, like the project; see
[LICENSE](../LICENSE). Third-party notices remain intact. The former Scope artwork
and its provenance are archived under [assets/brand/legacy](../assets/brand/legacy/).

This branding is in the source tree. Published Scope 1.4.1 installers retain their
original names and artwork until a new release is actually built and published.
See [migration notes](MIGRATING_TO_SNURPO.md) and [release instructions](RELEASING.md).
`snurpo.dev` is a domain candidate, not a purchased or connected destination.
