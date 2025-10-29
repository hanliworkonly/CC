defmodule PhoenixAdmin.Application do
  @moduledoc false

  use Application

  @impl true
  def start(_type, _args) do
    children = [
      PhoenixAdmin.Repo,
      {Phoenix.PubSub, name: PhoenixAdmin.PubSub},
      {Finch, name: PhoenixAdmin.Finch},
      PhoenixAdminWeb.Endpoint
    ]

    opts = [strategy: :one_for_one, name: PhoenixAdmin.Supervisor]
    Supervisor.start_link(children, opts)
  end

  @impl true
  def config_change(changed, _new, removed) do
    PhoenixAdminWeb.Endpoint.config_change(changed, removed)
    :ok
  end
end
