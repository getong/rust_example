module streamchat_go_example

go 1.18

require github.com/ihciah/rust2go v0.0.0

require (
	github.com/GetStream/getstream-go/v4 v4.0.4 // indirect
	github.com/golang-jwt/jwt/v5 v5.2.1 // indirect
)

replace github.com/ihciah/rust2go => ../../../../../test/rust/rust2go
