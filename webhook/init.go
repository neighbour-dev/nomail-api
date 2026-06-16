package webhook

import parser "nomail-api/webhook/parser"

func Init() Registry {

	registry := NewRegistry()
	registry.Register(&parser.GithubParser{})
	registry.Register(&parser.FeishuParser{})
	return *registry
}
