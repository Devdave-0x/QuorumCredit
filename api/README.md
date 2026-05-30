# QuorumCredit API

A REST API backend for QuorumCredit with JWT authentication, structured logging, and webhook support.

## Features

### 1. JWT Authentication (#720)
- Generate JWT tokens for API access
- Verify token validity
- Configurable token expiration

**Endpoints:**
- `POST /auth/token` - Generate a new JWT token
- `POST /auth/verify` - Verify a JWT token

### 2. Structured Request Logging (#721)
- Log all API requests with structured JSON format
- Track request duration, status codes, and errors
- Filter logs by API key
- Persistent in-memory log storage

**Endpoints:**
- `GET /logs` - Retrieve all request logs

### 3. Webhook Support (#722)
- Subscribe to contract events via webhooks
- Automatic retry mechanism with exponential backoff
- Track webhook delivery status
- Support for multiple event types

**Endpoints:**
- `POST /webhooks/subscribe` - Subscribe to webhook events
- `DELETE /webhooks/unsubscribe` - Unsubscribe from webhooks
- `POST /webhooks/events` - Deliver webhook events

## Setup

### Prerequisites
- Rust 1.70+
- Tokio runtime

### Installation

1. Copy `.env.example` to `.env` and configure:
```bash
cp .env.example .env
```

2. Set your JWT secret:
```bash
echo "JWT_SECRET=your_secure_secret_key" >> .env
```

3. Build the API:
```bash
cargo build --release -p quorum_credit_api
```

4. Run the server:
```bash
cargo run -p quorum_credit_api
```

The server will start on `http://localhost:3000` (or the port specified in `.env`)

## API Usage

### Authentication

Generate a token:
```bash
curl -X POST http://localhost:3000/auth/token \
  -H "Content-Type: application/json" \
  -d '{"api_key": "my_api_key"}'
```

Response:
```json
{
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
}
```

Verify a token:
```bash
curl -X POST http://localhost:3000/auth/verify \
  -H "Content-Type: application/json" \
  -d '{"token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."}'
```

### Webhooks

Subscribe to events:
```bash
curl -X POST http://localhost:3000/webhooks/subscribe \
  -H "Content-Type: application/json" \
  -d '{
    "url": "https://your-domain.com/webhook",
    "events": ["loan.created", "loan.repaid"]
  }'
```

Deliver an event:
```bash
curl -X POST http://localhost:3000/webhooks/events \
  -H "Content-Type: application/json" \
  -d '{
    "event_type": "loan.created",
    "data": {"loan_id": "123", "amount": 1000}
  }'
```

### Logging

Retrieve request logs:
```bash
curl http://localhost:3000/logs
```

## Architecture

- **auth.rs**: JWT token generation and verification
- **logging.rs**: Structured request logging with tracing
- **webhook.rs**: Webhook subscription and delivery management
- **main.rs**: Axum web server and route handlers

## Testing

Run tests:
```bash
cargo test -p quorum_credit_api
```

## Error Handling

All endpoints return appropriate HTTP status codes:
- `200 OK` - Successful request
- `201 Created` - Resource created
- `202 Accepted` - Request accepted for processing
- `204 No Content` - Successful deletion
- `400 Bad Request` - Invalid input
- `401 Unauthorized` - Authentication failed
- `404 Not Found` - Resource not found
- `500 Internal Server Error` - Server error

## Logging

Logs are output in JSON format with the following fields:
- `timestamp` - Request timestamp
- `method` - HTTP method
- `path` - Request path
- `status` - HTTP status code
- `duration_ms` - Request duration in milliseconds
- `api_key` - Associated API key (if authenticated)
- `ip` - Client IP address
- `error` - Error message (if applicable)

## Security Considerations

1. **JWT Secret**: Use a strong, randomly generated secret in production
2. **HTTPS**: Always use HTTPS in production
3. **Rate Limiting**: Consider implementing rate limiting for production use
4. **Webhook Validation**: Validate webhook signatures in production
5. **Log Retention**: Implement log rotation and retention policies

## Future Enhancements

- Database persistence for logs and webhooks
- Rate limiting and throttling
- Webhook signature verification
- API key management and rotation
- Metrics and monitoring
- Multi-tenant support
