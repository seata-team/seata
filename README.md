# Seata Server - Distributed Transaction Coordinator

[![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Docker](https://img.shields.io/badge/docker-ready-blue.svg)](docker-compose.yml)

A high-performance distributed transaction coordinator implemented in Rust, providing Saga and TCC transaction patterns with comprehensive monitoring and production-ready features.

## 🚀 Quick Start

### Prerequisites
- Rust 1.75+ 
- Docker & Docker Compose
- Redis, MySQL, or PostgreSQL (for production)

### Installation

```bash
# Clone the repository
git clone https://github.com/your-org/seata-server.git
cd seata-server

# Build the project
cargo build --release

# Run with Docker (recommended)
docker-compose up -d

# Or run locally
cargo run -p seata-server
```

### Test the API

```bash
# Health check
curl http://localhost:36789/health

# Create a transaction
curl -X POST http://localhost:36789/api/start \
  -H 'content-type: application/json' \
  -d '{}'

# Run comprehensive tests
./scripts/test-api.sh
```

## 📋 Table of Contents

- [Features](#features)
- [Architecture](#architecture)
- [API Documentation](#api-documentation)
- [Configuration](#configuration)
- [Deployment](#deployment)
- [Monitoring](#monitoring)
- [Development](#development)
- [Contributing](#contributing)

## ✨ Features

### Core Transaction Management
- ✅ **Saga Pattern**: Complete implementation with branch execution and rollback
- ✅ **TCC Pattern**: Try-Confirm-Cancel with barrier idempotency
- ✅ **Concurrent Execution**: Configurable branch parallelism
- ✅ **Retry Logic**: Timeout and retry mechanisms
- ✅ **Status Management**: Global and branch status tracking

### Storage Backends
- ✅ **Redis**: High-performance in-memory storage
- ✅ **MySQL**: Persistent storage with SeaORM
- ✅ **PostgreSQL**: ACID-compliant storage
- ✅ **Sled**: Embedded database for development

### API Interfaces
- ✅ **HTTP REST API**: Complete REST endpoints
- ✅ **gRPC API**: High-performance gRPC service
- ✅ **Query Operations**: Pagination and filtering
- ✅ **Health Checks**: Service monitoring

### Monitoring & Observability
- ✅ **Prometheus Metrics**: Success/failure counters, latency histograms
- ✅ **Structured Logging**: Comprehensive logging with tracing
- ✅ **Health Endpoints**: Service status monitoring

## 🏗️ Architecture

### System Components

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   HTTP Client   │    │   gRPC Client   │    │   Admin UI      │
└─────────┬───────┘    └─────────┬───────┘    └─────────┬───────┘
          │                      │                      │
          └──────────────────────┼──────────────────────┘
                                 │
                    ┌─────────────▼─────────────┐
                    │     Seata Server          │
                    │  ┌─────────────────────┐  │
                    │  │   HTTP API (36789)  │  │
                    │  └─────────────────────┘  │
                    │  ┌─────────────────────┐  │
                    │  │   gRPC API (36790)  │  │
                    │  └─────────────────────┘  │
                    └─────────────┬───────────────┘
                                │
                    ┌─────────────▼─────────────┐
                    │    Transaction Manager    │
                    │  ┌─────────────────────┐  │
                    │  │   Saga Manager      │  │
                    │  └─────────────────────┘  │
                    │  ┌─────────────────────┐  │
                    │  │   TCC Manager       │  │
                    │  └─────────────────────┘  │
                    │  ┌─────────────────────┐  │
                    │  │   Barrier Manager   │  │
                    │  └─────────────────────┘  │
                    └─────────────┬───────────────┘
                                │
                    ┌─────────────▼─────────────┐
                    │     Storage Layer        │
                    │  ┌─────────────────────┐  │
                    │  │   Redis / MySQL     │  │
                    │  │   PostgreSQL / Sled │  │
                    │  └─────────────────────┘  │
                    └───────────────────────────┘
```

### Transaction Flow

#### Saga Pattern
```
Start → Add Branches → Submit → Execute Branches → Commit/Abort
  ↓         ↓           ↓           ↓              ↓
Create   Prepare    Validate    HTTP Calls    Update Status
```

#### TCC Pattern
```
Start → Try Phase → Confirm/Cancel → Update Status
  ↓        ↓           ↓              ↓
Create   Reserve    Finalize      Complete
```

## 📚 API Documentation

### Base URL
```
http://localhost:36789
```

### Authentication
No authentication required for basic operations.

### Content Types
- **Request**: `application/json`
- **Response**: `application/json`

### Error Handling
All endpoints return appropriate HTTP status codes:
- `200 OK`: Success
- `400 Bad Request`: Invalid request format
- `404 Not Found`: Transaction not found
- `500 Internal Server Error`: Server error

## 🔧 Configuration

### Environment Variables

```bash
# Storage backend
export SEATA_STORE=redis  # or mysql, postgres, sled

# Redis configuration
export SEATA_REDIS_URL=redis://127.0.0.1/
# With password
# export SEATA_REDIS_URL=redis://:password@127.0.0.1:6379/0

# MySQL configuration (DSN)
export SEATA_MYSQL_DSN=mysql://user:pass@localhost:3306/seata

# PostgreSQL configuration (DSN)
export SEATA_POSTGRES_DSN=postgres://user:pass@localhost:5432/seata

# Server ports
export SEATA_HTTP_PORT=36789
export SEATA_GRPC_PORT=36790

# Execution settings
export SEATA_REQUEST_TIMEOUT_SECS=30
export SEATA_RETRY_INTERVAL_SECS=1
export SEATA_TIMEOUT_TO_FAIL_SECS=60
export SEATA_BRANCH_PARALLELISM=10

# Connection pool (for MySQL/Postgres via SeaORM)
export SEATA_MAX_OPEN_CONNS=200
export SEATA_MAX_IDLE_CONNS=20
export SEATA_CONN_MAX_LIFE_TIME_MINUTES=30
```

### Configuration File

Create `conf.yaml` in the server directory:

```yaml
server:
  http_port: 36789
  grpc_port: 36790

store:
  type: redis
  redis:
    # If password is required, prefer configuring via URL. Empty password means no AUTH.
    # Examples:
    #   redis://127.0.0.1:6379/0
    #   redis://:password@127.0.0.1:6379/0
    url: redis://127.0.0.1/

exec:
  request_timeout_secs: 30
  retry_interval_secs: 1
  timeout_to_fail_secs: 60
  branch_parallelism: 10
```

## 🚀 Deployment

### Docker Deployment (Recommended)

```bash
# Start all services
docker-compose up -d

# High concurrency override (compose v2+)
docker compose -f docker-compose.yml -f docker-compose.override.yml up -d

# Check service status
docker-compose ps

# View logs
docker-compose logs -f seata-server
```

### Manual Deployment

```bash
# Build the application
cargo build --release

# Run with configuration
SEATA_STORE=redis SEATA_REDIS_URL=redis://127.0.0.1/ \
  cargo run -p seata-server
```

### Production Deployment

1. **Use production storage backend** (Redis preferred for high throughput；MySQL/PostgreSQL for audit/ACID)
2. **Configure monitoring** (Prometheus/Grafana)
3. **Set up load balancing** for high availability
4. **Configure backup strategies** for data persistence

### Kubernetes Deployment (Examples)

```bash
# Redis Cluster for high QPS
kubectl apply -f seata/k8s/redis-cluster.yaml

# Seata 50k QPS tuned deployment + service + HPA
kubectl apply -f seata/k8s/seata-server-50k.yaml

# Ingress with TLS (requires cert-manager installed)
kubectl apply -f seata/k8s/cert-issuer.yaml
kubectl apply -f seata/k8s/ingress.yaml

# Optional: Gateway API variant
# kubectl apply -f seata/k8s/gateway.yaml

# k6 load test (50k rps target)
kubectl apply -f seata/k8s/k6-loadtest.yaml
```

> Replace `seata.example.com` and `admin@example.com` with your domain/email.

## 📊 Monitoring

### Metrics Endpoints

```bash
# Health check
curl http://localhost:36789/health

# Prometheus metrics
curl http://localhost:36789/metrics
```

### Key Metrics

- `seata_branch_success_total`: Successful branch executions
- `seata_branch_failure_total`: Failed branch executions
- `seata_branch_latency_seconds`: Branch execution latency
- `seata_active_transactions`: Currently active transactions

### Grafana Dashboard

Access Grafana at `http://localhost:3000` (admin/admin123) for:
- Transaction rates and success rates
- Branch execution performance
- Error rates and latency
- System health monitoring

## 🛠️ Development

### Prerequisites
- Rust 1.75+
- Docker & Docker Compose
- Git

### Setup Development Environment

```bash
# Clone repository
git clone https://github.com/your-org/seata-server.git
cd seata-server

# Start development environment
docker-compose -f docker-compose.dev.yml up -d

# Run tests
cargo test

# Run with hot reload
cargo watch -x "run -p seata-server"
```

### Project Structure

```
seata/
├── seata-proto/          # Protocol definitions
│   ├── proto/            # gRPC proto files
│   └── src/              # Generated Rust code
├── seata-server/         # Main server implementation
│   ├── src/
│   │   ├── domain.rs     # Domain models
│   │   ├── repo.rs       # Repository traits
│   │   ├── saga.rs       # Saga implementation
│   │   ├── barrier.rs    # Barrier implementation
│   │   ├── storage/      # Storage backends
│   │   └── main.rs       # Application entry point
│   └── Cargo.toml
├── docker-compose.yml    # Production Docker setup
├── docker-compose.dev.yml # Development Docker setup
└── README.md
```

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test categories
cargo test business_scenario_tests
cargo test integration_tests
cargo test performance_tests

# Run with coverage
cargo test --lib -- --nocapture
```

### Code Quality

```bash
# Format code
cargo fmt

# Lint code
cargo clippy

# Check for security issues
cargo audit
```

## 📈 Performance

### Benchmarks

- **Transaction Creation**: < 1ms per transaction
- **Branch Execution**: Concurrent processing with configurable limits
- **Memory Usage**: Efficient memory management for large datasets
- **Throughput**: 1000+ transactions per second
- **Latency**: Sub-second response times for most operations

### Optimization Tips

1. **Use Redis (Cluster) for high throughput**
2. **Tune branch_parallelism (e.g., 32–192)**
3. **Short timeouts + jittered retries**
4. **Enable HTTP compression and shared clients**
5. **Use SeaORM pool with proper max/min connections**
6. **Monitor P95/P99 latency and failure rate**

## 🔒 Security

### Best Practices

1. **Network Security**: Use TLS for production deployments
2. **Access Control**: Implement authentication for production
3. **Data Encryption**: Encrypt sensitive data in transit and at rest
4. **Audit Logging**: Enable comprehensive audit trails
5. **Regular Updates**: Keep dependencies updated

### Security Considerations

- No authentication by default (add for production)
- Input validation on all endpoints
- Rate limiting for API endpoints
- Secure storage backend configuration

## 🤝 Contributing

### Development Workflow

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Ensure all tests pass
6. Submit a pull request

### Code Standards

- Follow Rust naming conventions
- Add comprehensive tests
- Document public APIs
- Use meaningful commit messages
- Update documentation for new features

### Testing Requirements

- Unit tests for all new functionality
- Integration tests for API endpoints
- Performance tests for critical paths
- Documentation updates for API changes

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [DTM](https://github.com/dtm-labs/dtm) - Inspiration for distributed transaction patterns
- [Seata](https://github.com/seata/seata) - Reference implementation
- [Rust](https://www.rust-lang.org/) - The amazing systems programming language
- [Tokio](https://tokio.rs/) - Async runtime for Rust
- [Axum](https://github.com/tokio-rs/axum) - Web framework
- [Tonic](https://github.com/hyperium/tonic) - gRPC framework

## 📞 Support

- **Documentation**: [API Usage Guide](seata-server/API_USAGE.md)
- **Docker Guide**: [Docker Setup](README-Docker.md)
- **Issues**: [GitHub Issues](https://github.com/your-org/seata-server/issues)
- **Discussions**: [GitHub Discussions](https://github.com/your-org/seata-server/discussions)

---

**Built with ❤️ in Rust**

