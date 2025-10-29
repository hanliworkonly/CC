defmodule PhoenixAdmin.Auth.Token do
  use Joken.Config

  @impl true
  def token_config do
    default_claims(default_exp: 60 * 60 * 24)  # 24 hours
    |> add_claim("typ", fn -> "JWT" end, &(&1 == "JWT"))
  end

  def generate_token(user) do
    extra_claims = %{
      "user_id" => user.id,
      "email" => user.email,
      "role" => user.role
    }

    generate_and_sign(extra_claims, signer())
  end

  def verify_token(token) do
    verify_and_validate(token, signer())
  end

  defp signer do
    secret = Application.get_env(:phoenix_admin, PhoenixAdmin.Auth.Guardian)[:secret_key]
    Joken.Signer.create("HS256", secret)
  end
end
