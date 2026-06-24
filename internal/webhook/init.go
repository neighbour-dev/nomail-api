package webhook

import parser "nomail-api/internal/webhook/parser"

func Init() Registry {

	registry := NewRegistry()
	registry.Register(&parser.GithubParser{})
	registry.Register(&parser.FeishuParser{})
	return *registry
}
