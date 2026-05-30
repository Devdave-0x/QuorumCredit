# QuorumCredit API Rate Limiting Middleware

Off-chain Express middleware for per-IP and per-API-key rate limiting (issue #719).

## Usage

```js
const express = require('express');
const { RateLimiter } = require('./middleware/rate_limiter');

const app = express();

const limiter = new RateLimiter({ windowMs: 60_000, maxRequests: 100 });
app.use(limiter.middleware());
```

## Options

| Option | Default | Description |
|---|---|---|
| `windowMs` | `60000` | Sliding window duration in milliseconds |
| `maxRequests` | `100` | Maximum requests allowed per window |

## Behaviour

- If the request includes an `x-api-key` header, that key is used as the rate limit identifier.
- Otherwise, the client IP (`req.ip`) is used.
- Exceeded requests receive HTTP **429** with a `Retry-After` header (seconds until window resets).

## Algorithm

Sliding window — only timestamps within the last `windowMs` milliseconds are counted, giving a smooth rate limit without the burst spikes of a fixed window.

## Example: separate limits for authenticated vs anonymous traffic

```js
const keyLimiter = new RateLimiter({ windowMs: 60_000, maxRequests: 500 });
const ipLimiter  = new RateLimiter({ windowMs: 60_000, maxRequests: 30  });

app.use((req, res, next) => {
  const mw = req.headers['x-api-key'] ? keyLimiter.middleware() : ipLimiter.middleware();
  mw(req, res, next);
});
```
