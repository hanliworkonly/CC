import Config

config :phoenix_admin,
  ecto_repos: [PhoenixAdmin.Repo],
  generators: [timestamp_type: :utc_datetime]

config :phoenix_admin, PhoenixAdminWeb.Endpoint,
  url: [host: "localhost"],
  adapter: Phoenix.Endpoint.Cowboy2Adapter,
  render_errors: [
    formats: [html: PhoenixAdminWeb.ErrorHTML, json: PhoenixAdminWeb.ErrorJSON],
    layout: false
  ],
  pubsub_server: PhoenixAdmin.PubSub,
  live_view: [signing_salt: "your_secret_salt"]

config :phoenix_admin, PhoenixAdmin.Auth.Guardian,
  issuer: "phoenix_admin",
  secret_key: "your_super_secret_jwt_key_change_this_in_production"

config :logger, :console,
  format: "$time $metadata[$level] $message\n",
  metadata: [:request_id]

config :phoenix, :json_library, Jason

import_config "#{config_env()}.exs"
