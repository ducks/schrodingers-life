# Schrödinger's Life

A collective creature that is alive only while somebody is observing it.

Visiting the site opens a WebSocket observation. Visible browser tabs keep the
current life alive. When the final observer leaves, a 30-second decoherence
countdown begins. If nobody returns, that creature dies permanently and joins
the graveyard. The next observer begins a new life. If the apparatus restarts,
any life that was active at shutdown is recorded as having died while it was
offline. Returning observers are shown a memorial for the life that died while
they were away before they can open the box again. The graveyard records each
life's duration and peak observer count, and the site highlights the longest
life and provides a share action for the current observation.

## Run

```bash
cargo run
```

Then open <http://127.0.0.1:3000>.

Configuration:

- `SCHRODINGER_ADDR` — listen address, default `127.0.0.1:3000`
- `SCHRODINGER_DB` — SQLite path, default `schrodingers-life.db`
- `SCHRODINGER_GRACE_SECONDS` — final-observer grace period, default `30`
- `SCHRODINGER_ORIGIN` — optional public origin such as
  `https://schrodingers.life`; WebSocket observations otherwise require an
  origin matching the request host

`GET /healthz` checks the process and database and is suitable for a service
health check. Dynamic state responses are marked `Cache-Control: no-store`.
