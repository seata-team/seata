# Seata Server Docker Setup

This document describes how to run Seata Server with Docker, including Redis, MySQL, PostgreSQL, and monitoring tools.

## 🚀 Quick Start

### Prerequisites
- Docker and Docker Compose installed
- At least 4GB RAM available
- Ports 36789, 36790, 6379, 3306, 5432, 8080, 8081, 9090, 3000 available

### Start All Services
```bash
# Clone and navigate to the project
cd seata

# Start all services
./scripts/start.sh

# Or manually with docker-compose
docker-compose up -d
```

### Test the API
```bash
# Run API tests
./scripts/test-api.sh

# Or manually test
curl http://localhost:36789/health
curl http://localhost:36789/metrics
```

## 📊 Service URLs

| Service | URL | Credentials |
|---------|-----|-------------|
| Seata Server HTTP API | http://localhost:36789 | - |
| Seata Server gRPC API | localhost:36790 | - |
| Redis Commander | http://localhost:8081 | - |
| Adminer (DB Admin) | http://localhost:8080 | - |
| Prometheus | http://localhost:9090 | - |
| Grafana | http://localhost:3000 | admin/admin123 |

## 🗄️ Database Connections

| Database | Host | Port | Database | Username | Password |
|----------|------|------|----------|----------|----------|
| MySQL | localhost | 3306 | seata | seata | seata123 |
| PostgreSQL | localhost | 5432 | seata | seata | seata123 |
| Redis | localhost | 6379 | - | - | seata123 |

## 🐳 Docker Compose Files

### Production Setup (`docker-compose.yml`)
- Optimized for production
- Multi-stage Docker build
- Health checks
- Persistent volumes
- Monitoring stack (Prometheus + Grafana)

### Development Setup (`docker-compose.dev.yml`)
- Hot reload for development
- Source code mounting
- Debug logging
- Faster build times

## 🔧 Configuration

### Environment Variables
```bash
# Storage backend
SEATA_STORE=redis  # or mysql, postgres, sled

# Redis configuration
SEATA_REDIS_URL=redis://redis:6379/

# MySQL configuration
SEATA_MYSQL_URL=mysql://seata:seata123@mysql:3306/seata

# PostgreSQL configuration
SEATA_POSTGRES_URL=postgres://seata:seata123@postgres:5432/seata

# Server ports
SEATA_HTTP_PORT=36789
SEATA_GRPC_PORT=36790

# Logging
RUST_LOG=info
```

### Configuration File
The server uses `conf.yaml` for configuration. Key settings:

```yaml
server:
  http_port: 36789
  grpc_port: 36790

store:
  type: redis
  redis:
    url: redis://redis:6379/

exec:
  request_timeout_secs: 30
  retry_interval_secs: 1
  timeout_to_fail_secs: 60
  branch_parallelism: 10
```

## 📈 Monitoring

### Prometheus Metrics
- Transaction rates
- Branch execution success/failure rates
- Latency histograms
- Active transaction counts

### Grafana Dashboards
- Seata Server Dashboard
- Transaction metrics
- Performance monitoring
- Error tracking

### Health Checks
```bash
# Check service health
curl http://localhost:36789/health

# Check metrics
curl http://localhost:36789/metrics

# Check database connections
docker-compose exec redis redis-cli ping
docker-compose exec mysql mysqladmin ping -h localhost -u root -pseata123
docker-compose exec postgres pg_isready -U seata -d seata
```

## 🛠️ Development

### Development Mode
```bash
# Start development environment
docker-compose -f docker-compose.dev.yml up -d

# View logs with hot reload
docker-compose -f docker-compose.dev.yml logs -f seata-server-dev
```

### Building from Source
```bash
# Build the Docker image
docker build -t seata-server .

# Build development image
docker build -f Dockerfile.dev -t seata-server:dev .
```

## 🧪 Testing

### API Testing
```bash
# Run comprehensive API tests
./scripts/test-api.sh

# Test specific endpoints
curl -X POST http://localhost:36789/api/start -H 'content-type: application/json' -d '{}'
curl http://localhost:36789/api/tx/{transaction-id}
```

### Load Testing
```bash
# Test with multiple concurrent transactions
for i in {1..10}; do
  curl -X POST http://localhost:36789/api/start -H 'content-type: application/json' -d '{}' &
done
wait
```

## 🔍 Troubleshooting

### Common Issues

#### Services Not Starting
```bash
# Check service status
docker-compose ps

# View logs
docker-compose logs seata-server
docker-compose logs redis
docker-compose logs mysql
docker-compose logs postgres
```

#### Database Connection Issues
```bash
# Test Redis connection
docker-compose exec redis redis-cli ping

# Test MySQL connection
docker-compose exec mysql mysql -u seata -pseata123 -e "SELECT 1"

# Test PostgreSQL connection
docker-compose exec postgres psql -U seata -d seata -c "SELECT 1"
```

#### Port Conflicts
```bash
# Check port usage
netstat -tulpn | grep -E ':(36789|36790|6379|3306|5432|8080|8081|9090|3000)'

# Stop conflicting services
sudo systemctl stop mysql  # if MySQL is running locally
sudo systemctl stop postgresql  # if PostgreSQL is running locally
```

### Logs and Debugging
```bash
# View all logs
docker-compose logs -f

# View specific service logs
docker-compose logs -f seata-server
docker-compose logs -f redis
docker-compose logs -f mysql
docker-compose logs -f postgres

# Debug mode
RUST_LOG=debug docker-compose up seata-server
```

## 🧹 Cleanup

### Stop Services
```bash
# Stop all services
docker-compose down

# Stop and remove volumes
docker-compose down -v
```

### Remove Everything
```bash
# Remove containers, networks, and volumes
docker-compose down -v --remove-orphans

# Remove images
docker rmi seata-server
docker rmi redis:7-alpine
docker rmi mysql:8.0
docker rmi postgres:15-alpine
```

## 📚 Additional Resources

- [API Documentation](seata-server/API_USAGE.md)
- [Configuration Guide](seata-server/README.md)
- [Development Summary](seata-server/DEVELOPMENT_SUMMARY.md)
- [Docker Best Practices](https://docs.docker.com/develop/dev-best-practices/)

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Test with Docker setup
5. Submit a pull request

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.
