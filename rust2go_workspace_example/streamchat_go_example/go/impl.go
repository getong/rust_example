package main

import (
	getstream "github.com/GetStream/getstream-go/v4"
	"time"
)

type StreamChatService struct{}

func init() {
	StreamChatCallImpl = StreamChatService{}
}

func (StreamChatService) create_token(req *StreamChatTokenRequest) StreamChatTokenResponse {
	serverClient, err := getstream.NewClient(req.api_key, req.api_secret)
	if err != nil {
		return StreamChatTokenResponse{error: err.Error()}
	}

	token, err := serverClient.CreateToken(req.user_id)
	if err != nil {
		return StreamChatTokenResponse{error: err.Error()}
	}

	return StreamChatTokenResponse{token: token}
}

func (StreamChatService) create_token_with_expiration(req *StreamChatExpiringTokenRequest) StreamChatTokenResponse {
	serverClient, err := getstream.NewClient(req.api_key, req.api_secret)
	if err != nil {
		return StreamChatTokenResponse{error: err.Error()}
	}

	token, err := serverClient.CreateToken(
		req.user_id,
		getstream.WithExpiration(time.Duration(req.expiration_seconds)*time.Second),
	)
	if err != nil {
		return StreamChatTokenResponse{error: err.Error()}
	}

	return StreamChatTokenResponse{token: token}
}
