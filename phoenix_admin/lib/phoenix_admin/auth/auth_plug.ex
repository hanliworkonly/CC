defmodule PhoenixAdmin.Auth.AuthPlug do
  import Plug.Conn
  import Phoenix.Controller

  def init(opts), do: opts

  def call(conn, _opts) do
    case get_req_header(conn, "authorization") do
      ["Bearer " <> token] ->
        verify_token(conn, token)
      _ ->
        unauthorized(conn)
    end
  end

  defp verify_token(conn, token) do
    case PhoenixAdmin.Auth.Token.verify_token(token) do
      {:ok, claims} ->
        conn
        |> assign(:current_user_id, claims["user_id"])
        |> assign(:current_user_email, claims["email"])
        |> assign(:current_user_role, claims["role"])
      {:error, _reason} ->
        unauthorized(conn)
    end
  end

  defp unauthorized(conn) do
    conn
    |> put_status(:unauthorized)
    |> json(%{error: "Unauthorized"})
    |> halt()
  end
end
