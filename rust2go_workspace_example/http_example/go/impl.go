package main

import (
	"bufio"
	"net/http"
)

type HTTPService struct{}

func init() {
	HttpFetchCallImpl = HTTPService{}
}

func (HTTPService) fetch(req *HttpFetchRequest) HttpFetchResponse {
	resp, err := http.Get(req.url)
	if err != nil {
		return HttpFetchResponse{error: err.Error()}
	}
	defer resp.Body.Close()

	lines := make([]string, 0, req.max_lines)
	scanner := bufio.NewScanner(resp.Body)
	for i := uint32(0); scanner.Scan() && i < req.max_lines; i++ {
		lines = append(lines, scanner.Text())
	}

	if err := scanner.Err(); err != nil {
		return HttpFetchResponse{error: err.Error()}
	}

	return HttpFetchResponse{
		status: resp.Status,
		lines:  lines,
	}
}
