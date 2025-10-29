defmodule PhoenixAdminWeb.AdminController do
  use PhoenixAdminWeb, :controller

  alias PhoenixAdmin.Accounts

  def index(conn, _params) do
    render(conn, :index)
  end

  def dashboard(conn, _params) do
    render(conn, :dashboard)
  end

  def users(conn, _params) do
    users = Accounts.list_users()
    render(conn, :users, users: users)
  end

  def api_users(conn, _params) do
    users = Accounts.list_users()

    json(conn, %{
      users: Enum.map(users, fn user ->
        %{
          id: user.id,
          email: user.email,
          name: user.name,
          role: user.role,
          is_active: user.is_active,
          inserted_at: user.inserted_at
        }
      end)
    })
  end

  def api_update_user(conn, %{"id" => id, "user" => user_params}) do
    user = Accounts.get_user!(id)

    case Accounts.update_user(user, user_params) do
      {:ok, updated_user} ->
        json(conn, %{
          message: "User updated successfully",
          user: %{
            id: updated_user.id,
            email: updated_user.email,
            name: updated_user.name,
            role: updated_user.role,
            is_active: updated_user.is_active
          }
        })

      {:error, changeset} ->
        conn
        |> put_status(:unprocessable_entity)
        |> json(%{errors: translate_errors(changeset)})
    end
  end

  def api_delete_user(conn, %{"id" => id}) do
    user = Accounts.get_user!(id)

    case Accounts.delete_user(user) do
      {:ok, _user} ->
        json(conn, %{message: "User deleted successfully"})

      {:error, _changeset} ->
        conn
        |> put_status(:unprocessable_entity)
        |> json(%{error: "Failed to delete user"})
    end
  end

  defp translate_errors(changeset) do
    Ecto.Changeset.traverse_errors(changeset, fn {msg, opts} ->
      Regex.replace(~r"%{(\w+)}", msg, fn _, key ->
        opts |> Keyword.get(String.to_existing_atom(key), key) |> to_string()
      end)
    end)
  end
end
