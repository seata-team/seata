# Seata Server Deployment Guide

Complete deployment guide for the Seata Server distributed transaction coordinator in various environments.

## 📋 Table of Contents

- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Docker Deployment](#docker-deployment)
- [Kubernetes Deployment](#kubernetes-deployment)
- [Production Deployment](#production-deployment)
- [Configuration](#configuration)
- [Monitoring](#monitoring)
- [Troubleshooting](#troubleshooting)
- [Security](#security)

## 🔧 Prerequisites

### System Requirements

- **CPU**: 2+ cores recommended
- **Memory**: 4GB+ RAM recommended
- **Storage**: 10GB+ disk space
- **Network**: Stable internet connection

### Software Requirements

- **Docker**: 20.10+ (for containerized deployment)
- **Docker Compose**: 2.0+ (for multi-service deployment)
- **Kubernetes**: 1.20+ (for Kubernetes deployment)
- **Rust**: 1.75+ (for source deployment)

### Database Requirements

Choose one of the following storage backends:

- **Redis**: 6.0+ (high performance, in-memory)
- **MySQL**: 8.0+ (ACID compliance, persistent)
- **PostgreSQL**: 13+ (advanced features, persistent)
- **Sled**: Embedded (development only)

## 🚀 Quick Start

### Docker Compose (Recommended)

```bash
# Clone the repository
git clone https://github.com/your-org/seata-server.git
cd seata-server

# Start all services
docker-compose up -d

# Check service status
docker-compose ps

# View logs
docker-compose logs -f seata-server
```

### Manual Installation

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/your-org/seata-server.git
cd seata-server
cargo build --release

# Run with default configuration
cargo run -p seata-server
```

## 🐳 Docker Deployment

### Single Container

```bash
# Build the image
docker build -t seata-server .

# Run with Redis
docker run -d \
  --name seata-server \
  -p 36789:36789 \
  -p 36790:36790 \
  -e SEATA_STORE=redis \
  -e SEATA_REDIS_URL=redis://host.docker.internal:6379/ \
  seata-server
```

### Multi-Container with Docker Compose

```yaml
# docker-compose.yml
version: '3.8'

services:
  seata-server:
    build: .
    ports:
      - "36789:36789"
      - "36790:36790"
    environment:
      - SEATA_STORE=redis
      - SEATA_REDIS_URL=redis://redis:6379/
    depends_on:
      redis:
        condition: service_healthy
    restart: unless-stopped

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    command: redis-server --appendonly yes
    volumes:
      - redis_data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 3s
      retries: 5

volumes:
  redis_data:
```

### Production Docker Compose

```yaml
# docker-compose.prod.yml
version: '3.8'

services:
  seata-server:
    image: seata-server:latest
    ports:
      - "36789:36789"
      - "36790:36790"
    environment:
      - SEATA_STORE=mysql
      - SEATA_MYSQL_URL=mysql://seata:password@mysql:3306/seata
      - RUST_LOG=info
    depends_on:
      mysql:
        condition: service_healthy
    restart: unless-stopped
    deploy:
      resources:
        limits:
          memory: 2G
          cpus: '1.0'
        reservations:
          memory: 1G
          cpus: '0.5'

  mysql:
    image: mysql:8.0
    environment:
      MYSQL_ROOT_PASSWORD: rootpassword
      MYSQL_DATABASE: seata
      MYSQL_USER: seata
      MYSQL_PASSWORD: password
    volumes:
      - mysql_data:/var/lib/mysql
      - ./sqls/dtmsvr.storage.mysql.sql:/docker-entrypoint-initdb.d/01-init.sql
    healthcheck:
      test: ["CMD", "mysqladmin", "ping", "-h", "localhost"]
      interval: 10s
      timeout: 5s
      retries: 5
    restart: unless-stopped

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./monitoring/prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus_data:/prometheus
    restart: unless-stopped

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin123
    volumes:
      - grafana_data:/var/lib/grafana
    restart: unless-stopped

volumes:
  mysql_data:
  prometheus_data:
  grafana_data:
```

## ☸️ Kubernetes Deployment

### Namespace

```yaml
# namespace.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: seata
```

### ConfigMap

```yaml
# configmap.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: seata-config
  namespace: seata
data:
  conf.yaml: |
    server:
      http_port: 36789
      grpc_port: 36790
    
    store:
      type: redis
      redis:
        url: redis://redis-service:6379/
    
    exec:
      request_timeout_secs: 30
      retry_interval_secs: 1
      timeout_to_fail_secs: 60
      branch_parallelism: 10
```

### Redis Deployment

```yaml
# redis-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: redis
  namespace: seata
spec:
  replicas: 1
  selector:
    matchLabels:
      app: redis
  template:
    metadata:
      labels:
        app: redis
    spec:
      containers:
      - name: redis
        image: redis:7-alpine
        ports:
        - containerPort: 6379
        command: ["redis-server", "--appendonly", "yes"]
        volumeMounts:
        - name: redis-data
          mountPath: /data
        resources:
          requests:
            memory: "256Mi"
            cpu: "100m"
          limits:
            memory: "512Mi"
            cpu: "500m"
      volumes:
      - name: redis-data
        persistentVolumeClaim:
          claimName: redis-pvc
---
apiVersion: v1
kind: Service
metadata:
  name: redis-service
  namespace: seata
spec:
  selector:
    app: redis
  ports:
  - port: 6379
    targetPort: 6379
---
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: redis-pvc
  namespace: seata
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 10Gi
```

### Seata Server Deployment

```yaml
# seata-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: seata-server
  namespace: seata
spec:
  replicas: 3
  selector:
    matchLabels:
      app: seata-server
  template:
    metadata:
      labels:
        app: seata-server
    spec:
      containers:
      - name: seata-server
        image: seata-server:latest
        ports:
        - containerPort: 36789
          name: http
        - containerPort: 36790
          name: grpc
        env:
        - name: SEATA_STORE
          value: "redis"
        - name: SEATA_REDIS_URL
          value: "redis://redis-service:6379/"
        - name: RUST_LOG
          value: "info"
        volumeMounts:
        - name: config
          mountPath: /etc/seata
        resources:
          requests:
            memory: "512Mi"
            cpu: "200m"
          limits:
            memory: "1Gi"
            cpu: "1000m"
        livenessProbe:
          httpGet:
            path: /health
            port: 36789
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health
            port: 36789
          initialDelaySeconds: 5
          periodSeconds: 5
      volumes:
      - name: config
        configMap:
          name: seata-config
---
apiVersion: v1
kind: Service
metadata:
  name: seata-service
  namespace: seata
spec:
  selector:
    app: seata-server
  ports:
  - name: http
    port: 36789
    targetPort: 36789
  - name: grpc
    port: 36790
    targetPort: 36790
  type: LoadBalancer
```

### Ingress

```yaml
# ingress.yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: seata-ingress
  namespace: seata
  annotations:
    nginx.ingress.kubernetes.io/rewrite-target: /
spec:
  rules:
  - host: seata.example.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: seata-service
            port:
              number: 36789
```

### Horizontal Pod Autoscaler

```yaml
# hpa.yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: seata-hpa
  namespace: seata
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: seata-server
  minReplicas: 3
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
```

## 🏭 Production Deployment

### High Availability Setup

```yaml
# ha-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: seata-server
  namespace: seata
spec:
  replicas: 5
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxUnavailable: 1
      maxSurge: 2
  selector:
    matchLabels:
      app: seata-server
  template:
    metadata:
      labels:
        app: seata-server
    spec:
      affinity:
        podAntiAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
          - weight: 100
            podAffinityTerm:
              labelSelector:
                matchExpressions:
                - key: app
                  operator: In
                  values:
                  - seata-server
              topologyKey: kubernetes.io/hostname
      containers:
      - name: seata-server
        image: seata-server:latest
        ports:
        - containerPort: 36789
        - containerPort: 36790
        env:
        - name: SEATA_STORE
          value: "mysql"
        - name: SEATA_MYSQL_URL
          valueFrom:
            secretKeyRef:
              name: seata-secrets
              key: mysql-url
        resources:
          requests:
            memory: "1Gi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "2000m"
        livenessProbe:
          httpGet:
            path: /health
            port: 36789
          initialDelaySeconds: 30
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 3
        readinessProbe:
          httpGet:
            path: /health
            port: 36789
          initialDelaySeconds: 5
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 3
```

### Database Setup

#### MySQL Production Configuration

```yaml
# mysql-production.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: mysql
  namespace: seata
spec:
  replicas: 1
  selector:
    matchLabels:
      app: mysql
  template:
    metadata:
      labels:
        app: mysql
    spec:
      containers:
      - name: mysql
        image: mysql:8.0
        env:
        - name: MYSQL_ROOT_PASSWORD
          valueFrom:
            secretKeyRef:
              name: mysql-secrets
              key: root-password
        - name: MYSQL_DATABASE
          value: "seata"
        - name: MYSQL_USER
          value: "seata"
        - name: MYSQL_PASSWORD
          valueFrom:
            secretKeyRef:
              name: mysql-secrets
              key: user-password
        ports:
        - containerPort: 3306
        volumeMounts:
        - name: mysql-data
          mountPath: /var/lib/mysql
        - name: mysql-config
          mountPath: /etc/mysql/conf.d
        resources:
          requests:
            memory: "2Gi"
            cpu: "1000m"
          limits:
            memory: "4Gi"
            cpu: "2000m"
      volumes:
      - name: mysql-data
        persistentVolumeClaim:
          claimName: mysql-pvc
      - name: mysql-config
        configMap:
          name: mysql-config
```

#### PostgreSQL Production Configuration

```yaml
# postgres-production.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: postgres
  namespace: seata
spec:
  replicas: 1
  selector:
    matchLabels:
      app: postgres
  template:
    metadata:
      labels:
        app: postgres
    spec:
      containers:
      - name: postgres
        image: postgres:15
        env:
        - name: POSTGRES_DB
          value: "seata"
        - name: POSTGRES_USER
          value: "seata"
        - name: POSTGRES_PASSWORD
          valueFrom:
            secretKeyRef:
              name: postgres-secrets
              key: password
        ports:
        - containerPort: 5432
        volumeMounts:
        - name: postgres-data
          mountPath: /var/lib/postgresql/data
        resources:
          requests:
            memory: "2Gi"
            cpu: "1000m"
          limits:
            memory: "4Gi"
            cpu: "2000m"
      volumes:
      - name: postgres-data
        persistentVolumeClaim:
          claimName: postgres-pvc
```

### Load Balancer Configuration

```yaml
# loadbalancer.yaml
apiVersion: v1
kind: Service
metadata:
  name: seata-service
  namespace: seata
  annotations:
    service.beta.kubernetes.io/aws-load-balancer-type: nlb
    service.beta.kubernetes.io/aws-load-balancer-cross-zone-load-balancing-enabled: "true"
spec:
  selector:
    app: seata-server
  ports:
  - name: http
    port: 36789
    targetPort: 36789
    protocol: TCP
  - name: grpc
    port: 36790
    targetPort: 36790
    protocol: TCP
  type: LoadBalancer
```

## ⚙️ Configuration

### Environment Variables

| Variable | Description | Default | Required |
|----------|-------------|---------|----------|
| `SEATA_STORE` | Storage backend type | `sled` | No |
| `SEATA_REDIS_URL` | Redis connection URL | - | If using Redis |
| `SEATA_MYSQL_URL` | MySQL connection URL | - | If using MySQL |
| `SEATA_POSTGRES_URL` | PostgreSQL connection URL | - | If using PostgreSQL |
| `SEATA_HTTP_PORT` | HTTP server port | `36789` | No |
| `SEATA_GRPC_PORT` | gRPC server port | `36790` | No |
| `RUST_LOG` | Log level | `info` | No |
| `SEATA_REQUEST_TIMEOUT_SECS` | Request timeout | `30` | No |
| `SEATA_RETRY_INTERVAL_SECS` | Retry interval | `1` | No |
| `SEATA_TIMEOUT_TO_FAIL_SECS` | Timeout to fail | `60` | No |
| `SEATA_BRANCH_PARALLELISM` | Branch parallelism | `10` | No |

### Configuration File

```yaml
# conf.yaml
server:
  http_port: 36789
  grpc_port: 36790

store:
  type: redis  # or mysql, postgres, sled
  redis:
    url: redis://localhost:6379/
  mysql:
    url: mysql://user:password@localhost:3306/seata
  postgres:
    url: postgres://user:password@localhost:5432/seata

exec:
  request_timeout_secs: 30
  retry_interval_secs: 1
  timeout_to_fail_secs: 60
  branch_parallelism: 10

logging:
  level: info
  format: json
```

### Secrets Management

```yaml
# secrets.yaml
apiVersion: v1
kind: Secret
metadata:
  name: seata-secrets
  namespace: seata
type: Opaque
data:
  mysql-url: bXlzcWw6Ly91c2VyOnBhc3N3b3JkQGxvY2FsaG9zdDozMzA2L3NlYXRh
  redis-url: cmVkaXM6Ly9sb2NhbGhvc3Q6NjM3OS8=
```

## 📊 Monitoring

### Prometheus Configuration

```yaml
# prometheus-config.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: prometheus-config
  namespace: seata
data:
  prometheus.yml: |
    global:
      scrape_interval: 15s
      evaluation_interval: 15s
    
    scrape_configs:
    - job_name: 'seata-server'
      static_configs:
        - targets: ['seata-service:36789']
      metrics_path: '/metrics'
      scrape_interval: 5s
```

### Grafana Dashboard

```yaml
# grafana-dashboard.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: grafana-dashboard
  namespace: seata
data:
  seata-dashboard.json: |
    {
      "dashboard": {
        "title": "Seata Server Dashboard",
        "panels": [
          {
            "title": "Transaction Rate",
            "type": "stat",
            "targets": [
              {
                "expr": "rate(seata_transaction_total[5m])",
                "legendFormat": "Transactions/sec"
              }
            ]
          }
        ]
      }
    }
```

### Alerting Rules

```yaml
# alerting-rules.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: alerting-rules
  namespace: seata
data:
  seata-alerts.yml: |
    groups:
    - name: seata
      rules:
      - alert: SeataServerDown
        expr: up{job="seata-server"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Seata Server is down"
      
      - alert: HighFailureRate
        expr: rate(seata_branch_failure_total[5m]) / rate(seata_branch_success_total[5m] + seata_branch_failure_total[5m]) > 0.1
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "High branch failure rate detected"
```

## 🔧 Troubleshooting

### Common Issues

#### Service Not Starting

```bash
# Check pod status
kubectl get pods -n seata

# Check pod logs
kubectl logs -f deployment/seata-server -n seata

# Check pod events
kubectl describe pod <pod-name> -n seata
```

#### Database Connection Issues

```bash
# Test database connectivity
kubectl exec -it deployment/seata-server -n seata -- curl http://localhost:36789/health

# Check database logs
kubectl logs -f deployment/mysql -n seata
```

#### Performance Issues

```bash
# Check resource usage
kubectl top pods -n seata

# Check metrics
kubectl port-forward service/seata-service 36789:36789
curl http://localhost:36789/metrics
```

### Debug Commands

```bash
# Get all resources
kubectl get all -n seata

# Check service endpoints
kubectl get endpoints -n seata

# Check ingress
kubectl get ingress -n seata

# Check persistent volumes
kubectl get pv,pvc -n seata
```

### Log Analysis

```bash
# View logs with timestamps
kubectl logs -f deployment/seata-server -n seata --timestamps

# Filter logs by level
kubectl logs -f deployment/seata-server -n seata | grep ERROR

# Export logs
kubectl logs deployment/seata-server -n seata > seata-logs.txt
```

## 🔒 Security

### Network Policies

```yaml
# network-policy.yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: seata-network-policy
  namespace: seata
spec:
  podSelector:
    matchLabels:
      app: seata-server
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: default
    ports:
    - protocol: TCP
      port: 36789
    - protocol: TCP
      port: 36790
  egress:
  - to:
    - podSelector:
        matchLabels:
          app: redis
    ports:
    - protocol: TCP
      port: 6379
  - to:
    - podSelector:
        matchLabels:
          app: mysql
    ports:
    - protocol: TCP
      port: 3306
```

### RBAC Configuration

```yaml
# rbac.yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: seata-sa
  namespace: seata
---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: seata-role
  namespace: seata
rules:
- apiGroups: [""]
  resources: ["pods", "services", "endpoints"]
  verbs: ["get", "list", "watch"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: seata-rolebinding
  namespace: seata
subjects:
- kind: ServiceAccount
  name: seata-sa
  namespace: seata
roleRef:
  kind: Role
  name: seata-role
  apiGroup: rbac.authorization.k8s.io
```

### Pod Security

```yaml
# pod-security.yaml
apiVersion: v1
kind: Pod
metadata:
  name: seata-server
  namespace: seata
spec:
  securityContext:
    runAsNonRoot: true
    runAsUser: 1000
    fsGroup: 1000
  containers:
  - name: seata-server
    image: seata-server:latest
    securityContext:
      allowPrivilegeEscalation: false
      readOnlyRootFilesystem: true
      capabilities:
        drop:
        - ALL
    volumeMounts:
    - name: tmp
      mountPath: /tmp
    - name: var-tmp
      mountPath: /var/tmp
  volumes:
  - name: tmp
    emptyDir: {}
  - name: var-tmp
    emptyDir: {}
```

## 📈 Scaling

### Horizontal Scaling

```bash
# Scale deployment
kubectl scale deployment seata-server --replicas=5 -n seata

# Check scaling status
kubectl get hpa -n seata
```

### Vertical Scaling

```yaml
# resource-limits.yaml
resources:
  requests:
    memory: "1Gi"
    cpu: "500m"
  limits:
    memory: "2Gi"
    cpu: "1000m"
```

### Database Scaling

```yaml
# mysql-cluster.yaml
apiVersion: mysql.oracle.com/v2
kind: InnoDBCluster
metadata:
  name: mysql-cluster
  namespace: seata
spec:
  secretName: mysql-secrets
  tlsUseSelfSigned: true
  instances: 3
  router:
    instances: 2
```

## 🔄 Backup and Recovery

### Database Backup

```bash
# MySQL backup
kubectl exec -it deployment/mysql -n seata -- mysqldump -u root -p seata > backup.sql

# PostgreSQL backup
kubectl exec -it deployment/postgres -n seata -- pg_dump -U seata seata > backup.sql
```

### Configuration Backup

```bash
# Backup ConfigMaps
kubectl get configmap -n seata -o yaml > configmaps.yaml

# Backup Secrets
kubectl get secret -n seata -o yaml > secrets.yaml
```

### Disaster Recovery

```bash
# Restore from backup
kubectl apply -f configmaps.yaml
kubectl apply -f secrets.yaml

# Restore database
kubectl exec -it deployment/mysql -n seata -- mysql -u root -p seata < backup.sql
```

---

This deployment guide provides comprehensive instructions for deploying Seata Server in various environments. For more details, see the [Architecture Documentation](ARCHITECTURE.md) and [API Reference](API_REFERENCE.md).




