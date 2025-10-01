#!/bin/bash

# Seata Server Docker Startup Script

set -e

echo "🚀 Starting Seata Server with Docker Compose..."

# Check if Docker is running
if ! docker info > /dev/null 2>&1; then
    echo "❌ Docker is not running. Please start Docker first."
    exit 1
fi

# Check if docker-compose is available
if ! command -v docker-compose &> /dev/null; then
    echo "❌ docker-compose is not installed. Please install docker-compose first."
    exit 1
fi

# Create necessary directories
mkdir -p monitoring/grafana/dashboards
mkdir -p monitoring/grafana/datasources

# Start services
echo "📦 Starting services..."
docker-compose up -d

# Wait for services to be healthy
echo "⏳ Waiting for services to be healthy..."
sleep 30

# Check service health
echo "🔍 Checking service health..."

# Check Redis
if docker-compose exec redis redis-cli ping > /dev/null 2>&1; then
    echo "✅ Redis is healthy"
else
    echo "❌ Redis is not healthy"
fi

# Check MySQL
if docker-compose exec mysql mysqladmin ping -h localhost -u root -pseata123 > /dev/null 2>&1; then
    echo "✅ MySQL is healthy"
else
    echo "❌ MySQL is not healthy"
fi

# Check PostgreSQL
if docker-compose exec postgres pg_isready -U seata -d seata > /dev/null 2>&1; then
    echo "✅ PostgreSQL is healthy"
else
    echo "❌ PostgreSQL is not healthy"
fi

# Check Seata Server
if curl -f http://localhost:36789/health > /dev/null 2>&1; then
    echo "✅ Seata Server is healthy"
else
    echo "❌ Seata Server is not healthy"
fi

echo ""
echo "🎉 Seata Server is running!"
echo ""
echo "📊 Service URLs:"
echo "  Seata Server HTTP API: http://localhost:36789"
echo "  Seata Server gRPC API: localhost:36790"
echo "  Redis Commander:       http://localhost:8081"
echo "  Adminer (DB Admin):    http://localhost:8080"
echo "  Prometheus:            http://localhost:9090"
echo "  Grafana:               http://localhost:3000 (admin/admin123)"
echo ""
echo "🔧 Database Connections:"
echo "  MySQL:    localhost:3306 (seata/seata123)"
echo "  PostgreSQL: localhost:5432 (seata/seata123)"
echo "  Redis:    localhost:6379 (password: seata123)"
echo ""
echo "📝 Useful Commands:"
echo "  View logs:           docker-compose logs -f seata-server"
echo "  Stop services:       docker-compose down"
echo "  Restart services:    docker-compose restart"
echo "  View all services:   docker-compose ps"
