# Script for populating the database. You can run it as:
#
#     mix run priv/repo/seeds.exs

alias PhoenixAdmin.Accounts

# Create admin user
{:ok, _admin} = Accounts.register_user(%{
  email: "admin@example.com",
  password: "password",
  name: "管理员",
  role: "admin"
})

# Create test users
{:ok, _user1} = Accounts.register_user(%{
  email: "user1@example.com",
  password: "password",
  name: "测试用户1",
  role: "user"
})

{:ok, _user2} = Accounts.register_user(%{
  email: "user2@example.com",
  password: "password",
  name: "测试用户2",
  role: "user"
})

IO.puts("Database seeded successfully!")
IO.puts("Admin login: admin@example.com / password")
