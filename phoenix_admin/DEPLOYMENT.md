# 部署指南

本文档介绍如何将 Phoenix 后台管理系统部署到生产环境。

## 准备工作

### 1. 生成安全密钥

```bash
# 生成 SECRET_KEY_BASE
mix phx.gen.secret

# 生成 JWT 密钥
mix phx.gen.secret
```

### 2. 配置环境变量

创建 `.env` 文件或在服务器上设置环境变量：

```bash
# 数据库连接
export DATABASE_URL="ecto://username:password@hostname/database_name"

# Phoenix 配置
export SECRET_KEY_BASE="生成的密钥"
export PHX_HOST="yourdomain.com"
export PORT="4000"

# JWT 配置（更新 config/config.exs）
export JWT_SECRET_KEY="生成的 JWT 密钥"

# 数据库连接池大小
export POOL_SIZE="10"
```

## 部署方式

### 方式 1: 使用 Releases (推荐)

#### 1. 编译 Release

```bash
# 设置为生产环境
export MIX_ENV=prod

# 获取依赖
mix deps.get --only prod

# 编译项目
mix compile

# 编译 assets (如果有)
# mix assets.deploy

# 创建 release
mix release
```

#### 2. 运行 Release

```bash
# 创建并迁移数据库
_build/prod/rel/phoenix_admin/bin/phoenix_admin eval "PhoenixAdmin.Release.migrate"

# 启动应用
_build/prod/rel/phoenix_admin/bin/phoenix_admin start
```

#### 3. 创建 Release 迁移模块

创建 `lib/phoenix_admin/release.ex`:

```elixir
defmodule PhoenixAdmin.Release do
  @moduledoc """
  Used for executing DB release tasks when run in production without Mix
  installed.
  """
  @app :phoenix_admin

  def migrate do
    load_app()

    for repo <- repos() do
      {:ok, _, _} = Ecto.Migrator.with_repo(repo, &Ecto.Migrator.run(&1, :up, all: true))
    end
  end

  def rollback(repo, version) do
    load_app()
    {:ok, _, _} = Ecto.Migrator.with_repo(repo, &Ecto.Migrator.run(&1, :down, to: version))
  end

  defp repos do
    Application.fetch_env!(@app, :ecto_repos)
  end

  defp load_app do
    Application.load(@app)
  end
end
```

### 方式 2: 使用 Docker

#### 1. 创建 Dockerfile

创建 `Dockerfile`:

```dockerfile
FROM elixir:1.14-alpine AS build

# 安装构建依赖
RUN apk add --no-cache build-base npm git

# 准备构建目录
WORKDIR /app

# 安装 hex + rebar
RUN mix local.hex --force && \
    mix local.rebar --force

# 设置环境
ENV MIX_ENV=prod

# 安装 mix 依赖
COPY mix.exs mix.lock ./
RUN mix deps.get --only prod
RUN mix deps.compile

# 复制编译配置文件
COPY config/config.exs config/prod.exs config/
RUN mix compile

# 复制应用代码
COPY lib lib
COPY priv priv

# 创建 release
RUN mix release

# 开始新的阶段以获得更小的镜像
FROM alpine:3.18 AS app

RUN apk add --no-cache openssl ncurses-libs libstdc++

WORKDIR /app

# 从构建阶段复制 release
COPY --from=build /app/_build/prod/rel/phoenix_admin ./

# 创建普通用户
RUN addgroup -g 1000 phoenix && \
    adduser -D -u 1000 -G phoenix phoenix && \
    chown -R phoenix:phoenix /app

USER phoenix

ENV HOME=/app

EXPOSE 4000

CMD ["bin/phoenix_admin", "start"]
```

#### 2. 创建 docker-compose.yml

```yaml
version: '3.8'

services:
  db:
    image: postgres:14-alpine
    environment:
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: postgres
      POSTGRES_DB: phoenix_admin_prod
    volumes:
      - postgres_data:/var/lib/postgresql/data
    ports:
      - "5432:5432"

  web:
    build: .
    environment:
      DATABASE_URL: "ecto://postgres:postgres@db/phoenix_admin_prod"
      SECRET_KEY_BASE: "${SECRET_KEY_BASE}"
      PHX_HOST: "${PHX_HOST:-localhost}"
      PORT: "4000"
    ports:
      - "4000:4000"
    depends_on:
      - db
    command: >
      sh -c "
        bin/phoenix_admin eval 'PhoenixAdmin.Release.migrate' &&
        bin/phoenix_admin start
      "

volumes:
  postgres_data:
```

#### 3. 构建并运行

```bash
# 构建镜像
docker-compose build

# 启动服务
docker-compose up -d

# 查看日志
docker-compose logs -f web
```

### 方式 3: 传统部署 (Systemd)

#### 1. 创建 systemd 服务文件

创建 `/etc/systemd/system/phoenix_admin.service`:

```ini
[Unit]
Description=Phoenix Admin Application
After=network.target

[Service]
Type=simple
User=deploy
WorkingDirectory=/home/deploy/phoenix_admin
Environment="MIX_ENV=prod"
Environment="PORT=4000"
Environment="DATABASE_URL=ecto://user:pass@localhost/phoenix_admin_prod"
Environment="SECRET_KEY_BASE=your_secret_key"
ExecStart=/home/deploy/phoenix_admin/_build/prod/rel/phoenix_admin/bin/phoenix_admin start
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=phoenix_admin

[Install]
WantedBy=multi-user.target
```

#### 2. 启用并启动服务

```bash
# 重新加载 systemd
sudo systemctl daemon-reload

# 启用服务
sudo systemctl enable phoenix_admin

# 启动服务
sudo systemctl start phoenix_admin

# 查看状态
sudo systemctl status phoenix_admin

# 查看日志
sudo journalctl -u phoenix_admin -f
```

## Nginx 反向代理配置

创建 `/etc/nginx/sites-available/phoenix_admin`:

```nginx
upstream phoenix_admin {
    server 127.0.0.1:4000;
}

server {
    listen 80;
    server_name yourdomain.com;

    location / {
        return 301 https://$server_name$request_uri;
    }
}

server {
    listen 443 ssl http2;
    server_name yourdomain.com;

    ssl_certificate /etc/letsencrypt/live/yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/yourdomain.com/privkey.pem;

    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;

    location / {
        proxy_pass http://phoenix_admin;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

启用站点：

```bash
sudo ln -s /etc/nginx/sites-available/phoenix_admin /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx
```

## SSL 证书 (Let's Encrypt)

```bash
# 安装 certbot
sudo apt-get install certbot python3-certbot-nginx

# 获取证书
sudo certbot --nginx -d yourdomain.com

# 自动续期
sudo certbot renew --dry-run
```

## 数据库备份

### 自动备份脚本

创建 `/home/deploy/backup_db.sh`:

```bash
#!/bin/bash

BACKUP_DIR="/home/deploy/backups"
DATE=$(date +%Y%m%d_%H%M%S)
FILENAME="phoenix_admin_$DATE.sql"

mkdir -p $BACKUP_DIR

pg_dump -U postgres phoenix_admin_prod > $BACKUP_DIR/$FILENAME

# 压缩备份
gzip $BACKUP_DIR/$FILENAME

# 删除 30 天前的备份
find $BACKUP_DIR -name "*.gz" -mtime +30 -delete

echo "Backup completed: $FILENAME.gz"
```

添加到 crontab：

```bash
# 每天凌晨 2 点备份
0 2 * * * /home/deploy/backup_db.sh
```

## 监控和日志

### 1. 应用日志

```bash
# Systemd 日志
sudo journalctl -u phoenix_admin -f

# Release 日志
tail -f /home/deploy/phoenix_admin/_build/prod/rel/phoenix_admin/logs/*.log
```

### 2. 健康检查

添加健康检查端点到路由：

```elixir
scope "/", PhoenixAdminWeb do
  pipe_through :api
  get "/health", HealthController, :check
end
```

创建 HealthController:

```elixir
defmodule PhoenixAdminWeb.HealthController do
  use PhoenixAdminWeb, :controller

  def check(conn, _params) do
    # 检查数据库连接
    case Ecto.Adapters.SQL.query(PhoenixAdmin.Repo, "SELECT 1", []) do
      {:ok, _} ->
        json(conn, %{status: "ok", timestamp: DateTime.utc_now()})

      {:error, _} ->
        conn
        |> put_status(503)
        |> json(%{status: "error", message: "Database unavailable"})
    end
  end
end
```

## 性能优化

### 1. 数据库连接池

在 `config/prod.exs` 中调整：

```elixir
config :phoenix_admin, PhoenixAdmin.Repo,
  pool_size: String.to_integer(System.get_env("POOL_SIZE") || "10")
```

### 2. 启用 GZIP 压缩

在 endpoint.ex 中添加：

```elixir
plug Plug.Static,
  at: "/",
  from: :phoenix_admin,
  gzip: true,
  only: ~w(css fonts images js favicon.ico robots.txt)
```

## 故障排查

### 常见问题

1. **端口被占用**
   ```bash
   sudo lsof -i :4000
   sudo kill -9 <PID>
   ```

2. **数据库连接失败**
   ```bash
   # 检查 PostgreSQL 是否运行
   sudo systemctl status postgresql

   # 测试连接
   psql -U postgres -h localhost
   ```

3. **内存不足**
   ```bash
   # 查看内存使用
   free -h

   # 增加交换空间
   sudo fallocate -l 2G /swapfile
   sudo chmod 600 /swapfile
   sudo mkswap /swapfile
   sudo swapon /swapfile
   ```

## 安全检查清单

- [ ] 更改所有默认密码和密钥
- [ ] 启用 HTTPS
- [ ] 配置防火墙
- [ ] 设置 CORS 策略
- [ ] 启用速率限制
- [ ] 配置数据库备份
- [ ] 设置监控和告警
- [ ] 限制数据库访问权限
- [ ] 定期更新依赖包
- [ ] 审查日志文件权限

## 更新部署

```bash
# 拉取最新代码
git pull origin main

# 更新依赖
MIX_ENV=prod mix deps.get --only prod

# 运行迁移
MIX_ENV=prod mix ecto.migrate

# 重新编译和创建 release
MIX_ENV=prod mix release --overwrite

# 重启服务
sudo systemctl restart phoenix_admin
```

## 回滚

```bash
# 数据库回滚
MIX_ENV=prod mix ecto.rollback

# 回滚代码
git checkout <previous_commit>

# 重新部署
# ... (重复更新部署步骤)
```
