# Interface direction

Scope is a working network debugger. Information density and clear message ownership take precedence over decorative dashboard cards.

Palette: paper #f7f9fc, panel white #ffffff, slate ink #213047, divider #e3e8ef, cobalt #3468dc and teal #168978. HTTP errors use a muted red with a text status code, never color alone. Locally bundled sans type supports controls; a local monospace supports payloads.

Appearance: Light preserves the paper palette. Dark uses a #101721 canvas,
#17212e panels and #131c28 recessed editors, with #e3ebf5 primary text and
#a4b4c8 secondary annotations. Semantic `--scope-*` roles in `src/theme.css`
cover selection, focus, controls and success/warning/error states; existing light
values remain explicit fallbacks in the component stylesheets. The sidebar's
Light/Dark/System control persists only the local appearance preference. System
is the default and follows live desktop changes. A pre-render initializer avoids
a light flash; the existing native CSP hashes it without widening script access.

Layout: a persistent left rail for devices and archives, a central traffic table and a right inspector. Capture state, breakpoint state and archive state stay distinct. Full-page tools handle analysis, comparison, device setup and MCP.

Empty archives invite an actual device connection or explicit generation of real local test traffic. No fabricated requests appear at startup. Motion only acknowledges user action and connection state.

The palette and layout were reviewed against the brief: this is a dense debugging workspace with a live table and payload inspection, not a generic KPI dashboard.
