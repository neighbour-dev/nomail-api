package webhook

import "strings"

type Registry struct {
	parsers map[string]Parser
}

func NewRegistry() *Registry {
	return &Registry{parsers: make(map[string]Parser)}
}

func (r *Registry) Register(parser Parser) {
	r.parsers[NormalizeProvider(parser.Provider())] = parser
}

func (r *Registry) Get(provider string) (Parser, bool) {
	p, ok := r.parsers[NormalizeProvider(provider)]
	return p, ok
}

func NormalizeProvider(provider string) string {
	return strings.ToLower(strings.TrimSpace(provider))
}
