import Config

config :phoenix_admin, PhoenixAdmin.Repo,
  username: "postgres",
  password: "postgres",
  hostname: "localhost",
  database: "phoenix_admin_test#{System.get_env("MIX_TEST_PARTITION")}",
  pool: Ecto.Adapters.SQL.Sandbox,
  pool_size: 10

config :phoenix_admin, PhoenixAdminWeb.Endpoint,
  http: [ip: {127, 0, 0, 1}, port: 4002],
  secret_key_base: "test_secret_key_base",
  server: false

config :logger, level: :warning

config :phoenix, :plug_init_mode, :runtime
