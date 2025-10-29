defmodule PhoenixAdminWeb.AuthController do
  use PhoenixAdminWeb, :controller

  alias PhoenixAdmin.Accounts
  alias PhoenixAdmin.Auth.Token

  def register(conn, %{"user" => user_params}) do
    case Accounts.register_user(user_params) do
      {:ok, user} ->
        {:ok, token, _claims} = Token.generate_token(user)

        conn
        |> put_status(:created)
        |> json(%{
          message: "User registered successfully",
          token: token,
          user: %{
            id: user.id,
            email: user.email,
            name: user.name,
            role: user.role
          }
        })

      {:error, changeset} ->
        conn
        |> put_status(:unprocessable_entity)
        |> json(%{errors: translate_errors(changeset)})
    end
  end

  def login(conn, %{"email" => email, "password" => password}) do
    case Accounts.get_user_by_email_and_password(email, password) do
      nil ->
        conn
        |> put_status(:unauthorized)
        |> json(%{error: "Invalid email or password"})

      user ->
        if user.is_active do
          {:ok, token, _claims} = Token.generate_token(user)

          conn
          |> json(%{
            message: "Login successful",
            token: token,
            user: %{
              id: user.id,
              email: user.email,
              name: user.name,
              role: user.role
            }
          })
        else
          conn
          |> put_status(:forbidden)
          |> json(%{error: "Account is not active"})
        end
    end
  end

  def me(conn, _params) do
    user_id = conn.assigns.current_user_id
    user = Accounts.get_user!(user_id)

    conn
    |> json(%{
      user: %{
        id: user.id,
        email: user.email,
        name: user.name,
        role: user.role,
        is_active: user.is_active
      }
    })
  end

  defp translate_errors(changeset) do
    Ecto.Changeset.traverse_errors(changeset, fn {msg, opts} ->
      Regex.replace(~r"%{(\w+)}", msg, fn _, key ->
        opts |> Keyword.get(String.to_existing_atom(key), key) |> to_string()
      end)
    end)
  end
end
