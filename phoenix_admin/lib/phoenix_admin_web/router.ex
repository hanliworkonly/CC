defmodule PhoenixAdminWeb.Router do
  use PhoenixAdminWeb, :router

  pipeline :browser do
    plug :accepts, ["html"]
    plug :fetch_session
    plug :fetch_flash
    plug :protect_from_forgery
    plug :put_secure_browser_headers
  end

  pipeline :api do
    plug :accepts, ["json"]
  end

  pipeline :auth do
    plug PhoenixAdmin.Auth.AuthPlug
  end

  # Public routes
  scope "/", PhoenixAdminWeb do
    pipe_through :browser

    get "/", AdminController, :index
    get "/login", AdminController, :index
  end

  # API routes - Authentication
  scope "/api", PhoenixAdminWeb do
    pipe_through :api

    post "/register", AuthController, :register
    post "/login", AuthController, :login
  end

  # API routes - Protected
  scope "/api", PhoenixAdminWeb do
    pipe_through [:api, :auth]

    get "/me", AuthController, :me
    get "/users", AdminController, :api_users
    put "/users/:id", AdminController, :api_update_user
    delete "/users/:id", AdminController, :api_delete_user
  end

  # Admin routes - Protected
  scope "/admin", PhoenixAdminWeb do
    pipe_through :browser

    get "/dashboard", AdminController, :dashboard
    get "/users", AdminController, :users
  end
end
