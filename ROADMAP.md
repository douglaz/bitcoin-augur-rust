# Bitcoin Augur Rust - Implementation Roadmap

## Current Status (2025-08-22)
The core library and server infrastructure are complete. The remaining work focuses on the HTTP API layer and final integration.

## Immediate Next Steps

### 🔴 Priority 1: HTTP API Implementation
**Goal**: Create REST endpoints matching Kotlin implementation

#### Task 1.1: Create API Models
```rust
// src/api/models.rs
- FeeEstimateResponse
- BlockTargetResponse  
- ProbabilityResponse
- Transform functions from internal types
```

#### Task 1.2: Implement Fee Endpoints
```rust
// src/api/fee_endpoint.rs
- GET /fees -> current estimates
- GET /fees/target/{num_blocks} -> specific target
```

#### Task 1.3: Implement Historical Endpoint
```rust
// src/api/historical.rs
- GET /historical_fee?timestamp={ts}
```

### 🟡 Priority 2: Configuration Management
**Goal**: Flexible configuration via YAML and environment variables

#### Task 2.1: Configuration Structure
```yaml
# config/default.yaml
server:
  host: "0.0.0.0"
  port: 8080
  
bitcoin_rpc:
  url: "http://localhost:8332"
  username: ""
  password: ""
  
persistence:
  data_directory: "mempool_data"
  cleanup_days: 30
  
collector:
  interval_ms: 30000
```

#### Task 2.2: Environment Variable Support
- AUGUR_SERVER_HOST
- AUGUR_SERVER_PORT
- AUGUR_BITCOIN_RPC_URL
- AUGUR_BITCOIN_RPC_USERNAME
- AUGUR_BITCOIN_RPC_PASSWORD

### 🟢 Priority 3: Server Integration
**Goal**: Wire everything together in a working application

#### Task 3.1: Axum Router Setup
```rust
Router::new()
    .route("/fees", get(get_fees))
    .route("/fees/target/:num_blocks", get(get_fee_for_target))
    .route("/historical_fee", get(get_historical_fee))
    .route("/health", get(health_check))
    .with_state(app_state)
```

#### Task 3.2: Main Application
- Load configuration
- Initialize components
- Spawn collector task
- Start HTTP server
- Graceful shutdown

### 🔵 Priority 4: Testing & Documentation
**Goal**: Ensure reliability and usability

#### Task 4.1: Integration Tests
- Mock Bitcoin RPC responses
- Test all API endpoints
- Verify response formats
- Error handling scenarios

#### Task 4.2: Documentation
- API usage examples
- Deployment guide
- Configuration reference
- Migration from Kotlin

### ⚪ Priority 5: Docker & Deployment
**Goal**: Production-ready containerization

#### Task 5.1: Dockerfile
```dockerfile
FROM rust:1.75 as builder
# Multi-stage build for minimal image
```

#### Task 5.2: Docker Compose
- Bitcoin Core service
- Augur server service
- Volume for persistence

## Development Workflow

### Phase 1: API Implementation (Day 1)
1. Create API models matching Kotlin format
2. Implement all three endpoints
3. Test with curl/httpie

### Phase 2: Configuration (Day 1-2)
1. Create config module
2. Add YAML parsing
3. Environment variable overrides
4. Test configuration loading

### Phase 3: Integration (Day 2)
1. Wire up Axum server
2. Connect all components
3. Test end-to-end flow

### Phase 4: Testing (Day 2-3)
1. Write integration tests
2. Test error scenarios
3. Performance testing

### Phase 5: Documentation (Day 3)
1. Update README
2. API documentation
3. Deployment guide

## Definition of Done

### MVP (v0.1.0)
- [ ] All three API endpoints working
- [ ] Configuration via YAML/env vars
- [ ] Background collection running
- [ ] Persistence working
- [ ] Basic documentation

### Production Ready (v0.2.0)
- [ ] Integration tests passing
- [ ] Docker container built
- [ ] Performance benchmarks met
- [ ] Comprehensive documentation
- [ ] Error handling complete

### Feature Complete (v1.0.0)
- [ ] Full Kotlin API compatibility
- [ ] Production deployment tested
- [ ] Monitoring/metrics added
- [ ] Security audit complete
- [ ] Migration guide written

## Quick Start Commands

```bash
# Development
nix develop -c cargo run -p bitcoin-augur-server

# Testing
nix develop -c cargo test --workspace

# Build release
nix develop -c cargo build --release --target x86_64-unknown-linux-musl

# Run with config
AUGUR_CONFIG_FILE=config/production.yaml ./bitcoin-augur-server

# Docker build
docker build -t bitcoin-augur-server .

# Docker run
docker run -p 8080:8080 -v ./data:/data bitcoin-augur-server
```

## Success Metrics

1. **API Compatibility**: 100% match with Kotlin responses
2. **Performance**: <50ms API response time
3. **Reliability**: 99.9% uptime in production
4. **Resource Usage**: <100MB memory, <5% CPU
5. **Test Coverage**: >80% for server code

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| API format mismatch | Test against Kotlin implementation |
| Performance issues | Profile and optimize hot paths |
| Bitcoin RPC failures | Retry logic and error handling |
| Data corruption | JSON validation and backups |
| Memory leaks | Stress testing and monitoring |

## Support & Resources

- Original Kotlin implementation: [github.com/block/bitcoin-augur](https://github.com/block/bitcoin-augur)
- Rust documentation: [docs.rs/bitcoin-augur](https://docs.rs/bitcoin-augur)
- Issue tracking: GitHub Issues
- Community: Bitcoin Dev mailing list

---

*This roadmap is a living document and will be updated as implementation progresses.*