# Phoenix 后台管理系统

基于 Phoenix Framework 和 JWT 认证的后台管理系统。

## 功能特性

- ✅ 基于 JWT 的用户认证
- ✅ 用户管理（增删改查）
- ✅ 角色权限控制（管理员/普通用户）
- ✅ RESTful API 接口
- ✅ 响应式后台管理界面
- ✅ 安全的密码加密（Bcrypt）
- ✅ Token 过期验证

## 技术栈

- **后端框架**: Phoenix 1.7
- **数据库**: PostgreSQL + Ecto
- **认证**: JWT (Joken)
- **密码加密**: Bcrypt
- **前端**: HTML + Vanilla JavaScript (无需构建工具)

## 环境要求

- Elixir 1.14 或更高版本
- Erlang/OTP 25 或更高版本
- PostgreSQL 14 或更高版本

## 安装步骤

### 1. 克隆项目

```bash
cd phoenix_admin
```

### 2. 安装依赖

```bash
mix deps.get
```

### 3. 配置数据库

编辑 `config/dev.exs`，修改数据库连接信息：

```elixir
config :phoenix_admin, PhoenixAdmin.Repo,
  username: "postgres",
  password: "postgres",
  hostname: "localhost",
  database: "phoenix_admin_dev"
```

### 4. 创建并初始化数据库

```bash
mix ecto.setup
```

这个命令会：
- 创建数据库
- 运行 migrations
- 运行 seeds (创建默认管理员账号)

### 5. 启动服务器

```bash
mix phx.server
```

或者在 IEx 中启动：

```bash
iex -S mix phx.server
```

访问 http://localhost:4000

## 默认账号

- **邮箱**: admin@example.com
- **密码**: password

## API 文档

### 认证接口

#### 注册用户

```http
POST /api/register
Content-Type: application/json

{
  "user": {
    "email": "user@example.com",
    "password": "password",
    "name": "用户名",
    "role": "user"
  }
}
```

**响应**:
```json
{
  "message": "User registered successfully",
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "user": {
    "id": "uuid",
    "email": "user@example.com",
    "name": "用户名",
    "role": "user"
  }
}
```

#### 用户登录

```http
POST /api/login
Content-Type: application/json

{
  "email": "admin@example.com",
  "password": "password"
}
```

**响应**:
```json
{
  "message": "Login successful",
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "user": {
    "id": "uuid",
    "email": "admin@example.com",
    "name": "管理员",
    "role": "admin"
  }
}
```

#### 获取当前用户信息

```http
GET /api/me
Authorization: Bearer <token>
```

**响应**:
```json
{
  "user": {
    "id": "uuid",
    "email": "admin@example.com",
    "name": "管理员",
    "role": "admin",
    "is_active": true
  }
}
```

### 用户管理接口（需要认证）

#### 获取用户列表

```http
GET /api/users
Authorization: Bearer <token>
```

**响应**:
```json
{
  "users": [
    {
      "id": "uuid",
      "email": "admin@example.com",
      "name": "管理员",
      "role": "admin",
      "is_active": true,
      "inserted_at": "2024-01-01T00:00:00Z"
    }
  ]
}
```

#### 更新用户

```http
PUT /api/users/:id
Authorization: Bearer <token>
Content-Type: application/json

{
  "user": {
    "name": "新名字",
    "role": "admin"
  }
}
```

#### 删除用户

```http
DELETE /api/users/:id
Authorization: Bearer <token>
```

## 页面路由

- `/` - 登录页面
- `/login` - 登录页面
- `/admin/dashboard` - 仪表盘
- `/admin/users` - 用户管理

## 项目结构

```
phoenix_admin/
├── config/              # 配置文件
│   ├── config.exs      # 主配置
│   ├── dev.exs         # 开发环境
│   ├── test.exs        # 测试环境
│   ├── prod.exs        # 生产环境
│   └── runtime.exs     # 运行时配置
├── lib/
│   ├── phoenix_admin/           # 业务逻辑
│   │   ├── accounts/           # 用户账户模块
│   │   │   └── user.ex        # 用户模型
│   │   ├── auth/              # 认证模块
│   │   │   ├── token.ex       # JWT Token 生成和验证
│   │   │   └── auth_plug.ex   # 认证中间件
│   │   ├── accounts.ex        # 账户上下文
│   │   ├── application.ex     # 应用启动
│   │   └── repo.ex           # 数据库仓库
│   └── phoenix_admin_web/      # Web 层
│       ├── controllers/        # 控制器
│       │   ├── auth_controller.ex
│       │   └── admin_controller.ex
│       ├── components/         # 组件和布局
│       │   └── layouts/
│       ├── endpoint.ex        # HTTP 端点
│       └── router.ex          # 路由配置
├── priv/
│   └── repo/
│       ├── migrations/        # 数据库迁移
│       └── seeds.exs         # 种子数据
├── test/                      # 测试文件
└── mix.exs                   # 项目配置
```

## 安全配置

### 生产环境部署前必须修改：

1. **JWT 密钥** (`config/config.exs`):
```elixir
config :phoenix_admin, PhoenixAdmin.Auth.Guardian,
  secret_key: "your_super_secret_jwt_key_change_this_in_production"
```

生成新密钥：
```bash
mix phx.gen.secret
```

2. **Session 签名盐** (`lib/phoenix_admin_web/endpoint.ex`):
```elixir
signing_salt: "your_signing_salt"
```

3. **环境变量** (生产环境):
- `DATABASE_URL` - 数据库连接字符串
- `SECRET_KEY_BASE` - Phoenix 密钥
- `PHX_HOST` - 主机名
- `PORT` - 端口号

## 开发命令

```bash
# 安装依赖
mix deps.get

# 创建数据库
mix ecto.create

# 运行 migrations
mix ecto.migrate

# 运行 seeds
mix run priv/repo/seeds.exs

# 重置数据库
mix ecto.reset

# 启动服务器
mix phx.server

# 启动交互式 shell
iex -S mix phx.server

# 运行测试
mix test

# 代码格式化
mix format
```

## 常见问题

### 1. 数据库连接失败

确保 PostgreSQL 正在运行，并且连接信息正确。

### 2. Token 验证失败

检查：
- Token 是否正确传递在 `Authorization: Bearer <token>` header 中
- Token 是否过期（默认 24 小时）
- JWT 密钥配置是否正确

### 3. 密码验证失败

确保使用正确的密码，密码经过 Bcrypt 加密存储。

## 许可证

MIT License

## 贡献

欢迎提交 Issue 和 Pull Request！
