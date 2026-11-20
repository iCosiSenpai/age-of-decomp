# Optional diagnostics

Age of Decomp asks for your choice on first launch when the diagnostics service is available. You can change it
in **System → Anonymous diagnostics**. Diagnostics start disabled. Turning them off removes pending reports.
A game started from RecompLauncher sends no diagnostics yet: RecompLauncher starts it without the launcher window,
where the choice is made, and keeps its files in its own data folder. Send the log with your report there instead.
The choice is stored separately from ordinary preferences, so saving settings in another window cannot
turn sharing back on.

A report contains the compiler version, the operating system family, a fixed event category and a fixed detail
code. A completed game may also report average frame rate rounded to the nearest 5 FPS. With consent, an
unexpected compiler exit can be counted when it next opens. A local run marker also treats a forced close or
power loss as an unexpected exit. Full logs, game and save files, filenames, usernames, device addresses and
installation identifiers stay on the PC.

The collector stores daily counts for these fields for up to 90 days. It has no public read endpoint. Cloudflare
handles HTTPS and necessarily receives the network address used for the request. The collector does not read it
or include it in the counts, and Worker observability is disabled.

For maintainers, the collector uses Cloudflare Workers and a D1 database on the free plan. One accepted report
requires one database write. The account owner can see the counts in the D1 console.

1. Sign into a Cloudflare account on Workers Free and run `npx wrangler@4.165.0 login`.
2. In `tools/diagnostics/`, copy `wrangler.example.toml` to the ignored `wrangler.toml`. Create the database with
   `npx wrangler@4.165.0 d1 create age-of-decomp-diagnostics` and put its ID in `wrangler.toml`.
3. Apply the schema with `npx wrangler@4.165.0 d1 execute age-of-decomp-diagnostics --remote --file schema.sql`,
   then run `npx wrangler@4.165.0 deploy`.
4. The release builder sets `AOD_DIAGNOSTICS_URL` to the HTTPS Worker URL ending in `/v1/reports` when compiling
   both binaries. Omit it for offline builds. A release without an endpoint has no prompt and sends nothing.

Run `node --test tools/diagnostics/worker.test.mjs` to test validation, aggregation and expiry locally.
