package main

import getstream "github.com/GetStream/getstream-go/v4"

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
