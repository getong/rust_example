# Rust Rocket MongoDB token-auth REST API boilerplate


In this repository, you can find backend Rust rocket mongodb rest-api
boilerplate with token authorization.

```rust
#[get("/public/hello")]
pub async fn hello_world() -> Json<&'static str> {
  Json("Hello world")
}
```

Visiting `localhost:8000/api/v1/public/hello`, for example, will trigger the `hello`
route resulting in the string `Hello world` being sent to the
browser. In this example there is no authorization.

## How to start
1. First you need to download this repository to your computer

* Write this in your terminal:

```sh
git clone https://github.com/martyr00/Rust-rocket-mongoDB-token-auth-REST-API-boilerplate.git
```
* After open the project in your IDE

2. You need to make your private.rs file as in the private.sample.rs:

```rust
pub const JWT_SECRET: &[u8] = b"<YOUR_JWT_SECRET_KEY>";
pub const REFRESH_JWT_SECRET: &'static str = b"<YOUR_REFRESH_JWT_SECRET_KEY>";
pub(crate) const URL_DB: &str = "mongodb+srv://<YOUR_LOGIN>:<YOUR_PASSWORD>@cluster0.d5yn0.mongodb.net/<YOUR_DB_NAME>";
```

 * `JWT_SECRET` and `REFRESH_JWT_SECRET` you need to come up with your own secret word to encrypt the tokens
 * For `URL_DB` you need registration your mongoDB acc here https://cloud.mongodb.com/

3. To run write this in your terminal:

```sh
cargo check && cargo run
```

## Routes

* /api/v1/registration (POST)
* /api/v1/login (POST)
* /api/v1/refresh (POST)
* /api/v1/public/hello (GET)
* /api/v1/hello (GET)
* /api/v1/public/user (DELETE, PATCH)

## Registration acc

### Registration request:
* `login` (must be unique && len login must be from 3 to 200 letter)
* `password` (len password must be from 8 to 200 letter and password is hashed before being saved to the database.)
* `mail` (must be unique and mail)
* `first_name` (len must be from 2 to 150 letter and this field is optional)
* `last_name` (len must be from 2 to 200 letter and this field is optional)

```rust
pub struct RegistrationRequest {
    pub login: String,
    pub password: String,

    pub mail: String,

    pub first_name: String,
    pub last_name: String,
}
```
#### Example registration request:

```json
{
    "login": "test",
    "password": "12345678",
    "mail": "test@gmail.com",
    "first_name": "Test",
    "last_name": ""
}
```

### Registration response

In response, the server will send 2 JWT tokens.
The `token` is valid for 1 hour. `Refresh token` is valid for 7 days.
`token` is needed to verify user authorization.
With each private request, the `token` will be sent fronted to
the headers in the authorization field.

More about jwt authentication https://blog.logrocket.com/jwt-authentication-in-rust/

#### If everything is correct:
```rust
pub struct Token {
    pub token: String,
    pub refresh_token: String,
}
```
#### Example:

```json
{
    "token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJ1c2VyX2lkIjoiNjJiNGRhOTk4ZjgyMzc2YTk1MzM1MWIxIiwiZXhwIjoxNjU2MDIzMjA5fQ.aJFDZVyMBuNYh5EAArYYfzYCTnHHCQ7IHuZpKNCXHs0",
    "refresh_token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJ1c2VyX2lkIjoiNjJiNGRhOTk4ZjgyMzc2YTk1MzM1MWIxIiwiZXhwIjoxNjU4NjExNjA5fQ.2_DjxtQxtsLsprvhBfYU8rKAoDfWMdshoPKDUqq6QZQ"
}
```
#### Possible error:

* `bad login` -> Status 400 and string "Bad login" in json
* `already registered login` -> Status 400 and string "Already registered by login" in json
* `bad password` -> Status 400 and string "Bad password" in json
* `already registered password` -> Status 400 and string "Already registered by password" in json
* `bad mail` -> Status 400 and string "Bad mail" in json
* `already registered mail` -> Status 400 and string "Already registered by mail" in json

#### Example error:

Status 400 Bad Request
```json
{
    "cause": "Already registered by login"
}
```

## Login acc

### Login request:

* `login`(The server checks 2 logins from the database and the request)
* `password`(The server checks the encrypted password in the
  database with the password from the request)

```rust
pub struct LoginRequest {
  pub login: String,
  pub password: String,
}
```

#### Example:
```json
{
    "login": "test",
    "password": "12345678"
}
```

### Login response:

#### If everything is correct:
```rust
pub struct Token {
    pub token: String,
    pub refresh_token: String,
}
```

#### Example:

```json
{
    "token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJ1c2VyX2lkIjoiNjJhM2I3Zjg4MTE1OWVkYWJmNTcwZjYwIiwiZXhwIjoxNjU2MDI0MDM1fQ.5Nu0lbN5X656JhuY8PrK1IJhWFVjHxKbh8CssKqHQqk",
    "refresh_token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJ1c2VyX2lkIjoiNjJhM2I3Zjg4MTE1OWVkYWJmNTcwZjYwIiwiZXhwIjoxNjU4NjEyNDM1fQ.5OUiED1no-uizfYmq1xk6Z6XpX9TsbezDx8QxPSbyV0"
}
```

#### Possible error:
* `Bad request`(In any case)

#### Example:

Status 400
```json
{
    "cause": "Wrong request"
}
```

## Refresh token
In this route, the frontend asks the server to
refresh the `token` with a `refresh token` in json

### Refresh token request
* `refresh token`(specific user)

```rust
pub struct RefreshToken {
    pub(crate) refresh_token: String,
}

```

#### Example refresh token request:
```json
{
    "refresh_token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJ1c2VyX2lkIjoiNjJhM2I3Zjg4MTE1OWVkYWJmNTcwZjYwIiwiZXhwIjoxNjU3NDg5NDcyfQ.BcTanbs5lyT-Yv2ekf5-xl_NzEqpKsh5S59AEuZrmVQ"
}
```

### Refresh token response

#### If everything is correct:
```rust
pub struct Token {
    pub token: String,
    pub refresh_token: String,
}
```

#### Example:

```json
{
    "token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJ1c2VyX2lkIjoiNjJhM2I3Zjg4MTE1OWVkYWJmNTcwZjYwIiwiZXhwIjoxNjU2MDI0MDM1fQ.5Nu0lbN5X656JhuY8PrK1IJhWFVjHxKbh8CssKqHQqk",
    "refresh_token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJ1c2VyX2lkIjoiNjJhM2I3Zjg4MTE1OWVkYWJmNTcwZjYwIiwiZXhwIjoxNjU4NjEyNDM1fQ.5OUiED1no-uizfYmq1xk6Z6XpX9TsbezDx8QxPSbyV0"
}
```

#### Possible error:
* `Unauthorized`

#### Example:

Status 400
```json
{
    "cause": "Unauthorized"
}
```

## Public hello

This is the only route without authorization. Therefore there is no request in this route.

### Rust code:
```rust
#[get("/public/hello")]
pub async fn hello_world() -> Json<&'static str> {
  Json("Hello world")
}
```
### Response in json:
```
{
  "Hello world"
}
```

## Private hello
In this route, the server checks in the headers
token if the token is valid then the server executes the program.

### Private hello request:
#### From headers:

`authorization`   `Bearer` (TOKEN)

#### Example:

`authorization`       `Bearer eyJ0eXAiOiJKV1QiLCJhbGci....`

### Private hello response

the response will be a greeting with the user.
If the database contains his first name and surname,
then the program will greet you by the first name and
surname; if not, the program will greet you by login

#### If everything is correct:

```rust
pub struct HelloNameResponse {
    pub(crate) greetings: String,
}
```
#### Example:
```json
{
    "greetings": "Hello test"
}
```

#### Possible error:
* `Unauthorized`

#### Example:
Status 401

```json
{
"cause": "Unauthorized"
}
```
