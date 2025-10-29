# API 接口文档

## 基础信息

- **Base URL**: `http://localhost:4000`
- **认证方式**: JWT Bearer Token
- **Content-Type**: `application/json`

## 认证流程

1. 使用邮箱和密码调用登录接口获取 JWT Token
2. 在后续请求的 Header 中携带 Token：`Authorization: Bearer <token>`
3. Token 有效期为 24 小时，过期后需要重新登录

## API 端点

### 1. 用户注册

**端点**: `POST /api/register`

**请求头**:
```
Content-Type: application/json
```

**请求体**:
```json
{
  "user": {
    "email": "user@example.com",
    "password": "password123",
    "name": "张三",
    "role": "user"
  }
}
```

**字段说明**:
- `email` (必填): 用户邮箱，必须唯一
- `password` (必填): 密码，最少 6 个字符
- `name` (必填): 用户姓名
- `role` (可选): 用户角色，可选值: `user` (默认) 或 `admin`

**成功响应** (201 Created):
```json
{
  "message": "User registered successfully",
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJlbWFpbCI6InVzZXJAZXhhbXBsZS5jb20iLCJleHAiOjE3MDQwNjcyMDAsInJvbGUiOiJ1c2VyIiwidXNlcl9pZCI6IjEyMzQ1Njc4LWFiY2QtZWZnaC1pamtsLTEyMzQ1Njc4OTBhYiJ9.signature",
  "user": {
    "id": "12345678-abcd-efgh-ijkl-1234567890ab",
    "email": "user@example.com",
    "name": "张三",
    "role": "user"
  }
}
```

**错误响应** (422 Unprocessable Entity):
```json
{
  "errors": {
    "email": ["has already been taken"],
    "password": ["should be at least 6 character(s)"]
  }
}
```

---

### 2. 用户登录

**端点**: `POST /api/login`

**请求头**:
```
Content-Type: application/json
```

**请求体**:
```json
{
  "email": "admin@example.com",
  "password": "password"
}
```

**成功响应** (200 OK):
```json
{
  "message": "Login successful",
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "user": {
    "id": "12345678-abcd-efgh-ijkl-1234567890ab",
    "email": "admin@example.com",
    "name": "管理员",
    "role": "admin"
  }
}
```

**错误响应** (401 Unauthorized):
```json
{
  "error": "Invalid email or password"
}
```

**错误响应** (403 Forbidden) - 账号被禁用:
```json
{
  "error": "Account is not active"
}
```

---

### 3. 获取当前用户信息

**端点**: `GET /api/me`

**请求头**:
```
Authorization: Bearer <token>
```

**成功响应** (200 OK):
```json
{
  "user": {
    "id": "12345678-abcd-efgh-ijkl-1234567890ab",
    "email": "admin@example.com",
    "name": "管理员",
    "role": "admin",
    "is_active": true
  }
}
```

**错误响应** (401 Unauthorized):
```json
{
  "error": "Unauthorized"
}
```

---

### 4. 获取用户列表

**端点**: `GET /api/users`

**请求头**:
```
Authorization: Bearer <token>
```

**成功响应** (200 OK):
```json
{
  "users": [
    {
      "id": "12345678-abcd-efgh-ijkl-1234567890ab",
      "email": "admin@example.com",
      "name": "管理员",
      "role": "admin",
      "is_active": true,
      "inserted_at": "2024-01-01T00:00:00Z"
    },
    {
      "id": "87654321-dcba-hgfe-lkji-ba0987654321",
      "email": "user@example.com",
      "name": "普通用户",
      "role": "user",
      "is_active": true,
      "inserted_at": "2024-01-02T00:00:00Z"
    }
  ]
}
```

**错误响应** (401 Unauthorized):
```json
{
  "error": "Unauthorized"
}
```

---

### 5. 更新用户信息

**端点**: `PUT /api/users/:id`

**请求头**:
```
Authorization: Bearer <token>
Content-Type: application/json
```

**URL 参数**:
- `id`: 用户 UUID

**请求体**:
```json
{
  "user": {
    "name": "新名字",
    "role": "admin",
    "password": "newpassword123"
  }
}
```

**字段说明**:
- `name` (可选): 新的用户姓名
- `role` (可选): 新的角色
- `password` (可选): 新密码，如果不提供则不修改密码

**成功响应** (200 OK):
```json
{
  "message": "User updated successfully",
  "user": {
    "id": "12345678-abcd-efgh-ijkl-1234567890ab",
    "email": "user@example.com",
    "name": "新名字",
    "role": "admin",
    "is_active": true
  }
}
```

**错误响应** (422 Unprocessable Entity):
```json
{
  "errors": {
    "name": ["can't be blank"]
  }
}
```

**错误响应** (401 Unauthorized):
```json
{
  "error": "Unauthorized"
}
```

---

### 6. 删除用户

**端点**: `DELETE /api/users/:id`

**请求头**:
```
Authorization: Bearer <token>
```

**URL 参数**:
- `id`: 用户 UUID

**成功响应** (200 OK):
```json
{
  "message": "User deleted successfully"
}
```

**错误响应** (422 Unprocessable Entity):
```json
{
  "error": "Failed to delete user"
}
```

**错误响应** (401 Unauthorized):
```json
{
  "error": "Unauthorized"
}
```

---

## 使用示例

### cURL 示例

#### 1. 登录获取 Token

```bash
curl -X POST http://localhost:4000/api/login \
  -H "Content-Type: application/json" \
  -d '{
    "email": "admin@example.com",
    "password": "password"
  }'
```

#### 2. 使用 Token 获取用户列表

```bash
curl -X GET http://localhost:4000/api/users \
  -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
```

#### 3. 注册新用户

```bash
curl -X POST http://localhost:4000/api/register \
  -H "Content-Type: application/json" \
  -d '{
    "user": {
      "email": "newuser@example.com",
      "password": "password123",
      "name": "新用户",
      "role": "user"
    }
  }'
```

#### 4. 更新用户信息

```bash
curl -X PUT http://localhost:4000/api/users/12345678-abcd-efgh-ijkl-1234567890ab \
  -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..." \
  -H "Content-Type: application/json" \
  -d '{
    "user": {
      "name": "更新后的名字",
      "role": "admin"
    }
  }'
```

#### 5. 删除用户

```bash
curl -X DELETE http://localhost:4000/api/users/12345678-abcd-efgh-ijkl-1234567890ab \
  -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
```

---

## JavaScript/Fetch 示例

### 登录并保存 Token

```javascript
async function login(email, password) {
  const response = await fetch('http://localhost:4000/api/login', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({ email, password })
  });

  const data = await response.json();

  if (response.ok) {
    // 保存 token 到 localStorage
    localStorage.setItem('token', data.token);
    localStorage.setItem('user', JSON.stringify(data.user));
    return data;
  } else {
    throw new Error(data.error);
  }
}
```

### 使用 Token 调用受保护的 API

```javascript
async function getUsers() {
  const token = localStorage.getItem('token');

  const response = await fetch('http://localhost:4000/api/users', {
    headers: {
      'Authorization': `Bearer ${token}`
    }
  });

  if (response.status === 401) {
    // Token 过期或无效，重定向到登录页
    window.location.href = '/login';
    return;
  }

  return await response.json();
}
```

---

## 错误代码说明

| 状态码 | 说明 |
|--------|------|
| 200 | 请求成功 |
| 201 | 创建成功 |
| 401 | 未授权（Token 无效或过期） |
| 403 | 禁止访问（账号被禁用） |
| 422 | 请求参数错误 |
| 500 | 服务器内部错误 |

---

## JWT Token 结构

Token 包含以下声明 (claims):

```json
{
  "user_id": "12345678-abcd-efgh-ijkl-1234567890ab",
  "email": "admin@example.com",
  "role": "admin",
  "exp": 1704067200,
  "typ": "JWT"
}
```

- `user_id`: 用户 UUID
- `email`: 用户邮箱
- `role`: 用户角色
- `exp`: Token 过期时间（Unix 时间戳）
- `typ`: Token 类型

---

## 注意事项

1. **密码安全**: 所有密码都使用 Bcrypt 加密存储，不会以明文形式返回
2. **Token 过期**: JWT Token 默认有效期为 24 小时
3. **HTTPS**: 生产环境中务必使用 HTTPS 协议
4. **CORS**: 如需跨域访问，需要配置 CORS 中间件
5. **速率限制**: 建议在生产环境中添加 API 速率限制
